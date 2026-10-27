//! **Reading the GUID partition table** at the front of a device: finding it, listing it, and
//! warning when a volume opened as a whole file turns out to have one.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// How much of the front of a device to read before parsing the table: 34 blocks at the largest
/// logical block size this crate handles, which covers the protective MBR, the primary header and
/// the whole 16 KiB entry array on a 4Kn drive and covers them eight times over on a 512-byte one.
/// One read, then both candidate block sizes are tried against the same bytes.
pub(crate) const TABLE_BYTES: usize =
    34 * globally_unique_identifier_partition_table::MAX_BLOCK_SIZE;

/// Read the front of a device: enough for the protective MBR, the primary header and the entry
/// array at either candidate block size. A short file is not an error here, because the parse below
/// says what is missing in the format's own vocabulary and "read 8192 bytes, wanted 139264" does not.
pub(crate) fn read_head(file: &File, path: &Path) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::FileExt;

    let mut head = vec![0u8; TABLE_BYTES];
    let mut got = 0;
    while got < head.len() {
        match file.read_at(&mut head[got..], got as u64) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => {
                return Err(format!(
                    "cannot read the partition table off {}: {e}",
                    path.display()
                ));
            }
        }
    }
    head.truncate(got);
    Ok(head)
}

/// Parse the GUID partition table at the front of `head`, returning it and the logical block size it
/// was found at.
///
/// **The block size is discovered, not configured.** Nothing in a GPT records it (it is a property
/// of the device, and the block-service wire does not carry it either: `disk_surveyor`'s BUGS), so
/// the header is tried at 512 first, which is what almost every disk reports and what everything
/// this project has written, and then at 4096, which is what a native-4K drive reports. A wrong
/// guess fails on the signature rather than reading a plausible wrong table, because at the wrong
/// offset there is no `EFI PART`.
pub(crate) fn parse_table<'a>(
    head: &'a [u8],
    path: &Path,
) -> Result<(GloballyUniqueIdentifierPartitionTable<'a>, usize), String> {
    let mut first: Option<(usize, globally_unique_identifier_partition_table::Error)> = None;
    for block_size in [
        globally_unique_identifier_partition_table::MIN_BLOCK_SIZE,
        globally_unique_identifier_partition_table::MAX_BLOCK_SIZE,
    ] {
        let Some(header_block) = head.get(block_size..2 * block_size) else {
            continue;
        };
        // The header is decoded on its own first, because its `PartitionEntryLBA` is what says where
        // the array starts; passing the array from a hardcoded LBA 2 would be right on every disk
        // anyone has written and wrong on the one that is not.
        let header = match globally_unique_identifier_partition_table::Header::decode(header_block)
        {
            Ok(h) => h,
            Err(e) => {
                first.get_or_insert((block_size, e));
                continue;
            }
        };
        let array_at = (header.entry_array_lba as usize).saturating_mul(block_size);
        let array = head.get(array_at..).unwrap_or(&[]);
        match GloballyUniqueIdentifierPartitionTable::parse(header_block, array) {
            Ok(table) => return Ok((table, block_size)),
            Err(e) => {
                first.get_or_insert((block_size, e));
            }
        }
    }
    match first {
        Some((block_size, e)) => Err(format!(
            "{}: no GUID partition table ({e}, reading {block_size}-byte logical blocks). \
             An image file has no table and is read without a partition selector.",
            path.display(),
        )),
        None => Err(format!(
            "{}: only {} bytes, which is too short to hold a partition table",
            path.display(),
            head.len(),
        )),
    }
}

/// Say so when a volume opened as a whole file turns out to have a partition table on it.
///
/// `FileSystem::open` scans blocks 0..65536 for a header, so a partitioned device opens whatever
/// filesystem lies in its first 256 MiB: perhaps a partition nobody named, never one further out,
/// and with the allocator sized from the whole device. A warning rather than a refusal, because the
/// bytes do come back. See notes/host-recovery/devices-and-partitions.md.
pub(crate) fn warn_if_partitioned(file: &File, path: &Path) {
    let Ok(head) = read_head(file, path) else {
        return;
    };
    let Ok((table, block_size)) = parse_table(&head, path) else {
        return;
    };
    eprintln!(
        "redoxfs_host: {} has a GUID partition table ({}-byte blocks, {} partitions), so it is a \
         device rather than an image. Read whole, the engine takes whichever filesystem its scan \
         meets first and sizes itself from the whole device. Name the partition with --partition N \
         (`redoxfs_host partitions` prints them).",
        path.display(),
        block_size,
        table.partitions().count(),
    );
}

/// One used entry of a partition table, as the listing reports it.
pub struct PartitionInfo {
    /// The number a person types: 1-based, and a hole in the array does not renumber what follows.
    pub number: usize,
    pub type_guid: Uuid,
    /// The short name for a type this project recognises, or `None`, in which case the GUID is the
    /// string to go and look up.
    pub type_name: Option<&'static str>,
    pub first_lba: u64,
    pub last_lba: u64,
    pub bytes: u64,
    /// The GPT label, often empty: macOS writes none at all.
    pub name: String,
}

/// The table itself, for a listing.
pub struct TableInfo {
    /// The logical block size the table was found at. Printed because every LBA below counts in it.
    pub block_size: usize,
    pub disk_guid: Uuid,
    pub partitions: Vec<PartitionInfo>,
}

/// Read a device's partition table, so a person can see what is on the disk before choosing what to
/// recover from it.
///
/// This is the half that makes a partition selector usable at a keyboard: `--partition 3` is only
/// obvious once something has printed the three. It reads the front of the device and nothing else.
pub fn partitions(device: &Path) -> Result<TableInfo, String> {
    let file = File::open(device).map_err(|e| format!("cannot open {}: {e}", device.display()))?;
    let head = read_head(&file, device)?;
    let (table, block_size) = parse_table(&head, device)?;
    let mut out = Vec::new();
    for (index, entry) in table.partitions() {
        // 4 bytes per UTF-16 code unit is always enough (`name_utf8`), so this cannot overflow.
        let mut label = [0u8; 4 * globally_unique_identifier_partition_table::entry::NAME_UNITS];
        let written = entry.name_utf8(&mut label).unwrap_or(0);
        out.push(PartitionInfo {
            number: index + 1,
            type_guid: entry.type_guid,
            type_name: globally_unique_identifier_partition_table::guid::types::name(
                entry.type_guid,
            ),
            first_lba: entry.first_lba,
            last_lba: entry.last_lba,
            bytes: entry.blocks().unwrap_or(0) * block_size as u64,
            name: String::from_utf8_lossy(&label[..written]).into_owned(),
        });
    }
    Ok(TableInfo {
        block_size,
        disk_guid: table.disk_guid(),
        partitions: out,
    })
}
