//! **The installer** (DECISIONS §208 and §270): what `jig` holds, the installs it refuses
//! (tampered bytes, an image program's name, a name another package provides), the first
//! generation it writes, and the unvouched bytes the D2 ruling endows.
//!
//! Cut from the table in `script.rs` verbatim by milestone 900 (the swish-check script
//! splits by topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// The installer rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    // **The installer** (milestone 198 (a package manager) rung 3a, DECISIONS §208 (installing a
    // package is granting it, and the installed programs are versioned)), asked by `jig` since milestone
    // 809 (the package client becomes a program): a program holding an installer endpoint, where
    // this was the shell's `package` builtin. A package with one byte of its program flipped, and
    // its table of contents rewritten to agree, is refused by the image's catalog before anything
    // is written: the catalog is the only thing that can tell (`disk::stage_installed`). Nothing
    // is installed afterwards: the line after says generation 1.
    //
    // **What `jig` holds, before it runs** (DECISIONS §270 (a package manager holds an installer
    // endpoint, not the spawn endpoint)): the installer endpoint, which cannot spawn or vouch.
    line(
        0,
        "caps jig install greeting@0.1.0",
        &[
            "cap 2  endpoint  network  WRITE",
            // Milestone 892 (a std program at the prompt holds the network and a resolver): and the
            // resolver, for any name (calef's Q5 on #1884).
            "cap 9  endpoint  resolver  WRITE. it may look up the addresses of any name",
            "cap 15 endpoint  installer  WRITE",
            "cannot vouch for bytes, and cannot hand this on",
            "cap 17 frame     catalog  read-only",
        ],
    ),
    // **Grants by verb** (milestone 809, option V2): `jig list` holds the catalog and nothing
    // else, which the next line shows by what it can do. `caps` of it shows no installer row, and
    // a line can only assert what is printed, so the absence is proved by `jig list` running and
    // the preview tests in `crates/swish/src/preview/tests.rs`.
    line(
        1,
        "jig list",
        &[
            "packages this image vouches for (its catalog; no index yet):",
            "  greeting 0.1.0",
        ],
    ),
    // **A program without the installer grant is refused** (DECISIONS §270 (a package manager
    // holds an installer endpoint, not the spawn endpoint)): the same bytes, run by path, are
    // installed bytes nobody vouched for, endowed from their note, which asks for no installer and
    // could not ask for one. So the slot is empty, and nothing reaches the progenitor.
    line(
        1,
        "installed/jig rollback",
        &["this jig holds no installer endpoint; only the owner's console grants one"],
    ),
    // **From inside `downloads`, because a word is designated at the current directory and the
    // root cannot be narrowed** (`swish::designation`; `rm` meets the same refusal there). `./`
    // makes each word a file rather than a package name, and designates that one file read-only.
    line(0, "cd downloads", &[]),
    line(
        1,
        "jig install ./tampered.nifepkg",
        &[
            "refused: neither this image's catalog nor its index copy vouches for those bytes; nothing is installed",
        ],
    ),
    // **A package cannot take a name the image carries** (DECISIONS §229, calef's ruling of
    // 2026-09-27). `uptime`'s package is genuine and cataloged, and the image carries `uptime`, so
    // it is refused and nothing is written. Under §235 (the OS is built and updated from packages)
    // a base program is updated through the boot slot, never by install, so this blocks no update.
    line(
        1,
        "jig install ./uptime.nifepkg",
        &[
            "refused: the image carries the program uptime; a new base updates it, not \
             install; nothing is installed",
        ],
    ),
    // A genuine package the image lacks, whose digest the image's catalog carries: the
    // progenitor writes the program under `packages/<stem>/`, writes generation 1, and renames
    // `current` onto it.
    line(
        1,
        "jig install ./noteless.nifepkg",
        &["installed; generation 1 is live"],
    ),
    // **The floor: a rollback never targets the initial generation** (ruled 2026-10-28): the store
    // starts at the empty generation 0, so rolling back to it would wipe every installed program
    // in one verb. At generation 1 there is no earlier generation to return to, and nothing moves.
    line(
        1,
        "jig rollback",
        &["refused: there is no generation before the live one"],
    ),
    line(0, "cd /", &[]),
    // **And what it installed runs, by its bytes** (DECISIONS §219 (how the shell names an
    // installed program to the spawner) option D). A path, so the shell reads the file into frames
    // and the progenitor hashes its own copy and finds the digest in the generation just written.
    // Its line is the proof it ran: a refusal prints no such thing.
    line(
        1,
        "packages/noteless/0.1.0/noteless",
        &["noteless: installed, and carrying no manifest note"],
    ),
    // **`caps` names who vouched** (§219: "or the source that vouched"): the digest the shell
    // hashed is in the generation the install just wrote.
    line(
        0,
        "caps packages/noteless/0.1.0/noteless",
        &[
            "provenance: vouched by activation generation 1 (digest sha256:",
            // **No note, the default** (milestone 597 (a program carries its manifest in an ELF
            // note), provisional): `noteless` carries no manifest note, so it is bound and endowed
            // as `grant_plan::NO_NOTE_MANIFEST`, its output alone.
            "it carries no manifest note, so it asks for its output and nothing else",
        ],
    ),
    // **And bytes nobody installed, previewed** (§219 gate D2): no generation lists them, and the
    // boot prompt holds the run-unvouched capability (its provisional grant,
    // `crates/system_initializer`), so the preview is the ruling's endowment and says why it runs.
    line(
        0,
        "caps installed/unvouched",
        &[
            "cap 1  page      clock",
            "cap 2  page      config",
            "provenance: unvouched (digest sha256:",
            "runs on this session's capability to run unvouched bytes (slot 62)",
            // **What the note asks, beside what is granted** (milestone 597, provisional). The
            // witness's note asks for the three authorities it probes, and §219 says an unvouched
            // note grants nothing: the rows above are the ruling's three and no more.
            "its manifest note asks for: output bytes, entropy, the network, the process domain",
            "and its note grants nothing",
        ],
    ),
    // **And run: milestone 202 (every confinement test is a ritual until somebody breaks the confinement)'s claim that an unvouched child holds no capability the caller did
    // not delegate** (DECISIONS §219 gate D2). The bytes are `unreachable_network_witness`
    // stripped, so no table vouches for them and they run on the session's capability alone. The
    // line delegated the output and nothing else; the ruling adds the clock and configuration
    // pages. So each of the three authorities the progenitor holds must answer "refused", and the
    // census must read exactly slots 0, 1 and 2. Falsified once per authority before it was
    // committed: granting the domain, entropy or the network to an unvouched child turned this
    // line red (design/roadmap/202's block records the three runs).
    line(
        1,
        crate::disk::INSTALLED_UNVOUCHED,
        &[
            "network: refused (no capability at slot 10)",
            "entropy: refused (no capability at slot 9)",
            "domain: refused (no capability at slot 7)",
            "reboot: refused (no capability at slot 13)",
            "slots held: 0 1 2\n",
        ],
    ),
];
