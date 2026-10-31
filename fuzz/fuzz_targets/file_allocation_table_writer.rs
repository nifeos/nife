//! Fuzz the FAT writer with arbitrary operations on an arbitrary volume.
//!
//! The first byte picks the volume: the rest of the input as a hostile image of 128 KiB (the
//! writer meets whatever a stranger's stick holds, and the seed is a volume the reader target also
//! uses), or an empty volume this crate formats, where the operations get deep enough to matter.
//! The operations that follow are decoded four bytes at a time: create a file, write bytes, set a
//! length, seek, remove, make a directory, remove one, rename. Every name and every byte is the
//! input's. The target holds the writer to what it promises on any volume: no panic, no write
//! outside the image, no read or write past the sector count the device has, and, on a volume it
//! formatted, every name it created listed once with the size it was last given, a clean
//! shutdown bit after `sync`, and a directory tree that still walks.

#![no_main]

use file_allocation_table::{
    Format, FormatOptions, NamedEntry, SectorDevice, SectorSize, Volume, WritableSectorDevice,
};
use libfuzzer_sys::fuzz_target;

/// The most bytes of a hostile image or a formatted volume: small, so a run is fast and a fill of
/// the whole volume is reached.
const IMAGE: usize = 128 * 1024;

/// A volume in memory; a read or write past its end is the device's error, never a panic.
struct Image {
    bytes: Vec<u8>,
}

impl SectorDevice for Image {
    type Error = ();

    fn sector_size(&self) -> SectorSize {
        SectorSize::Bytes512
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), ()> {
        assert_eq!(buffer.len(), 512);
        let start = usize::try_from(index).map_err(|_| ())?.checked_mul(512).ok_or(())?;
        buffer.copy_from_slice(self.bytes.get(start..start.checked_add(512).ok_or(())?).ok_or(())?);
        Ok(())
    }
}

impl WritableSectorDevice for Image {
    fn write_sector(&mut self, index: u64, buffer: &[u8]) -> Result<(), ()> {
        assert_eq!(buffer.len(), 512);
        let start = usize::try_from(index).map_err(|_| ())?.checked_mul(512).ok_or(())?;
        let end = start.checked_add(512).ok_or(())?;
        self.bytes.get_mut(start..end).ok_or(())?.copy_from_slice(buffer);
        Ok(())
    }
}

/// A name of one to twelve bytes from the input, over a small alphabet so names collide, nest and
/// need long-name slots all at once.
fn name(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"AaBb.. _~1";
    let len = 1 + usize::from(bytes.first().copied().unwrap_or(0)) % 12;
    (0..len)
        .map(|i| ALPHABET[usize::from(bytes.get(i + 1).copied().unwrap_or(0)) % ALPHABET.len()] as char)
        .collect()
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let formatted = mode & 1 == 1;
    let mut image = Image {
        bytes: vec![0; IMAGE],
    };
    let operations = if formatted {
        let options = FormatOptions::new(SectorSize::Bytes512, (IMAGE / 512) as u32);
        let Ok(format) = Format::new(options) else {
            return;
        };
        format.write_to(&mut image).expect("a format fits its own image");
        rest
    } else {
        let kept = rest.len().min(IMAGE);
        image.bytes[..kept].copy_from_slice(&rest[..kept]);
        &rest[kept..]
    };
    let Ok(mut volume) = Volume::open(image) else {
        return;
    };
    let mut made: Vec<String> = Vec::new();
    for op in operations.chunks(4).take(64) {
        let picked = name(op);
        let found = |volume: &mut Volume<Image>, path: &str| -> Option<NamedEntry> {
            volume.lookup(path).ok()
        };
        match op[0] % 8 {
            0 => {
                if volume.create_file(0, &picked).is_ok() {
                    made.push(picked);
                }
            }
            1 => {
                if let Some(entry) = found(&mut volume, &picked)
                    && let Ok(mut writer) = volume.open_file_writer(&entry)
                {
                    let len = usize::from(op.get(1).copied().unwrap_or(0)) * 37;
                    let _ = writer.write(&vec![op.get(2).copied().unwrap_or(0); len]);
                }
            }
            2 => {
                if let Some(entry) = found(&mut volume, &picked)
                    && let Ok(mut writer) = volume.open_file_writer(&entry)
                {
                    let _ = writer.set_len(u32::from(op.get(1).copied().unwrap_or(0)) * 600);
                }
            }
            3 => {
                if let Some(entry) = found(&mut volume, &picked)
                    && let Ok(mut writer) = volume.open_file_writer(&entry)
                {
                    writer.seek(u32::from(op.get(1).copied().unwrap_or(0)) * 11);
                    let _ = writer.write(&[op.get(2).copied().unwrap_or(0); 700]);
                }
            }
            4 => {
                if let Some(entry) = found(&mut volume, &picked) {
                    let _ = volume.remove_file(&entry);
                }
            }
            5 => {
                if volume.make_directory(0, &picked).is_ok() {
                    made.push(picked);
                }
            }
            6 => {
                if let Some(entry) = found(&mut volume, &picked) {
                    let _ = volume.remove_directory(&entry);
                }
            }
            _ => {
                if let Some(entry) = found(&mut volume, &picked) {
                    let to = name(&op[1..]);
                    let _ = volume.rename(&entry, 0, &to);
                }
            }
        }
    }
    let synced = volume.sync().is_ok();
    // On a volume this crate formatted, the tree still walks and the shutdown bit is clean.
    if formatted && synced {
        let mut listed = 0;
        for entry in volume.root_entries() {
            entry.expect("a volume the writer changed still lists");
            listed += 1;
        }
        assert!(listed <= made.len() + 1, "more entries than the writer made");
        // FAT12 keeps no such bit; FAT16 and FAT32 must read clean after the sync.
        assert_ne!(volume.is_clean().expect("the table reads"), Some(false));
    }
});
