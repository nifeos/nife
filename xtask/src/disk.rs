//! The disks a boot is given: the virtio image, the RedoxFS images the FS server opens, the
//! GPT and NVMe fixtures, and the blank disk milestone 57 writes a table onto.
//!
//! Every one is remade per boot, so a run meets a fresh fixture rather than whatever the last
//! one wrote.

use crate::host::{run, workspace_root};
use crate::manual::doc_store;

/// Where the nifefs disk image is written.
pub(crate) fn disk_path() -> String {
    workspace_root()
        .join("target/nifefs.img")
        .display()
        .to_string()
}

/// The PCIe transport's copy of the disk image, a sibling of [`disk_path`]. Two files because
/// both transports are now attached **writable** (milestone 32's write path) and QEMU's image
/// locking refuses to attach one file to two devices once either attachment can write. The
/// runner derives this name from `NIFE_DISK`, so the two stay in lockstep.
fn disk_pci_path() -> String {
    workspace_root()
        .join("target/nifefs-pci.img")
        .display()
        .to_string()
}

/// Build the nifefs disk images the virtio-blk driver will read and write.
///
/// **The disk is generated, not checked in**, the same way the flat kernel image is: a binary
/// blob in git is a blob nobody can review. The contents are a couple of tiny files, written
/// through the same `nifefs::write_image` the userspace filesystem server reads back, so the
/// format has exactly one definition.
///
/// `scratch` is the write-path tests' one-block playground: the driver writes a pattern into its
/// block and reads it back, so nothing else on the disk is ever a write target. Regenerating the
/// images here is also what makes test runs independent: whatever a previous run wrote to
/// scratch is rebuilt to zeros.
pub(crate) fn mkdisk() -> bool {
    let files: [(&str, &[u8]); 3] = [
        (
            "motd",
            b"nife: read from a virtio disk, by a driver at EL0.\n",
        ),
        (
            "readme",
            b"this file came off a real block device through a userspace driver.\n",
        ),
        ("scratch", &[0u8; 512]),
    ];
    let size = nifefs::image_size(&files).max(64 * 1024); // pad to a friendly size
    let mut img = std::vec![0u8; size];
    if nifefs::write_image(&files, &mut img).is_err() {
        eprintln!("mkdisk: could not build the image");
        return false;
    }
    // One identical image per transport; see disk_pci_path for why they cannot share a file.
    for path in [disk_path(), disk_pci_path()] {
        if let Err(e) = std::fs::write(&path, &img) {
            eprintln!("mkdisk: could not write {path}: {e}");
            return false;
        }
    }
    true
}

// ===========================================================================================
// The RedoxFS FS server and its test image (milestone 32 phase 2).
//
// The FS-server binary is out-of-workspace (it links the vendored engine), built for the bare
// targets with the pure no_std core (`--no-default-features`) plus the EL0 runtime (`el0`),
// release so the initrd stays small. The test image is made HOST-side by the redoxfs_host tool,
// the same engine the server opens it with; the server never creates. See notes/fs-server.md.
// ===========================================================================================

/// Build the FS-server ELF for `triple`. Its own workspace, so it takes `--manifest-path` and its
/// artifacts land under `redoxfs_server/target/`.
pub(crate) fn redoxfs_server_build(triple: &str) -> bool {
    run(
        "cargo",
        &[
            "build",
            "--manifest-path",
            "redoxfs_server/Cargo.toml",
            // Both binaries out of the one package: the server that opens an image and never
            // creates, and `mkfs` (milestone 57), which creates one and never serves.
            "--bin",
            "redoxfs_server",
            "--bin",
            "mkfs",
            "--no-default-features",
            "--features",
            "el0",
            "--release",
            "--target",
            triple,
        ],
    )
}

/// The FS-server ELF path for a target triple (always the release profile; see `redoxfs_server_build`).
pub(crate) fn redoxfs_server_elf(triple: &str) -> String {
    workspace_root()
        .join(format!(
            "redoxfs_server/target/{triple}/release/redoxfs_server"
        ))
        .display()
        .to_string()
}

/// The `mkfs` ELF path for a target triple. Same package, same profile, same build.
pub(crate) fn mkfs_elf(triple: &str) -> String {
    workspace_root()
        .join(format!("redoxfs_server/target/{triple}/release/mkfs"))
        .display()
        .to_string()
}

/// Where the RedoxFS test image is written. The runners derive exactly this name from
/// `NIFE_DISK` (`${NIFE_DISK%.img}-redoxfs.img`), so the two stay in lockstep.
pub(crate) fn redoxfs_disk_path() -> String {
    workspace_root()
        .join("target/nifefs-redoxfs.img")
        .display()
        .to_string()
}

/// Drive the `redoxfs_host` tool (its own workspace) by `--manifest-path`, quietly. Returns success.
fn redoxfs_host(args: &[&str]) -> bool {
    let mut v = vec![
        "run",
        "--quiet",
        "--manifest-path",
        "tools/redoxfs_host/Cargo.toml",
        "--",
    ];
    v.extend_from_slice(args);
    run("cargo", &v)
}

/// Build the RedoxFS test image the FS server serves: an empty filesystem with the two fixture
/// files (`motd`, `scratch`) the client reads and writes, plus milestone 47's **subtree**
/// (`sub/` with a file and a grandchild, and the sibling `other/` a directory capability must not
/// reach). Made host-side with the pinned engine, so an image the server opens is proven against
/// exactly the code that opens it. Arch-neutral (the on-disk format does not depend on the CPU), so
/// one image serves both ISA test legs.
pub(crate) fn mkredoxfs() -> bool {
    mkredoxfs_of(REDOXFS_FIXTURE_MIB)
}

/// **How large the shared RedoxFS fixture is**, in MiB: every suite, benchmark and the kernel tests
/// read this size, and notes such as notes/xenon-bench-2026-10.md quote windows derived from it.
const REDOXFS_FIXTURE_MIB: &str = "16";

/// **How large `script/swish-check`'s RedoxFS image is**, in MiB: twice the fixture, for
/// [`RIPGREP_CORPUS`] (milestone 121, 2026-10-07). The copy of `crates/` is 8.2 MB in 661 files
/// and 275 directories, and RedoxFS spends a 4 KiB node on each of those 936 entries and rounds
/// every file up to its block, so it took about 13.5 MiB on disk; beside the 4.8 MiB the image
/// already held, the import failed with "No space left on device". Only this gate's image grows, so
/// the benchmarks and the suite keep the disk their notes describe.
pub(crate) const SWISH_CHECK_DISK_MIB: &str = "32";

/// [`mkredoxfs`] at `mib` MiB rather than the fixture's size.
pub(crate) fn mkredoxfs_of(mib: &str) -> bool {
    let img = redoxfs_disk_path();
    // **`NIFE_KEEP_REDOXFS=1` keeps an existing image instead of rebuilding it.** This is the
    // deliberate way to run the second-boot case: run the suite once normally, then again with this
    // set, and every mount in the second run is a mount of an image a previous *boot* wrote. That is
    // the condition the cross-boot write failure needs, and doing it this way keeps it independent of
    // which ISA leg happens to run first (the order-coupling that hid the bug for three rounds).
    // Absent the variable, each leg gets a fresh fixture, which is what makes the legs reproducible.
    if std::env::var_os("NIFE_KEEP_REDOXFS").is_some() && std::path::Path::new(&img).exists() {
        eprintln!("mkredoxfs: keeping the existing image (NIFE_KEEP_REDOXFS)");
        return true;
    }
    // Stage the fixture contents in temp files (the host tool's `put` takes a host file), then load
    // them. The contents live in filesystem_protocol::fixture, shared with the client and the fixture's readers.
    let motd = workspace_root().join("target/redoxfs-motd.tmp");
    let scratch = workspace_root().join("target/redoxfs-scratch.tmp");
    if std::fs::write(&motd, filesystem_protocol::fixture::MOTD).is_err()
        || std::fs::write(&scratch, filesystem_protocol::fixture::SCRATCH_INIT).is_err()
    {
        eprintln!("mkredoxfs: cannot stage the fixture files");
        return false;
    }
    let motd = motd.display().to_string();
    let scratch = scratch.display().to_string();
    // The shared-frame witness's two files (milestone 599 (a frame per filesystem client channel), provisional): same-length names, distinct
    // bodies, so a name substituted in the shared page shows up as the wrong body. Staged the same
    // way motd is, from the shared `filesystem_protocol::fixture` constants.
    let witness_victim = workspace_root().join("target/redoxfs-witness-victim.tmp");
    let witness_usurper = workspace_root().join("target/redoxfs-witness-usurper.tmp");
    if std::fs::write(
        &witness_victim,
        filesystem_protocol::fixture::SHARED_VICTIM_BODY,
    )
    .is_err()
        || std::fs::write(
            &witness_usurper,
            filesystem_protocol::fixture::SHARED_USURPER_BODY,
        )
        .is_err()
    {
        eprintln!("mkredoxfs: cannot stage the shared-frame witness files");
        return false;
    }
    let witness_victim = witness_victim.display().to_string();
    let witness_usurper = witness_usurper.display().to_string();
    let Some(tree) = stage_subtree() else {
        return false;
    };
    redoxfs_host(&["mkfs", &img, mib])
        && redoxfs_host(&["put", &img, filesystem_protocol::fixture::MOTD_NAME, &motd])
        && redoxfs_host(&[
            "put",
            &img,
            filesystem_protocol::fixture::SCRATCH_NAME,
            &scratch,
        ])
        && redoxfs_host(&[
            "put",
            &img,
            filesystem_protocol::fixture::SHARED_VICTIM_NAME,
            &witness_victim,
        ])
        && redoxfs_host(&[
            "put",
            &img,
            filesystem_protocol::fixture::SHARED_USURPER_NAME,
            &witness_usurper,
        ])
        && doc_store().is_some()
        && redoxfs_host(&["import", &img, &tree])
}

/// **Where `script/swish-check` finds a package to install** (milestone 198 (a package manager)
/// rung 3a's installer): the file `package install` names, relative to the image root. The name
/// is not the package's: the installer reads the stem out of the header, which is the point of
/// this being a plain file a person pointed at. Provisional; standing in for a download.
///
/// It is `uptime`'s package, which the image also carries, so since DECISIONS §229 (calef,
/// 2026-09-27) the gate types it to prove install refuses an image program's name;
/// [`DOWNLOADED_NOTELESS`] is what it installs.
pub(crate) const DOWNLOADED_PACKAGE: &str = "downloads/uptime.nifepkg";

/// **The same package with one byte flipped halfway through**, which the image's catalogue must
/// refuse before anything is written.
pub(crate) const TAMPERED_PACKAGE: &str = "downloads/tampered.nifepkg";

/// **`noteless`'s package, the one every leg installs from the disk** (milestone 47 (navigation and
/// naming)'s bare-name lane). It took that job from [`DOWNLOADED_PACKAGE`] when DECISIONS §229
/// (calef, 2026-09-27) refused a package named after an image program; `uptime`'s file stays, for
/// the line proving that refusal. Provisional.
pub(crate) const DOWNLOADED_NOTELESS: &str = "downloads/noteless.nifepkg";

/// **`greeting` at 0.2.0, the second version, on the disk for every leg** (milestone 614 (two
/// installed versions of one program, each runnable, and a caller granted the one it needs)): the
/// recipe-built package whose member is `greeting_v2`'s bytes under the name `greeting`, so its
/// digest differs from 0.1.0's and the table can hold both. Provisional name (the constant's and
/// the path's).
///
/// The version is a directory, not part of the file's name, because the shell can only type a
/// component of at most sixteen bytes (`grant_plan`'s `MAX_NAME`, the width the grant wire
/// packs): `greeting-0.2.0.nifepkg` is twenty-two, and CI's first run of the line answered `that
/// is not a name: one component, at most 16 bytes`. This is the same wall the 614 block's BUGS
/// records for a remove operand, met by a fixture rather than a package, and answered the same
/// way: the name stays inside the sixteen bytes rather than the wire growing. The shape mirrors
/// the install path (`packages/greeting/0.2.0/greeting`), where the version is a directory too.
pub(crate) const DOWNLOADED_GREETING_0_2_0: &str = "downloads/0.2.0/greeting.nifepkg";

/// **What the gate's package source serves this architecture** (milestone 198 rung 3a's fetch):
/// `helpers/package-http-peer` reads `NIFE_PACKAGE_SOURCE`, and `script/swish-check` points it here,
/// one directory per architecture so two legs never share one. It holds `greeting`'s package as the
/// archive build wrote it, and `uptime`'s under its genuine name with the same tampering as
/// [`TAMPERED_PACKAGE`]: a lying mirror, which the catalogue must catch.
pub(crate) fn package_source_dir(architecture: &str) -> std::path::PathBuf {
    workspace_root().join(format!("target/package-source-{architecture}"))
}

/// **And a real program that nothing installed**: `unreachable_network_witness`, stripped, which
/// is in no activation generation. It is milestone 202 (every confinement test is a ritual until
/// somebody breaks the confinement)'s unvouched-child fixture: run on §219 (how the shell names an installed program to the spawner)'s gate D2, it probes the
/// process domain, entropy and the network and lists the slots it holds.
pub(crate) const INSTALLED_UNVOUCHED: &str = "installed/unvouched";

/// **Bytes nobody vouched for whose manifest note asks for an argument** (milestone 597 (a program
/// carries its manifest in an ELF note), provisional): `least_authority_demo`, stripped. Unvouched
/// bytes may hold only what the D2 ruling names, and an argument is a designation the line would
/// have to make, so `script/swish-check` types it with one and the progenitor must refuse it. Not
/// named after the program because a name is at most 16 bytes (`filesystem_protocol`'s
/// `MAX_NAME`), which `least_authority_demo` is not; CI found that on the first run.
pub(crate) const INSTALLED_ASKS_AN_ARGUMENT: &str = "installed/asks-an-arg";

/// **A program whose manifest note cannot be read** (milestone 597, provisional): the unvouched
/// witness again, with its note's version word set to one no reader knows. The shell must refuse it
/// at the prompt, before anything is sent, rather than run it as if it carried no note.
pub(crate) const INSTALLED_MALFORMED_NOTE: &str = "installed/malformed-note";
/// **A `std` program whose note says it hears words** (milestone 205 (how a foreign program is
/// told what to do)): `std_echo`, from the `std_exerciser` workspace, unvouched. Run by path, it is
/// an installed foreign program told what to do, which is the shape an installed `rg` will have.
/// Written only if `cargo xtask std-exerciser` built it, which `script/test` does; the gate skips
/// its lines otherwise, as it skips `std_exerciser`'s. Provisional.
pub(crate) const INSTALLED_STD_ECHO: &str = "installed/std-echo";
/// **A `std` program that reads what its words name** (milestone 205's designation half):
/// `std_grep`, from the same workspace, unvouched, so every word it is granted is read-only.
/// Written only if it was built, as [`INSTALLED_STD_ECHO`] is. Provisional.
pub(crate) const INSTALLED_STD_GREP: &str = "installed/std-grep";
/// **Unmodified `ripgrep`, if somebody built it** (milestone 595 (the shell runs a `std` program),
/// 2026-10-07): `helpers/build-ripgrep.sh`'s `rg`, which links the note `cargo xtask foreign-note`
/// writes, so it runs by path as an unvouched `std` program that hears its words. Written only when
/// `crate::farm::ripgrep_elf` exists, which `script/ci-build`'s `swish-check` rows arrange by
/// running `helpers/build-ripgrep.sh` first (since 2026-10-08 UTC; its crates.io fetch is a §46
/// (thin primitives or whole subsystems) question asked on that lane's pull request). Provisional. Installing it as a package is milestone 121's, not this.
pub(crate) const INSTALLED_RIPGREP: &str = "installed/rg";

/// **`std_echo` as large as `ripgrep`** (milestone 595 (the shell runs a `std` program), 2026-09-27,
/// provisional): its bytes followed by zeros to [`LARGE_IMAGE_BYTES`]. `rg` is in CI only in
/// `swish-check` (since 2026-10-08 UTC; the kernel suite still skips it), and this predates that: it
/// is what proves an image of its size travels from the prompt: past the old 64-page
/// cap, across the shell's 2 MiB page-table spans, through both staging copies, and into a region
/// sized from its length. A loader reads an ELF through its program headers, so bytes past the
/// last segment are carried, hashed and ignored. What it does not prove is a segment that large;
/// the kernel harness already loads `rg` itself (`kernel/src/user/ripgrep_tests.rs`).
pub(crate) const INSTALLED_STD_ECHO_LARGE: &str = "installed/std-echo-large";

/// How large [`INSTALLED_STD_ECHO_LARGE`] is: 3 MiB, 768 pages, about `rg` 14.1.1's image
/// (`notes/ripgrep-on-nife.md`) and under `spawnproto::IMAGE_MAX_PAGES`.
const LARGE_IMAGE_BYTES: usize = 3 * 1024 * 1024;

/// **Where the shell stands to search [`RIPGREP_CORPUS`]**: a directory at the image root holding
/// it. The shell builds a caretaker for a word only by descending into a directory, so at `/` it
/// refuses to grant a word at all ("`cd` into one first", found 2026-10-07 on aarch64); the line
/// is typed after `cd` here. Provisional.
pub(crate) const RIPGREP_WALK_HOME: &str = "search";

/// **The tree `rg` walks at the prompt** (milestone 121 (`ripgrep`: enumeration as a capability),
/// calef's 2026-10-07 ruling on fatal risk 1): every file `git ls-files crates` names, copied under
/// `/search/crates`. Hundreds of files in a few hundred nested directories, Rust, TOML, patches
/// and binary device trees, which is the walk `rg` was chosen to exercise. Before this it searched
/// one file in one directory at the prompt. Written only beside [`INSTALLED_RIPGREP`], since
/// nothing else reads it. Provisional.
pub(crate) const RIPGREP_CORPUS: &str = "crates";

/// **What `rg` looks for in [`RIPGREP_CORPUS`]**: about 350 lines in about 140 files on
/// 2026-10-07, spread across the tree because every crate with a `BUGS` section has one. The count
/// moves with the tree; the gate never states it, it compares against the host's answer.
pub(crate) const RIPGREP_WALK_PATTERN: &str = "BUGS";

/// **What real `rg` on the host printed for [`RIPGREP_WALK_PATTERN`] over [`RIPGREP_CORPUS`]**,
/// written when the corpus is staged and read by `script/swish-check` to compare the prompt's answer
/// line for line. One file for every architecture, since the corpus is the same bytes on each.
pub(crate) fn ripgrep_walk_expected() -> std::path::PathBuf {
    workspace_root().join("target/ripgrep-walk-expected.txt")
}

/// **The host `rg` the expected set comes from**: `helpers/build-ripgrep.sh`'s host build, which is
/// the same 14.1.1 source the guest runs, or failing that whatever `rg` is on `PATH`, whose version
/// the staging line prints so a mismatch between versions is visible rather than silent.
fn host_ripgrep() -> String {
    let built = crate::farm::ripgrep_elf("host");
    if built.exists() {
        built.display().to_string()
    } else {
        "rg".to_string()
    }
}

/// Stage [`RIPGREP_CORPUS`] under `tree` and write [`ripgrep_walk_expected`] by running the host's
/// `rg` over it, returning a sentence for the log.
///
/// The host run stands in [`RIPGREP_WALK_HOME`] and names [`RIPGREP_CORPUS`], exactly as the line
/// typed at the prompt does, so the paths it prints are the guest's. `--no-config` keeps a person's `RIPGREP_CONFIG_PATH`
/// out of it, and `--no-ignore-parent` keeps this worktree's `.gitignore` (which ignores `target/`,
/// where the copy lives) from hiding what the guest, which has no repository, will search. Neither
/// flag is typed at the prompt: they make the host see what the guest sees, not the other way round.
fn stage_ripgrep_corpus(tree: &std::path::Path) -> Result<String, String> {
    let root = workspace_root();
    let listed = std::process::Command::new("git")
        .args(["ls-files", "-z", "crates"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("git ls-files crates: {e}"))?;
    if !listed.status.success() {
        return Err("git ls-files crates failed".into());
    }
    let home = tree.join(RIPGREP_WALK_HOME);
    let corpus = home.join(RIPGREP_CORPUS);
    let (mut files, mut bytes) = (0usize, 0u64);
    let mut dirs = std::collections::BTreeSet::new();
    for path in listed.stdout.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let path = std::str::from_utf8(path).map_err(|e| format!("a path under crates/: {e}"))?;
        let Some(inner) = path.strip_prefix("crates/") else {
            continue;
        };
        let from = root.join(path);
        // A file deleted in the working tree but not yet from the index is listed and absent.
        if !from.is_file() {
            continue;
        }
        let to = corpus.join(inner);
        let parent = to.parent().expect("a file under corpus/ has a parent");
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        bytes += std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
        files += 1;
        let mut up = std::path::Path::new(inner).parent();
        while let Some(dir) = up.filter(|d| !d.as_os_str().is_empty()) {
            dirs.insert(dir.to_path_buf());
            up = dir.parent();
        }
    }
    let rg = host_ripgrep();
    let version = std::process::Command::new(&rg)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next().map(str::to_string))
        .ok_or_else(|| {
            format!(
                "no host `rg` to compute the expected matches with ({rg}); \
                 `helpers/build-ripgrep.sh` builds one"
            )
        })?;
    let out = std::process::Command::new(&rg)
        .args([
            "--no-config",
            "--no-ignore-parent",
            RIPGREP_WALK_PATTERN,
            RIPGREP_CORPUS,
        ])
        .current_dir(&home)
        .output()
        .map_err(|e| format!("{rg}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "host `{rg}` over {RIPGREP_CORPUS} exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let expected = ripgrep_walk_expected();
    std::fs::write(&expected, &out.stdout).map_err(|e| format!("{}: {e}", expected.display()))?;
    let matches = out.stdout.iter().filter(|b| **b == b'\n').count();
    Ok(format!(
        "/{RIPGREP_WALK_HOME}/{RIPGREP_CORPUS}: {files} files in {} directories, {bytes} bytes; host {version} finds \
         {matches} lines for `{RIPGREP_WALK_PATTERN}` ({})",
        dirs.len() + 1,
        expected.display()
    ))
}

/// **A program with no symbol table, for the disk** ([`INSTALLED_RIPGREP`]). `read_stripped` takes
/// only the debug sections, and keeps the symbols for whoever reads the archive later; `rg` 14.1.1's
/// are 1.7 MiB of its 4.6, which put it over `spawnproto::IMAGE_MAX_PAGES` (4 MiB) and the shell
/// refused it as "larger than an image may be" (found 2026-10-07 on aarch64). Run by path, nothing
/// reads them. Without them it is 2.7 to 2.9 MiB on all three architectures, and `--strip-all`
/// keeps the `PT_NOTE`, since that is a loaded segment's content and not a section the tool drops.
fn without_symbols(path: &std::path::Path, architecture: &str) -> Result<Vec<u8>, String> {
    let objcopy = crate::host::llvm_tool("llvm-objcopy")
        .ok_or("llvm-objcopy not found; the llvm-tools rustup component provides it")?;
    let out = workspace_root().join(format!("target/stripped/all-{architecture}-rg"));
    let ok = std::process::Command::new(&objcopy)
        .arg("--strip-all")
        .arg(path)
        .arg(&out)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return Err(format!("{objcopy} --strip-all {} failed", path.display()));
    }
    std::fs::read(&out).map_err(|e| format!("{}: {e}", out.display()))
}

/// **`bytes` with its one manifest note's version word replaced by `version`**, for
/// [`INSTALLED_MALFORMED_NOTE`]. The note is found by its header and owner, which
/// `manifest_note::Note::of` writes for every manifest; exactly one must be present, or the fixture
/// would prove nothing.
fn with_note_version(bytes: &[u8], version: u32) -> Result<Vec<u8>, String> {
    let mut head = Vec::new();
    head.extend_from_slice(&((manifest_note::OWNER.len() + 1) as u32).to_le_bytes());
    head.extend_from_slice(&(manifest_note::DESCRIPTOR_LEN as u32).to_le_bytes());
    head.extend_from_slice(&manifest_note::MANIFEST.to_le_bytes());
    head.extend_from_slice(b"nife\0\0\0\0");
    let at: Vec<usize> = bytes
        .windows(head.len())
        .enumerate()
        .filter(|(_, w)| *w == head.as_slice())
        .map(|(i, _)| i)
        .collect();
    let [at] = at[..] else {
        return Err(format!(
            "expected one manifest note to corrupt, found {}",
            at.len()
        ));
    };
    let mut out = bytes.to_vec();
    let v = at + head.len();
    out[v..v + 4].copy_from_slice(&version.to_le_bytes());
    Ok(out)
}

/// **Put a package on the RedoxFS image for the target to install** (milestone 198 rung 3a). Copies
/// the package the archive build just made for `architecture` (`target/packages/<stem>.nifepkg`,
/// the same bytes whose digest it packed into the image's catalogue) to [`DOWNLOADED_PACKAGE`], a
/// tampered copy to [`TAMPERED_PACKAGE`], and an unvouched program to [`INSTALLED_UNVOUCHED`].
///
/// **It installs nothing.** Until 2026-09-26 this wrote `activation/` and the program itself,
/// standing in for an installer; the installer now exists on the target (`package install`), and a
/// fresh disk has no activation set at all. The one stand-in left is the download: the booted
/// system's network reaches no package source yet, so the file arrives with the disk. Run after
/// the archive build, whose outputs it reads, and before the boot.
pub(crate) fn seed_installed(architecture: &str) -> bool {
    match stage_installed(architecture) {
        Ok(tree) => redoxfs_host(&["import", &redoxfs_disk_path(), &tree]),
        Err(complaint) => {
            eprintln!("seed_installed ({architecture}): {complaint}");
            false
        }
    }
}

fn stage_installed(architecture: &str) -> Result<String, String> {
    let root = workspace_root();
    let stem = format!("uptime-0.1.0-{architecture}");
    let built = root.join(format!("target/packages/{stem}.nifepkg"));
    let package = std::fs::read(&built).map_err(|e| {
        format!(
            "could not read {} (the archive build writes it): {e}",
            built.display()
        )
    })?;
    // Checked here with the target's own parser so a seed that could never install says so on the
    // host, rather than as a refusal at the prompt that reads like the installer's fault.
    let parsed = package_archive::Package::parse(&package)
        .map_err(|e| format!("{} does not parse: {e:?}", built.display()))?;
    let (Some(index), Some(member)) = (parsed.index_of("uptime"), parsed.read("uptime")) else {
        return Err(format!("{} carries no uptime member", built.display()));
    };
    // **The tampered copy is consistent with itself**: one byte of the program flipped *and* the
    // table of contents' digest for it rewritten to match. So the only thing on the target that can
    // tell it from a genuine package is the image's catalogue, and the refusal at the prompt is that
    // check and no other. A plain flipped byte was the first cut, and falsifying the catalogue check
    // left the line green, because the member's own digest refused it instead (2026-09-26).
    let offset = member.as_ptr() as usize - package.as_ptr() as usize;
    let mut tampered = package.clone();
    tampered[offset + member.len() / 2] ^= 1;
    let digest = package_archive::sha256(&tampered[offset..offset + member.len()]);
    let at = package_archive::HEADER_LEN
        + index * package_archive::MEMBER_LEN
        + package_archive::NAME_LEN
        + 8;
    tampered[at..at + digest.len()].copy_from_slice(&digest);
    let reread = package_archive::Package::parse(&tampered)
        .map_err(|e| format!("the tampered copy does not parse: {e:?}"))?;
    if reread.verify().is_err() {
        return Err("the tampered copy is not consistent with itself".into());
    }

    let triple = match architecture {
        "aarch64" => crate::TARGET,
        "riscv64" => crate::RISCV_TARGET,
        _ => crate::X86_TARGET,
    };
    let witness = root.join(format!(
        "target/{triple}/{}/unreachable_network_witness",
        crate::profile_dir()
    ));
    let unvouched = crate::inspect::read_stripped(&witness.display().to_string())
        .map_err(|e| format!("could not read {}: {e}", witness.display()))?;
    let malformed = with_note_version(&unvouched, 9)?;
    let demo = root.join(format!(
        "target/{triple}/{}/least_authority_demo",
        crate::profile_dir()
    ));
    let asks_an_argument = crate::inspect::read_stripped(&demo.display().to_string())
        .map_err(|e| format!("could not read {}: {e}", demo.display()))?;
    let std_grep = crate::farm::std_grep_elf(&format!("{architecture}-unknown-nife"));
    let std_grep = std_grep
        .exists()
        .then(|| crate::inspect::read_stripped(&std_grep.display().to_string()))
        .transpose()
        .map_err(|e| format!("could not read std_grep: {e}"))?;
    let ripgrep = crate::farm::ripgrep_elf(&format!("{architecture}-unknown-nife"));
    let ripgrep = ripgrep
        .exists()
        .then(|| without_symbols(&ripgrep, architecture))
        .transpose()?;
    let std_echo = crate::farm::std_echo_elf(&format!("{architecture}-unknown-nife"));
    let std_echo = std_echo
        .exists()
        .then(|| crate::inspect::read_stripped(&std_echo.display().to_string()))
        .transpose()
        .map_err(|e| format!("could not read std_echo: {e}"))?;

    // `greeting`, the package whose program no image carries: to the gate's package source, with
    // the lying `uptime`. Every leg fetches it (x86_64 since 2026-10-05, over the `e1000e`).
    let greeting_stem = format!("greeting-0.1.0-{architecture}");
    let greeting_built = root.join(format!("target/packages/{greeting_stem}.nifepkg"));
    let greeting = std::fs::read(&greeting_built).map_err(|e| {
        format!(
            "could not read {} (the archive build writes it): {e}",
            greeting_built.display()
        )
    })?;
    let noteless_stem = format!("noteless-0.1.0-{architecture}");
    let noteless_built = root.join(format!("target/packages/{noteless_stem}.nifepkg"));
    let noteless = std::fs::read(&noteless_built).map_err(|e| {
        format!(
            "could not read {} (the archive build writes it): {e}",
            noteless_built.display()
        )
    })?;
    // The second version, whose member bytes differ from 0.1.0's (milestone 614): the row it
    // makes is a second digest, which is the whole point.
    let greeting_v2_stem = format!("greeting-0.2.0-{architecture}");
    let greeting_v2_built = root.join(format!("target/packages/{greeting_v2_stem}.nifepkg"));
    let greeting_v2 = std::fs::read(&greeting_v2_built).map_err(|e| {
        format!(
            "could not read {} (the archive build writes it): {e}",
            greeting_v2_built.display()
        )
    })?;
    // **The claim the prompt cannot check: the image does not carry them.** Neither is a
    // `grant_plan::Prog`, so the progenitor's image-name refusal (§229) cannot see them either way,
    // and only the archive can say. Read the archive this leg boots and refuse to seed if it has
    // either program, since every line about them after that would prove nothing.
    let archive = match architecture {
        "aarch64" => crate::archive::initrd_path(),
        "riscv64" => crate::archive::riscv_initrd_path(),
        _ => crate::archive::x86_initrd_path(),
    };
    let image = std::fs::read(&archive).map_err(|e| format!("could not read {archive}: {e}"))?;
    let fs = nifefs::Fs::parse(&image).map_err(|e| format!("{archive} does not parse: {e:?}"))?;
    for program in ["greeting", "noteless"] {
        if fs.read(program).is_some() {
            return Err(format!(
                "{archive} carries `{program}`, so installing it would prove nothing about a \
                 program the image lacks (is it still `packaged_only` in fixtures/Cargo.toml?)"
            ));
        }
    }

    let source = package_source_dir(architecture);
    let _ = std::fs::remove_dir_all(&source);
    std::fs::create_dir_all(&source).map_err(|e| format!("{}: {e}", source.display()))?;
    std::fs::write(source.join(format!("{greeting_stem}.nifepkg")), &greeting)
        .map_err(|e| format!("{}: {e}", source.display()))?;
    std::fs::write(source.join(format!("{stem}.nifepkg")), &tampered)
        .map_err(|e| format!("{}: {e}", source.display()))?;

    let tree = root.join("target/redoxfs-installed");
    let _ = std::fs::remove_dir_all(&tree);
    let write = |path: std::path::PathBuf, bytes: &[u8]| {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))
    };
    write(tree.join(DOWNLOADED_PACKAGE), &package)?;
    write(tree.join(TAMPERED_PACKAGE), &tampered)?;
    write(tree.join(INSTALLED_UNVOUCHED), &unvouched)?;
    write(tree.join(INSTALLED_ASKS_AN_ARGUMENT), &asks_an_argument)?;
    write(tree.join(INSTALLED_MALFORMED_NOTE), &malformed)?;
    if let Some(bytes) = &std_echo {
        write(tree.join(INSTALLED_STD_ECHO), bytes)?;
        let mut large = bytes.clone();
        large.resize(LARGE_IMAGE_BYTES.max(bytes.len()), 0);
        write(tree.join(INSTALLED_STD_ECHO_LARGE), &large)?;
    }
    if let Some(bytes) = &std_grep {
        write(tree.join(INSTALLED_STD_GREP), bytes)?;
    }
    if let Some(bytes) = &ripgrep {
        write(tree.join(INSTALLED_RIPGREP), bytes)?;
        let said = stage_ripgrep_corpus(&tree)?;
        eprintln!("seed_installed ({architecture}): {said}");
    }
    write(tree.join(DOWNLOADED_NOTELESS), &noteless)?;
    write(tree.join(DOWNLOADED_GREETING_0_2_0), &greeting_v2)?;
    eprintln!(
        "seed_installed ({architecture}): {stem} ({} bytes, digest {}) at {DOWNLOADED_PACKAGE}, \
         a tampered copy, {noteless_stem} at \
         {DOWNLOADED_NOTELESS}, {greeting_v2_stem} at {DOWNLOADED_GREETING_0_2_0}, and three \
         unvouched programs; no activation set. The package source at {} serves {greeting_stem} \
         and a lying {stem}",
        package.len(),
        String::from_utf8_lossy(&measured_boot::digest_text(&package_archive::sha256(
            &package
        ))),
        source.display(),
    );
    Ok(tree.display().to_string())
}

/// Stage milestone 47's subtree as a host directory and return its path, for `import` to copy into
/// the image root.
///
/// **`import` rather than a new `mkdir` verb on the host tool**, deliberately: `import` is upstream
/// RedoxFS's own archiver (`redoxfs::archive`), so the directories the confinement tests attack are
/// written by the people who defined the format rather than by us. notes/host-recovery.md already
/// makes that argument for `extract`; this is the same one on the write side.
///
/// The staging directory is rebuilt from scratch each time, because a name left behind by an older
/// fixture would end up in the image and fail the post-run `ls /` check for a reason that has
/// nothing to do with the run.
fn stage_subtree() -> Option<String> {
    use filesystem_protocol::fixture::tree;
    let root = workspace_root().join("target/redoxfs-tree");
    let _ = std::fs::remove_dir_all(&root);
    let sub = root.join(tree::SUB);
    let deeper = sub.join(tree::DEEPER);
    let other = root.join(tree::OTHER);
    // Milestone 47's `rm -r` tree, a sibling of `sub` so a capability to one is provably not a
    // capability to the other. `rm-keep` sits beside the doomed tree, inside the same grant: the
    // program could have removed it and does not, because nothing named it.
    let rmtree = root.join(tree::RMTREE);
    let doomed = rmtree.join(tree::RM_DOOMED);
    let nested = doomed.join(tree::RM_NESTED);
    // Milestone 47's globbing tree, a sibling of both. Two names the pattern matches and two it does
    // not, in one directory, so "the grant is what matched" is a claim about *which* names.
    let globset = root.join(tree::GLOBSET);
    let globdir = globset.join(tree::GLOB_DIR);
    // Milestone 109's batching tree, a sibling of the globbing one: eleven names one pattern
    // matches, which is more than a single grant can carry, so `xargs` has a real directory to
    // sweep at a real prompt. Its own directory rather than more files in `globset`, because the
    // globbing lane asserts that `gl-*.txt` matches exactly two names.
    let globmany = root.join(tree::GLOBMANY);
    // Milestone 50's redirection tree, a sibling of all of them: the witness shell writes files into
    // its root, and it needs somewhere those writes cannot be confused with another test's.
    let redir = root.join(tree::REDIR);
    let mut ok = std::fs::create_dir_all(&globmany).is_ok()
        && std::fs::write(globmany.join(tree::MANY_MISS), tree::MANY_BODY).is_ok();
    for name in tree::MANY_NAMES {
        ok = ok && std::fs::write(globmany.join(name), tree::MANY_BODY).is_ok();
    }
    let ok = ok
        && std::fs::create_dir_all(&deeper).is_ok()
        && std::fs::create_dir_all(&other).is_ok()
        && std::fs::create_dir_all(&nested).is_ok()
        && std::fs::create_dir_all(&globdir).is_ok()
        && std::fs::create_dir_all(&redir).is_ok()
        && std::fs::write(redir.join(tree::REDIR_ONE), tree::REDIR_BODY).is_ok()
        && std::fs::write(redir.join(tree::REDIR_TWO), tree::REDIR_BODY).is_ok()
        && std::fs::write(globset.join(tree::GLOB_ONE), tree::GLOB_BODY).is_ok()
        && std::fs::write(globset.join(tree::GLOB_TWO), tree::GLOB_BODY).is_ok()
        && std::fs::write(globset.join(tree::GLOB_MISS), tree::GLOB_BODY).is_ok()
        && std::fs::write(globdir.join(tree::GLOB_INNER), tree::GLOB_BODY).is_ok()
        && std::fs::write(sub.join(tree::INNER), tree::INNER_BODY).is_ok()
        && std::fs::write(deeper.join(tree::LEAF), tree::LEAF_BODY).is_ok()
        && std::fs::write(other.join(tree::SECRET), tree::SECRET_BODY).is_ok()
        && std::fs::write(rmtree.join(tree::RM_KEEP), tree::RM_KEEP_BODY).is_ok()
        && std::fs::write(rmtree.join(tree::RM_SOLO), tree::RM_BODY).is_ok()
        && std::fs::write(doomed.join(tree::RM_ONE), tree::RM_BODY).is_ok()
        && std::fs::write(doomed.join(tree::RM_TWO), tree::RM_BODY).is_ok()
        && std::fs::write(nested.join(tree::RM_LEAF), tree::RM_BODY).is_ok();
    if !ok {
        eprintln!("mkredoxfs: cannot stage the milestone-47 subtree");
        return None;
    }
    // Milestone 121 (`ripgrep` on nife: enumeration as a capability)'s priced tree, a sibling of
    // everything above and written by nothing after this: the walk pricing reads it through a grant
    // of its own, so its figures cannot depend on which test ran first. Staged by the same function
    // a host comparison stages it with. Milestone 835 (a C library, stage 1: files, clock and
    // memory)'s writable directory, empty: SQLite's `speedtest1` makes its database there.
    if std::fs::create_dir_all(root.join(c_program_fixture::ROOT)).is_err() {
        eprintln!("mkredoxfs: cannot stage milestone 835's directory");
        return None;
    }
    let walk = root.join(filesystem_protocol::fixture::walk::ROOT);
    if let Err(e) = walk_pricing::stage(&walk) {
        eprintln!("mkredoxfs: cannot stage milestone 121's priced tree: {e}");
        return None;
    }
    Some(root.display().to_string())
}

/// Where the **crash test's** RedoxFS image is written (milestone 37). The runners derive exactly
/// this name from `NIFE_DISK`, the way they derive the shared one.
pub(crate) fn crash_disk_path() -> String {
    workspace_root()
        .join("target/nifefs-redoxfs-crash.img")
        .display()
        .to_string()
}

/// Build the crash test's image: an empty filesystem with one file, `cut`, holding a known value.
///
/// **Its own disk, and regenerated every run, both deliberately** (milestone 37, DECISIONS §34
/// condition 1). The crash test kills an FS server mid-transaction and leaves the filesystem
/// half-written on purpose. Pointing it at the shared fixture would make every other FS test's
/// result depend on whether this one had run first, and keeping the image across runs would make
/// each run's starting state a function of the last one's damage. Both are the order-coupled fixture
/// DECISIONS §27 spent a day on, so `NIFE_KEEP_REDOXFS` deliberately does **not** apply here: the
/// cross-boot case is interesting for the shared disk and is nothing but noise for this one.
pub(crate) fn mkredoxfs_crash() -> bool {
    let img = crash_disk_path();
    let initial = workspace_root().join("target/redoxfs-crash-initial.tmp");
    if std::fs::write(&initial, filesystem_protocol::fixture::crash::INITIAL).is_err() {
        eprintln!("mkredoxfs_crash: cannot stage the fixture file");
        return false;
    }
    let initial = initial.display().to_string();
    redoxfs_host(&["mkfs", &img, "16"])
        && redoxfs_host(&[
            "put",
            &img,
            filesystem_protocol::fixture::crash::NAME,
            &initial,
        ])
}

/// Where the GPT-partitioned test image is written. The runners derive exactly this name from
/// `NIFE_DISK` (`${NIFE_DISK%.img}-gpt.img`), so the two stay in lockstep.
fn gpt_disk_path() -> String {
    workspace_root()
        .join("target/nifefs-gpt.img")
        .display()
        .to_string()
}

/// Build the milestone-57 test disk: a 64 MiB image whose partition table **`sgdisk` wrote**.
///
/// The point of this image is its provenance. `crates/globally_unique_identifier_partition_table`
/// can lay out a table, and a disk it laid out would test the reader against the writer, which is
/// the weakest test available: every mistake made on the way out is made symmetrically on the way
/// in. So the bytes come from the committed fixture in
/// `crates/globally_unique_identifier_partition_table/tests/fixtures/`, produced by `sgdisk` 1.0.10
/// (gptfdisk, C++), and the guest reads a table this project did not write. The regeneration
/// commands are at the top of
/// `crates/globally_unique_identifier_partition_table/tests/real_disks.rs`.
///
/// The fixture is the first 34 and last 33 blocks of a 64 MiB disk, which is exactly the primary
/// table and the backup table with the 64 MiB of nothing between them left out. Reconstituting it is
/// therefore the head, a run of zeros, and the tail. Nothing is put in the partitions: what is under
/// test is finding them.
pub(crate) fn mkgptdisk() -> bool {
    const BLOCK: usize = 512;
    const BLOCKS: usize = 131_072; // 64 MiB
    let dir =
        workspace_root().join("crates/globally_unique_identifier_partition_table/tests/fixtures");
    let (Ok(head), Ok(tail)) = (
        std::fs::read(dir.join("sgdisk-64m.head")),
        std::fs::read(dir.join("sgdisk-64m.tail")),
    ) else {
        eprintln!("mkgptdisk: cannot read the sgdisk fixtures");
        return false;
    };
    if head.len() != 34 * BLOCK || tail.len() != 33 * BLOCK {
        eprintln!(
            "mkgptdisk: the fixtures are {} and {} bytes; expected {} and {}",
            head.len(),
            tail.len(),
            34 * BLOCK,
            33 * BLOCK,
        );
        return false;
    }
    let mut img = std::vec![0u8; BLOCKS * BLOCK];
    img[..head.len()].copy_from_slice(&head);
    let at = BLOCKS * BLOCK - tail.len();
    img[at..].copy_from_slice(&tail);
    let path = gpt_disk_path();
    if let Err(e) = std::fs::write(&path, &img) {
        eprintln!("mkgptdisk: could not write {path}: {e}");
        return false;
    }
    true
}

/// Where the blank test disk is written. The runners derive exactly this name from `NIFE_DISK`
/// (`${NIFE_DISK%.img}-blank.img`), so the two stay in lockstep.
pub(crate) fn blank_disk_path() -> String {
    workspace_root()
        .join("target/nifefs-blank.img")
        .display()
        .to_string()
}

/// Where the NVMe test image is written; the runners take the full path in `NIFE_NVME` rather
/// than deriving it, because unlike the mmio disks it does not ride beside `NIFE_DISK`.
pub(crate) fn nvme_disk_path() -> String {
    workspace_root()
        .join("target/nife-nvme.img")
        .display()
        .to_string()
}

/// The NVMe test image (milestone 53's storage half): 8 MiB of zeros behind QEMU's `-device nvme`.
/// **Neither number is load-bearing any more** (milestone 318): the boot test compares the
/// server's size answer against the geometry the kernel read from IDENTIFY rather than against a
/// constant, and proves a write landed where it said by writing a second block rather than by
/// expecting an untouched one to read as zeros. So this file may be any size a controller will
/// take, which is what lets the same test run against xenon's 256 GB Micron. Zeros and 8 MiB
/// because a zero file is cheap to make and small is fast to attach. Regenerated per leg like the blank disk, and for the same reason: the
/// test writes it, and a leg starting from the previous leg's damage is not reproducible alone.
pub(crate) fn mknvmedisk() -> bool {
    let path = nvme_disk_path();
    if let Err(e) = std::fs::write(&path, std::vec![0u8; 8 * 1024 * 1024]) {
        eprintln!("mknvmedisk: could not write {path}: {e}");
        return false;
    }
    true
}

/// Build milestone 57's write-half disk: 64 MiB of **zeros**, and that is the whole point.
///
/// It carries no table, no filesystem and no fixture, because what the guest is going to do to it is
/// write both. Regenerated every run and never shared, for milestone 37's reason (DECISIONS §27): a
/// test that partitions a disk cannot be pointed at an image another test reads, and a test whose
/// starting state is last run's damage is not reproducible on its own. `NIFE_KEEP_REDOXFS` does
/// not apply here for the same reason it does not apply to the crash image.
pub(crate) fn mkblankdisk() -> bool {
    let path = blank_disk_path();
    let bytes = std::vec![0u8; (filesystem_protocol::fixture::blank::DISK_BLOCKS * filesystem_protocol::fixture::blank::LBA) as usize];
    if let Err(e) = std::fs::write(&path, &bytes) {
        eprintln!("mkblankdisk: could not write {path}: {e}");
        return false;
    }
    true
}
