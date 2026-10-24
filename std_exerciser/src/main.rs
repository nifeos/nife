//! The std proof (milestone 27): an ordinary Rust program, no `no_std`, running on the native
//! capability ABI. `println!` SENDs on slot 1, collections draw from the budget at slot 0,
//! `Instant` reads the virtual counter, `SystemTime` the clock page at slot 5 (milestone 51),
//! `std::random` the entropy service at slot 6 (milestone 56), and `std::env`'s `TZ`, `LANG` and
//! `TERM` the inert-configuration page at slot 7 (milestone 47's environment-variable fork,
//! DECISIONS §111).
//!
//! The two `#![feature]`s are upstream's API stability, not this platform's: `std::random`
//! (rust-lang/rust#130703) and `std::fs::Dir` (rust-lang/rust#120426) are unstable on every target.
#![feature(dirfd, random)]
//!
//! **One binary, its behavior chosen by the authority it was granted** (no ambient authority,
//! DECISIONS §10, §25, §27). A directory in slot 4 runs `fs_demo`; a subtree rather than the root
//! runs `walk_transcript` (milestone 121 (`ripgrep` on nife: enumeration as a capability)); the
//! network in slots 2 and 3 runs `net_demo`, which serves if its stack carries a listen grant; and
//! neither runs `offline_demo`, where `fs` and `net` are honestly `Unsupported`. Slots 5 to 7 are
//! granted on every run. The kernel suite checks each transcript byte for byte;
//! notes/std/the-proof.md has what each branch proves.

use std::collections::HashMap;
use std::fs::{Dir, File};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::random::{Rng, SystemRng};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use socket_contract::fixture;

fn main() {
    // `thread::sleep` blocks on a kernel timer (milestone 106 (a wait that ends on either the
    // interrupt or the deadline)), retyped from slot 0 on this first call. Silent when it works; a
    // sleep that returns early panics, and the transcript no longer matches.
    let before = Instant::now();
    std::thread::sleep(Duration::from_millis(20));
    assert!(
        before.elapsed() >= Duration::from_millis(20),
        "thread::sleep returned early"
    );

    // `Unsupported` means no FS-service endpoint in slot 4 and no ambient filesystem to fall back
    // on. Any other failure means a directory was granted, so it must not fall through to the net.
    match File::open(filesystem_fixture::MOTD_NAME) {
        Ok(f) => return fs_demo(f),
        Err(e) if e.kind() == ErrorKind::Unsupported => {}
        // A directory, and not the image root: the only narrower grant any spawn hands this
        // program is milestone 121's priced tree, so that is what it must be.
        Err(e) if e.kind() == ErrorKind::NotFound => return walk_transcript(),
        Err(e) => panic!("a directory capability was granted but the motd would not open: {e:?}"),
    }

    // Without the `Stack` endpoint and a frame untyped this is `Unsupported` (the RETYPE of a
    // shared frame fails with no untyped in slot 3), and the offline transcript runs instead.
    match UdpSocket::bind("0.0.0.0:0") {
        Ok(sock) => net_demo(sock),
        Err(_) => offline_demo(),
    }
}

/// The phase-one transcript: collections on the untyped heap, `Instant`, and the honest refusal of
/// `fs` and `net`. Runs when the program was not granted the network.
fn offline_demo() {
    let t0 = Instant::now();

    println!("hello from std on nife");
    println!("os {}", std::env::consts::OS);
    // The argv (milestone 205 (how a foreign program is told what to do), DECISIONS §170 (how a
    // foreign program is told what to do)): the argument page at slot 8, or `[]` when none was
    // granted, which is the kernel harness's case.
    println!("args {:?}", std::env::args_os().collect::<Vec<_>>());

    // Vec: growth reallocations against the untyped-backed heap.
    let v: Vec<u64> = (0..10_000).map(|i| i * 3).collect();
    let sum: u64 = v.iter().sum();
    println!("vec sum {sum}");

    // String: heap bytes whose length the receiver checks.
    let mut s = String::new();
    for _ in 0..100 {
        s.push_str("nife ");
    }
    println!("string len {}", s.len());

    // HashMap: exercises the platform RandomState seed (sys/random) plus many small allocations.
    let mut m = HashMap::new();
    for k in 0u64..100 {
        m.insert(k, k * k);
    }
    println!("map lookup {}", m[&37]);

    // The honesty checks: the platform must refuse, not pretend.
    match std::fs::File::open("/init") {
        Err(e) if e.kind() == ErrorKind::Unsupported => println!("fs honestly unsupported"),
        other => println!("fs lied: {other:?}"),
    }
    match TcpStream::connect("127.0.0.1:80") {
        Err(e) if e.kind() == ErrorKind::Unsupported => println!("net honestly unsupported"),
        other => println!("net lied: {other:?}"),
    }

    // Instant: monotonic and advancing, but asserted rather than printed (a printed duration
    // would make the transcript nondeterministic).
    let t1 = Instant::now();
    assert!(t1 >= t0, "the virtual counter went backwards");
    assert!(
        t1.duration_since(t0).as_nanos() > 0,
        "no time passed across real work"
    );
    println!("instant monotonic ok");

    // Wall-clock time (milestone 51), from the clock page at slot 5. Asserted rather than printed,
    // because a date is not a deterministic transcript, against the clock service's own window.
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the wall clock is before the Unix epoch");
    assert!(
        wall >= NOT_BEFORE && wall < NOT_AFTER,
        "the wall clock reads {wall:?}, outside the plausible window",
    );
    // And the property the counter-plus-offset design buys: `SystemTime` and `Instant` are read
    // from the same counter, so the wall clock advancing does not mean the monotonic one jumped.
    let t2 = Instant::now();
    assert!(t2 >= t1, "the monotonic counter went backwards");
    println!("wall clock ok");

    // Real entropy (milestone 56), from the entropy service at slot 6. Asserted, not printed. Two
    // 32-byte draws agree with probability 2^-256 from a real source, always from a counter seed.
    let mut a = [0u8; 32];
    let mut b = [0u8; 32];
    SystemRng.fill_bytes(&mut a);
    SystemRng.fill_bytes(&mut b);
    assert_ne!(a, b, "two draws from std::random are identical");
    assert!(a.iter().any(|&x| x != 0), "a draw is all zeros");
    println!("entropy ok");

    // Inert configuration (milestone 47's environment-variable fork, DECISIONS §111): before
    // `main` ran, `pal::nife::init` seeded `TZ`, `LANG` and `TERM` from the page at slot 7
    // (`sys/env/nife.rs::seed`). A process granted no such page is seeded with nothing.
    assert_eq!(
        std::env::var("TZ").as_deref(),
        Ok("UTC"),
        "TZ was not seeded from the granted inert-configuration page",
    );
    assert_eq!(
        std::env::var("LANG").as_deref(),
        Ok("C"),
        "LANG was not seeded from the granted inert-configuration page",
    );
    assert_eq!(
        std::env::var("TERM").as_deref(),
        Ok("dumb"),
        "TERM was not seeded from the granted inert-configuration page",
    );
    println!("config seeded");

    environment_holds_only_what_was_seeded();

    paths_answer_or_refuse_honestly();

    // `std::process::exit` is the only caller of `sys::exit::exit` in std, wired apart from a
    // return from `main` (notes/std/std-aborts.md). The `exiting` line proves the call was
    // reached: without it, a dropped exit would still pass the fault count in
    // `a_whole_std_program_runs_on_the_native_abi`. `rt::cleanup` runs first, so stdout is
    // flushed and the sink's end-of-stream still goes out.
    println!("exiting through process::exit");
    std::process::exit(0);
}

/// The `env ok` line of [`offline_demo`]'s transcript.
fn environment_holds_only_what_was_seeded() {
    // Three claims (milestone 64): `vars()` returns rather than panicking, it holds exactly the
    // three keys the config grant seeded and nothing ambient, and `set_var` takes.
    assert_eq!(
        std::env::vars().count(),
        3,
        "a nife process started with variables beyond what its grants seeded",
    );
    assert!(
        std::env::var("PATH").is_err(),
        "PATH resolved, so something is fabricating an ambient environment",
    );
    unsafe { std::env::set_var("NIFE_DEMO", "set-by-the-program") };
    assert_eq!(
        std::env::var("NIFE_DEMO").as_deref(),
        Ok("set-by-the-program"),
        "a variable this process set did not read back",
    );
    assert_eq!(
        std::env::vars().count(),
        4,
        "the variable this process set is not in its own listing",
    );
    unsafe { std::env::remove_var("NIFE_DEMO") };
    assert!(
        std::env::var("NIFE_DEMO").is_err(),
        "a removed variable still resolves",
    );
    println!("env ok");
}

/// The `paths ok` line of [`offline_demo`]'s transcript.
fn paths_answer_or_refuse_honestly() {
    // Milestone 64: `temp_dir`, `split_paths` and `process::id` answer rather than panic.
    // `process::id` is 0: nife issues no process identifier, and no Unix gives 0 to a user process.
    assert_eq!(
        std::process::id(),
        0,
        "a nife process reported a pid it was never issued",
    );
    // `temp_dir` is the held directory, steered by `TMPDIR` as on Unix, the seam milestone 47's
    // namespace arrives through; nothing seeds it yet, so the fallback answers.
    assert_eq!(
        std::env::temp_dir(),
        PathBuf::from("/"),
        "temp_dir is not the granted directory",
    );
    unsafe { std::env::set_var("TMPDIR", "scratch") };
    assert_eq!(
        std::env::temp_dir(),
        PathBuf::from("scratch"),
        "TMPDIR did not steer temp_dir",
    );
    unsafe { std::env::remove_var("TMPDIR") };
    // Round-tripped: a splitter that agrees with no joiner is worth less than none.
    let joined = std::env::join_paths(["a", "b", "c"]).expect("joining three plain names failed");
    let split: Vec<PathBuf> = std::env::split_paths(&joined).collect();
    assert_eq!(
        split,
        [PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")],
        "a path list did not survive a join and a split",
    );
    assert!(
        std::env::join_paths(["a:b"]).is_err(),
        "a segment containing the separator was joined into a list that reads back as two",
    );
    // The refusals that stay refusals. `current_dir` is the negative control for `fs_demo`'s: it
    // answers `/` only for a process holding a directory, and this one holds none.
    assert!(
        std::env::current_dir().is_err(),
        "current_dir answered for a process that holds no directory capability",
    );
    assert!(
        std::env::current_exe().is_err(),
        "current_exe answered, but nothing tells a nife process the path it was loaded from",
    );
    assert!(
        std::env::home_dir().is_none(),
        "home_dir answered, but nobody gave this program a home",
    );
    println!("paths ok");
}

/// The same sanity window `clock_contract::policy` applies, restated here because a std program links
/// std and not the contract crate. 2026-01-01 and 2100-01-01.
const NOT_BEFORE: Duration = Duration::from_secs(1_767_225_600);
const NOT_AFTER: Duration = Duration::from_secs(4_102_444_800);

/// **The `std::fs` transcript** (milestone 27 phase two, the FS half): `File`, `Read`,
/// `read_to_string` and `metadata`, served by the RedoxFS FS server over the §27 contract through
/// the one directory capability this process holds. `motd` is already open: opening it was the
/// probe that chose this branch.
///
/// A name means "under the directory I hold", so a path that would leave it is refused before a
/// byte reaches the server, as `InvalidFilename` and never `PermissionDenied`; an expressible name
/// that is absent is `NotFound`. See notes/std/the-proof.md.
fn fs_demo(mut motd: File) {
    println!("std fs on nife");

    // Printed as well as asserted, so the kernel test compares the bytes after the whole stack.
    let mut bytes = Vec::new();
    motd.read_to_end(&mut bytes)
        .expect("reading the motd through std::fs failed");
    assert_eq!(
        bytes,
        filesystem_fixture::MOTD,
        "std::fs read the wrong bytes off the image"
    );
    print!(
        "{}",
        String::from_utf8(bytes).expect("the motd is UTF-8 and ends in a newline")
    );
    drop(motd); // Drop CLOSEs the handle the server minted for us.

    // read_to_string reopens the same name and leans on the size hint the PAL answers with FSTAT.
    let text = std::fs::read_to_string(filesystem_fixture::MOTD_NAME)
        .expect("read_to_string through std::fs failed");
    println!("read_to_string {}", text.len());

    let meta =
        std::fs::metadata(filesystem_fixture::MOTD_NAME).expect("metadata through std::fs failed");
    assert!(meta.is_file(), "the motd is a regular file");
    println!("metadata len {}", meta.len());

    // `/` is the root of this process's namespace (milestone 47's namespace half), so `/motd` and
    // `motd` must read one file.
    let by_name =
        std::fs::read(filesystem_fixture::MOTD_NAME).expect("reading motd by name failed");
    let rooted = format!("/{}", filesystem_fixture::MOTD_NAME);
    let by_root = std::fs::read(&rooted).expect("reading motd by absolute path failed");
    assert_eq!(by_name, by_root, "`/motd` and `motd` named different files");
    // And `current_dir` says where that root is, in a spelling this process can type back.
    assert_eq!(
        std::env::current_dir().expect("current_dir refused for a process holding a directory"),
        PathBuf::from("/"),
        "current_dir is not this process's own root",
    );
    println!("absolute is my root");

    // The negative control: rooting a path grants nothing, because no wire message names a
    // directory's parent, so `/..` is refused exactly as `..` is.
    refused(
        &format!("/../{}", filesystem_fixture::MOTD_NAME),
        "absolute dotdot",
    );
    refused("../motd", "dotdot");
    // `..` is refused at every position (milestone 122): this names a file the root grant really
    // reaches, by an ascent no verb performs.
    refused(
        &format!(
            "{}/../{}/{}",
            filesystem_fixture::tree::SUB,
            filesystem_fixture::tree::OTHER,
            filesystem_fixture::tree::SECRET
        ),
        "inner dotdot",
    );

    match File::open("definitely-not-here") {
        Err(e) if e.kind() == ErrorKind::NotFound => println!("missing not found"),
        other => panic!("a missing name did not read as NotFound: {other:?}"),
    }

    // CREATE, then a SHORTER second write (milestone 31 phase 2): without TRUNCATE the first
    // write's tail would survive. §27's four corrections all trace to that, so it is pinned here.
    let long = b"the first write, deliberately the longer of the two";
    let short = b"the second write, shorter";
    assert!(
        short.len() < long.len(),
        "the shorter write must be shorter"
    );

    std::fs::write("made-by-std", long).expect("fs::write could not create a file");
    std::fs::write("made-by-std", short).expect("fs::write could not rewrite a file");
    let back = std::fs::read("made-by-std").expect("reading back what fs::write wrote failed");
    assert_eq!(
        back, short,
        "a shorter fs::write must REPLACE the contents, not leave the old tail",
    );
    println!("write create ok");

    // `create_new` on a name that now exists is AlreadyExists, not Unsupported and not a silent
    // overwrite. That distinction is the reason CREATE refuses rather than opening.
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open("made-by-std")
    {
        Err(e) if e.kind() == ErrorKind::AlreadyExists => println!("create_new refused"),
        other => panic!("create_new over an existing name did not refuse: {other:?}"),
    }

    // Creating outside the directory is refused as opening is, so CREATE did not widen reach.
    // `/tmp/escape` is an ordinary relative name under the namespace half; `/..` is the escape.
    for (path, what) in [("/../escape", "absolute"), ("../escape", "dotdot")] {
        match std::fs::write(path, b"x") {
            Err(e) if e.kind() != ErrorKind::PermissionDenied => {
                println!("create refused {what}")
            }
            other => panic!("creating an un-nameable path was not refused: {other:?}"),
        }
    }

    // Overwrites `scratch`, which the image ships; the host tool re-reads the image after the run
    // to prove the bytes reached the disk rather than a cache.
    let mut scratch = std::fs::OpenOptions::new()
        .write(true)
        .open(filesystem_fixture::SCRATCH_NAME)
        .expect("opening scratch for writing through std::fs failed");
    scratch
        .write_all(filesystem_fixture::WRITE_PATTERN)
        .expect("writing scratch through std::fs failed");
    drop(scratch);
    let back = std::fs::read(filesystem_fixture::SCRATCH_NAME)
        .expect("reading scratch back through std::fs failed");
    assert_eq!(
        back,
        filesystem_fixture::WRITE_PATTERN,
        "the write did not read back"
    );
    println!("write readback ok");

    namespace_transcript();
    descent_transcript();

    println!("fs ok");
}

/// **The namespace verbs** (milestone 64): `create_dir`, `read_dir`, `rename`, `remove_file`,
/// `remove_dir`, bound end to end on a real RedoxFS image.
///
/// It cleans up before it starts, not after: `NIFE_KEEP_REDOXFS=1` runs the suite against an image
/// a previous boot wrote (the cross-boot write case), so it must be idempotent, and the
/// directory's contents are then known when `read_dir` looks.
fn namespace_transcript() {
    const DIR: &str = "made-dir-by-std";
    const RENAMED: &str = "renamed-by-std";

    // Cleanup. `NotFound` is the expected answer on a fresh image and is not a failure; anything
    // else is, because it would mean the verb itself is broken rather than the name absent.
    for (name, removed) in [(RENAMED, false), (DIR, true)] {
        let r = if removed {
            std::fs::remove_dir(name)
        } else {
            std::fs::remove_file(name)
        };
        match r {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            other => panic!("cleaning up {name} failed: {other:?}"),
        }
    }

    std::fs::create_dir(DIR).expect("create_dir failed");
    println!("mkdir ok");

    // The listing must reach real image contents (the fixture's own name) and carry `dirent`'s
    // IS_DIR across the wire (the directory just made).
    let mut saw_motd = false;
    let mut saw_dir = false;
    for entry in std::fs::read_dir(".").expect("read_dir of the granted directory failed") {
        let entry = entry.expect("a directory entry did not decode");
        let name = entry.file_name();
        if name == filesystem_fixture::MOTD_NAME {
            saw_motd = true;
            assert!(
                entry.file_type().expect("file_type failed").is_file(),
                "the motd came back marked as a directory",
            );
        }
        if name == DIR {
            saw_dir = true;
            assert!(
                entry.file_type().expect("file_type failed").is_dir(),
                "a directory came back marked as a file",
            );
        }
    }
    assert!(saw_motd, "read_dir did not list the fixture's own file");
    assert!(saw_dir, "read_dir did not list the directory just created");
    println!("read_dir ok");

    // Descending is its own path: `OPENDIR` mints a capability and the listing runs through it.
    // No `.` or `..` entries: no capability designates what they would name.
    let n = std::fs::read_dir(DIR)
        .expect("read_dir of a subdirectory failed")
        .count();
    assert_eq!(n, 0, "a directory just created was not empty");
    println!("read_dir descend ok");

    // Neither remove verb removes the other's kind; no opcode means "remove whatever is there".
    match std::fs::remove_file(DIR) {
        Err(e) if e.kind() == ErrorKind::IsADirectory => println!("unlink refused a directory"),
        other => panic!("remove_file on a directory was not refused: {other:?}"),
    }
    match std::fs::remove_dir("made-by-std") {
        Err(e) if e.kind() == ErrorKind::NotADirectory => println!("rmdir refused a file"),
        other => panic!("remove_dir on a file was not refused: {other:?}"),
    }

    // Both halves of a rename, because a rename that copied would pass the first.
    std::fs::rename("made-by-std", RENAMED).expect("rename failed");
    let back = std::fs::read(RENAMED).expect("reading the renamed file failed");
    assert_eq!(
        back, b"the second write, shorter",
        "the renamed file did not carry the source's contents",
    );
    match File::open("made-by-std") {
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        other => panic!("the source name survived a rename: {other:?}"),
    }
    println!("rename ok");

    std::fs::remove_file(RENAMED).expect("remove_file failed");
    assert!(
        !std::fs::exists(RENAMED).expect("exists failed after remove_file"),
        "a removed name still resolves",
    );
    println!("unlink ok");

    // A directory can be stat'd although `OPEN` refuses one: `is_dir` and an idempotent
    // `create_dir_all` depend on it.
    assert!(
        std::path::Path::new(DIR).is_dir(),
        "a directory did not answer is_dir",
    );
    assert!(
        !std::path::Path::new(filesystem_fixture::MOTD_NAME).is_dir(),
        "a file answered is_dir",
    );
    assert!(
        std::path::Path::new(".").is_dir(),
        "the granted directory itself did not answer is_dir",
    );
    std::fs::create_dir_all(DIR).expect("create_dir_all over an existing directory failed");
    println!("is_dir ok");

    std::fs::remove_dir(DIR).expect("remove_dir failed");
    println!("rmdir ok");

    set_len_and_copy();
}

/// **`File::set_len` and `fs::copy`** (milestone 64, ranks 8 and 26), both of which the PAL refused
/// until this milestone and neither of which needed anything from the contract that was not there.
///
/// `set_len` is `TRUNCATE` with the size in the second word, which is the same message `File::open`
/// has been sending with a 0 in it since milestone 31. Both directions are checked, because
/// `ftruncate` grows as well as shrinks and a binding that only shrank would pass a shrink-only test.
///
/// `copy` needs no verb at all: it is an open, a read/write loop and two closes. What it proves that
/// a length check would not is that the destination gets the *source's bytes* rather than a file of
/// the right size, which is the way a copy loop with a bad offset fails.
fn set_len_and_copy() {
    const ORIGINAL: &str = "set-len-subject";
    const DUPLICATE: &str = "copy-destination";

    std::fs::write(ORIGINAL, b"0123456789").expect("write failed");

    // Shrink. The bytes past the new end must be gone, not merely unreported.
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(ORIGINAL)
        .expect("reopening for set_len failed");
    f.set_len(4).expect("set_len shrink failed");
    drop(f);
    assert_eq!(
        std::fs::read(ORIGINAL).expect("read after shrink failed"),
        b"0123",
        "set_len shrank the reported size without discarding the tail",
    );

    // Grow. POSIX extends with zeroes, and the contract's TRUNCATE promises the same, so the four
    // bytes must survive and the new tail must be NUL rather than whatever was there before.
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(ORIGINAL)
        .expect("reopening for set_len grow failed");
    f.set_len(8).expect("set_len grow failed");
    drop(f);
    assert_eq!(
        std::fs::read(ORIGINAL).expect("read after grow failed"),
        b"0123\0\0\0\0",
        "set_len grew the file with something other than zeroes",
    );
    println!("set_len ok");

    let n = std::fs::copy(ORIGINAL, DUPLICATE).expect("copy failed");
    assert_eq!(n, 8, "copy reported the wrong byte count");
    assert_eq!(
        std::fs::read(DUPLICATE).expect("read of the copy failed"),
        b"0123\0\0\0\0",
        "the copy does not hold the source's bytes",
    );
    // The source survives a copy. Trivially true and worth asserting once, because the way to
    // implement `copy` wrongly on a contract with a RENAME verb is to reach for the wrong one.
    assert!(
        std::fs::exists(ORIGINAL).expect("exists on the copy source failed"),
        "the source did not survive a copy",
    );
    println!("copy ok");

    std::fs::remove_file(ORIGINAL).expect("cleanup of the set_len subject failed");
    std::fs::remove_file(DUPLICATE).expect("cleanup of the copy destination failed");

    file_times();
}

/// The file [`file_times`] makes, sets and reads the times of.
const TIMES_SUBJECT: &str = "times-subject";
/// The directory [`file_times`] makes and sets the time of.
const TIMES_DIR: &str = "times-dir";

/// **File times** (milestone 64, ranks 19 and 28): `Metadata::modified` over `GETMTIME` and
/// `std::fs::set_times` over `SETMTIME_AT`, the verbs milestone 47's `touch` added (DECISIONS §112).
///
/// Four positives: the image's file reads a wall-clock time; a write moves a made file's time
/// forward (asserted before the set, since the engine only moves an mtime forward); a set time
/// reads back in whole seconds, sub-second truncated; and a directory and its listing entry agree.
/// Four refusals, each where a success would be a lie: an open handle, both ways; an access time;
/// and the granted directory itself. See notes/std/the-proof.md.
fn file_times() {
    use std::fs::FileTimes;

    // Cleanup first, for `namespace_transcript`'s reason: `NIFE_KEEP_REDOXFS=1` reuses an image.
    for (name, is_dir) in [(TIMES_SUBJECT, false), (TIMES_DIR, true)] {
        let r = if is_dir {
            std::fs::remove_dir(name)
        } else {
            std::fs::remove_file(name)
        };
        match r {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            other => panic!("cleaning up {name} failed: {other:?}"),
        }
    }

    let motd = std::fs::metadata(filesystem_fixture::MOTD_NAME)
        .expect("metadata of the motd failed")
        .modified()
        .expect("modified() refused for a name reached through the granted directory")
        .duration_since(UNIX_EPOCH)
        .expect("the motd's mtime is before 1970");
    assert!(
        motd >= NOT_BEFORE && motd < NOT_AFTER,
        "the motd's mtime is not a wall-clock second the host tool could have stamped: {motd:?}",
    );
    println!("mtime from the image ok");

    // A file this system makes is stamped by the FS server's own counter, so its time is small and
    // only its *order* means anything. What that order has to show is that a write moves it.
    std::fs::write(TIMES_SUBJECT, b"stamped").expect("write of the times subject failed");
    let made = std::fs::metadata(TIMES_SUBJECT)
        .expect("metadata of a file this program made failed")
        .modified()
        .expect("modified() refused for a file this program made");
    std::fs::write(TIMES_SUBJECT, b"written again").expect("the second write failed");
    let moved = std::fs::metadata(TIMES_SUBJECT)
        .expect("metadata after a write failed")
        .modified()
        .expect("modified() after a write failed");
    assert!(
        moved > made,
        "a write did not move the mtime forward: {made:?} then {moved:?}"
    );
    println!("write moves mtime ok");

    // 2027-01-15, and half a second, which the wire cannot carry.
    let asserted = UNIX_EPOCH + Duration::from_millis(1_800_000_000_500);
    std::fs::set_times(TIMES_SUBJECT, FileTimes::new().set_modified(asserted))
        .expect("set_times through a full-rights grant failed");
    let back = std::fs::metadata(TIMES_SUBJECT)
        .expect("metadata after set_times failed")
        .modified()
        .expect("modified() after set_times failed");
    assert_eq!(
        back,
        UNIX_EPOCH + Duration::from_secs(1_800_000_000),
        "set_times did not round-trip in whole seconds",
    );
    println!("set_times ok");

    a_directory_mtime_round_trips_and_its_listing_agrees();

    file_time_refusals(asserted, back);

    std::fs::remove_file(TIMES_SUBJECT).expect("cleanup of the times subject failed");
    std::fs::remove_dir(TIMES_DIR).expect("cleanup of the times directory failed");
}

/// **Descent** (milestone 122): a nested path is a chain of `OPENDIR`s, and a directory is a thing a
/// program can hold. Listing a subdirectory and then opening what it named, by the listed
/// `path()`, is the pair every walker needs.
///
/// Nothing widens the grant: each hop's rights are the last hop's intersected with the request,
/// and `sub/../other/secret`, reachable as `other/secret`, is refused because there is no ascent.
fn descent_transcript() {
    use filesystem_fixture::tree;

    // One descent, and the bytes are the fixture's own, so this is the whole stack again (disk,
    // block server, FS server, contract, PAL) with a directory capability minted in the middle.
    let inner = std::fs::read(format!("{}/{}", tree::SUB, tree::INNER))
        .expect("a file one directory down would not open");
    assert_eq!(
        inner,
        tree::INNER_BODY,
        "the descent read the wrong file's bytes"
    );
    println!("descend read ok");

    // Two descents. `deeper` exists in the fixture precisely so that a second one has somewhere to
    // go, and so that `dir::DESCEND` has something to withhold.
    let leaf = std::fs::read(format!("{}/{}/{}", tree::SUB, tree::DEEPER, tree::LEAF))
        .expect("a file two directories down would not open");
    assert_eq!(
        leaf,
        tree::LEAF_BODY,
        "the second descent read the wrong file"
    );
    println!("descend twice ok");

    // Not the fixture's `sub`: milestone 47's directory-capability attacker writes into it, so its
    // listing depends on test order (reading `sub/inner` is safe: the host check pins it). Building
    // it walks too: a nested `create_dir` is a walked `MKDIR`, a nested `write` a walked `CREATE`.
    const TREE: &str = "walked-by-std";
    match std::fs::remove_dir_all(TREE) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        other => panic!("cleaning up {TREE} failed: {other:?}"),
    }
    std::fs::create_dir(TREE).expect("create_dir failed");
    std::fs::create_dir(format!("{TREE}/{}", tree::DEEPER)).expect("a nested create_dir failed");
    std::fs::write(format!("{TREE}/top"), tree::LEAF_BODY).expect("a nested write failed");

    // Open every file the listing named by the `path()` it handed back, two components deep.
    let mut opened = 0;
    let mut dirs = 0;
    for entry in std::fs::read_dir(TREE).expect("read_dir of a subdirectory failed") {
        let entry = entry.expect("a directory entry did not decode");
        if entry.file_type().expect("file_type failed").is_dir() {
            dirs += 1;
            continue;
        }
        let bytes = std::fs::read(entry.path())
            .expect("a file read_dir listed would not open by the path read_dir gave");
        assert_eq!(
            bytes,
            tree::LEAF_BODY,
            "an entry opened by its listed path read the wrong bytes",
        );
        opened += 1;
    }
    assert_eq!(opened, 1, "the tree this program made holds one file");
    assert_eq!(dirs, 1, "the tree this program made holds one directory");
    println!("walk entry ok");

    // `OPENDIR` of a file is `ENOTDIR`, a fact about the name, so it does not read as a refusal.
    match File::open(format!("{}/{}", filesystem_fixture::MOTD_NAME, tree::INNER)) {
        Err(e) if e.kind() == ErrorKind::NotADirectory => println!("through a file refused"),
        other => panic!("a path walking through a file was not refused: {other:?}"),
    }

    held_directories();

    // **A move between two directories**, which is one atomic message rather than a copy and an
    // unlink: `RENAME` carries a handle on each side and both of them are now walked to.
    std::fs::rename(
        format!("{TREE}/top"),
        format!("{TREE}/{}/{}", tree::DEEPER, tree::LEAF),
    )
    .expect("renaming between two directories failed");
    assert!(
        !std::fs::exists(format!("{TREE}/top")).expect("exists after rename failed"),
        "the source name survived a rename",
    );
    assert_eq!(
        std::fs::read(format!("{TREE}/{}/{}", tree::DEEPER, tree::LEAF))
            .expect("the renamed file would not open"),
        tree::LEAF_BODY,
        "the rename moved a name without its bytes",
    );
    println!("rename across ok");

    // No nife code: std's recursion is written over `read_dir`, `remove_file` and `remove_dir`.
    std::fs::remove_dir_all(TREE).expect("remove_dir_all failed");
    assert!(
        !std::fs::exists(TREE).expect("exists after remove_dir_all failed"),
        "remove_dir_all left the tree behind",
    );
    println!("remove_dir_all ok");
}

/// The `dir mtime ok` line of [`file_times`].
fn a_directory_mtime_round_trips_and_its_listing_agrees() {
    use std::fs::FileTimes;

    std::fs::create_dir(TIMES_DIR).expect("create_dir for the times directory failed");
    let dir_time = UNIX_EPOCH + Duration::from_secs(1_900_000_000);
    std::fs::set_times(TIMES_DIR, FileTimes::new().set_modified(dir_time))
        .expect("set_times on a directory failed");
    assert_eq!(
        std::fs::metadata(TIMES_DIR)
            .expect("metadata of the times directory failed")
            .modified()
            .expect("modified() refused for a directory reached by name"),
        dir_time,
        "a directory's mtime did not round-trip",
    );
    let listed = std::fs::read_dir(".")
        .expect("read_dir of the granted directory failed")
        .map(|e| e.expect("a listing entry failed"))
        .find(|e| e.file_name() == TIMES_DIR)
        .expect("the times directory is not in the listing");
    assert_eq!(
        listed
            .metadata()
            .expect("DirEntry::metadata of a directory failed")
            .modified()
            .expect("modified() refused through a listing entry"),
        dir_time,
        "a listing entry reported a different time than the name did",
    );
    println!("dir mtime ok");
}

/// [`file_times`]' four refusals. `asserted` is the time it set, and `back` is what that read
/// back as, which a refused `set_times` must leave in place.
fn file_time_refusals(asserted: SystemTime, back: SystemTime) {
    use std::fs::FileTimes;

    // Through an open handle: refused, both ways, and the time on disk is untouched.
    let file = File::options()
        .read(true)
        .write(true)
        .open(TIMES_SUBJECT)
        .expect("reopening the times subject failed");
    match file.metadata().map(|m| m.modified()) {
        Ok(Err(e)) if e.kind() == ErrorKind::Unsupported => {}
        other => panic!("modified() through an open handle did not refuse: {other:?}"),
    }
    match file.set_modified(asserted) {
        Err(e) if e.kind() == ErrorKind::Unsupported => {}
        other => panic!("set_modified through an open handle did not refuse: {other:?}"),
    }
    drop(file);
    println!("handle mtime refused");

    // An access time is refused whole: the mtime asked for beside it must not be written either.
    match std::fs::set_times(
        TIMES_SUBJECT,
        FileTimes::new()
            .set_modified(asserted)
            .set_accessed(asserted),
    ) {
        Err(e) if e.kind() == ErrorKind::Unsupported => {}
        other => panic!("set_times with an access time did not refuse: {other:?}"),
    }
    assert_eq!(
        std::fs::metadata(TIMES_SUBJECT)
            .expect("metadata after a refused set_times failed")
            .modified()
            .expect("modified() after a refused set_times failed"),
        back,
        "a refused set_times changed the mtime anyway",
    );
    match std::fs::set_times(".", FileTimes::new().set_modified(asserted)) {
        Err(e) if e.kind() == ErrorKind::Unsupported => {}
        other => panic!("set_times on the granted directory did not refuse: {other:?}"),
    }
    match std::fs::set_times("times-nobody-made", FileTimes::new()) {
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        other => panic!("an empty set_times on a missing name did not say NotFound: {other:?}"),
    }
    println!("set_times refusals ok");
}

/// The `dir handle ok` line of [`descent_transcript`]: `std::fs::Dir`.
fn held_directories() {
    use filesystem_fixture::tree;

    // `std::fs::Dir` (milestone 122): `inner` resolves against a held directory, with no path
    // composition anywhere.
    let dir = Dir::open(tree::SUB).expect("Dir::open of a subdirectory failed");
    let mut f = dir
        .open_file(tree::INNER)
        .expect("Dir::open_file under a held directory failed");
    let mut held = Vec::new();
    f.read_to_end(&mut held)
        .expect("reading through a held directory failed");
    assert_eq!(
        held,
        tree::INNER_BODY,
        "the held directory opened the wrong file"
    );
    drop(f);

    // A held directory is bound exactly as the granted one is: `..` names nothing through it either.
    match dir.open_file(format!("../{}", filesystem_fixture::MOTD_NAME)) {
        Err(e) if e.kind() == ErrorKind::InvalidFilename => {}
        other => panic!("`..` through a held directory was not refused: {other:?}"),
    }

    // `Dir::open(".")` is the granted directory itself and costs no message at all, because it is
    // what the endpoint is bound to rather than a name inside anything.
    let granted = Dir::open(".").expect("Dir::open of the granted directory failed");
    let mut motd = granted
        .open_file(filesystem_fixture::MOTD_NAME)
        .expect("the granted directory would not open its own file");
    let mut bytes = Vec::new();
    motd.read_to_end(&mut bytes).expect("read failed");
    assert_eq!(
        bytes,
        filesystem_fixture::MOTD,
        "the granted directory read the wrong file"
    );
    println!("dir handle ok");
}

/// **Milestone 121's two halves, chosen by the rights of the one directory this process holds.**
///
/// Granted [`filesystem_fixture::walk`]'s tree with `ENUMERATE`, it prices a walk
/// (`walk_pricing`) and prints the figures. Granted the same tree without `ENUMERATE`, the same
/// walk must be **refused loudly**: `read_dir` answers `PermissionDenied`, and the walker a search
/// tool is built on returns that error rather than a walk of nothing. The program does not know
/// which grant it holds and does not ask; it tries to list, and the capability answers.
fn walk_transcript() {
    use filesystem_fixture::walk as tree;

    // Whichever grant this is, the tree is here: a name the program can spell opens under both,
    // because reaching what you can name is `READ` and `DESCEND`, and `ENUMERATE` is only the
    // power to find out what else there is.
    let first = tree::wide_name(0);
    let first = std::str::from_utf8(&first).expect("fixture names are ASCII");
    let named = format!("{}/{first}", tree::NARROW);
    let bytes = std::fs::read(&named).expect("a named file in the priced tree would not open");
    assert_eq!(
        bytes,
        tree::SMALL_BODY,
        "the named file read the wrong bytes"
    );

    match std::fs::read_dir(".") {
        Ok(_) => price_walk(),
        Err(e) if e.kind() == ErrorKind::PermissionDenied => enumeration_withheld(),
        Err(e) => {
            panic!("listing the granted directory failed for a reason that is not authority: {e:?}")
        }
    }
}

/// The walk, priced. The first line is the counts, which the kernel test asserts; the rest are
/// timings, which it prints and does not.
fn price_walk() {
    println!("walk granted with enumerate");
    let price = walk_pricing::price(std::path::Path::new(".")).expect("the priced walk failed");
    for line in walk_pricing::report(&price) {
        println!("{line}");
    }
}

/// The same walk against a grant lacking `ENUMERATE`, refused at every level and never empty.
fn enumeration_withheld() {
    use filesystem_fixture::walk as tree;
    println!("walk granted without enumerate");
    println!("read_dir refused");

    // One level down the right has not reappeared: a child's rights are its parent's intersected
    // with the request, so a descent asking for `ENUMERATE` is refused on the way.
    match std::fs::read_dir(tree::WIDE) {
        Err(e) if e.kind() == ErrorKind::PermissionDenied => println!("read_dir below refused"),
        other => panic!("a subdirectory listed under a grant without ENUMERATE: {other:?}"),
    }

    // Milestone 121's claim: the walk is an error, never a walk that found nothing.
    match walk_pricing::walk(std::path::Path::new(".")) {
        Err(e) if e.kind() == ErrorKind::PermissionDenied => println!("walk refused, not empty"),
        Ok(t) => panic!("a walk without ENUMERATE returned {t:?} instead of an error"),
        Err(e) => panic!("a walk without ENUMERATE failed for another reason: {e:?}"),
    }

    // And naming still works at depth, so the refusals above are about enumeration alone and not a
    // capability that reaches nothing (the control the directory attacker's bitmap also keeps).
    let deep = format!("{}/{}/{}", tree::CHAIN, tree::LEVEL, tree::LEVEL_FILE);
    let bytes = std::fs::read(&deep).expect("a named file two levels down would not open");
    assert_eq!(
        bytes,
        tree::SMALL_BODY,
        "the deep named file read the wrong bytes"
    );
    println!("named file opened");
}

/// Assert that a path is refused as un-nameable, and say which case it was.
fn refused(path: &str, label: &str) {
    match File::open(path) {
        Err(e) if e.kind() == ErrorKind::InvalidFilename => println!("{label} refused"),
        other => panic!("{path} was not refused as un-nameable: {other:?}"),
    }
}

/// The guestfwd echo peer the test runners attach. Both of this program's network fixtures now live
/// inside libslirp (this one and the TFTP server `udp_ok` uses), so the transcript is offline and
/// deterministic: nothing it depends on can be dropped by somebody else's router.
const ECHO_PEER: &str = "10.0.2.9:7777";

/// The networked transcript (milestone 27 phase two): a UDP round trip and a TCP echo round
/// trip, reached only through `std::net`. The program never sees a capability, a socket id, or a
/// shared frame; it writes to a socket and reads from it, the way any Rust program does. Runs when
/// the program holds the network. `sock` is the already-bound UDP socket the probe opened.
fn net_demo(sock: UdpSocket) {
    println!("std net on nife");

    // A listening port is a grant `net_stack` was spawned with (milestone 64), so asking is the
    // probe. A refusal is printed, so a lost grant is a diff in the pinned transcript.
    match TcpListener::bind(("0.0.0.0", fixture::LISTEN_PORT)) {
        Ok(listener) => {
            // The granted run serves and stops: a net boot costs minutes of smoltcp polling, and
            // the outbound half is proven by the run refused the port.
            drop(sock);
            return inbound_demo(&listener);
        }
        Err(e) if e.kind() == ErrorKind::PermissionDenied => println!("listen refused"),
        Err(e) => panic!("the listen probe failed for a reason that is not a refusal: {e:?}"),
    }

    // Assertions rather than printed status keep the transcript byte-stable: a failure faults (the
    // panic path), which the kernel test sees as a missing line and a timeout, not a wrong answer.
    assert!(udp_ok(&sock), "the UDP round trip through std::net failed");
    println!("udp ok");

    // Held across the TCP exchange: `net_stack` derives a local port from the socket id, and a
    // reused id's port can stall against slirp (notes/std/net.md, the reuse finding).
    assert!(
        tcp_echo_ok(),
        "the TCP echo round trip through std::net failed"
    );
    println!("tcp echo ok");
    drop(sock);
}

/// **The inbound transcript** (milestone 64): this program as a server, through `std::net` alone,
/// accepting connections a host process opens through QEMU's `hostfwd`.
///
/// It prints `denied refused` (a port outside the grant is `PermissionDenied`), `in use refused`
/// (the granted port twice is `AddrInUse`) and `served N` (`ROUNDS` connections on one listener;
/// the second is the load-bearing one). The host half is `xtask`'s inbound prober, which fails the
/// leg if its bytes never come back. See notes/std/the-proof.md.
fn inbound_demo(listener: &TcpListener) {
    println!("listen ok");

    match TcpListener::bind(("0.0.0.0", fixture::DENIED_PORT)) {
        Err(e) if e.kind() == ErrorKind::PermissionDenied => println!("denied refused"),
        Ok(_) => panic!("bound a port this program was never granted"),
        Err(e) => {
            panic!("a port outside the grant failed, but not as a refusal of authority: {e:?}")
        }
    }

    match TcpListener::bind(("0.0.0.0", fixture::LISTEN_PORT)) {
        Err(e) if e.kind() == ErrorKind::AddrInUse => println!("in use refused"),
        Ok(_) => panic!("two listeners bound one port"),
        Err(e) => panic!("a second bind of a held port failed, but not as a port collision: {e:?}"),
    }

    for round in 0..fixture::ROUNDS {
        serve_one_inbound(listener, round);
    }
    println!("served {}", fixture::ROUNDS);
}

/// How many of `net_stack`'s bounded accept waits (15 s each) a round sits through before it calls
/// the round lost: two minutes, which covers the host prober's longest hold on a connection made
/// before this listener existed (about 23 s) with room for a slow CPU model.
const ACCEPT_TRIES: usize = 8;

/// Accept one inbound connection, check what the host sent, answer it, and close. `read_exact`,
/// because a segment boundary is the host's business; dropping the stream is the close, and
/// `net_stack` drains the handshake inside it, so the answer is on the wire before this returns.
fn serve_one_inbound(listener: &TcpListener, round: usize) {
    // `WouldBlock` is the PAL's "the bounded wait expired, the listener is still armed": keep
    // accepting, since a window can open while the host prober still holds an earlier connection.
    let mut tries = 0;
    let waiting = Instant::now();
    let (mut conn, _peer) = loop {
        match listener.accept() {
            Ok(accepted) => break accepted,
            Err(e) if e.kind() == ErrorKind::WouldBlock && tries + 1 < ACCEPT_TRIES => tries += 1,
            Err(e) => panic!(
                "round {round}: nobody connected after {} tries over {} ms: {e:?}",
                tries + 1,
                waiting.elapsed().as_millis()
            ),
        }
    };

    let mut got = vec![0u8; fixture::IN_MSG.len()];
    conn.read_exact(&mut got)
        .unwrap_or_else(|e| panic!("round {round}: reading the request failed: {e:?}"));
    assert_eq!(
        got,
        fixture::IN_MSG,
        "round {round}: something connected and said something else",
    );

    conn.write_all(fixture::OUT_MSG)
        .unwrap_or_else(|e| panic!("round {round}: answering failed: {e:?}"));
}

/// **The gating UDP round trip: slirp's own TFTP server**, the `std::net` twin of
/// `socket_test_client`'s `udp_tftp`. libslirp serves TFTP itself (`tftp=` on the netdev), so
/// nothing leaves the emulator; 10.0.2.3:53 would not do, since slirp NATs it to the host's DNS.
///
/// Sends a read request (opcode 1, `octet`) for the fixture and requires the first data packet:
/// opcode 3, block 1, the fixture's bytes. The name and body must match what
/// `helpers/qemu-runner-*.sh` writes into `target/tftp`. See notes/std/the-proof.md.
fn udp_ok(sock: &UdpSocket) -> bool {
    const TFTP_SERVER: &str = "10.0.2.2:69";
    const TFTP_NAME: &[u8] = b"nife";
    const TFTP_BODY: &[u8] = b"nife-tftp!";

    if sock.connect(TFTP_SERVER).is_err() {
        return false;
    }

    // RRQ: { u16 opcode = 1 } filename 0 "octet" 0
    let mut rrq = vec![0x00, 0x01];
    rrq.extend_from_slice(TFTP_NAME);
    rrq.push(0x00);
    rrq.extend_from_slice(b"octet");
    rrq.push(0x00);
    if sock.send(&rrq).is_err() {
        return false;
    }

    let mut buf = [0u8; 512];
    let Ok(n) = sock.recv(&mut buf) else {
        return false;
    };
    if n < 4 + TFTP_BODY.len() {
        return false;
    }
    // An ERROR packet (opcode 5) here means the fixture is missing: see the runners.
    let opcode = u16::from_be_bytes([buf[0], buf[1]]);
    let block = u16::from_be_bytes([buf[2], buf[3]]);
    if opcode != 3 || block != 1 || &buf[4..4 + TFTP_BODY.len()] != TFTP_BODY {
        return false;
    }

    // ACK block 1 so the server stops retransmitting DATA; whether it is acknowledged is not this
    // test's business, so the result is ignored.
    let _ = sock.send(&[0x00, 0x04, 0x00, 0x01]);
    true
}

/// Connect to the echo peer over TCP, send a payload, and read the echo back whole.
fn tcp_echo_ok() -> bool {
    const MSG: &[u8] = b"nife-std-net!";
    let Ok(mut stream) = TcpStream::connect(ECHO_PEER) else {
        return false;
    };
    if stream.write_all(MSG).is_err() {
        return false;
    }
    let mut got = Vec::new();
    let mut buf = [0u8; 64];
    while got.len() < MSG.len() {
        match stream.read(&mut buf) {
            Ok(0) => break, // peer closed
            Ok(k) => got.extend_from_slice(&buf[..k]),
            Err(_) => return false,
        }
    }
    got == MSG
}
