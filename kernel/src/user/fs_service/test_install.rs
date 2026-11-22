//! The kernel test harness installs a package as an installer would.

use super::*;

/// Install `bytes` as the one program of package `name` at `version`, as an installer would: the
/// bytes at `packages/<name>/<version>/<program>`, and a generation `1` in `activation/` naming them
/// with their digest, made live by `current`. Anything already there is overwritten, and a
/// generation this writes replaces the whole table, which no test here needs more of.
pub fn install_for_test(program: &str, name: &str, version: &str, bytes: &[u8]) {
    use file_system_contract::{PAGE_BYTES, directory, file_system};
    let (fs_ep, fs_page_frame) = root_directory(
        blk_server_image(),
        crate::user::program("redoxfs_server").expect("no redoxfs_server in the initial ramdisk"),
    )
    .expect("wired() already brought the file service up");
    // SAFETY: as in `set_file`: the file service's own shared page, idle between logins.
    let page = unsafe {
        core::slice::from_raw_parts_mut(mmu::phys_to_virt(fs_page_frame) as *mut u8, PAGE_BYTES)
    };
    let call = |w: [u64; 2]| crate::sched::ipc_call(fs_ep, w)[0] as i64;
    let named = |page: &mut [u8], verb: u64, at: u64, name: &str, w1: u64| {
        page[..name.len()].copy_from_slice(name.as_bytes());
        call([file_system::request(verb, at, name.len() as u64), w1])
    };
    // A directory under `at`, made if it is not there.
    let dir = |page: &mut [u8], at: u64, name: &str| {
        let made = named(page, file_system::MKDIR, at, name, directory::rights::ALL);
        if made >= 0 {
            return made as u64;
        }
        let h = named(page, file_system::OPENDIR, at, name, directory::rights::ALL);
        assert!(h >= 0, "could not make or open {name} ({h})");
        h as u64
    };
    // `contents` as the whole of `file` under `at`, a page at a time.
    let put = |page: &mut [u8], at: u64, file: &str, contents: &[u8]| {
        let mut h = named(page, file_system::OPEN, at, file, 0);
        if h < 0 {
            h = named(page, file_system::CREATE, at, file, 0);
        }
        assert!(h >= 0, "could not open or create {file} ({h})");
        let h = h as u64;
        assert_eq!(
            call([file_system::request(file_system::TRUNCATE, h, 0), 0]),
            0,
            "could not truncate {file}"
        );
        for (k, chunk) in contents.chunks(PAGE_BYTES).enumerate() {
            page[..chunk.len()].copy_from_slice(chunk);
            let wrote = call([
                file_system::request(file_system::WRITE, h, chunk.len() as u64),
                (k * PAGE_BYTES) as u64,
            ]);
            assert_eq!(wrote, chunk.len() as i64, "short write of {file}");
        }
        call([file_system::request(file_system::CLOSE, h, 0), 0]);
    };
    let packages = dir(page, file_system::ROOT, installed_programs::PACKAGES);
    let n = dir(page, packages, name);
    let v = dir(page, n, version);
    put(page, v, program, bytes);
    for h in [v, n, packages] {
        call([file_system::request(file_system::CLOSE, h, 0), 0]);
    }

    // The row and its pointer: the bytes' path is its package and version columns, and its
    // manager is `jig`.
    let entry = installed_programs::Entry {
        program,
        version,
        package: name,
        digest: measured_boot::sha256(bytes),
        manager: "jig",
    };
    let mut table = [0u8; 512];
    let len = installed_programs::with_entry(
        "",
        &entry,
        crate::user::program(program).is_some(),
        &mut table,
    )
    .expect("one entry fits");
    let mut current = [0u8; 16];
    let current_len = installed_programs::format_current(1, &mut current).expect("fits");
    let mut digits = [0u8; 10];
    let first = installed_programs::generation_name(1, &mut digits);
    let activation = dir(page, file_system::ROOT, installed_programs::DIRECTORY);
    put(page, activation, first, &table[..len]);
    put(
        page,
        activation,
        installed_programs::CURRENT,
        &current[..current_len],
    );
    call([file_system::request(file_system::CLOSE, activation, 0), 0]);
}
