// What a freestanding probe needs and core does not supply, appended to each *-bare.rs probe by
// probe.sh so a probe stays one file: a panic handler, an allocator and an exit. The allocator is
// the tree's vendored talc over a static arena, the shape of talc's own no_std example.

#[global_allocator]
static HEAP: talc::TalcLock<spinning_top::RawSpinlock, talc::source::Claim> = talc::TalcLock::new(unsafe {
    static mut ARENA: [u8; 4 << 20] = [0; 4 << 20];
    // SAFETY: ARENA is used by nothing else, and this static is the only place it is named.
    talc::source::Claim::array(&raw mut ARENA)
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
