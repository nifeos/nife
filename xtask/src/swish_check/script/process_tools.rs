//! **The process tools at the real prompt** (milestone 126, and milestone 282's `top`):
//! `ps`, `pgrep`, `top`, `free`, `vmstat`, `slabtop` and `uptime`. No tid, count or figure
//! is pinned; each row asserts the scope or the walk instead.
//!
//! Cut from the table in `script.rs` verbatim by milestone 900 (the swish-check script
//! splits by topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// The process-tool rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    // **`ps`, at the real prompt** (milestone 126 (the `procps` package)). The listing itself: a header, and at least the
    // row for `ps` itself, which is a member of the domain the progenitor spawned it into. Asserting the
    // header rather than a tid is deliberate: a tid is a generational name that moves with how many
    // jobs ran before it, and a gate that pinned one would be pinning the boot's history.
    line(1, "ps", &["TID  STATE"]),
    // **`ps` cannot see the machine**: the listing above holds only the shell's domain, where a
    // `/proc`-shaped `ps` would list every server and driver. The count is not asserted, because
    // whether `ps` walks before or after `wc` exists is a race (a survey is a snapshot,
    // notes/process-view.md); `kernel::user::survey_tests` asserts the confinement claim
    // deterministically on both ISAs. And `caps ps` prints the scope before anything is spawned,
    // the half Linux cannot express: there every program sees the same processes.
    line(0, "caps ps", &["cap 7  endpoint  domain"]),
    // **`pgrep`, at the real prompt** (milestone 126), and what is asserted is deliberately not the
    // tids. `pgrep` prints nothing but names, one per line, and a tid is a generational name that
    // moves with how many jobs ran before it; a gate that pinned one would be pinning the boot's
    // history. So the claim is made through the **second stream**, which is the same trick the four
    // `date 2>` lines above use: `pgrep`'s diagnostics carry a sentence in exactly three cases (the
    // walk was refused, the selector can never match, or nothing matched), so an *empty* second
    // stream is the assertion that none of the three happened. This one line fails if the progenitor endowed
    // no domain, if it endowed one the kernel refuses, or if the filter came back empty.
    line(1, "pgrep 2> pgrep.txt", &[]),
    line(1, "wc < pgrep.txt", &["0 0 0"]),
    // And the asymmetry, printed before anything is spawned, which is the whole of what milestone
    // 126 has instead of the `pgrep`-beside-`pkill` comparison it originally promised. Two phrases:
    // the right (`ENUMERATE`, not `READ`, so the finder cannot receive a death or collect a corpse)
    // and the sentence that says so in English. There is no `caps pkill` line to put beneath this
    // one, because a tid is a name and no method turns one into authority.
    line(
        0,
        "caps pgrep",
        &["cap 7  endpoint  domain   ENUMERATE", "do nothing to them"],
    ),
    // **`top`, at the real prompt** (milestone 282 (a thread's CPU time, and the `top` it makes possible)), and what is asserted is the two things a
    // boot cannot make untrue: the summary line's opening, which proves the ambient uptime counter
    // and the domain count both reached the output stream, and the `TIME(ms)` header, which proves
    // the **second** walk happened. A `top` whose CPU-time walk was refused would print the table
    // without that column and this line would fail, which is the one failure a
    // `pgrep`-style empty-diagnostics check could not catch.
    //
    // The tids and the figures are deliberately not pinned. A tid is a generational name that moves
    // with the boot's history, and a CPU figure is a measurement of a real machine; a gate that
    // pinned either would be pinning this boot rather than the program.
    line(
        1,
        "top",
        &[
            "up ",
            "threads: ",
            "machine: ",
            "% busy since boot",
            "TID  STATE     TIME(ms)",
        ],
    ),
    // And the second stream is empty, the same trick the `pgrep 2>` line above uses: `top`
    // complains in exactly the cases `ps` does, so an empty second stream says none of them
    // happened and the table above it is the domain.
    line(1, "top 2> top.txt", &[]),
    line(1, "wc < top.txt", &["0 0 0"]),
    // The scope, printed before anything is spawned. `top` holds `ps`'s three capabilities and not
    // one more: the ranking costs no authority, because the CPU figures are a second walk of the
    // same endpoint under the same right.
    line(
        0,
        "caps top",
        &[
            "cap 7  endpoint  domain   ENUMERATE",
            "cap 11 frame     machine  read-only",
        ],
    ),
    // **`free`, `vmstat` and `slabtop`, at the real prompt** (milestone 126, DECISIONS §225 (`free`
    // sees the machine and your share)). What each line proves is that a read reached the output:
    // `Mem:` is the kernel's machine statistics page mapped into the child and recognized, `Yours:`
    // is `MemoryRegion::USAGE` answering on the job budget's `ENUMERATE` view. No figure is pinned,
    // since the free count is a measurement of this boot. The empty second streams say neither
    // read was refused.
    line(
        1,
        "free",
        &["total        used        free", "Mem:  ", "Yours:"],
    ),
    line(1, "free 2> free.txt", &[]),
    line(1, "wc < free.txt", &["0 0 0"]),
    line(
        0,
        "caps free",
        &[
            "cap 11 frame     machine  read-only",
            "cap 12 region    share    ENUMERATE",
        ],
    ),
    line(
        1,
        "vmstat",
        &["    r       free      total     in     cs busy  id"],
    ),
    line(1, "vmstat 2> vmstat.txt", &[]),
    line(1, "wc < vmstat.txt", &["0 0 0"]),
    // `slabtop`'s `threads` row is the proof that the kernel summed the budget's subtree: the budget
    // itself holds no thread, every job region split from it holds at least one.
    line(1, "slabtop", &["job budget: ", "SPENT ON", "threads"]),
    line(1, "slabtop 2> slabtop.txt", &[]),
    line(1, "wc < slabtop.txt", &["0 0 0"]),
    // **`uptime`, at the real prompt** (milestone 126). No domain, no clock: the manifest is
    // `least_authority_demo`'s, because `monotonic_nanos` is granted to every process unconditionally
    // (kernel/src/arch/*/timer.rs's exception to DECISIONS §10). A green line here proves the
    // program was loaded, measured, granted its report endpoint and actually ran at EL0; the exact
    // elapsed time is not asserted because a real boot's timing is not this check's business.
    line(1, "uptime", &["up "]),
];
