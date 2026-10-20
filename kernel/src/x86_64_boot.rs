//! **`x86_64`'s watch over the progenitor**, after `boot.rs`'s shared hand-over: the one step of
//! this port's boot the other two do not take. See notes/x86-port.md.
//!
//! **Temporary.** calef ruled so on 2026-10-12 (UTC), in §284 (the kernel crate root splits into
//! child modules, one per boot): this file is the port's route until it joins the shared path in
//! `boot.rs`. Its tour's claims are `self_test::run`'s checks (calef's ruling of 2026-10-19 (UTC) on
//! #2434). What is left is the watch, which goes when the gates that wait for its report wait for
//! the prompt instead (design/roadmap/0890-riscv64-and-x86-64-join-the-shared-boot-path.md).
//!
//! Name: provisional (milestone 876's lane, 2026-10-12 UTC).

use super::*;

/// **Watch the progenitor, bounded, and say what it saw** (milestone 182 (`x86_64`'s own
/// interactive-boot entry point)): `x86_64`'s half of `boot.rs`'s `hand_over`, which loads,
/// measures, endows and starts the first process through the same body on all three.
///
/// The boot thread has nothing else to do once the system is handed over. A progenitor still
/// running when the bound expires is the ordinary interactive outcome (the shell is up and waiting
/// for input), and a thread that left through a ring-3 fault left a record
/// (`arch::exceptions::last_user_fault`) the kernel's own fault report above it corroborates.
///
/// **The gates that type at this port's prompt wait for the line this prints**, which lands after
/// the prompt: `script/swish-check`'s `x86_64` leg (`xtask/src/swish_check/verdict.rs`) and the
/// install, rollback and confirm boots. Changing what it prints changes what they wait for.
///
/// Name: provisional (milestone 890 (riscv64 and `x86_64` join the shared boot path)'s lane, 2026-10-19 UTC).
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(super) fn watch_the_progenitor(tid: crate::thread::ThreadId, faults_before: usize) {
    use core::sync::atomic::Ordering;

    // Ten seconds of TCG is far past the progenitor's first console build, which is microseconds of
    // real time after it starts. The bound exists so the boot thread parks rather than waits.
    let deadline = arch::timer::now() + 10 * arch::timer::frequency();
    while sched::is_thread_present(tid) && arch::timer::now() < deadline {
        sched::yield_now();
    }

    let faults = arch::exceptions::USER_FAULTS
        .load(Ordering::Acquire)
        .wrapping_sub(faults_before);
    if sched::is_thread_present(tid) {
        println!(
            "nife x86_64: the progenitor is running at ring 3; {faults} of the processes it built \
             stopped on purpose."
        );
        // DECISIONS §121, reversed 2026-09-15, made the console server and the input driver
        // userspace processes holding a `PortRange` capability for COM1's ports (milestone 299), so
        // `swish`'s prompt above was transmitted by an `out` from ring 3, not by the kernel. A
        // holdover from §121's kernel-console era printed a "no prompt" line here; the prompt is now
        // the last thing above this, and `faults` above is zero because the two device drivers no
        // longer trap on first use.
        println!(
            "  prompt     : the shell above is served by userspace console and input drivers \
             holding COM1 as a port capability (milestone 299)."
        );
        return;
    }
    match arch::exceptions::last_user_fault() {
        Some((fault, addr)) if faults > 0 => println!(
            "nife x86_64: the progenitor ran at ring 3 and stopped ({fault:?} at {addr:#x}); \
             its fault report is above."
        ),
        _ => println!("nife x86_64: the progenitor left ring 3 without faulting."),
    }
}
