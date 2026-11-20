//! Chunked SHA-256. Digest never depends on path — only on file bytes.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Read size for hashing. 64 KiB keeps syscall count down on large files.
pub const HASH_CHUNK: usize = 64 * 1024;

/// First-pass sample for size-collision candidates (fdupes-class).
pub const PARTIAL_BYTES: usize = 4 * 1024;

pub fn hash_reader<R: Read>(mut reader: R) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; HASH_CHUNK];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().into())
}

pub fn hash_reader_prefix<R: Read>(mut reader: R, max_bytes: usize) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    let mut remaining = max_bytes;
    let mut buf = vec![0u8; HASH_CHUNK.min(max_bytes.max(1))];
    while remaining > 0 {
        let want = buf.len().min(remaining);
        let n = reader.read(&mut buf[..want])?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        remaining -= n;
    }
    Ok(hasher.finalize().into())
}

pub fn hash_file(path: &Path) -> Result<[u8; 32]> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::with_capacity(HASH_CHUNK, file);
    hash_reader(reader).with_context(|| format!("read {}", path.display()))
}

pub fn hash_file_prefix(path: &Path, max_bytes: usize) -> Result<[u8; 32]> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::with_capacity(HASH_CHUNK.min(max_bytes.max(1)), file);
    hash_reader_prefix(reader, max_bytes).with_context(|| format!("read {}", path.display()))
}

pub fn digest_hex(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for &b in digest {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Cursor;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("twinseek-hasher-tests");
        fs::create_dir_all(&dir).expect("temp dir");
        dir.join(name)
    }

    #[test]
    fn same_bytes_same_digest() {
        let payload = b"hello twinseek payload";
        let a = scratch("copy-a.bin");
        let b = scratch("copy-b.bin");
        fs::write(&a, payload).unwrap();
        fs::write(&b, payload).unwrap();
        assert_eq!(hash_file(&a).unwrap(), hash_file(&b).unwrap());
        assert_eq!(digest_hex(&hash_file(&a).unwrap()).len(), 64);
    }

    #[test]
    fn different_bytes_different_digest() {
        let a = scratch("diff-a.bin");
        let b = scratch("diff-b.bin");
        fs::write(&a, b"alpha").unwrap();
        fs::write(&b, b"beta!").unwrap();
        assert_ne!(hash_file(&a).unwrap(), hash_file(&b).unwrap());
    }

    #[test]
    fn empty_reader_is_sha256_of_empty() {
        let digest = hash_reader(Cursor::new(b"")).unwrap();
        assert_eq!(
            digest_hex(&digest),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn prefix_matches_reader_cap() {
        let data = vec![0xABu8; PARTIAL_BYTES + 80];
        let prefix = hash_reader_prefix(Cursor::new(data.clone()), PARTIAL_BYTES).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&data[..PARTIAL_BYTES]);
        let expected: [u8; 32] = hasher.finalize().into();
        assert_eq!(prefix, expected);
        let full = hash_reader(Cursor::new(data)).unwrap();
        assert_ne!(prefix, full);
    }

    #[test]
    fn chunked_matches_one_shot() {
        let data = vec![0x5Au8; HASH_CHUNK * 2 + 17];
        let chunked = hash_reader(Cursor::new(data.clone())).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let one_shot: [u8; 32] = hasher.finalize().into();
        assert_eq!(chunked, one_shot);
    }
}
