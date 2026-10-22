//! **Getting a screen on the two architectures whose firmware never gives one** (milestone 243).
//!
//! *Module name provisional (AGENTS.md: an architect names modules).*
//!
//! Only `x86_64`'s boot chain includes a stage that has already lit a display. This module is the
//! discovery half for aarch64 and riscv64, under the emulator; the console-on-a-framebuffer path
//! itself (`machine_discovery::framebuffer`, `screen_console`) is arch-neutral.
//!
//! - **On real boards**, the answer is milestone 157 (real display output on the board): U-Boot
//!   configures the display and advertises a `simple-framebuffer` node, and the kernel reads where
//!   it is. That is board work and stays in 157.
//! - **Under QEMU**, nothing has lit a display, so [`ramfb`](crate::drivers::ramfb) inverts the
//!   arrangement: the guest provides the memory and the emulator scans it out. That is this module.
//!
//! Both end at `console::attach_screen` with the same [`Framebuffer`], so 157 adds a branch above
//! this one and changes nothing below it.
//!
//! The pixels are a static because a `ramfb` needs a physically contiguous, permanently owned
//! region **before the frame allocator exists** (the boot-tour console is armed before
//! `memory::init`), and the allocator has no contiguous multi-frame request anyway.
//!

//! # EXAMPLES
//!
//! There is one caller, `kernel_main`'s boot tour, and the whole of it is:
//!
//! ```text
//! match screen::attach() {
//!     Ok((found, cols, rows)) => println!("  screen      : {}x{} ...", found.width, found.height),
//!     Err(why) => println!("  screen      : none ({why})"),
//! }
//! ```
//!
//! `cargo xtask test --arch aarch64` asks QEMU for `-device ramfb` and a monitor, takes a
//! `screendump`, and decodes it back into text with `serial_console::screen`, the gate that proves
//! the glyphs and not merely the boot line.
//!
//! # BUGS
//!
//! - **[`SPAN`] bytes of `.bss` are spent whether or not there is a screen**: 1.9 MB zeroed and
//!   never written on the VisionFive 2, where `ramfb` cannot exist.
//! - **The geometry is fixed at compile time**: 800x600, the smallest that holds the boot tour
//!   without scrolling being the common case.
//! - **Nothing here runs on `x86_64`**, which has its own discovery in
//!   `arch::x86_64::machine::attach_screen` and a real aperture. The two share a type, not a path.

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

/// **The pixels.**
///
/// Page-aligned, because `ramfb` is handed a physical address and a page boundary is the one
/// alignment every stage of the address arithmetic below agrees about.
#[repr(C, align(4096))]
struct Pixels([u8; SPAN]);

/// The scratch the `fw_cfg` conversation runs through: a command and one buffer. Eight-aligned,
/// which is what the driver's contract asks for.
#[repr(C, align(8))]
struct Scratch([u8; SCRATCH_LEN]);

/// **`.bss`, and the boot tour is single-threaded when it is touched.** Both statics are written by
/// exactly one core at exactly one point in `kernel_main`, before any other core is started and
/// before any interrupt can arrive, which is why they are not behind the console lock. After
/// [`attach`] returns, the console's own lock is what serializes the pixels, and nothing ever
/// touches [`SCRATCH`] again.
///
/// `SyncUnsafeCell`s rather than locks, since milestone 891 (a mutable static becomes an atomic, a
/// lock or a recorded cell): the emulator is handed both by physical address and reads them as
/// device memory, so they need a fixed placement and in-place writes, and nothing here ever forms a
/// reference to either. Only raw pointers leave this module.
static PIXELS: SyncUnsafeCell<Pixels> = SyncUnsafeCell::new(Pixels([0; SPAN]));
static SCRATCH: SyncUnsafeCell<Scratch> = SyncUnsafeCell::new(Scratch([0; SCRATCH_LEN]));

/// Why there is no screen. A string rather than an enum because the only consumer is one boot line:
/// nothing branches on this, a person reads it, and four of the five cases are already spelled by
/// [`ramfb::Error`].
pub(crate) type Why = &'static str;

/// **Bring up a `ramfb` and give it to the console.**
///
/// Returns what was brought up and the character grid it produced, for the caller's boot line.
///
/// **It must run before `arch::mmu::init`.** The `fw_cfg` register block is reached through the
/// coarse boot map, which covers every device window; the fine map does not carry it, because after
/// this call there is nothing more to say to the device. The framebuffer itself is the kernel's own
/// `.bss` and is mapped by both maps at the same address, which is what lets the console keep the
/// pointer across the switch without anybody re-pointing it.
///
/// # Errors
///
/// A string naming which of the five machines this is: no device tree, no `fw_cfg` node in it, no
/// `etc/ramfb` file behind that node (the ordinary case: QEMU started without `-device ramfb`), the
/// device refusing, or a geometry `screen_console` cannot hold a cell of.
// Dead on x86_64, which has its own discovery and a real aperture. Gated at the module rather than
// here would mean a `cfg` at the one call site too; this keeps the call site arch-neutral.
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

/// **Whether a framebuffer is this kernel's own memory rather than a device's aperture.**
///
/// The one question the rest of the kernel has to ask about a `ramfb`, and it is a safety property
/// rather than a curiosity: `user::boot_screen_terminal` hands the screen's physical range to a
/// userspace driver to map, which is right for a UEFI aperture (a BAR on a display adapter, memory
/// no part of this kernel owns) and is a hole for this one (`.bss`, next to every other kernel
/// static). Asked here because this module is what created the situation.
///
/// Answered by range rather than by a flag on the screen, so that it stays true for any future
/// caller that arrives with a framebuffer from somewhere else, milestone 157's included: U-Boot's
/// aperture is outside the kernel image and so passes, as it should.
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
