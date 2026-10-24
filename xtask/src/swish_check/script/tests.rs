//! Tests of the script table itself: the lines it types against the disk and the programs.

use super::*;

/// **Every program the shell can spawn is spawned by `script/swish-check`, or says why not**
/// (milestone 150). Before this, a program's presence in the booted system was proven only by a
/// transcript line somebody remembered to type, and three of thirteen had none. The check is a
/// token match (the program's name as a whole word anywhere in a line), which is weaker than
/// "the line ran it" and is enough to make forgetting loud.
/// **And the line every leg installs from names the file the seed writes**, for the same reason
/// (milestone 47's bare-name lane, 2026-09-27).
#[test]
fn the_noteless_install_line_names_the_seeded_file() {
    // Typed from inside `downloads` (milestone 809: `jig` is granted the word's file there).
    let inside = |path: &str| {
        let name = path.strip_prefix("downloads/").expect("a seeded download");
        if name.contains('/') {
            format!("jig install {name}")
        } else {
            format!("jig install ./{name}")
        }
    };
    let line = inside(crate::disk::DOWNLOADED_NOTELESS);
    assert!(SWISH_CHECK_SCRIPT.iter().any(|l| l.typed == line));
    let refused = inside(crate::disk::DOWNLOADED_PACKAGE);
    assert!(SWISH_CHECK_SCRIPT.iter().any(|l| l.typed == refused));
    assert!(SWISH_CHECK_SCRIPT.iter().any(|l| l.typed == "cd downloads"));
}

/// Bare names this script installs before it types them (§229 (how a bare name at the prompt
/// reaches an installed program), B2), which run as programs without being the image's, and
/// the version-qualified ask, which is not a bare name but resolves to one of these rows the
/// same way (milestone 614 (two installed versions of one program, each runnable), ruling 4).
/// `rg` is the program `jig install ripgrep` installs from basalt's index (milestone 810
/// (`ripgrep` is packaged in basalt and installed with `jig`)), `ioping` is `jig install
/// ioping`'s (milestone 906 (ioping is packaged in basalt)), `hackbench` is `jig install
/// hackbench`'s (milestone 907 (hackbench is packaged in basalt)), and `speedtest1` is `jig install
/// sqlite`'s (milestone 897 (SQLite is packaged in basalt)). `alto` and `tenor` are the two
/// programs of `duet` (milestone 919 (a package carries several programs)).
const INSTALLED_BY_THE_SCRIPT: [&str; 10] = [
    "greeting",
    "noteless",
    "greeting@0.1.0",
    "rg",
    "ioping",
    "hackbench",
    "speedtest1",
    "alto",
    "tenor",
    "reach",
];

/// **The several-program packages' install lines name the files the seed writes** (milestone
/// 919), on every leg: the disk carries both everywhere, and no leg fetches them.
#[test]
fn the_several_program_install_lines_name_the_seeded_files() {
    for file in [
        crate::disk::DOWNLOADED_DUET,
        crate::disk::DOWNLOADED_SOLO,
        crate::disk::DOWNLOADED_DIALER,
    ] {
        let file = file.strip_prefix("downloads/").expect("a seeded download");
        let line = format!("jig install ./{file}");
        assert!(SWISH_CHECK_AFTER_REBOOT.iter().any(|l| l.typed == line));
        for arch in ["aarch64", "riscv64", "x86_64"] {
            assert!(swish_check_omits(arch, &line).is_none(), "{arch}");
        }
    }
}

/// **The second version's install line names the file the seed writes** (milestone 614), the
/// same pairing the two tests above hold for 0.1.0's lines.
#[test]
fn the_second_version_install_line_names_the_seeded_file() {
    let line = format!(
        "jig install {}",
        crate::disk::DOWNLOADED_GREETING_0_2_0
            .strip_prefix("downloads/")
            .expect("a seeded download")
    );
    assert!(SWISH_CHECK_SCRIPT.iter().any(|l| l.typed == line));
    // No leg omits it: the disk carries it everywhere, and no leg fetches it.
    for arch in ["aarch64", "riscv64", "x86_64"] {
        assert!(swish_check_omits(arch, &line).is_none(), "{arch}");
    }
}

#[test]
fn no_script_types_a_silent_line_twice_in_a_row() {
    // [`without_redraws`] would read the second of two identical lines as the console's
    // redraw of the first if a gauge fell between them and the first printed nothing. A line
    // that must answer something fails either way when it prints nothing (the second copy
    // then answers "no answer at all"), so only a line wanting no answer is a hazard, and
    // `network_echo_client --mem 4`, typed twice on purpose, wants one.
    for script in [&**SWISH_CHECK_SCRIPT, SWISH_CHECK_AFTER_REBOOT] {
        for pair in script.windows(2) {
            assert!(
                pair[0].typed != pair[1].typed || !pair[0].answer.is_empty(),
                "{:?} is typed twice in a row and wants no answer",
                pair[0].typed
            );
        }
    }
}

/// **A job count names a program** ([`Line`]'s doc): no line may claim more jobs than it has
/// stages whose head is something the progenitor can build, a `grant_plan::Program` or an
/// installed package's path, after the `time` and `xargs` prefixes. A `caps` head is a preview
/// and builds nothing. A bound from above only; a tag that is too low is the case no host test
/// can see, and the transcript cannot either.
#[test]
fn a_job_count_names_a_program() {
    for l in SWISH_CHECK_SCRIPT.iter().chain(SWISH_CHECK_AFTER_REBOOT) {
        let heads = l
            .echoed
            .split(['|', '&', ';'])
            .filter_map(|stage| {
                stage
                    .split_whitespace()
                    .find(|w| *w != "time" && *w != "xargs")
            })
            // A token with a `/` in it runs a file's bytes (DECISIONS §219 D), which is the
            // shell's own test (`components/src/swish.rs`, `run`).
            .filter(|head| {
                head.contains('/')
                    || grant_plan::Program::ALL.iter().any(|p| p.name() == *head)
                    || INSTALLED_BY_THE_SCRIPT.contains(head)
            })
            .count();
        assert!(
            usize::from(l.jobs) <= heads,
            "`{}` claims {} job(s) and has {heads} stage(s) headed by a program",
            l.echoed,
            l.jobs
        );
    }
}

#[test]
fn every_spawnable_program_has_a_swish_check_line() {
    // Programs a transcript cannot drive, each with the reason. Empty: `SWISH_CHECK_INTERRUPTED`
    // presses `^C` for the two supervised demonstrators.
    const UNSCRIPTED: [&str; 0] = [];
    for p in grant_plan::Program::ALL {
        let name = p.name();
        let scripted = SWISH_CHECK_SCRIPT
            .iter()
            .any(|l| l.typed.split_whitespace().any(|w| w == name));
        assert!(
            scripted != UNSCRIPTED.contains(&name),
            "`{name}`: {}",
            if scripted {
                "scripted now, so take it off UNSCRIPTED"
            } else {
                "the shell can spawn it and SWISH_CHECK_SCRIPT never does; add a line (see \
                 notes/adding-a-program.md), or add it to UNSCRIPTED with the reason"
            }
        );
    }
}
