# file_allocation_table

FAT12, FAT16 and FAT32 as pure computation: read, write and format volumes through a sector
interface you implement. It does no I/O, needs no allocator (`no_std`), and names no device. Long
file names (VFAT) work both ways, and every parser that meets bytes off a stranger's stick is
checked by a model checker.

## What it does

- **Read.** `Volume::open` checks the boot sector, types the volume by its cluster count as the
  specification does, and reads directories, long names, paths and file bytes. `MasterBootRecord`
  opens the volume inside a partitioned disk.
- **Write.** Open a `WritableVolume` with a `Clock` (a read-only `Volume` has no write methods and
  takes none). Create, write, extend, truncate, rename and remove files; make and remove directories;
  set attributes (read-only, hidden, system, archive); read and change the volume label. A name that
  is not 8.3 gets long-name slots and an alias numbered against its directory. The first change clears
  the clean-shutdown bit and `Volume::mark_clean` sets it again, with the FAT32 `FSInfo` hints.
- **Format.** `FormatPlan` lays out an empty volume of any size and sector size (512 to 4096 bytes),
  with the cluster size, serial number and label of your choice. `VolumeLayout` lays out the one
  shape a bootloader needs: a FAT32 EFI system partition holding one boot file.

## Example

```rust
use file_allocation_table::{
    Attributes, FormatPlan, FormatOptions, NoClock, SectorDevice, SectorSize, WritableSectorDevice,
    WritableVolume,
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

fn main() {
    // Format 8 MiB. The size makes it FAT12; a bigger device makes FAT16 or FAT32.
    let sectors = 16 * 1024;
    let options = FormatOptions::new(SectorSize::Bytes512, sectors)
        .with_label("EXAMPLE")
        .unwrap();
    let format = FormatPlan::new(options, &mut NoClock).expect("a legal size");
    let mut device = Memory(vec![0; sectors as usize * 512]);
    format.write_to(&mut device).unwrap();

    // Open it, make a directory and a file with a long name, and write some bytes.
    let mut volume = WritableVolume::open(device, NoClock).unwrap();
    let docs = volume.create_directory(0, "Documents").unwrap();
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
    volume.mark_clean().unwrap();

    // Read it back by path, as another program would.
    let entry = volume.lookup("documents/A LONG FILE NAME.TXT").unwrap();
    let mut bytes = [0u8; 32];
    let read = volume.open_file_reader(&entry).unwrap().read(&mut bytes).unwrap();
    assert_eq!(&bytes[..read], b"hello, volume");
    assert_eq!(
        volume.volume_label().unwrap().unwrap().trimmed(),
        b"EXAMPLE"
    );
}
```

`tests/the_readme_example.rs` compiles and runs this example. Every write goes through a
`WritableVolume`, which needs a `Clock`; `NoClock` is the explicit "no time", and the entries it
stamps carry the zero timestamp. For an EFI system partition:
`VolumeLayout::new(total_sectors, hidden_sectors, file_len, volume_id, "BOOTX64.EFI")` tells you what
every metadata sector holds, and you write those sectors and then the file's bytes.

## What is proven

Kani harnesses (`cargo kani`) prove, over every input they can unroll:

- **Hostile input.** No parse step panics or reads outside the volume: the boot sector and BPB (every
  512-byte sector), FAT entries, directory slots, long-name runs, the partition table and `FSInfo`.
  A cluster chain walk cannot loop: it ends within the volume's cluster count.
- **Writes.** A stored table entry reads back and disturbs nothing else; allocation takes one free
  cluster and changes nothing else; a release frees only the chain it was given and never a bad
  cluster; a cut keeps exactly the head of the chain; a partial sector write keeps the head, zeroes
  the slack and lands the bytes; a write extends the chain to the size it needs.
- **Names.** The long-name checksum equals the specification's loop for every short name; every
  long-name run is well formed; every alias is a legal short name that carries its number; the
  finder takes the first run of free slots.
- **Format.** A geometry the format computes parses back as the same type, clusters, table size
  and data start.
- **Attributes and labels.** Setting attributes changes the four user bits and nothing else, and
  never makes a long-name piece; an accepted label is legal bytes then spaces.

Each harness has a falsification record at `falsifications/<module.path>.<harness>.patch`. Applying
it with `patch -p3` from the crate root makes that harness fail at its own assertion, which shows
the proof checks something. Kani bounds what it unrolls (the model volumes are a few clusters, the
format proof covers clusters of 1 to 8 sectors), so the fuzz targets and the oracle tests cover
whole images.

The oracle is rust-fatfs (the `lamfat` dev-dependency): the tests write trees on FAT12, FAT16 and
FAT32 volumes with each, and each must read what the other wrote, name for name and byte for byte.
Where `fsck.fat` or `fsck_msdos` is installed every image must pass `fsck -n`, and where
`mkfs.fat` is installed its FAT12 and FAT16 volumes take this crate's writer. Two FAT32 images a
macOS host formatted and wrote are read to their recorded hashes. An ignored test has the host's
own driver read an image (`cargo test -- --ignored`, on macOS).

## Limitations

- Not crash-atomic, and there is no journal, since FAT has none. The crate orders its writes so a
  crash leaks a cluster and never shares one, and a write has no barrier: the device decides when a
  write reaches the medium.
- Time. The clock is required of every writer. FAT stores a date and a time of day with no time
  zone. In nife the clock supplies UTC (ruled 2026-11-01 UTC). The crate stores exactly the fields
  the clock reports and converts nothing, so it cannot check that a clock gives UTC, and a reader
  that treats FAT times as local time, such as Windows, shows a UTC value as local time. A read
  takes no clock and never writes. A time outside 1980 to 2107, or a field outside its range,
  is refused with `TimeOutOfRange` before a byte is written; a clock that cannot be trusted reports
  `None` and gets the zero timestamp. A write sets the write and access times, a new
  entry its creation time, a rename keeps the times, and a read stamps nothing.
- A name with a character outside the Basic Multilingual Plane is refused, and a lower-case 8.3 name
  is stored as a long name, since the crate writes no lower-case flag.
- Names compare by simple Unicode upper-casing, not the volume's code page or upcase table.
- A directory slot is found by walking from the directory's first cluster each time, so a directory
  of thousands of clusters is slow. The 65,536-entry ceiling is not tested.
- The reader does not follow extended partitions and reads no GUID partition table.
- exFAT is not covered; it is a sibling crate's job.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of the MIT license ([LICENSE-MIT](LICENSE-MIT)) or the Apache License,
Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.
