//! **The entropy service** (§44 (entropy is a capability), notes/entropy.md): the one process that
//! reaches the entropy source. Clients hold an endpoint meaning "you may obtain randomness", a
//! strictly smaller authority than the device: no queue, no DMA region, no request it did not make
//! for them. Owning it is worth exactly an attacker choosing random numbers, no more.
//!
//! **These are the device's bytes**: no pool, whitening, mixing or DRBG. A [`POOL_LEN`] buffer is
//! handed out in order, once, and zeroed behind the cursor. The [`instr`] backend runs RDSEED or
//! RNDRRS in this process, behind the same endpoint, because an instruction a client ran itself
//! would make the endpoint theatre; RDRAND and RNDR are refused, since they may answer from a
//! hardware DRBG. The kernel picks the backend (`ID_AA64ISAR0_EL1.RNDR` is EL1-only).

//! # BUGS
//!
//! - An all-zero first draw condemns the backend: `NO_ENTROPY` for the rest of the boot, wrong
//!   with probability 2^-2048 (virtio) or 2^-64 (instruction), as `entropy_contract`'s `BUGS` says.
//! - A condemned backend does not recover short of restarting the service.
//! - Nothing checks a byte after bring-up; continuous health testing is
//!   `design/decisions/0137-trng-health-tests.md`, proposed.
//!
//! Name: ratified 2026-09-14 (calef): the resource name, like `clock`, because this service does
//! hand you entropy. It pairs with `jh7110_entropy`, ratified the day before.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use entropy_contract as proto;
use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
use user_mode_runtime::virtio::virtio_ring_barrier;
use user_mode_runtime::{
    exit, interrupt_acknowledge, interrupt_wait, receive_request, reply, send,
};

/// Capability slots for the virtio backend, by convention with `kernel/src/user/entropy_service.rs`.
/// Slot 0, the request endpoint (RECEIVE).
const REQ: u64 = 0;
/// Slot 1, the device's `Irq`.
const IRQ: u64 = 1;
/// Slot 2, the confined `Virtio` transport.
const VIRTIO: u64 = 2;
/// Slot 3, a readiness endpoint (WRITE).
const READY: u64 = 3;

/// What `arg0` means, by convention with `kernel/src/user/entropy_service.rs::start` and
/// `::start_instruction`. Not a wire contract between two *user* programs (rule 7 does not apply):
/// it is the kernel's own spawn-argument convention with the one program it spawns, the same
/// footing `DMA_VA` below is already on.
const MODE_VIRTIO: u64 = 0;
const MODE_INSTRUCTION: u64 = 1;

/// Capability slots for the instruction backend: two, not four. No `Irq`, no `Virtio`, no DMA
/// mapping at all, because `RNDRRS`/`RDSEED` need none of them. Smaller authority than the virtio
/// backend's, which is itself already minimal.
const I_REQ: u64 = 0;
const I_READY: u64 = 1;

/// Where the kernel maps this service's DMA page. Must match `kernel/src/user/entropy_service.rs`.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);

// SAFETY: the wiring maps one page read/write at DMA_VA before this program runs (milestone 139;
// see `user_mode_runtime::mapped_window`, which is what collapsed the hand-rolled r8/w8/r16/w16/r32 below).
const WINDOW: MappedWindow = unsafe { MappedWindow::new(DMA_VA, PAGE) };

// virtio-mmio register offsets. The §18 transport seam speaks this vocabulary on both buses, so
// this driver runs over PCIe and over mmio without knowing which one it got.
const MAGIC: u64 = 0x000;
const DEVICE_ID: u64 = 0x008;
const DEVICE_FEATURES: u64 = 0x010;
const DEVICE_FEATURES_SEL: u64 = 0x014;
const DRIVER_FEATURES: u64 = 0x020;
const DRIVER_FEATURES_SEL: u64 = 0x024;
const INTERRUPT_STATUS: u64 = 0x060;
const INTERRUPT_ACK: u64 = 0x064;
const STATUS: u64 = 0x070;

const S_ACKNOWLEDGE: u32 = 1;
const S_DRIVER: u32 = 2;
const S_DRIVER_OK: u32 = 4;
const S_FEATURES_OK: u32 = 8;
const F_VERSION_1_HI: u32 = 1; // feature bit 32
const F_ACCESS_PLATFORM_HI: u32 = 1 << 1; // feature bit 33 (behind an IOMMU)

const VIRTQ_DESC_F_WRITE: u16 = 2;

/// The virtio device type this driver will talk to. Checked, not assumed: the display driver found
/// the PCI transport answering every driver's `DeviceID` read with a hardcoded 2, and it found it
/// because it was the first driver that looked.
const VIRTIO_ID_ENTROPY: u32 = 4;

/// virtio-rng has exactly one virtqueue, the request queue. Nothing rides the second slot of the
/// two-queue confinement ceiling here.
const RNG_Q: u64 = 0;
const QSIZE: u16 = 8;

/// The DMA page's layout: the queue's rings at the kernel's per-queue offsets, then the buffer the
/// device fills. All inside the one 4 KiB page the kernel granted, so the shadow-ring validator
/// confines every address the device is handed.
const Q_DESC: u64 = 0x000;
const Q_AVAIL: u64 = 0x080;
const Q_USED: u64 = 0x100;
const POOL_OFF: u64 = 0x400;

/// How many bytes one device request fetches. 256 is 32 clients' worth of [`proto::MAX_BYTES`] per
/// round trip, and it leaves the rest of the page unused rather than crowding the rings.
const POOL_LEN: u64 = 256;

/// How many times to re-ask a device that returned nothing before telling the caller so. A device
/// that has momentarily run dry is worth one more request; a device that never answers is a fact
/// the caller needs, not something to spin on.
const REFILL_TRIES: usize = 4;

/// How many interrupt wakeups to absorb while waiting for one completion. A wakeup can be stale,
/// coalesced, or a previous operator's, so the **used ring**, not the wakeup, is the completion;
/// the bound is what keeps a device that stopped answering from becoming a hang.
const WAIT_WAKEUPS: usize = 64;

/// Bring-up failures, in a `0xDEAD_...` word so a failure names its step instead of hanging.
const E_MAGIC: u64 = 0x01;
const E_DEVICE_ID: u64 = 0x02;
const E_FEATURES: u64 = 0x03;
const E_QUEUE: u64 = 0x04;

/// The virtio steps above are this backend's own, and `entropy_contract` reserves `0x01..=0x0f` for
/// exactly that. The two steps every backend shares (`STEP_NO_FIRST_BYTES`, `STEP_FIRST_ALL_ZERO`)
/// start at `0x10`, and this assert is what keeps a fifth virtio step from silently becoming one
/// of them: a report word decodes to one step or it decodes to a lie.
const _: () = assert!(E_QUEUE < proto::STEP_NO_FIRST_BYTES);

fn r8(off: u64) -> u8 {
    WINDOW.r8(off)
}

fn w8(off: u64, v: u8) {
    WINDOW.w8(off, v);
}

fn r16(off: u64) -> u16 {
    WINDOW.r16(off)
}

fn w16(off: u64, v: u16) {
    WINDOW.w16(off, v);
}

fn r32(off: u64) -> u32 {
    WINDOW.r32(off)
}

fn mr(off: u64) -> u32 {
    user_mode_runtime::virtio::read_register(VIRTIO, off) as u32
}

fn mw(off: u64, v: u32) {
    user_mode_runtime::virtio::write_register(VIRTIO, off, v as u64);
}

fn die(code: u64) -> ! {
    send(READY, proto::bringup_failure(code), 0, 0);
    exit();
}

/// Write descriptor 0: the whole buffer, device-**writable**, which is the direction that matters
/// here. This is a receive-only device; nothing this process holds is ever read by it.
fn write_desc(addr: u64, len: u32) {
    // Inside the DMA page, at the descriptor table the kernel programmed the queue with; all four
    // writes are bounds-checked by `WINDOW` rather than trusted by hand.
    WINDOW.write(Q_DESC, addr);
    WINDOW.write(Q_DESC + 8, len);
    WINDOW.write::<u16>(Q_DESC + 12, VIRTQ_DESC_F_WRITE);
    WINDOW.write::<u16>(Q_DESC + 14, 0);
}

/// The service's running state: where the device's bytes are and how many of them are still ours
/// to give. Not a pool: `cursor` only ever moves forward, so no byte is served twice.
struct Pool {
    /// The DMA region's physical base. Descriptors speak physical addresses; a process knows
    /// virtual ones, so the spawner passes this in (rule 2: the driver is told, never told to look).
    direct_memory_access_phys: u64,
    /// Available-ring index we have published up to.
    avail: u16,
    /// Used-ring index we have drained up to.
    seen: u16,
    /// Next unserved byte's offset within the buffer.
    cursor: u64,
    /// One past the last byte the device wrote. `cursor == filled` means empty.
    filled: u64,
}

impl Pool {
    /// Ask the device for a bufferful and wait for it. Returns how many bytes arrived, which the
    /// spec allows to be fewer than asked for and, on a dry device, zero.
    fn request(&mut self) -> u64 {
        write_desc(self.direct_memory_access_phys + POOL_OFF, POOL_LEN as u32);
        w16(Q_AVAIL + 4 + (self.avail % QSIZE) as u64 * 2, 0); // ring[idx] = descriptor head 0
        virtio_ring_barrier(); // the descriptor must be visible before the index that advertises it
        self.avail = self.avail.wrapping_add(1);
        w16(Q_AVAIL + 2, self.avail);
        virtio_ring_barrier(); // and the index before the doorbell

        // The kernel walks the descriptor we just published, refuses it if it leaves our region,
        // and only then rings the device.
        if user_mode_runtime::virtio::notify(VIRTIO, RNG_Q) < 0 {
            return 0; // the kernel refused our own in-region descriptor: a bug here, not a dry device
        }

        // **The completion is the used ring advancing, not the wakeup**, so look before blocking.
        // QEMU completes a request inside `NOTIFY`, and PCI INTx lines are shared four ways on both
        // `virt` boards (`pci::intx_irq` swizzles modulo 4) while the kernel routes an intid to one
        // endpoint (`sched::bind_irq`): with five PCI functions no unshared line is left. A wakeup
        // can be stale, coalesced or a previous operator's, so the loop re-checks the ring, and it
        // is bounded so a silent device is an honest "no entropy" rather than a hang.
        let mut len = 0;
        for _ in 0..WAIT_WAKEUPS {
            virtio_ring_barrier();
            if r16(Q_USED + 2) != self.seen {
                // used-ring element: { u32 id; u32 len }. `len` is the device saying how many
                // bytes it actually wrote, and it is allowed to be short.
                let slot = (self.seen % QSIZE) as u64;
                len = (r32(Q_USED + 4 + slot * 8 + 4) as u64).min(POOL_LEN);
                self.seen = self.seen.wrapping_add(1);
                break;
            }
            interrupt_wait(IRQ);
            self.quiet();
        }
        // Quiet the device and re-enable the line even on the path that never waited: the device
        // raised its line when it completed and the kernel masked it, and a line left masked is one
        // the *next* holder of a shared intid never hears from. `interrupt::ACK` is
        // `arch::irq::enable`, which is idempotent, so this is safe when nothing fired.
        self.quiet();
        len
    }

    /// Acknowledge the device's interrupt and re-enable the line at the controller, in that order:
    /// the disk driver's discipline, and the reason an interrupt does not immediately re-fire.
    fn quiet(&self) {
        let istatus = mr(INTERRUPT_STATUS);
        mw(INTERRUPT_ACK, istatus);
        interrupt_acknowledge(IRQ); // re-enable the line the kernel masked when it fired
    }

    /// Refill the buffer, retrying a device that returned nothing. `false` when the device gave us
    /// nothing at all, which is the one case a client is told about.
    fn refill(&mut self) -> bool {
        for _ in 0..REFILL_TRIES {
            let n = self.request();
            if n > 0 {
                self.cursor = 0;
                self.filled = n;
                return true;
            }
        }
        false
    }

    /// Take `n` bytes, as a little-endian word plus a count.
    ///
    /// A refill can land short (QEMU's virtio-rng returns fewer than the 256 bytes asked), so the
    /// bytes a client asked for can straddle two device requests; gathering across them keeps a
    /// short device read invisible. A count below `n` means the device went dry part-way, never
    /// padding; zero means it was dry from the start.
    fn take(&mut self, n: u64) -> (u64, u64) {
        let mut word = 0u64;
        let mut got = 0;
        while got < n {
            if self.cursor == self.filled && !self.refill() {
                break;
            }
            let run = (n - got).min(self.filled - self.cursor);
            for i in 0..run {
                let at = POOL_OFF + self.cursor + i;
                word |= (r8(at) as u64) << (8 * (got + i));
                // Zero behind the cursor. The cursor alone already guarantees no byte is served
                // twice; this is hygiene, so a byte a client now holds is not also still sitting in
                // a page this long-lived process keeps mapped for the rest of the boot.
                w8(at, 0);
            }
            self.cursor += run;
            got += run;
        }
        (got, word)
    }
}

user_mode_runtime::program_entry!(start);

fn start(mode: u64, direct_memory_access_phys: u64, _arg2: u64) -> ! {
    if mode == MODE_INSTRUCTION {
        // No virtio device, no DMA page, no IRQ: the kernel would not have spawned this mode
        // unless it already confirmed the instruction's feature bit (aarch64's `ID_AA64ISAR0_EL1`,
        // checked at EL1 because EL0 cannot read it). This process trusts that the same way it
        // trusts a granted `Virtio` capability to name a real device.
        serve_instruction();
    }
    debug_assert_eq!(mode, MODE_VIRTIO, "entropy: unknown spawn mode {mode:#x}");
    if mr(MAGIC) != 0x7472_6976 {
        die(E_MAGIC);
    }
    // **Check what we are talking to**, for the reason the keyboard driver records: a transport
    // that answered every `DeviceID` read with a hardcoded number went unnoticed until a driver
    // looked. An entropy service that programmed a disk's queue would be worse than most.
    if mr(DEVICE_ID) != VIRTIO_ID_ENTROPY {
        die(E_DEVICE_ID);
    }

    mw(STATUS, 0);
    mw(STATUS, S_ACKNOWLEDGE);
    mw(STATUS, S_ACKNOWLEDGE | S_DRIVER);

    mw(DRIVER_FEATURES_SEL, 0);
    mw(DRIVER_FEATURES, 0); // virtio-rng defines no low-word features at all
    mw(DEVICE_FEATURES_SEL, 1);
    let dev_hi = mr(DEVICE_FEATURES);
    let mut ack_hi = F_VERSION_1_HI;
    if dev_hi & F_ACCESS_PLATFORM_HI != 0 {
        ack_hi |= F_ACCESS_PLATFORM_HI; // behind an IOMMU, which on the PCIe wiring it is
    }
    mw(DRIVER_FEATURES_SEL, 1);
    mw(DRIVER_FEATURES, ack_hi);

    mw(STATUS, S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK);
    if mr(STATUS) & S_FEATURES_OK == 0 {
        die(E_FEATURES);
    }

    // The kernel programs the queue's ring addresses; this service never writes a queue-address
    // register, which is the §18 seam and the reason the shadow ring can be trusted.
    if user_mode_runtime::virtio::setup_queue(VIRTIO, QSIZE as u64, RNG_Q) != 0 {
        die(E_QUEUE);
    }
    mw(
        STATUS,
        S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK | S_DRIVER_OK,
    );

    let mut pool = Pool {
        direct_memory_access_phys,
        avail: 0,
        seen: 0,
        cursor: 0,
        filled: 0,
    };

    // Fetch the first bufferful before reporting ready, so "the service is up" means "a client
    // that asks will be answered" rather than "the handshake completed". A device that produces
    // nothing is a fact worth learning at boot rather than at the first `CALL`.
    //
    // **And the bytes decide the word, not the fact that a request completed.** This line used to
    // send `proto::READY` unconditionally with the truth in a second word; the JH7110 backend was
    // the one that got caught doing it on silicon (2026-09-04, radon), but the shape of the bug
    // was here too. `readiness` reads the bufferful the device actually left in the DMA page,
    // through the same bounds-checked window everything else here uses.
    let first = pool.refill();
    let report = proto::readiness((0..pool.filled).map(|i| r8(POOL_OFF + i)));
    send(READY, report, u64::from(first), pool.filled);

    // A device that wrote a page of zeros is condemned for the rest of the boot; a device that
    // wrote nothing is not. See `jh7110_entropy.rs`'s `_start`, which says why at length: the report
    // word reaches whoever wired the service, and a client only ever sees a reply.
    serve(
        pool,
        report == proto::bringup_failure(proto::STEP_FIRST_ALL_ZERO),
    )
}

/// The serve loop: one endpoint, one wait point, forever.
fn serve(mut pool: Pool, refuse: bool) -> ! {
    loop {
        let req = receive_request(REQ);
        let w0 = req.w0;
        let Some(cap) = req.delivered.into_reply() else {
            // A plain SEND, or a SEND_CAP, on a CALL-only contract. Nobody is waiting for an
            // answer, so there is nothing to do and nothing to report; drop it (a delegation's slot
            // is deleted, milestone 706 (a `CALL` server can tell a Reply from a delegation))
            // rather than replying into a slot that is not a Reply. (The clock service answers the
            // same way for the same reason.)
            continue;
        };
        let (count, word) = match proto::operation(w0) {
            // `refuse` is a device this service condemned at bring-up for answering with zeros. It
            // is answered the way a dry device is: `NO_ENTROPY` already means the one thing a
            // client has to know, which is that it is not getting randomness here.
            proto::GET if !refuse => pool.take(proto::want(w0)),
            // There is exactly one operation, and the reply's first word is a byte count with no
            // room in it for an error code. So an unknown opcode is answered "you got nothing",
            // which is true. A second operation would be the moment to widen the reply, and that
            // is a contract change rather than a branch added here.
            _ => (proto::NO_ENTROPY, 0),
        };
        reply(cap, count, word);
    }
}

/// **The instruction backend** (milestone 162 (real hardware entropy on x86_64 and aarch64)):
/// RDSEED on `x86_64`, RNDRRS on aarch64, chosen by the kernel in
/// `kernel/src/user/entropy_service.rs::is_instruction_backend_available`. Either can run dry
/// under load, and then the answer is `NO_ENTROPY`, never the DRBG sibling; on-die conditioning is
/// part of each instruction's contract and is not a DRBG. A module of the one program that uses it
/// rather than a crate. **Provisional**, flagged for an architect: a second userspace program
/// needing per-architecture `asm!` makes this the first candidate for a crate.
mod instr {
    /// How many times to retry a transient "no data this cycle" result before giving up.
    ///
    /// Both instructions draw from the conditioned source, not a DRBG, so underflow under load is
    /// expected. Intel's DRNG Software Implementation Guide (rev. 2.2, section 5.3.1.2) tells an
    /// "asynchronous application" to give up after "somewhere between 1 and 100" retries, and one
    /// serialized request queue makes the high end free. ARM's text for `RNDR`/`RNDRRS` ("if the
    /// instruction cannot return a genuine random number in a reasonable period of time") reads as
    /// the hardware already retrying, and Linux's `arch/arm64/include/asm/archrandom.h` does not
    /// retry; one bound for both is no stronger a claim than either vendor makes.
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    const RETRIES: u32 = 100;

    /// Draw eight bytes of real entropy, or `None` if the source stayed dry across [`RETRIES`]
    /// attempts. Unlike the virtio backend's [`super::Pool`], there is no gathering across a
    /// boundary to do: one successful draw is exactly [`super::proto::MAX_BYTES`] bytes, so a
    /// single request never needs more than one attempt loop.
    #[cfg(target_arch = "x86_64")]
    pub fn draw() -> Option<[u8; 8]> {
        for _ in 0..RETRIES {
            let v: u64;
            let ok: u8;
            // SAFETY: `rdseed` is unprivileged at any ring (Intel DRNG guide §3.3.2: "no hardware
            // ring requirements... may be invoked as part of an operating system... or directly by
            // an application") and touches no memory. Success/failure rides the carry flag, not an
            // exception, which `setc` captures in the same block before anything else can disturb
            // it.
            unsafe {
                core::arch::asm!(
                    "rdseed {v}",
                    "setc {ok}",
                    v = out(reg) v,
                    ok = out(reg_byte) ok,
                    options(nomem, nostack),
                );
            }
            if ok != 0 {
                return Some(v.to_le_bytes());
            }
            // SAFETY: `pause` is Intel's own documented spin-loop hint for exactly this retry shape
            // (DRNG guide §5.3.1.1/.2); it touches no memory and has no failure mode.
            unsafe { core::arch::asm!("pause", options(nomem, nostack)) };
        }
        None
    }

    /// See the `x86_64` [`draw`] above; same contract, ARM's instruction.
    #[cfg(target_arch = "aarch64")]
    pub fn draw() -> Option<[u8; 8]> {
        for _ in 0..RETRIES {
            let v: u64;
            let ok: u64;
            // SAFETY: `RNDRRS` (`S3_3_C2_C4_1`, the numeric encoding rather than the mnemonic alias
            // so this assembles regardless of the assembler's FEAT_RNG name table) is unprivileged
            // once FEAT_RNG is present. The kernel already confirmed that before spawning this
            // process in instruction mode (`ID_AA64ISAR0_EL1` is not EL0-readable, so this file
            // cannot check it itself); executing it without the feature present would trap as an
            // unknown register access. Success/failure is this instruction's own architected side
            // effect on PSTATE.NZCV (Arm ARM, "Random Number instructions": `0b0000` success,
            // `0b0100` otherwise), which `cset ok, ne` reads back in the same block, the identical
            // idiom Linux's `arch/arm64/include/asm/archrandom.h` uses. Touches no memory.
            unsafe {
                core::arch::asm!(
                    "mrs {v}, S3_3_C2_C4_1",
                    "cset {ok}, ne",
                    v = out(reg) v,
                    ok = out(reg) ok,
                    options(nomem, nostack),
                );
            }
            if ok != 0 {
                return Some(v.to_le_bytes());
            }
        }
        None
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub fn draw() -> Option<[u8; 8]> {
        // The kernel never spawns MODE_INSTRUCTION on an architecture with neither instruction
        // (riscv64: milestone 159's JH7110 TRNG is the real hardware source there), so this arm
        // exists only to keep the crate building for a fourth architecture that adds neither
        // instruction, rather than to be reached.
        None
    }
}

/// The instruction-mode serve loop: `RECEIVE` on `I_REQ`, one instruction draw per request, one wait
/// point. No device, so no bring-up steps and nothing that can [`die`]: the only degenerate case is
/// a dry source, reported the same way a request's own [`proto::NO_ENTROPY`] answer already is.
fn serve_instruction() -> ! {
    let first = instr::draw();
    let report = proto::readiness(first.into_iter().flatten());
    send(
        I_READY,
        report,
        u64::from(first.is_some()),
        first.map_or(0, |_| proto::MAX_BYTES),
    );
    // Eight zero bytes out of `RDSEED`/`RNDRRS` condemn the source for the boot, the same rule the
    // two device backends follow. The false-positive probability is 2^-64 here rather than the
    // 2^-256 a 32-byte draw gets, which is the width of the answer rather than a weaker check;
    // `entropy_contract`'s `BUGS` carries the number.
    let refuse = report == proto::bringup_failure(proto::STEP_FIRST_ALL_ZERO);
    loop {
        let req = receive_request(I_REQ);
        let w0 = req.w0;
        let Some(cap) = req.delivered.into_reply() else {
            // Same reasoning as `serve`'s identical line: nobody is waiting for an answer.
            continue;
        };
        let (count, word) = match proto::operation(w0) {
            proto::GET if !refuse => match instr::draw() {
                Some(bytes) => (proto::want(w0), u64::from_le_bytes(bytes)),
                None => (proto::NO_ENTROPY, 0),
            },
            _ => (proto::NO_ENTROPY, 0),
        };
        reply(cap, count, word);
    }
}

user_mode_runtime::panic_handler!();
