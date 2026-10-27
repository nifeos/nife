//! **The scripts swish-check types**: [`Line`] and its constructors (in `line`), and the
//! tables of lines with the substrings each answer must hold. The main table is cut into
//! topic files beside this one, and adding a check is adding a row in the topic file the
//! line belongs to; [`SWISH_CHECK_SCRIPT`] is the topic slices concatenated in table order.
//!
//! Cut out of `swish_check.rs` unchanged by milestone 847 (the swish-check harness is split
//! along its seams); cut into topic files by milestone 900 (the swish-check script splits by
//! topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 847's lane, 2026-10-11 UTC; §280 (the swish-check harness
//! splits into submodules, with its script in one)).

use std::sync::LazyLock;

pub(super) use after_reboot::{
    FROM_BASALT, ON_THE_DISK_TREE, REACH_LINES, RESOLVING_LINE, SWISH_CHECK_AFTER_REBOOT,
};
pub(super) use cycles::{CYCLES_AFTER, CYCLES_BEFORE};
pub(super) use line::{JOB_DID_NOT_RUN, Line, asked, interrupted_at_prompt, keys, line};

use super::*;

mod after_reboot;
mod clock_configuration;
mod cycles;
mod directories;
mod installer;
mod interrupts;
mod line;
mod navigation;
mod network;
mod owner;
mod packages;
mod pipes_files;
mod process_tools;
mod redirection;
mod std_exerciser;
mod std_programs;
mod words;

/// **What the keyboard boot types before it launches** (milestone 715 (provisional)): one `caps`,
/// for its census, because this is the only boot with a keyboard and so the only one that can show
/// the keyboard's three slots (26 to 28) are not the shell's either. Before 715 the progenitor
/// placed them there beside the gpu's four. See the same line in [`SWISH_CHECK_AFTER_REBOOT`].
pub(super) const SWISH_CHECK_KEYBOARD_BOOT: &[Line] =
    &[line(0, "caps", &["slots held: 0 1 2", " 21 62\n"])];

/// The text this gate types and what each line must answer. `None` is a line whose answer is
/// checked by a later one rather than by itself, which is every line that writes a file.
///
/// `hello world` plus the newline `echo` adds is twelve bytes; the append arm is exactly twice
/// that. The numbers are spelled out here rather than derived because this is a **boot** gate: if
/// the arithmetic and the boot were both wrong, deriving one from the other would hide it.
///
/// The rows live in the topic files under this one, and this concatenates them in table order,
/// so the leg still types one slice and the transcript is the one the single table typed.
/// `Line` is `Clone` for `concat`.
// Index exception (2026-10-15): the assembly is this index's table of contents, one line
// per topic child, and grows only as the `mod` list grows.
pub(super) static SWISH_CHECK_SCRIPT: LazyLock<Vec<Line>> = LazyLock::new(|| {
    let cycles = cycles::lines();
    [
        pipes_files::LINES,
        clock_configuration::LINES,
        navigation::LINES,
        process_tools::LINES,
        installer::LINES,
        std_programs::LINES,
        packages::LINES,
        owner::LINES,
        redirection::LINES,
        directories::LINES,
        words::LINES,
        network::LINES,
        interrupts::LINES,
        cycles.as_slice(),
        std_exerciser::LINES,
    ]
    .concat()
});

/// **The lines after which this gate presses `^C`**, once the shell has said the job is running.
/// Each runs until interrupted, so without the keystroke the prompt never comes back. A program
/// here is also one `every_spawnable_program_has_a_swish_check_line` would otherwise have to excuse.
pub(super) const SWISH_CHECK_INTERRUPTED: [&str; 4] =
    ["interrupt_heeder", "interrupt_ignorer", "wait", "fg %1"];

/// **The lines after which this gate presses `^Z`** (milestone 48 (job control)), once the shell
/// has said the job is running, as [`SWISH_CHECK_INTERRUPTED`] does for `^C`. The job is suspended
/// and the prompt comes back, so nothing else needs pressing.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/48-ctrl-z.
pub(super) const SWISH_CHECK_SUSPENDED: [&str; 1] = ["fg"];

/// What the shell prints once a supervised job is running and watched. Pressing `^C` before it
/// would still be counted (the shell's watermark catches an early one), but after it is the case a
/// person types, so it is the one this gate proves.
pub(super) const SWISH_CHECK_RUNNING: &str = "^C interrupts it.";

#[cfg(test)]
mod tests;
