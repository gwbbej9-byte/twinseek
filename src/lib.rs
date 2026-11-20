//! twinseek — size-group, 4 KiB partial, then SHA-256. Unique sizes are never hashed.

pub mod hasher;
pub mod scan;
pub mod tui;

pub use hasher::{
    digest_hex, hash_file, hash_file_prefix, hash_reader, hash_reader_prefix, HASH_CHUNK,
    PARTIAL_BYTES,
};
pub use scan::{format_bytes, scan_dir, DuplicateGroup, ScanReport};
