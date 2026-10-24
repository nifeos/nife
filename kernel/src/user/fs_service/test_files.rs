//! Test-only reads and writes at the file service's root, as the owner's console or a
//! session would have made them.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// Write one of the owner's lists at the file service's root, as the owner's console would: the
/// run-unvouched list, the suspended list or the durable-session manifest. `None` removes it.
#[cfg(any(test, feature = "system_tests"))]
pub fn set_root_list(file: &str, list: Option<&[u8]>) {
    set_file(None, file, list);
}

/// Write `file` inside `identity`'s own subtree (§117 (a principal's subtree is named
/// by its identity string)), which must already exist: an identity's stored schedule (§122 (the
/// on-disk schedule store)), as that identity's session would have written it. `None` removes it.
#[cfg(any(test, feature = "system_tests"))]
pub fn set_home_file(identity: &[u8], file: &str, contents: Option<&[u8]>) {
    set_file(Some(identity), file, contents);
}

/// [`set_root_list`] and [`set_home_file`]'s one body: `file` at the root, or under the subtree
/// named `dir`.
#[cfg(any(test, feature = "system_tests"))]
fn set_file(dir: Option<&[u8]>, file: &str, contents: Option<&[u8]>) {
    use filesystem_contract::{dir as rights, fs};
    let (fs_ep, fs_page_frame) = root_directory(
        blk_server_image(),
        crate::user::program("redoxfs_server").expect("no redoxfs_server in the initial ramdisk"),
    )
    .expect("wired() already brought the file service up");
    // SAFETY: as in `ensure_home_subtree`: the file service's own shared page, idle between logins.
    let page = unsafe {
        core::slice::from_raw_parts_mut(
            mmu::phys_to_virt(fs_page_frame) as *mut u8,
            filesystem_contract::PAGE,
        )
    };
    let call = |w: [u64; 2]| crate::sched::ipc_call(fs_ep, w)[0] as i64;
    let at = match dir {
        None => fs::ROOT,
        Some(d) => {
            page[..d.len()].copy_from_slice(d);
            let h = call([fs::req(fs::OPENDIR, fs::ROOT, d.len() as u64), rights::ALL]);
            assert!(
                h >= 0,
                "could not open the subtree {:?} ({h})",
                core::str::from_utf8(d)
            );
            h as u64
        }
    };
    let name = file.as_bytes();
    let named = |page: &mut [u8], verb: u64| {
        page[..name.len()].copy_from_slice(name);
        call([fs::req(verb, at, name.len() as u64), 0])
    };
    if let Some(contents) = contents {
        let mut h = named(page, fs::OPEN);
        if h < 0 {
            h = named(page, fs::CREATE);
        }
        assert!(h >= 0, "could not open or create {file} ({h})");
        let h = h as u64;
        assert_eq!(
            call([fs::req(fs::TRUNCATE, h, 0), 0]),
            0,
            "could not truncate {file}"
        );
        page[..contents.len()].copy_from_slice(contents);
        let wrote = call([fs::req(fs::WRITE, h, contents.len() as u64), 0]);
        assert_eq!(wrote, contents.len() as i64, "short write of {file}");
        call([fs::req(fs::CLOSE, h, 0), 0]);
    } else {
        // `ENOENT` is the state asked for, so the answer is not checked.
        named(page, fs::UNLINK);
    }
    if at != fs::ROOT {
        call([fs::req(fs::CLOSE, at, 0), 0]);
    }
}

/// Read one of the owner's files at the file service's root into `out`, [`set_root_list`]'s
/// other half: the byte count, or `None` when there is no such file.
#[cfg(any(test, feature = "system_tests"))]
pub fn read_root_file(file: &str, out: &mut [u8]) -> Option<usize> {
    use filesystem_contract::fs;
    let (fs_ep, fs_page_frame) = root_directory(
        blk_server_image(),
        crate::user::program("redoxfs_server").expect("no redoxfs_server in the initial ramdisk"),
    )?;
    // SAFETY: as in `set_root_list`: the file service's own shared page, idle between logins.
    let page = unsafe {
        core::slice::from_raw_parts_mut(
            mmu::phys_to_virt(fs_page_frame) as *mut u8,
            filesystem_contract::PAGE,
        )
    };
    page[..file.len()].copy_from_slice(file.as_bytes());
    let h = crate::sched::ipc_call(fs_ep, [fs::req(fs::OPEN, fs::ROOT, file.len() as u64), 0])[0]
        as i64;
    if h < 0 {
        return None;
    }
    let want = out.len().min(filesystem_contract::PAGE) as u64;
    let n = crate::sched::ipc_call(fs_ep, [fs::req(fs::READ, h as u64, want), 0])[0] as i64;
    crate::sched::ipc_call(fs_ep, [fs::req(fs::CLOSE, h as u64, 0), 0]);
    let n = usize::try_from(n).ok()?.min(out.len());
    out[..n].copy_from_slice(&page[..n]);
    Some(n)
}
