//! Files read from a small FAT12 volume built byte by byte: whole and in odd chunks, seeks in both
//! directions, a size the chain cannot cover, a chain longer than the size, a loop, and a device
//! that fails.
//!
//! Name: provisional.

use super::*;
use crate::reader::tests::{FIRST_DATA, Image};

/// A byte that tells which cluster and offset it came from.
fn pattern(cluster: u32, offset: usize) -> u8 {
    (cluster as usize * 7 + offset * 13) as u8
}

/// Fill `cluster` of `image` (one sector) with its pattern.
fn fill(image: &mut Image, cluster: u32) {
    let at = (FIRST_DATA + cluster as usize - 2) * 512;
    for offset in 0..512 {
        image.bytes[at + offset] = pattern(cluster, offset);
    }
}

/// The byte of the file made of `clusters`, at file offset `position`.
fn expected(clusters: &[u32], position: usize) -> u8 {
    pattern(clusters[position / 512], position % 512)
}

/// An image with a file of `clusters`, linked in order.
fn with_file(clusters: &[u32]) -> Image {
    let mut image = Image::fat12();
    for &cluster in clusters {
        fill(&mut image, cluster);
    }
    image.link(clusters);
    image
}

/// Read the whole file in chunks of `chunk` bytes.
fn read_all(
    volume: &mut Volume<Image>,
    first: u32,
    size: u32,
    chunk: usize,
    out: &mut [u8],
) -> usize {
    let mut file = FileReader::new(volume, first, size);
    let mut total = 0;
    let mut buffer = [0u8; 700];
    loop {
        let got = file.read(&mut buffer[..chunk]).expect("the file reads");
        if got == 0 {
            return total;
        }
        out[total..total + got].copy_from_slice(&buffer[..got]);
        total += got;
    }
}

#[test]
fn a_file_across_clusters_reads_back_in_any_chunk_size() {
    let clusters = [9, 30, 4, 17];
    let size = 3 * 512 + 100;
    for chunk in [1usize, 7, 100, 511, 512, 513, 600, 700] {
        let mut volume = Volume::open(with_file(&clusters)).unwrap();
        let mut out = [0u8; 2000];
        let got = read_all(&mut volume, clusters[0], size as u32, chunk, &mut out);
        assert_eq!(got, size, "chunk {chunk}");
        for (position, byte) in out[..size].iter().enumerate() {
            assert_eq!(
                *byte,
                expected(&clusters, position),
                "chunk {chunk} at {position}"
            );
        }
    }
}

#[test]
fn a_read_returns_fewer_bytes_only_at_the_end_of_the_file() {
    let mut volume = Volume::open(with_file(&[5, 6])).unwrap();
    let mut file = FileReader::new(&mut volume, 5, 700);
    let mut buffer = [0u8; 1000];
    assert_eq!(file.read(&mut buffer), Ok(700));
    assert_eq!(file.read(&mut buffer), Ok(0));
    assert_eq!(file.position(), 700);
    assert_eq!(file.size_bytes(), 700);
    assert_eq!(file.read(&mut []), Ok(0));
}

#[test]
fn the_bytes_past_the_size_in_the_last_cluster_are_never_returned() {
    let mut volume = Volume::open(with_file(&[5])).unwrap();
    let mut file = FileReader::new(&mut volume, 5, 10);
    let mut buffer = [0xEE; 512];
    assert_eq!(file.read(&mut buffer), Ok(10));
    assert!(buffer[10..].iter().all(|b| *b == 0xEE));
}

#[test]
fn seeking_moves_forwards_and_backwards_across_clusters() {
    let clusters = [9, 30, 4, 17];
    let mut volume = Volume::open(with_file(&clusters)).unwrap();
    let size = 4 * 512;
    let mut file = FileReader::new(&mut volume, 9, size);
    let mut byte = [0u8; 1];
    for position in [1500usize, 3, 1024, 2047, 0, 600, 1536, 511, 512, 2047] {
        file.seek(position as u32);
        assert_eq!(file.position(), position as u32);
        assert_eq!(file.read(&mut byte), Ok(1));
        assert_eq!(byte[0], expected(&clusters, position), "at {position}");
    }
    file.seek(u32::MAX);
    assert_eq!(file.position(), size);
    assert_eq!(file.read(&mut byte), Ok(0));
    file.seek(0);
    assert_eq!(file.read(&mut byte), Ok(1));
    assert_eq!(byte[0], expected(&clusters, 0));
}

#[test]
fn a_chain_longer_than_the_size_is_read_only_as_far_as_the_size() {
    let mut volume = Volume::open(with_file(&[5, 6, 7])).unwrap();
    let mut file = FileReader::new(&mut volume, 5, 600);
    let mut buffer = [0u8; 2000];
    assert_eq!(file.read(&mut buffer), Ok(600));
}

#[test]
fn a_chain_shorter_than_the_size_fails_after_the_bytes_it_has() {
    let mut volume = Volume::open(with_file(&[5, 6])).unwrap();
    let mut file = FileReader::new(&mut volume, 5, 2000);
    let mut buffer = [0u8; 512];
    assert_eq!(file.read(&mut buffer), Ok(512));
    assert_eq!(file.read(&mut buffer), Ok(512));
    assert_eq!(
        file.read(&mut buffer),
        Err(FileReadError::ShorterThanItsSize)
    );
    assert_eq!(file.read(&mut buffer), Err(FileReadError::AfterAFailure));
}

#[test]
fn a_looping_chain_ends_with_the_walks_bound() {
    let mut image = Image::fat12();
    image.set(20, 20);
    let mut volume = Volume::open(image).unwrap();
    // 350 clusters of 512 bytes, and one more than the volume holds.
    let mut file = FileReader::new(&mut volume, 20, 351 * 512);
    let mut buffer = [0u8; 512];
    let mut reads = 0;
    let error = loop {
        match file.read(&mut buffer) {
            Ok(512) => reads += 1,
            Ok(other) => panic!("a short read of {other}"),
            Err(error) => break error,
        }
    };
    assert_eq!(reads, 350);
    assert_eq!(error, FileReadError::Chain(ChainError::TooLong));
}

#[test]
fn a_first_cluster_that_is_not_a_data_cluster_is_an_error_unless_the_file_is_empty() {
    for first in [0u32, 1, 352, u32::MAX] {
        let mut volume = Volume::open(Image::fat12()).unwrap();
        let mut file = FileReader::new(&mut volume, first, 5);
        let mut buffer = [0u8; 8];
        assert_eq!(
            file.read(&mut buffer),
            Err(FileReadError::Chain(ChainError::NotADataCluster(first)))
        );
        let mut empty = FileReader::new(&mut volume, first, 0);
        assert_eq!(empty.read(&mut buffer), Ok(0));
    }
}

#[test]
fn a_failing_device_ends_the_reader() {
    let mut image = with_file(&[5, 6]);
    image.refuse = Some(FIRST_DATA as u64 + 4);
    let mut volume = Volume::open(image).unwrap();
    let mut file = FileReader::new(&mut volume, 5, 1024);
    let mut buffer = [0u8; 512];
    assert_eq!(file.read(&mut buffer), Ok(512));
    assert_eq!(
        file.read(&mut buffer),
        Err(FileReadError::Chain(ChainError::Device(
            FIRST_DATA as u64 + 4
        )))
    );
    assert_eq!(file.read(&mut buffer), Err(FileReadError::AfterAFailure));
}

#[test]
fn open_file_refuses_a_directory_and_the_label_and_opens_a_file() {
    let mut image = with_file(&[5]);
    let entries: [([u8; 11], u8, u16, u32); 3] = [
        (*b"FILE    TXT", 0x20, 5, 100),
        (*b"DIR        ", 0x10, 6, 0),
        (*b"LABEL      ", 0x08, 0, 0),
    ];
    for (index, (name, attributes, cluster, size)) in entries.iter().enumerate() {
        let at = 5 * 512 + index * 32;
        image.bytes[at..at + 11].copy_from_slice(name);
        image.bytes[at + 11] = *attributes;
        image.bytes[at + 26..at + 28].copy_from_slice(&cluster.to_le_bytes());
        image.bytes[at + 28..at + 32].copy_from_slice(&size.to_le_bytes());
    }
    let mut volume = Volume::open(image).unwrap();
    let mut found = [None, None, None];
    for (slot, entry) in found.iter_mut().zip(volume.root_entries()) {
        *slot = Some(entry.unwrap());
    }
    let [Some(file), Some(directory), Some(label)] = found else {
        panic!("three entries");
    };
    assert!(volume.open_file(&directory).is_err());
    assert!(volume.open_file(&label).is_err());
    let mut reader = volume.open_file(&file).unwrap();
    let mut buffer = [0u8; 200];
    assert_eq!(reader.read(&mut buffer), Ok(100));
    assert_eq!(buffer[0], pattern(5, 0));
}
