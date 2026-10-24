//! **The edits std's own source takes**: the size hint and the `target_os = "nife"` dispatch arms.
//!
//! A child of `farm` (§282 (when a file is split, the parent becomes an index)): what was private
//! to `farm` is `pub(super)` here, which reaches the same modules.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// Insert `text` immediately after the first occurrence of `anchor` in `path`.
pub(super) fn patch_after(path: &Path, anchor: &str, insert: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("std-src: cannot read {}", path.display());
        return false;
    };
    let Some(pos) = text.find(anchor) else {
        eprintln!(
            "std-src: anchor not found in {} (std internals changed?): {anchor:?}",
            path.display()
        );
        return false;
    };
    let at = pos + anchor.len();
    let new = format!(
        "{}\n{}\n{}",
        &text[..at],
        insert.trim_end_matches('\n'),
        &text[at..]
    );
    if let Err(e) = std::fs::write(path, new) {
        eprintln!("std-src: cannot write {}: {e}", path.display());
        return false;
    }
    true
}

/// Replace every occurrence of `old` in `path` with `new`, failing loudly if there are not exactly
/// `count`: the same tripwire as [`patch_after`], for the one patch that has to change a line rather
/// than add one.
pub(super) fn patch_replace(path: &Path, old: &str, new: &str, count: usize) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("std-src: cannot read {}", path.display());
        return false;
    };
    let found = text.matches(old).count();
    if found != count {
        eprintln!(
            "std-src: expected {count} of {old:?} in {}, found {found} (std internals changed?)",
            path.display()
        );
        return false;
    }
    if let Err(e) = std::fs::write(path, text.replace(old, new)) {
        eprintln!("std-src: cannot write {}: {e}", path.display());
        return false;
    }
    true
}

/// **Size a whole-file read from `OPEN`'s reply rather than an `FSTAT`** (milestone 606 (a
/// directory walk costs what it does on Linux), calef's ruling B, form 2, 2026-09-27).
///
/// `std::fs::read`, `read_to_string` and `File::read_to_end` size their buffer from
/// `file.metadata()`, which on nife is one more message per file. Since ruling B the server's
/// `OPEN` reply carries the size, and the PAL keeps it as a hint (`File::open_size_hint`). This
/// routes those three buffer sizings through a helper that asks the hint first and falls back to
/// `metadata()`. `metadata()` itself is untouched and always asks the server: calef's reason is
/// that a stale hint costs at most one resize, while a stale `metadata()` lies to the program.
pub(super) fn std_patch_size_hint() -> bool {
    let fs = farm_std_src().join("fs.rs");
    let read_size =
        "let size = file.metadata().map(|m| usize::try_from(m.len()).unwrap_or(usize::MAX)).ok();";
    patch_replace(
        &fs,
        read_size,
        "let size = file_size_hint(&file).map(|n| usize::try_from(n).unwrap_or(usize::MAX));",
        2,
    ) && patch_replace(
        &fs,
        "fn buffer_capacity_required(mut file: &File) -> Option<usize> {\n    let size = file.metadata().map(|m| m.len()).ok()?;",
        "/// nife (milestone 606, ruling B): the size `OPEN` reported, else `metadata()`.\nfn file_size_hint(file: &File) -> Option<u64> {\n    #[cfg(target_os = \"nife\")]\n    if let Some(size) = file.inner.open_size_hint() {\n        return Some(size);\n    }\n    file.metadata().map(|m| m.len()).ok()\n}\n\nfn buffer_capacity_required(mut file: &File) -> Option<usize> {\n    let size = file_size_hint(file)?;",
        1,
    )
}

/// Add a `target_os = "nife"` arm to std's `cfg_select!` dispatchers so they pick the nife
/// backend, and add nife to std's `build.rs` known-platform chain (so std is not
/// `restricted_std` and ordinary programs need no `#![feature]`). These string anchors couple us
/// to the pinned nightly's std internals; a rustc bump that reshapes them fails loudly here, which
/// is the intended tripwire (see notes/std.md).
pub(super) fn std_patch_dispatch() -> bool {
    let sys = farm_std_src().join("sys");
    DISPATCH_ARMS.iter().all(|edit| edit.apply(&sys))
        && patch_after(
            // nife has a real PAL: not restricted_std.
            &farm_std_src().parent().unwrap().join("build.rs"),
            "        || target_os == \"vexos\"\n",
            "        || target_os == \"nife\"",
        )
}

/// One edit [`std_patch_dispatch`] makes to a file under std's `sys/`, named relative to it.
enum SysEdit {
    /// [`patch_after`]: the file, the anchor, and what goes immediately after it.
    After(&'static str, &'static str, &'static str),
    /// [`patch_replace`]: the file, the text, what replaces it, and how many times it must occur.
    Replace(&'static str, &'static str, &'static str, usize),
}

impl SysEdit {
    fn apply(&self, sys: &Path) -> bool {
        match *self {
            SysEdit::After(file, anchor, insert) => patch_after(&sys.join(file), anchor, insert),
            SysEdit::Replace(file, old, new, count) => {
                patch_replace(&sys.join(file), old, new, count)
            }
        }
    }
}

/// **The `target_os = "nife"` arms**, in the order [`std_patch_dispatch`] applies them; the first
/// that fails stops the rest.
const DISPATCH_ARMS: &[SysEdit] = &[
    SysEdit::After(
        "pal/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        pub(crate) mod nife;\n        pub use self::nife::*;\n    }",
    ),
    // `pub(crate)` so a thread's end can spill its allocation cache (milestone 561), from
    // `sys/thread_local/key/nife.rs`.
    SysEdit::After(
        "alloc/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        pub(crate) mod nife;\n        use nife as imp;\n    }",
    ),
    SysEdit::After(
        "stdio/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::*;\n    }",
    ),
    // random: `fill_bytes` AND `hashmap_random_keys`, because milestone 56 splits them. The
    // first promises cryptographic strength and panics without the entropy capability; the
    // second is a hash seed and degrades to the old counter-seeded stream. Exporting both means
    // std's blanket `hashmap_random_keys` (the `#[cfg(not(any(...)))]` fallback at the bottom of
    // the same file) must exclude nife, or the two definitions collide; that is the next
    // patch, and it is anchored on the wasi line because "xous" appears twice in the file.
    SysEdit::After(
        "random/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::{fill_bytes, hashmap_random_keys};\n    }",
    ),
    SysEdit::After(
        "random/mod.rs",
        "    all(target_os = \"wasi\", not(target_env = \"p1\")),",
        "    target_os = \"nife\",",
    ),
    SysEdit::After(
        "thread/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::{Thread, available_parallelism, current_os_id, set_name, sleep, yield_now, DEFAULT_MIN_STACK_SIZE};\n    }",
    ),
    SysEdit::After(
        "time/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        use nife as imp;\n    }",
    ),
    // net: TcpStream + outbound UdpSocket over the net_stack socket contract (milestone 27 phase
    // two). The first cfg_select in connection/mod.rs is the backend dispatcher; the nife
    // arm precedes the `_ =>` unsupported fallback that phase one used. hostname has its own
    // `_ =>` fallback to unsupported, so it needs no arm.
    SysEdit::After(
        "net/connection/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::*;\n    }",
    ),
    // fs: File open/read/metadata over the FS-service contract (milestone 27 phase two). The
    // arm precedes the `_ =>` unsupported fallback phase one used, and mirrors the shape of
    // the other single-backend arms (`use nife as imp`).
    // `pub(crate) mod` rather than `mod`: `sys/paths/nife.rs` asks `fs::nife::is_reachable()`
    // whether this process holds a directory capability, because `current_dir` must refuse for
    // a process that holds none rather than name a place it cannot reach (milestone 47).
    SysEdit::After(
        "fs/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        pub(crate) mod nife;\n        use nife as imp;\n    }",
    ),
    // env: a process-local variable table (milestone 64, rank 4). The arm precedes the `_ =>`
    // unsupported fallback, whose `env()` is `panic!("not supported on this platform")`: without
    // this, `std::env::vars()` aborted the process rather than yielding nothing. `sys/env/nife.rs`
    // defines its own `Env` instead of reusing `sys/env/common.rs`, so this is the only anchor
    // env costs us; `common` is gated on a `#[cfg(any(...))]` platform list that would have been
    // a second one to keep in step across nightlies.
    SysEdit::After(
        "env/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::*;\n    }",
    ),
    // args: the byte argv (milestone 205, DECISIONS §170). The arm precedes the `_ =>`
    // unsupported fallback, whose `args()` yields nothing, which is where unmodified `ripgrep`
    // stopped. `sys/args/nife.rs` defines its own `Args`, as `sys/env/nife.rs` does, because
    // `sys/args/common.rs` is gated on a platform list that would be a second anchor.
    SysEdit::After(
        "args/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::*;\n    }",
    ),
    // paths: `temp_dir`, `split_paths` and `join_paths` (milestone 64). The arm precedes the
    // `_ =>` unsupported fallback, whose `temp_dir()` is `panic!("no filesystem on this
    // platform")` and whose `split_paths()` is `panic!("unsupported")`: without this,
    // `std::env::temp_dir()` aborted the process, which is what `tempfile` reached before it
    // ever got to its own "not supported" arm. `getcwd`, `chdir`, `current_exe` and `home_dir`
    // keep refusing, in `sys/paths/nife.rs` rather than by falling through, so one file holds
    // both halves and a reader meets the reasons together.
    SysEdit::After(
        "paths/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        use nife as imp;\n    }",
    ),
    // process: `getpid` only (milestone 64). Everything else stays the shared `unsupported`
    // backend, which refuses honestly; `getpid` alone was `panic!("no pids on this platform")`,
    // so `std::process::id()` killed the program. The arm is spelled as a split `imp` rather
    // than a whole nife backend because `unsupported.rs` opens with `use super::env::...`, so
    // it cannot be pulled in through a `#[path]` module the way `sys/fs/nife.rs` does.
    SysEdit::After(
        "process/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        #[allow(dead_code)]\n        mod unsupported;\n        mod nife;\n        mod imp {\n            pub use super::nife::getpid;\n            pub use super::unsupported::{\n                ChildPipe, Command, CommandArgs, EnvKey, ExitCode, ExitStatus, ExitStatusError,\n                Process, Stdio, output, read_output,\n            };\n        }\n    }",
    ),
    // exit (milestone 64 (enough `std` to run somebody else's crate)): `sys/exit.rs` is not a
    // `sys/<module>/mod.rs` dispatcher; its `cfg_select!` sits inside `pub fn exit`, and the `_ =>`
    // arm is `crate::intrinsics::abort()`, which the kernel reports as a fault. This arm does what
    // `_start` already does on `main`'s return, the same `SYS_EXIT` with the same code (which the
    // kernel discards). `a_whole_std_program_runs_on_the_native_abi` asserts exit, not fault. See
    // notes/std/caveats.md.
    SysEdit::After(
        "exit.rs",
        "pub fn exit(code: i32) -> ! {\n    cfg_select! {",
        "        target_os = \"nife\" => {\n            crate::sys::pal::nife::rt::exit(code as i64)\n        }",
    ),
    // io/error has no fallback arm; route nife to the generic backend.
    SysEdit::After(
        "io/error/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod generic;\n        pub use generic::*;\n    }",
    ),
    // Single-threaded, no native TLS: storage is a plain static (no_threads).
    SysEdit::After(
        "thread_local/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod os;\n        pub(crate) use os::{LocalPointer, local_pointer};\n        pub use os::{Storage, thread_local_inner, value_align};\n    }",
    ),
    // ... and the TLS-destructor guard is a no-op.
    SysEdit::After(
        "thread_local/mod.rs",
        "pub(crate) mod guard {\n    cfg_select! {",
        "        target_os = \"nife\" => {\n            pub(crate) fn enable() {}\n        }",
    ),
    // Thread-local keys through the thread pointer (milestone 812 (`std::thread::spawn` runs
    // real threads in one address space)): xous's shape, `sys/thread_local/key/nife.rs`.
    SysEdit::After(
        "thread_local/mod.rs",
        "pub(crate) mod key {\n    cfg_select! {",
        "        target_os = \"nife\" => {\n            mod racy;\n            pub(crate) mod nife;\n            pub(super) use racy::LazyKey;\n            pub(super) use nife::{Key, get, set};\n            use nife::{create, destroy};\n        }",
    ),
    // ... and reachable from `sys/thread/nife.rs`, whose join waits on the word a thread's
    // `SYS_EXIT_THREAD` clears (milestone 812).
    SysEdit::Replace("sync/mod.rs", "\nmod futex;", "\npub(crate) mod futex;", 1),
    // The futex every lock below is built on (milestone 812): `sys/sync/futex/nife.rs`.
    SysEdit::After(
        "sync/futex/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod nife;\n        pub use nife::*;\n    }",
    ),
    SysEdit::After(
        "sync/mutex/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod futex;\n        pub use futex::Mutex;\n    }",
    ),
    SysEdit::After(
        "sync/condvar/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod futex;\n        pub use futex::Condvar;\n    }",
    ),
    SysEdit::After(
        "sync/rwlock/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod futex;\n        pub use futex::RwLock;\n    }",
    ),
    SysEdit::After(
        "sync/once/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod futex;\n        pub use futex::{Once, OnceState};\n    }",
    ),
    SysEdit::After(
        "sync/thread_parking/mod.rs",
        "cfg_select! {",
        "    target_os = \"nife\" => {\n        mod futex;\n        pub use futex::Parker;\n    }",
    ),
    // std::env::consts::OS. `cfg_unordered!` turns each arm's cfg into the fallback's
    // exclusion set, so adding a nife arm both defines OS and keeps the fallback off it.
    SysEdit::After(
        "env_consts.rs",
        "cfg_unordered! {",
        "#[cfg(target_os = \"nife\")]\npub mod os {\n    pub const FAMILY: &str = \"\";\n    pub const OS: &str = \"nife\";\n    pub const DLL_PREFIX: &str = \"\";\n    pub const DLL_SUFFIX: &str = \"\";\n    pub const DLL_EXTENSION: &str = \"\";\n    pub const EXE_SUFFIX: &str = \"\";\n    pub const EXE_EXTENSION: &str = \"\";\n}",
    ),
];
