//! **The words a line is made of**: quoted names (milestone 67 (`swish` the language: quoting,
//! sequencing, and exit status)), `&&`, `||` and `$?`, and the six-job run that walks the
//! progenitor's job pool (milestone 22 (trusted init), the interactive increment).
//!
//! Cut from the table in `script.rs` verbatim by milestone 900 (the swish-check script
//! splits by topic before it crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

use super::*;

/// The quoting-and-sequencing rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    // **Quoting, at the one interface a human touches** (milestone 67). The gap it closes is an
    // authority one: a file called `my notes.txt` could not be *named* before this, so it could not
    // be granted, in a shell whose whole thesis is that naming a resource is granting it.
    //
    // Three lines, and the third is the one that makes the pair a claim. The `>` writes twelve bytes
    // plus a newline into a name only quoting can express; the `<` reads them back; and `wc "my
    // notes.txt"` is the same designation with the operator left out, so if the two disagree, one of
    // them opened something else. That is `gate.txt`'s trio above, asked of a name with a space in
    // it.
    line(0, "echo hello world > \"my notes.txt\"", &[]),
    line(1, "wc < \"my notes.txt\"", &["1 2 12"]),
    line(1, "wc \"my notes.txt\"", &["1 2 12"]),
    // **And the one thing quoting does to authority**: it suppresses expansion, so the same four
    // characters are one name quoted and a set unquoted. `echo` prints what a grant would move, so
    // this line is the narrowing made visible before anything moves. Unquoted, the same pattern is
    // the refusal five lines up.
    line(0, "echo \"*.txt\"", &["*.txt"]),
    // And the preview names it, which is the pairing `caps` exists for: what the line designates is
    // on the screen before anything moves, and a name with a space in it is now something that
    // sentence can be about.
    line(0, "caps wc \"my notes.txt\"", &["input    my notes.txt"]),
    // **A name a customer's file has** (milestone 946 (a granted file name can be longer than
    // sixteen bytes)): `quarterly-report.txt` is twenty bytes, past what a grant's two argument
    // words hold, and is written, read through `<`, and granted to `wc` by naming it, as `my
    // notes.txt` is above.
    line(0, "echo a quarterly report > quarterly-report.txt", &[]),
    line(1, "wc < quarterly-report.txt", &["1 3 19"]),
    line(1, "wc quarterly-report.txt", &["1 3 19"]),
    line(
        0,
        "caps wc quarterly-report.txt",
        &["input    quarterly-report.txt"],
    ),
    // **Sequencing and the status** (milestone 67). `least_authority_demo 3` runs and `least_authority_demo` alone is refused at
    // the prompt for the integer its manifest requires, so these three lines cover both arms of the
    // condition table with real commands rather than with a branch written for a gate.
    line(1, "least_authority_demo 3 && echo yes", &["yes"]),
    line(0, "least_authority_demo || echo no", &["no"]),
    // **The decision this milestone settled, read at a prompt.** `least_authority_demo` alone is refused, and a
    // refusal is not an error: nothing was spawned, nothing was opened, and the status says so with
    // its own number. Unix cannot draw this line, because there `127` and a program's own `exit(1)`
    // are the same kind of integer.
    //
    // The bare `least_authority_demo` is here because the *first* draft of this gate put `echo $?` straight after
    // `least_authority_demo || echo no` and got `0`, which was the shell being right: the last thing that ran was
    // the `echo`. `$?` is the previous **command**, not the previous line, and that is bash's rule
    // and this shell's.
    line(0, "least_authority_demo", &["needs an integer argument"]),
    line(0, "echo $?", &["2"]),
    // **The progenitor's job budget is bounded and comes back** (milestone 22, the interactive
    // increment): a six-job pool, each job in a region `job_undertaker` returns when it ends. Every
    // job so far plus these six go through it several times over, so a boot that collected nothing
    // answers "could not spawn" here. The `rm` above is the first job whose region holds two
    // processes (the program and its `fs_subtree_caretaker`), the first to fail if the retry missed
    // one. Six distinct arguments, because a moving cursor would let a missed line pass as its twin.
    line(1, "least_authority_demo 3", &["3*3 = 9"]),
    line(1, "least_authority_demo 4", &["4*4 = 16"]),
    line(1, "least_authority_demo 5", &["5*5 = 25"]),
    line(1, "least_authority_demo 6", &["6*6 = 36"]),
    line(1, "least_authority_demo 7", &["7*7 = 49"]),
    line(1, "least_authority_demo 8", &["8*8 = 64"]),
    // **The one spawnable program that answers in a register and had no line** until milestone
    // 150's coverage test (`every_spawnable_program_has_a_swish_check_line`) asked. After the six
    // above, so their count is unchanged; the page count it reports depends on page-table overhead,
    // so the assertion is the sentence that says the grant was spent rather than the number.
    line(
        1,
        "memory_grant_depleter --mem 4",
        &["-page budget you granted"],
    ),
];
