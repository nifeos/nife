// Declared here and nowhere else, so no other module can even name `alloc`: a `no_std` crate
// has no `alloc` in its extern prelude unless the crate root declares it.
extern crate alloc;

use super::BUDGET;

/// **Room for a fixed number of values, on the heap.** A slice once filled (indexing works), and
/// there is no way to grow it: `Vec` is private to this module, so nothing outside can call a
/// method that would allocate and panic on failure.
pub(crate) struct Room<T>(alloc::vec::Vec<T>);

impl<T> core::ops::Deref for Room<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<T> core::ops::DerefMut for Room<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.0
    }
}

/// `n` copies of `x`, or `HeapFull`. Capacity is reserved with `try_reserve_exact`, and the
/// copies go in with `push_within_capacity`, which cannot allocate.
pub fn filled<T: Copy>(n: usize, x: T) -> Result<Room<T>, swish::Say> {
    let mut v = alloc::vec::Vec::new();
    v.try_reserve_exact(n).map_err(|_| swish::Say::HeapFull)?;
    for _ in 0..n {
        v.push_within_capacity(x)
            .map_err(|_| swish::Say::HeapFull)?;
    }
    Ok(Room(v))
}

/// **This shell's heap** (milestone 47 (navigation and naming), calef's ruling of 2026-09-26).
/// Capped at [`swish::HEAP_MAX_BYTES`] and mapped from [`BUDGET`] before any other carve, so a
/// leak exhausts the heap and never the budget children are built from.
#[global_allocator]
static HEAP: memory_region_heap::MemoryRegionHeap =
    memory_region_heap::MemoryRegionHeap::new();

/// Wire the allocator to [`BUDGET`] and map the whole capped heap at once. First thing at `_start`,
/// for the roles whose slot 3 is a budget: the heap's pages and their page tables are then the
/// budget's first retypes, under every later carve, which is what those carves' last-in first-out
/// return needs (`MemoryRegionHeap::commit_all`). A budget too small to cover it leaves what did
/// map usable, and an allocation past it refuses its line with [`Say::HeapFull`](crate::Say::HeapFull).
pub(super) fn heap_init() {
    HEAP.init(
        BUDGET,
        memory_region_heap::DEFAULT_BASE,
        swish::HEAP_MAX_BYTES,
    );
    HEAP.commit_all();
}
