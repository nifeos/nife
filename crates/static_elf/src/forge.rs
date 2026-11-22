//! **The test forge**: an ELF64 image built by hand, shared by the parser's tests and the note
//! reader's, so the byte offsets are written once. A forged hostile file is a few lines here; one
//! from a real toolchain would need that toolchain to cooperate.
//!
//! Name: provisional, minted 2026-11-22 (UTC) by lane 1088-finish-review.

use crate::note::PT_NOTE;
use crate::{
    EHDR_SIZE, ELFCLASS64, ELFDATA2LSB, ET_EXEC, EV_CURRENT, MAGIC, NATIVE_MACHINE, PHDR_SIZE,
    PT_LOAD, READABLE,
};
extern crate std;
use std::vec;
use std::vec::Vec;

/// One program header and the bytes it covers.
struct Header {
    kind: u32,
    flags: u32,
    virtual_address: u64,
    physical_address: u64,
    bytes: Vec<u8>,
    memory_bytes: u64,
    alignment: u64,
}

/// An ELF64 image under construction: the header fields a test may lie about, then the program
/// headers in order. The header table follows the 64-byte file header, and each segment's bytes
/// follow the table in the same order.
pub(crate) struct Builder {
    pub(crate) e_type: u16,
    pub(crate) e_machine: u16,
    pub(crate) class: u8,
    pub(crate) data: u8,
    pub(crate) version: u8,
    pub(crate) magic: [u8; 4],
    pub(crate) entry: u64,
    headers: Vec<Header>,
    /// Written as every segment's `p_filesz` in place of its byte count.
    pub(crate) lie_about_filesz: Option<u64>,
    /// Written as every segment's `p_offset` in place of where its bytes are.
    pub(crate) lie_about_offset: Option<u64>,
}

impl Builder {
    /// A static `ET_EXEC` for the machine this build accepts, so every forgery reaches the property
    /// past the machine check on any host, with its entry at `0x40_0000` and no segments.
    pub(crate) fn new() -> Self {
        Builder {
            e_type: ET_EXEC,
            e_machine: NATIVE_MACHINE,
            class: ELFCLASS64,
            data: ELFDATA2LSB,
            version: EV_CURRENT,
            magic: MAGIC,
            entry: 0x40_0000,
            headers: vec![],
            lie_about_filesz: None,
            lie_about_offset: None,
        }
    }

    /// A `PT_LOAD` whose physical address equals its virtual one, which is what every real user
    /// program looks like. Use `seg_at_paddr` for the kernel-image shape.
    pub(crate) fn seg(self, flags: u32, vaddr: u64, bytes: &[u8], memsz: u64) -> Self {
        self.seg_at_paddr(flags, vaddr, vaddr, bytes, memsz)
    }

    /// A `PT_LOAD` whose `p_paddr` and `p_vaddr` genuinely differ, the way a kernel image linked
    /// high and loaded low does.
    pub(crate) fn seg_at_paddr(
        mut self,
        flags: u32,
        vaddr: u64,
        paddr: u64,
        bytes: &[u8],
        memsz: u64,
    ) -> Self {
        self.headers.push(Header {
            kind: PT_LOAD,
            flags,
            virtual_address: vaddr,
            physical_address: paddr,
            bytes: bytes.to_vec(),
            memory_bytes: memsz,
            alignment: 0,
        });
        self
    }

    /// A readable `PT_NOTE` holding `bytes`, with `p_align` `alignment` and no address.
    pub(crate) fn note_segment(mut self, alignment: u64, bytes: &[u8]) -> Self {
        self.headers.push(Header {
            kind: PT_NOTE,
            flags: READABLE,
            virtual_address: 0,
            physical_address: 0,
            bytes: bytes.to_vec(),
            memory_bytes: bytes.len() as u64,
            alignment,
        });
        self
    }

    /// The image's bytes.
    pub(crate) fn build(self) -> Vec<u8> {
        let phnum = self.headers.len();
        let phoff = EHDR_SIZE;
        let mut body_off = EHDR_SIZE + phnum * PHDR_SIZE;

        let mut ehdr = vec![0u8; EHDR_SIZE];
        ehdr[0..4].copy_from_slice(&self.magic);
        ehdr[4] = self.class;
        ehdr[5] = self.data;
        ehdr[6] = self.version;
        ehdr[16..18].copy_from_slice(&self.e_type.to_le_bytes());
        ehdr[18..20].copy_from_slice(&self.e_machine.to_le_bytes());
        ehdr[24..32].copy_from_slice(&self.entry.to_le_bytes());
        ehdr[32..40].copy_from_slice(&(phoff as u64).to_le_bytes());
        ehdr[54..56].copy_from_slice(&(PHDR_SIZE as u16).to_le_bytes());
        ehdr[56..58].copy_from_slice(&(phnum as u16).to_le_bytes());

        let mut phdrs = vec![];
        let mut body = vec![];
        for header in &self.headers {
            let mut ph = vec![0u8; PHDR_SIZE];
            ph[0..4].copy_from_slice(&header.kind.to_le_bytes());
            ph[4..8].copy_from_slice(&header.flags.to_le_bytes());
            let off = self.lie_about_offset.unwrap_or(body_off as u64);
            ph[8..16].copy_from_slice(&off.to_le_bytes());
            ph[16..24].copy_from_slice(&header.virtual_address.to_le_bytes());
            ph[24..32].copy_from_slice(&header.physical_address.to_le_bytes());
            let fsz = self.lie_about_filesz.unwrap_or(header.bytes.len() as u64);
            ph[32..40].copy_from_slice(&fsz.to_le_bytes());
            ph[40..48].copy_from_slice(&header.memory_bytes.to_le_bytes());
            ph[48..56].copy_from_slice(&header.alignment.to_le_bytes());
            phdrs.extend_from_slice(&ph);
            body.extend_from_slice(&header.bytes);
            body_off += header.bytes.len();
        }

        let mut out = ehdr;
        out.extend_from_slice(&phdrs);
        out.extend_from_slice(&body);
        out
    }
}
