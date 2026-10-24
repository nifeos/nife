# Device details: an appendix to notes/progenitor-boot.md

## The entropy refusal

No instruction, or a first draw of all zeros (`entropy_contract::readiness`), and the instruction
service is refused with a sentence on the console. There is no fallback to `RDRAND` or `RNDR`
(DRBG output, which `components/src/entropy.rs`'s header refuses) and no software generator. The
bytes served are the instruction's own, unmixed and not health-tested after the first draw: option
A of DECISIONS §137 (a hardware TRNG with no published health-test claim), which every other backend here takes. Choosing B or C is calef's.

`entropy_service::ensure` hands the readiness report to whoever wired the service first, and on
`x86_64` that can be the installer earlier in the same boot. The installer does not read the
verdict either. A condemned service then answers `NO_ENTROPY` to every request, the progenitor's own
first draw (the login password) fails, and it builds no login stack.

## The screen handover

The order is the handover. `boot_screen_terminal` finds both programs first, so a build that lacks
one leaves the kernel painting rather than a blank screen. Then `crate::console::yield_screen`
clears the screen and stops the kernel's `print!` from painting it, under the console lock, and only
then is the driver spawned. There is no moment with two painters. Afterward the kernel's own lines
go to the UART alone.

A screen whose framebuffer is kernel memory is kept. `ramfb` on the two `virt` boards puts the
framebuffer in `kernel/src/screen.rs`'s own `.bss`, and mapping that range into a driver would hand a
userspace process a window onto kernel statics. The check is a range test made before the yield,
so a refusal leaves the kernel painting, and milestone 157 (real display output on the board)'s U-Boot aperture passes it unchanged.

A refusal after the yield (a screen too large to map, `display_service`'s `MAX_APERTURE_PAGES`)
leaves the screen blank, and the boot goes on over the UART. That is a bound no screen in the fleet
is near, so it has no second handover path. When the function returns, the driver and terminal are
running and the terminal has painted its blank grid.
