//! **The two images a macOS host formatted and wrote are read to their recorded hashes.**
//!
//! `fixtures/macos/` holds a bare FAT32 volume and the same files inside an MBR partition, both
//! compressed (see its README for provenance). Every file in them is found by walking the
//! directories with this crate and its bytes are hashed with a SHA-256 written here, so the
//! hashes come from nothing this crate computed. Each expected hash is the line of
//! `notes/filesystem-crates-2026-10-07/expected.sha256` for that file.

use file_allocation_table::{NamedEntry, SectorDevice, SectorSize, Volume};

const BARE: &[u8] = include_bytes!("fixtures/macos/fat32.img.zst");
const PARTITIONED: &[u8] = include_bytes!("fixtures/macos/fat32-mbr.img.zst");

/// `hello from macos\n`.
const HELLO: &str = "ca3aebca6f93d4540b896a6378ddf76d2c3fdd498d11b4b2709aa724d4669bb2";
/// 4096 lines of fifteen digits and a newline.
const DATA: &str = "b50e134d44c35d5c5d2f2a46db3aa41315f7456e6881771739527fbabd2cf3bc";
/// `long name\n`.
const LONG: &str = "1272a49868c41260330ce643f91dffd1114abc24bf149dfb4ebfb8833bbe5670";

/// Decompress a fixture.
fn expand(compressed: &[u8]) -> Vec<u8> {
    use std::io::Read;
    let mut decoder = ruzstd::decoding::StreamingDecoder::new(compressed).expect("a zstd frame");
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).expect("the frame decodes");
    out
}

/// An image in memory, starting `offset` sectors in.
struct Image<'a> {
    bytes: &'a [u8],
    offset: u64,
}

impl SectorDevice for Image<'_> {
    type Error = u64;

    fn sector_size(&self) -> SectorSize {
        SectorSize::Bytes512
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), u64> {
        let start = usize::try_from((index + self.offset) * 512).map_err(|_| index)?;
        let bytes = self.bytes.get(start..start + 512).ok_or(index)?;
        buffer.copy_from_slice(bytes);
        Ok(())
    }
}

/// The first sector of the MBR's first partition.
fn first_partition(disk: &[u8]) -> u64 {
    assert_eq!(&disk[510..512], &[0x55, 0xAA], "an MBR signature");
    u64::from(u32::from_le_bytes(
        disk[446 + 8..446 + 12].try_into().unwrap(),
    ))
}

/// The bytes of the file `entry` names, read cluster by cluster through its chain.
fn read(volume: &mut Volume<Image<'_>>, image: &[u8], offset: u64, entry: &NamedEntry) -> Vec<u8> {
    let boot = *volume.boot_sector();
    let size = entry.size_bytes() as usize;
    let mut out = Vec::new();
    if size == 0 {
        return out;
    }
    let clusters: Vec<u32> = volume
        .chain(entry.first_cluster())
        .map(|c| c.expect("the chain reads"))
        .collect();
    assert_eq!(
        clusters.len(),
        size.div_ceil(boot.bytes_per_cluster() as usize)
    );
    for cluster in clusters {
        let at = (offset as usize + boot.cluster_first_sector(cluster).unwrap() as usize) * 512;
        out.extend_from_slice(&image[at..at + boot.bytes_per_cluster() as usize]);
    }
    out.truncate(size);
    out
}

/// Every file under `first`, as its path (each part its long name when it has one) and bytes.
fn files(
    volume: &mut Volume<Image<'_>>,
    image: &[u8],
    offset: u64,
    first: u32,
    prefix: &str,
    out: &mut Vec<(String, Vec<u8>)>,
) {
    let entries: Vec<NamedEntry> = volume
        .entries(first)
        .map(|e| e.expect("the directory reads"))
        .collect();
    for entry in entries {
        if entry.is_volume_label() || entry.is_dot_entry() {
            continue;
        }
        let short = entry.short_name();
        let short = String::from_utf8_lossy(&short);
        let (base, extension) = short.split_at(8);
        let short = if extension.trim_end().is_empty() {
            base.trim_end().to_string()
        } else {
            format!("{}.{}", base.trim_end(), extension.trim_end())
        };
        let name = match entry.long_name() {
            Some(name) => name.characters().collect::<String>(),
            None => short,
        };
        let path = format!("{prefix}{name}");
        if entry.is_directory() {
            files(
                volume,
                image,
                offset,
                entry.first_cluster(),
                &format!("{path}/"),
                out,
            );
        } else {
            out.push((path, read(volume, image, offset, &entry)));
        }
    }
}

fn listing(image: &[u8], offset: u64) -> Vec<(String, Vec<u8>)> {
    let mut volume = Volume::open(Image {
        bytes: image,
        offset,
    })
    .expect("a macOS volume opens");
    let mut out = Vec::new();
    files(&mut volume, image, offset, 0, "", &mut out);
    out.sort();
    out
}

/// Hold `found` to the three recorded files. macOS also leaves `.fseventsd` on a volume it
/// mounted, which is not a file the script wrote.
fn check(found: &[(String, Vec<u8>)]) {
    let mut names: Vec<String> = found.iter().map(|(p, _)| p.to_ascii_lowercase()).collect();
    names.sort();
    for wanted in ["hello.txt", "long file name.txt", "sub/data.bin"] {
        assert!(
            names.iter().any(|n| n == wanted),
            "{wanted} is missing from {names:?}"
        );
    }
    for (path, bytes) in found {
        let want = match path.to_ascii_lowercase().as_str() {
            "hello.txt" => HELLO,
            "long file name.txt" => LONG,
            "sub/data.bin" => DATA,
            _ => continue,
        };
        assert_eq!(sha256(bytes), want, "{path}");
    }
}

#[test]
fn the_bare_volume_reads_to_its_recorded_hashes() {
    let image = expand(BARE);
    let found = listing(&image, 0);
    // The long name is stored as written; the 8.3 names macOS upper-cases.
    assert!(found.iter().any(|(p, _)| p == "Long File Name.txt"));
    check(&found);
}

#[test]
fn the_volume_inside_the_partition_table_reads_to_its_recorded_hashes() {
    let image = expand(PARTITIONED);
    check(&listing(&image, first_partition(&image)));
}

/// SHA-256 as lower-case hex (FIPS 180-4), so the expected hashes are checked by code that is
/// not this crate's.
fn sha256(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes(word.try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (a, b) in h.iter_mut().zip(v) {
            *a = a.wrapping_add(b);
        }
    }
    h.iter().map(|word| format!("{word:08x}")).collect()
}

#[test]
fn the_sha256_here_is_sha256() {
    assert_eq!(
        sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(sha256(b"long name\n"), LONG);
}
