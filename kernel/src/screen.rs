//! A screen on aarch64 and riscv64 under QEMU, whose firmware never lights one: a
//! [`ramfb`](crate::drivers::ramfb) over static pixels, handed to `console::attach_screen` as the
//! same [`Framebuffer`] `x86_64` produces. A real board's display is milestone 157 (real display
//! output on the board).
//!
//! *Module name provisional (AGENTS.md: an architect names modules).*
//!
//! The pixels are a static because the region must be contiguous and owned before the frame
//! allocator exists. See notes/serial-less-output/ramfb.md.
//!

//! # EXAMPLES
//!
//! The one caller, `kernel_main`'s boot tour, whole:
//!
//! ```text
//! match screen::attach() {
//!     Ok((found, cols, rows)) => println!("  screen      : {}x{} ...", found.width, found.height),
//!     Err(why) => println!("  screen      : none ({why})"),
//! }
//! ```

//!
//! `cargo xtask test --arch aarch64` takes a `screendump` and decodes it back into text with
//! `serial_console::screen`, proving the glyphs and not merely the boot line.
//!

//! # BUGS
//!
//! - [`SPAN`] bytes of `.bss` are spent whether or not there is a screen: 1.9 MB on the
//!   VisionFive 2, where `ramfb` cannot exist.
//! - The geometry is fixed at compile time: 800x600, the smallest that mostly holds the boot tour.
//! - Nothing here runs on `x86_64`, which has its own discovery and a real aperture.

use core::cell::SyncUnsafeCell;

use machine_discovery::framebuffer::{Framebuffer, PixelOrder};

use crate::drivers::ramfb::{self, FirmwareConfiguration, SCRATCH_LEN};

/// The screen's width in pixels. See the module's BUGS for why it is a constant.
pub(crate) const WIDTH: u32 = 800;
/// The screen's height in pixels.
pub(crate) const HEIGHT: u32 = 600;
/// Bytes from one row to the next. No padding: this kernel owns the memory, so there is no firmware
/// pitch to respect, and `screen_console` reads this field rather than assuming it either way.
pub(crate) const STRIDE: u32 = WIDTH * 4;
/// The whole framebuffer, in bytes.
pub(crate) const SPAN: usize = (STRIDE * HEIGHT) as usize;

/// The pixels. Page-aligned, the one alignment every stage of the address arithmetic agrees on.
#[repr(C, align(4096))]
struct Pixels([u8; SPAN]);

/// The scratch the `fw_cfg` conversation runs through: a command and one buffer. Eight-aligned,
/// which is what the driver's contract asks for.
#[repr(C, align(8))]
struct Scratch([u8; SCRATCH_LEN]);

/// The pixels and the `fw_cfg` scratch, in `.bss`. Written by one core in `kernel_main` before any
/// other core or interrupt exists; after [`attach`] the console's lock serializes the pixels and
/// [`SCRATCH`] is never touched again. `SyncUnsafeCell`s because the emulator reads both by
/// physical address: only raw pointers leave this module.
static PIXELS: SyncUnsafeCell<Pixels> = SyncUnsafeCell::new(Pixels([0; SPAN]));
static SCRATCH: SyncUnsafeCell<Scratch> = SyncUnsafeCell::new(Scratch([0; SCRATCH_LEN]));

/// Why there is no screen. A string rather than an enum because the only consumer is one boot line:
/// nothing branches on this, a person reads it, and four of the five cases are already spelled by
/// [`ramfb::Error`].
pub(crate) type Why = &'static str;

/// Bring up a `ramfb` and give it to the console. Returns the framebuffer and its character grid,
/// for the caller's boot line. Must run before `arch::mmu::init`: only the coarse boot map reaches
/// `fw_cfg`. The pixels are `.bss`, mapped at the same address by both maps, so the console keeps
/// its pointer.
///
/// # Errors
///
/// A string naming which case this is: no device tree, no `fw_cfg` node, no `etc/ramfb` file (the
/// ordinary case, QEMU without `-device ramfb`), the device refusing, or too small a geometry.
// Dead on x86_64, which has its own discovery; allowed so the one call site stays arch-neutral.
#[cfg_attr(target_arch = "x86_64", allow(dead_code))]
pub fn attach() -> Result<(Framebuffer, u32, u32), Why> {
    let dtb = crate::device_tree().map_err(|_| "no device tree to find a fw_cfg in")?;
    let mut node = [device_tree_blob::Region { start: 0, size: 0 }; 1];
    let found = matches!(dtb.node_reg_compatible(b"qemu,fw-cfg-mmio", &mut node), Ok(n) if n >= 1);
    if !found {
        return Err("no qemu,fw-cfg-mmio node: this machine has no ramfb to ask for");
    }

    let pixels = PIXELS.get();
    let scratch = SCRATCH.get();
    let screen = Framebuffer {
        base: crate::arch::mmu::virt_to_phys(pixels as u64),
        width: WIDTH,
        height: HEIGHT,
        stride: STRIDE,
        // The guest owns these bytes, so the byte order is a choice rather than a discovery, and
        // this is the one that needs no swizzle: `PixelOrder::Bgrx`'s own doc says a colour written
        // the way every constant in this tree is written stores unchanged.
        order: PixelOrder::Bgrx,
    };

    // SAFETY: `node[0].start` is the register block the machine's own device tree describes,
    // reached through the coarse boot map that covers it (this runs before `arch::mmu::init`).
    // `scratch` is this module's `SCRATCH_LEN`-byte, eight-aligned static, and the physical address
    // handed alongside it is that same static's, by the same translation `arch::mmu::map_everything`
    // uses on the kernel image.
    let mut config = unsafe {
        FirmwareConfiguration::new(
            crate::arch::mmu::phys_to_virt(node[0].start),
            scratch.cast::<u8>(),
            crate::arch::mmu::virt_to_phys(scratch as u64),
        )
    };

    config.point_ramfb(screen).map_err(|why| match why {
        ramfb::Error::NoSuchFile => "this qemu was started without -device ramfb",
        ramfb::Error::Refused => "the fw_cfg device refused the ramfb configuration",
        ramfb::Error::Hung => "the fw_cfg device never finished a transfer",
        ramfb::Error::Undecodable => "the fw_cfg file directory did not decode",
    })?;

    // SAFETY: `PIXELS` is this module's own `.bss`, `SPAN` bytes of it, mapped writable by the boot
    // map now and by the fine map after `arch::mmu::init` (it is inside the kernel image, which
    // `map_everything` maps at its linked addresses). It is exactly the region the emulator was
    // just told to scan out, and from here on the console's lock is the only writer.
    let (cols, rows) = unsafe { crate::console::attach_screen(screen, pixels as u64) }
        .ok_or("the screen is too small for one character cell")?;
    Ok((screen, cols, rows))
}

/// Whether a framebuffer is this kernel's own memory rather than a device's aperture. A safety
/// property: `user::boot_screen_terminal` maps a screen's range into a userspace driver, right for
/// an aperture and a hole for `.bss`. Answered by range, not a flag, so it holds for a framebuffer
/// from anywhere.
#[must_use]
pub(crate) fn is_kernel_memory(screen: &Framebuffer) -> bool {
    let lo = crate::memory::image_start();
    let hi = crate::memory::image_end();
    let start = screen.base;
    let end = screen.base + screen.span().unwrap_or(0) as u64;
    start < hi && lo < end
}

/// **The boot tour's screen line**, for the two architectures that call [`attach`].
///
/// One spelling for two callers, so the aarch64 and riscv64 tours cannot drift into describing the
/// same mechanism two ways. It is the same column layout `console::print_summary` and the x86 arm
/// already use.
///
/// The failure case prints too, and that is the point of having a line at all: a machine with no
/// screen and a machine whose screen refused are different machines, and only this line tells them
/// apart on a wire that both boots otherwise fill identically.
#[cfg_attr(target_arch = "x86_64", allow(dead_code))]
pub(crate) fn print_summary(screen: &Result<(Framebuffer, u32, u32), Why>) {
    match screen {
        Ok((found, cols, rows)) => crate::println!(
            "  screen      : {}x{} {} at {:#x}, {cols}x{rows} cells (ramfb, this kernel's own memory)",
            found.width,
            found.height,
            found.order.token(),
            found.base,
        ),
        Err(why) => crate::println!("  screen      : none ({why}); this console is the UART"),
    }
}
