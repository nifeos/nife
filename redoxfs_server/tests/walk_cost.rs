//! **What a directory walk costs the FS server, counted** (milestone 606 (a directory walk costs
//! what it does on Linux), provisional number).
//!
//! The on-target walk (`walk_pricing`, run by `std_exerciser`) is timed and cannot be asserted:
//! its figures carry the machine. What the server does per walk can be, because it is
//! deterministic: the requests a walk sends and the device reads they cost. `walk_model` replays
//! the walk `std`'s PAL sends; this asserts on what it cost.

use file_system_contract::directory;
use file_system_fixture::walk as tree;
use redoxfs::{DiskMemory, FileSystem};
use redoxfs_server::walk_model::{self, Counting};
use redoxfs_server::{CachedDisk, NameCopy, Server};
/// A fixed name as the [`NameCopy`] every `Server` verb takes; the server copies a client's names
/// out of its window the same way.
fn named(name: &str) -> NameCopy {
    NameCopy::copy_in(name.as_bytes()).expect("a fixed name fits one block and is UTF-8")
}

/// A fresh image holding the priced tree, reopened the way the EL0 binary opens its disk.
fn staged() -> Server<CachedDisk<Counting<DiskMemory>>> {
    let disk = DiskMemory::new(32 * 1024 * 1024);
    let fs = FileSystem::create(disk, None, 0, 0).expect("create");
    let mut srv = Server::open(fs.disk).expect("open the fresh image");
    walk_model::stage(&mut srv, file_system_contract::file_system::ROOT as u32)
        .expect("stage the tree");
    let disk = srv.into_disk();
    Server::open(CachedDisk::new(
        Counting::new(disk),
        redoxfs_server::CACHE_SLOTS,
    ))
    .expect("reopen")
}

/// **The replay visits exactly the fixture**, so the counts below are for the walk the target
/// times and not for some other one.
#[test]
fn the_replayed_walk_visits_the_fixture() {
    let mut srv = staged();
    let root = srv
        .open_dir(
            0,
            &named(tree::ROOT),
            directory::rights::ENUMERATE | directory::rights::READ | directory::rights::DESCEND,
        )
        .expect("open the tree");
    let w = walk_model::walk(&mut srv, root).expect("walk");
    assert_eq!(w.entries, tree::WALK_ENTRIES);
    assert_eq!(w.files, tree::WALK_FILES);
    assert_eq!(w.bytes, tree::WALK_BYTES);
    assert_eq!(w.components, tree::WALK_COMPONENTS);
}

/// **A second walk reads nothing from the device.** The whole tree, names, nodes and bytes, is
/// about 1.5 MiB of blocks, and the server's cache holds it: so a repeated walk is answered from
/// memory, which is what a Linux page cache and dentry cache do for a repeated `find` or `rg`.
/// Before milestone 606 the cache held 64 blocks and every file of the walk went to the device.
#[test]
fn a_warm_walk_reads_nothing_from_the_device() {
    let mut srv = staged();
    let root = srv
        .open_dir(
            0,
            &named(tree::ROOT),
            directory::rights::ENUMERATE | directory::rights::READ | directory::rights::DESCEND,
        )
        .expect("open the tree");
    walk_model::walk(&mut srv, root).expect("the cold walk");
    let before = srv.disk_mut().inner().reads;
    walk_model::walk(&mut srv, root).expect("the warm walk");
    let after = srv.disk_mut().inner().reads;
    assert_eq!(
        after - before,
        0,
        "a warm walk went to the device {} times",
        after - before
    );
}
