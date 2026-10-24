//! **Writes through the block chunking**: repeated, record-sized, after a reopen and after a
//! shorter write, each of which once looped or truncated.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// **Repeat writes through the EL0 binary's exact chunking.** The device's `IpcDisk` and this
/// `BlockDisk` split `Disk` calls the same way, so if the chunking is what breaks a repeat write
/// (the partial-block read-modify-write is the suspicious part, and compression plus a growing
/// allocator log exercise it more on later writes), it breaks here, on the host, in milliseconds.
#[test]
fn repeat_writes_through_the_block_chunking_do_not_loop() {
    let mut fs = FileSystem::create(BlockDisk(VecIo(vec![0u8; 16 * 1024 * 1024])), None, 0, 0)
        .expect("mkfs through BlockDisk");
    fs.tx(|tx| {
        let ptr = tx
            .create_node(TreePtr::root(), "scratch", Node::MODE_FILE | 0o644, 0, 0)?
            .ptr();
        tx.write_node(ptr, 0, b"(placeholder)", 0, 0)?;
        Ok(())
    })
    .expect("populate");
    let image = fs.disk.0.0;

    let mut srv = Server::open(BlockDisk(VecIo(image))).expect("Server::open");
    let h = srv.open_file(&named("scratch")).expect("open scratch");
    let mut buf = [0u8; 128];
    for pass in 1..=3u8 {
        let payload = repeat_write_payload(pass);
        assert_eq!(srv.write(h, 0, &payload).unwrap(), payload.len());
        let n = srv.read(h, 0, &mut buf).unwrap();
        assert_eq!(&buf[..n], &payload[..], "pass {pass} through the chunking");
    }
}

/// Repeat writes of a **multi-record** payload, so the chunking's multi-block path and the
/// compressed-record partial tail both get exercised across generations.
///
/// **The length is derived from the record size rather than written down**, and milestone 138 is
/// why. This used to say `160 * 1024`, chosen because it was 1.25 of the 128 KiB record of the
/// day; at the 8 KiB record that milestone moved to, the same constant is exactly twenty whole
/// records and the partial tail this test exists to reach **stops existing**, silently, with the
/// test still passing. Two whole records plus one block is that shape at any level, so the
/// coverage cannot be lost by a constant moving somewhere else.
#[test]
fn repeat_record_sized_writes_through_the_chunking_do_not_loop() {
    let mut fs = FileSystem::create(BlockDisk(VecIo(vec![0u8; 32 * 1024 * 1024])), None, 0, 0)
        .expect("mkfs");
    fs.tx(|tx| {
        tx.create_node(TreePtr::root(), "big", Node::MODE_FILE | 0o644, 0, 0)?;
        Ok(())
    })
    .expect("populate");
    let image = fs.disk.0.0;

    let mut srv = Server::open(BlockDisk(VecIo(image))).expect("open");
    let h = srv.open_file(&named("big")).expect("open big");
    let len = 2 * (redoxfs::BLOCK_SIZE << redoxfs::RECORD_LEVEL) as usize + BLOCK;
    for pass in 1..=3u8 {
        // Position-dependent and pass-dependent, and deliberately not compressible to a tiny
        // size, so the record path does real work each pass.
        let data: Vec<u8> = (0..len)
            .map(|i| ((i as u32).wrapping_mul(2_654_435_761) >> 16) as u8 ^ pass)
            .collect();
        assert_eq!(srv.write(h, 0, &data).unwrap(), len);
        let mut back = vec![0u8; len];
        let n = srv.read(h, 0, &mut back).unwrap();
        assert_eq!(n, len, "pass {pass} short read");
        assert!(back == data, "pass {pass} record data mismatch");
    }
}

#[test]
fn a_write_persists_and_reads_back() {
    let mut srv = server_with(&[("scratch", &[0u8; 32])]);
    let h = srv.open_file(&named("scratch")).unwrap();
    let payload = b"CRKWRIT1 and then some position-dependent bytes here";
    let n = srv.write(h, 0, payload).unwrap();
    assert_eq!(n, payload.len());
    let mut buf = [0u8; 128];
    let m = srv.read(h, 0, &mut buf).unwrap();
    assert_eq!(&buf[..m], payload);
    assert_eq!(srv.fstat(h).unwrap(), payload.len() as u64);
}

/// The payload for pass `n` of the repeat-write test: a fixed 64 bytes, tagged with the pass and
/// position-dependent after that, so every pass has the same length (no truncation confusion) and
/// a stale or shifted read cannot match. Shared with the on-device client through
/// `filesystem_fixture` where that matters.
pub(super) fn repeat_write_payload(pass: u8) -> [u8; 64] {
    let mut p = [0u8; 64];
    p[..8].copy_from_slice(b"CRKRPT__");
    p[7] = b'0' + pass;
    for (i, b) in p.iter_mut().enumerate().skip(8) {
        *b = (i as u8).wrapping_mul(31) ^ pass;
    }
    p
}

/// **The repeat-write gate** (fix/redoxfs-repeat-write). A first write to a pristine block
/// worked all along; a write to a block that has ALREADY been written is what loops on device.
/// The old gate never saw it because `mkredoxfs` rewrites the target to a placeholder before
/// every run, so every gated write was a first write and the bug hid behind the harness.
///
/// This writes the same file TWICE in one run, which is the honest reproduction: it depends on
/// nothing left over from a previous invocation. It is also the decisive host-vs-device
/// comparison: if this loops, the bug is reachable with no nife runtime at all (upstream or
/// our chunking); if it passes, the divergence is ours, in the device I/O path.
#[test]
fn a_second_write_to_the_same_block_does_not_loop() {
    let mut srv = server_with(&[("scratch", b"(placeholder)")]);
    let h = srv.open_file(&named("scratch")).unwrap();

    // Equal-length, position-dependent payloads: each write fully replaces the last (a shorter
    // write at offset 0 does not truncate, which is correct filesystem behaviour but would muddy
    // the comparison), and a buffer that came back shifted or stale cannot match.
    let mut buf = [0u8; 128];
    for pass in 1..=3u8 {
        let payload = repeat_write_payload(pass);
        assert_eq!(
            srv.write(h, 0, &payload).unwrap(),
            payload.len(),
            "write {pass} was short"
        );
        let n = srv.read(h, 0, &mut buf).unwrap();
        assert_eq!(&buf[..n], &payload[..], "write {pass} did not read back");
    }
}

/// **The real trigger** (fix/redoxfs-repeat-write): write, drop the mount WITHOUT a clean
/// unmount, reopen, and write again. Every existing test stopped short of this. The one that
/// looked closest, `a_write_survives_a_full_close_and_reopen`, only READS after the reopen.
///
/// This is exactly what the gate does across its two ISA legs: `mkredoxfs` runs once, the aarch64
/// leg mounts and writes (a first write, which works), the process dies without unmounting, and
/// then the riscv leg mounts the same image and writes again. That second boot's write is what
/// fails, which is why the bug looked like "repeat write" and why one leg passing hid it.
#[test]
fn a_write_after_a_reopen_of_a_previously_written_image_does_not_loop() {
    let mut disk = image_with(&[("scratch", b"(placeholder)")]);

    // Boot 1: mount, write, and drop the mount the way a dying process does (no unmount).
    {
        let mut srv = Server::open(disk).expect("open 1");
        let h = srv.open_file(&named("scratch")).expect("open scratch 1");
        let p1 = repeat_write_payload(1);
        assert_eq!(srv.write(h, 0, &p1).unwrap(), p1.len());
        disk = srv.fs.disk; // take the disk back; no unmount, as on device
    }

    // Boot 2: mount the image boot 1 wrote, and write again.
    let mut srv = Server::open(disk).expect("open 2");
    let h = srv.open_file(&named("scratch")).expect("open scratch 2");
    let p2 = repeat_write_payload(2);
    assert_eq!(srv.write(h, 0, &p2).unwrap(), p2.len(), "boot-2 write");
    let mut buf = [0u8; 128];
    let n = srv.read(h, 0, &mut buf).unwrap();
    assert_eq!(&buf[..n], &p2[..], "boot-2 write did not read back");
}

/// A write shorter than the file leaves the previous write's tail in place: correct behavior, and
/// the mechanism behind the "cross-boot second-mount write failure" (fix/redoxfs-second-mount). One
/// boot's client left a 64-byte payload in `scratch`; the next boot's `std::fs` test wrote 61 bytes,
/// compared the whole file against them and got 64 back. The write succeeded every time
/// (notes/fs-server/write-path-history.md). `TRUNCATE` is what replaces a file's contents. The byte
/// counts here are the real ones, so the arithmetic is legible.
#[test]
fn a_shorter_write_does_not_truncate_and_that_is_what_broke_across_boots() {
    let mut srv = server_with(&[("scratch", b"(placeholder)")]);
    let h = srv.open_file(&named("scratch")).unwrap();

    // Boot 1's client wrote 64 bytes.
    let long = [b'L'; 64];
    assert_eq!(srv.write(h, 0, &long).unwrap(), 64);
    assert_eq!(srv.fstat(h).unwrap(), 64);

    // Boot 2's std::fs wrote its 61-byte pattern over it. The write itself is fine.
    let short = [b'S'; 61];
    assert_eq!(srv.write(h, 0, &short).unwrap(), 61);

    // And the file is STILL 64 bytes, because nothing truncates it.
    assert_eq!(
        srv.fstat(h).unwrap(),
        64,
        "a shorter write must not truncate; TRUNCATE is the verb that fixes this, not WRITE"
    );
    let mut buf = [0u8; 128];
    let n = srv.read(h, 0, &mut buf).unwrap();
    assert_eq!(n, 64, "a whole-file read returns the OLD length");
    assert_eq!(&buf[..61], &short[..], "the new bytes landed");
    assert_eq!(
        &buf[61..64],
        b"LLL",
        "and the longer write's tail survives, which is what failed the comparison"
    );
}

#[test]
fn a_write_survives_a_full_close_and_reopen() {
    // Persistence across a full close/reopen is what the on-disk image buys, and it is the
    // property the kill-mid-write test leans on: a reopen sees the committed write.
    let disk = image_with(&[("f", b"")]);
    let mut srv = Server::open(disk).unwrap();
    let h = srv.open_file(&named("f")).unwrap();
    srv.write(h, 0, b"persisted").unwrap();
    let disk = srv.fs.disk; // the FileSystem committed on each tx; take the disk back
    let mut srv = Server::open(disk).unwrap();
    let h = srv.open_file(&named("f")).unwrap();
    let mut buf = [0u8; 16];
    let n = srv.read(h, 0, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"persisted");
}
