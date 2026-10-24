//! **Telling a device-tree kernel where its archive is**: a copy of the firmware's device tree with
//! `/chosen/linux,initrd-start` and `linux,initrd-end` set.
//!
//! On aarch64 and riscv64 the kernel finds the userspace archive the way Linux finds an initial
//! ramdisk, from those two properties (`memory::init`, via `DeviceTreeBlob::initial_ramdisk`).
//! QEMU's `-initrd` and U-Boot's `booti` write them; **UEFI firmware does not**, because a UEFI
//! loader places the archive itself and is expected to say so. This is the Linux EFI stub's job
//! (`drivers/firmware/efi/libstub/fdt.c`, recalled, not read) in its smallest form. The format is
//! the Devicetree Specification v0.4, chapter 5, and every test decodes the output with
//! `crates/device_tree_blob`, the crate the kernel decodes with, so the two cannot drift apart.

//! # BUGS
//!
//! - **The firmware's memory map is not carried into the tree.** Linux's stub replaces the
//!   `/memory` nodes with the UEFI map; this does not, so memory the firmware keeps for its runtime
//!   services is, to the kernel, ordinary RAM. That is harmless for exactly one reason, which is
//!   that nife never calls a UEFI runtime service. The day it does, this has to reserve those ranges
//!   (`/memreserve/` entries or `/reserved-memory` children) first.
//! - **Nothing is removed**: a stale `linux,initrd-*` pair the firmware left is replaced, and every
//!   other property is copied as it was.

/// The device-tree magic, big-endian at offset 0.
pub(crate) const MAGIC: u32 = 0xd00d_feed;
pub(crate) const HEADER_LEN: usize = 40;
pub(crate) const BEGIN_NODE: u32 = 1;
pub(crate) const END_NODE: u32 = 2;
pub(crate) const PROP: u32 = 3;
const NOP: u32 = 4;
pub(crate) const END: u32 = 9;

const INITRD_START: &[u8] = b"linux,initrd-start";
const INITRD_END: &[u8] = b"linux,initrd-end";

/// Why a tree could not be rewritten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The magic is wrong: this is not a flattened device tree.
    NotADeviceTree,
    /// A header offset or a token runs past the blob.
    Truncated,
    /// A token this format does not define, or nodes that do not nest.
    Malformed,
    /// The output buffer is smaller than [`output_len`] said it had to be.
    OutputTooSmall,
}

impl Error {
    /// The sentence for the firmware console.
    pub const fn reason(self) -> &'static str {
        match self {
            Error::NotADeviceTree => "the firmware's device tree has the wrong magic",
            Error::Truncated => "the firmware's device tree is truncated",
            Error::Malformed => "the firmware's device tree is malformed",
            Error::OutputTooSmall => "no room to rewrite the device tree",
        }
    }
}

fn be32(bytes: &[u8], at: usize) -> Result<u32, Error> {
    let b = bytes.get(at..at + 4).ok_or(Error::Truncated)?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

const fn align4(n: usize) -> usize {
    (n + 3) & !3
}

/// The parts of the header this needs.
struct Header {
    total: usize,
    off_struct: usize,
    off_strings: usize,
    off_rsvmap: usize,
    size_strings: usize,
    size_struct: usize,
    boot_cpuid: u32,
}

fn header(src: &[u8]) -> Result<Header, Error> {
    if be32(src, 0)? != MAGIC {
        return Err(Error::NotADeviceTree);
    }
    let h = Header {
        total: be32(src, 4)? as usize,
        off_struct: be32(src, 8)? as usize,
        off_strings: be32(src, 12)? as usize,
        off_rsvmap: be32(src, 16)? as usize,
        boot_cpuid: be32(src, 28)?,
        size_strings: be32(src, 32)? as usize,
        size_struct: be32(src, 36)? as usize,
    };
    if h.total > src.len()
        || h.off_struct
            .checked_add(h.size_struct)
            .is_none_or(|e| e > h.total)
        || h.off_strings
            .checked_add(h.size_strings)
            .is_none_or(|e| e > h.total)
        || h.off_rsvmap >= h.total
    {
        return Err(Error::Truncated);
    }
    Ok(h)
}

/// The size of the reservation block: 16-byte entries up to and including the all-zero one.
fn rsvmap_len(src: &[u8], h: &Header) -> Result<usize, Error> {
    let mut at = h.off_rsvmap;
    loop {
        let entry = src.get(at..at + 16).ok_or(Error::Truncated)?;
        at += 16;
        if entry.iter().all(|&b| b == 0) {
            return Ok(at - h.off_rsvmap);
        }
    }
}

/// **An upper bound on the rewritten tree's size**: the original plus a new `/chosen` node, two
/// eight-byte properties and their two names. The caller allocates this much.
pub fn output_len(src: &[u8]) -> Result<usize, Error> {
    let h = header(src)?;
    let rsv = rsvmap_len(src, &h)?;
    // BEGIN_NODE "chosen" (4 + 8) + END_NODE (4) + two props (12 + 8 each) + two names.
    let extra = 12 + 4 + 2 * 20 + INITRD_START.len() + 1 + INITRD_END.len() + 1;
    // **Exact, not padded**: this is the size `with_initial_ramdisk` writes for a tree with no
    // `/chosen` and neither name in its strings, which is the largest it can write, and a test
    // holds the two equal. The header needs no padding before the reservation block because 40 is
    // already the multiple of 8 that block needs (asserted below), so `with_initial_ramdisk`'s
    // `pad_to(8)` writes nothing. Until milestone 326 (2026-09-24) this added two unexplained 8s,
    // and fifteen mutants of its arithmetic survived because a bound with slack cannot be told from
    // a wrong one.
    const _: () = assert!(HEADER_LEN.is_multiple_of(8));
    Ok(HEADER_LEN + rsv + h.size_struct + h.size_strings + extra)
}

/// Offset of `name` in the strings block, if it is already there as a whole string.
fn find_string(strings: &[u8], name: &[u8]) -> Option<usize> {
    let mut at = 0;
    while at < strings.len() {
        let end = strings[at..].iter().position(|&b| b == 0)? + at;
        if &strings[at..end] == name {
            return Some(at);
        }
        at = end + 1;
    }
    None
}

pub(crate) struct Out<'a> {
    pub(crate) buf: &'a mut [u8],
    pub(crate) at: usize,
}

impl Out<'_> {
    pub(crate) fn bytes(&mut self, b: &[u8]) -> Result<(), Error> {
        let end = self.at + b.len();
        self.buf
            .get_mut(self.at..end)
            .ok_or(Error::OutputTooSmall)?
            .copy_from_slice(b);
        self.at = end;
        Ok(())
    }
    pub(crate) fn u32(&mut self, v: u32) -> Result<(), Error> {
        self.bytes(&v.to_be_bytes())
    }
    pub(crate) fn pad_to(&mut self, align: usize) -> Result<(), Error> {
        while !self.at.is_multiple_of(align) {
            self.bytes(&[0])?;
        }
        Ok(())
    }
    fn prop_u64(&mut self, name_off: usize, value: u64) -> Result<(), Error> {
        self.u32(PROP)?;
        self.u32(8)?;
        self.u32(name_off as u32)?;
        self.bytes(&value.to_be_bytes())
    }
}

/// **Rewrite `src` into `out` with `/chosen/linux,initrd-{start,end}` set to `[start, end)`**, and
/// return the new tree's length. `out` must be at least [`output_len`] bytes.
///
/// `/chosen` is created as the root's last child if the firmware's tree has none.
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11).
/// His words: "B for #1998." Refused `with_initrd` (the foreign spelling, which stops at the
/// boundary where it arrives).
pub fn with_initial_ramdisk(
    src: &[u8],
    start: u64,
    end: u64,
    out: &mut [u8],
) -> Result<usize, Error> {
    let h = header(src)?;
    let rsv = rsvmap_len(src, &h)?;
    let strings = &src[h.off_strings..h.off_strings + h.size_strings];
    let names = ramdisk_property_offsets(strings, h.size_strings);

    let mut o = Out { buf: out, at: 0 };
    o.bytes(&[0; HEADER_LEN])?; // filled in last
    o.pad_to(8)?;
    let new_rsvmap = o.at;
    o.bytes(&src[h.off_rsvmap..h.off_rsvmap + rsv])?;
    let new_struct = o.at;

    let mut walk = Walk {
        src,
        strings,
        struct_end: h.off_struct + h.size_struct,
        start,
        end,
        start_off: names.start_off,
        end_off: names.end_off,
        at: h.off_struct,
        depth: 0,
        in_chosen: false,
        wrote_chosen: false,
    };
    walk.copy_struct(&mut o)?;
    let size_struct = o.at - new_struct;
    let new_strings = o.at;
    o.bytes(strings)?;
    for name in &names.appended[..names.appended_count] {
        o.bytes(name)?;
        o.bytes(&[0])?;
    }
    let size_strings = o.at - new_strings;
    let total = o.at;

    let fields: [u32; 10] = [
        MAGIC,
        total as u32,
        new_struct as u32,
        new_strings as u32,
        new_rsvmap as u32,
        17, // version
        16, // last compatible version
        h.boot_cpuid,
        size_strings as u32,
        size_struct as u32,
    ];
    for (i, field) in fields.iter().enumerate() {
        o.buf[i * 4..i * 4 + 4].copy_from_slice(&field.to_be_bytes());
    }
    Ok(total)
}

/// The two `linux,initrd-*` property names' offsets in the rewritten strings block, and the names
/// that have to be appended to it.
struct NameOffsets {
    start_off: usize,
    end_off: usize,
    appended: [&'static [u8]; 2],
    appended_count: usize,
}

/// Find or place the two `linux,initrd-*` property names in the strings block.
fn ramdisk_property_offsets(strings: &[u8], size_strings: usize) -> NameOffsets {
    // Name offsets: reuse the firmware's where the names already exist, append otherwise.
    let mut appended: [&[u8]; 2] = [&[], &[]];
    let mut appended_count = 0;
    let mut next_off = size_strings;
    let mut name_off = |name: &'static [u8]| -> usize {
        if let Some(off) = find_string(strings, name) {
            return off;
        }
        let off = next_off;
        next_off += name.len() + 1;
        appended[appended_count] = name;
        appended_count += 1;
        off
    };
    let start_off = name_off(INITRD_START);
    let end_off = name_off(INITRD_END);
    NameOffsets {
        start_off,
        end_off,
        appended,
        appended_count,
    }
}

/// One pass over the structure block, copying it to the output with the `linux,initrd-*` pair set.
struct Walk<'a> {
    src: &'a [u8],
    strings: &'a [u8],
    struct_end: usize,
    start: u64,
    end: u64,
    start_off: usize,
    end_off: usize,
    at: usize,
    depth: usize,
    in_chosen: bool,
    wrote_chosen: bool,
}

impl Walk<'_> {
    /// Copy every token up to and including `END`, refusing a block that ends early or nests badly.
    fn copy_struct(&mut self, o: &mut Out<'_>) -> Result<(), Error> {
        loop {
            if self.at >= self.struct_end {
                return Err(Error::Truncated);
            }
            let token = be32(self.src, self.at)?;
            match token {
                BEGIN_NODE => self.begin_node(o)?,
                END_NODE => self.end_node(o)?,
                PROP => self.prop(o)?,
                NOP => self.at += 4,
                END => {
                    if self.depth != 0 {
                        return Err(Error::Malformed);
                    }
                    o.u32(END)?;
                    break;
                }
                _ => return Err(Error::Malformed),
            }
        }
        Ok(())
    }

    /// Copy a node's opening, and write the `linux,initrd-*` pair first thing inside `/chosen`.
    fn begin_node(&mut self, o: &mut Out<'_>) -> Result<(), Error> {
        let src = self.src;
        let at = self.at;
        let name_at = at + 4;
        let name_len = src
            .get(name_at..self.struct_end)
            .and_then(|s| s.iter().position(|&b| b == 0))
            .ok_or(Error::Truncated)?;
        let next = name_at + align4(name_len + 1);
        o.bytes(&src[at..next])?;
        self.depth += 1;
        if self.depth == 2 && &src[name_at..name_at + name_len] == b"chosen" {
            self.in_chosen = true;
            self.wrote_chosen = true;
            o.prop_u64(self.start_off, self.start)?;
            o.prop_u64(self.end_off, self.end)?;
        }
        self.at = next;
        Ok(())
    }

    /// Copy a node's closing, adding `/chosen` before the root closes if there was none.
    fn end_node(&mut self, o: &mut Out<'_>) -> Result<(), Error> {
        if self.depth == 0 {
            return Err(Error::Malformed);
        }
        if self.depth == 1 && !self.wrote_chosen {
            // The root is closing and there was no /chosen: add one as its last child.
            o.u32(BEGIN_NODE)?;
            o.bytes(b"chosen\0\0")?;
            o.prop_u64(self.start_off, self.start)?;
            o.prop_u64(self.end_off, self.end)?;
            o.u32(END_NODE)?;
            self.wrote_chosen = true;
        }
        if self.depth == 2 {
            self.in_chosen = false;
        }
        o.u32(END_NODE)?;
        self.depth -= 1;
        self.at += 4;
        Ok(())
    }

    /// Copy a property, dropping a stale `linux,initrd-*` pair inside `/chosen`.
    fn prop(&mut self, o: &mut Out<'_>) -> Result<(), Error> {
        let src = self.src;
        let at = self.at;
        let len = be32(src, at + 4)? as usize;
        let nameoff = be32(src, at + 8)? as usize;
        let next = at + 12 + align4(len);
        if next > self.struct_end {
            return Err(Error::Truncated);
        }
        let name = self
            .strings
            .get(nameoff..)
            .and_then(|s| s.iter().position(|&b| b == 0).map(|n| &s[..n]))
            .ok_or(Error::Truncated)?;
        // Replace, never duplicate: a stale pair from the firmware is dropped.
        let ours =
            self.in_chosen && self.depth == 2 && (name == INITRD_START || name == INITRD_END);
        if !ours {
            o.bytes(&src[at..next])?;
        }
        self.at = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use device_tree_blob::{DeviceTreeBlob, Region};

    use super::*;

    const PLAIN: &[u8] =
        include_bytes!("../../crates/device_tree_blob/tests/fixtures/qemu-aarch64-virt.dtb");
    const WITH_INITIAL_RAMDISK: &[u8] = include_bytes!(
        "../../crates/device_tree_blob/tests/fixtures/qemu-aarch64-virt-initial-ramdisk.dtb"
    );
    const RISCV: &[u8] =
        include_bytes!("../../crates/device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb");

    fn rewrite(src: &[u8], start: u64, end: u64) -> Vec<u8> {
        let mut out = vec![0; output_len(src).unwrap()];
        let n = with_initial_ramdisk(src, start, end, &mut out).unwrap();
        out.truncate(n);
        out
    }

    /// Everything the kernel reads from the tree, before and after, except the archive.
    fn same_machine(a: &[u8], b: &[u8]) {
        let (a, b) = (
            DeviceTreeBlob::from_bytes(a).unwrap(),
            DeviceTreeBlob::from_bytes(b).unwrap(),
        );
        let mut ra = [Region { start: 0, size: 0 }; 8];
        let mut rb = [Region { start: 0, size: 0 }; 8];
        let na = a.memory_regions(&mut ra).unwrap();
        let nb = b.memory_regions(&mut rb).unwrap();
        assert_eq!(ra[..na], rb[..nb], "the RAM the kernel sees");
        let na = a.reserved_memory_regions(&mut ra).unwrap();
        let nb = b.reserved_memory_regions(&mut rb).unwrap();
        assert_eq!(
            ra[..na],
            rb[..nb],
            "the /reserved-memory the kernel honours"
        );
        assert_eq!(
            a.node_prop(b"chosen", b"stdout-path").unwrap(),
            b.node_prop(b"chosen", b"stdout-path").unwrap()
        );
    }

    #[test]
    fn the_kernel_reads_back_the_archive_it_is_told_about() {
        for src in [PLAIN, WITH_INITIAL_RAMDISK, RISCV] {
            let out = rewrite(src, 0x4800_0000, 0x4849_0000);
            let tree = DeviceTreeBlob::from_bytes(&out).unwrap();
            assert_eq!(
                tree.initial_ramdisk().unwrap(),
                Some(Region {
                    start: 0x4800_0000,
                    size: 0x49_0000
                })
            );
            same_machine(src, &out);
            assert!(out.len() <= output_len(src).unwrap());
        }
    }

    #[test]
    fn a_stale_pair_from_the_firmware_is_replaced_not_duplicated() {
        let before = DeviceTreeBlob::from_bytes(WITH_INITIAL_RAMDISK)
            .unwrap()
            .initial_ramdisk()
            .unwrap();
        assert!(
            before.is_some(),
            "the fixture carries QEMU's own -initrd pair"
        );
        let out = rewrite(WITH_INITIAL_RAMDISK, 0x9000_0000, 0x9000_1000);
        let tree = DeviceTreeBlob::from_bytes(&out).unwrap();
        assert_eq!(
            tree.initial_ramdisk().unwrap(),
            Some(Region {
                start: 0x9000_0000,
                size: 0x1000
            })
        );
        let count = out
            .windows(INITRD_START.len())
            .filter(|w| *w == INITRD_START)
            .count();
        assert_eq!(count, 1, "the name is in the strings block once");
    }

    #[test]
    fn a_tree_without_chosen_gets_one() {
        // Build a minimal tree by hand: root with one property and no children.
        let mut src = vec![0u8; 40];
        let rsv = src.len();
        src.extend_from_slice(&[0; 16]);
        let st = src.len();
        for w in [BEGIN_NODE, 0, PROP, 4, 0, 0x2a, END_NODE, END] {
            src.extend_from_slice(&w.to_be_bytes());
        }
        let size_struct = src.len() - st;
        let strings = src.len();
        src.extend_from_slice(b"answer\0");
        let fields = [
            MAGIC,
            src.len() as u32,
            st as u32,
            strings as u32,
            rsv as u32,
            17,
            16,
            0,
            7,
            size_struct as u32,
        ];
        for (i, f) in fields.iter().enumerate() {
            src[i * 4..i * 4 + 4].copy_from_slice(&f.to_be_bytes());
        }
        let out = rewrite(&src, 0x1000, 0x2000);
        let tree = DeviceTreeBlob::from_bytes(&out).unwrap();
        assert_eq!(
            tree.initial_ramdisk().unwrap(),
            Some(Region {
                start: 0x1000,
                size: 0x1000
            })
        );
    }

    #[test]
    fn refuses_what_is_not_a_tree() {
        let mut out = [0u8; 256];
        assert_eq!(
            with_initial_ramdisk(&[0; 64], 0, 1, &mut out),
            Err(Error::NotADeviceTree)
        );
        assert_eq!(
            with_initial_ramdisk(&PLAIN[..20], 0, 1, &mut out),
            Err(Error::Truncated)
        );
        assert_eq!(
            with_initial_ramdisk(PLAIN, 0, 1, &mut out),
            Err(Error::OutputTooSmall),
            "256 bytes is not a whole QEMU tree"
        );
    }

    // --- Milestone 326 (turn a mutation score upward), 2026-09-24. The 2026-09-21 census left
    // --- 43 mutants of this file alive; notes/mutation-testing/uefi-loader.md has which test
    // --- killed which. Every tree below is built by hand so each property has one cause.

    fn word(s: &mut Vec<u8>, w: u32) {
        s.extend_from_slice(&w.to_be_bytes());
    }
    fn pad4(s: &mut Vec<u8>) {
        while !s.len().is_multiple_of(4) {
            s.push(0);
        }
    }
    fn begin(s: &mut Vec<u8>, name: &str) {
        word(s, BEGIN_NODE);
        s.extend_from_slice(name.as_bytes());
        s.push(0);
        pad4(s);
    }
    fn prop(s: &mut Vec<u8>, name_off: u32, data: &[u8]) {
        word(s, PROP);
        word(s, data.len() as u32);
        word(s, name_off);
        s.extend_from_slice(data);
        pad4(s);
    }

    /// Strings for the hand-built trees: `answer` at 0, `linux,initrd-start` at 7.
    const STRINGS: &[u8] = b"answer\0linux,initrd-start\0";

    /// A flattened tree around `structure` (which must end with `END`): header, reservation block
    /// holding `reserved` and its terminator, then the structure and strings blocks in that order
    /// or, with `strings_first`, the other, then `slack` bytes the header's total still covers.
    fn tree(
        reserved: &[(u64, u64)],
        structure: &[u8],
        strings_first: bool,
        slack: usize,
    ) -> Vec<u8> {
        let mut src = vec![0u8; HEADER_LEN];
        let rsv = src.len();
        for (address, size) in reserved.iter().chain([&(0, 0)]) {
            src.extend_from_slice(&address.to_be_bytes());
            src.extend_from_slice(&size.to_be_bytes());
        }
        let (st, strings);
        if strings_first {
            strings = src.len();
            src.extend_from_slice(STRINGS);
            pad4(&mut src);
            st = src.len();
            src.extend_from_slice(structure);
        } else {
            st = src.len();
            src.extend_from_slice(structure);
            strings = src.len();
            src.extend_from_slice(STRINGS);
        }
        src.extend(core::iter::repeat_n(0, slack));
        let fields = [
            MAGIC,
            src.len() as u32,
            st as u32,
            strings as u32,
            rsv as u32,
            17,
            16,
            3, // boot_cpuid_phys, which must survive the rewrite
            STRINGS.len() as u32,
            structure.len() as u32,
        ];
        for (i, f) in fields.iter().enumerate() {
            src[i * 4..i * 4 + 4].copy_from_slice(&f.to_be_bytes());
        }
        src
    }

    /// The root with one property and no children.
    fn bare_root() -> Vec<u8> {
        let mut s = Vec::new();
        begin(&mut s, "");
        prop(&mut s, 0, &[0, 0, 0, 0x2a]);
        word(&mut s, END_NODE);
        word(&mut s, END);
        s
    }

    fn field(blob: &[u8], i: usize) -> usize {
        u32::from_be_bytes(blob[i * 4..i * 4 + 4].try_into().unwrap()) as usize
    }

    /// How many nodes called `chosen`, at any depth, the structure block holds.
    fn chosen_nodes(blob: &[u8]) -> usize {
        let (st, size) = (field(blob, 2), field(blob, 9));
        let mut token = BEGIN_NODE.to_be_bytes().to_vec();
        token.extend_from_slice(b"chosen\0\0");
        blob[st..st + size]
            .windows(token.len())
            .filter(|w| *w == token)
            .count()
    }

    /// **The bound is exact for the tree it is a bound for**: a tree with no `/chosen` and neither
    /// name in its strings is rewritten to precisely `output_len` bytes, and every other tree to
    /// fewer. The caller allocates this from the firmware, so a bound short by a byte is a boot that
    /// stops at "no room to rewrite the device tree".
    #[test]
    fn the_bound_is_exact_for_a_tree_with_nothing_to_reuse() {
        let src = tree(&[(0x8000_0000, 0x1000)], &bare_root(), false, 0);
        let mut plain = src.clone();
        // Take `linux,initrd-start` out of the strings so neither name can be reused.
        let at = field(&plain, 3) + 7;
        plain[at] = b'X';
        let mut out = vec![0; output_len(&plain).unwrap()];
        assert_eq!(
            with_initial_ramdisk(&plain, 0x1000, 0x2000, &mut out),
            Ok(output_len(&plain).unwrap())
        );
        let mut out = vec![0; output_len(&src).unwrap()];
        let n = with_initial_ramdisk(&src, 0x1000, 0x2000, &mut out).unwrap();
        assert_eq!(n + 19, output_len(&src).unwrap(), "one name reused");
    }

    /// **The reservation block is carried entry for entry**, terminator included. It is where
    /// firmware says which memory is not the kernel's, so losing an entry hands the kernel memory
    /// that is in use.
    #[test]
    fn the_reservation_block_is_carried_entry_for_entry() {
        let reserved = [(0x8000_0000, 0x1000), (0x9000_0000, 0x2_0000)];
        let src = tree(&reserved, &bare_root(), false, 0);
        let out = rewrite(&src, 0x1000, 0x2000);
        let (from, to) = (field(&src, 4), field(&out, 4));
        assert_eq!(
            out[to..to + 48],
            src[from..from + 48],
            "two entries and the zero one"
        );
        let mut regions = [Region { start: 0, size: 0 }; 4];
        let n = DeviceTreeBlob::from_bytes(&out)
            .unwrap()
            .reserved_regions(&mut regions)
            .unwrap();
        assert_eq!(
            regions[..n],
            [
                Region {
                    start: 0x8000_0000,
                    size: 0x1000
                },
                Region {
                    start: 0x9000_0000,
                    size: 0x2_0000
                }
            ]
        );
    }

    /// **The header describes the blocks that were written**: each block's size is the distance
    /// to the next, the structure block ends on `END`, and the boot CPU is the firmware's.
    #[test]
    fn the_header_describes_the_blocks_it_wrote() {
        for src in [PLAIN, RISCV, &tree(&[], &bare_root(), false, 0)[..]] {
            let out = rewrite(src, 0x4800_0000, 0x4849_0000);
            let (total, st, strings, rsv) = (
                field(&out, 1),
                field(&out, 2),
                field(&out, 3),
                field(&out, 4),
            );
            assert_eq!(field(&out, 0) as u32, MAGIC);
            assert_eq!(total, out.len());
            assert_eq!(rsv, HEADER_LEN);
            assert_eq!(st + field(&out, 9), strings, "size_dt_struct");
            assert_eq!(strings + field(&out, 8), total, "size_dt_strings");
            assert_eq!(out[strings - 4..strings], END.to_be_bytes());
            assert_eq!((field(&out, 5), field(&out, 6)), (17, 16));
            assert_eq!(field(&out, 7), field(src, 7), "boot_cpuid_phys");
        }
    }

    /// **A header that points outside the blob is refused, not followed**, one field at a time; and
    /// the layouts that are legal but unusual are accepted: a buffer longer than the tree, space
    /// the tree's total covers after its last block, and the structure block last.
    #[test]
    fn each_header_bound_is_checked_on_its_own() {
        let mut out = vec![0u8; 4096];
        let src = tree(&[], &bare_root(), false, 0);
        let with = |i: usize, v: usize| {
            let mut t = src.clone();
            t[i * 4..i * 4 + 4].copy_from_slice(&(v as u32).to_be_bytes());
            t
        };
        let len = src.len();
        assert_eq!(
            with_initial_ramdisk(&with(1, len + 1), 0, 1, &mut out),
            Err(Error::Truncated),
            "total"
        );
        assert_eq!(
            with_initial_ramdisk(&with(9, len), 0, 1, &mut out),
            Err(Error::Truncated),
            "struct"
        );
        assert_eq!(
            with_initial_ramdisk(&with(8, len), 0, 1, &mut out),
            Err(Error::Truncated),
            "strings"
        );
        assert_eq!(
            with_initial_ramdisk(&with(4, len), 0, 1, &mut out),
            Err(Error::Truncated),
            "rsvmap"
        );

        let mut longer = src.clone();
        longer.extend_from_slice(&[0xff; 64]);
        assert!(
            with_initial_ramdisk(&longer, 0, 1, &mut out).is_ok(),
            "a buffer longer than the tree"
        );
        let slack = tree(&[], &bare_root(), false, 16);
        assert!(
            with_initial_ramdisk(&slack, 0, 1, &mut out).is_ok(),
            "covered space after the strings"
        );
        let struct_last = tree(&[], &bare_root(), true, 0);
        assert!(
            with_initial_ramdisk(&struct_last, 0, 1, &mut out).is_ok(),
            "the structure block last"
        );
    }

    /// **`NOP` tokens are skipped and dropped**: the specification lets firmware blank a property
    /// out with them in place, so a walker that stops at one loses the rest of the tree.
    #[test]
    fn nop_tokens_are_skipped_and_dropped() {
        let mut s = Vec::new();
        begin(&mut s, "");
        word(&mut s, NOP);
        prop(&mut s, 0, &[0, 0, 0, 0x2a]);
        word(&mut s, NOP);
        word(&mut s, END_NODE);
        word(&mut s, END);
        let out = rewrite(&tree(&[], &s, false, 0), 0x1000, 0x2000);
        let without = rewrite(&tree(&[], &bare_root(), false, 0), 0x1000, 0x2000);
        assert_eq!(out, without, "the same tree, less its NOPs");
        let tree = DeviceTreeBlob::from_bytes(&out).unwrap();
        assert_eq!(
            tree.initial_ramdisk().unwrap(),
            Some(Region {
                start: 0x1000,
                size: 0x1000
            })
        );
    }

    /// **Only the root's `chosen` child is `/chosen`.** A node that happens to be called `chosen`
    /// deeper down gets nothing, the real one still gets added, and a tree that has `/chosen`
    /// keeps exactly one.
    #[test]
    fn only_the_roots_chosen_child_receives_the_pair() {
        let nested = |name: &str| {
            let mut s = Vec::new();
            begin(&mut s, "");
            begin(&mut s, "soc");
            begin(&mut s, name);
            word(&mut s, END_NODE);
            word(&mut s, END_NODE);
            word(&mut s, END_NODE);
            word(&mut s, END);
            rewrite(&tree(&[], &s, false, 0), 0x1000, 0x2000)
        };
        let out = nested("chosen");
        let tree_out = DeviceTreeBlob::from_bytes(&out).unwrap();
        assert_eq!(
            tree_out.initial_ramdisk().unwrap(),
            Some(Region {
                start: 0x1000,
                size: 0x1000
            })
        );
        assert_eq!(chosen_nodes(&out), 2, "/soc/chosen and the new /chosen");
        // Byte for byte what a node of any other name there produces, less the name itself.
        let mut other = nested("chosex");
        let at = other.windows(7).position(|w| w == b"chosex\0").unwrap();
        other[at + 5] = b'n';
        assert_eq!(out, other, "/soc/chosen is treated like any other node");

        for src in [PLAIN, WITH_INITIAL_RAMDISK, RISCV] {
            let out = rewrite(src, 0x1000, 0x2000);
            assert_eq!(chosen_nodes(&out), chosen_nodes(src), "no second /chosen");
        }
    }

    /// **Replacing stops at `/chosen`'s end**: a later sibling that carries a property of the same
    /// name keeps it, because the pair being replaced is `/chosen`'s alone.
    #[test]
    fn a_sibling_after_chosen_keeps_its_own_properties() {
        let mut s = Vec::new();
        begin(&mut s, "");
        begin(&mut s, "chosen");
        word(&mut s, END_NODE);
        begin(&mut s, "other");
        prop(&mut s, 7, &0x1234u64.to_be_bytes());
        word(&mut s, END_NODE);
        word(&mut s, END_NODE);
        word(&mut s, END);
        let out = rewrite(&tree(&[], &s, false, 0), 0x1000, 0x2000);
        let tree = DeviceTreeBlob::from_bytes(&out).unwrap();
        assert_eq!(
            tree.node_prop(b"other", INITRD_START).unwrap(),
            Some(&0x1234u64.to_be_bytes()[..])
        );
    }

    /// **A property whose length runs past the structure block is refused**, rather than copied
    /// from bytes that are not the tree (or past the end of the buffer, which in a UEFI application
    /// is a panic with no console to report it).
    #[test]
    fn a_property_longer_than_its_block_is_refused() {
        let mut s = bare_root();
        s[8 + 4..8 + 8].copy_from_slice(&0x1000u32.to_be_bytes()); // the property's length
        let mut out = vec![0u8; 4096];
        assert_eq!(
            with_initial_ramdisk(&tree(&[], &s, false, 0), 0, 1, &mut out),
            Err(Error::Truncated)
        );
    }

    /// **Every refusal has its own sentence for the firmware console**, since it is the only thing
    /// a person at a machine with no other output will see.
    #[test]
    fn every_refusal_says_something_different() {
        let all = [
            Error::NotADeviceTree,
            Error::Truncated,
            Error::Malformed,
            Error::OutputTooSmall,
        ];
        for (i, a) in all.iter().enumerate() {
            assert!(a.reason().contains("device tree"), "{a:?}");
            for b in &all[i + 1..] {
                assert_ne!(a.reason(), b.reason());
            }
        }
    }
}
