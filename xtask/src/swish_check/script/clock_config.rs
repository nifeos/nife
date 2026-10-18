//! **The clock and the configuration page, from the prompt** (milestones 51 and 47):
//! `date`, `printenv`, and the `caps` previews that prove the wiring the words depend on.
//!
//! Cut from the table in `script.rs` verbatim by milestone 900 (the swish-check script
//! splits by topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// The clock-and-configuration rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    // **The clock, from the prompt** (milestone 51's wiring). The answer cannot be a constant, so
    // the check is the one word that separates a real time from both ways of not having one:
    // `Format::Human` ends in the offset's name and the two unknown-clock sentences ("the machine
    // has no clock it believes" / "this process holds no clock capability") contain no `UTC` at
    // all. So this fails if the clock service did not run, if the kernel granted the progenitor no page, if
    // the progenitor did not endow `date`, or if `date` was handed a page nobody published to.
    line(1, "date", &["UTC"]),
    // And the visibility surface agrees with the wiring. `caps` is the only thing in this system
    // that claims to print a process's whole authority, so a clock endowed and not printed would
    // make that claim false. Its wording is host-tested; this proves the wording is about a
    // capability the boot really moves.
    line(0, "caps date", &["cap 1  frame     clock"]),
    // **The inert-configuration page, from the prompt** (milestone 47's environment-variable fork,
    // DECISIONS §111). `date`'s own proof, one manifest field over: this fails if the kernel
    // granted the progenitor no config page, if the progenitor did not endow `printenv`, or if the page's validated
    // domains rejected the boot's own defaults, none of which a host test can see, because
    // `crates/system_initializer`'s spawn wiring is provable only against a real progenitor
    // (this file's module doc names `script/swish-check` as exactly that gate).
    line(1, "printenv", &["TZ=UTC", "LANG=C", "TERM=dumb"]),
    // And the visibility surface agrees with the wiring, `date`'s own check repeated for `config`:
    // `caps` claims to print a process's whole authority, so a config page endowed and not printed
    // would make that claim false.
    //
    // **And the values, before anything runs** (DECISIONS §111 (inert configuration is a validated
    // page)'s preview, milestone 47 (navigation and naming)). The shell prints them from its own
    // read-only view of the frame `printenv` was just handed, so this fails if the progenitor did
    // not place the view at `grant_plan::SHELL_CONFIGURATION_SLOT`, did not map it at `SHELL_CONFIGURATION_VIRTUAL_ADDRESS`,
    // or the shell's probe missed it (it then says it "cannot show their values" and none of the
    // three appears).
    line(
        0,
        "caps printenv",
        &[
            "cap 1  frame     config",
            "the page this shell reads too",
            "TZ=UTC",
            "LANG=C",
            "TERM=dumb",
        ],
    ),
];
