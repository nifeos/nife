//! **The conversation with the person running the program**, and every decision in it.
//!
//! Everything between "the program started" and "the files are on the stick" lives here, behind
//! two small traits: [`Host`], which is the machine's disks, and [`Console`], which is the person.
//! `src/host/` implements both for real; the tests below implement them with fakes, which is how the
//! refusals that matter most are proved in milliseconds rather than trusted:
//!
//! - nothing is erased without `--erase` (non-interactive) or the disk's name typed back;
//! - a disk the offer rule refuses stays refused when named with `--disk`;
//! - a host that cannot erase says so before anyone is asked to confirm anything.

use std::path::{Path, PathBuf};

use crate::disk::{self, Disk, Plan, Policy, human_size};
use crate::payload::{self, File, architecture};
use crate::write;

/// The machine's disks, and the few acts that change one.
pub trait Host {
    /// Every disk the host can describe, offered or not.
    fn discover(&self) -> Result<Vec<Disk>, String>;
    /// `Err` with a sentence when this host cannot erase `disk` at all, asked **before** the person
    /// is asked to confirm (Windows prints its `diskpart` steps here).
    fn can_erase(&self, disk: &Disk) -> Result<(), String>;
    /// Erase `disk` as one FAT32 volume and return where that volume is mounted.
    fn erase(&self, disk: &Disk) -> Result<PathBuf, String>;
    /// Eject or unmount after writing, and return the sentence saying the stick can be pulled.
    fn finish(&self, disk: &Disk, root: &Path) -> Result<String, String>;
}

/// The person at the keyboard.
pub trait Console {
    /// One line of output.
    fn say(&mut self, line: &str);
    /// A prompt, and the line typed back, trimmed. `Err` at end of input.
    fn ask(&mut self, prompt: &str) -> Result<String, String>;
}

/// What was asked for on the command line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// `--list`: show, then stop.
    pub list: bool,
    /// `--all`: with `--list`, also the refused disks and why.
    pub all: bool,
    /// `--disk ID`: this disk, without asking which.
    pub disk: Option<String>,
    /// `--yes`: copy without asking. Never erases.
    pub yes: bool,
    /// `--erase`: with `--disk`, erase if the disk is not FAT.
    pub erase: bool,
    /// `--keep-mounted`: do not eject afterwards.
    pub keep_mounted: bool,
    /// `--include-disk-images`.
    pub policy: Policy,
}

/// What the command line resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// Do the work.
    Run(Options),
    /// `--help`.
    Help,
    /// `--version`.
    Version,
}

/// The usage text.
pub const USAGE: &str = "\
usage: stick_maker [--list [--all]] [--disk ID] [--yes] [--erase] [--keep-mounted]
                   [--include-disk-images]

Writes nife's boot files to a USB stick or SD card. With no arguments it lists the removable
disks, asks which to use, and asks again before writing anything.

  --list                 show the disks that would be offered, then stop
  --all                  with --list, also show every disk that is not offered, and why
  --disk ID              use this disk (as --list names it) instead of asking
  --yes                  with --disk, copy without asking (never erases)
  --erase                with --disk, erase the disk first if it is not FAT32
  --keep-mounted         do not eject the disk afterwards
  --include-disk-images  also offer file-backed disks, which is how this program is tested
";

/// Read the arguments (without the program name).
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Invocation, String> {
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--list" => options.list = true,
            "--all" => options.all = true,
            "--disk" => options.disk = Some(args.next().ok_or("--disk needs a disk name")?),
            "--yes" => options.yes = true,
            "--erase" => options.erase = true,
            "--keep-mounted" => options.keep_mounted = true,
            "--include-disk-images" => options.policy.include_disk_images = true,
            "--help" | "-h" => return Ok(Invocation::Help),
            "--version" => return Ok(Invocation::Version),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if (options.yes || options.erase) && options.disk.is_none() {
        return Err("--yes and --erase only mean something with --disk".to_owned());
    }
    Ok(Invocation::Run(options))
}

/// One line describing a disk: identifier, name, size, bus, volumes.
pub fn describe(disk: &Disk) -> String {
    let volumes: Vec<String> = disk
        .volumes
        .iter()
        .map(|v| {
            let fs = match &v.filesystem {
                disk::Filesystem::Fat => "FAT".to_owned(),
                disk::Filesystem::Other(name) => name.clone(),
                disk::Filesystem::Unknown => "unrecognised".to_owned(),
            };
            if v.label.is_empty() {
                fs
            } else {
                format!("{fs} \"{}\"", v.label)
            }
        })
        .collect();
    let volumes = if volumes.is_empty() {
        "no volumes".to_owned()
    } else {
        volumes.join(", ")
    };
    let name = if disk.description.is_empty() {
        "(no name)"
    } else {
        &disk.description
    };
    format!(
        "{:<8} {:<28} {:>9}  {:<14} {volumes}",
        disk.id,
        name,
        human_size(disk.size),
        disk.bus
    )
}

/// **The whole program**, given its boot files, a host and a person.
pub fn run(
    options: &Options,
    files: &[File],
    build: &str,
    host: &dyn Host,
    console: &mut dyn Console,
) -> Result<(), String> {
    console.say(&format!("nife stick maker, build {build}"));
    if files.is_empty() {
        return Err(
            "this copy of the program carries no boot files. It was built without them; \
             `cargo xtask stick` builds one that has them."
                .to_owned(),
        );
    }
    let needed = payload::total(files);
    for file in files {
        console.say(&format!(
            "  {:<8} {:<24} {}",
            architecture(file.path).unwrap_or("?"),
            file.path,
            human_size(file.bytes.len() as u64)
        ));
    }
    console.say("");

    let disks = host.discover()?;
    let offered: Vec<&Disk> = disks
        .iter()
        .filter(|d| disk::refusal(d, options.policy).is_none())
        .collect();

    if options.list {
        list(options, &disks, &offered, console);
        return Ok(());
    }

    let chosen = choose(options, &disks, &offered, console)?;
    let replaced = |volume: &disk::Volume| {
        volume
            .mount
            .as_deref()
            .map_or(0, |root| write::replaced_bytes(root, files))
    };
    let root = match disk::plan(chosen, needed, replaced) {
        Plan::Copy { volume } => confirm_copy(options, chosen, volume, needed, console)?,
        Plan::Erase { because } => {
            confirm_erase(options, chosen, &because, host, console)?;
            host.erase(chosen)?
        }
    };

    let note = payload::note(files, build);
    let written = write::write_set(&root, files, &note)
        .map_err(|e| format!("writing to {} failed: {e}", root.display()))?;
    for w in &written {
        console.say(&format!(
            "  wrote {} ({}{})",
            w.path.display(),
            human_size(w.bytes),
            if w.replaced {
                ", replacing the old one"
            } else {
                ""
            }
        ));
    }

    if options.keep_mounted {
        console.say(&format!(
            "{} is still mounted at {}.",
            chosen.id,
            root.display()
        ));
    } else {
        let done = host.finish(chosen, &root)?;
        console.say(&done);
    }
    console.say("");
    say_how_to_boot(files, console);
    Ok(())
}

/// `--list`: the offered disks, and with `--all` every refused one and why.
fn list(options: &Options, disks: &[Disk], offered: &[&Disk], console: &mut dyn Console) {
    if offered.is_empty() {
        console.say("No removable disk found.");
    }
    for d in offered {
        console.say(&format!("  {}", describe(d)));
    }
    if options.all {
        for d in disks {
            if let Some(refusal) = disk::refusal(d, options.policy) {
                console.say(&format!(
                    "  {}\n           not offered: {}",
                    describe(d),
                    refusal.reason()
                ));
            }
        }
    }
}

/// The copy plan: say what will be written where, ask unless `--yes`, and return the mount.
fn confirm_copy(
    options: &Options,
    chosen: &Disk,
    volume: usize,
    needed: u64,
    console: &mut dyn Console,
) -> Result<PathBuf, String> {
    let mount = chosen.volumes[volume]
        .mount
        .clone()
        .ok_or("the plan copies only to a mounted volume")?;
    console.say(&format!(
        "Will copy {} of boot files to {} on {} ({}). Nothing on it is erased.",
        human_size(needed),
        mount.display(),
        chosen.id,
        chosen.description
    ));
    if !options.yes {
        let answer = console.ask("Type y to copy: ")?;
        if !answer.eq_ignore_ascii_case("y") && !answer.eq_ignore_ascii_case("yes") {
            return Err("not confirmed; nothing was written".to_owned());
        }
    }
    Ok(mount)
}

/// The erase plan: name everything that will be lost, ask the host whether it can erase at all,
/// then require `--erase` with `--disk`, or the disk's name typed back.
fn confirm_erase(
    options: &Options,
    chosen: &Disk,
    because: &str,
    host: &dyn Host,
    console: &mut dyn Console,
) -> Result<(), String> {
    console.say(&format!(
        "{} ({}, {}) has to be ERASED first: {because}.",
        chosen.id,
        chosen.description,
        human_size(chosen.size)
    ));
    for v in &chosen.volumes {
        let label = if v.label.is_empty() {
            String::new()
        } else {
            format!(" (\"{}\")", v.label)
        };
        console.say(&format!("  everything on {}{label} will be lost", v.id));
    }
    host.can_erase(chosen)?;
    if options.disk.is_some() {
        if !options.erase {
            return Err("pass --erase to allow it; nothing was written".to_owned());
        }
    } else {
        let answer = console.ask(&format!(
            "Type the disk's name, {}, to erase it (anything else stops): ",
            chosen.id
        ))?;
        if answer != chosen.id {
            return Err("not confirmed; nothing was written".to_owned());
        }
    }
    Ok(())
}

/// The closing instructions, one line per architecture the stick carries a boot file for.
fn say_how_to_boot(files: &[File], console: &mut dyn Console) {
    console.say("Done. To boot from it:");
    for file in files {
        let how = match architecture(file.path) {
            Some("x86_64") => {
                "  a PC: turn Secure Boot off in its firmware settings, then choose the stick from \
                 its boot menu (F12 on Dell, F11 or Esc on many others)"
            }
            Some("aarch64") => {
                "  an aarch64 board or VM with UEFI firmware (U-Boot's bootefi, or EDK2): boot the \
                 stick's removable-media entry"
            }
            Some("riscv64") => {
                "  a riscv64 board or VM with UEFI firmware (U-Boot's bootefi, or EDK2): boot the \
                 stick's removable-media entry"
            }
            _ => continue,
        };
        console.say(how);
    }
}

/// Which disk: the one named with `--disk` (if the rule offers it), or the one the person picks.
fn choose<'a>(
    options: &Options,
    disks: &'a [Disk],
    offered: &[&'a Disk],
    console: &mut dyn Console,
) -> Result<&'a Disk, String> {
    if let Some(id) = &options.disk {
        let found = disks
            .iter()
            .find(|d| &d.id == id)
            .ok_or_else(|| format!("there is no disk called {id}; `--list --all` shows them"))?;
        if let Some(refusal) = disk::refusal(found, options.policy) {
            return Err(format!("{id} is not offered: it is {}", refusal.reason()));
        }
        return Ok(found);
    }
    if offered.is_empty() {
        return Err(
            "no USB stick or SD card found. Insert one and run this again \
                    (`--list --all` shows every disk and why it was not offered)."
                .to_owned(),
        );
    }
    console.say("Removable disks:");
    for (i, d) in offered.iter().enumerate() {
        console.say(&format!("  {}) {}", i + 1, describe(d)));
    }
    let answer = console.ask(&format!(
        "Which one? (1-{}, or Enter to stop) ",
        offered.len()
    ))?;
    answer
        .parse::<usize>()
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|n| offered.get(n).copied())
        .ok_or_else(|| "nothing chosen; nothing was written".to_owned())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;

    use super::*;
    use crate::disk::{Filesystem, Volume};

    /// A machine with a FAT stick (mounted at a real temporary directory), an exFAT card, and a USB
    /// backup disk, recording every act the program asks of it.
    struct FakeHost {
        disks: Vec<Disk>,
        erasable: bool,
        erased: RefCell<Vec<String>>,
        finished: RefCell<Vec<String>>,
        erase_root: PathBuf,
    }

    impl Host for FakeHost {
        fn discover(&self) -> Result<Vec<Disk>, String> {
            Ok(self.disks.clone())
        }
        fn can_erase(&self, disk: &Disk) -> Result<(), String> {
            if self.erasable {
                Ok(())
            } else {
                Err(format!(
                    "cannot erase {} here; the steps by hand are ...",
                    disk.id
                ))
            }
        }
        fn erase(&self, disk: &Disk) -> Result<PathBuf, String> {
            self.erased.borrow_mut().push(disk.id.clone());
            fs::create_dir_all(&self.erase_root).map_err(|e| e.to_string())?;
            Ok(self.erase_root.clone())
        }
        fn finish(&self, disk: &Disk, _root: &Path) -> Result<String, String> {
            self.finished.borrow_mut().push(disk.id.clone());
            Ok(format!("Ejected {}.", disk.id))
        }
    }

    /// A person who types the scripted answers in order, and a transcript of what they were shown.
    struct Script {
        answers: Vec<&'static str>,
        shown: Vec<String>,
    }

    impl Console for Script {
        fn say(&mut self, line: &str) {
            self.shown.push(line.to_owned());
        }
        fn ask(&mut self, prompt: &str) -> Result<String, String> {
            self.shown.push(prompt.to_owned());
            if self.answers.is_empty() {
                return Err("end of input".to_owned());
            }
            Ok(self.answers.remove(0).to_owned())
        }
    }

    impl Script {
        fn with(answers: &[&'static str]) -> Self {
            Script {
                answers: answers.to_vec(),
                shown: Vec::new(),
            }
        }
        fn saw(&self, text: &str) -> bool {
            self.shown.iter().any(|l| l.contains(text))
        }
    }

    const FILES: [File; 2] = [
        File {
            path: "EFI/BOOT/BOOTX64.EFI",
            bytes: b"x86 boot file",
        },
        File {
            path: "EFI/BOOT/BOOTRISCV64.EFI",
            bytes: b"riscv64 boot file",
        },
    ];

    fn machine(name: &str) -> (FakeHost, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("stick_maker-cli-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let stick_root = dir.join("stick");
        fs::create_dir_all(&stick_root).unwrap();
        let volume = |id: &str, fs: Filesystem, mount: Option<PathBuf>| Volume {
            id: id.into(),
            label: String::new(),
            filesystem: fs,
            mount,
            free: None,
            size: 1 << 30,
        };
        let disk = |id: &str, removable: bool, volumes: Vec<Volume>| Disk {
            id: id.into(),
            description: format!("{id} model"),
            size: 32_000_000_000,
            bus: "USB".into(),
            internal: false,
            removable_media: removable,
            disk_image: false,
            volumes,
        };
        let host = FakeHost {
            disks: vec![
                disk(
                    "stick",
                    true,
                    vec![volume("stick1", Filesystem::Fat, Some(stick_root))],
                ),
                disk(
                    "card",
                    true,
                    vec![volume("card1", Filesystem::Other("ExFAT".into()), None)],
                ),
                disk(
                    "backup",
                    false,
                    vec![volume("backup1", Filesystem::Fat, Some(dir.join("b")))],
                ),
            ],
            erasable: true,
            erased: RefCell::default(),
            finished: RefCell::default(),
            erase_root: dir.join("erased"),
        };
        (host, dir)
    }

    fn options(disk: Option<&str>) -> Options {
        Options {
            disk: disk.map(str::to_owned),
            ..Options::default()
        }
    }

    #[test]
    fn the_common_case_copies_without_erasing_and_ejects() {
        let (host, dir) = machine("copy");
        let mut person = Script::with(&[]);
        let opts = Options {
            yes: true,
            ..options(Some("stick"))
        };
        run(&opts, &FILES, "test", &host, &mut person).unwrap();
        assert_eq!(
            fs::read(dir.join("stick/EFI/BOOT/BOOTX64.EFI")).unwrap(),
            b"x86 boot file"
        );
        assert!(
            fs::read_to_string(dir.join("stick/NIFE.TXT"))
                .unwrap()
                .contains("build test")
        );
        assert!(
            host.erased.borrow().is_empty(),
            "a FAT stick is never erased"
        );
        assert_eq!(*host.finished.borrow(), ["stick"]);
        assert!(person.saw("Nothing on it is erased"));
        assert!(person.saw("Secure Boot off"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn nothing_is_erased_without_the_flag_or_the_typed_name() {
        let (host, dir) = machine("refuse-erase");
        // Named on the command line, but no --erase.
        let err = run(
            &options(Some("card")),
            &FILES,
            "t",
            &host,
            &mut Script::with(&[]),
        )
        .unwrap_err();
        assert!(err.contains("--erase"), "{err}");
        // Picked interactively, and the confirmation is anything but the disk's own name.
        for wrong in ["y", "yes", "CARD", " card2", ""] {
            let mut person = Script::with(&["2", wrong]);
            let err = run(&options(None), &FILES, "t", &host, &mut person).unwrap_err();
            assert!(err.contains("not confirmed"), "{wrong:?}: {err}");
        }
        assert!(host.erased.borrow().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_typed_name_erases_and_then_writes() {
        let (host, dir) = machine("erase");
        let mut person = Script::with(&["2", "card"]);
        run(&options(None), &FILES, "t", &host, &mut person).unwrap();
        assert_eq!(*host.erased.borrow(), ["card"]);
        assert!(person.saw("has to be ERASED first: it is formatted ExFAT, not FAT"));
        assert_eq!(
            fs::read(dir.join("erased/EFI/BOOT/BOOTRISCV64.EFI")).unwrap(),
            b"riscv64 boot file"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_refused_disk_stays_refused_when_named() {
        let (host, dir) = machine("named-refusal");
        let opts = Options {
            yes: true,
            erase: true,
            ..options(Some("backup"))
        };
        let err = run(&opts, &FILES, "t", &host, &mut Script::with(&[])).unwrap_err();
        assert!(err.contains("not removable media"), "{err}");
        assert!(!dir.join("b").exists(), "nothing was written to it");
        let err = run(
            &options(Some("nosuch")),
            &FILES,
            "t",
            &host,
            &mut Script::with(&[]),
        )
        .unwrap_err();
        assert!(err.contains("no disk called nosuch"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_host_that_cannot_erase_says_so_before_asking_for_confirmation() {
        let (mut host, dir) = machine("cannot-erase");
        host.erasable = false;
        let mut person = Script::with(&["2", "card"]);
        let err = run(&options(None), &FILES, "t", &host, &mut person).unwrap_err();
        assert!(err.contains("by hand"), "{err}");
        assert!(
            !person.saw("Type the disk's name"),
            "never asked to confirm an erase it cannot do"
        );
        assert_eq!(
            person.answers,
            ["card"],
            "the confirmation was never consumed"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn declining_or_stopping_writes_nothing() {
        let (host, dir) = machine("decline");
        for answers in [&["1", "n"][..], &[""][..], &["9"][..], &[][..]] {
            let mut person = Script::with(answers);
            assert!(
                run(&options(None), &FILES, "t", &host, &mut person).is_err(),
                "{answers:?}"
            );
        }
        assert!(!dir.join("stick/EFI").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn listing_names_every_refusal_and_writes_nothing() {
        let (host, dir) = machine("list");
        let mut person = Script::with(&[]);
        let opts = Options {
            list: true,
            all: true,
            ..Options::default()
        };
        run(&opts, &FILES, "t", &host, &mut person).unwrap();
        assert!(person.saw("stick    stick model"));
        assert!(person.saw("not offered: not removable media"));
        assert!(host.erased.borrow().is_empty() && host.finished.borrow().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn keep_mounted_skips_the_eject_and_says_where() {
        let (host, dir) = machine("keep");
        let mut person = Script::with(&["1", "y"]);
        let opts = Options {
            keep_mounted: true,
            ..Options::default()
        };
        run(&opts, &FILES, "t", &host, &mut person).unwrap();
        assert!(host.finished.borrow().is_empty());
        assert!(person.saw("is still mounted at"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_build_without_boot_files_refuses_before_looking_at_disks() {
        let (host, dir) = machine("empty");
        let err = run(&options(None), &[], "t", &host, &mut Script::with(&[])).unwrap_err();
        assert!(err.contains("carries no boot files"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_command_line() {
        let args = |a: &[&str]| parse(a.iter().map(|s| (*s).to_owned()));
        assert_eq!(args(&["--help"]), Ok(Invocation::Help));
        assert_eq!(args(&["--version"]), Ok(Invocation::Version));
        assert!(args(&["--yes"]).is_err(), "--yes needs --disk");
        assert!(args(&["--erase"]).is_err(), "--erase needs --disk");
        assert!(args(&["--disk"]).is_err());
        assert!(args(&["--frobnicate"]).is_err());
        let Ok(Invocation::Run(o)) = args(&[
            "--list",
            "--all",
            "--disk",
            "disk7",
            "--yes",
            "--erase",
            "--keep-mounted",
            "--include-disk-images",
        ]) else {
            panic!("every flag at once parses");
        };
        assert!(o.list && o.all && o.yes && o.erase && o.keep_mounted);
        assert!(o.policy.include_disk_images);
        assert_eq!(o.disk.as_deref(), Some("disk7"));
    }

    #[test]
    fn a_disk_with_no_name_and_no_volumes_still_describes_itself() {
        let bare = Disk {
            id: "sdz".into(),
            description: String::new(),
            size: 0,
            bus: "USB".into(),
            internal: false,
            removable_media: true,
            disk_image: false,
            volumes: Vec::new(),
        };
        let line = describe(&bare);
        assert!(
            line.contains("(no name)") && line.contains("no volumes"),
            "{line}"
        );
    }

    /// **Each boot file on the stick gets its own way to boot it.** The closing instructions are
    /// the last thing a person reads before walking to the board; milestone 326 (turn a mutation score upward)
    /// found that the aarch64 and riscv64 lines could each be deleted with every test green,
    /// because the only assertion was on the x86 one.
    #[test]
    fn each_architecture_on_the_stick_gets_its_boot_instructions() {
        let (host, dir) = machine("per-arch");
        let files = [
            File {
                path: "EFI/BOOT/BOOTAA64.EFI",
                bytes: b"arm",
            },
            File {
                path: "EFI/BOOT/BOOTRISCV64.EFI",
                bytes: b"riscv64",
            },
        ];
        let mut person = Script::with(&[]);
        let opts = Options {
            yes: true,
            ..options(Some("stick"))
        };
        run(&opts, &files, "test", &host, &mut person).unwrap();
        assert!(person.saw("an aarch64 board"));
        assert!(person.saw("a riscv64 board"));
        assert!(!person.saw("Secure Boot"), "no x86 file, no x86 advice");
        fs::remove_dir_all(dir).unwrap();
    }
}
