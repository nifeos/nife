//! **Fetching packages and choosing among versions** (milestones 809 and 614): the
//! catalog's refusals, the fetched installs, two live versions of one program,
//! `.jig-versions`, and the bare names the live generation answers to.
//!
//! Cut from the table in `script.rs` verbatim by milestone 900 (the swish-check script
//! splits by topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// The fetch-and-version rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    // **A note that asks more than its vouch allows is refused** (milestone 597, provisional).
    // `least_authority_demo`'s note declares an argument, which only a command line designates,
    // and unvouched bytes may hold only what the D2 ruling names. The shell binds the line
    // against the note (it cannot know the verdict), so the refusal is the progenitor's, on its
    // own copy, and nothing is built. `caps` says so first.
    line(
        0,
        "caps installed/asks-an-arg 5",
        &["more than unvouched bytes may hold"],
    ),
    line(
        0,
        "installed/asks-an-arg 5",
        &["the progenitor read those bytes' own manifest, and it does not allow this line"],
    ),
    // **And by its own name** (milestone 946 (a granted file name can be longer than sixteen
    // bytes)): `least_authority_demo` is twenty bytes, and the shell opens a file by a path whose
    // last component is that long, reads its note and sends it, so the answer is the progenitor's
    // verdict on the bytes rather than the shell's refusal of the name.
    line(
        0,
        "installed/least_authority_demo 5",
        &["the progenitor read those bytes' own manifest, and it does not allow this line"],
    ),
    // **A note that cannot be read is refused, not ignored** (milestone 597): the witness again,
    // with its note's version word set to 9 (`disk::with_note_version`). Running it as if it
    // carried no note would endow a program with a manifest it did not declare.
    line(
        0,
        crate::disk::INSTALLED_MALFORMED_NOTE,
        &["carries a manifest note that cannot be read"],
    ),
    // **Fetching, refused before the network is touched**: the catalog names no such package,
    // so nothing is asked of the package source. `jig` reads its copy of the catalog and sends
    // nothing, so it names no generation: it holds no view of the installed-programs table.
    line(
        1,
        "jig install nosuch",
        &["refused: this image's catalog names no such package, so nothing was fetched"],
    ),
    // **And twice more, so this boot starts nine `jig install`s** (milestone 892 (a std program
    // at the prompt holds the network and a resolver)). Each holds the network and so a resolver
    // badge, and the resolver has eight windows: without the forget at each job's reap, the ninth
    // `jig` of the boot is refused a badge and the shell says it could not spawn.
    line(
        1,
        "jig install nosuch",
        &["refused: this image's catalog names no such package, so nothing was fetched"],
    ),
    line(
        1,
        "jig install nosuch",
        &["refused: this image's catalog names no such package, so nothing was fetched"],
    ),
    // **A lying package source.** The gate serves this leg a copy of `uptime` whose program has one
    // byte flipped and whose table of contents agrees (`disk::stage_installed`), under the genuine
    // name: a whole, well-formed HTTP exchange of a well-formed package. Only the image's
    // catalog can refuse it. `jig` checks the bytes against its copy before it sends them
    // (milestone 809's item 8), so the installer is not asked.
    line(
        1,
        "jig install uptime",
        &[
            "refused: this image's catalog does not vouch for those bytes; nothing was sent to \
             the installer",
        ],
    ),
    // **Fetched over the booted system's network and installed** (rung 3a's first gap): the
    // progenitor finds `greeting`'s stem in the catalog, fetches it from the gate's package
    // source through the stack it built at boot, and installs what arrived as it installs a file.
    // **Two versions cataloged, so a bare name is refused** (milestone 614 (two installed
    // versions of one program, each runnable, and a caller granted the one it needs)). The image
    // vouches for `greeting` at 0.1.0 and at 0.2.0 (every archive build builds every recipe), and
    // nothing orders versions, so a bare fetch names no one package. Before this refusal the first
    // catalog line won, which recipe filenames ordered as 0.2.0; the source serves only 0.1.0,
    // and every leg that fetched answered "the package source did not send a whole package". The
    // catalog refuses before the network is asked.
    line(
        1,
        "jig install greeting",
        &[
            "refused: this image's catalog vouches for several versions of that package; \
             name one with <package>@<version>",
        ],
    ),
    // **Fetched by `jig`, not by the progenitor** (milestone 809): `jig` holds the network and
    // reads the package source itself, and the progenitor, which no longer links an HTTP reader,
    // installs what `jig` sends exactly as it installs a file.
    line(
        1,
        "jig install greeting@0.1.0",
        &["fetched and installed; generation 2 is live"],
    ),
    // **And it runs** (rung 3a's second gap): bytes no boot image carries, vouched only by the
    // generation just written. The line it prints is its own; no program in the image prints it.
    // That the image lacks `greeting` is checked on the host before the boot, by reading the
    // archive (`disk::stage_installed`), because the prompt cannot tell: `greeting` is no
    // `grant_plan::Program`, so its bare name is refused whether or not the archive packs it.
    line(
        1,
        "packages/greeting/0.1.0/greeting",
        &[
            "hello from a package this image never carried",
            "clock: held at slot 1, as its manifest note asked",
        ],
    ),
    // **And the manifest it runs with is the one it carries** (milestone 597, provisional): a
    // vouched note is honored, which the `clock: held` line above proves at the child, and `caps`
    // shows here at the prompt.
    line(
        0,
        "caps packages/greeting/0.1.0/greeting",
        &[
            "clock    read-only. it can read the time",
            "its manifest note asks for: output bytes, the clock",
        ],
    ),
    // **And by its bare name** (DECISIONS §229 (how a bare name at the prompt reaches an installed
    // program), B2): the live generation's entry of that name, run down the same road as its path,
    // so `caps` names the same generation.
    line(
        1,
        "greeting",
        &["hello from a package this image never carried"],
    ),
    line(
        0,
        "caps greeting",
        &["provenance: vouched by activation generation 2 (digest sha256:"],
    ),
    // **Milestone 614: the second version installs beside the first** (rulings 2 and 3). Rows key
    // on the digest, so installing over a live version appends and moves the default pointer
    // instead of replacing. The package is `greeting` at 0.2.0, whose member is `greeting_v2`'s
    // bytes under the name `greeting` (`as` in the recipe), so its digest differs from 0.1.0's and
    // the table holds both. The gate installs from the disk on all three legs; the image's
    // catalog carries the stem because every archive build builds every recipe for its
    // architecture.
    line(0, "cd downloads", &[]),
    line(
        1,
        "jig install 0.2.0/greeting.nifepkg",
        &["installed; generation 3 is live"],
    ),
    line(0, "cd /", &[]),
    // **And the new version runs by its path**, printing its own line, which is how the transcript
    // tells the two copies apart.
    line(
        1,
        "packages/greeting/0.2.0/greeting",
        &["hello from the second copy of the package"],
    ),
    // **And the old version still runs by its path, at two versions live.** This is the line the
    // one-entry table made impossible: its digest left that table at the install above, and a
    // digest not in the live generation is `SPAWN_UNVOUCHED`.
    line(
        1,
        "packages/greeting/0.1.0/greeting",
        &["hello from a package this image never carried"],
    ),
    // **The version set, the ruled selection** (ruling 4). The nearest `.jig-versions` file at or
    // above the working directory (here the root's) says `greeting 0.1.0`, and the bare word runs
    // that version, although the default pointer names 0.2.0. The set only selects among live
    // versions; a cloned repository can ask, and cannot run uninstalled bytes.
    line(0, "echo greeting 0.1.0 > .jig-versions", &[]),
    line(
        1,
        "greeting",
        &["hello from a package this image never carried"],
    ),
    // **The explicit ask** (ruling 4's other override): `program@version` answers its own row, and
    // is not reached by the image's claim on the bare name because it is not the bare name.
    line(
        1,
        "greeting@0.1.0",
        &["hello from a package this image never carried"],
    ),
    // **And the divergence notice**, the guard on the whole mechanism: the set now names a version
    // nobody installed, so the default runs and the spawn line says both, naming the file to fix
    // (wording ratified 2026-10-07, §258 (names for two installed versions of one program)).
    line(0, "echo greeting 0.9.9 > .jig-versions", &[]),
    line(
        1,
        "greeting",
        &[
            "greeting 0.2.0 (.jig-versions asks for 0.9.9)",
            "hello from the second copy of the package",
        ],
    ),
    // **A name the image and a package both have** is refused at the prompt, naming both (§229
    // B2), but no line here can make one: install now refuses an image program's name (§229,
    // 2026-09-27), and only a later base adding a name a package holds can produce the pair.
    // `swish::bare`'s host tests prove the refusal; its BUGS say what is not gated.
    // The bare name of what generation 1 installed, which is not the image's either.
    line(
        1,
        "noteless",
        &["noteless: installed, and carrying no manifest note"],
    ),
];
