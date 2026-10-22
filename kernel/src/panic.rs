//! What happens when the kernel panics.
//!
//! `std` provides a panic handler, so you have never had to think about this.
//! Without `std` you must write one, and it forces a real question: there is no
//! process to kill, no stderr, no shell to return to. What *should* happen?
//!
//! Our answer: say what went wrong, then stop the machine. Under `cargo test`, a
//! panic is a failing test, so we exit QEMU with a failure status instead.

// Both imports feed the handler below and nothing else, so a `cfg(kani)` build (which has no
// handler; see the comment on it) has no use for them.
#[cfg(not(kani))]
use core::panic::PanicInfo;

#[cfg(not(kani))]
use crate::{arch, println};

// **Absent under `cfg(kani)`, and that is a hole this milestone opened on purpose.** Kani links
// `std` into the crate it proves, and `std` already defines the `panic_impl` lang item, so a second
// definition here is a hard compile error rather than a choice. The shipping kernel is unaffected:
// `cargo kani` is the only thing that ever sets this cfg. What it costs a reader is that a proof
// cannot observe this handler's behaviour, so nothing proved about the kernel says anything about
// what happens after a panic. See notes/kernel-proofs.md.
#[cfg(not(kani))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // SAFETY: we are dying. If we panicked while holding the console lock (a fault taken
    // inside a print), taking it again here would hang and we would lose the one message
    // that matters. Break it open. See sync.rs.
    // Forget what we thought we held, BEFORE trying to print.
    //
    // If we panicked while holding the console lock, HELD_RANK is 10, and the print below would
    // try to take rank 10 again. `10 < 10` is false, so the lock-ranking assertion would fire a
    // violation *inside the panic handler* and we would lose the original message to a
    // recursive panic. The bookkeeping is a debugging aid; it must never be the thing that
    // stops us saying what went wrong.
    //
    // SAFETY: we are dying. See sync.rs.
    unsafe {
        crate::sync::force_reset_ranks();
        crate::console::force_unlock();
    };
    // A screen a userspace terminal was painting comes back to the kernel, so a machine with no
    // serial port still shows why it stopped (the shell on the firmware screen). A no-op on every
    // machine whose screen is still the kernel's or that has none.
    crate::console::reclaim_screen_for_panic();
    // Lines the log service had not yet printed go out now, and nothing is held back after this
    // (milestone 342 (the kernel and the `console` server drive one UART from two address
    // spaces), the panic escape of §175 (where the kernel's own output goes).
    crate::console::enter_panic();

    // A repeated run names its failing iteration first, so a console that drops the panic's
    // second line still carries it (kernel/src/testing.rs, `TEST_REPEAT`).
    #[cfg(any(test, feature = "system_tests"))]
    crate::testing::note_repeat_failure("below");
    println!();
    println!("[PANIC] {info}");
    crate::stack::warn_if_smashed();

    #[cfg(any(test, feature = "system_tests"))]
    arch::semihosting::exit(arch::semihosting::EXIT_FAILURE);

    #[cfg(not(any(test, feature = "system_tests")))]
    arch::halt(arch::HaltReason::panicked(info))
}
