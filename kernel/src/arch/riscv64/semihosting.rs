//! The test harness's exit, `arch::semihosting::exit`, and the SBI SRST reboot.
//!
//! On QEMU `virt` the exit writes the `sifive_test` finisher; under the `board` feature it prints a
//! `NIFE-TEST-EXIT:` marker and asks SBI SRST to shut down. [`reboot`] is the same SRST call with
//! the cold-reboot type, and returns when firmware refuses. See notes/riscv-port/firmware.md.

// Only the test harness and the test-mode panic arm reach the exit, so the gates are `not(test)`
// rather than a blanket allow, keeping the test build on the dead-code gate.

/// The harness's success code (a passing exit).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) const EXIT_SUCCESS: u32 = 0;
/// The harness's failure code (any non-zero exit).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) const EXIT_FAILURE: u32 = 1;

// ---- QEMU `virt` exit: the `sifive_test` finisher ----

/// The `sifive_test` finisher's physical address on QEMU's `virt`, reached through the direct map.
#[cfg_attr(
    all(not(any(test, feature = "system_tests")), not(feature = "board")),
    allow(dead_code)
)]
#[cfg(not(feature = "board"))]
const SIFIVE_TEST_PHYS: u64 = 0x10_0000;
/// Write this to exit QEMU with status 0.
#[cfg_attr(
    all(not(any(test, feature = "system_tests")), not(feature = "board")),
    allow(dead_code)
)]
#[cfg(not(feature = "board"))]
const FINISHER_PASS: u32 = 0x5555;
/// Base value for a failing exit; the caller's code is packed into the high half.
#[cfg_attr(
    all(not(any(test, feature = "system_tests")), not(feature = "board")),
    allow(dead_code)
)]
#[cfg(not(feature = "board"))]
const FINISHER_FAIL: u32 = 0x3333;

/// Terminate the QEMU guest with `code` (0 = success). Drives the `sifive_test` finisher: `PASS` for
/// a clean exit, `FAIL` with the code in the high bits otherwise.
#[cfg_attr(
    all(not(any(test, feature = "system_tests")), not(feature = "board")),
    allow(dead_code)
)]
#[cfg(not(feature = "board"))]
pub fn exit(code: u32) -> ! {
    let word = if code == 0 {
        FINISHER_PASS
    } else {
        FINISHER_FAIL | (code << 16)
    };
    // SAFETY: an MMIO store to the finisher register through the direct map (mapped device-typed by
    // mmu::map_everything). The write exits QEMU, so nothing after it runs.
    let reg = super::mmu::phys_to_virt(SIFIVE_TEST_PHYS) as *mut u32;
    // SAFETY: as the comment above says: an MMIO store to the finisher register through the direct map, which `mmu::map_everything` mapped device-typed. The write exits QEMU, so nothing after it runs.
    unsafe { core::ptr::write_volatile(reg, word) };

    // The finisher terminates the guest; if it somehow does not, stop rather than run on.
    loop {
        super::instructions::wfi();
    }
}

// ---- Board exit: UART marker + SBI SRST shutdown ----

/// The SBI SRST extension id, "SRST" in ASCII.
const SBI_SRST_EID: usize = 0x5352_5354;
/// The SBI SRST `system_reset` function id.
const SBI_SYSTEM_RESET_FID: usize = 0;
/// SRST reset type: shutdown (power off the board).
#[cfg(feature = "board")]
const SRST_RESET_TYPE_SHUTDOWN: usize = 0;
/// SRST reset type: cold reboot. Firmware may support any subset of the types, and refuses one it
/// does not with `SBI_ERR_NOT_SUPPORTED`. See notes/riscv-port/firmware.md.
const SRST_RESET_TYPE_COLD_REBOOT: usize = 1;
/// SRST reset reason: none (no additional reason specified).
const SRST_RESET_REASON_NONE: usize = 0;

/// Call SBI SRST `system_reset` with `reset_type`.
///
/// Returns only when firmware refuses, with `sbiret.error`: expect `SBI_ERR_NOT_SUPPORTED` (-2)
/// for an unsupported type.
fn sbi_system_reset(reset_type: usize) -> isize {
    let args = [reset_type, SRST_RESET_REASON_NONE, 0, 0, 0, 0];
    // SAFETY: SRST system_reset with (reset type, reason none). It resets or shuts the machine down,
    // which is what every caller is asking for; a return means the firmware refused.
    unsafe { super::sbi::call(SBI_SRST_EID, SBI_SYSTEM_RESET_FID, args) }.error
}

/// Ask the firmware for a cold reboot, printing each attempt prefixed with `marker`, and return
/// the refusal as an `abi::Error` only if firmware says no.
///
/// The arch reboot contract: one line per route before trying it, drained to the wire, then the
/// firmware's error after. Called by the rebooting soak and `kernel::reboot::restart`. See
/// notes/riscv-port/firmware.md.
///
/// Name: provisional (milestone 249 (the boot lottery is sampled by a person walking to the
/// board)): calef names public items.
pub fn reboot(marker: &str) -> abi::Error {
    crate::println!(
        "{marker} attempt 1 of 1: SBI SRST system_reset, reset type 1 (cold reboot). The next thing \
         this console should show is the firmware's banner."
    );
    // Drain first: the reset cuts power mid-line if the transmitter is still sending.
    crate::console::drain();
    let error = sbi_system_reset(SRST_RESET_TYPE_COLD_REBOOT);
    crate::println!(
        "{marker} SBI SRST refused: sbiret.error={error} (-2 is SBI_ERR_NOT_SUPPORTED, an OpenSBI \
         that implements shutdown and not reset type 1)"
    );
    abi::reboot::from_sbi(error as i64)
}

/// Terminate the board run with `code` (0 = success): print the UART marker line a serial harness
/// reads, then SBI SRST shutdown. Boards have no `sifive_test` finisher.
#[cfg_attr(
    all(not(any(test, feature = "system_tests")), feature = "board"),
    allow(dead_code)
)]
#[cfg(feature = "board")]
pub fn exit(code: u32) -> ! {
    // Before the ecall: once shutdown begins the UART stops draining.
    if code == 0 {
        crate::println!("NIFE-TEST-EXIT: PASS");
    } else {
        crate::println!("NIFE-TEST-EXIT: FAIL {}", code);
    }
    // Drained, for `reboot`'s reason: a cut marker misreports the run.
    crate::console::drain();
    sbi_system_reset(SRST_RESET_TYPE_SHUTDOWN);

    // SBI SRST should not return. If it does, stop rather than run on.
    loop {
        super::instructions::wfi();
    }
}
