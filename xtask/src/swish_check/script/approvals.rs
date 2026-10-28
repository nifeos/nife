//! **Scripted installs grant only what the approval file allows** (milestone 911 (scripted
//! installs grant only what the owner's approval file allows)): the owner's file replacing the
//! prompt, the subset test refusing with every extra grant named, the delegated endpoint checked
//! in the installer, and the approvals that a rollback leaves alone. All of it runs from
//! `downloads`, on the packages and approval files the seed writes, so no leg omits a line.
//!
//! Name: provisional (milestone 911's lane, 2026-10-28 UTC).

use after_reboot::OWNERS_QUESTION;

use super::*;

/// The approvals rows of [`SWISH_CHECK_AFTER_REBOOT`], in table order, after the reach lines.
pub(super) const LINES: &[Line] = &[
    // **The confirmation lists the service ask, and one yes covers it** (milestone 911 (scripted
    // installs grant only what the owner's approval file allows), with the confirmation's wording;
    // the start itself is milestone 892 (a std program at the prompt holds the network and a name
    // resolver)'s). `servant`'s note asks to run as a service; nothing starts it, which is
    // milestone 910 (a package declares a service)'s.
    line(0, "cd downloads", &[]),
    asked(
        1,
        "jig install ./servant.nifepkg",
        (OWNERS_QUESTION, "y"),
        &[
            "servant asks for:",
            "runs as a service, now and at every boot",
            "grant these and install? [y/N] y\n",
            "installed; generation",
        ],
    ),
    line(
        1,
        "servant",
        &["servant: installed; its note asks to run as a service"],
    ),
    // **The owner's approval is stored, by name and digest** (milestone 911, `jig approve`): the
    // file's grants printed, the one yes, the records beside the generations.
    asked(
        1,
        "jig approve ./lab-runner.toml",
        (APPROVERS_QUESTION, "y"),
        &[
            "lab-runner approves:",
            "servant:",
            "runs as a service, now and at every boot",
            "seeker:",
            "looks up names in nife.test",
            "approve these delegations? [y/N] y\n",
            "approved lab-runner (sha256:",
        ],
    ),
    // **A scripted install succeeds with no prompt, and the package runs with exactly its asked
    // grants** (Done-when 1): the file's check replaces the owner's yes, and the line asks
    // nothing. The answer appearing is the proof that nothing was asked.
    line(
        1,
        "jig install --approved ./lab-runner.toml ./seeker.nifepkg",
        &["installed; generation"],
    ),
    line(
        1,
        "seeker",
        &["seeker 0.1.0: holding the network and nife.test"],
    ),
    // **An update whose manifest adds a zone is refused, naming the zone, and the previous
    // version stays installed and running** (Done-when 2). 0.2.0's note asks `example.net` beside
    // `nife.test`; the entry allows the one zone and no other.
    line(
        1,
        "jig install --approved ./lab-runner.toml ./0.2.0/seeker.nifepkg",
        &[
            "refused: seeker 0.2.0 asks for looks up names in example.net, which lab-runner does \
             not allow",
            "generation",
        ],
    ),
    line(
        1,
        "seeker",
        &["seeker 0.1.0: holding the network and nife.test"],
    ),
    // **A delegated endpoint cannot install a package outside its file** (Done-when 3): the
    // check is the progenitor's, under the badge, and the refusal names the package.
    line(
        1,
        "jig install --within lab-runner ./dialer.nifepkg",
        &[
            "installing within approval lab-runner; the progenitor decides",
            "refused: approval lab-runner does not allow dialer",
        ],
    ),
    // **Nor a listed package asking more than its entry**: 0.2.0 again, through the delegation,
    // whose check the progenitor runs against the stored records.
    line(
        1,
        "jig install --within lab-runner ./0.2.0/seeker.nifepkg",
        &["refused: approval lab-runner does not allow seeker"],
    ),
    // **Nor send `Approve`**: a delegated badge can neither widen its own approval nor mint
    // another (DECISIONS §270 (a package manager holds an installer endpoint, not the spawn
    // endpoint)'s amendment).
    line(
        1,
        "jig approve ./lab-runner.toml --within lab-runner",
        &[
            "refused: the installer endpoint does not take that request; a delegated badge may \
             neither approve nor revoke",
        ],
    ),
    // **The entry's `remove` answer bounds a delegated removal**, and the `update` answer a
    // delegated reinstall of the installed bytes (milestone 911's two policy answers at work).
    line(
        1,
        "jig remove --within lab-runner seeker",
        &["refused: approval lab-runner does not allow seeker"],
    ),
    line(
        1,
        "jig install --within lab-runner ./servant.nifepkg",
        &["refused: approval lab-runner does not allow servant"],
    ),
    // **An edited file changes nothing until `jig approve` runs again, and the re-approval shows
    // only the changed grants** (Done-when 5): the stored records lack `example.net`, which is
    // what B9's refusal was; the edited file allows it, and the diff says so and only so.
    asked(
        1,
        "jig approve ./lab-runner-edited.toml",
        (APPROVERS_QUESTION, "y"),
        &[
            "lab-runner, changed since the stored approval:",
            "seeker:",
            "looks up names in example.net",
            "approved lab-runner (sha256:",
        ],
    ),
    // **A delegation already running then installs within the new records** (Done-when 5): the
    // same approval name, checked against the new file per request, with no restart and no new
    // endpoint.
    line(
        1,
        "jig install --within lab-runner ./0.2.0/seeker.nifepkg",
        &["installed; generation"],
    ),
    line(
        1,
        "seeker",
        &["seeker 0.2.0: holding the network and two zones"],
    ),
    // **A delegated holder rolls back to a generation within its approval** (Done-when 7): the
    // rows the previous generation restores all lie in `lab-runner`, dialer's only when the
    // owner has installed it, which is the next line's business. The interactive install asks;
    // the owner's yes covers the root zone the note asks for.
    asked(
        1,
        "jig install ./dialer.nifepkg",
        (OWNERS_QUESTION, "y"),
        &["installed; generation"],
    ),
    line(
        1,
        "jig rollback --within lab-runner",
        &["rolled back; generation"],
    ),
    line(0, "dialer", &["no such program"]),
    // **And is refused a rollback to one holding a row outside it, which the refusal names**
    // (Done-when 7): dialer again, and the generation below the live one is the one that
    // installed it.
    asked(
        1,
        "jig install ./dialer.nifepkg",
        (OWNERS_QUESTION, "y"),
        &["installed; generation"],
    ),
    line(
        1,
        "jig rollback --within lab-runner",
        &[
            "refused: approval lab-runner does not allow dialer",
            "generation",
        ],
    ),
    // **A rollback leaves approvals unchanged** (Done-when 8): `runner` is approved now, after
    // the generations a later rollback restores; `lab-runner` is revoked before that rollback and
    // stays revoked after it. The approvals live beside the generations, never as rows in them.
    asked(
        1,
        "jig approve ./runner.toml",
        (APPROVERS_QUESTION, "y"),
        &["runner approves:", "servant:", "approved runner (sha256:"],
    ),
    line(
        1,
        "jig revoke lab-runner",
        &["revoked lab-runner; generation"],
    ),
    line(
        1,
        "jig install --within lab-runner ./noteless.nifepkg",
        &[
            "installing within approval lab-runner; the progenitor decides",
            "refused: approval lab-runner is not stored; jig approve stores it",
        ],
    ),
    line(1, "jig rollback", &["rolled back; generation"]),
    line(
        1,
        "jig install --within lab-runner ./noteless.nifepkg",
        &["refused: approval lab-runner is not stored; jig approve stores it"],
    ),
    // **And an approval made after the restored generation stays** (Done-when 8): `runner`'s
    // records outlived the rollback, and a delegated install of the installed bytes passes under
    // them, its `update` answer allowing the reinstall.
    line(
        1,
        "jig install --within runner ./servant.nifepkg",
        &["installed; generation"],
    ),
    line(0, "cd /", &[]),
];

/// What `jig approve` asks before it stores (milestone 911): one yes covers every grant the file
/// contains, the way one confirmation covers an install's.
const APPROVERS_QUESTION: &str = "approve these delegations? [y/N]";
