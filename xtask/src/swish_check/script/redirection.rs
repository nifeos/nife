//! **Redirection on both streams** (DECISIONS §67 (a program's second stream is a declaration, not a number)), with `uuid` and `time` as the programs
//! that exercise it: `>`, `<` and `2>`, the empty second streams, and the previews that
//! name each destination.
//!
//! Cut from the table in `script.rs` verbatim by milestone 900 (the swish-check script
//! splits by topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// The redirection rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    // **`uuid`, at the real prompt** (milestone 111), the only gate that can run it: the progenitor
    // places a `WRITE` view of the entropy service at `grant_plan::ENTROPY_SLOT`, and elsewhere the
    // kernel plays the progenitor and fills a table from slot 0 upward. The value is not asserted,
    // since a predictable version-4 identifier is drawn from nothing; the shape is. 36 characters
    // and a newline make `1 1 37`, and a `uuid` endowed with nothing leaves the file empty (its
    // refusal goes to the second stream), so the count fails exactly then.
    line(1, "uuid > id.txt", &[]),
    line(1, "wc < id.txt", &["1 1 37"]),
    // And the second stream is empty, which is the same trick the `pgrep 2>` line above uses: this
    // program complains in exactly one case (it holds no entropy capability), so an empty second
    // stream is the assertion that the case did not happen. The two lines together are "it drew
    // real bytes and had nothing to complain about", said without asserting any byte of them.
    line(1, "uuid 2> ent.txt", &[]),
    line(1, "wc < ent.txt", &["0 0 0"]),
    // **A `std` program's words on a redirected line** (milestone 205 (how a foreign program is
    // told what to do)): `>` sends the line down the pipeline path, `spawn_stage`, rather than
    // `spawn`'s, so the argv takes the other of the shell's two routes. The count is
    // `std_tests::EXPECTED`'s transcript with its `args []` line read as `args ["std_exerciser",
    // "redirected"]`: 15 lines, 42 words, 272 bytes.
    //
    // **Not `std_exerciser | wc`**, which was this line until CI said otherwise on 2026-09-27. The
    // job pool returns a region only when it is the most recent carve, and a pipeline's head is
    // carved first and reaped first, so a `std` head strands its whole region and no `std` job
    // runs again until reboot. See 205's BUGS.
    line(1, "std_exerciser redirected > args.txt", &[]),
    line(1, "wc < args.txt", &["15 42 272"]),
    // And the visibility surface, which is what a person meets before anything is spawned. On
    // Linux there is nothing here to say: no tool reports whether a program will read
    // `/dev/urandom`, and nothing about running one reveals it either. Here it is a row, and the
    // right on it (`WRITE`, so it may ask the service and may not receive another client's
    // request) is the claim rather than decoration.
    line(
        0,
        "caps uuid",
        &[
            "cap 9  endpoint  entropy  WRITE",
            "draws no randomness at all",
        ],
    ),
    // **`2>`, at the one interface a human touches** (DECISIONS §67). The four lines below are the
    // whole of the decision, and only this gate runs them through the real progenitor: the guest tests
    // wire the shell from the kernel, whose `Spawn` fills a capability table from zero and cannot place a
    // capability at the slot a manifest names, so `date` there never receives a second stream.
    //
    // `date` is the declarer, and at *this* prompt it has a clock and nothing to complain about. So
    // the assertion is that its second stream exists, is separate, and is **empty**: `2> err.txt`
    // creates the file, `date` closes the stream with nothing on it, and `wc` counts zero of
    // everything. A shell that had merged the two streams would put a timestamp in there.
    line(1, "date 2> err.txt", &["UTC"]),
    line(1, "wc < err.txt", &["0 0 0"]),
    // And the visibility surface names the second destination, which is what stops `caps date >
    // when.txt` being a half-truth: two destinations on one line, and a reader can see that the
    // complaint is not going into the file.
    line(0, "caps date 2> err.txt", &["diags    err.txt"]),
    // The refusal, which is the other half of "a declaration, not a number". `wc` writes one stream
    // and its diagnostics ride it, so `2>` names nothing and the line does not run.
    line(0, "wc gate.txt 2> err.txt", &["declares no second output"]),
    // **`time`, at the one interface a human touches** (milestone 86). Only this gate runs the real
    // inits, and the clock the shell times with is granted by them: the guest tests wire it from the
    // kernel, so a boot where the progenitor never handed the shell a clock would pass every one of those and
    // print "this shell holds no clock capability" here.
    //
    // The answer is the same three numbers `wc gate.txt` gave four lines up, which is the claim the
    // milestone rests on: the tail runs exactly as typed and timing it changes nothing about it. A
    // `time` that re-tokenized its tail, or spawned a differently endowed child, would answer
    // something else here and the duration would still look fine.
    line(1, "time wc gate.txt", &["2 4 24"]),
    // And the duration itself, checked for its shape rather than its value: the number is a real
    // measurement and cannot be a constant, but `real` and a unit are what a stopwatch prints.
    line(1, "time date", &["time: real"]),
    // `caps` claims to print a process's whole authority, so the shell's own clock must show; READ
    // without GRANT is why nothing typed here can hand a clock to a child. The second phrase is
    // milestone 31 phase 3's: `holdings().dir` is true exactly when the progenitor granted the
    // shell a directory. Elsewhere the kernel plays the progenitor, so a boot that stopped granting
    // it would fail nothing else; `wc gate.txt` keeps working because the shell opens that file.
    line(
        0,
        "caps",
        &[
            "frame     clock      READ only, NOT delegable",
            "endpoint  directory",
        ],
    ),
];
