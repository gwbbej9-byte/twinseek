use serde_json::Value;
use twinseek::scan::{scan_dir, testdata_dir};

#[test]
fn json_shape() {
    let report = scan_dir(&testdata_dir(), 1).expect("scan testdata");
    let v = serde_json::to_value(&report).expect("serialize");

    assert!(v["root"].is_string());
    assert_eq!(v["min_bytes"], 1);
    assert!(v["files_scanned"].as_u64().unwrap() >= 8);
    assert!(
        v["files_hashed"].as_u64().unwrap() < v["files_scanned"].as_u64().unwrap(),
        "unique sizes must not be hashed"
    );
    assert!(v["skipped_small"].as_u64().unwrap() >= 1);
    assert!(v["unique_files"].as_u64().unwrap() >= 2);
    assert_eq!(v["group_count"], 3);
    assert!(v["wasted_bytes"].as_u64().unwrap() > 0);
    assert!(v["groups"].is_array());
    assert_eq!(v["groups"].as_array().unwrap().len(), 3);

    for g in v["groups"].as_array().unwrap() {
        let hash = g["hash"].as_str().expect("hash string");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(g["size"].as_u64().unwrap() >= 1);
        let count = g["count"].as_u64().unwrap();
        assert!(count >= 2);
        let paths = g["paths"].as_array().expect("paths array");
        assert_eq!(paths.len(), count as usize);
        for p in paths {
            assert!(p.is_string());
            assert!(!p.as_str().unwrap().contains('\\'));
        }
        assert_eq!(
            g["wasted_bytes"].as_u64().unwrap(),
            g["size"].as_u64().unwrap() * (count - 1)
        );
    }
}

#[test]
fn json_roundtrip_keys() {
    let report = scan_dir(&testdata_dir(), 1).expect("scan");
    let text = serde_json::to_string(&report).expect("to json");
    let v: Value = serde_json::from_str(&text).expect("from json");
    for key in [
        "root",
        "min_bytes",
        "files_scanned",
        "files_hashed",
        "skipped_small",
        "unique_files",
        "group_count",
        "wasted_bytes",
        "groups",
    ] {
        assert!(v.get(key).is_some(), "missing key {key}");
    }
}
