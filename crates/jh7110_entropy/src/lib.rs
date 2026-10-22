#![cfg_attr(not(test), no_std)]
//! **The `StarFive` JH7110 TRNG, as pure logic** (notes/entropy.md): the register
//! layout, the device-tree query that finds the device, and the decision of whether a generation
//! attempt succeeded, failed or is still running. It never dereferences a pointer, which keeps it
//! host-testable and Kani-reachable; `components/src/jh7110_entropy.rs` is the backend that
//! touches the registers. [`discover`] returns the address rather than assuming one.
//!
//! **None of it has run against real silicon.** Offsets and bits come from Linux's driver, named
//! from the TRM and checked against NetBSD's polling driver, whose rule this crate applies:
//! `ISTAT.RAND_RDY` counts only when `STAT.SEEDED` is set. `notes/entropy/jh7110-trng.md` has the
//! sources, the bring-up order all three drivers agree on, and the health-testing argument.
//!
//! # Health testing
//!
//! The one hardware fault signal is `ISTAT.LFSR_LOCKUP`, surfaced as [`Outcome::Lockup`] rather
//! than "not ready", so a caller can count it, retry or refuse. It does not catch a degraded ring
//! oscillator, and whether a software SP 800-90B test is needed is an undecided architect's call.
//!
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
//!
//! Name: ratified 2026-09-13 (calef); kept as the crates.io name (calef, 2026-10-07 UTC, #1806).

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
    /// **A driver that never writes this register does not get 32 usable bytes**, and the earlier
    /// version of this comment (that reading all eight words made the write unnecessary) had it
    /// backwards. \[trm\]'s `FEATURES.MAX_RAND_LENGTH` and `BUILD_CONFIG.PRNG_LEN_AFTER_RST` are
    /// build-time parameters, so the width this block resets to is a property of the silicon and
    /// not something a reader can assume; `STAT.R256` ([`super::STAT_R256`]) reports what it
    /// currently is. In 128-bit mode only `RAND0..RAND3` carry the answer, which is exactly why
    /// \[driver\] clamps its read to four words when it has not selected `R256`, and why all
    /// three drivers this crate cites write `MODE` during bring-up.
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
    /// writing a one to it, which is what \[driver\] and \[netbsd\] both do and what this crate's
    /// earlier "not confirmed from the summarized driver source" note could not say. A driver that
    /// only ever reads this register sees `RAND_RDY` stay set forever after the first generation.
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
/// **The `IE_*` bits gate the IRQ pin, not [`regs::ISTAT`]**, and that distinction is what lets a
/// polling driver leave this register at zero. \[trm\] describes each `IE` bit as including or
/// excluding an "interrupt contribution" and describes `ISTAT` as monitoring "the interrupt
/// **and/or status** contributions", which reads as latch-regardless but is not decisive on its
/// own. **The board settled it**: on 2026-09-04 radon completed a reseed and a generation, both
/// detected by polling `ISTAT`, with this driver having never written `IE` at all
/// (`target/board/radon-2026-09-04-clock-and-first-entropy.log`). So `ISTAT` latches with the
/// interrupt disabled, measured rather than inferred.
///
/// A driver that wants the completion interrupt instead needs this bit plus the contribution bits,
/// routed at PLIC line 30 per \[binding\]'s `interrupts = <30>`. `components/src/jh7110_entropy.rs` says
/// why it does not.
pub const IE_GLBL_EN: u32 = 1 << 31;

/// `ISTAT` bit 0: a generation completed and `RAND0..RAND7` hold a fresh answer.
pub const ISTAT_RAND_RDY: u32 = 1 << 0;
/// `ISTAT` bit 1: a reseed completed.
pub const ISTAT_SEED_DONE: u32 = 1 << 1;
/// `ISTAT` bit 2: the auto-reseed **age** alarm, a reminder that `AUTO_AGE` counted down to zero
/// and the seed is stale \[trm\], \[netbsd\].
///
/// **Named here because it was missing, and missing bits are how a status word becomes
/// unreadable.** Mainline's `jh7110-trng.c` does not define this bit at all, and this crate was
/// transcribed from it, so a bench session that read a 2 or an 8 in `ISTAT` had nothing in this
/// tree to decode it with. It is a reminder rather than a fault: [`interpret`] does not act on it,
/// and a bring-up that writes 0 to [`regs::AUTO_AGE`] never raises it.
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
/// A driver clears the whole register rather than the bits it cares about, because the point of
/// the init clear is to discard whatever the block latched before this kernel existed: U-Boot, a
/// previous boot of nife, or a reset that left a stale `RAND_RDY` standing. \[netbsd\] writes
/// `~0` for exactly this and \[driver\] reads-then-writes-back, which reaches the same place for
/// the bits that are set. This crate names the five documented bits instead of `!0` so the
/// constant says what it is acknowledging, and so a future undocumented bit shows up as a residue
/// a driver can report rather than being silently swallowed.
pub const ISTAT_ALL: u32 =
    ISTAT_RAND_RDY | ISTAT_SEED_DONE | ISTAT_AGE_ALARM | ISTAT_RQST_ALARM | ISTAT_LFSR_LOCKUP;

/// The device tree `compatible` string \[binding\] defines. `starfive,jh8100-trng` also lists this
/// as a fallback compatible (the JH8100 reuses the same TRNG IP), which is out of scope: this
/// crate matches the JH7110's own string only, never the fallback, so it says nothing about a `SoC`
/// this project has no board for.
pub(crate) const COMPATIBLE: &[u8] = b"starfive,jh7110-trng";

/// **The spelling radon's own firmware uses for the same device**, and the reason milestone 239
/// (radon's device tree does not describe the TRNG, so a working driver never runs) exists.
///
/// The tree nife is handed on the VisionFive 2 is the vendor U-Boot's control DTB (U-Boot 2021.10,
/// per `crates/serial_console/tests/fixtures/captured/vf2-2026-09-01-manual-boot.log`), from
/// `StarFive`'s fork, which spells the node \[uboot-dtsi\]:
///
/// ```text
/// trng: trng@1600C000 {
///     compatible = "starfive,trng";
///     reg = <0x0 0x1600C000 0x0 0x4000>;
///     clocks = <&clkgen JH7110_SEC_HCLK>,
///          <&clkgen JH7110_SEC_MISCAHB_CLK>;
///     clock-names = "hclk", "miscahb_clk";
///     resets = <&rstgen RSTN_U0_SEC_TOP_HRESETN>;
///     interrupts = <30>;
///     status = "disabled";
/// };
/// ```
///
/// **Same device, same window, same interrupt, different string**: a stale fork, not different
/// silicon. `StarFive`'s own kernel driver for this block \[vendor-driver\] defines its registers
/// byte-for-byte as the offsets in [`regs`].
///
/// **Accepting it is a claim about the register layout, so here is the evidence for that claim.**
/// The vendor Linux driver at the firmware's own vintage \[vendor-driver\] defines `STARFIVE_CTRL`
/// 0x00, `STAT` 0x04, `MODE` 0x08, `SMODE` 0x0C, `IE` 0x10, `ISTAT` 0x14, `RAND0`..`RAND7`
/// 0x20..0x3C, `AUTO_RQSTS` 0x60, `AUTO_AGE` 0x64, and the same `CTRL`/`STAT`/`ISTAT` bit
/// positions mainline's `jh7110-trng.c` does. Two drivers written against one IP block agree
/// completely, so a node claiming `starfive,trng` at `0x1600_C000` is claiming [`regs`].
///
/// \[uboot-dtsi\]: `arch/riscv/dts/jh7110.dtsi` in `starfive-tech/u-boot` at `bfbdce9b86a2`:
/// <https://github.com/starfive-tech/u-boot/blob/bfbdce9b86a2/arch/riscv/dts/jh7110.dtsi>
///
/// \[vendor-driver\]: `drivers/char/hw_random/starfive-trng.c` in `starfive-tech/linux` at
/// `202b558ae34c`:
/// <https://github.com/starfive-tech/linux/blob/202b558ae34c/drivers/char/hw_random/starfive-trng.c>
pub const COMPATIBLE_VENDOR: &[u8] = b"starfive,trng";

/// What [`discover`] found: the register window's physical base and size (from `reg`), and the
/// PLIC interrupt number if the tree gives one (`interrupts`, \[binding\]'s single cell).
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
    /// than discarded because the two spellings mean different things about the tree a bench
    /// session is looking at, and a boot tour that says which one it found tells the next reader
    /// whether they are on the vendor U-Boot's control DTB or on something fuller.
    pub compatible: &'static [u8],
    /// What the node's `status` property says, decoded to the one question a driver has: may this
    /// device be used? `true` when the property is absent (the device-tree specification's default
    /// is `"okay"`) or spells `"okay"`/`"ok"`; `false` for `"disabled"` and every other value.
    ///
    /// **[`discover`] reports this and does not act on it**, deliberately. The vendor control tree
    /// marks the TRNG `status = "disabled"` because *U-Boot* has no driver for it, not because the
    /// silicon is absent: `StarFive`'s own Linux enables the same node from
    /// `jh7110-common.dtsi` (`&trng { status = "okay"; };`). This tree also has a recorded reason
    /// not to take that firmware's `status` at face value, in the other direction: the same DTB
    /// marks the S7 monitor core `status = "okay"` and claims it has an Sv39 MMU, and both are
    /// false (notes/visionfive2.md, "Second bench stop"). So the honest thing is to say what the
    /// tree says and let the caller decide, rather than to refuse a device that is there or trust
    /// one that is not.
    ///
    /// It is also the first thing to read when the register window comes back all zeros: a node
    /// the firmware calls disabled is a node whose clocks the firmware had no reason to ungate.
    pub status_okay: bool,
}

/// Find the JH7110 TRNG in `tree`, if it is there at all.
///
/// **`Ok(None)` is not a parse failure, it is the honest answer on every machine this repository
/// currently boots under CI**: QEMU's riscv64 `virt` board has no TRNG node under either spelling,
/// so this function must return `None` against it, and the fixture test below pins exactly that
/// against the same `.dtb` `crates/device_tree_blob`'s own tests already boot-verify against.
/// Discovery is the one piece of "does this device exist at all" this crate can prove without
/// silicon: it is a pure query over bytes, and a device tree dumped from the real board (once
/// someone captures one) is a drop-in fixture for the same test, not a new code path.
///
/// **Two spellings are tried, mainline's first** ([`COMPATIBLE`], then [`COMPATIBLE_VENDOR`]), and
/// the order is the whole of the policy: a tree that carries both is describing itself in the
/// language the binding standardised, and that is the one to believe. Trying the vendor string at
/// all is milestone 239's finding; [`COMPATIBLE_VENDOR`] carries the evidence that it names the
/// same register block.
pub fn discover(
    tree: &device_tree_blob::DeviceTreeBlob<'_>,
) -> Result<Option<Discovered>, device_tree_blob::Error> {
    for compatible in [COMPATIBLE, COMPATIBLE_VENDOR] {
        if let Some(found) = discover_as(tree, compatible)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// One spelling's worth of [`discover`]. Split out so the two attempts cannot drift apart: every
/// property is read against the *same* string that matched the node, so a tree carrying an
/// unrelated node under the other spelling can never contribute a `reg` or an `interrupts` to this
/// answer.
fn discover_as(
    tree: &device_tree_blob::DeviceTreeBlob<'_>,
    compatible: &'static [u8],
) -> Result<Option<Discovered>, device_tree_blob::Error> {
    let mut regions = [device_tree_blob::Region { start: 0, size: 0 }; 1];
    let n = tree.node_reg_compatible(compatible, &mut regions)?;
    if n == 0 {
        return Ok(None);
    }
    let interrupt = tree
        .node_prop_compatible(compatible, b"interrupts")?
        .and_then(|bytes| bytes.get(0..4))
        .and_then(|b| b.try_into().ok())
        .map(u32::from_be_bytes);
    let status_okay = tree
        .node_prop_compatible(compatible, b"status")?
        .is_none_or(status_says_okay);
    Ok(Some(Discovered {
        reg_base: regions[0].start,
        reg_size: regions[0].size,
        interrupt,
        compatible,
        status_okay,
    }))
}

/// Decode a `status` property value. The device-tree specification defines `"okay"` as the only
/// value meaning "operational", with `"disabled"`, `"fail"` and `"fail-sss"` all meaning it is
/// not; `"ok"` is a widespread legacy spelling of `"okay"` that trees in the wild still carry, so
/// it is accepted here rather than read as a fourth kind of "no".
///
/// The value is a NUL-terminated string in the tree, so both the terminated and the unterminated
/// forms are matched here rather than trimming: a four-way `matches!` says exactly which byte
/// strings count, where a trim would have to decide what an empty or multi-string value means and
/// would answer for cases no tree produces.
fn status_says_okay(value: &[u8]) -> bool {
    matches!(value, b"okay\0" | b"ok\0" | b"okay" | b"ok")
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
    /// `CTRL_EXEC_RANDRESEED` and retry, bounded, the same "ask again, bounded, then tell the
    /// truth" shape `entropy.rs`'s `REFILL_TRIES` already uses for a dry virtio-rng device.
    Lockup,
    /// Neither bit was set: still generating, or not yet asked. Not a failure on its own; a caller
    /// polls again, bounded by its own timeout (this crate does not impose one, because how long
    /// "too long" is is a fact about the real device's timing this crate cannot know without it).
    NotReady,
    /// **`STAT.SEEDED` was clear**, so whatever the `RAND` words hold did not come from a seeded
    /// PRNG core and must not be served as randomness.
    ///
    /// This is \[netbsd\]'s gate, and it is the one check the other two drivers leave implicit:
    /// its poll path reads `STAT` and only looks at `RAND_RDY` when [`STAT_SEEDED`] is set,
    /// issuing [`CTRL_EXEC_RANDRESEED`] when it is not, and its interrupt handler asserts
    /// `STAT & SEEDED` whenever `RAND_RDY` fires. \[trm\] backs it: `STAT.LAST_RESEED` has an
    /// enumerated value `0x7` meaning "Unseeded (zeroized state)", so an unseeded core is a state
    /// the block can genuinely be in and report.
    ///
    /// A caller answers this by reseeding and retrying, bounded, not by reading the words anyway.
    Unseeded,
}

/// Decide what one `(STAT, ISTAT)` snapshot means, given the `RAND` words alongside it.
///
/// `rand` is read every call regardless of the status words, which costs nothing (it is already in
/// hand, the same eight loads a real driver would do) and keeps this function a pure decode with
/// no hidden state: the same `(stat, istat, rand)` triple always decides the same [`Outcome`].
///
/// **The precedence is fault, then seeded, then ready**, and each step is refusing to hand back
/// bytes for a different reason:
///
/// - **Lockup beats everything**, even though the bits live in unrelated positions and nothing in
///   \[driver\] says they cannot both be set in one snapshot: a register file the hardware is
///   actively re-seeding must not be read as if it had just answered cleanly, so a caller cannot
///   be handed 32 bytes from underneath a fault.
/// - **`SEEDED` beats `RAND_RDY`** (\[netbsd\]'s gate; see [`Outcome::Unseeded`]). `RAND_RDY` is
///   a latched, write-1-to-clear bit, so it can be standing from a generation that happened before
///   this driver ran or before the last reset, while `STAT.SEEDED` is live state. Trusting the
///   latch alone is how a driver serves the contents of a register file nobody seeded.
///
/// **`stat` is a parameter because reading it is not optional** (milestone 159). An earlier version
/// of this function took `istat` alone, which made "the device says it has never been seeded" a
/// state this crate could not express and a driver could not check.
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
/// byte 0. There is no cryptographic transform here for `entropy.rs`'s own reason (DECISIONS
/// §44): with no one-way function in this tree, any reshuffling would change the bytes without
/// adding unpredictability an attacker could not undo. This is data movement, not conditioning.
pub fn assemble(rand: [u32; 8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, word) in rand.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    out
}

/// How many bytes fit in the one word [`Pool::take`] answers with. The same 8 as
/// `entropy_contract::MAX_BYTES`, stated here rather than depended on: this crate is the device's
/// logic and knows nothing about the wire format, and the two agreeing is a fact the driver
/// program (which depends on both) is where a reader can check.
pub const WORD_BYTES: u64 = 8;

/// **The 32 bytes in hand, and how many are still ours to give** (milestone 159), lifted out of
/// `components/src/jh7110_entropy.rs` so it can be tested somewhere a register does not have to exist.
///
/// This is the one piece of the driver that can serve a byte twice, hand back a byte it already
/// zeroed, or lose the seam between two generations, and none of that is visible in the register
/// decode above. It has no idea where its bytes come from: [`take`](Pool::take) is handed a
/// generator, which in the program is a poll of a real device and in the tests below is a counter.
///
/// **Not a pool in the "reservoir" sense.** `cursor` only ever moves forward, so no byte is served
/// twice, and each byte is zeroed as it leaves: a byte a client now holds is not also still
/// sitting in a buffer a long-lived process keeps for the rest of the boot. The same shape
/// `components/src/entropy.rs`'s virtio-rng `Pool` has, at a quarter the size, because there is no
/// device round trip here to amortize over.
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
    /// nowhere to put a ninth byte. `entropy_contract::want` already clamps to the same 8 before the
    /// driver ever calls this, so in the program the clamp is unreachable; it is here because a
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

    const JH7110_TRNG_PRESENT: &[u8] = include_bytes!("../tests/fixtures/jh7110-trng-present.dtb");
    const QEMU_RISCV64_VIRT: &[u8] =
        include_bytes!("../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb");

    #[test]
    fn discover_finds_the_device_on_a_tree_shaped_like_the_real_board() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(JH7110_TRNG_PRESENT)
            .expect("fixture parses");
        let found = discover(&tree)
            .expect("no parse error")
            .expect("the node is there");
        assert_eq!(found.reg_base, 0x1600_C000);
        assert_eq!(found.reg_size, 0x4000);
        assert_eq!(found.interrupt, Some(30));
        assert_eq!(found.compatible, COMPATIBLE);
        // Mainline's binding example states no `status`, and the device-tree specification's
        // default for an absent `status` is "okay".
        assert!(found.status_okay);
    }

    const JH7110_TRNG_VENDOR_UBOOT: &[u8] =
        include_bytes!("../tests/fixtures/jh7110-trng-vendor-uboot.dtb");

    /// **Milestone 239's whole finding, in one assertion.** radon is handed the vendor U-Boot's
    /// control DTB, and that tree does describe the TRNG: as `trng@1600C000`, `compatible =
    /// "starfive,trng"`, `status = "disabled"`. Before this milestone `discover` matched mainline's
    /// string only and answered `None`, so the boot tour printed "this machine's tree describes no
    /// starfive,jh7110-trng" against a tree that describes the device at the address the driver
    /// wanted.
    ///
    /// The fixture is transcribed from the firmware's own source, not dumped from the board (see
    /// its header), so what this test proves is that the decoder handles that shape. **Whether the
    /// running firmware's tree really carries the node is a bench fact and is still open**;
    /// `design/roadmap/0239-radons-tree-describes-less-than-the-chip-has.md` carries the two
    /// commands that settle it.
    #[test]
    fn discover_finds_the_device_under_the_vendor_uboots_own_spelling() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(JH7110_TRNG_VENDOR_UBOOT)
            .expect("fixture parses");
        let found = discover(&tree)
            .expect("no parse error")
            .expect("the vendor tree describes the device under its own compatible");
        assert_eq!(found.reg_base, 0x1600_C000);
        assert_eq!(found.reg_size, 0x4000);
        assert_eq!(found.interrupt, Some(30));
        assert_eq!(found.compatible, COMPATIBLE_VENDOR);
        // The one property a caller must not lose: the firmware calls its own TRNG disabled.
        assert!(!found.status_okay);
    }

    /// `status` decoding, at the four values a tree can carry and one it cannot. This is a
    /// three-line function guarding a fact a bench session will read off the boot tour, and the
    /// failure it would cause is the expensive kind: a `status` misread as okay turns "the
    /// firmware never ungated this device's clocks" into an unexplained window of zeros.
    #[test]
    fn status_is_decoded_the_way_the_specification_defines_it() {
        assert!(status_says_okay(b"okay\0"));
        assert!(status_says_okay(b"ok\0"));
        assert!(!status_says_okay(b"disabled\0"));
        assert!(!status_says_okay(b"fail\0"));
        assert!(!status_says_okay(b""));
    }

    /// **The negative case this milestone can actually prove without hardware.** QEMU's riscv64
    /// `virt` machine (the board every kernel test in this repository boots against) has no
    /// `starfive,jh7110-trng` node, so `discover` must say so rather than finding a false match at
    /// some unrelated node's `reg`. This is
    /// `crates/device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb` itself, the same bytes
    /// `crates/device_tree_blob/tests/qemu_riscv64_virt.rs` already boot-verifies, so a green
    /// result here is a claim about the tree this project actually runs, not a claim about a tree
    /// nobody has looked at.
    /// **No byte is ever served twice**, across as many refills as it takes to drain several
    /// generations. The generator hands out a distinct byte per call, so a cursor that wrapped
    /// instead of refilling, or a refill that did not reset the cursor, shows up as a repeat here
    /// rather than as a security property nobody checked.
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
    /// whole of `entropy_contract`'s honesty: a client that asked for eight bytes and got four must
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
    /// This is a transcription check and says so: the constants came off \[trm\]'s register tables
    /// by hand, one line at a time, and the failure a hand transcription produces is not a clever
    /// one. It is a bit copied to the wrong line, which reads as a device that is permanently in
    /// mission mode, or never seeded, or that reports a lockup the silicon never raised. Fifteen
    /// of this crate's mutation survivors were exactly this shape: a `<<` becoming
    /// a `>>`, which takes a named bit to **zero**, at which point `stat & STAT_SEEDED` is false
    /// forever and every test that samples [`interpret`] still passes, because none of them names
    /// the bit it is exercising. Found by milestone 326 (nobody has been assigned to turn a mutation score upward).
    ///
    /// The distinctness half is the one a reader should care about more. A duplicated bit inside a
    /// register is the copy-and-paste mistake this table invites, and it is invisible in the
    /// source: two constants twenty lines apart, both `1 << 3`, look correct individually.
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
    /// `|` or `^`, which milestone 326 had to argue for four surviving mutants and would rather a
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
    /// Every pass of the gather loop ends by making progress: it refills a spent buffer, copies a
    /// run, and advances both the cursor and the count. Break any one of those and the function
    /// does not give a wrong answer, it gives none. Milestone 326 found four mutants here (a
    /// `refill` reporting success without filling, `got < n` widened to `got <= n`, the
    /// spent-buffer test inverted, and `got += run` turned into a multiply) and every one of them
    /// spun, with the suite above answering by hanging, which `cargo mutants` can only report as a
    /// timeout. The same shape `memory_corruption_canary_gate`'s own tests carry, and for the same
    /// reason: a loop that waits is broken by making it wait forever.
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

    #[test]
    fn discover_finds_nothing_on_qemus_virt_board() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(QEMU_RISCV64_VIRT)
            .expect("fixture parses");
        assert_eq!(discover(&tree).expect("no parse error"), None);
    }
}

/// Machine-checked proofs (DECISIONS §14). The four `#[test]`s above sample `interpret`'s
/// classification at a handful of chosen bit patterns; `ISTAT` is a real register a fault could
/// set to anything, so what matters is the classification rule holding for **every** 32-bit value,
/// not the four this crate's author thought to write down. That is exactly the gap Kani closes
/// cheaply here: `interpret` is a few comparisons over two `u32`s (well, one `u32` and an unused
/// `[u32; 8]` for the `NotReady`/`Lockup` cases), so the whole input space is small enough to
/// exhaust rather than sample.
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
        // **The layout written out, not `assemble` compared with itself** (milestone 211).
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

    /// **An unseeded core never yields bytes, whatever `ISTAT` claims** (milestone 159). For every
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
