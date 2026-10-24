//! **The host side of milestone 121 (`ripgrep` on nife: enumeration as a capability)'s walk
//! pricing**: stage the fixture tree in a scratch directory and price a walk over it, with the
//! very function nife's `std_exerciser` runs through a granted directory. On macOS this is a
//! reference point, not a competitor (APFS on NVMe against nife under an emulator or hypervisor).
//! `DIR` defaults to a fresh directory under the host's temp dir, and is removed afterwards.
//!
//! ```text
//! cargo run --release -p walk_pricing --example host [DIR]
//! ```

//! # BUGS
//!
//! - The matched-tier Linux run stages on tmpfs, which matters for a cold walk; see
//!   notes/walk-pricing.md, "What the instrument does not see".

fn main() {
    let dir = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("walk-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir);
    walk_pricing::stage(&dir).expect("could not stage the priced tree");
    let price = walk_pricing::price(&dir).expect("the walk failed");
    for line in walk_pricing::report(&price) {
        println!("{line}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
