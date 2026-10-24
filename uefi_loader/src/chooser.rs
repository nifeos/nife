//! **The chooser**: the half of this loader that decides which image boots, so a bad upgrade
//! cannot brick the machine. The file the firmware starts, `\EFI\BOOT\BOOTX64.EFI`, reads the slot
//! state from the GPT (`crates/boot_slot`, which carries the policy and the layout), picks a slot,
//! **spends one of its tries and writes that to disk**, and chain-loads the image in it. It has to
//! be ours: UEFI reserves GPT attribute bits 48 to 63 to the owner of the partition type, so
//! ordinary firmware never reads them. The chooser and the image are the same binary, so a machine
//! whose slots are all bad still boots the chooser's own image; [`STARTED_BY_CHOOSER`] in the
//! child's `LoadOptions` stops it chain-loading itself. notes/boot-slots.md has the failure table.

//! # The steps
//!
//! 1. Find the disk this file is on (a device-path prefix, `uefi_loader::device_path`) and read its
//!    slots. Another disk's slots are another system's, so a booted stick boots the stick.
//! 2. Pick the bootable slot of highest priority ([`boot_slot::select_excluding`]).
//! 3. **Spend a try and write the table back, flushed, before anything else**: an image that hangs
//!    is caught only by a try already on the disk, on the next boot.
//! 4. Read the slot header and image, and check the image against the header's checksum.
//! 5. `LoadImage` from that buffer, `StartImage`, and never come back. A slot failing a check,
//!    `LoadImage` or `StartImage` is given up ([`boot_slot::State::unreadable`]), the next tried.

//! # BUGS (the full accounts are in notes/boot-slots.md)
//!
//! - Only disks the firmware connected are seen; `install-boot` names the disk as a second option.
//! - The logical block size is assumed to be 512; other disks are skipped.
//! - A power cut during step 3's four block writes can leave the copies disagreeing. Untested.
//! - The writes go primary-first, so for a moment neither copy parses (`ROLE_CONFIRM`'s do not).
//! - Nothing reads the backup table when the primary fails to parse.
//! - An image started from a slot has no boot file, so it cannot install onto a second disk, and
//!   [`booted_from_nvme`] assumes NVMe without looking.
//! - x86_64 only: a device-tree machine has nowhere for a boot file (milestone 568).

use core::ffi::c_void;
use core::ptr::{self, NonNull};

use boot_slot::{SlotHeader, State, select_excluding};
use globally_unique_identifier_partition_table as gpt;
use gpt::guid::types;
use gpt::{Entry, GloballyUniqueIdentifierPartitionTable as Table};
use r_efi::efi::{self, BY_PROTOCOL, BootServices, Handle, SystemTable};
use r_efi::protocols::block_io::{self, Protocol as BlockIo};
use r_efi::protocols::loaded_image;
use uefi_loader::device_path;
use uuid::Uuid;

use crate::{BOOT_FILE_MAX, PAGE, SUCCESS, allocate_below, boot_services, say, say_decimal};

/// **The marker the chooser puts in a chain-loaded image's `LoadOptions`**, followed by one ASCII
/// digit naming the slot.
///
/// It is the one thing that stops a chooser starting itself forever, and it is deliberately plain
/// ASCII rather than the UTF-16 a UEFI shell would pass: these bytes are a contract between this
/// file and itself, and a reader running `strings` on the image should meet them without decoding.
///
/// Provisional, like everything else about this format; calef names what a reader meets.
pub(crate) const STARTED_BY_CHOOSER: &[u8] = b"nife-slot=";

/// How many boot slots the chooser will consider on one disk. Two are laid out; the policy does not
/// care and this is only the size of the arrays below.
const MAX_SLOTS: usize = 4;

/// How many partition entries the chooser will decode and write back. A table with more used
/// entries than this is read but never rewritten, so a try is never silently unspent.
const MAX_PARTITIONS: usize = 16;

/// The logical block size this chooser reads and writes. See `BUGS`.
const LBA: usize = 512;

/// Blocks of the primary table: the protective MBR, the header, and the 32-block entry array.
const PRIMARY_BLOCKS: usize = 34;

/// A buffer the firmware's block driver will accept: `BlockIo` requires the caller's buffer to meet
/// the medium's `IoAlign`, and a page is more alignment than any disk asks for.
#[repr(C, align(4096))]
struct Aligned<const N: usize>([u8; N]);

/// **The chooser's buffers, in one firmware allocation that [`choose`] makes and lends down.**
///
/// They were three `static mut`s until milestone 891 (a mutable static becomes an atomic, a lock or
/// a recorded cell), reached through safe functions that each handed out a fresh `&'static mut`, so
/// nothing but a comment stopped two live borrows of one buffer. Owned here, the borrow checker
/// does that. Pages rather than the stack: 24 KiB, and the frame that holds them is still live
/// while the child image runs on the same firmware stack. Never freed, which is what
/// [`Self::options`] needs: the child reads its `LoadOptions` after this loader has handed off.
///
/// Name: provisional, lane maintainer/static-mut-891 (#1997), 2026-10-12 (UTC). The kernel's
/// `fw_cfg` buffer in `screen.rs` is also `Scratch`; the pull request records no other name.
#[repr(C)]
struct Scratch {
    /// LBA 0 through 33 of the candidate disk, read once and then used as the staging buffer for
    /// the rewrite.
    primary: Aligned<{ PRIMARY_BLOCKS * LBA }>,
    /// One block of scratch for the backup header, which is the only block written on its own.
    block: Aligned<LBA>,
    /// The `LoadOptions` handed to a chain-loaded image: [`STARTED_BY_CHOOSER`] and a digit.
    options: [u8; 16],
}

impl Scratch {
    /// Ask the firmware for zeroed, page-aligned pages to hold one, or `None` if it has none.
    fn allocate(services: &BootServices) -> Option<&'static mut Self> {
        let pages = size_of::<Self>().div_ceil(PAGE as usize);
        let base = allocate_below(services, pages, efi::LOADER_DATA)?;
        let scratch = base as *mut Self;
        // SAFETY: the firmware just granted `pages` pages at `base` to this loader alone, which is
        // at least `size_of::<Self>()` bytes and page-aligned, more than `Self`'s alignment. Every
        // field is bytes, so the zeroing makes a valid value before the reference exists, and the
        // pages are never freed, so `'static` holds.
        unsafe {
            scratch.write_bytes(0, 1);
            Some(&mut *scratch)
        }
    }
}

/// **Which boot slot started this image, if a chooser did.**
///
/// `None` means the firmware started this file directly, which is the stick, the first boot after
/// an install, and every `-kernel` boot. The caller uses it twice: to know not to act as a chooser,
/// and to know that there is no boot file to read back.
pub(crate) fn started_from_slot(handle: Handle, services: &BootServices) -> Option<u8> {
    let loaded = loaded_image(handle, services)?;
    if loaded.load_options.is_null() {
        return None;
    }
    let len = loaded.load_options_size as usize;
    if len <= STARTED_BY_CHOOSER.len() {
        return None;
    }
    // SAFETY: the firmware reports `load_options_size` bytes at `load_options`, and they were
    // written by this loader's own chooser in the parent image, which outlives the child.
    let options = unsafe { core::slice::from_raw_parts(loaded.load_options.cast::<u8>(), len) };
    if !options.starts_with(STARTED_BY_CHOOSER) {
        return None;
    }
    let digit = options[STARTED_BY_CHOOSER.len()];
    digit.is_ascii_digit().then(|| digit - b'0')
}

/// **Choose a boot slot and start the image in it.**
///
/// Returns only when nothing was started, which is the caller's signal to boot the kernel embedded
/// in this file. Every refusal says why on the console first: this runs while there is still a
/// firmware console, and a machine that silently did something other than what its disk asked for
/// is the worst outcome available here.
pub fn choose(handle: Handle, table: &SystemTable, services: &BootServices) {
    // **Where this file came from.** Without it there is no telling a stick from the installed
    // disk, and the answer to not knowing is the image in hand: a stick that boots the disk's
    // system is the defect this is here to prevent (the live stick proposal's G1).
    let Some(own) = loaded_image(handle, services).and_then(|l| path_of(l.device_handle, services))
    else {
        say(
            table,
            "uefi_loader: the firmware did not say which disk this file is on; using the image in this file\r\n",
        );
        return;
    };
    let Some(scratch) = Scratch::allocate(services) else {
        say(
            table,
            "uefi_loader: no memory for the boot slot chooser; using the image in this file\r\n",
        );
        return;
    };
    let Some(mut disk) = the_disk_with_slots(table, services, own, scratch) else {
        // Not an installed machine, or a stick on one. Both are ordinary.
        return;
    };

    say(table, "uefi_loader: boot slots on one disk, ");
    say_decimal(table, disk.slots as u32);
    say(table, " of them\r\n");

    let mut tried = 0u64;
    while let Some(slot) = select_excluding(&disk.states[..disk.slots], tried) {
        // **The try is spent here, before anything is read and long before anything is started.**
        // Everything below this line can hang the machine, and the whole point of the design is
        // that a hang costs a try anyway.
        let spent = disk.states[slot].attempted();
        if spent != disk.states[slot] {
            disk.states[slot] = spent;
            if !disk.write_back(table, services, slot, scratch) {
                say(table, "uefi_loader: the disk refused the boot state; ");
                say(
                    table,
                    "not starting a slot whose try could not be spent\r\n",
                );
                break;
            }
        }

        tried |= 1 << slot;
        match disk.start(handle, table, services, slot, scratch) {
            Started::Unreadable => {
                // A failure the chooser watched happen. Give up on the slot now rather than over
                // three reboots of a machine somebody is waiting on.
                disk.states[slot] = disk.states[slot].unreadable();
                let _ = disk.write_back(table, services, slot, scratch);
            }
            Started::Refused => {}
        }
    }

    say(
        table,
        "uefi_loader: no boot slot would start; using the image in this file\r\n",
    );
}

/// Why a slot did not boot. There is no success arm: a slot that starts never comes back.
enum Started {
    /// `StartImage` was never reached: the header, the checksum or `LoadImage` said no.
    Unreadable,
    /// The image started and returned, which for a nife boot image means it failed with the
    /// firmware console still up and has already said why.
    Refused,
}

/// The candidate disk, its table, and the slot state read off it.
struct Disk {
    block_io: *mut BlockIo,
    media_id: u32,
    disk_guid: Uuid,
    block_count: u64,
    /// Every used entry, in index order, so a rewrite puts them back where they were.
    parts: [Entry; MAX_PARTITIONS],
    used: usize,
    /// For each slot, its index in [`Self::parts`].
    at: [usize; MAX_SLOTS],
    states: [State; MAX_SLOTS],
    slots: usize,
}

/// **Find the disk this file is on, if its GPT carries boot slots.**
///
/// `own` is the device path of the medium this file was loaded from, without its end node. A disk
/// carrying slots that is not that medium is another machine's system as far as this boot is
/// concerned (an installed disk under a stick), and is said out loud and left alone: not read past
/// its table, and never written.
fn the_disk_with_slots(
    table: &SystemTable,
    services: &BootServices,
    own: &[u8],
    scratch: &mut Scratch,
) -> Option<Disk> {
    let mut count = 0usize;
    let mut handles: *mut Handle = ptr::null_mut();
    let mut guid = block_io::PROTOCOL_GUID;
    // SAFETY: a boot service, before `ExitBootServices`. `BY_PROTOCOL` takes no search key; the
    // firmware reads the GUID and writes the count and its own pool buffer of handles.
    let located = unsafe {
        (services.locate_handle_buffer)(
            BY_PROTOCOL,
            &mut guid,
            ptr::null_mut(),
            &mut count,
            &mut handles,
        )
    };
    if located != SUCCESS || handles.is_null() {
        return None;
    }
    // SAFETY: the firmware answered with `count` handles in a pool buffer it owns, and it stays
    // valid until boot services end. It is never freed, which costs a few dozen bytes for the rest
    // of a boot.
    let handles = unsafe { core::slice::from_raw_parts(handles, count) };

    let mut elsewhere = false;
    for &h in handles {
        let Some(disk) = read_table(h, services, scratch) else {
            continue;
        };
        if path_of(h, services).is_some_and(|path| device_path::is_on(own, path)) {
            // One file is on one disk, so the first match is the only one.
            return Some(disk);
        }
        elsewhere = true;
    }
    if elsewhere {
        say(
            table,
            "uefi_loader: the boot slots here are on another disk; this file's medium has none, \
             so it is using the image in this file\r\n",
        );
    }
    None
}

/// **Did this boot come from the NVMe disk?** The answer the loader writes as
/// `boot_slot::medium::NVME`, and the only case in which the kernel mounts that disk (the live stick
/// proposal's G2; calef ruled the token on PR #1652, 2026-10-04 UTC).
///
/// An image a chooser started has no device handle of its own (`LoadImage` from a buffer), so for
/// it the answer is the chooser's: since G1 a chooser starts only slots on its own disk, and the
/// installer writes slots only to an NVMe disk. See `BUGS`.
pub fn booted_from_nvme(handle: Handle, services: &BootServices, from_slot: Option<u8>) -> bool {
    if from_slot.is_some() {
        return true;
    }
    loaded_image(handle, services)
        .and_then(|l| path_of(l.device_handle, services))
        .is_some_and(device_path::is_nvme)
}

/// **A handle's device path, without its end node**, or `None` when the firmware has none for it or
/// it does not parse. See `uefi_loader::device_path`.
fn path_of(handle: Handle, services: &BootServices) -> Option<&'static [u8]> {
    if handle.is_null() {
        return None;
    }
    let interface = boot_services::handle_protocol(
        services,
        handle,
        r_efi::protocols::device_path::PROTOCOL_GUID,
    )?;
    let base = interface.cast::<u8>().cast_const();
    let len = device_path::length(|at| {
        // SAFETY: `length` asks only for the header at the start of a node it has reached by
        // adding the lengths of the nodes before it, none of them past the end node, and stops
        // within `MAX_LEN`; the firmware's path is at least that long up to its end node.
        Some(unsafe { ptr::read_unaligned(base.add(at).cast::<[u8; 4]>()) })
    })?;
    // SAFETY: `length` walked these `len` bytes node by node; the firmware owns them until boot
    // services end, which is after the chooser is done with them.
    Some(unsafe { core::slice::from_raw_parts(base, len) })
}

/// Read one handle's table, and answer `Some` only if it is a whole disk carrying boot slots.
fn read_table(handle: Handle, services: &BootServices, scratch: &mut Scratch) -> Option<Disk> {
    let interface = boot_services::handle_protocol(services, handle, block_io::PROTOCOL_GUID)?;
    let block_io = interface.cast::<BlockIo>();
    // SAFETY: the firmware answered with its own `EFI_BLOCK_IO_PROTOCOL` for this handle, and the
    // `media` pointer inside it is the firmware's and outlives boot services.
    let media = unsafe { media(block_io) }?;
    let media_id = media.media_id;
    // **Whole disks only.** A GPT lives at LBA 1 of a disk; LBA 1 of a partition is somebody's
    // filesystem, and every partition on this machine also carries a `BlockIo`.
    if media.logical_partition || !media.media_present || media.block_size as usize != LBA {
        return None;
    }
    if media.last_block < PRIMARY_BLOCKS as u64 {
        return None;
    }

    let head = &mut scratch.primary.0;
    // SAFETY: `block_io` is the firmware's protocol, `head` is page-aligned and exactly
    // `PRIMARY_BLOCKS` blocks long, and the disk has at least that many.
    if unsafe {
        ((*block_io).read_blocks)(block_io, media_id, 0, head.len(), head.as_mut_ptr().cast())
            != SUCCESS
    } {
        return None;
    }

    let head: &[u8] = head;
    let table = Table::parse(&head[LBA..2 * LBA], &head[2 * LBA..]).ok()?;

    let mut disk = Disk {
        block_io,
        media_id,
        disk_guid: table.disk_guid(),
        block_count: table.block_count(),
        parts: [Entry::UNUSED; MAX_PARTITIONS],
        used: 0,
        at: [0; MAX_SLOTS],
        states: [State::EMPTY; MAX_SLOTS],
        slots: 0,
    };
    for (index, part) in table.partitions() {
        // `create` writes the partitions it is given at indices 0, 1, 2..., so a table whose used
        // entries are not contiguous from zero would be renumbered by a rewrite. Ours never are;
        // refusing is what keeps that an assumption this code states rather than one it holds.
        if index != disk.used || disk.used == MAX_PARTITIONS {
            return None;
        }
        disk.parts[disk.used] = part;
        if part.type_guid == types::NIFE_BOOT && disk.slots < MAX_SLOTS {
            disk.at[disk.slots] = disk.used;
            disk.states[disk.slots] = State::from_attributes(part.attributes);
            disk.slots += 1;
        }
        disk.used += 1;
    }
    (disk.slots > 0).then_some(disk)
}

impl Disk {
    /// **Write one slot's boot state back to both copies of the table**, and flush, because a try
    /// this loader has spent and a controller has not written down is a try nothing spent.
    ///
    /// `false` if any part of it was refused, and the caller's answer to that is not to start the
    /// slot: an image tried without its try being recorded is an image that will be tried forever.
    fn write_back(
        &mut self,
        table: &SystemTable,
        services: &BootServices,
        slot: usize,
        scratch: &mut Scratch,
    ) -> bool {
        let at = self.at[slot];
        self.parts[at].attributes = self.states[slot].into_attributes(self.parts[at].attributes);

        let (head, array) = scratch.primary.0.split_at_mut(2 * LBA);
        let Ok(built) = Table::create(
            self.disk_guid,
            LBA,
            self.block_count,
            &self.parts[..self.used],
            array,
        ) else {
            say(
                table,
                "uefi_loader: could not rebuild the partition table\r\n",
            );
            return false;
        };

        // `head[..LBA]` is still the protective MBR as it was read, and it is not rewritten: the
        // attribute bits are the only thing changing and the MBR does not describe them.
        let header_block = &mut head[LBA..];
        if built.write_primary_header(header_block).is_err() {
            return false;
        }
        let backup_entry_lba = built.backup_entry_lba();
        let backup_header_lba = built.backup_header_lba();

        let backup_header = &mut scratch.block.0;
        backup_header.fill(0);
        if built.write_backup_header(backup_header).is_err() {
            return false;
        }

        let _ = services;
        // Four ranges and no journal; see BUGS. The primary header goes last of the two primary
        // writes so that a table whose array landed and whose header did not fails its own CRC
        // rather than pointing at an array that is not the one it describes.
        self.write(built.primary_entry_lba(), built.entry_array())
            && self.write(backup_entry_lba, built.entry_array())
            && self.write(backup_header_lba, backup_header)
            && self.write(gpt::PRIMARY_HEADER_LBA, header_block)
            && self.flush()
    }

    /// One `WriteBlocks`, with the caller's buffer already a whole number of blocks.
    fn write(&self, lba: u64, bytes: &[u8]) -> bool {
        // SAFETY: `block_io` is the firmware's protocol for this disk; `bytes` lives in the
        // page-aligned [`Scratch`] and its length is a whole number of blocks.
        unsafe {
            ((*self.block_io).write_blocks)(
                self.block_io,
                self.media_id,
                lba,
                bytes.len(),
                // `WriteBlocks` takes `*mut` and only reads the buffer.
                bytes.as_ptr().cast_mut().cast(),
            ) == SUCCESS
        }
    }

    /// `FlushBlocks`, whose absence is the difference between a recorded try and a remembered one.
    fn flush(&self) -> bool {
        // SAFETY: as above.
        unsafe { ((*self.block_io).flush_blocks)(self.block_io) == SUCCESS }
    }

    /// Read the slot's header and image, and start it. Returns only on failure.
    fn start(
        &self,
        handle: Handle,
        table: &SystemTable,
        services: &BootServices,
        slot: usize,
        scratch: &mut Scratch,
    ) -> Started {
        let first = self.parts[self.at[slot]].first_lba;
        let block = &mut scratch.block.0;
        // SAFETY: `block_io` is the firmware's protocol and `block` is one page-aligned block.
        if unsafe {
            ((*self.block_io).read_blocks)(
                self.block_io,
                self.media_id,
                first,
                LBA,
                block.as_mut_ptr().cast(),
            ) != SUCCESS
        } {
            say(table, "uefi_loader: boot slot ");
            say_decimal(table, slot as u32);
            say(table, " would not read\r\n");
            return Started::Unreadable;
        }
        let Some(header) = SlotHeader::decode(block) else {
            say(table, "uefi_loader: boot slot ");
            say_decimal(table, slot as u32);
            say(table, " holds no image\r\n");
            return Started::Unreadable;
        };
        if header.image_len == 0 || header.image_len > BOOT_FILE_MAX {
            say(table, "uefi_loader: boot slot ");
            say_decimal(table, slot as u32);
            say(table, " claims an impossible size\r\n");
            return Started::Unreadable;
        }

        let blocks = header.image_len.div_ceil(LBA as u64);
        let pages = (blocks * LBA as u64).div_ceil(PAGE) as usize;
        let Some(base) = allocate_below(services, pages, efi::LOADER_DATA) else {
            say(table, "uefi_loader: no memory for a boot slot's image\r\n");
            return Started::Unreadable;
        };
        // SAFETY: `pages` covers `blocks` whole blocks at `base`, which the firmware just granted
        // exclusively and which is page-aligned.
        if unsafe {
            ((*self.block_io).read_blocks)(
                self.block_io,
                self.media_id,
                first + boot_slot::SLOT_IMAGE_OFFSET / LBA as u64,
                (blocks * LBA as u64) as usize,
                base as *mut c_void,
            ) != SUCCESS
        } {
            say(table, "uefi_loader: boot slot ");
            say_decimal(table, slot as u32);
            say(table, "'s image would not read\r\n");
            return Started::Unreadable;
        }

        // SAFETY: the firmware granted the range and the read above filled it.
        let image =
            unsafe { core::slice::from_raw_parts(base as *const u8, header.image_len as usize) };
        if !header.matches(image) {
            // The failure worth having a checksum for: an install or an upgrade that died partway
            // through the copy leaves a valid-looking prefix of a boot image.
            say(table, "uefi_loader: boot slot ");
            say_decimal(table, slot as u32);
            say(table, " failed its checksum\r\n");
            return Started::Unreadable;
        }

        say(table, "uefi_loader: starting boot slot ");
        say_decimal(table, slot as u32);
        say(table, "\r\n");

        let mut child: Handle = ptr::null_mut();
        // SAFETY: a boot service, before `ExitBootServices`. A null device path with a source buffer
        // is the specification's way to start an image that is not a file; the firmware copies
        // `image` (it only reads it, though the signature says `*mut`) and writes only `child`.
        let loaded = unsafe {
            (services.load_image)(
                // BootPolicy FALSE: this is not a boot-manager path, it is a buffer we hold.
                efi::Boolean::FALSE,
                handle,
                ptr::null_mut(),
                image.as_ptr().cast_mut().cast(),
                image.len(),
                &mut child,
            )
        };
        if loaded != SUCCESS || child.is_null() {
            say(table, "uefi_loader: the firmware would not load boot slot ");
            say_decimal(table, slot as u32);
            say(table, "\r\n");
            return Started::Unreadable;
        }

        tell_it_which_slot(child, services, slot, &mut scratch.options);

        let mut exit_size = 0usize;
        let mut exit_data: *mut u16 = ptr::null_mut();
        // SAFETY: a boot service, for the image `LoadImage` just returned; it writes only the exit
        // size and data, and returns only if the started image does.
        let _ = unsafe { (services.start_image)(child, &mut exit_size, &mut exit_data) };
        // A nife boot image never returns, so arriving here means it failed and has already said so
        // on this same console.
        say(table, "uefi_loader: boot slot ");
        say_decimal(table, slot as u32);
        say(table, " came back, so it did not boot\r\n");
        Started::Refused
    }
}

/// **Tell the child which slot it is**, by writing [`STARTED_BY_CHOOSER`] and a digit into its
/// `LoadOptions`.
///
/// A failure here is not fatal and is not reported: the child then looks like an image the firmware
/// started directly, which means it boots its own kernel, which is what it was going to do anyway.
/// What it loses is the ability to say which slot it came from.
fn tell_it_which_slot(child: Handle, services: &BootServices, slot: usize, options: &mut [u8; 16]) {
    let Some(mut loaded) = loaded_image_pointer(child, services) else {
        return;
    };
    options[..STARTED_BY_CHOOSER.len()].copy_from_slice(STARTED_BY_CHOOSER);
    options[STARTED_BY_CHOOSER.len()] = b'0' + (slot as u8 % 10);
    let len = STARTED_BY_CHOOSER.len() + 1;

    // SAFETY: the firmware's `EFI_LOADED_IMAGE_PROTOCOL` for an image that has been loaded and not
    // yet started is the caller's to fill in; that is the specified way to pass load options, and
    // nothing else holds a reference to it while the child has not started. `options` is in
    // [`Scratch`]'s pages, which are never freed and so outlive the child.
    let loaded = unsafe { loaded.as_mut() };
    loaded.load_options = options.as_mut_ptr().cast::<c_void>();
    loaded.load_options_size = len as u32;
}

/// `HandleProtocol` for `EFI_LOADED_IMAGE_PROTOCOL`, read-only, which the readers here want.
fn loaded_image(
    handle: Handle,
    services: &BootServices,
) -> Option<&'static loaded_image::Protocol> {
    let pointer = loaded_image_pointer(handle, services)?;
    // SAFETY: `loaded_image_pointer` returns only an interface the firmware answered SUCCESS with,
    // non-null by construction; it is the firmware's own for this image handle, aligned for the
    // protocol it names, and it outlives boot services.
    Some(unsafe { pointer.as_ref() })
}

/// `HandleProtocol` for `EFI_LOADED_IMAGE_PROTOCOL` as the firmware's pointer, or `None` unless it
/// answered SUCCESS with a non-null interface. [`tell_it_which_slot`] writes through it, so it
/// must not come from a shared reference.
fn loaded_image_pointer(
    handle: Handle,
    services: &BootServices,
) -> Option<NonNull<loaded_image::Protocol>> {
    let interface = boot_services::handle_protocol(services, handle, loaded_image::PROTOCOL_GUID)?;
    NonNull::new(interface.cast::<loaded_image::Protocol>())
}

/// The fields of `EFI_BLOCK_IO_MEDIA` the chooser reads.
struct Medium {
    /// Changes when the medium is swapped. Every read and write must carry the current value, which
    /// is how the firmware refuses a request aimed at a disk that is no longer there.
    media_id: u32,
    /// **False on an empty optical drive**, which is the one handle a naive enumeration trips over.
    media_present: bool,
    /// **True for a partition, false for the disk it is on.** The chooser wants whole disks: a GPT
    /// lives at LBA 1 of a disk, and LBA 1 of a partition is somebody's filesystem.
    logical_partition: bool,
    /// Bytes per logical block. 512 on everything this has been run on, and not assumed.
    block_size: u32,
    /// The last addressable block, so the disk is `last_block + 1` blocks long.
    last_block: u64,
}

/// What the firmware says the medium behind a `BlockIo` is, or `None` for a null description.
///
/// **Read field by field, never as a `&block_io::Media`**: `r-efi` declares revision 3's fields
/// after `last_block`, which a revision-1 firmware does not allocate, and declares the five
/// `BOOLEAN`s as Rust `bool`, which is undefined behavior to read for any byte but 0 or 1. So the
/// flags are read as the bytes the firmware writes, and anything nonzero is true.
///
/// # Safety
/// `block_io` must be a firmware-owned `EFI_BLOCK_IO_PROTOCOL` whose `media` pointer is the
/// firmware's own and outlives boot services.
unsafe fn media(block_io: *mut BlockIo) -> Option<Medium> {
    // SAFETY: the caller's contract.
    let pointer = unsafe { (*block_io).media };
    if pointer.is_null() {
        return None;
    }
    // SAFETY: every field read here is in revision 1 of the structure, which any firmware that
    // installs the protocol allocates; the flags are read through `u8` pointers, as above.
    unsafe {
        Some(Medium {
            media_id: (&raw const (*pointer).media_id).read(),
            media_present: (&raw const (*pointer).media_present).cast::<u8>().read() != 0,
            logical_partition: (&raw const (*pointer).logical_partition)
                .cast::<u8>()
                .read()
                != 0,
            block_size: (&raw const (*pointer).block_size).read(),
            last_block: (&raw const (*pointer).last_block).read(),
        })
    }
}
