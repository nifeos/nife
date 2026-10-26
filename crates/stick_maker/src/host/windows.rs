//! **Windows: the four `kernel32` calls.** The pure half, which turns what they report into a
//! [`Disk`], is `crate::windows`.

use crate::disk::Disk;
use crate::windows::disk_from;

mod system {
    // `raw-dylib`: rustc writes the import table itself, so linking needs no `libkernel32.a` from
    // a MinGW installation, and the program cross-links from any host with `rust-lld`.
    #[link(name = "kernel32", kind = "raw-dylib")]
    unsafe extern "system" {
        pub fn GetLogicalDrives() -> u32;
        pub fn GetDriveTypeW(root: *const u16) -> u32;
        pub fn GetVolumeInformationW(
            root: *const u16,
            name: *mut u16,
            name_len: u32,
            serial: *mut u32,
            maximum_component: *mut u32,
            flags: *mut u32,
            fs_name: *mut u16,
            fs_name_len: u32,
        ) -> i32;
        pub fn GetDiskFreeSpaceExW(
            root: *const u16,
            free_to_caller: *mut u64,
            total: *mut u64,
            total_free: *mut u64,
        ) -> i32;
        pub fn SetErrorMode(mode: u32) -> u32;
    }
}

/// Every drive letter, as a [`Disk`], offered or not.
pub fn discover() -> Result<Vec<Disk>, String> {
    fn text(buffer: &[u16]) -> String {
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..end])
    }
    // SEM_FAILCRITICALERRORS: an empty card reader answers "no disk" rather than raising a dialog
    // box in front of the person, which is what Windows does by default.
    // SAFETY: sets a process-wide flag and touches no memory of ours.
    unsafe { system::SetErrorMode(1) };
    // SAFETY: takes nothing and returns a bitmask.
    let mask = unsafe { system::GetLogicalDrives() };
    let mut disks = Vec::new();
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = char::from(b'A' + i as u8);
        let root: Vec<u16> = format!("{letter}:\\").encode_utf16().chain([0]).collect();
        // SAFETY: `root` is NUL-terminated UTF-16 and outlives the call.
        let drive_type = unsafe { system::GetDriveTypeW(root.as_ptr()) };
        let mut label = [0u16; 261];
        let mut fs = [0u16; 261];
        let (mut serial, mut maximum, mut flags) = (0u32, 0u32, 0u32);
        // SAFETY: every out-pointer is to a local sized as passed.
        let have_volume = unsafe {
            system::GetVolumeInformationW(
                root.as_ptr(),
                label.as_mut_ptr(),
                label.len() as u32,
                &mut serial,
                &mut maximum,
                &mut flags,
                fs.as_mut_ptr(),
                fs.len() as u32,
            )
        } != 0;
        let (mut free, mut total, mut total_free) = (0u64, 0u64, 0u64);
        // SAFETY: as above.
        let have_space = unsafe {
            system::GetDiskFreeSpaceExW(root.as_ptr(), &mut free, &mut total, &mut total_free)
        } != 0;
        if !have_volume {
            continue; // an empty card reader, or a drive Windows cannot read
        }
        disks.push(disk_from(
            letter,
            drive_type,
            &text(&label),
            &text(&fs),
            if have_space { total } else { 0 },
            have_space.then_some(free),
        ));
    }
    Ok(disks)
}
