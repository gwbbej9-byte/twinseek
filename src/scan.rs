//! Directory walk + size → 4 KiB partial → full SHA-256 grouping.
//! Unique sizes are never hashed. Size-collision hashes run on rayon.

use crate::hasher::{digest_hex, hash_file, hash_file_prefix, PARTIAL_BYTES};
use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub size: u64,
    pub count: usize,
    pub wasted_bytes: u64,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanReport {
    pub root: String,
    pub min_bytes: u64,
    pub files_scanned: usize,
    pub files_hashed: usize,
    pub skipped_small: usize,
    pub unique_files: usize,
    pub group_count: usize,
    pub wasted_bytes: u64,
    pub groups: Vec<DuplicateGroup>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

pub fn format_bytes(n: u64) -> String {
    const K: u64 = 1024;
    if n < K {
        format!("{n} B")
    } else if n < K * K {
        format!("{:.1} KiB", n as f64 / K as f64)
    } else if n < K * K * K {
        format!("{:.1} MiB", n as f64 / (K * K) as f64)
    } else {
        format!("{:.1} GiB", n as f64 / (K * K * K) as f64)
    }
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn hash_paths_prefix(
    paths: Vec<PathBuf>,
    files_hashed: &mut usize,
    errors: &mut Vec<String>,
) -> Vec<([u8; 32], PathBuf)> {
    let results: Vec<_> = paths
        .into_par_iter()
        .map(|path| {
            hash_file_prefix(&path, PARTIAL_BYTES).map(|digest| (digest, path))
        })
        .collect();
    let mut hits = Vec::new();
    for result in results {
        match result {
            Ok(hit) => {
                *files_hashed += 1;
                hits.push(hit);
            }
            Err(e) => errors.push(e.to_string()),
        }
    }
    hits
}

fn hash_paths_full(paths: Vec<PathBuf>, errors: &mut Vec<String>) -> Vec<([u8; 32], PathBuf)> {
    let results: Vec<_> = paths
        .into_par_iter()
        .map(|path| hash_file(&path).map(|digest| (digest, path)))
        .collect();
    let mut hits = Vec::new();
    for result in results {
        match result {
            Ok(hit) => hits.push(hit),
            Err(e) => errors.push(e.to_string()),
        }
    }
    hits
}

fn push_group(
    groups: &mut Vec<DuplicateGroup>,
    root: &Path,
    digest: [u8; 32],
    size: u64,
    mut paths: Vec<PathBuf>,
) {
    paths.sort();
    let paths: Vec<String> = paths.iter().map(|p| rel_path(root, p)).collect();
    let count = paths.len();
    groups.push(DuplicateGroup {
        hash: digest_hex(&digest),
        size,
        count,
        wasted_bytes: size.saturating_mul((count as u64).saturating_sub(1)),
        paths,
    });
}

pub fn scan_dir(root: &Path, min_bytes: u64) -> Result<ScanReport> {
    if !root.exists() {
        anyhow::bail!("path not found: {}", root.display());
    }
    if !root.is_dir() {
        anyhow::bail!("not a directory: {}", root.display());
    }

    let root_disp = root.to_string_lossy().replace('\\', "/");
    let mut files_scanned = 0usize;
    let mut skipped_small = 0usize;
    let mut files_hashed = 0usize;
    let mut unique_files = 0usize;
    let mut errors = Vec::new();
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    let walker = WalkDir::new(root).follow_links(false).into_iter();
    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                errors.push(e.to_string());
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                errors.push(format!("{}: {e}", entry.path().display()));
                continue;
            }
        };
        let size = meta.len();
        if size < min_bytes {
            skipped_small += 1;
            continue;
        }
        files_scanned += 1;
        by_size.entry(size).or_default().push(entry.into_path());
    }

    let mut groups = Vec::new();
    for (size, paths) in by_size {
        if paths.len() < 2 {
            unique_files += paths.len();
            continue;
        }

        let prefix_hits = hash_paths_prefix(paths, &mut files_hashed, &mut errors);
        let mut by_prefix: HashMap<[u8; 32], Vec<PathBuf>> = HashMap::new();
        for (digest, path) in prefix_hits {
            by_prefix.entry(digest).or_default().push(path);
        }

        for (prefix, prefix_paths) in by_prefix {
            if prefix_paths.len() < 2 {
                unique_files += prefix_paths.len();
                continue;
            }
            if size <= PARTIAL_BYTES as u64 {
                push_group(&mut groups, root, prefix, size, prefix_paths);
                continue;
            }

            let full_hits = hash_paths_full(prefix_paths, &mut errors);
            let mut by_full: HashMap<[u8; 32], Vec<PathBuf>> = HashMap::new();
            for (digest, path) in full_hits {
                by_full.entry(digest).or_default().push(path);
            }
            for (digest, full_paths) in by_full {
                if full_paths.len() < 2 {
                    unique_files += full_paths.len();
                    continue;
                }
                push_group(&mut groups, root, digest, size, full_paths);
            }
        }
    }

    groups.sort_by(|a, b| {
        b.wasted_bytes
            .cmp(&a.wasted_bytes)
            .then_with(|| b.count.cmp(&a.count))
            .then_with(|| a.hash.cmp(&b.hash))
    });

    let wasted_bytes = groups.iter().map(|g| g.wasted_bytes).sum();
    let group_count = groups.len();

    Ok(ScanReport {
        root: root_disp,
        min_bytes,
        files_scanned,
        files_hashed,
        skipped_small,
        unique_files,
        group_count,
        wasted_bytes,
        groups,
        errors,
    })
}

pub fn print_human(report: &ScanReport) {
    println!("twinseek  {}  min-bytes={}", report.root, report.min_bytes);
    println!(
        "scanned {}  hashed {}  unique {}  groups {}  wasted {}  skipped {}",
        report.files_scanned,
        report.files_hashed,
        report.unique_files,
        report.group_count,
        format_bytes(report.wasted_bytes),
        report.skipped_small
    );
    if report.groups.is_empty() {
        println!("no duplicates");
        return;
    }
    println!();
    for (i, g) in report.groups.iter().enumerate() {
        println!(
            "#{}  {} copies  {} each  wasted {}  sha256:{}",
            i + 1,
            g.count,
            format_bytes(g.size),
            format_bytes(g.wasted_bytes),
            g.hash
        );
        for p in &g.paths {
            println!("    {p}");
        }
        println!();
    }
    for e in &report.errors {
        eprintln!("warn: {e}");
    }
}

pub fn testdata_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hasher_groups_copies() {
        let report = scan_dir(&testdata_dir(), 1).expect("scan testdata");
        let notes = report
            .groups
            .iter()
            .find(|g| g.paths.iter().any(|p| p.ends_with("notes.txt")))
            .expect("notes group");
        assert_eq!(notes.count, 3);
        assert_eq!(notes.paths.len(), 3);
        assert!(notes.paths.iter().any(|p| p.ends_with("notes.copy.txt")));
        assert!(notes.paths.iter().any(|p| p.contains("archive/notes.txt")));

        let twins = report
            .groups
            .iter()
            .find(|g| g.paths.iter().any(|p| p.contains("twins/")))
            .expect("twins group");
        assert_eq!(twins.count, 2);
        assert_eq!(twins.size, 64);

        let shots = report
            .groups
            .iter()
            .find(|g| g.paths.iter().any(|p| p.contains("photos/")))
            .expect("photos group");
        assert_eq!(shots.count, 2);
    }

    #[test]
    fn unique_files_stay_alone() {
        let report = scan_dir(&testdata_dir(), 1).expect("scan testdata");
        for g in &report.groups {
            assert!(g.count >= 2, "group must not contain singles: {g:?}");
            for p in &g.paths {
                assert!(
                    !p.contains("unique/"),
                    "unique file leaked into a group: {p}"
                );
            }
        }
        assert!(report.unique_files >= 2);
    }

    #[test]
    fn min_bytes_skips_small() {
        let all = scan_dir(&testdata_dir(), 1).expect("scan");
        assert!(
            all.skipped_small >= 1,
            "empty.dat should be skipped at min-bytes=1"
        );

        let none = scan_dir(&testdata_dir(), 10_000).expect("scan large min");
        assert_eq!(none.group_count, 0);
        assert_eq!(none.files_scanned, 0);
        assert!(none.skipped_small >= all.files_scanned + all.skipped_small);
    }

    #[test]
    fn unique_sizes_are_not_hashed() {
        let report = scan_dir(&testdata_dir(), 1).expect("scan testdata");
        assert!(
            report.files_hashed < report.files_scanned,
            "unique-size files must skip hashing: hashed={} scanned={}",
            report.files_hashed,
            report.files_scanned
        );
        assert_eq!(report.group_count, 3);
        assert_eq!(report.files_scanned, 9);
        assert_eq!(report.files_hashed, 8);
    }

    #[test]
    fn partial_then_full_splits_same_size_prefix() {
        use std::fs;

        let dir = std::env::temp_dir().join("twinseek-partial-split");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        let a = vec![0x11u8; 5000];
        let mut b = vec![0x11u8; 5000];
        b[PARTIAL_BYTES] = 0x22;
        fs::write(dir.join("a.bin"), &a).expect("write a");
        fs::write(dir.join("b.bin"), &b).expect("write b");
        fs::write(dir.join("solo.bin"), b"solo-unique-size").expect("write solo");

        let report = scan_dir(&dir, 1).expect("scan temp");
        assert_eq!(report.group_count, 0);
        assert_eq!(report.files_scanned, 3);
        assert_eq!(report.files_hashed, 2);
        assert!(report.files_hashed < report.files_scanned);
        assert_eq!(report.unique_files, 3);
    }
}
