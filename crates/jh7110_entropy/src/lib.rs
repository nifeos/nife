#![cfg_attr(not(test), no_std)]
//! The `StarFive` JH7110 TRNG, as pure logic: the register layout, what each of the chip's two
//! device trees calls the block, and the decision of whether a generation succeeded, failed or is
//! running.
//!
//! It never dereferences a pointer, so it is host-testable and Kani-reachable; the caller maps the
//! register window and does the volatile reads. `ISTAT.RAND_RDY` counts only when `STAT.SEEDED` is
//! set (NetBSD's rule), and `ISTAT.LFSR_LOCKUP`, the one hardware fault signal, is
//! [`Outcome::Lockup`]. None of it has run on silicon.
//!
//! Name: ratified 2026-09-13 (calef); kept as the crates.io name (calef, 2026-10-07 UTC, #1806).
//! The argument its minting lane gave (milestone 159's lane, 2026-08-24): chip-qualified and
//! unambiguous rather than generic, since `trng` alone would be a generic word that could name
//! almost anything, the same reasoning that keeps `non_volatile_memory_express` and `pci` named for
//! the specification rather than for "storage" or "bus".

//! # Sources
//!
//! The bracketed labels in the documentation below name these:
//!
//! - \[binding\]: Linux's `starfive,jh7110-trng.yaml`, mainline as of 2026-08-24: the compatible
//!   string, the `reg` window, two clocks, one reset and one interrupt.
//! - \[driver\]: Linux's `drivers/char/hw_random/jh7110-trng.c`, mainline as of 2026-08-24. The
//!   register offsets and bit positions are transcribed from its `#define`s.
//! - \[trm\]: the *JH7110 Technical Reference Manual*, Preliminary V2 (2023-04-24), "TRNG > Control
//!   Registers". It settles that `ISTAT` is write-1-to-clear and that zero disables `AUTO_RQSTS`
//!   and `AUTO_AGE`, and names two `ISTAT` bits no Linux driver defines.
//! - \[netbsd\]: NetBSD's `sys/arch/riscv/starfive/jh7110_trng.c`, revision 1.2: the only driver
//!   that polls, and the source of the interrupt bit positions mainline omits.

//! # The two device trees
//!
//! Mainline Linux describes the block as `compatible = "starfive,jh7110-trng"` ([`COMPATIBLE`]).
//! The vendor U-Boot's control tree, which a VisionFive 2 boots with, spells the same block
//! `"starfive,trng"` ([`COMPATIBLE_VENDOR`]) at the same window (`0x1600_C000`, `0x4000` bytes)
//! with the same interrupt (30), and marks it `status = "disabled"` because U-Boot has no driver
//! for it. [`COMPATIBLES`] lists both in the order to try them. A caller walks its own tree for a
//! node carrying one of them, decodes `interrupts` with [`interrupt`] and `status` with
//! [`status_okay`], and fills in a [`Discovered`].

//! # Examples
//!
//! ```
//! use jh7110_entropy::{Outcome, interpret, ISTAT_RAND_RDY, ISTAT_LFSR_LOCKUP, STAT_SEEDED};
//!
//! // Seeded, but the generation has not finished.
//! assert_eq!(interpret(STAT_SEEDED, 0, [0; 8]), Outcome::NotReady);
//!
//! // The eight output words assemble little-endian, word 0 first.
//! let rand = [0x0403_0201, 0x0807_0605, 0, 0, 0, 0, 0, 0];
//! match interpret(STAT_SEEDED, ISTAT_RAND_RDY, rand) {
//!     Outcome::Ready(bytes) => assert_eq!(&bytes[..8], &[1, 2, 3, 4, 5, 6, 7, 8]),
//!     other => panic!("expected Ready, got {other:?}"),
//! }
//!
//! // A latched RAND_RDY on a core never seeded is not an answer, and a fault beats a stale one.
//! assert_eq!(interpret(0, ISTAT_RAND_RDY, rand), Outcome::Unseeded);
//! let both = ISTAT_RAND_RDY | ISTAT_LFSR_LOCKUP;
//! assert_eq!(interpret(STAT_SEEDED, both, [0; 8]), Outcome::Lockup);
//! ```

/// Register byte offsets from the device's base, transcribed from `jh7110-trng.c`'s `#define`s
/// (\[driver\]). `RAND0..RAND7` are the eight 32-bit words a completed generation leaves behind;
/// [`assemble`] is the order they combine in.
pub mod regs {
    /// Command register: write [`super::CTRL_GENE_RANDNUM`] or [`super::CTRL_EXEC_RANDRESEED`].
    pub const CTRL: u64 = 0x00;
    /// Status register: read-only, [`super::STAT_SEEDED`] / `RAND_GENERATING` / `RAND_SEEDING`.
    pub const STAT: u64 = 0x04;
    /// Output width mode: [`super::MODE_R256`] selects a 256-bit rather than a 128-bit `RAND`
    /// answer.
    ///
    /// **A driver that never writes this register does not get 32 usable bytes.** The width after
    /// reset is a build-time parameter (\[trm\]'s `FEATURES.MAX_RAND_LENGTH` and
    /// `BUILD_CONFIG.PRNG_LEN_AFTER_RST`), and in 128-bit mode only `RAND0..RAND3` carry the
    /// answer, which is why \[driver\] clamps its read to four words without `R256`.
    /// `STAT.R256` ([`super::STAT_R256`]) reports the current width, and all three cited drivers
    /// write `MODE` during bring-up.
    pub const MODE: u64 = 0x08;
    /// Status-mode register: `MISSION_MODE`, `NONCE_MODE` and `MAX_REJECTS` per \[trm\]. None of
    /// the three drivers cited here writes a value of its own into it (mainline leaves it alone
    /// entirely and the vendor driver writes back what it read), so this crate names the offset
    /// and [`super::SMODE_MISSION_MODE`] without proposing a write.
    pub const SMODE: u64 = 0x0C;
    /// Interrupt enable: the four `super::IE_*` contribution bits plus [`super::IE_GLBL_EN`].
    /// **Enabling a contribution here is about the IRQ pin, not about whether [`ISTAT`] latches**;
    /// see [`super::ISTAT_ALL`].
    pub const IE: u64 = 0x10;
    /// Interrupt status, and **\[trm\]'s register map marks it `R/W1C`**: a bit is acknowledged by
    /// writing a one to it, as \[driver\] and \[netbsd\] both do. A driver that only ever reads
    /// this register sees `RAND_RDY` stay set forever after the first generation.
    pub const ISTAT: u64 = 0x14;
    /// Build-time parameter enumerations, read-only: `MISSION_MODE_RESET_STATE`,
    /// `RAND_SEED_AVAIL` and `MAX_RAND_LENGTH` \[trm\]. Offset named so a bench session can dump
    /// it; nothing here reads it.
    pub const FEATURES: u64 = 0x1C;
    /// The eight output words. Populated once `ISTAT.RAND_RDY` is set. See [`RAND1`]..[`RAND7`].
    pub const RAND0: u64 = 0x20;
    /// See [`RAND0`].
    pub const RAND1: u64 = 0x24;
    /// See [`RAND0`].
    pub const RAND2: u64 = 0x28;
    /// See [`RAND0`].
    pub const RAND3: u64 = 0x2C;
    /// See [`RAND0`].
    pub const RAND4: u64 = 0x30;
    /// See [`RAND0`].
    pub const RAND5: u64 = 0x34;
    /// See [`RAND0`].
    pub const RAND6: u64 = 0x38;
    /// See [`RAND0`].
    pub const RAND7: u64 = 0x3C;
    /// The eight seed words, used to load a host-generated nonce seed and, per \[trm\], "in
    /// several test modes to access internal data". This crate never seeds by nonce (it asks the
    /// ring oscillator for a random reseed instead), so these are offsets only.
    pub const SEED0: u64 = 0x40;
    /// See [`SEED0`]. The set runs `SEED0`..`SEED7` at `0x40`..`0x5C`.
    pub const SEED7: u64 = 0x5C;
    /// Auto-reseed request-count threshold, and **zero is what disables it**: \[trm\] says a
    /// `RQSTS` field of 0 means "disable the `AUTO_RQSTS` alarm feature", other values are a
    /// reload value for the internal counter. \[driver\] writes its `autoreq` module parameter
    /// here during init, which defaults to 0, so upstream's default bring-up disables the alarm
    /// rather than leaving whatever the reset value is in place.
    pub const AUTO_RQSTS: u64 = 0x60;
    /// Auto-reseed age (time) threshold. Same shape as [`AUTO_RQSTS`]: 0 disables the `AUTO_AGE`
    /// alarm, and \[driver\] writes 0 there at init via its `autoage` parameter.
    pub const AUTO_AGE: u64 = 0x64;
    /// Build-time configuration enumerations, read-only \[trm\]. The highest offset the register
    /// map defines, which is why one 4 KiB page covers this device with room to spare.
    pub const BUILD_CONFIG: u64 = 0x68;
}

/// `CTRL`: execute a NOP. \[trm\] lists it as one of the command encodings; the vendor driver
/// issues it once between programming the mode registers and asking for a reseed. Named for
/// completeness rather than used, since \[driver\] and \[netbsd\] both skip it.
pub const CTRL_EXEC_NOP: u32 = 0x0;
/// `CTRL`: ask the device for a fresh random number, 256-bit or 128-bit according to
/// [`MODE_R256`].
pub const CTRL_GENE_RANDNUM: u32 = 0x1;
/// `CTRL`: force a reseed from the ring oscillator. The driver issues this once at init and again
/// on every [`Outcome::Lockup`].
pub const CTRL_EXEC_RANDRESEED: u32 = 0x2;
/// `CTRL`: reseed from the host-written `SEED0..SEED7` words instead of from the ring oscillator
/// \[trm\]. **Named so it is visibly not used**: a nonce reseed makes the output a function of
/// bytes the host chose, which is the opposite of what an entropy source is for, and nothing in
/// this tree should reach for it.
pub const CTRL_EXEC_NONCE_RESEED: u32 = 0x3;

/// `MODE` bit 3: select a 256-bit `RAND0..RAND7` answer rather than a 128-bit `RAND0..RAND3` one.
///
/// **A bring-up that does not set this may be assembling 32 bytes out of a 16-byte answer.** The
/// width after reset is a build-time parameter of the silicon (\[trm\]'s
/// `BUILD_CONFIG.PRNG_LEN_AFTER_RST`), so it cannot be assumed either way from documentation;
/// [`STAT_R256`] is how a driver finds out what it actually got.
pub const MODE_R256: u32 = 1 << 3;

/// `SMODE` bit 8: mission mode (1) rather than test mode (0). In test mode \[trm\] gives the host
/// "access to internal state and test fields", which is not a mode an entropy source should serve
/// from. None of the three drivers cited writes this bit, so this crate names it and leaves the
/// reset value alone; [`STAT_MISSION_MODE`] reports which mode the block is in.
pub const SMODE_MISSION_MODE: u32 = 1 << 8;

/// `STAT` bit 2: nonce mode is enabled, reflecting `SMODE.NONCE_MODE`.
pub const STAT_NONCE_MODE: u32 = 1 << 2;
/// `STAT` bit 3: the `RAND` register set is currently 256 bits wide, reflecting [`MODE_R256`].
pub const STAT_R256: u32 = 1 << 3;
/// `STAT` bit 8: the block is in mission mode, reflecting [`SMODE_MISSION_MODE`].
pub const STAT_MISSION_MODE: u32 = 1 << 8;
/// `STAT` bit 9: the device has been seeded at least once and may be asked to generate.
pub const STAT_SEEDED: u32 = 1 << 9;
/// `STAT` bit 27: there is an unacknowledged service request \[trm\].
pub const STAT_SRVC_RQST: u32 = 1 << 27;
/// `STAT` bit 30: a generation is in flight. The driver's `wait_idle` polls this (and
/// `RAND_SEEDING`) clear before issuing a new command, so two commands are never in flight at
/// once.
pub const STAT_RAND_GENERATING: u32 = 1 << 30;
/// `STAT` bit 31: a reseed is in flight.
pub const STAT_RAND_SEEDING: u32 = 1 << 31;

/// `IE` bit 0: include `RAND_RDY` in the interrupt the block drives.
pub const IE_RAND_RDY_EN: u32 = 1 << 0;
/// `IE` bit 1: include `SEED_DONE`.
pub const IE_SEED_DONE_EN: u32 = 1 << 1;
/// `IE` bit 2: include `AGE_ALARM`.
pub const IE_AGE_ALARM_EN: u32 = 1 << 2;
/// `IE` bit 3: include `RQST_ALARM`.
pub const IE_RQST_ALARM_EN: u32 = 1 << 3;
/// `IE` bit 4: include `LFSR_LOCKUP`.
pub const IE_LFSR_LOCKUP_EN: u32 = 1 << 4;
/// `IE` bit 31: the global interrupt enable.
///
/// **The `IE_*` bits gate the IRQ pin, not [`regs::ISTAT`]**, so a polling driver may leave this
/// register at zero. \[trm\]'s wording ("interrupt **and/or status** contributions") is not
/// decisive; the bench settled it on 2026-09-04, when a reseed and a generation were both seen by
/// polling `ISTAT` with `IE` never written on a VisionFive 2. The completion interrupt needs this
/// bit plus the contribution bits, at PLIC line 30 (\[binding\]'s `interrupts = <30>`); a polling
/// driver needs none of them.
pub const IE_GLBL_EN: u32 = 1 << 31;

/// `ISTAT` bit 0: a generation completed and `RAND0..RAND7` hold a fresh answer.
pub const ISTAT_RAND_RDY: u32 = 1 << 0;
/// `ISTAT` bit 1: a reseed completed.
pub const ISTAT_SEED_DONE: u32 = 1 << 1;
/// `ISTAT` bit 2: the auto-reseed **age** alarm, a reminder that `AUTO_AGE` counted down to zero
/// and the seed is stale \[trm\], \[netbsd\].
///
/// Mainline's `jh7110-trng.c` does not define it, and a bit with no name leaves a status word
/// undecodable. [`interpret`] does not act on it, and a bring-up that writes 0 to
/// [`regs::AUTO_AGE`] never raises it.
pub(crate) const ISTAT_AGE_ALARM: u32 = 1 << 2;
/// `ISTAT` bit 3: the auto-reseed **request-count** alarm, the `AUTO_RQSTS` counterpart of
/// [`ISTAT_AGE_ALARM`] \[trm\], \[netbsd\]. Same status: named, not acted on, and disabled by a
/// bring-up that writes 0 to [`regs::AUTO_RQSTS`].
pub(crate) const ISTAT_RQST_ALARM: u32 = 1 << 3;
/// `ISTAT` bit 4: the hardware's own fault signal, an SEU (single-event upset) in the LFSR-based
/// post-processing stage. The Linux driver treats this as "reseed and try again", never as "trust
/// the RAND registers anyway"; [`interpret`] gives it priority over `RAND_RDY` in the same
/// snapshot for the same reason (a register file that says both "ready" and "faulted" at once
/// must not be read as an answer). See the module doc's "Health testing" section: this is the one
/// hardware self-check the datasheet documents, and it is not a substitute for a statistical
/// health test over the bitstream, which the datasheet does not claim either.
pub const ISTAT_LFSR_LOCKUP: u32 = 1 << 4;

/// Every `ISTAT` bit this crate can name, for the write-1-to-clear that starts a bring-up.
///
/// The init clear discards whatever the block latched before this kernel ran (U-Boot, a previous
/// boot, a stale `RAND_RDY`). \[netbsd\] writes `~0`; this names the five documented bits instead,
/// so the constant says what it acknowledges and an undocumented bit shows up as a residue a
/// driver can report.
pub const ISTAT_ALL: u32 =
    ISTAT_RAND_RDY | ISTAT_SEED_DONE | ISTAT_AGE_ALARM | ISTAT_RQST_ALARM | ISTAT_LFSR_LOCKUP;

/// The device tree `compatible` string \[binding\] defines. `starfive,jh8100-trng` also lists this
/// as a fallback compatible (the JH8100 reuses the same TRNG IP), which is out of scope: this
/// crate names the JH7110's own string only, never the fallback, so it says nothing about a `SoC`
/// it has no board for.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, made public when the
/// device-tree walk left the crate. The binding's own word for the property it holds.
pub const COMPATIBLE: &[u8] = b"starfive,jh7110-trng";

/// **The vendor U-Boot's spelling for the same device**, which the VisionFive 2's control tree
/// uses.
///
/// Same window (`0x1600_C000`), interrupt and clocks, a different string: a stale fork, not
/// different silicon. Accepting it claims the [`regs`] layout, and `StarFive`'s own Linux driver
/// for the block (`drivers/char/hw_random/starfive-trng.c` in `starfive-tech/linux` at
/// `202b558ae34c`) defines byte-for-byte the same offsets and bits as mainline's. The node, in
/// `arch/riscv/dts/jh7110.dtsi` of `starfive-tech/u-boot` at `bfbdce9b86a2`, reads
/// `trng: trng@1600C000 { compatible = "starfive,trng"; reg = <0x0 0x1600C000 0x0 0x4000>;
/// interrupts = <30>; status = "disabled"; }`.
pub const COMPATIBLE_VENDOR: &[u8] = b"starfive,trng";

/// Both spellings, in the order a caller should try them: mainline's first. A tree that carries
/// the standardized binding is describing itself in that language, and that is the one to
/// believe.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone: both spellings, in the
/// order to try them.
pub const COMPATIBLES: [&[u8]; 2] = [COMPATIBLE, COMPATIBLE_VENDOR];

/// Where a tree says the TRNG is: the register window's physical base and size (from `reg`), and
/// the PLIC interrupt number if the tree gives one (`interrupts`, \[binding\]'s single cell).
///
/// A caller fills one in from its own device-tree walk, reading every property from the one node
/// whose `compatible` matched, so an unrelated node under the other spelling never contributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Discovered {
    /// Physical base address of the register window (`reg`'s address cell).
    pub reg_base: u64,
    /// Size of the register window in bytes (`reg`'s size cell). \[binding\]'s example claims
    /// `0x4000`; this crate's [`regs`] only uses the low `0x68` of it.
    pub reg_size: u64,
    /// The PLIC interrupt line, if the tree names one. `None` is not an error: a tree with `reg`
    /// but no `interrupts` still names a real device, one a poll-only driver could still use.
    pub interrupt: Option<u32>,
    /// Which `compatible` string matched: [`COMPATIBLE`] or [`COMPATIBLE_VENDOR`]. Carried rather
    /// than discarded because the two spellings say which firmware's tree is in hand: the vendor
    /// U-Boot's control tree, or something fuller.
    pub compatible: &'static [u8],
    /// What the node's `status` property says, decoded by [`status_okay`] to the one question a
    /// driver has: may this device be used?
    ///
    /// **A caller reports this and need not act on it.** The vendor control tree marks the TRNG
    /// disabled because U-Boot has no driver for it, and the same tree misdescribes another core
    /// in the other direction, so the caller decides. A node the firmware calls disabled is the
    /// first suspect when the window reads all zeros.
    pub status_okay: bool,
}

/// Decode a node's `interrupts` property to the PLIC line, \[binding\]'s single big-endian cell.
/// `None` when the property is shorter than one cell.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, for the property it
/// decodes.
pub fn interrupt(value: &[u8]) -> Option<u32> {
    value
        .get(0..4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_be_bytes)
}

/// Decode a node's `status` property: `true` when it is absent (`None`, since the device-tree
/// specification's default is `"okay"`) or spells `"okay"` or `"ok"`; `false` for `"disabled"`,
/// `"fail"`, `"fail-sss"` and every other value. `"ok"` is a legacy spelling of `"okay"` that trees
/// in the wild still carry, so it is accepted rather than read as a fourth kind of "no".
///
/// The value is a NUL-terminated string in the tree, so both the terminated and the unterminated
/// forms are matched rather than trimmed: a four-way `matches!` says exactly which byte strings
/// count, where a trim would have to decide what an empty or multi-string value means.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after the `Discovered`
/// field it fills.
pub fn status_okay(value: Option<&[u8]>) -> bool {
    value.is_none_or(|v| matches!(v, b"okay\0" | b"ok\0" | b"okay" | b"ok"))
}

/// The result of looking at one `ISTAT` snapshot (and the `RAND` words, if it looked ready).
///
/// Deliberately not `Result`: neither variant is this crate's idea of an error. `NotReady` is the
/// expected answer on every poll but the last; `Lockup` is a real, documented hardware condition a
/// caller is expected to handle (retry after reseed), not a bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// A generation completed cleanly. The 32 bytes are `RAND0..RAND7`, little-endian, word 0
    /// first: see [`assemble`].
    Ready([u8; 32]),
    /// `ISTAT.LFSR_LOCKUP` was set: an SEU, per \[driver\]'s own comment. The caller should issue
    /// `CTRL_EXEC_RANDRESEED` and retry, bounded: ask again a few times, then tell the truth.
    Lockup,
    /// Neither bit was set: still generating, or not yet asked. Not a failure on its own; a caller
    /// polls again, bounded by its own timeout (this crate does not impose one, because how long
    /// "too long" is is a fact about the real device's timing this crate cannot know without it).
    NotReady,
    /// **`STAT.SEEDED` was clear**, so whatever the `RAND` words hold did not come from a seeded
    /// PRNG core and must not be served as randomness.
    ///
    /// This is \[netbsd\]'s gate: its poll path looks at `RAND_RDY` only when [`STAT_SEEDED`] is
    /// set, reseeding otherwise. \[trm\]'s `STAT.LAST_RESEED` value `0x7`, "Unseeded (zeroized
    /// state)", shows the block can really be in this state. A caller reseeds and retries,
    /// bounded, rather than reading the words anyway.
    Unseeded,
}

/// Decide what one `(STAT, ISTAT)` snapshot means, given the `RAND` words alongside it.
///
/// A pure decode: the same `(stat, istat, rand)` always decides the same [`Outcome`]. The
/// precedence is fault, then seeded, then ready. A lockup beats everything, because a register
/// file the hardware is re-seeding must not be read as a clean answer. `SEEDED` beats `RAND_RDY`,
/// because `RAND_RDY` is a latched write-1-to-clear bit that can be standing from before this
/// driver ran, while `STAT.SEEDED` is live state; that is why `stat` is a parameter.
pub fn interpret(stat: u32, istat: u32, rand: [u32; 8]) -> Outcome {
    if istat & ISTAT_LFSR_LOCKUP != 0 {
        Outcome::Lockup
    } else if stat & STAT_SEEDED == 0 {
        Outcome::Unseeded
    } else if istat & ISTAT_RAND_RDY != 0 {
        Outcome::Ready(assemble(rand))
    } else {
        Outcome::NotReady
    }
}

/// Lay the eight `RAND` words out as 32 bytes, little-endian, word 0 first: `RAND0`'s low byte is
/// byte 0. There is no cryptographic transform here: without a one-way function, any reshuffling
/// would change the bytes without adding unpredictability an attacker could not undo. Conditioning,
/// if a caller wants it, belongs to the caller's entropy pool. This is data movement, not conditioning.
pub fn assemble(rand: [u32; 8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, word) in rand.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    out
}

/// How many bytes fit in the one word [`Pool::take`] answers with. Eight, one
/// `u64`: this crate is the device's logic and knows nothing about any wire format, so a caller
/// whose protocol carries a different count clamps to its own.
pub const WORD_BYTES: u64 = 8;

/// **The 32 bytes in hand, and how many are still ours to give**, testable here without a
/// register. [`take`](Pool::take) is handed a generator: a device poll in a driver, a counter in
/// the tests. `cursor` only moves forward, so no byte is served twice, and each byte is zeroed as
/// it leaves.
///
/// # Examples
///
/// ```
/// # // A worker with a deadline, as in `take_returns_rather_than_spinning`: a `take` that stops
/// # // making progress spins, and a doctest that spins never fails, it keeps running.
/// # let (done, finished) = std::sync::mpsc::channel();
/// # std::thread::spawn(move || {
/// use jh7110_entropy::Pool;
///
/// // A device that answers with 32 bytes of 0xAB, forever.
/// let mut generate = || Some([0xab; 32]);
/// let mut pool = Pool::new();
/// assert_eq!(pool.take(4, &mut generate), (4, 0xabab_abab));
///
/// // A device that never answers: the caller is told, rather than spun on.
/// let mut dry = || None;
/// let mut pool = Pool::new();
/// assert_eq!(pool.take(4, &mut dry), (0, 0));
/// # let _ = done.send(());
/// # });
/// # finished
/// #     .recv_timeout(std::time::Duration::from_secs(5))
/// #     .expect("the example panicked (see above), or take never returned");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pool {
    buf: [u8; 32],
    cursor: usize,
    filled: usize,
}

impl Default for Pool {
    fn default() -> Self {
        Self::new()
    }
}

impl Pool {
    /// An empty pool: the first [`take`](Pool::take) generates.
    #[must_use]
    pub const fn new() -> Self {
        Pool {
            buf: [0; 32],
            cursor: 0,
            filled: 0,
        }
    }

    /// How many bytes are left before the next generation. Test and diagnostic support; the
    /// program reports it once, in its readiness message.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.filled - self.cursor
    }

    /// Fill from `generate`, discarding whatever was left. `false` when the generator would not
    /// answer, which the caller reports rather than retries forever.
    pub fn refill(&mut self, generate: &mut impl FnMut() -> Option<[u8; 32]>) -> bool {
        match generate() {
            Some(bytes) => {
                self.buf = bytes;
                self.cursor = 0;
                self.filled = 32;
                true
            }
            None => false,
        }
    }

    /// Take `n` bytes, as a little-endian word plus a count. Gathers across a refill boundary, so
    /// a request can straddle two generations without the client ever seeing the seam, and returns
    /// a **short count** rather than zeros when the device stops answering mid-gather.
    ///
    /// **`n` is clamped to [`WORD_BYTES`]**, because the answer is one 64-bit word and there is
    /// nowhere to put a ninth byte. A caller whose protocol already clamps to 8 never reaches this
    /// clamp; it is here because a
    /// public function that shifts by `8 * n` must not be callable into an overflow, and the first
    /// host test written against this API found exactly that edge.
    pub fn take(&mut self, n: u64, generate: &mut impl FnMut() -> Option<[u8; 32]>) -> (u64, u64) {
        let n = n.min(WORD_BYTES);
        let mut word = 0u64;
        let mut got = 0u64;
        while got < n {
            if self.cursor == self.filled && !self.refill(generate) {
                break;
            }
            let run = (n - got).min((self.filled - self.cursor) as u64);
            for i in 0..run {
                let at = self.cursor + i as usize;
                word |= u64::from(self.buf[at]) << (8 * (got + i));
                self.buf[at] = 0;
            }
            self.cursor += run as usize;
            got += run;
        }
        (got, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_ready_when_neither_bit_is_set() {
        assert_eq!(interpret(STAT_SEEDED, 0, [0; 8]), Outcome::NotReady);
        assert_eq!(
            interpret(STAT_SEEDED, ISTAT_SEED_DONE, [0xffff_ffff; 8]),
            Outcome::NotReady
        );
    }

    #[test]
    fn ready_assembles_little_endian_word_zero_first() {
        let rand = [
            0x0403_0201,
            0x0807_0605,
            0x0c0b_0a09,
            0x100f_0e0d,
            0x1413_1211,
            0x1817_1615,
            0x1c1b_1a19,
            0x201f_1e1d,
        ];
        match interpret(STAT_SEEDED, ISTAT_RAND_RDY, rand) {
            Outcome::Ready(bytes) => {
                let expected: [u8; 32] = core::array::from_fn(|i| (i + 1) as u8);
                assert_eq!(bytes, expected);
            }
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    #[test]
    fn an_unseeded_core_is_not_read_as_an_answer() {
        // The failure this prevents: `RAND_RDY` is latched and write-1-to-clear, so it can be
        // standing from before this driver ran. Believing it on a core whose `STAT` says it was
        // never seeded is how a driver serves a register file nobody put entropy into.
        assert_eq!(
            interpret(0, ISTAT_RAND_RDY, [0xffff_ffff; 8]),
            Outcome::Unseeded
        );
        // ...and a fault still outranks it, because a reseeding register file is not a "just
        // reseed it" situation, it is one already in progress.
        assert_eq!(
            interpret(0, ISTAT_RAND_RDY | ISTAT_LFSR_LOCKUP, [0; 8]),
            Outcome::Lockup
        );
    }

    #[test]
    fn every_istat_bit_the_trm_names_is_in_the_acknowledge_mask() {
        // `ISTAT_ALL` is what a bring-up writes to clear the register. A bit named in this crate
        // but missing from the mask would be one the init clear silently leaves latched.
        for bit in [
            ISTAT_RAND_RDY,
            ISTAT_SEED_DONE,
            ISTAT_AGE_ALARM,
            ISTAT_RQST_ALARM,
            ISTAT_LFSR_LOCKUP,
        ] {
            assert_eq!(ISTAT_ALL & bit, bit, "{bit:#x} is not acknowledged");
        }
        // Bits 0..=4, which is every bit [trm] documents in this register.
        assert_eq!(ISTAT_ALL, 0b1_1111);
    }

    #[test]
    fn lockup_beats_ready_in_the_same_snapshot() {
        assert_eq!(
            interpret(
                STAT_SEEDED,
                ISTAT_RAND_RDY | ISTAT_LFSR_LOCKUP,
                [0xffff_ffff; 8]
            ),
            Outcome::Lockup
        );
    }

    #[test]
    fn lockup_alone_is_reported_even_with_no_rand_rdy() {
        assert_eq!(
            interpret(STAT_SEEDED, ISTAT_LFSR_LOCKUP, [0; 8]),
            Outcome::Lockup
        );
    }

    #[test]
    fn seed_done_alone_is_not_confused_with_rand_rdy() {
        // SEED_DONE (bit 1) and RAND_RDY (bit 0) are adjacent; a shift-by-one bug here would pass
        // every other test in this file and fail silently on real hardware after a reseed.
        assert_eq!(
            interpret(STAT_SEEDED, ISTAT_SEED_DONE, [0; 8]),
            Outcome::NotReady
        );
    }

    /// `status` decoding, at the four values a tree can carry, one it cannot, and the absent
    /// property. This is a three-line function guarding a fact a bench session will read off the
    /// boot tour, and the failure it would cause is the expensive kind: a `status` misread as okay
    /// turns "the firmware never ungated this device's clocks" into an unexplained window of zeros.
    #[test]
    fn status_is_decoded_the_way_the_specification_defines_it() {
        assert!(status_okay(None));
        assert!(status_okay(Some(b"okay\0")));
        assert!(status_okay(Some(b"ok\0")));
        assert!(status_okay(Some(b"okay")));
        assert!(status_okay(Some(b"ok")));
        assert!(!status_okay(Some(b"disabled\0")));
        assert!(!status_okay(Some(b"fail\0")));
        assert!(!status_okay(Some(b"")));
    }

    /// `interrupts` is one big-endian cell; a short property names no line, and a longer one
    /// is read from its first cell only.
    #[test]
    fn the_interrupt_is_the_first_big_endian_cell() {
        assert_eq!(interrupt(&[0, 0, 0, 30]), Some(30));
        assert_eq!(
            interrupt(&[0, 0, 1, 2, 0xff, 0xff, 0xff, 0xff]),
            Some(0x102)
        );
        assert_eq!(interrupt(&[0, 0, 30]), None);
        assert_eq!(interrupt(&[]), None);
    }

    /// Mainline's spelling is tried first, and the two spellings are the two trees' own.
    #[test]
    fn both_trees_spellings_in_mainline_first_order() {
        assert_eq!(
            COMPATIBLES,
            [&b"starfive,jh7110-trng"[..], &b"starfive,trng"[..]]
        );
    }

    /// **No byte is ever served twice**, across as many refills as it takes to drain several
    /// generations. The generator hands out a distinct byte per call, so a cursor that wrapped
    /// instead of refilling, or a refill that did not reset the cursor, shows up as a repeat.
    #[test]
    fn no_byte_is_served_twice_across_refills() {
        let mut next = 1u8;
        let mut generate = || {
            let block = core::array::from_fn(|i| next.wrapping_add(i as u8));
            next = next.wrapping_add(32);
            Some(block)
        };
        let mut pool = Pool::new();
        let mut seen = [0u32; 256];
        // 40 draws of 8 bytes is 320 bytes, ten generations, so the seam is crossed nine times.
        for _ in 0..40 {
            let (got, word) = pool.take(8, &mut generate);
            assert_eq!(got, 8);
            for b in word.to_le_bytes() {
                seen[b as usize] += 1;
            }
        }
        // The generator's byte stream is 1, 2, 3, ... wrapping, so 320 bytes visits every value
        // once or twice and never three times; a repeat from the pool would push one to three.
        for (value, &count) in seen.iter().enumerate() {
            assert!(count <= 2, "byte {value} came back {count} times");
        }
    }

    /// **A request that straddles a generation gets all of its bytes**, in order, with the seam
    /// invisible: the last four of one block and the first four of the next.
    #[test]
    fn a_request_straddles_the_seam_without_losing_bytes() {
        let mut which = 0u8;
        let mut generate = || {
            which += 1;
            Some([which; 32])
        };
        let mut pool = Pool::new();
        // Five-byte draws: the seventh starts at byte 30 of a 32-byte block, so it takes two
        // bytes from block 1 and three from block 2.
        for _ in 0..6 {
            assert_eq!(pool.take(5, &mut generate).0, 5);
        }
        let (got, word) = pool.take(5, &mut generate);
        assert_eq!(got, 5);
        assert_eq!(word.to_le_bytes()[..5], [1, 1, 2, 2, 2]);
    }

    /// **A device that stops answering produces a short count, not zeros.** The distinction is the
    /// whole of an entropy service's honesty: a client that asked for eight bytes and got four must
    /// be told four, or it will treat four zeros as entropy.
    #[test]
    fn a_dry_device_shortens_the_count_rather_than_padding() {
        let mut answers = 1;
        let mut generate = || {
            if answers > 0 {
                answers -= 1;
                Some([0xff; 32])
            } else {
                None
            }
        };
        let mut pool = Pool::new();
        assert_eq!(pool.take(8, &mut generate).0, 8);
        // Drain the one block this generator will ever give: 32 bytes, so three more eights.
        for _ in 0..3 {
            assert_eq!(pool.take(8, &mut generate).0, 8);
        }
        assert_eq!(pool.take(8, &mut generate), (0, 0));
    }

    /// **A served byte is not still sitting in the buffer.** Reading the same region back after a
    /// take must not reproduce it; the pool zeroes behind its cursor.
    #[test]
    fn a_served_byte_is_zeroed_behind_the_cursor() {
        let mut generate = || Some([0xa5; 32]);
        let mut pool = Pool::new();
        assert_eq!(pool.take(8, &mut generate), (8, 0xa5a5_a5a5_a5a5_a5a5));
        assert_eq!(pool.buf[..8], [0u8; 8]);
        assert_eq!(pool.remaining(), 24);
    }

    /// **A caller asking for more than a word gets a word**, not a shift-left overflow. The wire
    /// format clamps first, so this is defence at the API rather than a live path.
    #[test]
    fn a_request_larger_than_the_word_is_clamped() {
        let mut generate = || Some([0x11; 32]);
        let mut pool = Pool::new();
        assert_eq!(pool.take(64, &mut generate), (8, 0x1111_1111_1111_1111));
    }

    /// **Every named bit is the bit its own documentation says it is, and no two bits in a
    /// register are the same bit.**
    ///
    /// A transcription check: the constants came off \[trm\]'s tables by hand, and a bit copied to
    /// the wrong line, or a `<<` turned `>>` that takes a bit to zero, passes every test that does
    /// not name the bit it exercises. A mutation sweep found fifteen survivors of that shape. A duplicated bit inside one register is
    /// the copy-and-paste mistake the table invites.
    #[test]
    fn every_named_bit_is_the_bit_the_datasheet_names() {
        // One row per named bit: (register, name, bit index, constant). The indices are \[trm\]'s
        // own numbering, repeated from each constant's doc comment so the two cannot drift apart
        // silently. Flat rather than grouped by register, so the table reads down the page the way
        // the datasheet's own does.
        let named: [(&str, &str, u32, u32); 20] = [
            ("MODE", "R256", 3, MODE_R256),
            ("SMODE", "MISSION_MODE", 8, SMODE_MISSION_MODE),
            ("STAT", "NONCE_MODE", 2, STAT_NONCE_MODE),
            ("STAT", "R256", 3, STAT_R256),
            ("STAT", "MISSION_MODE", 8, STAT_MISSION_MODE),
            ("STAT", "SEEDED", 9, STAT_SEEDED),
            ("STAT", "SRVC_RQST", 27, STAT_SRVC_RQST),
            ("STAT", "RAND_GENERATING", 30, STAT_RAND_GENERATING),
            ("STAT", "RAND_SEEDING", 31, STAT_RAND_SEEDING),
            ("IE", "RAND_RDY_EN", 0, IE_RAND_RDY_EN),
            ("IE", "SEED_DONE_EN", 1, IE_SEED_DONE_EN),
            ("IE", "AGE_ALARM_EN", 2, IE_AGE_ALARM_EN),
            ("IE", "RQST_ALARM_EN", 3, IE_RQST_ALARM_EN),
            ("IE", "LFSR_LOCKUP_EN", 4, IE_LFSR_LOCKUP_EN),
            ("IE", "GLBL_EN", 31, IE_GLBL_EN),
            ("ISTAT", "RAND_RDY", 0, ISTAT_RAND_RDY),
            ("ISTAT", "SEED_DONE", 1, ISTAT_SEED_DONE),
            ("ISTAT", "AGE_ALARM", 2, ISTAT_AGE_ALARM),
            ("ISTAT", "RQST_ALARM", 3, ISTAT_RQST_ALARM),
            ("ISTAT", "LFSR_LOCKUP", 4, ISTAT_LFSR_LOCKUP),
        ];
        // The `CTRL_*` constants are deliberately absent: they are an enumerated command value
        // (0, 1, 2, 3) written to one field, not bits in a mask, so neither assertion below is
        // true of them and neither would mean anything if it were.

        for &(register, name, index, value) in &named {
            assert_eq!(
                value.count_ones(),
                1,
                "{register}.{name} is not one bit: {value:#010x}"
            );
            assert_eq!(
                value.trailing_zeros(),
                index,
                "{register}.{name} is at bit {}, and its own documentation says {index}",
                value.trailing_zeros()
            );
        }

        for (a, &(register_a, name_a, _, value_a)) in named.iter().enumerate() {
            for &(register_b, name_b, _, value_b) in &named[a + 1..] {
                if register_a == register_b {
                    assert_ne!(
                        value_a, value_b,
                        "{register_a}.{name_a} and {register_b}.{name_b} are the same bit"
                    );
                }
            }
        }
    }

    /// **[`ISTAT_ALL`] is five distinct bits**, which is what makes the write-1-to-clear an
    /// acknowledgement of exactly the bits this crate can name rather than a `!0` that swallows
    /// whatever else the block latched. Disjointness is the reason it may be written with either
    /// `|` or `^`, which a mutation sweep had to argue for four surviving mutants and would rather a
    /// reader could simply check.
    #[test]
    fn the_clear_mask_is_every_named_istat_bit_and_nothing_else() {
        assert_eq!(ISTAT_ALL.count_ones(), 5, "{ISTAT_ALL:#010x}");
        for bit in [
            ISTAT_RAND_RDY,
            ISTAT_SEED_DONE,
            ISTAT_AGE_ALARM,
            ISTAT_RQST_ALARM,
            ISTAT_LFSR_LOCKUP,
        ] {
            assert_eq!(ISTAT_ALL & bit, bit, "{bit:#010x} is not in the clear mask");
        }
    }

    /// **`take` returns**, and what this asserts is that it returns at all rather than what it
    /// returns.
    ///
    /// Every pass of the gather loop must make progress (refill, copy, advance cursor and count);
    /// break one and the call never answers. A mutation sweep found four such mutants, each of which
    /// hung the suite, so this test runs `take` under a deadline.
    #[test]
    fn take_returns_rather_than_spinning() {
        assert_eq!(
            take_within_a_deadline(4, || Some([0xab; 32])),
            (4, 0xabab_abab)
        );
        assert_eq!(take_within_a_deadline(8, || None), (0, 0));

        // One answer and then nothing, so a single call crosses from "has bytes" to "dry".
        let mut answers = 1;
        assert_eq!(
            take_within_a_deadline(8, move || {
                if answers > 0 {
                    answers -= 1;
                    Some([0x5a; 32])
                } else {
                    None
                }
            })
            .0,
            8
        );
    }

    /// The deadline itself. A fresh [`Pool`] per call, because the point is the call and not the
    /// history; a panic inside is reported by the worker's own default hook.
    fn take_within_a_deadline(
        n: u64,
        mut generate: impl FnMut() -> Option<[u8; 32]> + Send + 'static,
    ) -> (u64, u64) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut pool = Pool::new();
            let _ = tx.send(pool.take(n, &mut generate));
        });
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("take never returned: its gather loop made no progress")
    }
}

/// Machine-checked proofs, run with `cargo kani`.
/// The unit tests sample `interpret` at chosen bit patterns; `ISTAT` is a register a fault could
/// set to anything, and `interpret` is a few comparisons over two `u32`s, so these exhaust the
/// whole input space instead.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **A hardware fault is never read as a clean answer.** For every `istat` with
    /// `LFSR_LOCKUP` set, `interpret` returns `Lockup`, regardless of what `RAND_RDY` or any other
    /// bit says and regardless of what the `RAND` words happen to hold. This is the property the
    /// module doc's "Health testing" section argues for in prose; here it holds for the full
    /// `2^32` space of `istat`, not the two combinations the unit tests above pick.
    /// Falsification: replayable `crates/jh7110_entropy/falsifications/verification.a_lockup_bit_is_never_overridden.patch`
    #[kani::proof]
    fn a_lockup_bit_is_never_overridden() {
        let stat: u32 = kani::any();
        let istat: u32 = kani::any();
        kani::assume(istat & ISTAT_LFSR_LOCKUP != 0);
        let rand: [u32; 8] = kani::any();
        assert_eq!(interpret(stat, istat, rand), Outcome::Lockup);
    }

    /// **`Ready` is returned exactly when the register file says so, and carries exactly those
    /// bytes.** No `istat` value produces `Ready` unless `RAND_RDY` is set and `LFSR_LOCKUP` is
    /// clear, and when it is, the bytes are `rand`'s little-endian encoding with nothing dropped,
    /// substituted, or reordered.
    /// Falsification: replayable `crates/jh7110_entropy/falsifications/verification.ready_requires_rand_rdy_and_carries_the_words_untouched.patch`
    #[kani::proof]
    fn ready_requires_rand_rdy_and_carries_the_words_untouched() {
        let stat: u32 = kani::any();
        let istat: u32 = kani::any();
        kani::assume(istat & ISTAT_LFSR_LOCKUP == 0 && istat & ISTAT_RAND_RDY != 0);
        kani::assume(stat & STAT_SEEDED != 0);
        let rand: [u32; 8] = kani::any();
        let Outcome::Ready(bytes) = interpret(stat, istat, rand) else {
            panic!("RAND_RDY set and LFSR_LOCKUP clear did not read as Ready");
        };
        // **The layout written out, not `assemble` compared with itself.**
        // `interpret` calls `assemble`, so `assert_eq!(interpret(..), Ready(assemble(rand)))`
        // is satisfied by any consistently wrong `assemble`: a byte-swap, a word reversal or a
        // silent truncation would leave both sides equal and this harness green, while the doc
        // above claims the bytes are little-endian with nothing reordered. Indexing the words
        // by hand states that claim in terms the implementation cannot pick.
        let mut i = 0;
        while i < 32 {
            assert_eq!(
                bytes[i],
                (rand[i / 4] >> (8 * (i % 4))) as u8,
                "a RAND word reached the caller reordered, truncated or substituted",
            );
            i += 1;
        }
    }

    /// **`NotReady` is the only answer when neither bit is set**, whatever the rest of `istat` or
    /// the stale `RAND` words say: a device that has not finished is not accidentally read as done
    /// or as faulted because some other bit happened to be set.
    /// Falsification: replayable `crates/jh7110_entropy/falsifications/verification.neither_bit_set_is_always_not_ready.patch`
    #[kani::proof]
    fn neither_bit_set_is_always_not_ready() {
        let stat: u32 = kani::any();
        let istat: u32 = kani::any();
        kani::assume(istat & ISTAT_LFSR_LOCKUP == 0 && istat & ISTAT_RAND_RDY == 0);
        kani::assume(stat & STAT_SEEDED != 0);
        let rand: [u32; 8] = kani::any();
        assert_eq!(interpret(stat, istat, rand), Outcome::NotReady);
    }

    /// **An unseeded core never yields bytes, whatever `ISTAT` claims**. For every
    /// `istat` with `LFSR_LOCKUP` clear, if `STAT.SEEDED` is clear then `interpret` returns
    /// `Unseeded` rather than `Ready`, so no path exists from a latched `RAND_RDY` on an unseeded
    /// device to 32 bytes handed to a caller. This is the property `Outcome::Unseeded`'s doc
    /// argues for in prose, over the full `2^64` space of the two status words.
    /// Falsification: replayable `crates/jh7110_entropy/falsifications/verification.an_unseeded_core_never_yields_bytes.patch`
    #[kani::proof]
    fn an_unseeded_core_never_yields_bytes() {
        let stat: u32 = kani::any();
        let istat: u32 = kani::any();
        kani::assume(istat & ISTAT_LFSR_LOCKUP == 0);
        kani::assume(stat & STAT_SEEDED == 0);
        let rand: [u32; 8] = kani::any();
        assert_eq!(interpret(stat, istat, rand), Outcome::Unseeded);
    }
}
