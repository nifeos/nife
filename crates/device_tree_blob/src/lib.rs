//! A minimal flattened device tree (FDT) parser: bytes in, regions and property values out.
//!
//! The blob is the machine describing itself (where RAM, the UART and the interrupt controller
//! are, how many CPUs exist), handed over by firmware (notes/boot-protocol.md). Every integer in
//! it is big-endian, so every read goes through `be32` or `be64`. The blob is untrusted input:
//! every offset is checked, every walk is bounded, and a malformed blob is an [`Error`] rather
//! than a panic. The crate is pure logic, so its tests run on the host against trees dumped from
//! QEMU (§7 (testing: QEMU harness + host-testable crates)). See notes/device-tree.md.
//!

//! Name: ratified 2026-09-18 (calef, `design/decisions/` §154), **deratifying the 2026-08-01
//! ratification** to do it, and performed 2026-09-19. Refused `dtb`.
//!
//! §154's test is whether the expansion is a phrase people actually say. "device tree blob" is,
//! so it goes, where `pci` stays because "peripheral component interconnect" is not.
//!
//! The 2026-08-01 block called this one of the standard terms "already right and must not be
//! touched". That was an exemption rather than a test, and §154 records the three-layer
//! contradiction the exemptions left behind.
//!
//! **Known cost**: datasheets, error messages and this kernel's own output say `dtb`, so a reader
//! grepping the word the machine printed will not find this identifier. Weighed and accepted.
//!
//! **Performed 2026-09-19**, with the type (`Dtb` to `DeviceTreeBlob`) and the fuzz target
//! (`dtb_walk` to `device_tree_blob_walk`, dictionary included) under calef's rulings of that
//! day. Census of `dtb` as a word, not counting the blob files' extension or `dtc -I dtb`: 327 before, 159
//! after. Of the survivors, 64 name **the blob rather than the crate** and pass the ownership test
//! the other way: `let dtb`, `kernel_main(dtb)`, `crate::DTB`, `dtb_ptr`, `configure_from_dtb`,
//! and the test file `crates/pci/tests/qemu_virt_dtb.rs`. 15 are in `design/decisions/`, 2 are this
//! block's refusal and known cost, and 78 are the old name in an account, a measurement or a
//! ruling. **The blob files' extension did not move**: 86 occurrences and 20 files before and after,
//! every filename unchanged, the five fixtures here included (their directory moved, they did not).

#![cfg_attr(not(test), no_std)]

/// A contiguous span of physical memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// The physical address of the first byte.
    pub start: u64,
    /// Length in bytes.
    pub size: u64,
}

impl Region {
    /// One past the last byte, **saturating**.
    ///
    /// Saturating because the type is `pub` with `pub` fields, so its values may come from a
    /// hostile blob, and `kernel/src/memory.rs`'s `place_bitmap` calls this on every RAM region
    /// before a boot can report a failure; a plain add would panic there under overflow checks
    /// (regression in `tests/hostile.rs`). A saturated end is still wrong, so the parser refuses a
    /// wrapping region with [`Error::RegionOverflow`]; this is the backstop, not the check. See
    /// notes/device-tree.md ("The parser's bounds").
    pub fn end(&self) -> u64 {
        self.start.saturating_add(self.size)
    }
}

/// Why parsing the blob failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The first four bytes weren't `0xd00dfeed`. Either the pointer is wrong or the
    /// bootloader didn't give us a device tree at all.
    BadMagic(u32),
    /// The blob claims a version we don't understand.
    UnsupportedVersion(u32),
    /// An offset in the header points outside the blob. Truncated or corrupt.
    Truncated,
    /// A token we don't recognize. The structure block is malformed.
    BadToken(u32),
    /// The caller's output slice was too small to hold every region found.
    TooManyRegions,
    /// The header's `totalsize` is larger than [`MAX_TOTALSIZE`], so
    /// [`from_ptr`](DeviceTreeBlob::from_ptr) refused to build a slice of that length over
    /// memory nobody has checked. Carries the size claimed.
    TooLarge(usize),
    /// A `reg` pair whose `start + size` does not fit in 64 bits, so the region names memory that
    /// cannot exist.
    ///
    /// Refused rather than clamped, and here rather than left to [`Region::end`]: the caller is
    /// `kernel/src/memory.rs` deciding where RAM is, and a boot path should be told the firmware's
    /// memory map is impossible rather than quietly work around it.
    RegionOverflow,
}

// Structure-block tokens.
const FDT_BEGIN_NODE: u32 = 0x1;
const FDT_END_NODE: u32 = 0x2;
const FDT_PROP: u32 = 0x3;
const FDT_NOP: u32 = 0x4;
const FDT_END: u32 = 0x9;

const MAGIC: u32 = 0xd00d_feed;
const HEADER_LEN: usize = 40;

/// The largest `totalsize` [`DeviceTreeBlob::from_ptr`] will believe, 2 MiB.
///
/// `from_ptr` trusts the header's length before anything is validated, and the magic says only
/// that the pointer is at a blob. Without this ceiling a `totalsize` of `0xffff_ffff` becomes a
/// 4 GiB slice read past the end of RAM, a data abort in `memory::init` before any handler can say
/// why (2026-09-24 security audit). 2 MiB is Linux's `MAX_FDT_SIZE` (`SZ_2M` in
/// `arch/arm64/include/asm/boot.h`); the largest blob this tree has met is QEMU `virt`'s, under
/// 64 KiB. See notes/device-tree.md ("The parser's bounds").
pub const MAX_TOTALSIZE: usize = 2 * 1024 * 1024;

fn be32(bytes: &[u8], at: usize) -> Result<u32, Error> {
    // `at + 4` is a checked add, not a bare one: `at` comes straight out of the (untrusted) blob,
    // and a near-`usize::MAX` offset would otherwise panic on the overflow before `get` ever runs.
    // Proved total in the verification module.
    let end = at.checked_add(4).ok_or(Error::Truncated)?;
    let slice = bytes.get(at..end).ok_or(Error::Truncated)?;
    Ok(u32::from_be_bytes(slice.try_into().unwrap()))
}

fn be64(bytes: &[u8], at: usize) -> Result<u64, Error> {
    let end = at.checked_add(8).ok_or(Error::Truncated)?;
    let slice = bytes.get(at..end).ok_or(Error::Truncated)?;
    Ok(u64::from_be_bytes(slice.try_into().unwrap()))
}

/// A parsed, borrowed device tree blob.
#[derive(Debug)]
pub struct DeviceTreeBlob<'a> {
    bytes: &'a [u8],
    off_struct: usize,
    off_strings: usize,
    off_rsvmap: usize,
}

impl<'a> DeviceTreeBlob<'a> {
    /// # Safety
    ///
    /// `ptr` must point at a device tree blob that stays valid for `'a`. We read the
    /// header to learn the blob's own length, which means we trust the first 8 bytes
    /// before we have validated anything. The magic check immediately after is what
    /// makes that survivable: a wrong pointer almost certainly fails it.
    pub unsafe fn from_ptr(ptr: *const u8) -> Result<Self, Error> {
        // Read just enough to learn how long the thing claims to be.
        // SAFETY: the caller's contract is that `ptr` points at a blob valid for `'a`. This reads only the fixed-size header, which is the deliberate leap of faith the magic check on the next line exists to catch.
        let header = unsafe { core::slice::from_raw_parts(ptr, HEADER_LEN) };
        let magic = be32(header, 0)?;
        if magic != MAGIC {
            return Err(Error::BadMagic(magic));
        }
        let total = be32(header, 4)? as usize;
        if total > MAX_TOTALSIZE {
            return Err(Error::TooLarge(total));
        }

        // SAFETY: the magic checked out, so `ptr` really is a blob, and `total` is the length it
        // declares for itself, now bounded by `MAX_TOTALSIZE`; the caller's contract covers the
        // whole of it for `'a`.
        let bytes = unsafe { core::slice::from_raw_parts(ptr, total) };
        Self::from_bytes(bytes)
    }

    /// Parse a blob already in hand as a safe slice, rather than through an unchecked pointer.
    /// This is what [`from_ptr`](Self::from_ptr) delegates to once it has trusted the header
    /// enough to build one, and it is the entry point every host test uses directly.
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, Error> {
        let magic = be32(bytes, 0)?;
        if magic != MAGIC {
            return Err(Error::BadMagic(magic));
        }

        let total = be32(bytes, 4)? as usize;
        if bytes.len() < total || total < HEADER_LEN {
            return Err(Error::Truncated);
        }

        // We only understand version 17 and later, which is everything made since about
        // 2005. The `last_comp_version` field is the blob telling us the oldest reader
        // it is still compatible with.
        let last_comp_version = be32(bytes, 24)?;
        if last_comp_version > 17 {
            return Err(Error::UnsupportedVersion(last_comp_version));
        }

        let dtb = DeviceTreeBlob {
            off_struct: be32(bytes, 8)? as usize,
            off_strings: be32(bytes, 12)? as usize,
            off_rsvmap: be32(bytes, 16)? as usize,
            bytes: &bytes[..total],
        };

        if dtb.off_struct >= total || dtb.off_strings >= total || dtb.off_rsvmap >= total {
            return Err(Error::Truncated);
        }

        Ok(dtb)
    }

    /// How many bytes the blob occupies. We need this to mark it as reserved: it is
    /// sitting in the very RAM we are about to start handing out.
    pub fn total_size(&self) -> usize {
        self.bytes.len()
    }

    /// The memory reservation block: regions the bootloader is telling us **not to
    /// touch**, before we've parsed a single node.
    ///
    /// This is a separate, deliberately dead-simple structure precisely so that a
    /// kernel can honour it without having to parse anything. QEMU's `virt` leaves it
    /// empty, but a real board's firmware often does not, and a kernel that skips it
    /// will happily allocate over the firmware's own tables.
    pub fn reserved_regions(&self, out: &mut [Region]) -> Result<usize, Error> {
        let mut at = self.off_rsvmap;
        let mut n = 0;

        loop {
            let start = be64(self.bytes, at)?;
            let size = be64(self.bytes, at + 8)?;
            at += 16;

            // The list is terminated by an all-zero entry.
            if start == 0 && size == 0 {
                return Ok(n);
            }

            *out.get_mut(n).ok_or(Error::TooManyRegions)? = Region { start, size };
            n += 1;
        }
    }

    /// Every `/memory` node's `reg` property: the actual RAM, written into `out`.
    ///
    /// Returns how many regions it wrote; refuses a full `out` with [`Error::TooManyRegions`] and a
    /// wrapping pair with [`Error::RegionOverflow`].
    pub fn memory_regions(&self, out: &mut [Region]) -> Result<usize, Error> {
        // `reg` is a list of (address, size) pairs, but how many 32-bit cells each of
        // those takes is not fixed. It's declared by #address-cells and #size-cells on
        // the PARENT node. For a /memory node the parent is the root.
        //
        // The spec's defaults are 2 and 1. Nearly every 64-bit machine says 2 and 2
        // (i.e. 64-bit addresses, 64-bit sizes), but we read them rather than assume.
        let mut address_cells = 2u32;
        let mut size_cells = 1u32;

        let mut found = 0;
        let mut depth = 0i32;

        // The depth at which we entered a /memory node, or None.
        //
        // Tracking the *depth* rather than a bare "am I inside one" flag matters: a
        // node's properties are the ones seen while `depth` equals the node's own depth.
        // If a /memory node ever had a child, a bare flag would be cleared by the
        // child's END_NODE and we'd stop reading the parent's remaining properties. No
        // real device tree does this today, which is exactly why it would be a lurking
        // bug rather than an obvious one.
        let mut memory_at: Option<i32> = None;
        let mut at = self.off_struct;

        loop {
            let token = be32(self.bytes, at)?;
            at += 4;

            match token {
                FDT_BEGIN_NODE => {
                    let name = self.cstr(at)?;
                    at += align4(name.len() + 1);
                    depth += 1;

                    // A memory node is a child of the root named `memory` or
                    // `memory@<address>`. (There is also a `device_type = "memory"`
                    // property, which is the more correct check, but it arrives *after*
                    // the node name and the name is unambiguous in practice.)
                    if depth == 2 && (name == b"memory" || name.starts_with(b"memory@")) {
                        memory_at = Some(depth);
                    }
                }

                FDT_END_NODE => {
                    if memory_at == Some(depth) {
                        memory_at = None;
                    }
                    depth -= 1;
                }

                FDT_PROP => {
                    let len = be32(self.bytes, at)? as usize;
                    let name_off = be32(self.bytes, at + 4)? as usize;
                    let value_at = at + 8;
                    at = value_at + align4(len);

                    let name = self.cstr(self.off_strings + name_off)?;

                    // The root's cell counts, which we need before we can decode any
                    // `reg`. They appear on the root node, which we visit first, so by
                    // the time we reach a /memory node these are correct.
                    if depth == 1 {
                        match name {
                            b"#address-cells" => address_cells = be32(self.bytes, value_at)?,
                            b"#size-cells" => size_cells = be32(self.bytes, value_at)?,
                            _ => {}
                        }
                    }

                    if memory_at == Some(depth) && name == b"reg" {
                        found += self.decode_reg(
                            value_at,
                            len,
                            address_cells,
                            size_cells,
                            &mut out[found..],
                        )?;
                    }
                }

                FDT_NOP => {}
                FDT_END => return Ok(found),
                other => return Err(Error::BadToken(other)),
            }
        }
    }

    /// The `reg` regions of the first node whose name starts with `prefix`, searched at any depth.
    ///
    /// Decodes with the `#address-cells`/`#size-cells` of the node's **parent**, carried down a
    /// per-depth stack, so a device nested under `/soc` reads as well as one under the root. A GIC
    /// decodes to two regions, distributor first, as its binding orders them. Matching a name prefix
    /// rather than `compatible` is a simplification; [`node_reg_compatible`](Self::node_reg_compatible)
    /// is the correct lookup. A node deeper than the stack is not matched. See notes/device-tree.md
    /// ("Finding a node").
    pub fn node_reg(&self, prefix: &[u8], out: &mut [Region]) -> Result<usize, Error> {
        // #address-cells/#size-cells declared by the node at each depth, applying to its children.
        // Depth 0 is the pre-root sentinel; the root is depth 1. Spec defaults are 2 and 1.
        const MAX_DEPTH: usize = 16;
        let mut acells = [2u32; MAX_DEPTH];
        let mut scells = [1u32; MAX_DEPTH];

        let mut depth = 0usize;
        let mut target_at: Option<usize> = None;
        let mut at = self.off_struct;

        loop {
            let token = be32(self.bytes, at)?;
            at += 4;

            match token {
                FDT_BEGIN_NODE => {
                    let name = self.cstr(at)?;
                    at += align4(name.len() + 1);
                    depth += 1;
                    // Inherit the parent's cell counts as this node's defaults, until/unless this
                    // node declares its own (its props, processed below, come before its children).
                    if depth < MAX_DEPTH {
                        acells[depth] = acells[depth - 1];
                        scells[depth] = scells[depth - 1];
                    }

                    // `(2..MAX_DEPTH)`, not `>= 2`: the `reg` arm below reads `acells[depth - 1]`,
                    // which is out of bounds once `depth` reaches 17, a boot-time panic on a hostile
                    // blob (tests/hostile.rs). Past `MAX_DEPTH` the cell counts are no longer
                    // tracked, so refusing to match is honest where a clamped index would decode the
                    // wrong widths. `node_reg_compatible` refuses the same way at its END_NODE.
                    if (2..MAX_DEPTH).contains(&depth)
                        && name.starts_with(prefix)
                        && target_at.is_none()
                    {
                        target_at = Some(depth);
                    }
                }

                FDT_END_NODE => {
                    if target_at == Some(depth) {
                        // We have walked the whole node. If it had a `reg` we already decoded
                        // it; either way, stop looking.
                        target_at = None;
                    }
                    depth = depth.saturating_sub(1);
                }

                FDT_PROP => {
                    let len = be32(self.bytes, at)? as usize;
                    let name_off = be32(self.bytes, at + 4)? as usize;
                    let value_at = at + 8;
                    at = value_at + align4(len);

                    let name = self.cstr(self.off_strings + name_off)?;

                    // Record the cell counts this node declares for its children (depth+1 will read
                    // them off the stack). A node's own `reg`, decoded below, uses its PARENT's.
                    if depth < MAX_DEPTH {
                        match name {
                            b"#address-cells" => acells[depth] = be32(self.bytes, value_at)?,
                            b"#size-cells" => scells[depth] = be32(self.bytes, value_at)?,
                            _ => {}
                        }
                    }

                    if target_at == Some(depth) && name == b"reg" {
                        // Decode with the parent's cells: the node one level up (depth - 1).
                        return self.decode_reg(
                            value_at,
                            len,
                            acells[depth - 1],
                            scells[depth - 1],
                            out,
                        );
                    }
                }

                FDT_NOP => {}
                FDT_END => return Ok(0),
                other => return Err(Error::BadToken(other)),
            }
        }
    }

    /// The `reg` regions of the first node whose `compatible` list contains `compat`, searched at
    /// any depth.
    ///
    /// A match on **any** NUL-separated entry counts, as the binding intends: a node that says
    /// `"starfive,jh7110-rtc\0arm,pl031"` claims the PL031 register layout. `compatible` may follow
    /// `reg` in the same node, so each open node's `reg` is remembered on a per-depth stack and
    /// decoded when the matched node closes; a matched parent is still answered after its
    /// children close. Returns zero regions for a match with no `reg`. See notes/device-tree.md
    /// ("Finding a node").
    pub fn node_reg_compatible(&self, compat: &[u8], out: &mut [Region]) -> Result<usize, Error> {
        const MAX_DEPTH: usize = 16;
        let mut acells = [2u32; MAX_DEPTH];
        let mut scells = [1u32; MAX_DEPTH];
        // Per open node: where its `reg` value sits and how wide its cells are, and whether its
        // `compatible` named the device we are looking for.
        let mut reg = [None::<(usize, usize, u32, u32)>; MAX_DEPTH];
        let mut matched = [false; MAX_DEPTH];

        let mut depth = 0usize;
        let mut at = self.off_struct;

        loop {
            let token = be32(self.bytes, at)?;
            at += 4;

            match token {
                FDT_BEGIN_NODE => {
                    let name = self.cstr(at)?;
                    at += align4(name.len() + 1);
                    depth += 1;
                    if depth < MAX_DEPTH {
                        acells[depth] = acells[depth - 1];
                        scells[depth] = scells[depth - 1];
                        reg[depth] = None;
                        matched[depth] = false;
                    }
                }

                FDT_END_NODE => {
                    // Depth 1 is the root, which has no `compatible` worth matching and whose `reg`
                    // would have no parent to decode against.
                    if (2..MAX_DEPTH).contains(&depth) && matched[depth] {
                        return match reg[depth] {
                            Some((value_at, len, a, s)) => {
                                self.decode_reg(value_at, len, a, s, out)
                            }
                            // Compatible but no `reg`: the device exists and has no register block
                            // we can name. Zero regions, not an error.
                            None => Ok(0),
                        };
                    }
                    depth = depth.saturating_sub(1);
                }

                FDT_PROP => {
                    let len = be32(self.bytes, at)? as usize;
                    let name_off = be32(self.bytes, at + 4)? as usize;
                    let value_at = at + 8;
                    at = value_at + align4(len);

                    let name = self.cstr(self.off_strings + name_off)?;

                    if depth < MAX_DEPTH {
                        match name {
                            b"#address-cells" => acells[depth] = be32(self.bytes, value_at)?,
                            b"#size-cells" => scells[depth] = be32(self.bytes, value_at)?,
                            b"reg" if depth >= 2 => {
                                reg[depth] =
                                    Some((value_at, len, acells[depth - 1], scells[depth - 1]));
                            }
                            b"compatible" => {
                                let bytes = self
                                    .bytes
                                    .get(value_at..value_at + len)
                                    .ok_or(Error::Truncated)?;
                                matched[depth] = bytes.split(|&b| b == 0).any(|s| s == compat);
                            }
                            _ => {}
                        }
                    }
                }

                FDT_NOP => {}
                FDT_END => return Ok(0),
                other => return Err(Error::BadToken(other)),
            }
        }
    }

    /// The raw bytes of property `name` on the first node whose `compatible` list contains
    /// `compat`, searched at any depth. `Ok(None)` when no such node exists, or the node has no
    /// such property.
    ///
    /// The property twin of [`node_reg_compatible`](Self::node_reg_compatible), deciding when the
    /// matched node closes for the same reason. It exists because labels differ between machines:
    /// the PLIC is `plic@c000000` on QEMU `virt` and `interrupt-controller@c000000` on the JH7110.
    pub fn node_prop_compatible(
        &self,
        compat: &[u8],
        name: &[u8],
    ) -> Result<Option<&'a [u8]>, Error> {
        self.prop_compatible_up(compat, name, 0)
    }

    /// The raw bytes of property `name` on the **parent** of the first node whose `compatible`
    /// list contains `compat`. `Ok(None)` when no such node exists, or its parent has no such
    /// property.
    ///
    /// For a device described as a child of its bus, such as a PMIC under one of seven identical
    /// `snps,designware-i2c` controllers, whose `clocks` and `resets` sit on the bus (milestone 592
    /// (radon's cold reboot dies in OpenSBI's PMIC write), provisional). The parent's slot is filled
    /// when the child closes, because a node's properties precede its first subnode.
    ///
    /// Name: provisional (milestone 592), 2026-09-25. calef names public functions.
    pub fn parent_prop_compatible(
        &self,
        compat: &[u8],
        name: &[u8],
    ) -> Result<Option<&'a [u8]>, Error> {
        self.prop_compatible_up(compat, name, 1)
    }

    /// The shared walk behind the two `*_prop_compatible` reads: property `name` on the node `up`
    /// levels above the first node matching `compat` (0 for the node itself, 1 for its parent).
    fn prop_compatible_up(
        &self,
        compat: &[u8],
        name: &[u8],
        up: usize,
    ) -> Result<Option<&'a [u8]>, Error> {
        const MAX_DEPTH: usize = 16;
        // Per open node: where `name`'s value sits, and whether `compatible` matched.
        let mut found = [None::<(usize, usize)>; MAX_DEPTH];
        let mut matched = [false; MAX_DEPTH];

        let mut depth = 0usize;
        let mut at = self.off_struct;

        loop {
            let token = be32(self.bytes, at)?;
            at += 4;

            match token {
                FDT_BEGIN_NODE => {
                    let nm = self.cstr(at)?;
                    at += align4(nm.len() + 1);
                    depth += 1;
                    if depth < MAX_DEPTH {
                        found[depth] = None;
                        matched[depth] = false;
                    }
                }

                FDT_END_NODE => {
                    // Depth 1 is the root, which carries no `compatible` worth matching here; a
                    // node past MAX_DEPTH was never tracked, so it cannot answer (the same refusal
                    // node_reg records: not finding is honest, misreporting is not).
                    if (2..MAX_DEPTH).contains(&depth) && matched[depth] {
                        // `depth >= 2` and `up <= 1` keep this at the root or below it.
                        return match found[depth - up] {
                            Some((value_at, len)) => self
                                .bytes
                                .get(value_at..value_at + len)
                                .map(Some)
                                .ok_or(Error::Truncated),
                            None => Ok(None),
                        };
                    }
                    depth = depth.saturating_sub(1);
                }

                FDT_PROP => {
                    let len = be32(self.bytes, at)? as usize;
                    let name_off = be32(self.bytes, at + 4)? as usize;
                    let value_at = at + 8;
                    at = value_at + align4(len);

                    let pname = self.cstr(self.off_strings + name_off)?;
                    if depth < MAX_DEPTH {
                        // Two independent `if`s, so asking for `compatible` itself still answers.
                        if pname == name {
                            found[depth] = Some((value_at, len));
                        }
                        if pname == b"compatible" {
                            let bytes = self
                                .bytes
                                .get(value_at..value_at + len)
                                .ok_or(Error::Truncated)?;
                            matched[depth] = bytes.split(|&b| b == 0).any(|s| s == compat);
                        }
                    }
                }

                FDT_NOP => {}
                FDT_END => return Ok(None),
                other => return Err(Error::BadToken(other)),
            }
        }
    }

    /// The raw bytes of property `name` on the first node whose name starts with `prefix`,
    /// searched at any depth. `Ok(None)` when no such node, or the node has no such property.
    ///
    /// The generic escape hatch next to the decoded accessors above: some properties have
    /// per-binding layouts no general decoder can know (`interrupt-map` is the motivating case:
    /// its entry width depends on three different nodes' cell counts). The caller gets the bytes
    /// (big-endian cells, per the DTB spec) and owns the interpretation; the pci crate's fixture
    /// tests use this to hold the INTx swizzle against the machine's own routing table.
    pub fn node_prop(&self, prefix: &[u8], name: &[u8]) -> Result<Option<&'a [u8]>, Error> {
        let mut depth = 0usize;
        let mut target_at: Option<usize> = None;
        let mut at = self.off_struct;

        loop {
            let token = be32(self.bytes, at)?;
            at += 4;

            match token {
                FDT_BEGIN_NODE => {
                    let nm = self.cstr(at)?;
                    at += align4(nm.len() + 1);
                    depth += 1;
                    if depth >= 2 && nm.starts_with(prefix) && target_at.is_none() {
                        target_at = Some(depth);
                    }
                }

                FDT_END_NODE => {
                    if target_at == Some(depth) {
                        // Walked the whole matched node: it has no such property.
                        return Ok(None);
                    }
                    depth = depth.saturating_sub(1);
                }

                FDT_PROP => {
                    let len = be32(self.bytes, at)? as usize;
                    let name_off = be32(self.bytes, at + 4)? as usize;
                    let value_at = at + 8;
                    at = value_at + align4(len);

                    if target_at == Some(depth) && self.cstr(self.off_strings + name_off)? == name {
                        return self
                            .bytes
                            .get(value_at..value_at + len)
                            .map(Some)
                            .ok_or(Error::Truncated);
                    }
                }

                FDT_NOP => {}
                FDT_END => return Ok(None),
                other => return Err(Error::BadToken(other)),
            }
        }
    }

    /// Property `name` on the first node whose name starts with `prefix`, **falling back to the
    /// nearest ancestor** that carries it. `Ok(None)` when no such node, or neither the node nor
    /// any ancestor has the property.
    ///
    /// For the properties the spec defines as inheritable, such as `interrupt-parent`, which QEMU's
    /// two `virt` boards and the JH7110 dtsi each write at a different level. The nearest enclosing
    /// value wins, per the spec. A node's properties precede its children, so one walk suffices:
    /// the answer is taken at the target's `END_NODE`. See notes/device-tree.md.
    pub fn node_prop_inherited(
        &self,
        prefix: &[u8],
        name: &[u8],
    ) -> Result<Option<&'a [u8]>, Error> {
        const MAX_DEPTH: usize = 16;
        // Where `name`'s value sits on the node open at each depth, `None` where it has none. A
        // BEGIN_NODE resets its own depth's slot, so a closed sibling's value can never leak into
        // the ancestor scan (which only reads the depths still open above the target anyway).
        let mut values = [None::<(usize, usize)>; MAX_DEPTH];

        let mut depth = 0usize;
        let mut target_at: Option<usize> = None;
        let mut at = self.off_struct;

        loop {
            let token = be32(self.bytes, at)?;
            at += 4;

            match token {
                FDT_BEGIN_NODE => {
                    let nm = self.cstr(at)?;
                    at += align4(nm.len() + 1);
                    depth += 1;
                    if depth < MAX_DEPTH {
                        values[depth] = None;
                    }
                    // The same `(2..MAX_DEPTH)` refusal as `node_reg`: a node deeper than the
                    // value stack was never tracked, and answering for it would mean answering
                    // from ancestors we stopped recording. Not finding it is honest.
                    if (2..MAX_DEPTH).contains(&depth)
                        && nm.starts_with(prefix)
                        && target_at.is_none()
                    {
                        target_at = Some(depth);
                    }
                }

                FDT_END_NODE => {
                    if target_at == Some(depth) {
                        // The whole target node has been walked: its own value if it had one,
                        // else the nearest open ancestor's, root included.
                        for d in (1..=depth).rev() {
                            if let Some((value_at, len)) = values[d] {
                                return self
                                    .bytes
                                    .get(value_at..value_at + len)
                                    .map(Some)
                                    .ok_or(Error::Truncated);
                            }
                        }
                        return Ok(None);
                    }
                    depth = depth.saturating_sub(1);
                }

                FDT_PROP => {
                    let len = be32(self.bytes, at)? as usize;
                    let name_off = be32(self.bytes, at + 4)? as usize;
                    let value_at = at + 8;
                    at = value_at + align4(len);

                    if depth < MAX_DEPTH && self.cstr(self.off_strings + name_off)? == name {
                        values[depth] = Some((value_at, len));
                    }
                }

                FDT_NOP => {}
                FDT_END => return Ok(None),
                other => return Err(Error::BadToken(other)),
            }
        }
    }

    /// The raw bytes of property `name` on the node whose `phandle` is `phandle`. `Ok(None)` when
    /// no node carries that phandle, or the node has no such property.
    ///
    /// How a property that refers to another node is followed, such as `interrupt-parent` to the
    /// controller's `#interrupt-cells`. Both `phandle` and the legacy `linux,phandle` are honored,
    /// because old dtc wrote the latter. As in [`node_prop_compatible`](Self::node_prop_compatible),
    /// the answer is taken when the matching node closes.
    pub fn phandle_prop(&self, phandle: u32, name: &[u8]) -> Result<Option<&'a [u8]>, Error> {
        const MAX_DEPTH: usize = 16;
        // Per open node: where `name`'s value sits, and whether its phandle matched.
        let mut found = [None::<(usize, usize)>; MAX_DEPTH];
        let mut matched = [false; MAX_DEPTH];

        let mut depth = 0usize;
        let mut at = self.off_struct;

        loop {
            let token = be32(self.bytes, at)?;
            at += 4;

            match token {
                FDT_BEGIN_NODE => {
                    let nm = self.cstr(at)?;
                    at += align4(nm.len() + 1);
                    depth += 1;
                    if depth < MAX_DEPTH {
                        found[depth] = None;
                        matched[depth] = false;
                    }
                }

                FDT_END_NODE => {
                    // Depth 1 is the root, which no phandle names in practice; a node past
                    // MAX_DEPTH was never tracked, so it cannot answer (node_reg's refusal).
                    if (2..MAX_DEPTH).contains(&depth) && matched[depth] {
                        return match found[depth] {
                            Some((value_at, len)) => self
                                .bytes
                                .get(value_at..value_at + len)
                                .map(Some)
                                .ok_or(Error::Truncated),
                            None => Ok(None),
                        };
                    }
                    depth = depth.saturating_sub(1);
                }

                FDT_PROP => {
                    let len = be32(self.bytes, at)? as usize;
                    let name_off = be32(self.bytes, at + 4)? as usize;
                    let value_at = at + 8;
                    at = value_at + align4(len);

                    let pname = self.cstr(self.off_strings + name_off)?;
                    if depth < MAX_DEPTH {
                        if pname == name {
                            found[depth] = Some((value_at, len));
                        }
                        if (pname == b"phandle" || pname == b"linux,phandle")
                            && len == 4
                            && be32(self.bytes, value_at)? == phandle
                        {
                            matched[depth] = true;
                        }
                    }
                }

                FDT_NOP => {}
                FDT_END => return Ok(None),
                other => return Err(Error::BadToken(other)),
            }
        }
    }

    /// Property `name` on **every** node whose name starts with `prefix`, in tree order.
    ///
    /// For a node that repeats, such as each hart's `cpu@` on a heterogeneous RISC-V machine like
    /// the JH7110 (milestone 60 (ISA discovery)). `out[i]` is `None` when the *i*th match has no
    /// such property, which differs from there being no *i*th match. Returns the number of matching
    /// **nodes**, which may exceed `out.len()`; the excess is counted and not written, so a caller
    /// can tell it under-provisioned. See notes/device-tree.md.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn f(dt: &device_tree_blob::DeviceTreeBlob<'_>) -> Result<(), device_tree_blob::Error> {
    /// let mut isa = [None; 8];
    /// let harts = dt.node_props(b"cpu@", b"riscv,isa", &mut isa)?;
    /// assert!(harts <= isa.len(), "more harts than slots: widen the array");
    /// # Ok(())
    /// # }
    /// ```
    pub fn node_props(
        &self,
        prefix: &[u8],
        name: &[u8],
        out: &mut [Option<&'a [u8]>],
    ) -> Result<usize, Error> {
        let mut depth = 0usize;
        // The depth of the node currently being read, `None` when the walk is outside every match.
        // Nested matches are impossible by construction: a node only becomes the target while there
        // is no target, so a `cpu@0/cpu@1` (which no binding produces) reads as one node.
        let mut target_at: Option<usize> = None;
        let mut matched = 0usize;
        let mut at = self.off_struct;

        for slot in out.iter_mut() {
            *slot = None;
        }

        loop {
            let token = be32(self.bytes, at)?;
            at += 4;

            match token {
                FDT_BEGIN_NODE => {
                    let nm = self.cstr(at)?;
                    at += align4(nm.len() + 1);
                    depth += 1;
                    if depth >= 2 && nm.starts_with(prefix) && target_at.is_none() {
                        target_at = Some(depth);
                        matched += 1;
                    }
                }

                FDT_END_NODE => {
                    if target_at == Some(depth) {
                        target_at = None;
                    }
                    depth = depth.saturating_sub(1);
                }

                FDT_PROP => {
                    let len = be32(self.bytes, at)? as usize;
                    let name_off = be32(self.bytes, at + 4)? as usize;
                    let value_at = at + 8;
                    at = value_at + align4(len);

                    if target_at == Some(depth)
                        && self.cstr(self.off_strings + name_off)? == name
                        && let Some(slot) = out.get_mut(matched - 1)
                    {
                        *slot = Some(
                            self.bytes
                                .get(value_at..value_at + len)
                                .ok_or(Error::Truncated)?,
                        );
                    }
                }

                FDT_NOP => {}
                FDT_END => return Ok(matched),
                other => return Err(Error::BadToken(other)),
            }
        }
    }

    /// The regions carved out by the `/reserved-memory` node's children.
    ///
    /// The other place firmware names memory the OS must not touch, beside the block
    /// [`reserved_regions`](Self::reserved_regions) reads; on RISC-V, OpenSBI reserves its PMP-guarded
    /// region here, and missing it faults the first write into it. Each depth-3 child's `reg` is
    /// decoded with `/reserved-memory`'s own cell counts, which default to the root's.
    pub fn reserved_memory_regions(&self, out: &mut [Region]) -> Result<usize, Error> {
        let mut address_cells = 2u32;
        let mut size_cells = 2u32;

        let mut depth = 0i32;
        let mut resv_depth: Option<i32> = None;
        let mut n = 0;
        let mut at = self.off_struct;

        loop {
            let token = be32(self.bytes, at)?;
            at += 4;

            match token {
                FDT_BEGIN_NODE => {
                    let name = self.cstr(at)?;
                    at += align4(name.len() + 1);
                    depth += 1;

                    if resv_depth.is_none() && name == b"reserved-memory" {
                        resv_depth = Some(depth);
                    }
                }

                FDT_END_NODE => {
                    if resv_depth == Some(depth) {
                        // Walked the whole /reserved-memory node; nothing more to find.
                        return Ok(n);
                    }
                    depth -= 1;
                }

                FDT_PROP => {
                    let len = be32(self.bytes, at)? as usize;
                    let name_off = be32(self.bytes, at + 4)? as usize;
                    let value_at = at + 8;
                    at = value_at + align4(len);

                    let name = self.cstr(self.off_strings + name_off)?;

                    // The root's cell counts are the defaults for the reserved-memory children.
                    if depth == 1 {
                        match name {
                            b"#address-cells" => address_cells = be32(self.bytes, value_at)?,
                            b"#size-cells" => size_cells = be32(self.bytes, value_at)?,
                            _ => {}
                        }
                    }

                    if let Some(rd) = resv_depth {
                        if depth == rd {
                            // The reserved-memory node may declare its own cell counts for children.
                            match name {
                                b"#address-cells" => address_cells = be32(self.bytes, value_at)?,
                                b"#size-cells" => size_cells = be32(self.bytes, value_at)?,
                                _ => {}
                            }
                        } else if depth == rd + 1 && name == b"reg" {
                            let dest = out.get_mut(n..).ok_or(Error::TooManyRegions)?;
                            n += self.decode_reg(value_at, len, address_cells, size_cells, dest)?;
                        }
                    }
                }

                FDT_NOP => {}
                FDT_END => return Ok(n),
                other => return Err(Error::BadToken(other)),
            }
        }
    }

    /// The initial ramdisk, if the bootloader placed one.
    ///
    /// Declared in `/chosen` as `linux,initrd-start` and `linux,initrd-end`.
    ///
    /// **This memory is ours to protect.** The bootloader loaded a file into RAM for us
    /// and told us where it put it. If we don't reserve it, the frame allocator hands it
    /// out to the first caller and the initial ramdisk is destroyed before we ever read a byte of
    /// it. Milestone 10 (a shell at EL0) and milestone 32 (a real filesystem) both
    /// want this, and by then the bug would be far away from its cause.
    ///
    /// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round
    /// 11). His words: "B for #1998." Refused `initrd` (the foreign spelling, which stops at the
    /// boundary where it arrives).
    pub fn initial_ramdisk(&self) -> Result<Option<Region>, Error> {
        let mut start: Option<u64> = None;
        let mut end: Option<u64> = None;

        let mut depth = 0i32;
        let mut chosen_at: Option<i32> = None;
        let mut at = self.off_struct;

        loop {
            let token = be32(self.bytes, at)?;
            at += 4;

            match token {
                FDT_BEGIN_NODE => {
                    let name = self.cstr(at)?;
                    at += align4(name.len() + 1);
                    depth += 1;
                    if depth == 2 && name == b"chosen" {
                        chosen_at = Some(depth);
                    }
                }

                FDT_END_NODE => {
                    if chosen_at == Some(depth) {
                        chosen_at = None;
                    }
                    depth -= 1;
                }

                FDT_PROP => {
                    let len = be32(self.bytes, at)? as usize;
                    let name_off = be32(self.bytes, at + 4)? as usize;
                    let value_at = at + 8;
                    at = value_at + align4(len);

                    if chosen_at == Some(depth) {
                        match self.cstr(self.off_strings + name_off)? {
                            b"linux,initrd-start" => start = Some(self.int(value_at, len)?),
                            b"linux,initrd-end" => end = Some(self.int(value_at, len)?),
                            _ => {}
                        }
                    }
                }

                FDT_NOP => {}
                FDT_END => break,
                other => return Err(Error::BadToken(other)),
            }
        }

        Ok(match (start, end) {
            // `linux,initrd-end` is exclusive, so an empty initial ramdisk (start == end) is a
            // "no".
            (Some(s), Some(e)) if e > s => Some(Region {
                start: s,
                size: e - s,
            }),
            _ => None,
        })
    }

    /// An integer property.
    ///
    /// The device tree spec lets these be either 32 or 64 bits wide, and **the only way
    /// to tell is the property's length**. QEMU writes `linux,initrd-start` as 8 bytes;
    /// a 32-bit platform writes 4. Assume one and you silently misread the other.
    fn int(&self, at: usize, len: usize) -> Result<u64, Error> {
        match len {
            4 => Ok(be32(self.bytes, at)? as u64),
            8 => Ok(be64(self.bytes, at)?),
            _ => Err(Error::Truncated),
        }
    }

    /// Decode a `reg` property: a packed list of (address, size) pairs, where each
    /// value is `cells` 32-bit big-endian words concatenated.
    fn decode_reg(
        &self,
        at: usize,
        len: usize,
        address_cells: u32,
        size_cells: u32,
        out: &mut [Region],
    ) -> Result<usize, Error> {
        let pair_bytes = (address_cells as usize + size_cells as usize) * 4;
        if pair_bytes == 0 {
            return Ok(0);
        }

        let mut n = 0;
        let mut cursor = at;

        while cursor + pair_bytes <= at + len {
            let start = self.cells(cursor, address_cells)?;
            let size = self.cells(cursor + address_cells as usize * 4, size_cells)?;
            cursor += pair_bytes;

            // A zero-size region is legal and useless. Skip it rather than handing the
            // allocator an empty range to reason about.
            if size == 0 {
                continue;
            }

            // A region that runs past the top of the address space is not a region. Refused rather
            // than skipped, because a zero-size entry is a *legal* thing firmware writes and this is
            // not: see `Error::RegionOverflow`.
            if start.checked_add(size).is_none() {
                return Err(Error::RegionOverflow);
            }

            *out.get_mut(n).ok_or(Error::TooManyRegions)? = Region { start, size };
            n += 1;
        }

        Ok(n)
    }

    /// Read `count` 32-bit big-endian cells and concatenate them into one u64.
    fn cells(&self, at: usize, count: u32) -> Result<u64, Error> {
        let mut value = 0u64;
        for i in 0..count as usize {
            value = (value << 32) | be32(self.bytes, at + i * 4)? as u64;
        }
        Ok(value)
    }

    /// A null-terminated string in the blob, returned without its terminator.
    fn cstr(&self, at: usize) -> Result<&'a [u8], Error> {
        let rest = self.bytes.get(at..).ok_or(Error::Truncated)?;
        let end = rest.iter().position(|&b| b == 0).ok_or(Error::Truncated)?;
        Ok(&rest[..end])
    }
}

/// Everything in the structure block is padded to a 4-byte boundary.
fn align4(n: usize) -> usize {
    n.div_ceil(4) * 4
}

/// Machine-checked proofs of the device-tree parser's leaf readers (DECISIONS §14, milestone 18).
///
/// The device tree is untrusted boot input, and the whole-parse walk (a token loop over the
/// structure block) is past what bounded model checking can do, the same wall the ELF parser hit
/// (see notes/verification.md). So, as there, the leaves are proved: the big-endian readers every
/// field goes through, and the padding helper. `be32`/`be64` were hardened with a checked add first,
/// so they are now *total*; `align4` is proved correct for any realistic length (it can overflow
/// only far above any device-tree size, and its callers pass node-name and property lengths).
#[cfg(kani)]
mod verification {
    use super::*;

    /// Enough bytes to place a `be64` read at a few different offsets.
    const N: usize = 12;

    /// **`be32` is total: no offset panics.** For any bytes and any offset, `be32` returns `Ok` or
    /// `Err(Truncated)`, never panicking, even at `usize::MAX` where the old `at + 4` would have
    /// wrapped. This is the hardening proved.
    /// Falsification: replayable `crates/device_tree_blob/falsifications/verification.be32_is_total.patch`
    #[kani::proof]
    fn be32_is_total() {
        let bytes: [u8; N] = kani::any();
        let at: usize = kani::any();
        let _ = be32(&bytes, at);
    }

    /// **`be64` is total: no offset panics.** As `be32`, for the 8-byte read.
    /// Falsification: replayable `crates/device_tree_blob/falsifications/verification.be64_is_total.patch`
    #[kani::proof]
    fn be64_is_total() {
        let bytes: [u8; N] = kani::any();
        let at: usize = kani::any();
        let _ = be64(&bytes, at);
    }

    /// **An in-bounds `be32` reads four big-endian bytes.** When it returns `Ok`, the value is
    /// exactly `bytes[at..at+4]` most-significant-byte first, so the endianness conversion the whole
    /// crate depends on is correct, for every input.
    /// Falsification: replayable `crates/device_tree_blob/falsifications/verification.be32_reads_big_endian_when_in_bounds.patch`
    #[kani::proof]
    fn be32_reads_big_endian_when_in_bounds() {
        let bytes: [u8; N] = kani::any();
        let at: usize = kani::any();
        if let Ok(v) = be32(&bytes, at) {
            let expected = ((bytes[at] as u32) << 24)
                | ((bytes[at + 1] as u32) << 16)
                | ((bytes[at + 2] as u32) << 8)
                | (bytes[at + 3] as u32);
            assert_eq!(v, expected);
        }
    }

    /// **`align4` rounds up to a multiple of four.** For any realistic length, `align4(n)` is a
    /// multiple of 4, at least `n`, and less than `n + 4`. The bound is far above any device-tree
    /// field length and only rules out the overflow point of the internal `* 4`.
    /// Falsification: replayable `crates/device_tree_blob/falsifications/verification.align4_rounds_up_to_a_multiple_of_four.patch`
    #[kani::proof]
    fn align4_rounds_up_to_a_multiple_of_four() {
        let n: usize = kani::any();
        kani::assume(n <= 1 << 60);
        let a = align4(n);
        assert!(a.is_multiple_of(4));
        assert!(a >= n);
        assert!(a < n + 4);
    }
}
