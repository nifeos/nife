//! What a bare-metal library test image needs: a runner with nothing to run and a panic handler.
//!
//! Name: provisional.

/// The test runner of a bare-metal build: the host tests are gated off there, so it runs nothing.
pub(crate) fn bare_metal_runner(_tests: &[&dyn Fn()]) {}

/// A bare-metal test image links no panic runtime, so it carries the one handler the build needs.
#[panic_handler]
fn bare_metal_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
