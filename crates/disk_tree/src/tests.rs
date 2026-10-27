//! Host tests for the disk tree's namespace and block arithmetic.
//!
//! Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.

extern crate std;

use super::*;

const SIZE: u64 = 256 << 20;
const BS: usize = blk::BLOCK_SIZE;

fn call(t: &mut Tree, code: u64, handle: u64, name: &[u8], w1: u64) -> Answer {
    let mut page = [0u8; fs::TRANSFER_MAX];
    page[..name.len()].copy_from_slice(name);
    t.handle(fs::req(code, handle, name.len() as u64), w1, &mut page)
}

fn read(t: &mut Tree, handle: u64, len: u64, offset: u64) -> Answer {
    let mut page = [0u8; fs::TRANSFER_MAX];
    t.handle(fs::req(fs::READ, handle, len), offset, &mut page)
}

fn handle_of(a: Answer) -> u64 {
    match a {
        Answer::Reply(h, _) if h >= 0 => h as u64,
        other => panic!("expected a handle, got {other:?}"),
    }
}

/// The walk a C program behind a directory grant of [`DIRECTORY`] makes: its caretaker's one
/// `OPENDIR`, then `OPEN` of the file, which answers the disk's size in the second word.
fn opened(t: &mut Tree) -> u64 {
    let d = handle_of(call(t, fs::OPENDIR, fs::ROOT, DIRECTORY, dir::READ));
    match call(t, fs::OPEN, d, FILE, 0) {
        Answer::Reply(h, size) if h >= 0 => {
            assert_eq!(size, SIZE);
            h as u64
        }
        other => panic!("OPEN refused: {other:?}"),
    }
}

#[test]
fn ioping_at_one_mebibyte_reads_block_256_from_the_device() {
    let mut t = Tree::new(SIZE);
    let f = opened(&mut t);
    assert_eq!(
        call(&mut t, fs::FSTAT, f, b"", 0),
        Answer::Reply(SIZE as i64, 0)
    );
    assert_eq!(
        read(&mut t, f, BS as u64, 1 << 20),
        Answer::Read(Read {
            block: 256,
            blocks: 1,
            skip: 0,
            len: BS
        })
    );
}

/// The property the whole tree exists for: nothing answers a read from memory. The same read
/// twice is two device reads, where `redoxfs_server` would answer the second from its memo.
#[test]
fn the_same_read_twice_goes_to_the_device_twice() {
    let mut t = Tree::new(SIZE);
    let f = opened(&mut t);
    let first = read(&mut t, f, BS as u64, 8 * BS as u64);
    assert!(matches!(first, Answer::Read(_)));
    assert_eq!(read(&mut t, f, BS as u64, 8 * BS as u64), first);
}

#[test]
fn an_unaligned_read_spans_the_blocks_it_touches() {
    let mut t = Tree::new(SIZE);
    let f = opened(&mut t);
    assert_eq!(
        read(&mut t, f, BS as u64, 3 * BS as u64 + 100),
        Answer::Read(Read {
            block: 3,
            blocks: 2,
            skip: 100,
            len: BS
        })
    );
}

#[test]
fn a_read_comes_back_short_at_the_window_and_at_the_end() {
    let mut t = Tree::new(SIZE);
    let f = opened(&mut t);
    let window = blk::TRANSFER_BLOCKS * BS;
    assert_eq!(
        read(&mut t, f, fs::TRANSFER_MAX as u64, 10),
        Answer::Read(Read {
            block: 0,
            blocks: blk::TRANSFER_BLOCKS,
            skip: 10,
            len: window - 10
        })
    );
    assert_eq!(
        read(&mut t, f, BS as u64, SIZE - 10),
        Answer::Read(Read {
            block: SIZE / BS as u64 - 1,
            blocks: 1,
            skip: BS - 10,
            len: 10
        })
    );
    assert_eq!(read(&mut t, f, BS as u64, SIZE), Answer::Reply(0, 0));
}

#[test]
fn a_path_reaches_the_file_from_the_root() {
    let mut t = Tree::new(SIZE);
    let path = [DIRECTORY, b"/", FILE].concat();
    assert!(
        matches!(call(&mut t, fs::OPEN, fs::ROOT, &path, dir::READ), Answer::Reply(h, SIZE) if h > 0)
    );
    assert_eq!(
        call(&mut t, fs::OPEN, fs::ROOT, b"nvme0/../n1", dir::READ),
        err(dir::EINVAL)
    );
}

#[test]
fn nothing_writes() {
    let mut t = Tree::new(SIZE);
    let f = opened(&mut t);
    for code in [fs::WRITE, fs::TRUNCATE] {
        assert_eq!(call(&mut t, code, f, b"", 0), err(dir::EROFS));
    }
    for code in [fs::CREATE, fs::MKDIR, fs::UNLINK, fs::RMDIR, fs::SETMTIME] {
        assert_eq!(call(&mut t, code, fs::ROOT, b"x", 0), err(dir::EROFS));
    }
    // A descent cannot ask for a right the root never had.
    assert_eq!(
        call(&mut t, fs::OPENDIR, fs::ROOT, DIRECTORY, dir::WRITE),
        err(dir::EPERM)
    );
}

#[test]
fn a_grant_without_read_cannot_open_the_file() {
    let mut t = Tree::new(SIZE);
    let d = handle_of(call(
        &mut t,
        fs::OPENDIR,
        fs::ROOT,
        DIRECTORY,
        dir::ENUMERATE,
    ));
    assert_eq!(call(&mut t, fs::OPEN, d, FILE, 0), err(ENOENT));
}

#[test]
fn listings_name_what_is_there() {
    let mut t = Tree::new(SIZE);
    let mut page = [0u8; fs::TRANSFER_MAX];
    let Answer::Reply(n, _) = t.handle(fs::req(fs::READDIR, fs::ROOT, 0), 0, &mut page) else {
        panic!()
    };
    let got: std::vec::Vec<_> = dirent::iter(&page[..n as usize]).collect();
    assert_eq!(got, [(DIRECTORY, true)]);
    assert_eq!(
        t.handle(fs::req(fs::READDIR, fs::ROOT, 0), 1, &mut page),
        Answer::Reply(0, 0)
    );
}

/// The C library's reachability probe must get a server's refusal, not one of the kernel's
/// "not granted" errors (-1, -2, -3), or the program concludes it holds no directory.
#[test]
fn the_reachability_probe_is_refused_as_a_bad_handle() {
    let mut t = Tree::new(SIZE);
    assert_eq!(call(&mut t, fs::FSTAT, fs::MAX_HANDLE, b"", 0), err(EBADF));
    assert_eq!(call(&mut t, fs::CLOSE, fs::ROOT, b"", 0), err(dir::EINVAL));
}

/// Names that are not in the tree are not there, at either level, and the two levels are not
/// interchangeable.
#[test]
fn only_the_two_names_resolve() {
    let mut t = Tree::new(SIZE);
    assert_eq!(
        call(&mut t, fs::OPENDIR, fs::ROOT, b"nope", dir::READ),
        err(ENOENT)
    );
    assert_eq!(
        call(&mut t, fs::OPEN, fs::ROOT, b"n1", dir::READ),
        err(ENOENT)
    );
    let d = handle_of(call(&mut t, fs::OPENDIR, fs::ROOT, DIRECTORY, dir::READ));
    assert_eq!(call(&mut t, fs::OPEN, d, b"nope", 0), err(ENOENT));
    assert_eq!(
        call(&mut t, fs::OPEN, fs::ROOT, DIRECTORY, dir::READ),
        err(dir::EISDIR)
    );
    // A descent from `nvme0` needs `DESCEND` on it: without, `n1` is not there; with, it is a file.
    assert_eq!(call(&mut t, fs::OPENDIR, d, FILE, dir::READ), err(ENOENT));
    let walkable = dir::READ | dir::DESCEND;
    let d = handle_of(call(&mut t, fs::OPENDIR, fs::ROOT, DIRECTORY, walkable));
    assert_eq!(
        call(&mut t, fs::OPENDIR, d, FILE, dir::READ),
        err(dir::ENOTDIR)
    );
}

/// A path's every step is checked: an empty, `.` or `..` component is `EINVAL` wherever it is,
/// and a descent through the path asks for `DESCEND` beside what the caller asked.
#[test]
fn a_path_is_checked_at_every_step() {
    let mut t = Tree::new(SIZE);
    for name in [&b"."[..], b"..", b"nvme0/", b"../n1", b"./n1", b"/n1"] {
        assert_eq!(
            call(&mut t, fs::OPEN, fs::ROOT, name, dir::READ),
            err(dir::EINVAL),
            "{name:?}"
        );
    }
    // Through `nvme0` asking `READ | DESCEND`, the last step needs `DESCEND` on `nvme0`, so the
    // answer is about `n1` being a file rather than about a right the walk dropped.
    let both = dir::READ | dir::DESCEND;
    assert_eq!(
        call(&mut t, fs::OPENDIR, fs::ROOT, b"nvme0/n1", both),
        err(dir::ENOTDIR)
    );
}

/// The name rides in the page, at most 256 bytes and never more than the page holds.
#[test]
fn a_name_longer_than_the_buffer_or_the_page_is_refused() {
    let mut t = Tree::new(SIZE);
    let long = [b'x'; 256];
    assert_eq!(
        call(&mut t, fs::OPEN, fs::ROOT, &long, dir::READ),
        err(ENOENT)
    );
    let longer = [b'x'; 257];
    assert_eq!(
        call(&mut t, fs::OPEN, fs::ROOT, &longer, dir::READ),
        err(dir::EINVAL)
    );
    let mut page = *b"nvme0";
    let w0 = fs::req(fs::OPENDIR, fs::ROOT, 5);
    assert!(matches!(t.handle(w0, dir::READ, &mut page), Answer::Reply(h, _) if h > 0));
    let w0 = fs::req(fs::OPENDIR, fs::ROOT, 6);
    assert_eq!(t.handle(w0, dir::READ, &mut page), err(dir::EINVAL));
}

/// What the verbs that are neither a read nor a write answer.
#[test]
fn the_other_verbs_answer_what_a_read_only_disk_can_say() {
    let mut t = Tree::new(SIZE);
    let f = opened(&mut t);
    assert_eq!(call(&mut t, fs::SYNC, f, b"", 0), Answer::Reply(0, 0));
    assert_eq!(call(&mut t, fs::LISTXATTR, f, b"", 0), Answer::Reply(0, 0));
    assert_eq!(
        call(&mut t, fs::GETXATTR, f, b"user.x", 0),
        err(xattr::ENODATA)
    );
    assert_eq!(
        call(&mut t, fs::GETMTIME, fs::ROOT, DIRECTORY, 0),
        err(EOPNOTSUPP)
    );
    assert_eq!(
        call(&mut t, fs::FSTAT, fs::ROOT, b"", 0),
        Answer::Reply(0, 0)
    );
    assert_eq!(call(&mut t, 0xfe, fs::ROOT, b"", 0), err(dir::EINVAL));
    assert_eq!(read(&mut t, fs::ROOT, BS as u64, 0), err(dir::EISDIR));
    assert_eq!(read(&mut t, f, 0, 0), Answer::Reply(0, 0));
    let mut page = [0u8; 64];
    assert_eq!(
        t.handle(fs::req(fs::READDIR, f, 0), 0, &mut page),
        err(dir::ENOTDIR)
    );
}

/// A listing needs `ENUMERATE`, and its absence is a refusal rather than an empty directory.
#[test]
fn a_listing_without_enumerate_is_refused() {
    let mut t = Tree::new(SIZE);
    let d = handle_of(call(&mut t, fs::OPENDIR, fs::ROOT, DIRECTORY, dir::READ));
    let mut page = [0u8; 64];
    assert_eq!(
        t.handle(fs::req(fs::READDIR, d, 0), 0, &mut page),
        err(dir::EPERM)
    );
}

/// A closed handle is gone, the root cannot be closed, and a full table says so on `OPEN`
/// without a size.
#[test]
fn handles_close_and_run_out() {
    let mut t = Tree::new(SIZE);
    let f = opened(&mut t);
    assert_eq!(call(&mut t, fs::CLOSE, f, b"", 0), Answer::Reply(0, 0));
    assert_eq!(call(&mut t, fs::FSTAT, f, b"", 0), err(EBADF));
    let d = handle_of(call(&mut t, fs::OPENDIR, fs::ROOT, DIRECTORY, dir::READ));
    while matches!(call(&mut t, fs::OPEN, d, FILE, 0), Answer::Reply(h, _) if h >= 0) {}
    assert_eq!(call(&mut t, fs::OPEN, d, FILE, 0), err(EMFILE));
}
