//! **milestone 223 (read a card and say whether its kernel and archive match), before the power cycle.**
//!
//! `cargo xtask card-check [path]`, and `script/card-check` is the front door. With no path it reads
//! `target/board`, the payload this tree last built; with one it reads a mounted card, a staged
//! stick directory, or a single UEFI boot file.
//!
//! The comparison is `sealed_pair`'s and is documented there: the SHA-256 of each archive entry the
//! kernel may enter, looked for in the kernel image's compiled-in trust root. This file finds the
//! two files and says what to do about the answer before a person walks to the board.

//! # Which half is stale
//!
//! `sealed_pair` cannot say on its own, because a digest the kernel does not carry is not a digest
//! anything can find in it. This file compares both files against what the tree last built, in
//! `target/board`. When that build is absent or itself stale the answer is "one of these two", and
//! it says so rather than guessing.

use std::path::{Path, PathBuf};

use crate::host::workspace_root;

/// The names `script/board-image` writes and copies onto a card.
const CARD_KERNEL: &str = "nife-vf2.img";
const CARD_ARCHIVE: &str = "nife-initial-ramdisk.img";

/// Where the last `script/board-image` left the payload.
fn built_payload() -> PathBuf {
    workspace_root().join("target/board")
}

/// One pair to judge: two files, or one file with both halves inside it.
struct Pair {
    /// What to print for the kernel side.
    kernel_label: String,
    /// What to print for the archive side.
    archive_label: String,
    kernel: Vec<u8>,
    archive: Vec<u8>,
    /// The file on disk holding the kernel, when the two are separate files. `None` for a stick,
    /// where they are one file and cannot be stale against each other.
    kernel_path: Option<PathBuf>,
    /// The file on disk holding the archive, on the same terms.
    archive_path: Option<PathBuf>,
}

/// **The command.** Exit codes are the contract, because this is meant to be run from a script and
/// from a person's shell before a bench evening:
///
/// - `0`: every pair found is sealed.
/// - `1`: something is not sealed. The board would refuse it.
/// - `2`: nothing could be judged (no payload at that path, or a file that is not an archive).
///
/// A refusal and an unreadable path are deliberately different codes. "This card is wrong" and "I
/// am looking in the wrong place" want different reactions from whoever ran it.
pub(crate) fn card_check(arg: Option<String>) -> std::process::ExitCode {
    let root = arg.map_or_else(built_payload, PathBuf::from);
    let pairs = match collect(&root) {
        Ok(pairs) => pairs,
        Err(why) => {
            eprintln!("card-check: {why}");
            return std::process::ExitCode::from(2);
        }
    };

    let mut refused = false;
    for pair in &pairs {
        let seal = match sealed_pair::inspect(&pair.kernel, &pair.archive) {
            Ok(seal) => seal,
            Err(why) => {
                println!("UNREADABLE: {} {why}", pair.archive_label);
                refused = true;
                continue;
            }
        };
        println!("{}", seal.explain(&pair.kernel_label, &pair.archive_label));
        if !seal.is_sealed() {
            refused = true;
            print_the_fix(pair);
            println!("\n{}", sealed_pair::THE_BOARD_IS_THE_AUTHORITY);
        }
    }

    if refused {
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}

/// Everything at `root` that is a kernel-and-archive pair: a card's two files, a stick's boot files,
/// or a single file named directly.
fn collect(root: &Path) -> Result<Vec<Pair>, String> {
    if root.is_file() {
        return Ok(vec![embedded(root)?]);
    }
    if !root.is_dir() {
        return Err(format!(
            "{} is neither a directory nor a file. Mount the card first, or run \
             `script/board-image` to build a payload to check.",
            root.display()
        ));
    }

    let mut pairs = Vec::new();
    let kernel = root.join(CARD_KERNEL);
    let archive = root.join(CARD_ARCHIVE);
    if kernel.is_file() && archive.is_file() {
        pairs.push(Pair {
            kernel_label: kernel.display().to_string(),
            archive_label: archive.display().to_string(),
            kernel: read(&kernel)?,
            archive: read(&archive)?,
            kernel_path: Some(kernel),
            archive_path: Some(archive),
        });
    } else if kernel.is_file() || archive.is_file() {
        // Half a pair is the failure milestone 217 (the card carries a kernel and an archive
        // from different builds) narrowed and could not remove, and it is worth its own sentence: a
        // person who copied one file is about to meet the refusal rather than a missing file.
        return Err(format!(
            "{} holds only half the payload ({} without {}). Copy the set:\n  \
             script/board-image --card {}",
            root.display(),
            if kernel.is_file() {
                CARD_KERNEL
            } else {
                CARD_ARCHIVE
            },
            if kernel.is_file() {
                CARD_ARCHIVE
            } else {
                CARD_KERNEL
            },
            root.display()
        ));
    }

    // The stick's shape, milestone 441 (the program that makes the stick): one file per
    // architecture under the path UEFI fixes for removable media, with the kernel and the archive
    // both inside it.
    let boot_dir = root.join("EFI/BOOT");
    if boot_dir.is_dir() {
        let mut names: Vec<_> = std::fs::read_dir(&boot_dir)
            .map_err(|e| format!("cannot read {}: {e}", boot_dir.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("efi")))
            .collect();
        names.sort();
        for path in names {
            pairs.push(embedded(&path)?);
        }
    }

    if pairs.is_empty() {
        return Err(format!(
            "{} holds no nife payload: no {CARD_KERNEL} beside a {CARD_ARCHIVE}, and no \
             EFI/BOOT/*.EFI.",
            root.display()
        ));
    }
    Ok(pairs)
}

/// A single boot file with both halves in it: judge the file against the archive inside it.
///
/// The kernel side is the whole file rather than a slice of it, which is sound for the reason
/// `sealed_pair` gives: the check is that the digest's bytes are somewhere in the image, and the
/// image is in there.
fn embedded(path: &Path) -> Result<Pair, String> {
    let blob = read(path)?;
    let archive = sealed_pair::embedded_archive(&blob)
        .ok_or_else(|| {
            format!(
                "{} carries no nifefs archive. A boot file built with no archive is a legitimate \
                 build (the kernel boots its tour and says it has no module), and there is nothing \
                 here to check.",
                path.display()
            )
        })?
        .to_vec();
    Ok(Pair {
        kernel_label: format!("the kernel in {}", path.display()),
        archive_label: format!("the archive in {}", path.display()),
        kernel: blob,
        archive,
        kernel_path: None,
        archive_path: None,
    })
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// **Which half to copy, and the command that copies it.** A refusal a person cannot act on is only
/// slightly better than the board's.
fn print_the_fix(pair: &Pair) {
    let (Some(kernel_path), Some(archive_path)) = (&pair.kernel_path, &pair.archive_path) else {
        // A stick's two halves are sealed together when the boot file is built
        // (`uefi_loader/build.rs`), so a stick that fails this check was not built by this tree's
        // own path, and the fix is to build it again rather than to copy one file.
        println!(
            "  This is one file with both halves inside it, so neither can be copied on its own:\n  \
             rebuild it with `cargo xtask stick`, whose build refuses an unsealed pair outright."
        );
        return;
    };

    let built_kernel = built_payload().join(CARD_KERNEL);
    let built_archive = built_payload().join(CARD_ARCHIVE);
    let kernel_is_current = same_file(kernel_path, &built_kernel);
    let archive_is_current = same_file(archive_path, &built_archive);
    let card = kernel_path.parent().unwrap_or(Path::new(".")).display();

    match (kernel_is_current, archive_is_current) {
        (Some(true), Some(false)) => println!(
            "  The kernel is the one this tree last built and the archive is not, so the ARCHIVE \
             is stale.\n  Copy the set (never one file):\n    script/board-image --card {card}"
        ),
        (Some(false), Some(true)) => println!(
            "  The archive is the one this tree last built and the kernel is not, so the KERNEL is \
             stale.\n  Copy the set (never one file):\n    script/board-image --card {card}"
        ),
        (Some(false), Some(false)) => println!(
            "  Neither file is the one this tree last built, so this payload is older than \
             {}.\n  Build and copy the set:\n    script/board-image --card {card}",
            built_payload().display()
        ),
        (Some(true), Some(true)) => println!(
            "  Both files ARE the ones this tree last built, which means the build itself produced \
             an unsealed pair.\n  That is the build's bug, not the card's: the archive must be \
             packed before the kernel is compiled, which is what `script/board-image` does. Run it \
             again and check {} before copying.",
            built_payload().display()
        ),
        _ => println!(
            "  Which half is stale cannot be said from here: {} holds no payload to compare \
             against.\n  Build one and copy the set:\n    script/board-image --card {card}",
            built_payload().display()
        ),
    }
}

/// Whether two paths hold the same bytes. `None` when the reference is not there to compare with,
/// which is an answer rather than a failure: a tree that has not built a payload cannot say which
/// half of somebody's card is old.
fn same_file(a: &Path, b: &Path) -> Option<bool> {
    let want = std::fs::read(b).ok()?;
    let have = std::fs::read(a).ok()?;
    Some(want == have)
}
