//! **A read through `Volume` never writes, and works where writing is impossible.**
//!
//! A `Volume` takes no clock, and the ruling of 2026-11-01 (UTC) says a read must work on a
//! read-only device or image. Two checks: a device that implements only `SectorDevice` (so the
//! type system forbids a write), and a device whose write method panics, both read through a
//! `Volume` and nothing else. After every kind of read the image is byte for byte what it was.

use file_allocation_table::{
    Format, FormatOptions, NoClock, SectorDevice, SectorSize, Volume, WritableSectorDevice,
    WritableVolume,
};

/// A device that reads only: it has no write method at all.
struct ReadOnly<'a>(&'a [u8]);

impl SectorDevice for ReadOnly<'_> {
    type Error = ();

    fn sector_size(&self) -> SectorSize {
        SectorSize::Bytes512
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), ()> {
        let start = usize::try_from(index).map_err(|_| ())? * 512;
        buffer.copy_from_slice(self.0.get(start..start + 512).ok_or(())?);
        Ok(())
    }
}

/// A device that could write, and panics if the crate ever tries.
struct WritePanics<'a>(&'a [u8]);

impl SectorDevice for WritePanics<'_> {
    type Error = ();

    fn sector_size(&self) -> SectorSize {
        SectorSize::Bytes512
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), ()> {
        ReadOnly(self.0).read_sector(index, buffer)
    }
}

impl WritableSectorDevice for WritePanics<'_> {
    fn write_sector(&mut self, index: u64, _: &[u8]) -> Result<(), ()> {
        panic!("a read wrote sector {index}");
    }
}

/// A memory device the volume is built on.
struct Memory(Vec<u8>);

impl SectorDevice for Memory {
    type Error = ();
    fn sector_size(&self) -> SectorSize {
        SectorSize::Bytes512
    }
    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), ()> {
        ReadOnly(&self.0).read_sector(index, buffer)
    }
}

impl WritableSectorDevice for Memory {
    fn write_sector(&mut self, index: u64, buffer: &[u8]) -> Result<(), ()> {
        let start = index as usize * 512;
        self.0
            .get_mut(start..start + 512)
            .ok_or(())?
            .copy_from_slice(buffer);
        Ok(())
    }
}

/// A volume with directories, a long-named file, a label and some bytes, on each FAT type.
fn populated(sectors: u32) -> Vec<u8> {
    let options = FormatOptions::new(SectorSize::Bytes512, sectors)
        .with_label("READS")
        .unwrap();
    let format = Format::new(options, &mut NoClock).unwrap();
    let mut device = Memory(vec![0; sectors as usize * 512]);
    format.write_to(&mut device).unwrap();
    let mut volume = WritableVolume::open(device, NoClock).unwrap();
    let dir = volume.make_directory(0, "Some Directory").unwrap();
    let file = volume
        .create_file(dir.first_cluster(), "A file with a long name.txt")
        .unwrap();
    volume
        .open_file_writer(&file)
        .unwrap()
        .write(&[7u8; 5000])
        .unwrap();
    volume.sync().unwrap();
    volume.into_device().0
}

/// Every kind of read the crate has.
fn read_everything<D: SectorDevice>(device: D) {
    let mut volume = Volume::open(device).ok().expect("the volume opens");
    let found = volume
        .lookup("some directory/A FILE WITH A LONG NAME.TXT")
        .ok()
        .unwrap();
    let mut bytes = vec![0u8; 6000];
    let mut reader = volume.open_file(&found).ok().unwrap();
    assert_eq!(reader.read(&mut bytes).ok().unwrap(), 5000);
    assert!(bytes[..5000].iter().all(|b| *b == 7));
    assert!(volume.root_entries().count() >= 2);
    assert_eq!(
        volume.volume_label().ok().unwrap().unwrap().trimmed(),
        b"READS"
    );
    assert!(volume.count_free_clusters().ok().unwrap() > 0);
    let _ = volume.is_clean();
    let _ = volume.fs_info();
    for _ in volume.chain(2) {}
}

#[test]
fn reads_work_over_a_device_that_cannot_write() {
    for sectors in [16 * 1024u32, 64 * 1024, 200 * 1024] {
        let image = populated(sectors);
        read_everything(ReadOnly(&image));
    }
}

#[test]
fn reads_never_call_a_write_method_and_leave_the_image_as_it_was() {
    for sectors in [16 * 1024u32, 64 * 1024, 200 * 1024] {
        let image = populated(sectors);
        let before = image.clone();
        read_everything(WritePanics(&image));
        assert!(image == before, "a read changed the image");
    }
}
