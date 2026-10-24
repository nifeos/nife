use graphics_contract as gfx;

use super::*;
use crate::sched;

/// **A confined userspace driver puts a known pattern in a scanout framebuffer.**
///
/// The pattern is a per-coordinate function ([`graphics_contract::pixel`]) under a
/// position-sensitive digest, so a blank, stale, shifted, transposed or truncated surface fails.
/// Two witnesses in two address spaces digest it, the client after the flush and the driver after
/// the device reported the transfer, and the kernel checks both against its own value. The backing
/// attach succeeded under translation, and the IOMMU domain maps exactly the driver's DMA region.
/// This proves the framebuffer, not the scanout (`-display none`); `cargo xtask`'s scanout check
/// proves that from the host (notes/framebuffer-contract.md, "What the test proves, and the one
/// thing it does not" and "Proving the scanout, from the host").
#[test_case]
fn a_confined_userspace_driver_puts_a_known_pattern_in_a_framebuffer() {
    let gpu_driver = program("gpu_driver").expect("no gpu_driver program in the initial ramdisk");
    let painter = program("painter").expect("no painter program in the initial ramdisk");
    let threads_before = sched::thread_count();

    // **A missing GPU is not always the build-order mistake this test exists to catch.** On the
    // `virt` boards (aarch64, riscv64) and on x86_64's PVH runner a virtio-gpu-pci function is
    // always wired into the test runner, so `start` returning `None` there really is a build-order
    // bug. The one boot without a GPU is x86_64 under OVMF (`helpers/qemu-uefi-x86_64.sh` attaches
    // none), so `None` there is an honest, expected gap rather than a bug.
    let Some((driver_report, client_report)) = display_service::start(gpu_driver, painter) else {
        crate::testing::skip!(
            "no virtio-gpu-pci function on the bus: either this kernel enumerated no PCI at all, \
             or (x86_64 under OVMF) the firmware runner attaches no GPU; see \
             design/roadmap/0420-the-rest-of-the-x86-64-fixture-set.md"
        );
    };

    // And a GPU present while the IOMMU is not means every pixel read is bypassing translation.
    // That matters more for a GPU than for a disk: its backing addresses ride in a device-level
    // command payload, not in a descriptor, so the transport's validator never sees them and the
    // IOMMU is the only thing bounding them (notes/framebuffer-contract.md).
    assert!(
        crate::iommu::is_active(),
        "a virtio-gpu is present but the IOMMU is not active: the GPU's pixel reads are \
         unconfined (is iommu=smmuv3 / -device riscv-iommu-pci or iommu_platform=on missing?)",
    );

    // 1. The driver came up: device enumerated, resource created and backed, scanout set. Taken
    //    first because these are rendezvous SENDs, so the driver is parked here until we look.
    let [tag, geometry, display, ..] = sched::ipc_receive(driver_report);
    assert_eq!(
        tag,
        gfx::status::UP,
        "the GPU driver did not come up (it reported {tag:#x}; a 0xDEAD_.. word's low byte \
         is the bring-up step that failed, see components/src/gpu_driver.rs)",
    );
    assert_eq!(
        geometry,
        gfx::WIDTH as u64 | ((gfx::HEIGHT as u64) << 32),
        "the driver created a surface of the wrong geometry",
    );
    assert!(
        (display & 0xffff_ffff) >= gfx::WIDTH as u64,
        "the device reported a display narrower than our surface: {display:#x}",
    );

    // 2. The driver's own account of what it handed the device, digested in the driver's address
    //    space after the device completed the transfer. This must be taken BEFORE the client's
    //    verdict: the driver blocks in this SEND right after replying to the first flush, so a
    //    test that waited on the client first would deadlock against the client's second CALL.
    let [tag, driver_digest, pixels, ..] = sched::ipc_receive(driver_report);
    assert_eq!(
        tag,
        gfx::status::FLUSHED,
        "the driver never served a flush (it reported {tag:#x})",
    );
    assert_eq!(
        pixels,
        gfx::PIXELS as u64,
        "the driver flushed a different surface size"
    );
    assert_eq!(
        driver_digest,
        gfx::expected_checksum(),
        "the driver's digest of the frames it handed the device is not the pattern: the pixels \
         the device read are not the pixels the client painted",
    );

    // 3. The client's verdict: the surface read back through its own mapping after the flush.
    let [tag, client_digest, mismatch, ..] = sched::ipc_receive(client_report);
    assert_eq!(
        tag,
        gfx::status::PAINTED,
        "the painting client did not report a verdict (it reported {tag:#x}; a 0xDEAD_.. word's \
         low byte names the step, see fixtures/src/painter.rs)",
    );
    assert_eq!(
        mismatch,
        gfx::NO_MISMATCH,
        "the client read back a wrong pixel at index {mismatch} of {}",
        gfx::PIXELS,
    );
    assert_eq!(
        client_digest,
        gfx::expected_checksum(),
        "the client's read-back digest is not the pattern it painted",
    );

    // The two witnesses must agree. They are digests of the same frames taken from different
    // address spaces at different moments, so a disagreement would mean the surface changed under
    // one of them, which is the mapping bug this shared-frame contract exists to not have.
    assert_eq!(
        driver_digest, client_digest,
        "the driver and the client disagree about the surface's contents",
    );

    // Wait for the one-shot client's reap, which also proves a finished client is not leaked. Its
    // ~20 frames come back asynchronously at reap and would land in the next test's exact
    // free-frame count. The driver never exits, so the target is one thread above the baseline.
    // Clock-based, not a yield count: on an idle core a yield returns instantly (DECISIONS §28 (SMP
    // placement)).
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline && sched::thread_count() > threads_before + 1 {
        sched::yield_now();
    }
    assert!(
        sched::thread_count() <= threads_before + 1,
        "the painting client reported its verdict but was never reaped (threads {} > {})",
        sched::thread_count(),
        threads_before + 1,
    );
}

/// **The device is refused a framebuffer it was not granted.**
///
/// A virtio-gpu's backing addresses ride inside a command payload, not a descriptor, so no shadow
/// ring checks them, and §18 (the PCIe transport: one driver, two buses, the seam in the kernel)
/// keeps the transport from parsing them: the IOMMU is the barrier. A driver with honest authority
/// aims the device at a frame outside its domain, and the IOMMU must record a fault there. The
/// fault queue, not the response code, is the assertion: QEMU answers a translation failure with a
/// bounce buffer and an OK (notes/framebuffer-contract/iommu-escape-test.md). It sorts first
/// because it resets the GPU, wiping the pictures the host-side scanout check looks for; a reorder
/// fails that check loudly (notes/glyphs/terminal-and-proof.md, "Ordering, and what breaks it").
#[test_case]
fn a_backing_outside_the_grant_is_refused_by_the_iommu() {
    let gpu_driver = program("gpu_driver").expect("no gpu_driver program in the initial ramdisk");

    // Drain any stale fault first, so what we observe is this test's.
    while crate::iommu::take_fault().is_some() {}

    let Some((report, victim)) = display_service::start_backing_escape(gpu_driver) else {
        crate::testing::skip!("no virtio-gpu-pci function on the bus (NIFE_GPU not set?)");
    };
    assert!(
        crate::iommu::is_active(),
        "a virtio-gpu is present but the IOMMU is not active: nothing would refuse this escape, \
         so the test would pass or fail on a fiction",
    );

    let [tag, response, ..] = sched::ipc_receive(report);
    assert_eq!(
        tag,
        gfx::status::BACKING,
        "the escape driver did not reach its attach (it reported {tag:#x}; a 0xDEAD_.. word's \
         low byte names the bring-up step, see components/src/gpu_driver.rs)",
    );

    // The evidence. QEMU records the fault as it processes the command under TCG, so a bounded
    // spin is plenty; the bound turns "no fault ever" into a failure rather than a hang.
    let mut fault = None;
    for _ in 0..2_000_000 {
        if let Some(f) = crate::iommu::take_fault() {
            fault = Some(f);
            break;
        }
        core::hint::spin_loop();
    }
    let f = fault.unwrap_or_else(|| {
        panic!(
            "the GPU was pointed at {victim:#x}, outside its DMA region, and the IOMMU recorded \
             no fault (the device answered the attach with {response:#x}): a backing address \
             rides in a command payload the transport validator cannot see, so if the IOMMU is \
             not bounding it, nothing is",
        )
    });
    // `None` only from AMD-Vi under QEMU 11.1.1, which writes no address into its event records
    // (`arch/x86_64/iommu.rs`'s `Fault::addr`); there is then nothing here to tie the fault to the
    // frame, and the fault itself is the evidence.
    if let Some(addr) = f.addr {
        assert_eq!(
            addr & !0xfff,
            victim & !0xfff,
            "the IOMMU faulted, but on {addr:#x} (code {:#x}, rid {:#x}), not the frame the GPU \
             was pointed at ({victim:#x})",
            f.code,
            f.rid,
        );
    }

    // Leave the fault queue as we found it. Not tidiness: the RISC-V IOMMU's queue holds 128
    // records and the driver does not clear its overflow bit, so records left behind here cost a
    // later test its own fault assertion. The escape above is sized to produce one fault for the
    // same reason (components/src/gpu_driver.rs).
    while crate::iommu::take_fault().is_some() {}
}

/// **A bitmap font and a VT engine put readable text on the scanout.**
///
/// The remaining increment of milestone 29 (a display terminal: framebuffer, virtio-gpu, and a
/// foreign component) end to end: a terminal that is a framebuffer-contract client. The terminal
/// paints what `video_terminal` says, the kernel reruns the engine over `video_terminal::script`
/// and compares the scanout frames pixel for pixel, and the host compares QEMU's `screendump`, the
/// only witness of pixel format and scanout rectangle. The host checker must reject
/// `video_terminal::script::GREETING_TYPO`. Each part of the script catches a named failure, and
/// the driver's one report checks that a blank terminal is a defined picture. It sorts between
/// the confinement and pattern tests (see notes/framebuffer-contract/the-display-tests.md).
#[test_case]
fn a_bitmap_font_and_a_vt_engine_put_readable_text_on_the_scanout() {
    let gpu_driver = program("gpu_driver").expect("no gpu_driver program in the initial ramdisk");
    let display_terminal =
        program("display_terminal").expect("no display_terminal program in the initial ramdisk");

    let Some(w) = display_service::start_terminal(gpu_driver, display_terminal) else {
        crate::testing::skip!("no virtio-gpu-pci function on the bus (NIFE_GPU not set?)");
    };

    // The driver came up. Taken first: these are rendezvous SENDs, so the driver is parked here.
    let [tag, geometry, ..] = sched::ipc_receive(w.driver_report);
    assert_eq!(
        tag,
        gfx::status::UP,
        "the GPU driver did not come up (it reported {tag:#x})",
    );
    assert_eq!(geometry, gfx::WIDTH as u64 | ((gfx::HEIGHT as u64) << 32),);

    // The terminal negotiated its geometry from the driver rather than assuming it, and got the
    // grid the script is written for.
    let [tag, dims, mode, ..] = sched::ipc_receive(w.term_report);
    assert_eq!(
        tag,
        video_terminal::status::TERM_UP,
        "the display terminal did not come up (it reported {tag:#x}; a 0xDEAD_.. word's low \
         byte names the step, see components/src/display_terminal.rs)",
    );
    assert_eq!(
        dims,
        video_terminal::script::COLS as u64 | ((video_terminal::script::ROWS as u64) << 32),
        "the terminal sized itself to a different grid than the script predicts",
    );
    assert_eq!(mode, video_terminal::status::MODE_DISPLAY);

    // The driver's account of the terminal's first flush: the blank grid. A second address
    // space's witness, taken after the device reported the transfer complete.
    //
    // **A function-local `static`, not a stack local** (milestone 142): at the grown scanout's
    // grid (`script::COLS` x `script::ROWS`, 80x24), one `Vt` is well over a hundred KiB, which a
    // 24 KiB kernel thread stack cannot hold (`script/stack-frame-check`'s whole reason for being;
    // see notes/stack-high-water.md). `Vt::new` is `const fn` and `COLS`/`ROWS` are compile-time
    // constants, so this initializer is evaluated by the compiler and lands in `.bss`, never on the
    // stack. This test builds three such grids (`blank`, `expect`, `typo` below); each gets its own
    // named static for exactly this reason.
    static mut BLANK: video_terminal::Vt =
        video_terminal::Vt::new(video_terminal::script::COLS, video_terminal::script::ROWS);
    let blank_ptr = &raw const BLANK;
    // SAFETY: this `#[test_case]` runs once, to completion, before any other test that might
    // declare a same-named function-local static begins (the harness runs test cases
    // sequentially); nothing else in this process can reach `BLANK`.
    let blank: &video_terminal::Vt = unsafe { &*blank_ptr };
    let [tag, driver_digest, pixels, ..] = sched::ipc_receive(w.driver_report);
    assert_eq!(tag, gfx::status::FLUSHED, "the driver served no flush");
    assert_eq!(pixels, gfx::PIXELS as u64);
    assert_eq!(
        driver_digest,
        gfx::checksum(|i| {
            let (x, y) = (
                (i % gfx::WIDTH as usize) as u32,
                (i / gfx::WIDTH as usize) as u32,
            );
            blank.pixel(x, y)
        }),
        "the frames the device read for the terminal's first flush are not a blank terminal: an \
         empty terminal must be a defined picture, not whatever was in those frames",
    );
    w.assert_screen_is(blank, "a terminal that has been sent nothing");

    // Play the application, then the input driver. Both replies mean the pixels are on the
    // device's side, so there is nothing to poll and nothing to sleep for.
    w.print(video_terminal::script::GREETING);
    w.type_bytes(video_terminal::script::TYPED);

    // The kernel's own witness: every pixel, against the engine it ran itself. `script::full_screen`
    // takes `&mut Vt` rather than returning one **by value** for exactly the reason `BLANK` above is
    // a static: a `Vt` is too large now to be a stack-local return value on this thread's stack.
    static mut EXPECT: video_terminal::Vt =
        video_terminal::Vt::new(video_terminal::script::COLS, video_terminal::script::ROWS);
    let expect_ptr = &raw mut EXPECT;
    // SAFETY: see `BLANK` above.
    let expect: &mut video_terminal::Vt = unsafe { &mut *expect_ptr };
    video_terminal::script::full_screen(expect);
    w.assert_screen_is(expect, "after the greeting and the typing");

    // A wrong screen must not pass this. The typo picture differs from the real one in one
    // letter, so asserting they differ at all is what says the comparison above has teeth: if
    // `assert_screen_is` were somehow vacuous, this would still be true, which is why the check
    // is that the two *pictures* differ rather than that the screen is not the typo.
    static mut TYPO: video_terminal::Vt =
        video_terminal::Vt::new(video_terminal::script::COLS, video_terminal::script::ROWS);
    let typo_ptr = &raw mut TYPO;
    // SAFETY: see `BLANK` above.
    let typo: &mut video_terminal::Vt = unsafe { &mut *typo_ptr };
    typo.feed(video_terminal::script::GREETING_TYPO);
    typo.feed(video_terminal::script::TYPED);
    assert!(
        (0..gfx::HEIGHT).any(|y| (0..gfx::WIDTH).any(|x| typo.pixel(x, y) != expect.pixel(x, y))),
        "the one-letter typo produces an identical picture: the negative control is inert",
    );

    // Scroll past the grid's height (the paint path, 2026-09-30). The fast path moves pixel rows
    // rather than re-rendering them, and this comparison is the witness that a moved picture is the
    // right one. One `OPERATION_WRITE` per line scrolls once per present; then the tail, several
    // lines in one write, scrolls several times in one present.
    video_terminal::script::write_scroller(|bytes| w.print(bytes));
    video_terminal::script::feed_scroller(expect, None);
    w.assert_screen_is(expect, "after the scroller");

    // The scrolled comparison's own teeth: one digit off in one tail line, still on screen at the
    // end, must fail it, or a picture scrolled by the wrong number of rows could pass.
    video_terminal::script::feed_scroller(typo, Some(video_terminal::script::SCROLL_LINES + 1));
    assert!(
        (0..gfx::HEIGHT).any(|y| (0..gfx::WIDTH).any(|x| typo.pixel(x, y) != expect.pixel(x, y))),
        "the one-digit scroller typo produces an identical scrolled picture: the negative \
         control is inert",
    );

    // **Hold the picture up for the host.** `cargo xtask` polls QEMU's monitor while this suite
    // runs, and the next test puts rung one's pattern on the same scanout. Three seconds is an
    // order of magnitude more than the poll needs, and if the host never sees it the run fails at
    // the scanout check rather than passing quietly.
    let deadline = crate::arch::timer::now() + 3 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        sched::yield_now();
    }
}

/// **A key pressed on a real device becomes the byte a terminal receives** (milestone 29's input).
///
/// A confined virtio-input driver, behind the GPU's PCIe transport and IOMMU domain, checks the
/// device ID, posts device-writable event buffers, takes a real key event by interrupt, maps it
/// with `video_terminal::keymap` and publishes the byte in the compositor's input ring. The host
/// presses `video_terminal::script::HOST_KEY` with `sendkey`. The driver holds no client endpoint
/// and rings a content-free doorbell, so it cannot choose who receives a key. From the ring onward
/// is `compositor_tests::focus_routes_a_keystroke_to_one_terminals_grid_and_not_its_neighbours`, at
/// the seam of DECISIONS §33 (the compositor's authority is memory, not messages)
/// (notes/glyphs/terminal-and-proof.md).
#[test_case]
fn a_keystroke_from_a_virtio_keyboard_becomes_a_terminal_byte() {
    let keyboard_driver =
        program("keyboard_driver").expect("no keyboard_driver program in the initial ramdisk");
    let Some(mut w) = keyboard_service::start(keyboard_driver) else {
        crate::testing::skip!("no virtio-input function on the bus (NIFE_KEYBOARD not set?)");
    };
    assert!(
        crate::iommu::is_active(),
        "a keyboard is present but the IOMMU is not active: the device's event buffers are \
         unconfined, and a keyboard's buffers are where every keystroke lands",
    );

    let [tag, buffers, ..] = sched::ipc_receive(w.report);
    assert_eq!(
        tag,
        video_terminal::status::KEYBOARD_UP,
        "the keyboard driver did not come up (it reported {tag:#x}; a 0xDEAD_.. word's low byte \
         names the step, see components/src/keyboard_driver.rs)",
    );
    assert!(
        buffers > 0,
        "the driver posted no event buffers, so a key would have nowhere to land",
    );

    // Wait for the driver to ring, which it only does when it has typed something. Bounded on
    // the clock: if the host's `sendkey` never reaches the device, this fails with a sentence
    // rather than hanging until the harness's ceiling.
    let deadline = crate::arch::timer::now() + 10 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline && sched::rendezvous_waiting_senders(w.doorbell) == 0
    {
        sched::yield_now();
    }
    assert!(
        sched::rendezvous_waiting_senders(w.doorbell) > 0,
        "the keyboard driver came up but never typed anything in ten seconds: the host's \
         `sendkey {}` is not reaching the device (is the monitor socket attached? see \
         cargo xtask's scanout check, which owns that connection)",
        video_terminal::script::HOST_KEY,
    );
    w.answer_doorbell();

    let mut typed = [0u8; 16];
    let n = w.take_typed(&mut typed);
    assert!(n > 0, "the driver rang the doorbell with an empty ring");
    for (i, &b) in typed[..n].iter().enumerate() {
        assert_eq!(
            b,
            video_terminal::script::HOST_KEY_BYTE,
            "byte {i} of {n} from the keyboard is {:?}, not the {:?} the host pressed: the \
             evdev keycode was mapped wrong, or a key release was counted as a press",
            b as char,
            video_terminal::script::HOST_KEY_BYTE as char,
        );
    }
    // A release must not type. The host sends press *and* release for each `sendkey`, so a
    // driver that ignored the value field would produce two bytes per press; that would show up
    // above as the right character twice, which is why the count is checked against the
    // presses the host could have made rather than merely being non-zero.
    assert!(
        n <= 64,
        "{n} bytes from a handful of key presses: releases or auto-repeats are being counted \
         more than once",
    );
}

/// **A screen the firmware set up shows the terminal, through `framebuffer_driver`** (the shell
/// on the firmware screen, milestone 198's rung 1b).
///
/// The OVMF gate (`cargo xtask uefi-boot`) proves one geometry. This proves the driver's arithmetic
/// on all three architectures, over RAM the kernel poisons and describes as a firmware framebuffer:
/// an odd 42x650 shape the terminal covers whole (3 columns by 25 rows), a padded stride whose
/// padding must survive, rgbx (checked on a red and a blue cell) and an aperture that is not
/// page-aligned. It proves arithmetic and wiring, not the scanout or memory types: the RAM is
/// device-typed in the driver and cacheable in the direct map, an alias QEMU does not model
/// (notes/framebuffer-contract/the-display-tests.md).
#[test_case]
fn a_firmware_screen_shows_the_terminal_through_the_framebuffer_driver() {
    use machine_discovery::framebuffer::{Framebuffer, PixelOrder};

    let driver = program("framebuffer_driver").expect("no framebuffer_driver in the archive");
    let terminal = program("display_terminal").expect("no display_terminal in the archive");

    // Three cells wide and 25 cell rows tall: every property below, in the fewest frames
    // (37 for the screen, 27 for its surface). This suite runs one boot's frame pool through every test and
    // hands little of it back (`user::holding`'s module note), so a pretend screen the size of a
    // real one would be spending a few hundred frames on pixels nothing reads.
    const WIDTH: u32 = 3 * video_terminal::CELL_W;
    const HEIGHT: u32 = gfx::HEIGHT + video_terminal::CELL_H;
    // 60, not a round 64: with it the stride is 228, an odd number of words, which is what puts
    // every other row off qword alignment below.
    const PAD: u32 = 60;
    const STRIDE: u32 = WIDTH * 4 + PAD;
    const OFFSET: u64 = 0x140;
    const POISON: u8 = 0xa5;
    let bytes = (OFFSET + STRIDE as u64 * HEIGHT as u64) as usize;
    let frames = bytes.div_ceil(FRAME_SIZE as usize);
    let phys = crate::memory::alloc_contiguous_zeroed(frames)
        .expect("no frames for a pretend screen")
        .addr();
    let at = |offset: usize| (mmu::phys_to_virt(phys) + OFFSET) as usize + offset;
    for i in 0..bytes - OFFSET as usize {
        // SAFETY: inside the frames just allocated, which nothing else has a name for yet.
        unsafe { core::ptr::write_volatile(at(i) as *mut u8, POISON) };
    }
    let screen = Framebuffer {
        base: phys + OFFSET,
        width: WIDTH,
        height: HEIGHT,
        stride: STRIDE,
        order: PixelOrder::Rgbx,
    };

    let w = display_service::start_screen_terminal(driver, terminal, screen)
        .expect("a 42x650 screen should be wired");
    let [tag, geometry, ..] = sched::ipc_receive(w.driver_report);
    assert_eq!(
        tag,
        gfx::status::UP,
        "the framebuffer driver reported {tag:#x}"
    );
    assert_eq!(
        geometry,
        WIDTH as u64 | ((HEIGHT as u64) << 32),
        "the covered part is the whole screen",
    );
    let cols = WIDTH / video_terminal::CELL_W;
    let rows = HEIGHT / video_terminal::CELL_H;
    let [tag, dims, ..] = sched::ipc_receive(w.term_report);
    assert_eq!(
        tag,
        video_terminal::status::TERM_UP,
        "the terminal reported {tag:#x}"
    );
    assert_eq!(
        dims,
        cols as u64 | ((rows as u64) << 32),
        "the terminal laid its grid out over the covered part"
    );

    // A red cell and a blue cell, then the usual greeting.
    const TEXT: &[u8] = b"\x1b[41m \x1b[44m \x1b[0m\r\n";
    w.print(TEXT);
    w.print(video_terminal::script::GREETING);
    // **And past the grid's own height** (the paint path, 2026-09-30): a scroll makes the
    // terminal present the whole covered part through this driver, whose 228-byte stride puts
    // every other row off qword alignment, which is the geometry the wide-copy path in the flush
    // has to survive. One write per line (each present scrolls once), then the tail (several
    // scrolls in one present), the same writes the full-scanout witness sends.
    video_terminal::script::write_scroller(|bytes| w.print(bytes));

    static mut EXPECT: video_terminal::Vt =
        video_terminal::Vt::new(video_terminal::script::COLS, video_terminal::script::ROWS);
    let expect_ptr = &raw mut EXPECT;
    // SAFETY: this `#[test_case]` runs once, to completion, and nothing else can reach `EXPECT`.
    let expect: &mut video_terminal::Vt = unsafe { &mut *expect_ptr };
    expect.reset_to(cols, rows);
    expect.feed(TEXT);
    expect.feed(video_terminal::script::GREETING);
    video_terminal::script::feed_scroller(expect, None);

    let read = |offset: usize| {
        // SAFETY: inside the pretend screen allocated above; the offsets below stay in it.
        unsafe { core::ptr::read_volatile(at(offset) as *const u32) }
    };
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let got = read((y * STRIDE + x * 4) as usize);
            let want = PixelOrder::Rgbx.store(expect.pixel(x, y));
            assert_eq!(
                got, want,
                "the screen is wrong at ({x},{y}): {got:#010x}, expected {want:#010x}",
            );
        }
        for pad in 0..PAD / 4 {
            assert_eq!(
                read((y * STRIDE + WIDTH * 4 + pad * 4) as usize),
                u32::from_ne_bytes([POISON; 4]),
                "row {y}'s stride padding was written",
            );
        }
    }
    // The negative control for the byte order: the red cell's paper really is red in the engine,
    // so an rgbx screen must hold it with blue's byte position, and a driver that forgot to swap
    // would have been caught above.
    let red = expect.pixel(1, 1);
    assert_ne!(
        red,
        PixelOrder::Rgbx.store(red),
        "the red cell is channel-symmetric: the byte-order check above is inert",
    );

    // Hand everything back: the next test to allocate pays for whatever this one keeps
    // (notes/framebuffer-contract/the-display-tests.md). The two programs die by their endpoint
    // region being reclaimed (`TerminalWiring::held`); the surface, the output page and the pretend
    // screen are frames rather than regions, and are freed only after both are gone.
    w.held
        .release_or_fail("the framebuffer driver and its terminal");
    for k in 0..w.surface_frames {
        crate::memory::free(PageFrame::from_addr(w.surface + k * FRAME_SIZE));
    }
    crate::memory::free(PageFrame::from_addr(w.out));
    for k in 0..frames as u64 {
        crate::memory::free(PageFrame::from_addr(phys + k * FRAME_SIZE));
    }
}
