//! **`free`: the machine's memory, and this prompt's share of it** (milestone 126 (the `procps` package), DECISIONS §225
//! (`free` sees the machine and your share)).
//!
//! The program's logic, lifted out so it runs on the host; `components/src/free.rs` is the two
//! reads and the two sinks. `Mem:` is the machine, from the machine statistics page
//! (`crates/machine_statistics_contract`), which the owner can withhold. `Yours:` is the job pool
//! this prompt's jobs are carved from, read through `MemoryRegion::USAGE` on a view narrowed to
//! `ENUMERATE`: the limit that binds a program started here. Why both:
//! notes/process-view/the-machine-and-your-share.md.

//! # EXAMPLES
//!
//! ```text
//! $ free
//!               total        used        free
//! Mem:         131072       20480      110592
//! Yours:         2048         320        1728
//! ```
//!
//! Figures are KiB. Without the machine page `Yours:` prints alone, and the second stream says why.

//! # BUGS
//!
//! - No `shared`, `buff/cache` or `available` columns: there is no page cache to report.
//! - No `Swap:` line: nife refuses paging out (calef, 2026-09-26, #1356), and a zero would say swap
//!   exists and is empty.
//! - `Yours:` counts `free` itself, the way `ps` lists itself.
//! - The two lines are read at slightly different moments, and neither is a snapshot of the other.
//! - **`Yours:` is the machine's whole job pool (`jobs_ut`), not one prompt's**: one session sees
//!   pages another spent, an accepted counting channel (a number, never a name).
//!
//! Name: ratified 2026-09-27 (calef, #1360's table): upstream `procps`'s name for the
//! program a reader types to ask this.

#![cfg_attr(not(test), no_std)]

use machine_statistics_contract::Snapshot;
use number_writer::{write_right, write_signed};

/// The job pool this prompt's jobs are carved from (the machine's one pool; see BUGS), as
/// `MemoryRegion::USAGE` answered: pages held, and pages spent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Share {
    /// `abi::usage::SIZE`.
    pub pages: u64,
    /// `abi::usage::COMMITTED`.
    pub committed: u64,
}

/// Why there is no `Mem:` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineRefusal {
    /// No capability at the slot: the owner withheld it.
    Withheld,
    /// A capability, and a page without the magic.
    Unrecognized,
}

/// What the machine read found: the page, or why not.
pub type Machine<'a> = Result<&'a Snapshot, MachineRefusal>;

/// Why there is no `Yours:` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareRefusal {
    /// No view of the budget was granted.
    NotHeld,
    /// The kernel refused the question, with this error code.
    Refused(i64),
}

/// **The complaints, for the second stream**, one line each. Written before the report, under
/// DECISIONS §67 (a program's second stream is a declaration)'s order.
pub fn write_diagnostics(
    machine: Machine<'_>,
    share: Result<Share, ShareRefusal>,
    out: &mut dyn FnMut(&[u8]),
) {
    match machine {
        Ok(_) => {}
        Err(MachineRefusal::Withheld) => {
            out(b"free: the machine's owner has not granted the machine statistics page\n");
        }
        Err(MachineRefusal::Unrecognized) => {
            out(b"free: the machine statistics page is not one this program recognizes\n");
        }
    }
    match share {
        Ok(_) => {}
        Err(ShareRefusal::NotHeld) => out(b"free: this process holds no view of a job budget\n"),
        Err(ShareRefusal::Refused(code)) => {
            out(b"free: the kernel refused to say what the job budget spent (error ");
            write_signed(code, out);
            out(b")\n");
        }
    }
}

/// **The table**: a header and whichever of the two lines could be read. Nothing at all when
/// neither could, so `free > out.txt` on a machine that shows it nothing is an empty file rather
/// than a header over no rows.
pub fn write_report(
    machine: Machine<'_>,
    share: Result<Share, ShareRefusal>,
    out: &mut dyn FnMut(&[u8]),
) {
    let seen = machine.ok();
    if seen.is_none() && share.is_err() {
        return;
    }
    out(b"              total        used        free\n");
    if let Some(s) = seen {
        let total = s.total_bytes() / 1024;
        let free = s.free_bytes() / 1024;
        row(b"Mem:  ", total, total.saturating_sub(free), free, out);
    }
    if let Ok(sh) = share {
        let kib = address_space_map::PAGE / 1024;
        let total = sh.pages * kib;
        let used = sh.committed.min(sh.pages) * kib;
        row(b"Yours:", total, used, total - used, out);
    }
}

fn row(label: &[u8], total: u64, used: u64, free: u64, out: &mut dyn FnMut(&[u8])) {
    out(label);
    for v in [total, used, free] {
        write_right(v, 12, out);
    }
    out(b"\n");
}

#[cfg(test)]
mod tests {
    use machine_statistics_contract::{WORDS, build_header, word};

    use super::*;

    fn machine(total: u64, free: u64) -> Snapshot {
        let mut w = [0u64; WORDS];
        w[..word::LINE].copy_from_slice(&build_header(4096, 100));
        w[word::TOTAL_FRAMES] = total;
        w[word::FREE_FRAMES] = free;
        Snapshot::from_words(&w).unwrap()
    }

    fn shown(m: Machine<'_>, s: Result<Share, ShareRefusal>) -> (String, String) {
        let (mut out, mut diag) = (Vec::new(), Vec::new());
        write_diagnostics(m, s, &mut |b| diag.extend_from_slice(b));
        write_report(m, s, &mut |b| out.extend_from_slice(b));
        (
            String::from_utf8(out).unwrap(),
            String::from_utf8(diag).unwrap(),
        )
    }

    #[test]
    fn both_lines_read_as_upstream_free_does_plus_yours() {
        let (out, diag) = shown(
            Ok(&machine(32768, 27648)),
            Ok(Share {
                pages: 512,
                committed: 80,
            }),
        );
        assert_eq!(
            out,
            "              total        used        free\n\
             Mem:        131072       20480      110592\n\
             Yours:        2048         320        1728\n"
        );
        assert_eq!(diag, "");
    }

    /// §225's switch, seen from the reader: a withheld page is said, never printed as zeroes.
    #[test]
    fn a_withheld_machine_page_is_a_sentence_and_not_a_machine_of_zero_bytes() {
        let (out, diag) = shown(
            Err(MachineRefusal::Withheld),
            Ok(Share {
                pages: 512,
                committed: 80,
            }),
        );
        assert!(!out.contains("Mem:"), "{out}");
        assert!(out.contains("Yours:"), "{out}");
        assert!(diag.contains("owner has not granted"), "{diag}");
    }

    #[test]
    fn nothing_readable_prints_no_table_at_all() {
        let (out, diag) = shown(
            Err(MachineRefusal::Unrecognized),
            Err(ShareRefusal::Refused(-3)),
        );
        assert_eq!(out, "");
        assert!(diag.contains("not one this program recognizes"), "{diag}");
        assert!(diag.contains("(error -3)"), "{diag}");
    }

    #[test]
    fn a_budget_reporting_more_spent_than_held_does_not_underflow() {
        let (out, _) = shown(
            Err(MachineRefusal::Withheld),
            Ok(Share {
                pages: 4,
                committed: 9,
            }),
        );
        assert!(
            out.contains("Yours:          16          16           0"),
            "{out}"
        );
    }

    #[test]
    fn a_refusal_code_of_zero_prints_without_a_sign() {
        let (_, diag) = shown(Err(MachineRefusal::Withheld), Err(ShareRefusal::Refused(0)));
        assert!(diag.contains("(error 0)"), "{diag}");
    }
}
