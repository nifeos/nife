//! **What more than one layer's tests lean on**: the shipped document, a way to read what a writer
//! wrote, and a scheduler holding a clock.
//!
//! Name: provisional (milestone 877, 2026-10-12).

extern crate std;
use std::string::String;

use crate::Held;

/// **The shipped document is a specification, not a comment.** The retired `multicast_dns_config` reached
/// this shape first (notes/mdns.md) and the reason is the same: a configuration file nothing parses in CI is a file that
/// rots, and the first person to notice is the one whose machine will not boot.
pub(crate) const REFERENCE: &str = include_str!("../../../components/timetable.conf");

pub(crate) fn shown(f: impl FnOnce(&mut dyn FnMut(&[u8]))) -> String {
    let mut s = String::new();
    let mut out = |b: &[u8]| s.push_str(core::str::from_utf8(b).unwrap());
    f(&mut out);
    s
}

pub(crate) fn with_clock() -> Held {
    Held {
        clock: true,
        ..Held::default()
    }
}
