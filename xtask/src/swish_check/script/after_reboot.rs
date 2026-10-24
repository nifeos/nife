//! **The second boot's script**: the boot the first boot hands its disk to. The installed
//! program still runs, a removal takes its vouch away, a rollback restores it, and the
//! packages basalt publishes install and run. `FROM_BASALT` lists the lines skipped
//! without a repository.
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// **The one line that resolves a name at the prompt**: `jig install uptime` tries the entry's
/// listed location, `packages.basalt.test`, by name before the repository's own copy. Typed from
/// this constant so `resolver_refusal_failure` reads the line the script types.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane `lane/jig-resolves-by-name`.
pub(in crate::swish_check) const RESOLVING_LINE: &str = "jig install uptime";

/// **What the second boot types** (milestone 198 (a package manager) rung 3a): the installed
/// program still runs after a reboot, a removal takes its vouch away without deleting it, and a
/// rollback vouches for it again. The disk is the only thing the first boot hands this one.
///
/// The boot prompt holds the run-unvouched capability (DECISIONS §219 gate D2), so the removal
/// shows in `caps` and the bytes still run. `greeting` (generation 2, and its 0.2.0 as generation
/// 3) keeps running when `noteless` is removed, since a generation drops one program and not its
/// neighbors. The numbers skip one: the first boot vouched for generation 4 and rolled it back
/// (DECISIONS §221), so the removal is 5 and the rollback, by number (`notes/packages.md`'s BUGS),
/// lands on 4. See notes/swish-check-harness.md.
pub(in crate::swish_check) const SWISH_CHECK_AFTER_REBOOT: &[Line] = &[
    // **The shell holds no display device** (milestone 715 (provisional); row 32 of
    // notes/confinement-claims.md). The spawn service keeps the gpu's four capabilities, which
    // would otherwise sit from `spawnproto::SHELL_GPU_SLOT` (22), so " 21 62" asserts nothing sits
    // between the configuration page (21) and the run-unvouched slot (62). On x86_64 this boot has
    // no gpu and the line holds trivially; that leg's gap is milestone 632's.
    line(0, "caps", &["slots held: 0 1 2", " 21 62\n"]),
    line(
        1,
        "packages/noteless/0.1.0/noteless",
        &["noteless: installed, and carrying no manifest note"],
    ),
    line(
        1,
        "packages/greeting/0.1.0/greeting",
        &[
            "hello from a package this image never carried",
            "clock: held at slot 1, as its manifest note asked",
        ],
    ),
    line(1, "jig remove noteless", &["removed; generation 5 is live"]),
    // **Removed means unvouched, not unrunnable, for a session holding D2** (DECISIONS §219 gate
    // D2). Until D2 this line was a refusal. The boot prompt now holds the run-unvouched
    // capability (provisionally), so the bytes still run, with the ruling's endowment rather than
    // the installed manifest; `caps` is what shows the vouch is gone.
    line(
        0,
        "caps packages/noteless/0.1.0/noteless",
        &[
            "provenance: unvouched (digest sha256:",
            "runs on this session's capability to run unvouched bytes",
        ],
    ),
    line(
        1,
        "packages/noteless/0.1.0/noteless",
        &["noteless: installed, and carrying no manifest note"],
    ),
    line(
        1,
        "packages/greeting/0.1.0/greeting",
        &[
            "hello from a package this image never carried",
            "clock: held at slot 1, as its manifest note asked",
        ],
    ),
    line(1, "jig rollback", &["rolled back; generation 4 is live"]),
    line(
        1,
        "packages/noteless/0.1.0/noteless",
        &["noteless: installed, and carrying no manifest note"],
    ),
    // **The distribution's index, over HTTPS** (milestone 801 (packages over the internet)).
    // `jig update` reads the image's index sources (an archive entry the kernel measured), fetches
    // the stand-in index from `basalt.test` at the address the sources give, pinned to the test
    // authority, and writes this architecture's lines to `activation/indexes/image`. The peer's
    // index is `disk::stage_index`'s: `greeting` 0.1.0, `uptime`, a `rebound` twin
    // (`helpers/tls-peer` adds it) and `greeting` 0.3.0, which the image's catalog does not name.
    line(
        1,
        "jig update",
        &[
            // Four from the fixtures, and one more per package from basalt's repository when one
            // was fetched (milestone 810), so the count is not asserted.
            "index updated: ",
            "from basalt.test over TLS",
        ],
    ),
    // `list` reads the copy once there is one, and says so.
    line(
        1,
        "jig list",
        &[
            "packages the index offers (the copy jig update wrote):",
            "  greeting 0.3.0",
        ],
    ),
    // **`list -u`: what is installed against what the copy offers** (milestone 809 (the package
    // client becomes a program)). The live generation holds `greeting` 0.1.0 and 0.2.0, from the
    // first boot's catalog and file installs, and the copy offers 0.3.0, newer in Debian's order
    // than the newest of them. The line holds
    // `activation/` read only and nothing else, which `caps` shows; `--upgradable` is refused
    // naming `-u` (calef, 2026-10-17 UTC).
    line(
        0,
        "caps jig list -u",
        &["endpoint  dir      /activation  read-only."],
    ),
    line(
        1,
        "jig list -u",
        &[
            "installed packages the index offers newer (the copy jig update wrote):",
            "  greeting 0.2.0 -> 0.3.0",
        ],
    ),
    line(
        1,
        "jig list --upgradable",
        &["refused: jig spells that list -u; it takes no long form"],
    ),
    // **A lying repository, refused by the index's digest** before the installer is asked: the
    // peer serves `uptime` with a byte flipped. `jig` tries the entry's listed location first,
    // which this prompt cannot resolve, and then the repository's own copy. That listed location
    // is the one name a std program resolves at this prompt, through the resolver the progenitor
    // started, so its line is where `resolver_refusal_failure` looks.
    line(
        1,
        RESOLVING_LINE,
        &[
            "passed over https://packages.basalt.test:8443",
            "refused: the index does not vouch for the bytes that arrived; nothing was sent to the \
             installer",
        ],
    ),
    // **Installed on the index copy's word alone** (ruling I2): the image's catalog names no
    // `greeting` 0.3.0, so the progenitor's installer accepts it only because the copy in the
    // store the owner's console granted lists its digest.
    line(
        1,
        "jig install greeting@0.3.0",
        &[
            "fetched greeting-0.3.0-",
            "from basalt.test over TLS",
            "fetched and installed; generation 6 is live",
        ],
    ),
    line(
        1,
        "packages/greeting/0.3.0/greeting",
        &["hello from a package this image never carried"],
    ),
    // With 0.3.0 installed beside the others, the newest installed is the newest offered.
    line(
        1,
        "jig list -u",
        &["no installed package has a newer version in the index copy"],
    ),
    // **A package carries several programs** (milestone 919 (a package carries several
    // programs)). `duet` carries `bin/alto` and `bin/tenor`, neither named after it: one install
    // writes a row and a pointer for each, in one generation, and each runs by its own bare name. Their notes differ, so they are
    // endowed differently: `alto` holds the clock and `tenor` does not.
    // `jig` is granted the directory holding the file it names, so the line runs from there.
    line(0, "cd downloads", &[]),
    line(
        1,
        "jig install ./duet.nifepkg",
        &["installed; generation 7 is live"],
    ),
    line(
        1,
        "alto",
        &[
            "alto: one of two programs in the package duet",
            "clock: held at slot 1, as its manifest note asked",
        ],
    ),
    line(
        1,
        "tenor",
        &[
            "tenor: one of two programs in the package duet",
            "clock: not held; slot 1 is empty",
        ],
    ),
    line(
        0,
        "caps alto",
        &["its manifest note asks for: output bytes, the clock\n"],
    ),
    line(
        0,
        "caps tenor",
        &["its manifest note asks for: output bytes\n"],
    ),
    // **A name one package holds refuses another package whole** (calef, 2026-10-14 (UTC): "If
    // two packages offer the same program name, refuse the second package. Need to remove the
    // first one before installing the second one."). `solo` carries `bin/alto` too. The refusal
    // names the program and its holder, and writes no generation.
    line(
        1,
        "jig install ./solo.nifepkg",
        &[
            "refused: the package duet already provides the program alto; jig remove duet first; \
             generation 7 is live",
        ],
    ),
    // **`jig remove` names a package** (calef, 2026-10-14 (UTC): "jig remove should work on
    // packages, not programs."): a program's name removes nothing, and the package's name takes
    // both programs, so neither bare word reaches anything after it.
    line(
        1,
        "jig remove alto",
        &["refused: no package of that name is in the live generation; generation 7 is live"],
    ),
    line(1, "jig remove duet", &["removed; generation 8 is live"]),
    line(0, "alto", &["no such program"]),
    line(0, "tenor", &["no such program"]),
    // And with the holder gone, the second package installs, and the name is its.
    line(
        1,
        "jig install ./solo.nifepkg",
        &["installed; generation 9 is live"],
    ),
    line(0, "cd /", &[]),
    line(
        1,
        "alto",
        &["alto: one of two programs in the package duet"],
    ),
    line(1, "jig remove solo", &["removed; generation 10 is live"]),
    // **Installing asks the owner, and a no installs nothing** (milestone 892 (a std program at
    // the prompt holds the network and a resolver); calef's rulings on #2405). `dialer`'s note
    // asks for the network, `nife.test` and the root zone; `jig install` lists them, the root zone
    // apart, and asks once. The gate answers `n`, so no generation is written and the bare word
    // reaches nothing.
    line(0, "cd downloads", &[]),
    asked(
        1,
        "jig install ./dialer.nifepkg",
        (OWNERS_QUESTION, "n"),
        &[
            "dialer asks for:",
            "reaches the network (connects out)",
            "looks up names in nife.test",
            "LOOKS UP ANY NAME AT ALL (the root zone)",
            "grant these and install? [y/N] n\n",
            "not installed; nothing was granted",
        ],
    ),
    line(0, "cd /", &[]),
    line(0, "dialer", &["no such program"]),
    // **`ripgrep`, built and published by basalt** (milestone 810 (`ripgrep` is packaged in
    // basalt and installed with `jig`)): the package basalt published, fetched by its pinned
    // digest (`cargo xtask basalt-fetch`), or the repository basalt's own gate just built
    // (`disk::BASALT_REPOSITORY`). The peer serves its index lines beside the fixtures'. The
    // package is `ripgrep`; its program is the member `bin/rg`
    // (`package_archive::PROGRAM_PREFIX`), so the bare word is `rg`. Skipped without a repository.
    line(
        1,
        "jig install ripgrep",
        &[
            "fetched ripgrep-",
            "from basalt.test over TLS",
            "fetched and installed; generation 11 is live",
        ],
    ),
    line(1, "rg --version", &["ripgrep "]),
    // **`rg pattern dir` at the prompt, `rg` as `jig` installed it** (milestone 121 (`ripgrep`:
    // enumeration as a capability), exit 4, calef's 2026-10-07 ruling on #1797's fork 4).
    // `/search/crates` is a copy of this repository's `crates/` (`crate::disk::RIPGREP_CORPUS`),
    // hundreds of files in nested directories, which the first boot wrote and this boot inherits
    // with the disk. `caps` shows the whole grant before anything runs: the one directory the word
    // named, read-only, which is `ENUMERATE | READ | DESCEND` and nothing else. No flag narrows the
    // walk: `rg` lists every directory it meets, opens every file, skips the binary ones itself, and
    // prints every matching line. The answer is wanted whole, so the line lists nothing here and
    // `rg_walk_verdict` compares it line for line with what the host's `rg` printed over the same
    // copy.
    line(0, RG_WALK_CD, &[]),
    line(
        0,
        RG_WALK_PREVIEW,
        &[
            "endpoint  dir",
            "(only crates in it, named on the line)",
            "...read-only, and everything under what was named",
        ],
    ),
    line(1, RG_WALK_LINE, &[]),
    line(0, "cd /", &[]),
    // **`ioping`, basalt's first C package** (milestone 906 (ioping is packaged in basalt, and its
    // test installs it with `jig`)), on `ripgrep`'s terms: compiled by basalt against the C library
    // at the nife commit basalt pins. Installed, it times five 4 KiB reads of a file its word
    // names, and the file is its own installed program: unvouched, it is granted that file
    // read-only and the clock page. The line stands in `packages` and names the file relative to
    // it: only a word whose first component names an entry here designates anything (an absolute
    // one reached ioping with nothing granted, and its `stat` answered `ENOSYS`), and a word in
    // the shell's root is refused, because a caretaker is built by descending into a directory.
    // Pointed at a directory, ioping would need to write a working file there. The kernel suite's
    // `c_program_tests` runs the same assertions on the same bytes against a granted directory.
    line(
        1,
        "jig install ioping",
        &[
            "fetched ioping-",
            "from basalt.test over TLS",
            "fetched and installed; generation 12 is live",
        ],
    ),
    line(1, "ioping -v", &["ioping 1.3"]),
    line(0, "cd packages", &[]),
    line(
        1,
        IOPING_LINE,
        &[
            "4 KiB <<< ioping/1.3/ioping ",
            "request=1 ",
            "request=5 ",
            "(warmup)",
            "ioping statistics ---",
            "4 requests completed in ",
            ", 16 KiB read, ",
            "generated 5 requests in ",
            "min/avg/max/mdev = ",
        ],
    ),
    line(0, "cd /", &[]),
    // **The same program, timing the disk rather than a file server's memory** (milestone 834 (ioping on nife and Linux)).
    // `/disk` is the second mounted tree, the disk tree: the whole NVMe namespace as one
    // read-only file, `n1`, served with no cache, so every request reaches the controller.
    // `-o 1m -S 4m` is the range Linux's side of the bench names by flag, scaled to this 8 MiB
    // disk. The library's `O_DIRECT` is not asked for here: basalt's ioping is built without
    // `HAVE_DIRECT_IO`, so `-D` would be refused until it is rebuilt.
    line(0, "ls /disk/nvme0", &["n1"]),
    line(0, "cd /disk/nvme0", &[]),
    line(
        1,
        DISK_TREE_IOPING_LINE,
        &[
            "4 KiB <<< n1 ",
            "request=5 ",
            "ioping statistics ---",
            "4 requests completed in ",
            ", 16 KiB read, ",
        ],
    ),
    line(0, "cd /", &[]),
    // **`hackbench`, basalt's second C package** (milestone 907 (hackbench is packaged in basalt,
    // and its test installs it with `jig`)), installed the same way. Its thread mode is the run
    // `c_thread_tests` makes, smaller: one group of two senders and two receivers, four threads
    // and the main one, whose 64 KiB stacks fit an ordinary `std` program's heap. Unvouched, it
    // needs no grant beyond the clock page it times itself with.
    line(
        1,
        "jig install hackbench",
        &[
            "fetched hackbench-",
            "from basalt.test over TLS",
            "fetched and installed; generation 13 is live",
        ],
    ),
    line(
        1,
        HACKBENCH_LINE,
        &[
            "Running in threaded mode with 1 groups using 4 file descriptors each (== 4 tasks)",
            "Each sender will pass 10 messages of 100 bytes",
            "Time: ",
        ],
    ),
    // **SQLite, basalt's third C package** (milestone 897 (SQLite is packaged in basalt)): the
    // package `sqlite` carries the program `speedtest1`. The prompt runs the kernel suite's
    // in-memory line, which opens no file and needs only the clock page, and holds it to the same
    // hash of every result SQLite read back.
    line(
        1,
        "jig install sqlite",
        &[
            "fetched sqlite-",
            "from basalt.test over TLS",
            "fetched and installed; generation 14 is live",
        ],
    ),
    line(
        1,
        c_program_fixture::SPEEDTEST1_MEMDB,
        &[
            "-- Speedtest1 for SQLite 3.50.4",
            "TOTAL",
            c_program_fixture::SPEEDTEST1_HASH,
        ],
    ),
    // **`rmle`, the first first-party package basalt builds** (milestone 921 (rmle is packaged in
    // basalt)): the editor's `[[bin]]`, built by basalt at the nife commit it pins, installed from
    // basalt's index. It is not typed afterwards: no prompt line can start `rmle` yet, because only
    // the kernel harness wires its raw terminal (`kernel/src/user/rmle_service.rs`).
    line(
        1,
        "jig install rmle",
        &[
            "fetched rmle-",
            "from basalt.test over TLS",
            "fetched and installed; generation 15 is live",
        ],
    ),
    // **The demos, the second first-party package** (milestone 922 (the demos are packaged in
    // basalt)): `demos` carries `least_authority_demo`. The package is fetched from basalt and
    // accepted by its digest, and then the installer refuses it, because this image carries that
    // program and a base program is updated by a new base, not installed over. The refusal is the
    // answer until milestone 909 (basalt composes the bootable image from its packages) leaves
    // optional programs out of the image.
    line(
        1,
        "jig install demos",
        &[
            "fetched demos-",
            "from basalt.test over TLS",
            "refused: the image carries the program least_authority_demo; a new base updates it, not \
             install; generation 15 is live",
        ],
    ), // **A yes grants exactly what was asked, and the program holds it** (milestone 892). `reach`
    // is a `std` program whose note asks for the network and `nife.test`. The gate answers `y`, so
    // the generation records a `grant` line for each, and run at the prompt it connects to the
    // runner's echo peer and holds a resolver badge for its zone: a name outside it is the
    // resolver's own refusal, which needs no name server the runner cannot provide. Last in the
    // script, and with no generation numbers, because it is skipped where `std_exerciser` was not
    // built ([`REACH_LINES`]).
    line(0, "cd downloads", &[]),
    asked(
        1,
        "jig install ./reach.nifepkg",
        (OWNERS_QUESTION, "y"),
        &[
            "reach asks for:",
            "reaches the network (connects out)",
            "looks up names in nife.test",
            "grant these and install? [y/N] y\n",
            "installed; generation",
        ],
    ),
    line(0, "cd /", &[]),
    line(
        1,
        "reach",
        &[
            "reach: echo ok",
            "reach: lookup example.com: PermissionDenied",
        ],
    ),
    line(1, "jig remove reach", &["removed; generation"]),
    line(0, "reach", &["no such program"]),
];

/// What `jig install` asks before it installs a package that asks for grants (milestone 892).
const OWNERS_QUESTION: &str = "grant these and install? [y/N]";

/// The lines that install and run `reach`, skipped where `std_exerciser` was not built, since its
/// package is built from that workspace (`package::Source::StdProgram`).
pub(in crate::swish_check) const REACH_LINES: &[&str] =
    &["jig install ./reach.nifepkg", "reach", "jig remove reach"];

/// The run `jig install hackbench` is followed by (milestone 907): `c_thread_tests`' pipe run,
/// with one group of two and ten messages a sender.
const HACKBENCH_LINE: &str = "hackbench -T -p -g 1 -f 2 -l 10";

/// The timed reads `jig install ioping` is followed by (milestone 906): `c_program_fixture::IOPING`'s
/// flags, pointed at the installed program's own file rather than a directory.
const IOPING_LINE: &str = "ioping -c 5 -i 0 -s 4k ioping/1.3/ioping";

/// `ioping` against the disk tree (milestone 834): five 4 KiB reads at random offsets in the 4 MiB
/// from 1 MiB of swish-check's NVMe disk.
const DISK_TREE_IOPING_LINE: &str = "ioping -c 5 -i 0 -s 4k -S 4m -o 1m n1";

/// The lines that read the disk tree, left out where a leg attaches no NVMe disk
/// (`verdict::swish_check_omits`).
pub(in crate::swish_check) const ON_THE_DISK_TREE: &[&str] =
    &["ls /disk/nvme0", "cd /disk/nvme0", DISK_TREE_IOPING_LINE];

/// The lines that need a repository from basalt (milestones 810, 906, 907, 897, 921 and 922),
/// skipped without one outside CI.
pub(in crate::swish_check) const FROM_BASALT: &[&str] = &[
    "jig install ripgrep",
    "rg --version",
    RG_WALK_CD,
    RG_WALK_PREVIEW,
    RG_WALK_LINE,
    "jig install ioping",
    "ioping -v",
    IOPING_LINE,
    DISK_TREE_IOPING_LINE,
    "jig install hackbench",
    HACKBENCH_LINE,
    "jig install sqlite",
    c_program_fixture::SPEEDTEST1_MEMDB,
    "jig install rmle",
    "jig install demos",
];
