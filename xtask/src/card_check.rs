//! **milestone 223 (read a card and say whether its kernel and archive match), before the power cycle.**
//!
//! `cargo xtask card-check [path]`, and `script/card-check` is the front door. With no path it reads
//! `target/board`, the payload this tree last built; with one it reads a mounted card, a staged
//! stick directory, or a single UEFI boot file.
//!
//! The comparison is `sealed_pair`'s and is documented there: the SHA-256 of each archive entry the
//! kernel may enter, looked for in the kernel image's compiled-in trust root. This file finds the
//! two files and says what to do about the answer before a person walks to the board.
//!
//! # The words the boot script hands over
//!
//! The kernel also holds the archive to the loader's two digest words, and on radon the loader's
//! words are whatever the boot script's `setenv bootargs` line says. So the words judged are read
//! out of the scripts themselves, `boot.scr.uimg` and, in a served directory, the served script,
//! rather than out of the `.loader-digests` file they were written from: a card carries no such
//! file, and a script that disagrees with its archive is the thing that halts the board. A card
//! script that takes the archive over the network is refused outright, because it carries digests
//! for whatever the network serves and goes stale on the next build (#3690).

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
/// The loader's two digest words, the third file of the set.
const CARD_WORDS: &str = "nife-initial-ramdisk.img.loader-digests";

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
    /// The loader's two digest words, each with where it was read: every `setenv bootargs` in the
    /// boot scripts beside the pair, or the `.loader-digests` file when there is no script. Empty
    /// for a stick, whose loader checks them when it is built.
    words: Vec<Words>,
    /// What is wrong with the boot scripts themselves, each a refusal on its own.
    script_refusals: Vec<String>,
}

/// One set of the loader's words and the file they came from.
struct Words {
    label: String,
    words: String,
    /// Whether they came from the served script, which a rebuild rewrites, rather than from a card.
    served: bool,
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
        refused |= judge(pair);
    }

    if refused {
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}

/// Judge one pair, against every set of words its boot scripts hand over, print the verdict and
/// the fix, and say whether anything was refused.
fn judge(pair: &Pair) -> bool {
    let mut refused = false;
    let judged: Vec<Option<&Words>> = if pair.words.is_empty() {
        vec![None]
    } else {
        pair.words.iter().map(Some).collect()
    };
    for words in judged {
        let seal = match sealed_pair::inspect_with_words(
            &pair.kernel,
            &pair.archive,
            words.map(|w| w.words.as_str()),
        ) {
            Ok(seal) => seal,
            Err(why) => {
                println!("UNREADABLE: {} {why}", pair.archive_label);
                refused = true;
                break;
            }
        };
        let explained = seal.explain(&pair.kernel_label, &pair.archive_label);
        match words {
            Some(w) => println!(
                "{}\n  (the loader's words judged are the ones in {})",
                explained.trim_end(),
                w.label
            ),
            None => println!("{explained}"),
        }
        if !seal.is_sealed() {
            refused = true;
            match words {
                Some(w) if !seal.kernel_refuses() && !seal.progenitor_refuses() => {
                    print_the_words_fix(pair, w);
                }
                _ => print_the_fix(pair),
            }
            println!("\n{}", sealed_pair::AUTHORITY_NOTICE);
        }
    }
    for why in &pair.script_refusals {
        println!("NOT SEALED: {why}");
        refused = true;
    }
    refused
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
        let (words, script_refusals) = boot_script_words(root);
        pairs.push(Pair {
            kernel_label: kernel.display().to_string(),
            archive_label: archive.display().to_string(),
            kernel: read(&kernel)?,
            archive: read(&archive)?,
            kernel_path: Some(kernel),
            archive_path: Some(archive),
            words,
            script_refusals,
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
        words: Vec::new(),
        script_refusals: Vec::new(),
    })
}

/// **The words the boot scripts at `root` hand the kernel**, and what is wrong with the scripts.
///
/// The card's script is always read when it is there; the served script only in a directory that
/// has one, which is `target/board` after a `--tftp` build. With neither, the `.loader-digests`
/// file beside the archive stands in, so a directory of loose files is still judged.
fn boot_script_words(root: &Path) -> (Vec<Words>, Vec<String>) {
    let mut words = Vec::new();
    let mut refusals = Vec::new();
    for (name, served) in [
        (crate::board::BOARD_CARD_SCRIPT, false),
        (crate::board::BOARD_SERVED_SCRIPT, true),
    ] {
        let path = root.join(name);
        if !path.is_file() {
            continue;
        }
        let label = path.display().to_string();
        let text = match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|image| crate::board::uboot_script_text(&image))
        {
            Ok(text) => text,
            Err(why) => {
                refusals.push(format!(
                    "{label} cannot be read as U-Boot will read it ({why})."
                ));
                continue;
            }
        };
        let found = crate::board::script_bootargs(&text);
        if found.is_empty() {
            refusals.push(format!(
                "{label} sets no `bootargs`, so the kernel is handed no digest words and halts at \
                 MEASURED BOOT REFUSED."
            ));
        }
        for value in found {
            words.push(Words {
                label: label.clone(),
                words: value.to_string(),
                served,
            });
        }
        if !served && crate::board::card_script_fetches_the_archive(&text) {
            refusals.push(format!(
                "{label} takes the archive over the network but carries its own digest words, so \
                 it names the build it was written with and refuses every later one. Rewrite it \
                 once with a card script that fetches the served script instead:\n  \
                 script/board-image --tftp --card {}",
                root.display()
            ));
        }
    }
    if words.is_empty()
        && refusals.is_empty()
        && let Ok(file) = std::fs::read_to_string(root.join(CARD_WORDS))
    {
        words.push(Words {
            label: root.join(CARD_WORDS).display().to_string(),
            words: file,
            served: false,
        });
    }
    (words, refusals)
}

/// **The fix when only the words are wrong**: the kernel and the archive are one pair, and the
/// script that hands the kernel their digests names another build.
fn print_the_words_fix(pair: &Pair, words: &Words) {
    let dir = pair
        .archive_path
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(Path::new("."))
        .display()
        .to_string();
    if words.served {
        println!(
            "  The served script {} names another build's archive. Build again, which writes the \
             served script beside the archive it names:\n    script/board-image --tftp",
            words.label
        );
    } else {
        println!(
            "  {} names another build's archive than the one beside it. Copy the set, script \
             and all (add --tftp for a card that fetches over the network):\n    \
             script/board-image --card {dir}",
            words.label
        );
    }
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
             is stale.\n  Copy the set of three files (never one):\n    script/board-image --card {card}"
        ),
        (Some(false), Some(true)) => println!(
            "  The archive is the one this tree last built and the kernel is not, so the KERNEL is \
             stale.\n  Copy the set of three files (never one):\n    script/board-image --card {card}"
        ),
        (Some(false), Some(false)) => println!(
            "  Neither file is the one this tree last built, so this payload is older than \
             {}.\n  Build and copy the set of three files:\n    script/board-image --card {card}",
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
             against.\n  Build one and copy the set of three files:\n    script/board-image --card {card}",
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

#[cfg(test)]
mod tests {
    use sealed_pair::test_support;

    use super::*;
    use crate::board::{
        BOARD_BOOT_SCRIPT_NAME, BOARD_CARD_SCRIPT, BOARD_SERVED_SCRIPT, board_network_boot_script,
        board_served_boot_script, uboot_script_image,
    };

    /// A card directory in a scratch place, holding one build's kernel and archive.
    struct Card(PathBuf);

    impl Card {
        fn new(what: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("nife-card-check-{what}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch card");
            let (kernel, archive) = test_support::matched_pair();
            std::fs::write(dir.join(CARD_KERNEL), kernel).expect("kernel");
            std::fs::write(dir.join(CARD_ARCHIVE), archive).expect("archive");
            Card(dir)
        }

        fn script(&self, name: &str, text: &str) {
            std::fs::write(
                self.0.join(name),
                uboot_script_image(BOARD_BOOT_SCRIPT_NAME, text),
            )
            .expect("script");
        }

        fn refused(&self) -> bool {
            collect(&self.0).expect("a pair").iter().any(judge)
        }
    }

    impl Drop for Card {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn words(pair: (Vec<u8>, Vec<u8>)) -> String {
        test_support::words_for(&pair.1)
    }

    /// **A card whose script names its own archive passes; one whose script names another
    /// build's is refused**, read out of the script image itself, which is what U-Boot hands the
    /// kernel. No `.loader-digests` file is on either card, as none is on a real one.
    #[test]
    fn the_words_judged_are_the_card_scripts() {
        let good = Card::new("good");
        good.script(
            BOARD_CARD_SCRIPT,
            &board_network_boot_script("10.1.2.3", &words(test_support::matched_pair())),
        );
        assert!(!good.refused());

        let stale = Card::new("stale");
        stale.script(
            BOARD_CARD_SCRIPT,
            &board_network_boot_script("10.1.2.3", &words(test_support::other_pair())),
        );
        assert!(stale.refused());
    }

    /// **The served script is judged too**, so a served directory whose script names another
    /// build's archive fails `script/board-image`'s own check before any board asks for it.
    #[test]
    fn a_served_script_naming_another_build_is_refused() {
        let served = Card::new("served");
        let mine = words(test_support::matched_pair());
        served.script(
            BOARD_CARD_SCRIPT,
            &board_network_boot_script("10.1.2.3", &mine),
        );
        served.script(BOARD_SERVED_SCRIPT, &board_served_boot_script(&mine));
        assert!(!served.refused());
        served.script(
            BOARD_SERVED_SCRIPT,
            &board_served_boot_script(&words(test_support::other_pair())),
        );
        assert!(served.refused());
    }

    /// **The shape #3690 found on a card is refused even when its words agree today**: a card
    /// script that takes the archive over the network names one build and goes stale on the next.
    #[test]
    fn a_card_script_that_fetches_the_archive_is_refused() {
        let old = Card::new("old-shape");
        let mine = words(test_support::matched_pair());
        old.script(BOARD_CARD_SCRIPT, &board_served_boot_script(&mine));
        assert!(old.refused());
    }

    /// A script that is not a script image, or hands the kernel nothing, is a refusal, never a
    /// silent pass for want of words to judge.
    #[test]
    fn an_unreadable_or_wordless_script_is_refused() {
        let garbage = Card::new("garbage");
        std::fs::write(garbage.0.join(BOARD_CARD_SCRIPT), b"not an image").expect("write");
        assert!(garbage.refused());

        let wordless = Card::new("wordless");
        wordless.script(BOARD_CARD_SCRIPT, "echo nothing\n");
        assert!(wordless.refused());
    }
}
