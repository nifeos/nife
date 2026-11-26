//! What the oracle binaries share: the manifest fields the two readers print ([`manifest`]), the
//! CRC32C path every binary takes before it mounts ([`crc32c_path`]), and the server path's
//! `rsext4` adapter ([`volume`]), which the ext4 fuzz targets (`tools/ext4_oracle_fuzz`) link too.
//!
//! Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-oracle-three-architectures,
//! for the index the library's root became; the library keeps the name `ext4_oracle_manifest`.
//! calef names files.

pub mod crc32c_path;
pub mod manifest;
pub mod volume;

pub use manifest::{fnv, hex, hex_or_text, namespace};
