//! The input format and the invariants: decode a byte string into heap operations, apply each,
//! and check the heap after every one. The fuzz workspace's README.md lists both.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane talc-fuzz-and-miri.

use core::alloc::Layout;

use crate::policy::{Heap, PAGE, PageSource, nonempty};

/// The largest arena an input may ask for, 16 MiB: past a typical program's heap, and small
/// enough that a run stays inside libFuzzer's memory limit.
pub(crate) const MAXIMUM_ARENA_PAGES: u64 = 4096;

/// Operations past this count are ignored, so one input cannot run for minutes.
pub(crate) const MAXIMUM_OPERATIONS: usize = 512;

/// Bytes at each end of a block that every step checks; the whole block is checked when it is
/// freed, moved, or a full check is asked for.
const EDGE_BYTES: usize = 16;

/// The operation codes, each the first byte of an operation, taken modulo the count.
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub(crate) enum Operation {
    /// `initialize(slot, base, maximum)`; slot byte `0xFF` stands for no region.
    Initialize = 0,
    /// `commit_all`.
    CommitAll = 1,
    /// `alloc(layout)`.
    Allocate = 2,
    /// `dealloc` of a live block, chosen by index.
    Deallocate = 3,
    /// `realloc` of a live block to a new size.
    Reallocate = 4,
    /// Check every byte of every live block.
    FullCheck = 5,
    /// Change the pages the region still pays for.
    SetBudget = 6,
    /// Allocate a block sized to end at the top of the free bytes, near the arena's end.
    AllocateAtTop = 7,
}

/// The number of operation codes.
pub(crate) const OPERATION_COUNT: u8 = 8;

impl Operation {
    fn from_byte(byte: u8) -> Self {
        match byte % OPERATION_COUNT {
            0 => Operation::Initialize,
            1 => Operation::CommitAll,
            2 => Operation::Allocate,
            3 => Operation::Deallocate,
            4 => Operation::Reallocate,
            5 => Operation::FullCheck,
            6 => Operation::SetBudget,
            _ => Operation::AllocateAtTop,
        }
    }
}

/// A reader over the input that yields zeros once the bytes run out, so every prefix is an input.
struct Reader<'a> {
    bytes: &'a [u8],
}

impl Reader<'_> {
    fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    fn byte(&mut self) -> u8 {
        match self.bytes.split_first() {
            Some((&first, rest)) => {
                self.bytes = rest;
                first
            }
            None => 0,
        }
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.byte(), self.byte()])
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }
}

/// A budget from two bytes: `0xFFFF` is unlimited, anything else that many pages.
fn budget(raw: u16) -> Option<u64> {
    (raw != u16::MAX).then_some(u64::from(raw))
}

/// A size from a class byte and a raw word. The classes reach the sizes a heap mishandles: empty,
/// small, a page, the arena's cap and just past it, powers of two and their neighbors, and sizes
/// near `isize::MAX` that no layout accepts.
fn size(class: u8, raw: u32, maximum: u64) -> usize {
    let raw = raw as usize;
    let maximum = maximum as usize;
    match class % 8 {
        0 => 0,
        1 => raw % 64,
        2 => raw % 4096,
        3 => raw % (1 << 20),
        4 => (maximum + raw % 8192).saturating_sub(4096),
        5 => (1usize << (raw % 40))
            .wrapping_add((raw >> 8) % 3)
            .wrapping_sub(1),
        6 => (isize::MAX as usize).saturating_sub(raw % 8192),
        _ => raw,
    }
}

/// An alignment from one byte: bit 7 asks for 64, a common cache line;
/// otherwise a power of two up to 64 KiB.
fn alignment(raw: u8) -> usize {
    if raw & 0x80 != 0 { 64 } else { 1 << (raw % 17) }
}

/// The byte a block holds at `offset`, a function of the block's tag and the offset, so a block
/// that moves or overlaps another shows a wrong byte.
fn pattern(tag: u32, offset: usize) -> u8 {
    let mixed = (tag as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (offset as u64);
    (mixed.wrapping_mul(0xBF58_476D_1CE4_E5B9) >> 56) as u8
}

/// A live allocation, as the program that asked for it knows it.
struct Block {
    pointer: *mut u8,
    layout: Layout,
    tag: u32,
}

impl Block {
    /// The bytes talc set aside for the block: at least one, as `nonempty` asks.
    fn extent(&self) -> usize {
        nonempty(self.layout).size()
    }

    fn fill(&self, from: usize) {
        for offset in from..self.layout.size() {
            // SAFETY: `offset` is inside the block, which is live and ours.
            unsafe { self.pointer.add(offset).write(pattern(self.tag, offset)) };
        }
    }

    fn check(&self, range: core::ops::Range<usize>) {
        for offset in range {
            // SAFETY: as `fill`.
            let byte = unsafe { self.pointer.add(offset).read() };
            assert_eq!(
                byte,
                pattern(self.tag, offset),
                "a live block's contents changed"
            );
        }
    }

    fn check_edges(&self) {
        let size = self.layout.size();
        if size <= 2 * EDGE_BYTES {
            self.check(0..size);
        } else {
            self.check(0..EDGE_BYTES);
            self.check(size - EDGE_BYTES..size);
        }
    }

    fn check_all(&self) {
        self.check(0..self.layout.size());
    }
}

/// The state one input drives.
struct Run {
    source: PageSource,
    heap: Heap,
    blocks: Vec<Block>,
    next_tag: u32,
    /// What `initialize` should have left, by its own rule: the first call that finds the heap
    /// uninitialized sets the base and cap, and later calls change nothing.
    expected: Option<(u64, u64)>,
}

impl Run {
    fn allocate(&mut self, layout: Layout) {
        // SAFETY: `alloc` takes any layout; `nonempty` handles the empty one.
        let pointer = unsafe { self.heap.alloc(&mut self.source, layout) };
        if pointer.is_null() {
            self.check_refusal(layout);
            return;
        }
        let block = Block {
            pointer,
            layout,
            tag: self.next_tag,
        };
        self.next_tag += 1;
        block.fill(0);
        self.blocks.push(block);
    }

    /// A refused allocation is one the heap could not grow for: no region, too little room under
    /// the cap, or a budget that ran out.
    fn check_refusal(&self, layout: Layout) {
        let (_, maximum) = self.heap.base_and_maximum();
        let room = maximum - self.heap.committed_bytes();
        let need = self.heap.shortfall(layout).div_ceil(PAGE) * PAGE;
        assert!(
            !self.heap.is_initialized() || room < need || self.source.budget_pages() == Some(0),
            "the heap refused an allocation it had room and budget for"
        );
    }

    fn reallocate(&mut self, index: usize, new_size: usize) {
        let block = &self.blocks[index];
        let old = block.layout;
        let Ok(new_layout) = Layout::from_size_align(new_size, old.align()) else {
            return;
        };
        if new_size == 0 {
            return;
        }
        block.check_all();
        // SAFETY: the block is live under `old`, `new_size` is nonzero, and `new_layout` exists.
        let pointer = unsafe {
            self.heap
                .realloc(&mut self.source, block.pointer, old, new_size)
        };
        if pointer.is_null() {
            self.check_refusal(new_layout);
            return;
        }
        let block = &mut self.blocks[index];
        block.pointer = pointer;
        block.layout = new_layout;
        block.check(0..old.size().min(new_size));
        block.fill(old.size().min(new_size));
    }

    fn deallocate(&mut self, index: usize) {
        let block = self.blocks.swap_remove(index);
        block.check_all();
        // SAFETY: the block is live under its layout, and leaves `blocks` here.
        unsafe { self.heap.dealloc(block.pointer, block.layout) };
    }

    /// The invariants every step keeps, as the fuzz workspace's README.md lists them.
    fn check(&mut self) {
        let committed = self.heap.committed_bytes();
        let (base, maximum) = self.heap.base_and_maximum();
        if let Some(expected) = self.expected {
            assert_eq!(
                (base, maximum),
                expected,
                "a later initialize changed the heap"
            );
        }
        assert_eq!(committed % PAGE, 0, "committed bytes are not whole pages");
        assert!(committed <= maximum, "the heap committed past its cap");
        assert_eq!(
            committed,
            self.source.mapped_bytes() as u64,
            "committed bytes differ from the pages mapped"
        );
        if committed > 0 {
            let end = self.heap.heap_end().map(|end| end.as_ptr() as u64);
            assert_eq!(
                end,
                Some(base + committed),
                "talc's heap end is not the top of the committed pages"
            );
        }

        let counters = *self.heap.counters();
        assert_eq!(
            counters.allocation_count,
            self.blocks.len(),
            "talc counts a different number of live allocations"
        );
        let live: usize = self.blocks.iter().map(Block::extent).sum();
        assert_eq!(
            counters.allocated_bytes, live,
            "talc counts different live bytes"
        );
        assert!(
            counters.claimed_bytes as u64 <= committed,
            "talc claims more than the heap committed"
        );
        assert!(counters.heap_count <= 1, "talc holds more than one heap");

        self.blocks
            .sort_unstable_by_key(|block| block.pointer as usize);
        let mut previous_end = base as usize;
        for block in &self.blocks {
            let start = block.pointer as usize;
            assert_eq!(
                start % block.layout.align(),
                0,
                "a block is not aligned as asked"
            );
            assert!(
                start >= previous_end,
                "two live blocks overlap, or one starts below the heap"
            );
            let end = start + block.extent();
            assert!(
                end as u64 <= base + committed,
                "a block runs past the committed pages"
            );
            previous_end = end;
            block.check_edges();
        }
    }

    fn step(&mut self, reader: &mut Reader<'_>) {
        let (_, maximum) = self.heap.base_and_maximum();
        match Operation::from_byte(reader.byte()) {
            Operation::Initialize => {
                let slot = reader.byte();
                let pages = u64::from(reader.u16());
                let extra = u64::from(reader.u16()) % PAGE;
                let arena = self.source.arena_bytes() as u64;
                let maximum_bytes = (pages * PAGE + extra).min(arena);
                let slot = if slot == u8::MAX {
                    u64::MAX
                } else {
                    u64::from(slot)
                };
                if !self.heap.is_initialized() {
                    self.expected = Some((self.source.base(), maximum_bytes & !(PAGE - 1)));
                }
                self.heap
                    .initialize(slot, self.source.base(), maximum_bytes);
            }
            Operation::CommitAll => {
                let mapped = self.heap.commit_all(&mut self.source);
                let (_, maximum) = self.heap.base_and_maximum();
                if mapped {
                    assert_eq!(
                        self.heap.committed_bytes(),
                        maximum,
                        "commit_all reported success short of the cap"
                    );
                }
            }
            Operation::Allocate => {
                let class = reader.byte();
                let raw = reader.u32();
                let align = alignment(reader.byte());
                if let Ok(layout) = Layout::from_size_align(size(class, raw, maximum), align) {
                    self.allocate(layout);
                }
            }
            Operation::Deallocate => {
                let index = usize::from(reader.u16());
                if !self.blocks.is_empty() {
                    self.deallocate(index % self.blocks.len());
                }
            }
            Operation::Reallocate => {
                let index = usize::from(reader.u16());
                let class = reader.byte();
                let raw = reader.u32();
                if !self.blocks.is_empty() {
                    self.reallocate(index % self.blocks.len(), size(class, raw, maximum));
                }
            }
            Operation::FullCheck => self.blocks.iter().for_each(Block::check_all),
            Operation::SetBudget => self.source.set_budget(budget(reader.u16())),
            Operation::AllocateAtTop => {
                let align = alignment(reader.byte());
                let free_top = self.heap.free_top() as usize;
                let size = free_top.saturating_sub(core::mem::size_of::<usize>()) & !31;
                if let Ok(layout) = Layout::from_size_align(size, align) {
                    self.allocate(layout);
                }
            }
        }
    }

    fn finish(mut self) {
        while !self.blocks.is_empty() {
            self.deallocate(self.blocks.len() - 1);
        }
        let counters = *self.heap.counters();
        assert_eq!(
            counters.allocation_count, 0,
            "talc counts a leaked allocation"
        );
        assert_eq!(counters.allocated_bytes, 0, "talc counts leaked bytes");
    }
}

/// Run one input, and panic on the first broken invariant. The header is two bytes of arena size in pages and two of page budget; each
/// operation after it is an [`Operation`] code and its arguments.
///
/// Name: provisional, minted 2026-11-14 (UTC) by lane talc-fuzz-and-miri.
pub fn run(data: &[u8]) {
    let mut reader = Reader { bytes: data };
    let arena_pages = u64::from(reader.u16()) % MAXIMUM_ARENA_PAGES + 1;
    let budget_pages = budget(reader.u16());
    let arena_bytes = (arena_pages * PAGE) as usize;
    let arena_layout = Layout::from_size_align(arena_bytes, PAGE as usize).expect("arena layout");
    // SAFETY: the layout is nonzero.
    let arena = unsafe { std::alloc::alloc(arena_layout) };
    assert!(!arena.is_null(), "the host refused the arena");

    {
        let mut run = Run {
            // SAFETY: the arena is page-aligned, `arena_bytes` long, and freed only after `run`.
            source: unsafe { PageSource::new(arena, arena_bytes, budget_pages) },
            heap: Heap::new(),
            blocks: Vec::new(),
            next_tag: 0,
            expected: None,
        };
        let mut operations = 0;
        while !reader.is_empty() && operations < MAXIMUM_OPERATIONS {
            run.step(&mut reader);
            run.check();
            operations += 1;
        }
        run.finish();
    }

    // SAFETY: allocated above with this layout; the heap and its source are gone.
    unsafe { std::alloc::dealloc(arena, arena_layout) };
}
