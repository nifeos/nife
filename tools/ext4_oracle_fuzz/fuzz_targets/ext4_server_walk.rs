//! Fuzz `ext4_server` with the walk `ext4_oracle_server` makes, over a corrupted corpus image.
//!
//! An input picks a generated image and a few corruptions of it (`ext4_oracle_fuzz::image`). The
//! target then walks the tree from `ROOT`, breadth first, as a client would: list each directory, open each entry,
//! read its size, its first bytes and its attributes, and descend. The walk is bounded
//! ([`MAXIMUM_DIRECTORIES`], [`MAXIMUM_READ_BYTES`]) so a corrupted image that claims a huge tree
//! costs a run time, not a timeout.
//!
//! # The rules it checks, beyond "no panic"
//!
//! 1. *Listings.* Every entry `READDIR` returns decodes, and is one name: never `.`, `..`, empty,
//!    holding `/` or NUL, or the attribute store's reserved name.
//! 2. *Handles.* Every `OPEN` and `OPENDIR` that succeeds mints a handle in `1..=MAXIMUM_HANDLE`
//!    not already held, and every handle the walk minted closes.
//! 3. *Sizes.* `FSTAT` on a file answers the size `OPEN` answered, and no `READ` returns more
//!    bytes than it asked for.
//! 4. *Bounds.* Every error word carries a code in `1..=MAXIMUM_ERRNO`.
//!
//! Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation; calef names things.

#![no_main]

use std::collections::{BTreeSet, VecDeque};

use ext4_oracle_fuzz::{Served, image, request, result, serve};
use file_system_contract::directory::rights;
use file_system_contract::{directory_entry, extended_attribute, file_system};
use libfuzzer_sys::arbitrary::{Result, Unstructured};
use libfuzzer_sys::fuzz_target;

/// The most directories one walk lists.
const MAXIMUM_DIRECTORIES: usize = 64;
/// The most entries one walk visits.
const MAXIMUM_ENTRIES: usize = 128;
/// The most bytes the walk reads from one file.
const MAXIMUM_READ_BYTES: u64 = 16 * 1024;
/// The most listing pages the walk reads from one directory.
const MAXIMUM_LISTING_PAGES: usize = 16;

struct Client {
    server: Served,
    window: Vec<u8>,
    held: BTreeSet<u64>,
}

type Reply = core::result::Result<u64, i32>;

impl Client {
    fn call(&mut self, code: u64, handle: u64, name: &[u8], len: u64, w1: u64) -> (Reply, u64) {
        self.window[..name.len()].copy_from_slice(name);
        let (r0, r1) = self
            .server
            .handle(request(code, handle, len), w1, &mut self.window);
        (result(r0), r1)
    }

    fn mint(&mut self, handle: u64) {
        assert!(
            (1..=file_system::MAXIMUM_HANDLE).contains(&handle),
            "minted handle {handle} is out of range"
        );
        assert!(
            self.held.insert(handle),
            "minted handle {handle} is already held"
        );
    }

    fn close(&mut self, handle: u64) {
        let (reply, _) = self.call(file_system::CLOSE, handle, &[], 0, 0);
        assert_eq!(
            reply,
            Ok(0),
            "handle {handle}, minted by this walk, did not close"
        );
        self.held.remove(&handle);
    }

    fn entries(&mut self, handle: u64) -> Vec<(Vec<u8>, bool)> {
        let mut all = Vec::new();
        for _ in 0..MAXIMUM_LISTING_PAGES {
            let (reply, _) = self.call(file_system::READDIR, handle, &[], 0, all.len() as u64);
            let Ok(filled) = reply else { break };
            let filled = filled as usize;
            assert!(filled <= self.window.len(), "READDIR filled {filled} bytes");
            if filled == 0 {
                break;
            }
            let before = all.len();
            for (name, is_directory) in directory_entry::iter(&self.window[..filled]) {
                assert!(
                    !name.is_empty()
                        && name != b"."
                        && name != b".."
                        && !name.contains(&b'/')
                        && !name.contains(&0)
                        && name != extended_attribute::STORE_DIRECTORY.as_bytes(),
                    "READDIR listed {name:?}"
                );
                all.push((name.to_vec(), is_directory));
            }
            if all.len() == before {
                break;
            }
        }
        all
    }

    fn attributes(&mut self, handle: u64) {
        let (reply, _) = self.call(file_system::LISTXATTR, handle, &[], 0, 0);
        let Ok(filled) = reply else { return };
        let names: Vec<Vec<u8>> = extended_attribute::list::iter(&self.window[..filled as usize])
            .map(<[u8]>::to_vec)
            .collect();
        for name in names {
            let _ = self.call(file_system::GETXATTR, handle, &name, name.len() as u64, 0);
        }
    }

    fn file(&mut self, parent: u64, name: &[u8]) {
        let (reply, size) = self.call(
            file_system::OPEN,
            parent,
            name,
            name.len() as u64,
            rights::READ,
        );
        let Ok(handle) = reply else { return };
        self.mint(handle);
        if let Ok(stat) = self.call(file_system::FSTAT, handle, &[], 0, 0).0 {
            assert_eq!(stat, size, "FSTAT answered {stat}, OPEN answered {size}");
        }
        let mut at = 0;
        while at < MAXIMUM_READ_BYTES {
            let ask = file_system::MAXIMUM_TRANSFER_BYTES as u64;
            let Ok(n) = self.call(file_system::READ, handle, &[], ask, at).0 else {
                break;
            };
            assert!(n <= ask, "READ of {ask} answered {n}");
            if n == 0 {
                break;
            }
            at += n;
        }
        self.attributes(handle);
        self.close(handle);
    }

    fn walk(&mut self) {
        let mut pending = VecDeque::from([file_system::ROOT]);
        let mut directories = 0;
        let mut visited = 0;
        while let Some(directory) = pending.pop_front() {
            if directories == MAXIMUM_DIRECTORIES {
                if directory != file_system::ROOT {
                    self.close(directory);
                }
                continue;
            }
            directories += 1;
            let _ = self.call(file_system::GET_VOLUME_STATISTICS, directory, &[], 0, 0);
            self.attributes(directory);
            for (name, is_directory) in self.entries(directory) {
                if visited == MAXIMUM_ENTRIES {
                    break;
                }
                visited += 1;
                let _ = self.call(
                    file_system::GET_MODIFICATION_TIME_UNIX_SECONDS,
                    directory,
                    &name,
                    name.len() as u64,
                    0,
                );
                if is_directory {
                    let (reply, _) = self.call(
                        file_system::OPENDIR,
                        directory,
                        &name,
                        name.len() as u64,
                        rights::PARENT_RIGHTS,
                    );
                    if let Ok(child) = reply {
                        self.mint(child);
                        pending.push_back(child);
                    }
                } else {
                    self.file(directory, &name);
                }
            }
            if directory != file_system::ROOT {
                self.close(directory);
            }
        }
        assert!(
            self.held.is_empty(),
            "the walk left {} handles held",
            self.held.len()
        );
    }
}

fn run(u: &mut Unstructured) -> Result<()> {
    let image = image(u)?;
    let Some(server) = serve(image) else {
        return Ok(());
    };
    Client {
        server,
        window: vec![0; file_system::MAXIMUM_TRANSFER_BYTES],
        held: BTreeSet::new(),
    }
    .walk();
    Ok(())
}

fuzz_target!(|data: &[u8]| {
    let _ = run(&mut Unstructured::new(data));
});
