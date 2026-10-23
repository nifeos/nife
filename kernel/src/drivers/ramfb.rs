//! `ramfb` over QEMU's `fw_cfg`: a screen on a board whose firmware never lit one. The guest owns
//! the pixels and hands the device their physical address, and QEMU scans them out.
//!
//! It reaches no kernel global (§4): the register block, a scratch region and that region's
//! physical address are all arguments, from `crate::screen`. It runs only before `arch::mmu::init`,
//! through the coarse boot map; the fine map does not cover `fw_cfg`. See
//! notes/serial-less-output/ramfb.md.
//!

//! # BUGS
//!
//! - It polls with an arbitrary spin bound, since nothing this early has a timer; QEMU completes a
//!   transfer inside the write that starts it.
//! - No cache maintenance, safe only because QEMU has no cache to be stale. It must not be pointed
//!   at anything that is not an emulator.
//! - A screen this brings up is guest RAM, not an aperture, so it cannot be handed to a userspace
//!   terminal (`crate::screen::is_kernel_memory`).

use firmware_configuration::{
    COMMAND_LEN, DIRECTORY_KEY, DirectoryEntry, DmaCommand, ENTRY_LEN, RAMFB, RAMFB_LEN,
    RamFramebuffer,
};
use machine_discovery::framebuffer::Framebuffer;

/// The DMA address register's offset in the `fw_cfg` MMIO block.
///
/// Written as two 32-bit halves, high first: the device starts the transfer on the **low** write,
/// which is what makes the pair atomic without the interface needing a doorbell of its own.
const DMA_REGISTER: u64 = 0x10;

/// How much scratch this driver needs, in bytes: a command and the largest buffer any one command
/// moves (a directory entry, at [`ENTRY_LEN`]).
pub(crate) const SCRATCH_LEN: usize = 128;

/// Where the data buffer starts inside the scratch. The command occupies the first sixteen bytes.
const DATA_AT: usize = 64;

/// The most directory entries this will walk before giving up. QEMU publishes a couple of dozen on
/// either `virt` board; the cap is here because the count is a number the device stated and a
/// corrupt one must bound the loop rather than read its way across memory.
const MAX_ENTRIES: u32 = 256;

/// How many times the control word is read before a transfer is called hung. See BUGS.
const MAX_POLLS: u32 = 1_000_000;

/// Why a `ramfb` did not come up.
///
/// Four reasons rather than an `Option`, because they are four different machines: one with no such
/// device, one whose device refused, one that never answered, and one whose directory is not the
/// shape this reads. A boot line that named none of them would send the next reader to the wrong
/// half of the problem.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// The device published no `etc/ramfb` file: this QEMU was started without `-device ramfb`.
    NoSuchFile,
    /// The device set its error bit.
    Refused,
    /// The control word never cleared within [`MAX_POLLS`].
    Hung,
    /// The file directory did not decode, or claimed more entries than [`MAX_ENTRIES`].
    Undecodable,
}

/// A `fw_cfg` register block, with a scratch region to talk through.
pub(crate) struct FirmwareConfiguration {
    /// The register block, as the caller can address it now.
    registers: u64,
    /// [`SCRATCH_LEN`] bytes the caller owns, as the caller can address them now.
    scratch: *mut u8,
    /// The same bytes, as the **device** must address them.
    scratch_physical: u64,
}

impl FirmwareConfiguration {
    /// # Safety
    ///
    /// `registers` must name a mapped `fw_cfg` MMIO block (QEMU's `qemu,fw-cfg-mmio`), and
    /// `scratch`/`scratch_physical` must name one region of at least [`SCRATCH_LEN`] bytes that is
    /// writable by this kernel and readable by the device. No alignment is required; every access
    /// here is a byte. Nothing here can check either.
    #[must_use]
    pub const unsafe fn new(registers: u64, scratch: *mut u8, scratch_physical: u64) -> Self {
        Self {
            registers,
            scratch,
            scratch_physical,
        }
    }

    /// Run one command and wait for the device to finish it.
    fn run(&mut self, command: DmaCommand) -> Result<(), Error> {
        let encoded = command.encode();
        // SAFETY: `scratch` is at least `SCRATCH_LEN` bytes by this type's contract, and
        // `COMMAND_LEN` is sixteen. Written a byte at a time so that no alignment beyond one is
        // assumed of the caller's region; the device reads it as a whole after the register write
        // below, which is ordered after these stores by the barrier there.
        unsafe {
            for (i, byte) in encoded.iter().enumerate() {
                self.scratch.add(i).write_volatile(*byte);
            }
        }

        let address = self.scratch_physical;
        // SAFETY: the register block, by this type's contract. The high half is written first and
        // the device begins the transfer on the low one, which is this interface's own convention.
        unsafe {
            // The command bytes (normal memory) must be visible before the register write
            // (device memory) that starts the transfer, or the device reads a stale command
            // (notes/memory-ordering.md).
            // PAIR: none in this tree, and none possible: the reader is QEMU's `fw_cfg` model.
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            let high = ((address >> 32) as u32).to_be();
            let low = ((address & 0xffff_ffff) as u32).to_be();
            ((self.registers + DMA_REGISTER) as *mut u32).write_volatile(high);
            ((self.registers + DMA_REGISTER + 4) as *mut u32).write_volatile(low);
        }

        for _ in 0..MAX_POLLS {
            // Byte at a time and reassembled, rather than one `u32` read. The interface is
            // big-endian, so the bytes have to be reordered either way, and reading them
            // individually means this driver asks nothing of the caller's alignment: a `*mut u32`
            // read of a region somebody else allocated is an alignment assumption nothing checks.
            let mut raw = [0u8; 4];
            for (i, byte) in raw.iter_mut().enumerate() {
                // SAFETY: the first four bytes of the caller's scratch, which the device writes the
                // control word back into. Volatile, because the value changes underneath this
                // kernel without any store it can see.
                *byte = unsafe { self.scratch.add(i).read_volatile() };
            }
            let word = u32::from_be_bytes(raw);
            match DmaCommand::settled(word) {
                Ok(true) => {
                    // PAIR: the `fence(SeqCst)` a few lines above, in this same function, which is
                    // the release leg of the same transaction. Everything the device wrote into the
                    // data buffer must be visible to the loads that follow this, and the volatile
                    // load that observed the cleared control word does not by itself order them on
                    // a weakly ordered machine.
                    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
                    return Ok(());
                }
                Ok(false) => core::hint::spin_loop(),
                Err(firmware_configuration::Refused) => return Err(Error::Refused),
            }
        }
        Err(Error::Hung)
    }

    /// The physical address of the data buffer inside the scratch.
    const fn data_physical(&self) -> u64 {
        self.scratch_physical + DATA_AT as u64
    }

    /// Read `len` bytes of the data buffer back out.
    fn data(&self, len: usize) -> [u8; ENTRY_LEN] {
        let mut out = [0u8; ENTRY_LEN];
        for (i, byte) in out.iter_mut().enumerate().take(len) {
            // SAFETY: `DATA_AT + ENTRY_LEN` is `SCRATCH_LEN`, so every offset read here is inside
            // the caller's region.
            *byte = unsafe { self.scratch.add(DATA_AT + i).read_volatile() };
        }
        out
    }

    /// Fill the data buffer from `bytes`.
    fn put(&mut self, bytes: &[u8]) {
        for (i, byte) in bytes.iter().enumerate() {
            // SAFETY: as [`data`](Self::data)'s; every caller here writes at most `RAMFB_LEN`.
            unsafe { self.scratch.add(DATA_AT + i).write_volatile(*byte) };
        }
    }

    /// **Which key is the file called `name`**, by walking the directory the device publishes.
    ///
    /// The keys are assigned in the order devices are attached, so this walk is the only correct
    /// way to ask. See the crate's BUGS.
    fn file(&mut self, name: &[u8]) -> Result<u16, Error> {
        self.run(DmaCommand::read(DIRECTORY_KEY, 4, self.data_physical()))?;
        let header = self.data(4);
        let count = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        if count > MAX_ENTRIES {
            return Err(Error::Undecodable);
        }
        for _ in 0..count {
            self.run(DmaCommand::read_more(
                ENTRY_LEN as u32,
                self.data_physical(),
            ))?;
            let raw = self.data(ENTRY_LEN);
            let entry = DirectoryEntry::parse(&raw).ok_or(Error::Undecodable)?;
            if entry.is(name) {
                return Ok(entry.select);
            }
        }
        Err(Error::NoSuchFile)
    }

    /// Point the `ramfb` at `screen`, whose `base` must be the physical address of memory this
    /// kernel owns for the life of the machine. From then the emulator scans those bytes out
    /// continuously, with no flush, doorbell or interrupt.
    ///
    /// # Errors
    ///
    /// See [`Error`]. The common one by far is [`Error::NoSuchFile`], which is every QEMU started
    /// without `-device ramfb` and is not a failure.
    pub(crate) fn point_ramfb(&mut self, screen: Framebuffer) -> Result<(), Error> {
        let key = self.file(RAMFB)?;
        self.put(&RamFramebuffer(screen).encode());
        self.run(DmaCommand::write(
            key,
            RAMFB_LEN as u32,
            self.data_physical(),
        ))
    }
}

/// The scratch region is written through a raw pointer and never shared, and the one caller holds
/// it for the length of one call. The pointer is what makes this type `!Send` by default; saying so
/// explicitly is not needed and is not claimed.
const _: () = {
    assert!(SCRATCH_LEN >= DATA_AT + ENTRY_LEN);
    assert!(DATA_AT >= COMMAND_LEN);
};
