//! The manifest fields both readers print as `debugfs_manifest.py` does: the content hash, hex,
//! a path as text or hex, and an extended attribute's namespace prefix. The direct reader
//! (`main.rs`) and the server path (`bin/ext4_oracle_server.rs`) both link this library.
//!
//! Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only. calef names
//! files.
use rsext4::XattrNamespace;

/// FNV-1a-64, the hash `debugfs_manifest.py` prints for content and attribute values.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
pub fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Lowercase hexadecimal, two digits a byte.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A path as text when it is UTF-8, else in hex.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
pub fn hex_or_text(p: &[u8]) -> String {
    String::from_utf8(p.to_vec()).unwrap_or_else(|_| hex(p))
}

/// The prefix an attribute's name carries in its namespace, or its whole name for the two POSIX
/// access-control lists, which have no name of their own.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
pub fn namespace(attribute_namespace: XattrNamespace) -> String {
    match attribute_namespace {
        XattrNamespace::User => "user.".into(),
        XattrNamespace::Trusted => "trusted.".into(),
        XattrNamespace::Security => "security.".into(),
        XattrNamespace::System => "system.".into(),
        XattrNamespace::PosixAclAccess => "system.posix_acl_access".into(),
        XattrNamespace::PosixAclDefault => "system.posix_acl_default".into(),
        other => format!("{other:?}."),
    }
}
