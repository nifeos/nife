//! A minimal flattened device tree (FDT) parser: bytes in, regions and property values out.
//!
//! The blob is the machine describing itself (where RAM, the serial port and the interrupt
//! controller are, how many CPUs exist), handed over by firmware: a bootloader, a UEFI loader or
//! QEMU. Every integer in it is big-endian, so every read goes through `be32` or `be64`. The blob
//! is untrusted input: every offset is checked, every walk is bounded, and a malformed blob is an
//! [`Error`] rather than a panic. The crate is pure logic with no target code, so its tests run on
//! the host against trees dumped from QEMU's `virt` machines and against hand-built hostile blobs.
//!
//! The format is the flattened one that chapter 5 of the Devicetree Specification (release 0.4)
//! defines, and the one libfdt reads.
//!
//! Name: ratified 2026-09-18 (§154 (the acronym test is whether the phrase is spoken)) and
//! performed 2026-09-19. Refused `dtb`: "device tree blob" is a phrase people say, so the
//! abbreviation goes. Known cost: datasheets, error messages and boot logs say `dtb`, so a reader
//! grepping the word the machine printed will not find this identifier.

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
    /// Saturating because the type is `pub` with `pub` fields, so anyone can build one whose
    /// `start + size` wraps, and a caller placing an allocator over every RAM region may call this
    /// before it can report a failure; a plain add would panic there under overflow checks
    /// (regression in `tests/hostile.rs`). A saturated end is still wrong, so the parser refuses a
    /// wrapping region with [`Error::RegionOverflow`]; this is the backstop, not the check.
    pub fn end(&self) -> u64 {
        self.start.saturating_add(self.size)
    }
}

/// Why parsing the blob failed.
///
/// Each fault has its own variant, as libfdt separates `FDT_ERR_TRUNCATED`, `FDT_ERR_BADVALUE` and
/// `FDT_ERR_BADSTRUCTURE`, so a caller printing the error at boot names the fault the firmware made.
/// The enum is `#[non_exhaustive]`: a later fault gets its own variant without breaking a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The first four bytes were not `0xd00dfeed`. Either the pointer is wrong or the
    /// bootloader did not hand over a device tree at all.
    BadMagic(u32),
    /// The blob claims a version this parser does not read: its `last_comp_version` is above 17.
    UnsupportedVersion(u32),
    /// The blob, or one of its blocks, ends before something it must hold: an offset in the header
    /// points outside the blob, a property's value runs past the structure block, or the walk
    /// reaches the end of the structure block without its end token.
    Truncated,
    /// A value has the wrong shape for what it holds: an integer property that is neither 4 nor 8
    /// bytes long, or a node or property name with no terminating NUL inside its block.
    BadValue,
    /// A token this parser does not recognize. The structure block is malformed.
    BadToken(u32),
    /// The caller's output slice was too small to hold every region found.
    TooManyRegions,
    /// The structure block's nesting does not balance: a node closes when none is open, or the
    /// block's end token arrives while a node is still open.
    BadStructure,
    /// The structure block nests deeper than [`MAXIMUM_DEPTH`].
    TooDeep,
    /// The header's `totalsize` is larger than [`MAXIMUM_TOTALSIZE_BYTES`], so
    /// [`from_ptr`](DeviceTreeBlob::from_ptr) refused to build a slice of that length over
    /// memory nobody has checked. Carries the size claimed.
    TooLarge(usize),
    /// A `reg` pair whose `start + size` does not fit in 64 bits, so the region names memory that
    /// cannot exist.
    ///
    /// Refused rather than clamped, and here rather than left to [`Region::end`]: a caller deciding
    /// where RAM is should be told the firmware's memory map is impossible rather than quietly
    /// work around it.
    RegionOverflow,
}

impl core::fmt::Display for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::BadMagic(magic) => write!(formatter, "bad device tree magic {magic:#010x}"),
            Error::UnsupportedVersion(version) => {
                write!(formatter, "unsupported device tree version {version}")
            }
            Error::Truncated => formatter.write_str("device tree truncated"),
            Error::BadValue => formatter.write_str("malformed device tree property value or name"),
            Error::BadToken(token) => write!(formatter, "unknown device tree token {token:#x}"),
            Error::BadStructure => formatter.write_str("device tree nodes do not balance"),
            Error::TooDeep => formatter.write_str("device tree nests deeper than the maximum"),
            Error::TooManyRegions => formatter.write_str("too many regions for the output slice"),
            Error::TooLarge(size) => {
                write!(
                    formatter,
                    "device tree claims {size} bytes, above the ceiling"
                )
            }
            Error::RegionOverflow => formatter.write_str("region wraps the 64-bit address space"),
        }
    }
}

impl core::error::Error for Error {}

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
/// 4 GiB slice read past the end of RAM, a fault on the boot path before any handler can say why.
/// 2 MiB is Linux's `MAX_FDT_SIZE` (`SZ_2M` in `arch/arm64/include/asm/boot.h`); QEMU's `virt`
/// blobs are under 64 KiB.
///
/// Name: provisional. Ratified as `MAXIMUM_TOTALSIZE` on 2026-10-27 (#2797); `_BYTES` added at the
/// crate's own name review, since a size carries its unit.
#[doc(alias = "MAX_FDT_SIZE")]
pub const MAXIMUM_TOTALSIZE_BYTES: usize = 2 * 1024 * 1024;

/// How deeply the structure block may nest, the root counting as depth 1: 64, as Linux's
/// `FDT_MAX_DEPTH`.
///
/// A lookup keeps a few words per open node on the caller's stack, so the nesting has a bound. A
/// tree nested deeper is refused with [`Error::TooDeep`] by every lookup alike, rather than read by
/// some and not others. Real trees nest well under ten deep.
///
/// Name: provisional.
#[doc(alias = "FDT_MAX_DEPTH")]
pub const MAXIMUM_DEPTH: usize = 64;

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
///
/// Each lookup walks the structure block from its first token, so a lookup costs one pass over the
/// tree and holds nothing between calls. Every walk refuses the same faults when it reaches them:
/// a tree whose nesting does not balance ([`Error::BadStructure`]), one nested deeper than
/// [`MAXIMUM_DEPTH`] ([`Error::TooDeep`]), and a structure block that ends before its end token
/// ([`Error::Truncated`]). A lookup that finds its answer first returns it without walking on.
#[derive(Debug)]
pub struct DeviceTreeBlob<'a> {
    bytes: &'a [u8],
    /// The structure block alone, as the prefix of the blob that ends where the block does, so an
    /// offset into it is an offset into the blob and a read past the block is a read past the end.
    structure: &'a [u8],
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
        if total > MAXIMUM_TOTALSIZE_BYTES {
            return Err(Error::TooLarge(total));
        }

        // SAFETY: the magic checked out, so `ptr` really is a blob, and `total` is the length it
        // declares for itself, now bounded by `MAXIMUM_TOTALSIZE_BYTES`; the caller's contract covers the
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

        let structure_start = be32(bytes, 8)? as usize;
        let off_strings = be32(bytes, 12)? as usize;

        // Where the structure block ends. From version 17 the header says how long it is; before
        // that the block runs up to the strings block, which is how libfdt reads such a blob. A
        // walk that reaches this end without `FDT_END` is truncated; without the bound it would
        // read the strings block as tokens.
        let structure_end = if be32(bytes, 20)? >= 17 {
            structure_start
                .checked_add(be32(bytes, 36)? as usize)
                .ok_or(Error::Truncated)?
        } else {
            off_strings
        };
        if structure_end > total || structure_end < structure_start {
            return Err(Error::Truncated);
        }

        let tree = DeviceTreeBlob {
            off_struct: structure_start,
            off_strings,
            off_rsvmap: be32(bytes, 16)? as usize,
            bytes: &bytes[..total],
            structure: &bytes[..structure_end],
        };

        if tree.off_struct >= total || tree.off_strings >= total || tree.off_rsvmap >= total {
            return Err(Error::Truncated);
        }

        Ok(tree)
    }

    /// How many bytes the blob occupies. We need this to mark it as reserved: it is
    /// sitting in the very RAM we are about to start handing out.
    pub fn total_size(&self) -> usize {
        self.bytes.len()
    }

    /// The memory reservation block: regions the bootloader is telling us **not to
    /// touch**, before a single node is parsed.
    ///
    /// This is a separate, deliberately dead-simple structure precisely so that a
    /// kernel can honor it without having to parse anything. QEMU's `virt` leaves it
    /// empty, but a real board's firmware often does not, and a kernel that skips it
    /// will happily allocate over the firmware's own tables.
    ///
    /// Name: provisional. Refused `reserved_regions`, with no alias for it (milestone 1085 (what an
    /// outside maintainer would fix before it leaves) D5).
    pub fn memory_reservations(&self, out: &mut [Region]) -> Result<usize, Error> {
        let mut at = self.off_rsvmap;
        let mut n = 0;

        loop {
            let start = be64(self.bytes, at)?;
            let size = be64(self.bytes, at.checked_add(8).ok_or(Error::Truncated)?)?;
            at = at.checked_add(16).ok_or(Error::Truncated)?;

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
        // `reg` is a list of (address, size) pairs, but how many 32-bit cells each of those takes
        // is not fixed. It is declared by #address-cells and #size-cells on the PARENT node, which
        // for a /memory node is the root. The specification's defaults are 2 and 1. Nearly every
        // 64-bit machine says 2 and 2, but they are read rather than assumed.
        let mut address_cells = 2u32;
        let mut size_cells = 1u32;

        let mut found = 0;
        // Whether the depth-2 node now open is a memory node. A flag keyed on depth 2 rather than
        // a bare "inside one" flag: a child of a /memory node, which no real tree has, closes at
        // depth 3 and leaves its parent's remaining properties still read.
        let mut in_memory = false;
        let mut tokens = self.tokens();

        while let Some((depth, token)) = tokens.step()? {
            match token {
                Token::BeginNode { name } => {
                    // A memory node is a child of the root named `memory` or `memory@<address>`.
                    if depth == 2 {
                        in_memory = name == b"memory" || name.starts_with(b"memory@");
                    }
                }
                Token::EndNode => {
                    if depth == 2 {
                        in_memory = false;
                    }
                }
                Token::Property { name, value } => {
                    // The root's cell counts. A node's properties precede its children, so by the
                    // time a /memory node is reached these are the root's.
                    if depth == 1 {
                        declare_cells(name, value, &mut address_cells, &mut size_cells);
                    }
                    if depth == 2 && in_memory && name == b"reg" {
                        let remaining = out.get_mut(found..).ok_or(Error::TooManyRegions)?;
                        found += decode_reg(value, address_cells, size_cells, remaining)?;
                    }
                }
            }
        }
        Ok(found)
    }

    /// The register regions (`reg`, the specification's name for the property) of the first node
    /// whose name starts with `prefix`, searched at any depth.
    ///
    /// Decodes with the `#address-cells`/`#size-cells` of the node's **parent**, carried down a
    /// per-depth stack, so a device nested under `/soc` reads as well as one under the root. A GIC
    /// decodes to two regions, distributor first, as its binding orders them. Matching a name prefix
    /// rather than `compatible` is a simplification; [`node_reg_compatible`](Self::node_reg_compatible)
    /// is the correct lookup. The first match is the answer: one with no `reg` yields zero regions, and
    /// a later match's `reg` is never read in its place.
    #[doc(alias = "register regions")]
    pub fn node_reg(&self, prefix: &[u8], out: &mut [Region]) -> Result<usize, Error> {
        // The cell counts declared by the node open at each depth, applying to its children. Depth
        // 0 is the slot above the root; the root is depth 1. The specification's defaults are 2
        // and 1.
        let mut address_cells = [2u32; DEPTH_SLOTS];
        let mut size_cells = [1u32; DEPTH_SLOTS];
        let mut target_at: Option<usize> = None;
        let mut tokens = self.tokens();

        while let Some((depth, token)) = tokens.step()? {
            match token {
                Token::BeginNode { name } => {
                    // A node inherits its parent's cell counts until it declares its own (its
                    // properties come before its children).
                    address_cells[depth] = address_cells[depth - 1];
                    size_cells[depth] = size_cells[depth - 1];
                    if depth >= 2 && target_at.is_none() && name.starts_with(prefix) {
                        target_at = Some(depth);
                    }
                }
                Token::EndNode => {
                    if target_at == Some(depth) {
                        // The whole node has been walked. Had it a `reg`, the property arm would
                        // have answered, so it has none. The first match is the answer, as for
                        // `node_property` and `node_reg_compatible`: zero regions, and a later
                        // node's `reg` is not a substitute.
                        return Ok(0);
                    }
                }
                Token::Property { name, value } => {
                    declare_cells(
                        name,
                        value,
                        &mut address_cells[depth],
                        &mut size_cells[depth],
                    );
                    if target_at == Some(depth) && name == b"reg" {
                        // A node's own `reg` uses its parent's cells, one level up.
                        return decode_reg(
                            value,
                            address_cells[depth - 1],
                            size_cells[depth - 1],
                            out,
                        );
                    }
                }
            }
        }
        Ok(0)
    }

    /// The register regions (`reg`, the specification's name for the property) of the first node
    /// whose `compatible` property lists the entry `compatible`, searched at any depth.
    ///
    /// A match on **any** NUL-separated entry counts, as the binding intends: a node that says
    /// `"starfive,jh7110-rtc\0arm,pl031"` claims the PL031 register layout. The empty piece after the
    /// final NUL is not an entry, so an empty `compatible` matches nothing. The property may follow
    /// `reg` in the same node, so each open node's `reg` is remembered on a per-depth stack and
    /// decoded when the matched node closes; a matched parent is still answered after its
    /// children close. Returns zero regions for a match with no `reg`.
    #[doc(alias = "register regions")]
    pub fn node_reg_compatible(
        &self,
        compatible: &[u8],
        out: &mut [Region],
    ) -> Result<usize, Error> {
        let mut address_cells = [2u32; DEPTH_SLOTS];
        let mut size_cells = [1u32; DEPTH_SLOTS];
        // Per open node: its `reg` value with the parent's cell counts it decodes with, and whether
        // its `compatible` named the device being looked for.
        let mut reg = [None::<(&'a [u8], u32, u32)>; DEPTH_SLOTS];
        let mut matched = [false; DEPTH_SLOTS];
        let mut tokens = self.tokens();

        while let Some((depth, token)) = tokens.step()? {
            match token {
                Token::BeginNode { .. } => {
                    address_cells[depth] = address_cells[depth - 1];
                    size_cells[depth] = size_cells[depth - 1];
                    reg[depth] = None;
                    matched[depth] = false;
                }
                Token::EndNode => {
                    // Depth 1 is the root, which has no `compatible` worth matching and whose `reg`
                    // would have no parent to decode against.
                    if depth >= 2 && matched[depth] {
                        return match reg[depth] {
                            Some((value, address, size)) => decode_reg(value, address, size, out),
                            // Compatible but no `reg`: the device exists and has no register block
                            // to name. Zero regions, not an error.
                            None => Ok(0),
                        };
                    }
                }
                Token::Property { name, value } => {
                    declare_cells(
                        name,
                        value,
                        &mut address_cells[depth],
                        &mut size_cells[depth],
                    );
                    if depth >= 2 && name == b"reg" {
                        reg[depth] = Some((value, address_cells[depth - 1], size_cells[depth - 1]));
                    }
                    if name == b"compatible" {
                        matched[depth] = lists_entry(value, compatible);
                    }
                }
            }
        }
        Ok(0)
    }

    /// The raw bytes of property `name` on the first node whose `compatible` property lists the
    /// entry `compatible`, searched at any depth. `Ok(None)` when no such node exists, or the node has no
    /// such property.
    ///
    /// The property twin of [`node_reg_compatible`](Self::node_reg_compatible), deciding when the
    /// matched node closes for the same reason. It exists because labels differ between machines:
    /// the PLIC is `plic@c000000` on QEMU `virt` and `interrupt-controller@c000000` on the JH7110.
    ///
    /// Name: provisional. Refused `node_prop_compatible`, with no alias for it (milestone 1085
    /// (what an outside maintainer would fix before it leaves) D1).
    pub fn node_property_compatible(
        &self,
        compatible: &[u8],
        name: &[u8],
    ) -> Result<Option<&'a [u8]>, Error> {
        self.property_compatible_up(compatible, name, 0)
    }

    /// The raw bytes of property `name` on the **parent** of the first node whose `compatible`
    /// property lists the entry `compatible`. `Ok(None)` when no such node exists, or its parent
    /// has no such property.
    ///
    /// For a device described as a child of its bus, such as a PMIC under one of seven identical
    /// `snps,designware-i2c` controllers on the StarFive JH7110, whose `clocks` and `resets` sit on
    /// the bus. The parent's slot is filled when the child closes, because a node's properties
    /// precede its first subnode.
    ///
    /// Name: provisional, 2026-09-25. Refused `parent_prop_compatible`, the abbreviation, with no
    /// alias for it; `property` is spelled out across the crate's lookups.
    pub fn parent_property_compatible(
        &self,
        compatible: &[u8],
        name: &[u8],
    ) -> Result<Option<&'a [u8]>, Error> {
        self.property_compatible_up(compatible, name, 1)
    }

    /// The shared walk behind the two `*_property_compatible` reads: property `name` on the node `up`
    /// levels above the first node whose `compatible` property lists the argument (0 for the node itself, 1 for its parent).
    fn property_compatible_up(
        &self,
        compatible: &[u8],
        name: &[u8],
        up: usize,
    ) -> Result<Option<&'a [u8]>, Error> {
        // Per open node: `name`'s value, and whether `compatible` matched.
        let mut found = [None::<&'a [u8]>; DEPTH_SLOTS];
        let mut matched = [false; DEPTH_SLOTS];
        let mut tokens = self.tokens();

        while let Some((depth, token)) = tokens.step()? {
            match token {
                Token::BeginNode { .. } => {
                    found[depth] = None;
                    matched[depth] = false;
                }
                Token::EndNode => {
                    // Depth 1 is the root, which carries no `compatible` worth matching here.
                    // `depth >= 2` and `up <= 1` keep the answer at the root or below it.
                    if depth >= 2 && matched[depth] {
                        return Ok(found[depth - up]);
                    }
                }
                Token::Property {
                    name: property_name,
                    value,
                } => {
                    // Two independent `if`s, so asking for `compatible` itself still answers.
                    if property_name == name {
                        found[depth] = Some(value);
                    }
                    if property_name == b"compatible" {
                        matched[depth] = lists_entry(value, compatible);
                    }
                }
            }
        }
        Ok(None)
    }

    /// The raw bytes of property `name` on the first node whose name starts with `prefix`,
    /// searched at any depth. `Ok(None)` when no such node, or the node has no such property.
    ///
    /// The generic escape hatch next to the decoded accessors above: some properties have
    /// per-binding layouts no general decoder can know (`interrupt-map` is the motivating case:
    /// its entry width depends on three different nodes' cell counts). The caller gets the bytes
    /// (big-endian cells, as the specification lays them out) and owns the interpretation.
    ///
    /// Name: provisional. Refused `node_prop`, with no alias for it (milestone 1085 (what an
    /// outside maintainer would fix before it leaves) D1).
    pub fn node_property(&self, prefix: &[u8], name: &[u8]) -> Result<Option<&'a [u8]>, Error> {
        let mut target_at: Option<usize> = None;
        let mut tokens = self.tokens();

        while let Some((depth, token)) = tokens.step()? {
            match token {
                Token::BeginNode { name: node_name } => {
                    if depth >= 2 && target_at.is_none() && node_name.starts_with(prefix) {
                        target_at = Some(depth);
                    }
                }
                Token::EndNode => {
                    if target_at == Some(depth) {
                        // Walked the whole matched node: it has no such property.
                        return Ok(None);
                    }
                }
                Token::Property {
                    name: property_name,
                    value,
                } => {
                    if target_at == Some(depth) && property_name == name {
                        return Ok(Some(value));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Property `name` on the first node whose name starts with `prefix`, **falling back to the
    /// nearest ancestor** that carries it. `Ok(None)` when no such node, or neither the node nor
    /// any ancestor has the property.
    ///
    /// For the properties the spec defines as inheritable, such as `interrupt-parent`, which QEMU's
    /// two `virt` boards and the JH7110 dtsi each write at a different level. The nearest enclosing
    /// value wins, per the spec. A node's properties precede its children, so one walk suffices:
    /// the answer is taken at the target's `END_NODE`.
    ///
    /// Name: provisional. Refused `node_prop_inherited`, with no alias for it (milestone 1085 (what
    /// an outside maintainer would fix before it leaves) D1).
    pub fn node_property_inherited(
        &self,
        prefix: &[u8],
        name: &[u8],
    ) -> Result<Option<&'a [u8]>, Error> {
        // `name`'s value on the node open at each depth, `None` where it has none. A node's
        // beginning resets its own slot, so a closed sibling's value never reaches the ancestor
        // scan (which reads only the depths still open above the target anyway).
        let mut values = [None::<&'a [u8]>; DEPTH_SLOTS];
        let mut target_at: Option<usize> = None;
        let mut tokens = self.tokens();

        while let Some((depth, token)) = tokens.step()? {
            match token {
                Token::BeginNode { name: node_name } => {
                    values[depth] = None;
                    if depth >= 2 && target_at.is_none() && node_name.starts_with(prefix) {
                        target_at = Some(depth);
                    }
                }
                Token::EndNode => {
                    if target_at == Some(depth) {
                        // The whole target node has been walked: its own value if it had one,
                        // else the nearest open ancestor's, root included.
                        return Ok(values[1..=depth].iter().rev().find_map(|value| *value));
                    }
                }
                Token::Property {
                    name: property_name,
                    value,
                } => {
                    if property_name == name {
                        values[depth] = Some(value);
                    }
                }
            }
        }
        Ok(None)
    }

    /// The raw bytes of property `name` on the node whose phandle (`phandle`, the specification's
    /// name for a node's numeric handle, which other nodes use to refer to it) is `phandle`.
    /// `Ok(None)` when no node carries that phandle, or the node has no such property.
    ///
    /// How a property that refers to another node is followed, such as `interrupt-parent` to the
    /// controller's `#interrupt-cells`. Both `phandle` and the legacy `linux,phandle` are honored,
    /// because old dtc wrote the latter. As in [`node_property_compatible`](Self::node_property_compatible),
    /// the answer is taken when the matching node closes.
    ///
    /// Name: provisional. Refused `phandle_prop`, with no alias for it (milestone 1085 (what an
    /// outside maintainer would fix before it leaves) D1).
    pub fn phandle_property(&self, phandle: u32, name: &[u8]) -> Result<Option<&'a [u8]>, Error> {
        // Per open node: `name`'s value, and whether its phandle matched.
        let mut found = [None::<&'a [u8]>; DEPTH_SLOTS];
        let mut matched = [false; DEPTH_SLOTS];
        let mut tokens = self.tokens();

        while let Some((depth, token)) = tokens.step()? {
            match token {
                Token::BeginNode { .. } => {
                    found[depth] = None;
                    matched[depth] = false;
                }
                Token::EndNode => {
                    // Depth 1 is the root, which no phandle names in practice.
                    if depth >= 2 && matched[depth] {
                        return Ok(found[depth]);
                    }
                }
                Token::Property {
                    name: property_name,
                    value,
                } => {
                    if property_name == name {
                        found[depth] = Some(value);
                    }
                    if (property_name == b"phandle" || property_name == b"linux,phandle")
                        && value.len() == 4
                        && be32(value, 0)? == phandle
                    {
                        matched[depth] = true;
                    }
                }
            }
        }
        Ok(None)
    }

    /// Property `name` on **every** node whose name starts with `prefix`, in tree order.
    ///
    /// For a node that repeats, such as each hart's `cpu@` on a heterogeneous RISC-V machine like
    /// the JH7110, whose harts differ in their `riscv,isa`. `out[i]` is `None` when the *i*th match has no
    /// such property, which differs from there being no *i*th match. Returns the number of matching
    /// **nodes**, which may exceed `out.len()`; the excess is counted and not written, so a caller
    /// can tell it under-provisioned.
    ///
    /// Name: provisional. Refused `node_props`, with no alias for it (milestone 1085 (what an
    /// outside maintainer would fix before it leaves) D1).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn f(dt: &device_tree_blob::DeviceTreeBlob<'_>) -> Result<(), device_tree_blob::Error> {
    /// let mut isa = [None; 8];
    /// let harts = dt.node_properties(b"cpu@", b"riscv,isa", &mut isa)?;
    /// assert!(harts <= isa.len(), "more harts than slots: widen the array");
    /// # Ok(())
    /// # }
    /// ```
    pub fn node_properties(
        &self,
        prefix: &[u8],
        name: &[u8],
        out: &mut [Option<&'a [u8]>],
    ) -> Result<usize, Error> {
        // The depth of the node being read, `None` outside every match. Nested matches are
        // impossible by construction: a node becomes the target only while there is none, so a
        // `cpu@0/cpu@1` (which no binding produces) reads as one node.
        let mut target_at: Option<usize> = None;
        let mut matched = 0usize;
        let mut tokens = self.tokens();

        for slot in out.iter_mut() {
            *slot = None;
        }

        while let Some((depth, token)) = tokens.step()? {
            match token {
                Token::BeginNode { name: node_name } => {
                    if depth >= 2 && target_at.is_none() && node_name.starts_with(prefix) {
                        target_at = Some(depth);
                        matched += 1;
                    }
                }
                Token::EndNode => {
                    if target_at == Some(depth) {
                        target_at = None;
                    }
                }
                Token::Property {
                    name: property_name,
                    value,
                } => {
                    if target_at == Some(depth)
                        && property_name == name
                        && let Some(slot) = out.get_mut(matched - 1)
                    {
                        *slot = Some(value);
                    }
                }
            }
        }
        Ok(matched)
    }

    /// The regions carved out by the root's `reserved-memory` node's children. A node of that name
    /// anywhere deeper is not the reservation list and is not read.
    ///
    /// The other place firmware names memory the OS must not touch, beside the block
    /// [`memory_reservations`](Self::memory_reservations) reads; on RISC-V, OpenSBI reserves its PMP-guarded
    /// region here, and missing it faults the first write into it. Each depth-3 child's `reg` is
    /// decoded with `/reserved-memory`'s own cell counts, which default to the root's.
    pub fn reserved_memory_regions(&self, out: &mut [Region]) -> Result<usize, Error> {
        let mut address_cells = 2u32;
        let mut size_cells = 2u32;
        // Whether the depth-2 node now open is `/reserved-memory`. A child of the root only, as
        // `memory_regions` reads `/memory`: one nested under another node is not the reservation
        // list, and its children would be decoded with the root's cell counts rather than their
        // own parent's.
        let mut in_reserved_memory = false;
        let mut found = 0;
        let mut tokens = self.tokens();

        while let Some((depth, token)) = tokens.step()? {
            match token {
                Token::BeginNode { name } => {
                    if depth == 2 && name == b"reserved-memory" {
                        in_reserved_memory = true;
                    }
                }
                Token::EndNode => {
                    if depth == 2 && in_reserved_memory {
                        // Walked the whole /reserved-memory node; nothing more to find.
                        return Ok(found);
                    }
                }
                Token::Property { name, value } => {
                    // The root's cell counts are the defaults for the reserved-memory children,
                    // and the node may declare its own.
                    if depth == 1 || (depth == 2 && in_reserved_memory) {
                        declare_cells(name, value, &mut address_cells, &mut size_cells);
                    }
                    if depth == 3 && in_reserved_memory && name == b"reg" {
                        let remaining = out.get_mut(found..).ok_or(Error::TooManyRegions)?;
                        found += decode_reg(value, address_cells, size_cells, remaining)?;
                    }
                }
            }
        }
        Ok(found)
    }

    /// The initial ramdisk, if the bootloader placed one.
    ///
    /// Declared in `/chosen` as `linux,initrd-start` and `linux,initrd-end`.
    ///
    /// **This memory is the kernel's to protect.** The bootloader loaded a file into RAM for it
    /// and said where it put it. A kernel that does not reserve it lets its frame allocator hand it
    /// out to the first caller, and the initial ramdisk is destroyed before anything reads a byte of
    /// it, a bug that shows up far from its cause.
    ///
    /// Name: ratified 2026-10-12 (#1998). Refused `initrd` (the foreign spelling, which stops at the
    /// boundary where it arrives).
    pub fn initial_ramdisk(&self) -> Result<Option<Region>, Error> {
        let mut start: Option<u64> = None;
        let mut end: Option<u64> = None;
        let mut in_chosen = false;
        let mut tokens = self.tokens();

        while let Some((depth, token)) = tokens.step()? {
            match token {
                Token::BeginNode { name } => {
                    if depth == 2 {
                        in_chosen = name == b"chosen";
                    }
                }
                Token::EndNode => {
                    if depth == 2 {
                        in_chosen = false;
                    }
                }
                Token::Property { name, value } => {
                    if depth == 2 && in_chosen {
                        match name {
                            b"linux,initrd-start" => start = Some(integer(value)?),
                            b"linux,initrd-end" => end = Some(integer(value)?),
                            _ => {}
                        }
                    }
                }
            }
        }

        Ok(match (start, end) {
            // `linux,initrd-end` is exclusive, so an empty initial ramdisk (start == end) is a
            // "no".
            (Some(start), Some(end)) if end > start => Some(Region {
                start,
                size: end - start,
            }),
            _ => None,
        })
    }

    /// **The kernel's command line**: `/chosen/bootargs`, without its terminating NUL, or
    /// `Ok(None)` when the tree has no `/chosen` or no such property.
    ///
    /// QEMU writes it from `-append`, U-Boot from its `bootargs` variable, and a UEFI loader from
    /// the device-tree patch it applies. Only the root's own `chosen` child counts, as for
    /// [`Self::initial_ramdisk`]. A value with no NUL is returned whole, since the property's
    /// length bounds it either way.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC). Spelled out rather than `bootargs`, the
    /// binding's spelling, for the reason the ramdisk's accessor refused `initrd`.
    pub fn boot_arguments(&self) -> Result<Option<&'a [u8]>, Error> {
        let mut in_chosen = false;
        let mut tokens = self.tokens();

        while let Some((depth, token)) = tokens.step()? {
            match token {
                Token::BeginNode { name } => {
                    if depth == 2 && name == b"chosen" {
                        in_chosen = true;
                    }
                }
                Token::EndNode => {
                    if depth == 2 && in_chosen {
                        return Ok(None);
                    }
                }
                Token::Property { name, value } => {
                    if depth == 2 && in_chosen && name == b"bootargs" {
                        let end = value.iter().position(|&b| b == 0).unwrap_or(value.len());
                        return Ok(Some(&value[..end]));
                    }
                }
            }
        }
        Ok(None)
    }

    /// A walk over the structure block's tokens, from its first.
    fn tokens(&self) -> Tokens<'_, 'a> {
        Tokens {
            tree: self,
            at: self.off_struct,
            depth: 0,
        }
    }

    /// The header of the property whose `FDT_PROP` token ended just before `at`: its value's length,
    /// the offset of its name in the strings block, where its value starts, and where the next
    /// token starts (after the value's padding).
    ///
    /// Every sum is checked, because `len` and the name offset are `u32`s from the blob and a target
    /// whose `usize` is 32 bits wide can overflow them. The value's whole extent must lie inside
    /// the structure block, so a caller can slice `value_at..value_at + len` without a further check.
    fn property_header(&self, at: usize) -> Result<PropertyHeader, Error> {
        let len = be32(self.structure, at)? as usize;
        let name_off = be32(self.structure, at.checked_add(4).ok_or(Error::Truncated)?)? as usize;
        let value_at = at.checked_add(8).ok_or(Error::Truncated)?;
        let value_end = value_at.checked_add(len).ok_or(Error::Truncated)?;
        if value_end > self.structure.len() {
            return Err(Error::Truncated);
        }
        Ok(PropertyHeader {
            length_bytes: len,
            name_offset: name_off,
            value_at,
            next: skip_padded(value_at, len)?,
        })
    }

    /// A property's name: the string at `name_off` in the strings block.
    fn string(&self, name_off: usize) -> Result<&'a [u8], Error> {
        self.cstr(
            self.off_strings
                .checked_add(name_off)
                .ok_or(Error::Truncated)?,
        )
    }

    /// A node's name: the string at `at` in the structure block, which must end inside it.
    fn node_name(&self, at: usize) -> Result<&'a [u8], Error> {
        let rest = self.structure.get(at..).ok_or(Error::Truncated)?;
        let end = rest.iter().position(|&b| b == 0).ok_or(Error::BadValue)?;
        Ok(&rest[..end])
    }

    /// A null-terminated string in the blob, returned without its terminator.
    fn cstr(&self, at: usize) -> Result<&'a [u8], Error> {
        let rest = self.bytes.get(at..).ok_or(Error::Truncated)?;
        let end = rest.iter().position(|&b| b == 0).ok_or(Error::BadValue)?;
        Ok(&rest[..end])
    }
}

/// One slot per depth a walk can reach, and one for depth 0 above the root, so a per-depth stack
/// indexed by a depth [`Tokens`] hands out is never out of bounds.
const DEPTH_SLOTS: usize = MAXIMUM_DEPTH + 1;

/// One token of the structure block, as [`Tokens`] hands it out. `FDT_NOP` is skipped and
/// `FDT_END` ends the walk, so neither appears.
enum Token<'a> {
    /// A node opens. Its depth is its own.
    BeginNode { name: &'a [u8] },
    /// The node open at this depth closes.
    EndNode,
    /// A property of the node open at this depth: its name from the strings block, and its value,
    /// which lies inside the structure block.
    Property { name: &'a [u8], value: &'a [u8] },
}

/// The one walk over the structure block that every lookup uses.
///
/// It carries the checks every lookup needs in one place: each token advances the cursor, the walk
/// never leaves the structure block, the nesting balances, and it never goes deeper than
/// [`MAXIMUM_DEPTH`]. So a depth it hands out is between 0 and `MAXIMUM_DEPTH`, and a node's depth
/// is at least 1. libfdt's `fdt_next_tag` is the same shape.
struct Tokens<'t, 'a> {
    tree: &'t DeviceTreeBlob<'a>,
    at: usize,
    /// How many nodes are open.
    depth: usize,
}

impl<'a> Tokens<'_, 'a> {
    /// The next token, with the depth of the node it belongs to; `Ok(None)` at `FDT_END`.
    fn step(&mut self) -> Result<Option<(usize, Token<'a>)>, Error> {
        loop {
            let token = be32(self.tree.structure, self.at)?;
            // `be32` succeeded, so four bytes lay inside the block at `at`, and this cannot wrap.
            self.at += 4;
            match token {
                FDT_BEGIN_NODE => {
                    let name = self.tree.node_name(self.at)?;
                    self.at = skip_name(self.at, name)?;
                    if self.depth == MAXIMUM_DEPTH {
                        return Err(Error::TooDeep);
                    }
                    self.depth += 1;
                    return Ok(Some((self.depth, Token::BeginNode { name })));
                }
                FDT_END_NODE => {
                    let depth = self.depth;
                    self.depth = depth.checked_sub(1).ok_or(Error::BadStructure)?;
                    return Ok(Some((depth, Token::EndNode)));
                }
                FDT_PROP => {
                    let header = self.tree.property_header(self.at)?;
                    self.at = header.next;
                    let name = self.tree.string(header.name_offset)?;
                    // `property_header` checked the value lies inside the structure block.
                    let value = &self.tree.structure
                        [header.value_at..header.value_at + header.length_bytes];
                    return Ok(Some((self.depth, Token::Property { name, value })));
                }
                FDT_NOP => {}
                FDT_END if self.depth == 0 => return Ok(None),
                FDT_END => return Err(Error::BadStructure),
                other => return Err(Error::BadToken(other)),
            }
        }
    }
}

/// Record a `#address-cells` or `#size-cells` declaration into the counts the node declares for
/// its children. A declaration that is not exactly one cell is ignored, so the inherited width
/// stands: a shorter value read as a cell would take the padding and the next token as the count.
fn declare_cells(name: &[u8], value: &[u8], address_cells: &mut u32, size_cells: &mut u32) {
    let Ok(cell) = <[u8; 4]>::try_from(value) else {
        return;
    };
    match name {
        b"#address-cells" => *address_cells = u32::from_be_bytes(cell),
        b"#size-cells" => *size_cells = u32::from_be_bytes(cell),
        _ => {}
    }
}

/// Whether a `compatible` value, a list of NUL-terminated strings, lists `entry`. The empty piece
/// after the final NUL is not an entry, so an empty `entry` matches nothing.
fn lists_entry(value: &[u8], entry: &[u8]) -> bool {
    value
        .split(|&b| b == 0)
        .any(|piece| !piece.is_empty() && piece == entry)
}

/// An integer property.
///
/// The specification lets these be either 32 or 64 bits wide, and **the only way to tell is the
/// property's length**. QEMU writes `linux,initrd-start` as 8 bytes; a 32-bit platform writes 4.
/// Assume one and you silently misread the other.
fn integer(value: &[u8]) -> Result<u64, Error> {
    match value.len() {
        4 => Ok(be32(value, 0)? as u64),
        8 => be64(value, 0),
        _ => Err(Error::BadValue),
    }
}

/// Decode a `reg` property: a packed list of (address, size) pairs, where each value is `cells`
/// 32-bit big-endian words concatenated.
fn decode_reg(
    value: &[u8],
    address_cells: u32,
    size_cells: u32,
    out: &mut [Region],
) -> Result<usize, Error> {
    // Every product below is checked, because the cell counts come from the blob and a target
    // whose `usize` is 32 bits wide can overflow them. An overflow is a refusal.
    let cell_bytes = |cells: u32| (cells as usize).checked_mul(4).ok_or(Error::Truncated);
    let start_bytes = cell_bytes(address_cells)?;
    let pair_bytes = start_bytes
        .checked_add(cell_bytes(size_cells)?)
        .ok_or(Error::Truncated)?;
    if pair_bytes == 0 {
        return Ok(0);
    }

    let mut n = 0;
    for pair in value.chunks_exact(pair_bytes) {
        let (start, size) = pair.split_at(start_bytes);
        let start = cells(start)?;
        let size = cells(size)?;

        // A zero-size region is legal and useless. Skip it rather than handing the allocator an
        // empty range to reason about.
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

/// Concatenate 32-bit big-endian cells into one u64.
fn cells(value: &[u8]) -> Result<u64, Error> {
    let mut number = 0u64;
    for cell in value.chunks_exact(4) {
        number = (number << 32) | be32(cell, 0)? as u64;
    }
    Ok(number)
}

/// A property header, as [`DeviceTreeBlob::property_header`] reads it.
struct PropertyHeader {
    length_bytes: usize,
    name_offset: usize,
    value_at: usize,
    next: usize,
}

/// Everything in the structure block is padded to a 4-byte boundary. `None` for a length so close
/// to `usize::MAX` that rounding up wraps, which only a 32-bit target's `u32` length can reach.
fn align4(n: usize) -> Option<usize> {
    n.checked_add(3).map(|up| up & !3)
}

/// Where the token after `n` bytes of padded data at `at` starts, or `Truncated` when the sum wraps.
fn skip_padded(at: usize, n: usize) -> Result<usize, Error> {
    align4(n)
        .and_then(|padded| at.checked_add(padded))
        .ok_or(Error::Truncated)
}

/// Where the token after a node's name starts: the name, its NUL, and the padding to four bytes.
fn skip_name(at: usize, name: &[u8]) -> Result<usize, Error> {
    skip_padded(at, name.len().checked_add(1).ok_or(Error::Truncated)?)
}

/// The cursor arithmetic, on the values a 32-bit target's `usize` would overflow, which a 64-bit
/// host cannot reach through a blob (a blob's lengths are `u32`).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_step_that_would_wrap_is_refused_and_a_normal_one_is_not() {
        assert_eq!(skip_padded(8, 5), Ok(16));
        assert_eq!(skip_padded(0, 0), Ok(0));
        assert_eq!(skip_padded(usize::MAX - 2, 4), Err(Error::Truncated));
        assert_eq!(skip_padded(0, usize::MAX - 2), Err(Error::Truncated));
        assert_eq!(skip_padded(0, usize::MAX), Err(Error::Truncated));
        assert_eq!(align4(usize::MAX - 3), Some(usize::MAX - 3));
        assert_eq!(align4(usize::MAX - 2), None);
        assert_eq!(skip_name(4, b"abc"), Ok(8));
    }
}

/// Machine-checked proofs, with Kani, of the device-tree parser's leaf readers.
///
/// The device tree is untrusted boot input, and the whole-parse walk (a token loop over the
/// structure block) is past what bounded model checking can do over a blob of real size. So the
/// leaves are proved: the big-endian readers every
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

    /// **`align4` rounds up to a multiple of four.** For any length, `align4(n)` is either `None`
    /// (only when rounding up would wrap) or a multiple of 4 that is at least `n` and less than
    /// `n + 4`.
    /// Falsification: replayable `crates/device_tree_blob/falsifications/verification.align4_rounds_up_to_a_multiple_of_four.patch`
    #[kani::proof]
    fn align4_rounds_up_to_a_multiple_of_four() {
        let n: usize = kani::any();
        match align4(n) {
            Some(a) => {
                assert!(a.is_multiple_of(4));
                assert!(a >= n);
                assert!(a - n < 4);
            }
            None => assert!(n > usize::MAX - 3),
        }
    }

    /// **Stepping over padded data is total.** For any position and length, `skip_padded` returns
    /// a position at least as far along or refuses; it never panics or wraps.
    /// Falsification: replayable `crates/device_tree_blob/falsifications/verification.skip_padded_never_wraps.patch`
    #[kani::proof]
    fn skip_padded_never_wraps() {
        let at: usize = kani::any();
        let n: usize = kani::any();
        if let Ok(next) = skip_padded(at, n) {
            assert!(next >= at);
            assert!(next - at >= n);
        }
    }
}
