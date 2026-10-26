//! **The USB keyboard driver** (milestone 242 (USB host and HID, because on commodity hardware the
//! keyboard is not a UART); notes/usb.md): a confined userspace xHCI driver that finds a boot
//! keyboard on a root port and turns its reports into the bytes a terminal understands.
//! ```text
//!   a USB keyboard ──xHCI (PCIe, behind the IOMMU)──► usb_keyboard_driver ──OPERATION_BYTES, CALL──► line_editor
//! ```
//! It is milestone 192 (a keyboard on real silicon)'s option B: a third keystroke source beside
//! `input` (the UART) and `keyboard_driver` (virtio-input), in the same `OPERATION_BYTES` framing
//! to the same terminal endpoint, so nothing downstream can tell which source a key came from.
//! What it does, in order, is notes/usb.md, "What the driver does, in order".

//! # Its whole authority
//!
//! - slot 0, the controller's **interrupt** (`Irq`);
//! - slot 1, an **attach** endpoint (READ): one message ever arrives, carrying `WRITE` on the
//!   terminal endpoint from the progenitor, the only destination this program can name;
//! - slot 2, a **report** endpoint (WRITE): one bring-up report to the kernel;
//! - mapped: the xHCI register file **less the MSI-X pages**, and the eleven work pages of its DMA
//!   region. The IOMMU confines the controller to the region and any firmware-reserved region (an
//!   RMRR) VT-d requires mapped, so an address written into a ring reaches nothing else.
//! - denied: the matching list is in `kernel/src/user/usb_keyboard_service.rs`.

//! # BUGS
//!
//! - **Root ports only.** A keyboard behind a hub, or with one inside it, is not found; the hub
//!   is refused as not a keyboard.
//! - **One keyboard.** The first found is served; a second is ignored until the first is unplugged.
//! - **No key repeat, no lock keys, no LEDs** (`crates/usb`'s BUGS): holding a key types it once.
//! - **Bring-up polls the event ring**, with bounds from the specification (notes/usb.md).
//! - **Only port changes survive a command wait**; a key pressed meanwhile is lost (notes/usb.md).
//!
//! Name: ratified 2026-10-17 (calef, a program-naming session). Kept, with the acronym, because
//! "USB" is the everyday word and the expansion adds nothing a newcomer can use. He weighed
//! `universal_serial_bus_keyboard_driver`, then answered "`usb_keyboard_driver`". Refused
//! `universal_serial_bus_keyboard_driver` (the expansion). It is the `<device>_driver` shape of
//! `keyboard_driver` and `block_driver`; the controller crate's name
//! (`extensible_host_controller_interface`, 36 bytes) does not fit `nifefs`'s 32-byte program name,
//! and the program is for a keyboard rather than for the controller in general.

#![no_std]
// A program entry point, not the crates/ library surface milestone 68 (code-quality gates: one lint
// policy)'s ratchet tracks (DECISIONS §107 (`missing_docs` moves to `workspace.lints.rust`)).
#![allow(missing_docs)]
#![no_main]

use extensible_host_controller_interface::{
    Capabilities, Event, EventRing, PAGE_BYTES, ProducerRing, TransferRequestBlock,
    TransferRequestBlockPlacement, completion, context, dma, port, regs,
};
use extensible_host_controller_interface_contract::Handoff;
use line_editor::proto;
use usb::boot_keyboard::{BootReport, REPORT_LEN, changes};
use usb::descriptor::{self, ConfigurationHeader, DeviceDescriptor};
use usb::request::SetupPacket;
use usb_keyboard_report::{self as report, BringUpStep};
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{
    call, exit, interrupt_acknowledge, interrupt_wait, monotonic_nanos, receive_cap, send,
};
use video_terminal::keymap::{EV_KEY, Keyboard};

/// Capability slots, by convention with `kernel/src/user/usb_keyboard_service.rs`.
const IRQ: u64 = 0;
const ATTACH: u64 = 1;
const REPORT: u64 = 2;

/// Where the kernel maps register page `n` (at `REGS_VA + n * 4096`) and the DMA work pages.
/// **Must match `kernel/src/user/usb_keyboard_service.rs`.**
const REGS_VA: u64 = address_space_map::pair_page(0x0000_0000_0100_0000);
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0200_0000);

// SAFETY: the kernel maps the register pages its window chose at REGS_VA before `_start` runs;
// this program reads only the capability, operational, port, interrupter-0 and doorbell registers,
// which `register_window` refused to start it without.
const REGS: MappedWindow = unsafe {
    MappedWindow::new(
        REGS_VA,
        extensible_host_controller_interface_contract::MAX_REGISTER_PAGES * PAGE_BYTES,
    )
};
// SAFETY: the kernel maps the WORK_PAGES pages read/write at DMA_VA before `_start` runs.
const DMA: MappedWindow = unsafe {
    MappedWindow::new(
        DMA_VA,
        extensible_host_controller_interface_contract::WORK_PAGES * PAGE_BYTES,
    )
};
// SAFETY: the control buffer page, inside the DMA window above.
const CONTROL_BUFFER: MappedWindow =
    unsafe { MappedWindow::new(DMA_VA + dma::CONTROL_BUFFER * PAGE_BYTES, PAGE_BYTES) };

/// One millisecond, in the units [`monotonic_nanos`] counts.
const MS: u64 = 1_000_000;

/// **Order ring writes before the doorbell, and a cycle bit's read before the payload it gates.**
/// The NVMe server's barrier and for its reason (`components/src/non_volatile_memory_express.rs`
/// says why it is local rather than shared): the doorbell is a direct store to a device page.
fn barrier() {
    // SAFETY: a barrier has no operands and cannot be unsound; it only constrains ordering.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("dmb sy", options(nostack, nomem, preserves_flags));
    }
    // SAFETY: as above.
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!("fence", options(nostack, preserves_flags));
    }
    // SAFETY: as above.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!("mfence", options(nostack, nomem, preserves_flags));
    }
    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "x86_64"
    )))]
    compile_error!("barrier(): name the instruction that orders a ring write before a doorbell");
}

/// A failure: where, and one word of detail.
type Fail = (BringUpStep, u32);

/// The keyboard being served.
#[derive(Clone, Copy)]
struct Attached {
    slot: u8,
    port: u8,
    dci: u8,
}

/// **The driver's whole state.**
struct Host {
    capabilities: Capabilities,
    dma_phys: u64,
    commands: ProducerRing,
    events: EventRing,
    control: ProducerRing,
    interrupt: ProducerRing,
    /// A port change event arrived and has not been looked at.
    ports_changed: bool,
    keyboard: Option<Attached>,
    /// The served keyboard's vendor, product and speed, for the report.
    found: Option<(u16, u16, u8)>,
    last: BootReport,
    keys: Keyboard,
}

fn deadline(ms: u64) -> u64 {
    monotonic_nanos().saturating_add(ms * MS)
}

fn past(deadline: u64) -> bool {
    monotonic_nanos() > deadline
}

fn zero_page(page: u64) {
    for off in (0..PAGE_BYTES).step_by(8) {
        DMA.w64(page * PAGE_BYTES + off, 0);
    }
}

impl Host {
    fn reg(&self, off: u64) -> u32 {
        REGS.r32(off)
    }

    fn set_reg(&self, off: u64, v: u32) {
        REGS.w32(off, v);
    }

    /// A 64-bit register as two aligned dwords, low first, which xHCI 1.2 section 5.1 allows.
    fn set_reg64(&self, off: u64, v: u64) {
        REGS.w32(off, v as u32);
        REGS.w32(off + 4, (v >> 32) as u32);
    }

    fn op(&self, off: u64) -> u64 {
        self.capabilities.operational + off
    }

    fn phys(&self, page: u64) -> u64 {
        self.dma_phys + page * PAGE_BYTES
    }

    fn ring_doorbell(&self, slot: u8, target: u32) {
        barrier();
        self.set_reg(self.capabilities.doorbell(slot), target);
    }

    /// Write one TRB, the cycle dword last.
    fn write_trb(page: u64, index: u16, t: TransferRequestBlock) {
        let at = page * PAGE_BYTES + u64::from(index) * 16;
        DMA.w32(at, t.0[0]);
        DMA.w32(at + 4, t.0[1]);
        DMA.w32(at + 8, t.0[2]);
        barrier();
        DMA.w32(at + 12, t.0[3]);
    }

    fn read_trb(page: u64, index: u16) -> TransferRequestBlock {
        let at = page * PAGE_BYTES + u64::from(index) * 16;
        let d3 = DMA.r32(at + 12);
        barrier();
        TransferRequestBlock([DMA.r32(at), DMA.r32(at + 4), DMA.r32(at + 8), d3])
    }

    fn put(page: u64, p: TransferRequestBlockPlacement) {
        Host::write_trb(page, p.index, p.trb);
        if let Some((at, link)) = p.link {
            Host::write_trb(page, at, link);
        }
    }

    /// Take the next event off the ring, if the controller has written one, and tell it so.
    fn poll_event(&mut self) -> Option<Event> {
        let t = Host::read_trb(dma::EVENT_RING, self.events.index());
        if !self.events.is_ready(&t) {
            return None;
        }
        self.events.advance();
        self.set_reg64(
            self.capabilities.interrupter(regs::ERDP),
            self.events.dequeue_pointer() | regs::ERDP_BUSY,
        );
        Some(Event::decode(t))
    }

    /// Poll for an event `wanted` accepts, remembering port changes and dropping the rest.
    fn wait_for(&mut self, ms: u64, wanted: &dyn Fn(&Event) -> bool) -> Option<Event> {
        let end = deadline(ms);
        loop {
            if let Some(e) = self.poll_event() {
                if wanted(&e) {
                    return Some(e);
                }
                if matches!(e, Event::PortStatus { .. }) {
                    self.ports_changed = true;
                }
                continue;
            }
            if past(end) {
                return None;
            }
            core::hint::spin_loop();
        }
    }

    /// **One command, to its completion.** The slot the completion names, on success.
    fn command(&mut self, trb: TransferRequestBlock) -> Result<u8, Fail> {
        let p = self.commands.enqueue(trb);
        let at = self.commands.address_of(p.index);
        Host::put(dma::COMMAND_RING, p);
        self.ring_doorbell(0, 0);
        match self.wait_for(
            1000,
            &|e| matches!(e, Event::Command { trb, .. } if *trb == at),
        ) {
            Some(Event::Command { code, slot, .. }) if code == completion::SUCCESS => Ok(slot),
            Some(Event::Command { code, .. }) => Err((BringUpStep::CommandFailed, u32::from(code))),
            _ => Err((BringUpStep::CommandTimeout, trb.kind() as u32)),
        }
    }

    /// **One control transfer on endpoint 0 of `slot`**, data into the control buffer. The number
    /// of bytes the data stage moved, on success.
    fn transfer(&mut self, slot: u8, setup: SetupPacket) -> Result<usize, Fail> {
        let len = setup.length.min(PAGE_BYTES as u16);
        let inbound = setup.is_device_to_host();
        let data_in = (len > 0).then_some(inbound);
        let p = self
            .control
            .enqueue(TransferRequestBlock::setup(setup.to_u64(), data_in));
        Host::put(dma::CONTROL_RING, p);
        let mut data_at = None;
        if len > 0 {
            let buffer = self.phys(dma::CONTROL_BUFFER);
            let p = self
                .control
                .enqueue(TransferRequestBlock::data(buffer, len, inbound));
            data_at = Some(self.control.address_of(p.index));
            Host::put(dma::CONTROL_RING, p);
        }
        let p = self
            .control
            .enqueue(TransferRequestBlock::status(len > 0 && inbound));
        let status_at = self.control.address_of(p.index);
        Host::put(dma::CONTROL_RING, p);
        self.ring_doorbell(slot, 1);

        let mut moved = usize::from(len);
        loop {
            let e = self.wait_for(
                1000,
                &|e| matches!(e, Event::Transfer { slot: s, endpoint: 1, .. } if *s == slot),
            );
            let Some(Event::Transfer {
                trb,
                residual,
                code,
                ..
            }) = e
            else {
                return Err((BringUpStep::TransferTimeout, u32::from(setup.request)));
            };
            match code {
                completion::SUCCESS | completion::SHORT_PACKET if trb == status_at => {
                    barrier();
                    return Ok(moved);
                }
                completion::SUCCESS | completion::SHORT_PACKET if Some(trb) == data_at => {
                    moved = usize::from(len).saturating_sub(residual as usize);
                }
                completion::SUCCESS | completion::SHORT_PACKET => {}
                code => {
                    // A stall halts the endpoint; until it is reset and its ring moved past what
                    // the failed transfer left, nothing else on endpoint 0 runs.
                    self.recover(slot, 1, true)?;
                    return Err((BringUpStep::TransferFailed, u32::from(code)));
                }
            }
        }
    }

    /// Reset a halted endpoint and point it at the ring's next free entry.
    fn recover(&mut self, slot: u8, dci: u8, control: bool) -> Result<(), Fail> {
        self.command(TransferRequestBlock::reset_endpoint(slot, dci))?;
        let pointer = if control {
            self.control.dequeue_pointer()
        } else {
            self.interrupt.dequeue_pointer()
        };
        self.command(TransferRequestBlock::set_dequeue(pointer, slot, dci))?;
        Ok(())
    }

    fn input_context(&self, index: u8, words: [u32; 8]) {
        let at = dma::INPUT_CONTEXT * PAGE_BYTES
            + context::input_offset(index, self.capabilities.context_bytes);
        for (i, w) in words.iter().enumerate() {
            DMA.w32(at + 4 * i as u64, *w);
        }
    }

    /// **Bring one connected root port's device up to a served keyboard**, or say why not.
    fn attach(&mut self, port_number: u8) -> Result<Attached, Fail> {
        let portsc = self.capabilities.portsc(port_number);
        let sc = self.reg(portsc);
        // Acknowledge every change pending, so a stale connect change does not bring us back.
        self.set_reg(portsc, port::write_value(sc, sc & port::CHANGES));
        if sc & port::ENABLED == 0 {
            self.set_reg(portsc, port::write_value(sc, port::RESET));
            let end = deadline(500);
            while self.reg(portsc) & port::RESET_CHANGE == 0 {
                if past(end) {
                    return Err((BringUpStep::PortResetTimeout, port_number.into()));
                }
                core::hint::spin_loop();
            }
            let sc = self.reg(portsc);
            self.set_reg(portsc, port::write_value(sc, port::RESET_CHANGE));
            if sc & port::ENABLED == 0 {
                return Err((BringUpStep::PortNotEnabled, sc));
            }
        }
        let speed = port::speed_of(self.reg(portsc));
        let Some(packet0) = port::default_control_packet(speed) else {
            return Err((BringUpStep::UnknownSpeed, speed.into()));
        };
        let slot = self.command(TransferRequestBlock::enable_slot())?;
        match self.configure(slot, port_number, speed, packet0) {
            Ok(a) => Ok(a),
            Err(f) => {
                let _ = self.command(TransferRequestBlock::disable_slot(slot));
                Err(f)
            }
        }
    }

    fn configure(
        &mut self,
        slot: u8,
        port_number: u8,
        speed: u8,
        packet0: u16,
    ) -> Result<Attached, Fail> {
        // Fresh rings and contexts for this device: a stale TRB carrying the new lap's cycle bit
        // would be run by the controller as if it had just been written.
        for page in [
            dma::DEVICE_CONTEXT,
            dma::CONTROL_RING,
            dma::INTERRUPT_RING,
            dma::INPUT_CONTEXT,
        ] {
            zero_page(page);
        }
        self.control = ProducerRing::new(self.phys(dma::CONTROL_RING), dma::RING_ENTRIES);
        self.interrupt = ProducerRing::new(self.phys(dma::INTERRUPT_RING), dma::RING_ENTRIES);
        DMA.w64(
            dma::DEVICE_CONTEXT_ARRAY * PAGE_BYTES + 8 * u64::from(slot),
            self.phys(dma::DEVICE_CONTEXT),
        );

        // Address Device: the slot context and endpoint 0.
        self.input_context(0, context::input_control(0b11, 0));
        self.input_context(1, context::slot(speed, port_number, 1));
        self.input_context(
            2,
            context::control_endpoint(packet0, self.control.dequeue_pointer()),
        );
        self.command(TransferRequestBlock::address_device(
            self.phys(dma::INPUT_CONTEXT),
            slot,
        ))?;

        // The first eight bytes say endpoint 0's real packet size.
        let n = self.transfer(slot, SetupPacket::get_descriptor(descriptor::DEVICE, 0, 8))?;
        if n < 8 {
            return Err((BringUpStep::BadDeviceDescriptor, n as u32));
        }
        let b0 = CONTROL_BUFFER.r8(7);
        let Some(packet) = usb::descriptor::control_max_packet(b0, context::is_superspeed(speed))
        else {
            return Err((BringUpStep::BadControlPacket, b0.into()));
        };
        if packet != packet0 {
            zero_page(dma::INPUT_CONTEXT);
            self.input_context(0, context::input_control(0b10, 0));
            self.input_context(
                2,
                context::control_endpoint(packet, self.control.dequeue_pointer()),
            );
            self.command(TransferRequestBlock::evaluate_context(
                self.phys(dma::INPUT_CONTEXT),
                slot,
            ))?;
        }

        let n = self.transfer(
            slot,
            SetupPacket::get_descriptor(descriptor::DEVICE, 0, descriptor::DEVICE_LEN as u16),
        )?;
        // SAFETY: the controller finished writing the buffer before the status stage's completion,
        // which `transfer` waited for and fenced; nothing writes it until the next transfer.
        let device = DeviceDescriptor::parse(&unsafe { CONTROL_BUFFER.as_slice() }[..n])
            .ok_or((BringUpStep::BadDeviceDescriptor, n as u32))?;

        let n = self.transfer(
            slot,
            SetupPacket::get_descriptor(
                descriptor::CONFIGURATION,
                0,
                descriptor::CONFIGURATION_LEN as u16,
            ),
        )?;
        // SAFETY: as above.
        let header = ConfigurationHeader::parse(&unsafe { CONTROL_BUFFER.as_slice() }[..n])
            .map_err(|r| (BringUpStep::NotABootKeyboard, refusal_code(r)))?;
        let total = header.total_length.min(PAGE_BYTES as u16);
        let n = self.transfer(
            slot,
            SetupPacket::get_descriptor(descriptor::CONFIGURATION, 0, total),
        )?;
        // SAFETY: as above.
        let keyboard =
            usb::descriptor::find_boot_keyboard(&unsafe { CONTROL_BUFFER.as_slice() }[..n])
                .map_err(|r| (BringUpStep::NotABootKeyboard, refusal_code(r)))?;

        self.transfer(slot, SetupPacket::set_configuration(keyboard.configuration))?;
        // Both tolerated when stalled: some keyboards refuse SET_PROTOCOL and report boot anyway,
        // and SET_IDLE is optional for a boot device (HID 1.11 section 7.2.4).
        let _ = self.transfer(slot, SetupPacket::set_boot_protocol(keyboard.interface));
        let _ = self.transfer(slot, SetupPacket::set_idle_forever(keyboard.interface));

        // Configure Endpoint: the interrupt IN endpoint, its ring, its interval.
        let dci = context::endpoint_index(keyboard.endpoint);
        zero_page(dma::INPUT_CONTEXT);
        self.input_context(0, context::input_control(1 | 1 << dci, 0));
        self.input_context(1, context::slot(speed, port_number, dci));
        self.input_context(
            dci + 1,
            context::endpoint(
                context::INTERRUPT_IN,
                keyboard.max_packet,
                extensible_host_controller_interface::interrupt_interval(speed, keyboard.interval),
                self.interrupt.dequeue_pointer(),
                REPORT_LEN as u16,
            ),
        );
        self.command(TransferRequestBlock::configure_endpoint(
            self.phys(dma::INPUT_CONTEXT),
            slot,
        ))?;

        let attached = Attached {
            slot,
            port: port_number,
            dci,
        };
        for i in 0..dma::REPORTS_QUEUED {
            self.queue_report(
                attached,
                self.phys(dma::REPORT_BUFFERS) + u64::from(i) * dma::REPORT_SLOT,
            );
        }
        self.last = BootReport::RELEASED;
        self.found = Some((device.vendor, device.product, speed));
        Ok(attached)
    }

    fn queue_report(&mut self, k: Attached, buffer: u64) {
        let p = self
            .interrupt
            .enqueue(TransferRequestBlock::normal(buffer, REPORT_LEN as u16));
        Host::put(dma::INTERRUPT_RING, p);
        self.ring_doorbell(k.slot, u32::from(k.dci));
    }
}

/// A `usb::Refusal` as one report word.
fn refusal_code(r: usb::Refusal) -> u32 {
    match r {
        usb::Refusal::Truncated => 1,
        usb::Refusal::NotThatDescriptor => 2,
        usb::Refusal::Malformed { .. } => 3,
        usb::Refusal::NoBootKeyboard => 4,
        usb::Refusal::NoInterruptIn { .. } => 5,
        usb::Refusal::PacketTooSmall { .. } => 6,
    }
}

impl Host {
    /// **Halt, reset, lay out, run.** The controller is the driver's from here; the kernel only
    /// found it, took it from the firmware and confined it.
    fn bring_up(h: Handoff) -> Result<Host, Fail> {
        let capabilities = Capabilities::decode(
            REGS.r32(regs::CAPLENGTH),
            REGS.r32(regs::HCSPARAMS1),
            REGS.r32(regs::HCSPARAMS2),
            REGS.r32(regs::HCCPARAMS1),
            REGS.r32(regs::DBOFF),
            REGS.r32(regs::RTSOFF),
        );
        let phys = |page: u64| h.dma_phys + page * PAGE_BYTES;
        let host = Host {
            capabilities,
            dma_phys: h.dma_phys,
            commands: ProducerRing::new(phys(dma::COMMAND_RING), dma::RING_ENTRIES),
            events: EventRing::new(phys(dma::EVENT_RING), dma::RING_ENTRIES),
            control: ProducerRing::new(phys(dma::CONTROL_RING), dma::RING_ENTRIES),
            interrupt: ProducerRing::new(phys(dma::INTERRUPT_RING), dma::RING_ENTRIES),
            ports_changed: false,
            keyboard: None,
            found: None,
            last: BootReport::RELEASED,
            keys: Keyboard::new(),
        };
        host.halt_and_reset(h.dma_pages)?;
        host.lay_out();
        host.run()?;
        host.power_ports();
        Ok(host)
    }

    /// Halt the controller (the firmware may have left it running), reset it, and check it can
    /// use this driver's pages and that `dma_pages` holds everything it will lay out.
    fn halt_and_reset(&self, dma_pages: u32) -> Result<(), Fail> {
        let usbcmd = self.op(regs::USBCMD);
        let usbsts = self.op(regs::USBSTS);

        // Halt (the firmware may have left it running), then reset.
        self.set_reg(usbcmd, self.reg(usbcmd) & !regs::CMD_RUN);
        let end = deadline(100);
        while self.reg(usbsts) & regs::STS_HALTED == 0 {
            if past(end) {
                return Err((BringUpStep::HaltTimeout, self.reg(usbsts)));
            }
            core::hint::spin_loop();
        }
        self.set_reg(usbcmd, regs::CMD_RESET);
        let end = deadline(1000);
        while self.reg(usbcmd) & regs::CMD_RESET != 0 || self.reg(usbsts) & regs::STS_NOT_READY != 0
        {
            if past(end) {
                return Err((BringUpStep::ResetTimeout, self.reg(usbsts)));
            }
            core::hint::spin_loop();
        }
        if self.reg(self.op(regs::PAGESIZE)) & 1 == 0 {
            return Err((
                BringUpStep::NoFourKibibytePages,
                self.reg(self.op(regs::PAGESIZE)),
            ));
        }
        if u64::from(dma_pages)
            < extensible_host_controller_interface_contract::region_pages(
                self.capabilities.scratchpads,
            )
        {
            return Err((
                BringUpStep::RegionTooSmall,
                self.capabilities.scratchpads.into(),
            ));
        }
        Ok(())
    }

    /// Lay out the slot array, the command ring and the event ring, and enable interrupter 0.
    fn lay_out(&self) {
        let phys = |page: u64| self.dma_phys + page * PAGE_BYTES;
        // One slot: this driver serves one device at a time, and gives a slot back before it
        // asks for another.
        let config = self.op(regs::CONFIG);
        self.set_reg(config, (self.reg(config) & !0xff) | 1);

        // The slot array, its entry 0 the scratchpad array when the controller wants one.
        if self.capabilities.scratchpads > 0 {
            for i in 0..u64::from(self.capabilities.scratchpads) {
                DMA.w64(
                    dma::SCRATCHPAD_ARRAY * PAGE_BYTES + 8 * i,
                    phys(extensible_host_controller_interface_contract::WORK_PAGES + i),
                );
            }
            DMA.w64(
                dma::DEVICE_CONTEXT_ARRAY * PAGE_BYTES,
                phys(dma::SCRATCHPAD_ARRAY),
            );
        }
        barrier();
        self.set_reg64(self.op(regs::DCBAAP), phys(dma::DEVICE_CONTEXT_ARRAY));
        self.set_reg64(self.op(regs::CRCR), self.commands.dequeue_pointer());

        // The event ring: its one segment table entry, then interrupter 0, base last.
        let entry = self.events.segment_table_entry();
        for (i, w) in entry.iter().enumerate() {
            DMA.w32(dma::EVENT_SEGMENT_TABLE * PAGE_BYTES + 4 * i as u64, *w);
        }
        barrier();
        self.set_reg(self.capabilities.interrupter(regs::ERSTSZ), 1);
        self.set_reg64(
            self.capabilities.interrupter(regs::ERDP),
            self.events.dequeue_pointer(),
        );
        self.set_reg64(
            self.capabilities.interrupter(regs::ERSTBA),
            phys(dma::EVENT_SEGMENT_TABLE),
        );
        self.set_reg(
            self.capabilities.interrupter(regs::IMAN),
            regs::IMAN_ENABLE | regs::IMAN_PENDING,
        );
    }

    /// Set the controller running with interrupts on, and wait for it to leave the halted state.
    fn run(&self) -> Result<(), Fail> {
        let usbcmd = self.op(regs::USBCMD);
        let usbsts = self.op(regs::USBSTS);
        self.set_reg(usbcmd, regs::CMD_RUN | regs::CMD_INTERRUPTS);
        let end = deadline(100);
        while self.reg(usbsts) & regs::STS_HALTED != 0 {
            if past(end) {
                return Err((BringUpStep::RunTimeout, self.reg(usbsts)));
            }
            core::hint::spin_loop();
        }
        Ok(())
    }

    /// Power every port that is not powered (only on a controller with port power control).
    fn power_ports(&self) {
        for n in 1..=self.capabilities.max_ports {
            let sc = self.reg(self.capabilities.portsc(n));
            if sc & port::POWER == 0 {
                self.set_reg(
                    self.capabilities.portsc(n),
                    port::write_value(sc, port::POWER),
                );
            }
        }
    }

    /// **Wait until every root port's connection has held still** ([`port::PortDebounce`]), so the
    /// boot-time scan reads what is plugged in rather than what the controller has re-detected
    /// so far. `false` when the ports were still changing at the timeout. Nothing here is the
    /// answer: a device that arrives later is attached from its connect change, in
    /// [`Host::service_ports`].
    fn settle_ports(&self) -> bool {
        self.hold_still(&|h: &Host| h.connections())
    }

    /// **Debounce one port** whose connect change just arrived, as Linux's hub driver does before
    /// it resets a new device: `true` when its connection held for [`port::DEBOUNCE_STABLE_MS`]
    /// and it is still connected. A bouncing plug is reset once it stops bouncing, not mid-bounce.
    fn debounce(&self, n: u8) -> bool {
        let only = |h: &Host| {
            let mut bits = [0u32; 8];
            if h.reg(h.capabilities.portsc(n)) & port::CONNECTED != 0 {
                bits[usize::from(n / 32)] = 1 << (n % 32);
            }
            bits
        };
        self.hold_still(&only) && self.reg(self.capabilities.portsc(n)) & port::CONNECTED != 0
    }

    /// Read `read` every [`port::DEBOUNCE_STEP_MS`] until [`port::PortDebounce`] says it held still
    /// (`true`) or gave up (`false`).
    fn hold_still(&self, read: &dyn Fn(&Host) -> port::Connections) -> bool {
        let mut settle = port::PortDebounce::new(monotonic_nanos() / MS, read(self));
        loop {
            let end = deadline(port::DEBOUNCE_STEP_MS);
            while !past(end) {
                core::hint::spin_loop();
            }
            if let Some(settled) = settle.sample(monotonic_nanos() / MS, read(self)) {
                return settled;
            }
        }
    }

    /// Every root port's `CCS`, one bit per port.
    fn connections(&self) -> port::Connections {
        let mut bits = [0u32; 8];
        for n in 1..=self.capabilities.max_ports {
            if self.reg(self.capabilities.portsc(n)) & port::CONNECTED != 0 {
                bits[usize::from(n / 32)] |= 1 << (n % 32);
            }
        }
        bits
    }

    /// **Try every connected port until one is a keyboard.** `(connected, last refusal)`.
    fn scan(&mut self) -> (u8, u64) {
        let mut connected = 0;
        let mut refused = 0;
        for n in 1..=self.capabilities.max_ports {
            if self.reg(self.capabilities.portsc(n)) & port::CONNECTED == 0 {
                continue;
            }
            connected += 1;
            if self.keyboard.is_some() {
                continue;
            }
            match self.attach(n) {
                Ok(k) => self.keyboard = Some(k),
                Err((step, detail)) => refused = report::failure_report(step, n, detail),
            }
        }
        (connected, refused)
    }

    /// **A port changed**: give back the slot of a keyboard that left, and look at a port that
    /// gained a device when nothing is being served.
    fn service_ports(&mut self) {
        for n in 1..=self.capabilities.max_ports {
            let at = self.capabilities.portsc(n);
            let sc = self.reg(at);
            let changed = sc & port::CHANGES;
            if changed != 0 {
                self.set_reg(at, port::write_value(sc, changed));
            }
            match self.keyboard {
                Some(k) if k.port == n && sc & port::CONNECTED == 0 => {
                    let _ = self.command(TransferRequestBlock::disable_slot(k.slot));
                    self.keyboard = None;
                    self.found = None;
                }
                None if sc & port::CONNECTED != 0 && changed & port::CONNECT_CHANGE != 0 => {
                    if !self.debounce(n) {
                        continue;
                    }
                    if let Ok(k) = self.attach(n) {
                        self.keyboard = Some(k);
                    }
                }
                _ => {}
            }
        }
    }

    /// **One interrupt-endpoint completion**: decode the report, type what changed, requeue.
    fn report_arrived(&mut self, out: u64, trb: u64, residual: u32, code: u8) {
        let Some(k) = self.keyboard else { return };
        if code != completion::SUCCESS && code != completion::SHORT_PACKET {
            // A halted endpoint: reset it and queue fresh reads. A device that left is noticed by
            // its port change instead, and this fails harmlessly.
            if self.recover(k.slot, k.dci, false).is_ok() {
                for i in 0..dma::REPORTS_QUEUED {
                    self.queue_report(
                        k,
                        self.phys(dma::REPORT_BUFFERS) + u64::from(i) * dma::REPORT_SLOT,
                    );
                }
            }
            return;
        }
        // Which buffer: the TRB that completed names it, so read it back off the ring.
        let base = self.phys(dma::INTERRUPT_RING);
        let Some(index) = trb.checked_sub(base).map(|o| o / 16) else {
            return;
        };
        if index >= u64::from(dma::RING_ENTRIES) {
            return;
        }
        let sent = Host::read_trb(dma::INTERRUPT_RING, index as u16);
        let buffer = u64::from(sent.0[0]) | u64::from(sent.0[1]) << 32;
        let Some(offset) = buffer.checked_sub(self.phys(dma::REPORT_BUFFERS)) else {
            return;
        };
        if offset + dma::REPORT_SLOT > PAGE_BYTES {
            return;
        }
        barrier();
        let mut bytes = [0u8; REPORT_LEN];
        let got = REPORT_LEN.saturating_sub(residual as usize);
        for (i, b) in bytes.iter_mut().enumerate().take(got) {
            *b = DMA.r8(dma::REPORT_BUFFERS * PAGE_BYTES + offset + i as u64);
        }
        if let Some(now) = BootReport::parse(&bytes[..got]) {
            let mut typed = Typed::new(out);
            let keys = &mut self.keys;
            changes(&self.last, &now, &mut |code, pressed| {
                if let Some(b) = keys.event(EV_KEY, code, u32::from(pressed)) {
                    for &c in b.as_slice() {
                        typed.push(c);
                    }
                }
            });
            typed.flush();
            self.last = now;
        }
        self.queue_report(k, buffer);
    }
}

/// **Bytes for the line discipline, eight to a `CALL`**: `OPERATION_BYTES` packs up to eight in
/// one word, the framing `input` and `keyboard_driver` already use on the same endpoint.
struct Typed {
    out: u64,
    buf: [u8; 8],
    n: usize,
}

impl Typed {
    fn new(out: u64) -> Typed {
        Typed {
            out,
            buf: [0; 8],
            n: 0,
        }
    }

    fn push(&mut self, b: u8) {
        self.buf[self.n] = b;
        self.n += 1;
        if self.n == self.buf.len() {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.n == 0 {
            return;
        }
        let mut word = 0u64;
        for (i, &b) in self.buf[..self.n].iter().enumerate() {
            word |= u64::from(b) << (8 * i);
        }
        call(
            self.out,
            proto::req(proto::OPERATION_BYTES, self.n as u64),
            word,
        );
        self.n = 0;
    }
}

user_mode_runtime::program_entry!(start);

fn start(arg0: u64, arg1: u64, arg2: u64) -> ! {
    let Some(h) = Handoff::unpack([arg0, arg1, arg2]) else {
        send(
            REPORT,
            report::BRING_UP_FAILED,
            report::failure_report(BringUpStep::BadHandoff, 0, 0),
            arg1,
        );
        exit();
    };
    let mut host = match Host::bring_up(h) {
        Ok(host) => host,
        Err((step, detail)) => {
            send(
                REPORT,
                report::BRING_UP_FAILED,
                report::failure_report(step, 0, detail),
                0,
            );
            exit();
        }
    };
    let settled = host.settle_ports();
    let (connected, refused) = host.scan();
    match (host.keyboard, host.found) {
        (Some(k), Some((vendor, product, speed))) => send(
            REPORT,
            report::KEYBOARD_FOUND,
            report::keyboard_report(k.port, speed, vendor, product),
            u64::from(k.dci),
        ),
        _ => send(
            REPORT,
            report::NO_KEYBOARD_FOUND,
            report::ports_report(host.capabilities.max_ports, connected, settled),
            refused,
        ),
    };

    // **The attach handshake, first and alone**: the progenitor's `SEND_CAP` waits for this
    // receive, so nothing that could fail goes before it. What arrives is `WRITE` on the line
    // discipline's endpoint, and it is the only destination this program ever names.
    let (_, out, _) = receive_cap(ATTACH);

    loop {
        interrupt_wait(IRQ);
        // Clear the pending bits before draining, so an event that lands after the drain raises
        // the interrupt again rather than waiting for the next one.
        host.set_reg(
            host.capabilities.interrupter(regs::IMAN),
            regs::IMAN_ENABLE | regs::IMAN_PENDING,
        );
        host.set_reg(
            host.op(regs::USBSTS),
            regs::STS_EVENT_INTERRUPT | regs::STS_PORT_CHANGE,
        );
        while let Some(e) = host.poll_event() {
            match e {
                Event::Transfer {
                    trb,
                    residual,
                    code,
                    slot,
                    endpoint,
                } if host
                    .keyboard
                    .is_some_and(|k| k.slot == slot && k.dci == endpoint) =>
                {
                    host.report_arrived(out, trb, residual, code);
                }
                Event::PortStatus { .. } => host.ports_changed = true,
                _ => {}
            }
        }
        interrupt_acknowledge(IRQ);
        if host.ports_changed {
            host.ports_changed = false;
            host.service_ports();
        }
    }
}

user_mode_runtime::panic_handler!();
