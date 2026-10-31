// What a freestanding probe needs and core does not supply, appended to each *-bare.rs probe by
// probe.sh so a probe stays one file: a panic handler, an allocator and an exit. The allocator is
// the tree's vendored talc over a static arena, the shape of talc's own no_std example.

const ARENA_BYTES: usize = 4 << 20;

// Atomic cells rather than a `static mut`: they carry interior mutability, so the heap may write
// through them, and nothing else names them.
static ARENA: [core::sync::atomic::AtomicU8; ARENA_BYTES] =
    [const { core::sync::atomic::AtomicU8::new(0) }; ARENA_BYTES];

#[global_allocator]
static HEAP: talc::TalcLock<spinning_top::RawSpinlock, talc::source::Claim> = talc::TalcLock::new(unsafe {
    // SAFETY: ARENA is interior-mutable, the allocator is its only user, and AtomicU8 has u8's
    // size and alignment, so the cast covers exactly ARENA_BYTES bytes.
    talc::source::Claim::array(ARENA.as_ptr() as *mut [u8; ARENA_BYTES])
});

pub fn halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    halt()
}
