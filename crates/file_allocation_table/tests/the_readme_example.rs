//! The example in `README.md`, compiled and run, so the example cannot stop compiling unnoticed.

use file_allocation_table::{
    Attributes, Format, FormatOptions, SectorDevice, SectorSize, Volume, WritableSectorDevice,
};

/// A volume held in memory. A real caller wraps a block device the same way.
struct Memory(Vec<u8>);

impl SectorDevice for Memory {
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

impl WritableSectorDevice for Memory {
    fn write_sector(&mut self, index: u64, buffer: &[u8]) -> Result<(), ()> {
        let start = usize::try_from(index).map_err(|_| ())? * 512;
        self.0
            .get_mut(start..start + 512)
            .ok_or(())?
            .copy_from_slice(buffer);
        Ok(())
    }
}

#[test]
fn the_readme_example_runs() {
    // Format 8 MiB. The size makes it FAT12; a bigger device makes FAT16 or FAT32.
    let sectors = 16 * 1024;
    let options = FormatOptions::new(SectorSize::Bytes512, sectors)
        .with_label("EXAMPLE")
        .unwrap();
    let format = Format::new(options).expect("a legal size");
    let mut device = Memory(vec![0; sectors as usize * 512]);
    format.write_to(&mut device).unwrap();

    // Open it, make a directory and a file with a long name, and write some bytes.
    let mut volume = Volume::open(device).unwrap();
    let docs = volume.make_directory(0, "Documents").unwrap();
    let file = volume
        .create_file(docs.first_cluster(), "A long file name.txt")
        .unwrap();
    volume
        .open_file_writer(&file)
        .unwrap()
        .write(b"hello, volume")
        .unwrap();
    let stored = volume.lookup("Documents/A long file name.txt").unwrap();
    let hidden = Attributes::ARCHIVE.union(Attributes::HIDDEN);
    volume.set_attributes(&stored, hidden).unwrap();
    volume.sync().unwrap();

    // Read it back by path, as another program would.
    let entry = volume.lookup("documents/A LONG FILE NAME.TXT").unwrap();
    let mut bytes = [0u8; 32];
    let read = volume.open_file(&entry).unwrap().read(&mut bytes).unwrap();
    assert_eq!(&bytes[..read], b"hello, volume");
    assert_eq!(
        volume.volume_label().unwrap().unwrap().trimmed(),
        b"EXAMPLE"
    );
}
