//! **Directory grants at the prompt**: `rm` through a caretaker the progenitor builds for
//! one command (milestone 31 (a capability shell: designation is authorization), phase 3), and `xargs` batching a pattern that matches more
//! names than one grant carries (milestone 109 (`xargs`: batching a grant too large to hand
//! over)).
//!
//! Cut from the table in `script.rs` verbatim by milestone 900 (the swish-check script
//! splits by topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// The directory rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    // **Milestone 31 phase 3, at the one interface a human touches** (2026-08-17). Naming a
    // directory in a command IS granting a capability to it, and until this landed the prompt could
    // say so and not do it: the progenitor deleted the file service during the boot, so a directory grant had
    // nothing to build a caretaker out of and `rm` was a refusal.
    //
    // Four lines, and they are one argument in order. **The preview first**, because the whole claim
    // of this milestone is that the authority is legible before it moves: the row names the
    // directory the grant is over and the sentence says what `-r` would have added, so a reader can
    // see the narrower of the two capabilities being chosen.
    line(
        0,
        "caps rm rmtree/rm-solo",
        &[
            "dir      /rmtree  (the directory holding rm-solo)",
            "and nothing under it: no -r, so it cannot even look",
        ],
    ),
    // **The removal, through a caretaker the progenitor built for this one command.** `-v` because `rm`'s
    // default is silence and a gate needs something to read; the name it prints is the name the
    // command line designated, which is the whole of the endowment.
    line(1, "rm -v rmtree/rm-solo", &["rm-solo"]),
    // **And exactly that name went.** Two entries left in a directory that had three, so the grant
    // took what was designated and not what it could reach: `rm-keep` and the whole `rm-doomed`
    // subtree were inside the same capability and are still there, because nothing named them.
    // Eleven bytes of `rm-doomed/` and eight of `rm-keep`, newlines included.
    line(1, "ls rmtree | wc", &["2 2 19"]),
    // **A name longer than `rm`'s two argument words** (milestone 946 (a granted file name can be
    // longer than sixteen bytes)). Its operand cannot ride `rm`'s `START` words, so the shell grants
    // a set of that one name and tells `rm` to remove what it can see; the progenitor puts a
    // nameset caretaker in front of the bound badge, which is bound to the whole directory. So the
    // `ls` after it is the claim: the other two entries are still there.
    line(0, "echo gone > rmtree/a-name-longer-than-two-words", &[]),
    line(
        1,
        "rm -v rmtree/a-name-longer-than-two-words",
        &["a-name-longer-than-two-words"],
    ),
    line(1, "ls rmtree | wc", &["2 2 19"]),
    // **The one shape this still cannot deliver, and the refusal now says what is true.** It used to
    // read "needs the progenitor to build the caretaker", which stopped being true on the line above. A
    // caretaker's whole attenuation is one `OPENDIR` *into* the granted directory, and a name typed
    // at the top prompt designates the root of this shell's namespace, which has no name to descend
    // into; the contract has no verb for "the directory I already hold, with fewer rights". So this
    // is a refusal at the prompt with **nothing spawned**, which is the one outcome this model must
    // never trade away, and it is a design fork rather than a missing line of code. See
    // design/roadmap/0031-capability-shell.md and notes/dir-capability.md's BUGS.
    line(0, "rm gate.txt", &["there is no name here to descend into"]),
    // **`xargs`, at the one interface a human touches** (milestone 109). `globmany` holds eleven
    // names one pattern matches, which is more than the eight a single grant can carry.
    //
    // The negative control first, and it is the state of the world this milestone answers: unbatched,
    // a match over the bound is a **refusal at the prompt with nothing spawned**, which is milestone
    // 47's answer at the bound and the reason `xargs` was raised. It still is the answer, because
    // batching is opt-in: a line that silently ran N times would make `caps rm *.txt`'s single
    // printed grant a lie.
    line(
        0,
        "echo globmany/m-*.txt",
        &["matched more names than one grant can carry"],
    ),
    // And batched, the same pattern is swept. **Asserting the second batch is what pins the resume
    // rule**: `m-08.txt` first means batch one ended at `m-07.txt` and the watermark carried, so
    // this one line rules out an off-by-one at the boundary, a batch that restarted from the top,
    // and a batch that took the first eight the directory happened to yield.
    line(
        0,
        "xargs echo globmany/m-*.txt",
        &["batch 2: m-08.txt m-09.txt m-10.txt"],
    ),
    // **And the authority per batch is exactly that batch**, which is the claim the milestone rests
    // on and the one only `caps` can make before the delegation chain exists. The preview prints
    // what `rm` would be handed, and what it would be handed in the second invocation is the three
    // remaining names: not the eleven the pattern matched, and not the directory they live in.
    line(
        0,
        "xargs caps rm globmany/m-*.txt",
        &["the directory holding m-08.txt m-09.txt m-10.txt"],
    ),
];
