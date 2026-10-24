//! The JH7110 TRNG driver (notes/entropy.md): a backend for `entropy_contract` beside
//! `entropy.rs`'s virtio-rng one, with no virtqueue and no DMA page, just a register window.
//! `kernel/src/user/entropy_service.rs` spawns it on the riscv64 boot tour; QEMU has no JH7110, so
//! CI resolves that to a skip. The register decode, the device-tree query and the byte buffer are
//! in `crates/jh7110_entropy`, host-tested and Kani-reachable. It holds [`REQ`], [`READY`] and one
//! device page at [`TRNG_VA`], and no IRQ, so its authority is smaller than virtio-rng's by
//! construction. It polls `ISTAT` rather than taking PLIC line 30: a wrong interrupt binding fails
//! silently as a hang, a wrong poll bound loudly as `NO_ENTROPY`. See
//! notes/entropy/jh7110-trng.md.

//! # BUGS
//!
//! - It programs no clock and deasserts no reset, and checks neither: a boot where the kernel did
//!   not ungate `SEC_HCLK` and `SEC_MISCAHB_CLK` and release the reset presents as zeros.
//! - An all-zero first generation condemns the device for the boot, even if it powers on later.
//! - [`POLL_TRIES`] and [`LOCKUP_RETRIES`] are unmeasured guesses; [`POLL_TRIES`] counts undelayed
//!   loop iterations, so it guards against a hang and is not a timeout in microseconds.
//! - `MODE.R256` has never been read back. If radon's `STAT.R256` reads clear, only 16 bytes of
//!   each generation are device output (notes/entropy/jh7110-trng.md).
//!
//! Name: ratified 2026-09-13 (calef), replacing `jh7110_entropy_source`. It shares the crate's
//! name as the crate-and-program pair; `jh7110_entropy_driver` was refused for hiding that pair.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use entropy_contract as proto;
use jh7110_entropy::{
    CTRL_EXEC_RANDRESEED, CTRL_GENE_RANDNUM, ISTAT_ALL, ISTAT_RAND_RDY, ISTAT_SEED_DONE, MODE_R256,
    Outcome, Pool, interpret,
};
use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::register_structs;
use tock_registers::registers::{ReadOnly, ReadWrite, WriteOnly};
use user_mode_runtime::{receive_request, reply, send};

register_structs! {
    /// The JH7110 TRNG's register block, offsets from `jh7110_entropy::regs` (itself transcribed
    /// from the Linux driver), checked at compile time. Unlike the NS16550's, this layout has no
    /// runtime stride or width: [binding] gives one `reg` window (`reg = <0x1600C000 0x4000>`) with
    /// no `reg-shift` or `reg-io-width`. Only the registers this driver touches are named; the rest
    /// are reserved padding here.
    ///
    /// [binding]: https://github.com/torvalds/linux/blob/master/Documentation/devicetree/bindings/rng/starfive%2Cjh7110-trng.yaml
    #[allow(non_snake_case)]
    RegisterBlock {
        (0x00 => CTRL: WriteOnly<u32>),
        // `STAT` is read on **every** poll now, not only for the bring-up diagnostic: it carries
        // `SEEDED`, which is what says a latched `RAND_RDY` is an answer rather than a leftover.
        // See `jh7110_entropy::Outcome::Unseeded`.
        (0x04 => STAT: ReadOnly<u32>),
        (0x08 => MODE: ReadWrite<u32>),
        (0x0c => _reserved_smode),
        (0x10 => IE: ReadWrite<u32>),
        // Write-1-to-clear, per the TRM's register map (`jh7110_entropy::regs::ISTAT`). `RAND_RDY`
        // latches, so a driver that never writes this register sees every generation after the
        // first complete instantly.
        (0x14 => ISTAT: ReadWrite<u32>),
        (0x18 => _reserved_pad),
        (0x20 => RAND0: ReadOnly<u32>),
        (0x24 => RAND1: ReadOnly<u32>),
        (0x28 => RAND2: ReadOnly<u32>),
        (0x2c => RAND3: ReadOnly<u32>),
        (0x30 => RAND4: ReadOnly<u32>),
        (0x34 => RAND5: ReadOnly<u32>),
        (0x38 => RAND6: ReadOnly<u32>),
        (0x3c => RAND7: ReadOnly<u32>),
        (0x40 => _reserved_seed),
        (0x60 => AUTO_RQSTS: ReadWrite<u32>),
        (0x64 => AUTO_AGE: ReadWrite<u32>),
        (0x68 => @END),
    }
}

/// Capability slots, by convention with `kernel/src/user/entropy_service.rs`. Slot 0, the request
/// endpoint, received on in `entropy_contract`'s wire format.
const REQ: u64 = 0;
/// Slot 1, a readiness endpoint (`WRITE`) that gets one message:
/// [`proto::READY`](entropy_contract::READY) once a first generation is not all zero, or a
/// [`bringup_failure`](entropy_contract::bringup_failure) word naming the failed step.
const READY: u64 = 1;

/// Where the spawner maps the TRNG's register page (a device-typed mapping, the same mechanism
/// `console.rs`'s `UART_VA` uses), placed by the spawner (rule 2). One page: the binding's window is
/// `0x4000`, but `jh7110_entropy::regs` reaches only `0x68`. **Must match
/// `kernel/src/user/entropy_service.rs`'s `TRNG_VA`.**
/// Provisional and distinct from the `0x0090_0000` DMA convention `entropy.rs` and
/// `keyboard_driver.rs` share, so the two backends cannot collide if a wiring ever needs both
/// mapped in different processes at once.
const TRNG_VA: u64 = address_space_map::pair_page(0x0000_0000_0094_0000);

/// How many times to poll one generation attempt before giving up on it. **Not a measured bound**:
/// see the module doc. Large enough that a real device answering in microseconds never hits it in
/// practice, small enough that a device that will never answer does not hang the driver.
const POLL_TRIES: usize = 100_000;

/// How many lockup-and-reseed cycles to absorb before telling a caller the device would not
/// answer. Bounded for the reason `entropy.rs`'s `REFILL_TRIES` is: a device that faults once is
/// worth retrying, one that never stops faulting is a fact the caller needs, not something to
/// spin on forever.
const LOCKUP_RETRIES: usize = 4;

fn regs() -> &'static RegisterBlock {
    // SAFETY: TRNG_VA is our device mapping of the JH7110 TRNG's register window, handed to us at
    // spawn (rule 2: this driver is told the address, never told to look), for the whole lifetime
    // of this process. This is the same invariant the hand-written r32/w32 calls used to assert by
    // comment; register_structs! now checks every offset above at compile time instead.
    unsafe { &*(TRNG_VA as *const RegisterBlock) }
}

fn rand_words() -> [u32; 8] {
    let r = regs();
    [
        r.RAND0.get(),
        r.RAND1.get(),
        r.RAND2.get(),
        r.RAND3.get(),
        r.RAND4.get(),
        r.RAND5.get(),
        r.RAND6.get(),
        r.RAND7.get(),
    ]
}

/// **Acknowledge `ISTAT` bits.** The register is write-1-to-clear (`jh7110_entropy::regs::ISTAT`),
/// so writing a mask back clears exactly the bits in it and leaves the rest standing.
fn ack(bits: u32) {
    regs().ISTAT.set(bits);
}

/// Put the block in a known state before asking it for anything (milestone 159 (a real hardware
/// entropy source: the JH7110's TRNG)): zero `AUTO_AGE` and `AUTO_RQSTS`, disabling the reseed alarms; clear every `ISTAT` bit, since a `RAND_RDY`
/// latched under U-Boot would satisfy the first poll; set `MODE.R256`, since the width after reset
/// is a build-time parameter of the silicon; and leave `IE` at zero to poll. The order and the
/// reasons are in notes/entropy/jh7110-trng.md.
///
/// Returns `STAT` as read afterwards, the bench diagnostic: `STAT.R256` says whether `MODE.R256`
/// took, and `STAT.SEEDED` whether the reseed the caller runs next is still needed.
fn init() -> u32 {
    let r = regs();
    r.AUTO_AGE.set(0);
    r.AUTO_RQSTS.set(0);
    ack(ISTAT_ALL);
    r.MODE.set(r.MODE.get() | MODE_R256);
    r.IE.set(0);
    r.STAT.get()
}

/// Force a reseed and wait (bounded) for `ISTAT.SEED_DONE`, acknowledging it. Called once at
/// bring-up, mirroring the init sequence in `jh7110_entropy`'s module doc, and again whenever
/// [`generate`] sees [`jh7110_entropy::Outcome::Lockup`] or [`jh7110_entropy::Outcome::Unseeded`].
/// `false` on a bound-out: the caller decides what that means. `SEED_DONE` is latched, so it is
/// acknowledged on both sides; an unacked one makes every later reseed appear to complete at once.
fn reseed_and_wait() -> bool {
    ack(ISTAT_SEED_DONE);
    regs().CTRL.set(CTRL_EXEC_RANDRESEED);
    for _ in 0..POLL_TRIES {
        if regs().ISTAT.get() & ISTAT_SEED_DONE != 0 {
            ack(ISTAT_SEED_DONE);
            return true;
        }
    }
    false
}

/// Ask the device for 32 fresh bytes, retrying a hardware-reported lockup or an unseeded core by
/// reseeding, bounded by [`LOCKUP_RETRIES`]. `None` if the device never produced an answer inside
/// the bound: this driver's whole failure mode, reported to callers as [`proto::NO_ENTROPY`]
/// rather than a hang.
///
/// `RAND_RDY` is acknowledged on both sides of the command: before the write, because a bit
/// standing from the previous generation would satisfy the first poll and hand back the previous
/// answer, and after the words are read, so the next call starts from a clean register.
fn generate() -> Option<[u8; 32]> {
    for _ in 0..=LOCKUP_RETRIES {
        ack(ISTAT_RAND_RDY);
        regs().CTRL.set(CTRL_GENE_RANDNUM);
        for _ in 0..POLL_TRIES {
            match interpret(regs().STAT.get(), regs().ISTAT.get(), rand_words()) {
                Outcome::Ready(bytes) => {
                    ack(ISTAT_RAND_RDY);
                    return Some(bytes);
                }
                // Both of these mean "the core is not in a state to answer, seed it and ask
                // again", and they are bounded together because a device that keeps returning to
                // either one is a device to tell the caller about rather than to spin on.
                Outcome::Lockup | Outcome::Unseeded => {
                    reseed_and_wait();
                    break; // out of the poll loop; the outer loop's next iteration retries GENE_RANDNUM
                }
                Outcome::NotReady => continue,
            }
        }
    }
    None
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    // Put the block in a known state before asking it for anything, then seed it. `init` returns
    // what `STAT` read back, which is the one snapshot taken before any generation has happened
    // and so the only one that can say whether `MODE.R256` took.
    let stat_after_init = init();
    reseed_and_wait();

    // Draw the first 32 bytes before reporting, as `entropy.rs` does, so "up" means a client that
    // asks will be answered. The bytes decide the word, so a device whose clock is gated cannot
    // report itself ready holding zeros.
    let first = generate();
    let report = proto::readiness(first.into_iter().flatten());
    let mut pool = Pool::new();
    if let Some(bytes) = first {
        pool.refill(&mut || Some(bytes));
    }
    // Word 2 is always `(STAT << 32) | ISTAT`, read now, whatever word 0 says, so the number a
    // bench session reads first means one thing. All zeros says the window read as nothing (a gated
    // clock, an undeasserted reset, or a base that is not the TRNG); a nonzero `STAT` with `SEEDED`
    // clear says the device is alive and the seeding sequence did not finish.
    let diagnostic = (u64::from(regs().STAT.get()) << 32) | u64::from(regs().ISTAT.get());
    // Word 1 carries the byte count and, in its high half, the `STAT` seen right after `init`,
    // whose `R256` says whether the 32 bytes are all device output or only 16 of them.
    let bytes_in_hand = pool.remaining() as u64;
    send(
        READY,
        report,
        (u64::from(stat_after_init) << 32) | bytes_in_hand,
        diagnostic,
    );

    // A device that answered with zeros is refused for the rest of the boot: a client only sees a
    // reply, so serving that zero buffer would hand zeros out as randomness to everyone not
    // watching the handshake. A device that answered nothing is not refused: `take` answers
    // `NO_ENTROPY` while it stays dry, and it recovers by itself if it starts answering.
    serve(
        pool,
        report == proto::bringup_failure(proto::STEP_FIRST_ALL_ZERO),
    )
}

/// The serve loop: one endpoint, one wait point, forever. Identical in shape to `entropy.rs`'s,
/// because the contract (`entropy_contract`) is the thing that does not change between backends.
fn serve(mut pool: Pool, refuse: bool) -> ! {
    loop {
        let req = receive_request(REQ);
        let w0 = req.w0;
        let Some(cap) = req.delivered.into_reply() else {
            // A plain SEND or a SEND_CAP on a CALL-only contract: nothing to answer. See
            // `entropy.rs`'s identical comment.
            continue;
        };
        let (count, word) = match proto::operation(w0) {
            // `refuse` is a device this driver condemned at bring-up (see `_start`). It is
            // answered exactly the way a dry device is, because `NO_ENTROPY` already means the one
            // thing a client has to know: it is not getting randomness here.
            proto::GET if !refuse => pool.take(proto::want(w0), &mut generate),
            _ => (proto::NO_ENTROPY, 0),
        };
        reply(cap, count, word);
    }
}

/// **The two 8s are the same 8.** `jh7110_entropy::Pool::take` clamps to the width of the word it
/// answers with; `entropy_contract::want` clamps to the width of the word the wire carries. This file
/// is the only one that depends on both, so it is where the agreement can be checked, and a
/// compile-time assert is the rung this project reaches for when a fact can be made unrepresentable
/// rather than remembered.
const _: () = assert!(jh7110_entropy::WORD_BYTES == proto::MAX_BYTES);

user_mode_runtime::panic_handler!();

// There is no capability slot for the registers, and that is worth a sentence rather than a
// silence: this driver reaches them by direct volatile access at `TRNG_VA`, the same way
// `console.rs` reaches the UART, not through a kernel-mediated `invoke` the way the virtio
// backends reach `Virtio` capabilities. The authority is the mapping the spawner installed; the
// driver holds no name for it.
