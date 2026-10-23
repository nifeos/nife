//! **`vmstat`: the machine's counters since boot** (§225 (`free` sees the machine and your share)).
//!
//! The logic of `components/src/vmstat.rs`: averages since boot, from the machine statistics page.
//! Its columns: notes/process-view/the-machine-and-your-share.md, "`vmstat`'s columns".
//!
//! # BUGS
//!
//! No interval and no count (a prompt program holds no timer); `busy` is `us` and `sy` together;
//! each figure is read a moment apart from the others.
//!
//! Name: ratified 2026-09-27 (calef, #1360's table): upstream `procps`'s.

#![cfg_attr(not(test), no_std)]

use machine_statistics_contract::Snapshot;
use number_writer::write_right;

/// The two header lines and the one report line, or nothing if `machine` is `None`, since a
/// `vmstat` that cannot see the machine has nothing to tabulate. The caller says why on its
/// second stream.
pub fn write_report(machine: Option<&Snapshot>, uptime_nanos: u64, out: &mut dyn FnMut(&[u8])) {
    let Some(s) = machine else { return };
    let secs = (uptime_nanos / 1_000_000_000).max(1);
    let (busy, idle) = (s.busy_ticks(), s.idle_ticks());
    let ticks = busy + idle;
    let busy_pct = (busy * 100).checked_div(ticks).unwrap_or(0);
    let idle_pct = if ticks == 0 { 0 } else { 100 - busy_pct };

    out(b"procs ------memory (KiB)------ --system-- -cpu-\n");
    out(b"    r       free      total     in     cs busy  id\n");
    for (v, width) in [
        (s.runnable(), 5),
        (s.free_bytes() / 1024, 11),
        (s.total_bytes() / 1024, 11),
        (s.interrupts() / secs, 7),
        (s.context_switches() / secs, 7),
        (busy_pct, 5),
        (idle_pct, 4),
    ] {
        write_right(v, width, out);
    }
    out(b"\n");
}

/// The one complaint `vmstat` can have, for the second stream.
pub fn write_diagnostics(granted: bool, machine: Option<&Snapshot>, out: &mut dyn FnMut(&[u8])) {
    if machine.is_some() {
        return;
    }
    if granted {
        out(b"vmstat: the machine statistics page is not one this program recognizes\n");
    } else {
        out(b"vmstat: the machine's owner has not granted the machine statistics page\n");
    }
}

#[cfg(test)]
mod tests {
    use machine_statistics_contract::{WORDS, build_header, word};

    use super::*;

    fn page() -> Snapshot {
        let mut w = [0u64; WORDS];
        w[..word::LINE].copy_from_slice(&build_header(4096, 100));
        w[word::TOTAL_FRAMES] = 32768;
        w[word::FREE_FRAMES] = 27648;
        for (cpu, busy, idle, intr, cs, r) in [(0, 10, 90, 520, 150, 1), (2, 14, 86, 520, 200, 0)] {
            let at = word::cpu(cpu);
            w[at + word::ONLINE] = 1;
            w[at + word::BUSY_TICKS] = busy;
            w[at + word::IDLE_TICKS] = idle;
            w[at + word::INTERRUPTS] = intr;
            w[at + word::CONTEXT_SWITCHES] = cs;
            w[at + word::RUNNABLE] = r;
        }
        Snapshot::from_words(&w).unwrap()
    }

    fn shown(m: Option<&Snapshot>, nanos: u64) -> String {
        let mut v = Vec::new();
        write_report(m, nanos, &mut |b| v.extend_from_slice(b));
        String::from_utf8(v).unwrap()
    }

    #[test]
    fn the_report_is_the_header_and_one_line_of_averages_since_boot() {
        assert_eq!(
            shown(Some(&page()), 10_000_000_000),
            "procs ------memory (KiB)------ --system-- -cpu-\n\
             \x20   r       free      total     in     cs busy  id\n\
             \x20   1     110592     131072    104     35   12  88\n"
        );
    }

    /// The first second of a boot must not divide by zero, and must not print a rate a thousand
    /// times too large either: under a second counts as one.
    #[test]
    fn a_boot_under_a_second_old_counts_as_one_second() {
        let line = shown(Some(&page()), 400_000_000);
        assert!(line.contains("   1040    350"), "{line}");
    }

    #[test]
    fn no_machine_page_prints_nothing_and_says_which_kind_of_nothing() {
        assert_eq!(shown(None, 0), "");
        let mut d = Vec::new();
        write_diagnostics(false, None, &mut |b| d.extend_from_slice(b));
        assert!(
            String::from_utf8(d)
                .unwrap()
                .contains("owner has not granted")
        );
    }
}
