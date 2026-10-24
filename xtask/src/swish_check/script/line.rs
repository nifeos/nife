//! **The row type and its constructors**: [`Line`], what every script here is a slice
//! of, `line` and `keys`, which build one, and `JOB_DID_NOT_RUN`, the sentences a refused
//! job prints.
//!
//! Cut from `script.rs` by milestone 900 (the swish-check script splits by topic before it
//! crosses the ceiling).
//!
//! Name: provisional (milestone 900's lane, 2026-10-15 UTC).

/// **One line this gate types, what it must answer, and how many jobs it runs.**
///
/// `jobs` counts the children the progenitor builds from its job pool, one per program stage, so
/// `wc gate.txt | wc` is two and `builtin | program` one. It is zero for a builtin, a `caps`
/// preview, a refusal at the prompt, an image the installed-programs table refuses, and a
/// supervised job (`interrupt_heeder`, `interrupt_ignorer`). It has no default, and the summary's
/// total is summed from it. A line with jobs whose answer says "could not spawn" or "faulted"
/// fails even with no wanted phrases; `a_job_count_names_a_program` bounds a tag from above. See
/// notes/swish-check-harness.md, "Why the script's rows assert what they assert".
#[derive(Clone)]
pub(in crate::swish_check) struct Line {
    pub(crate) typed: &'static str,
    /// What the prompt shows once `typed` has been edited: the same text for a plain line, and
    /// the finished word for one with a Tab in it, or `^C` for one abandoned (see [`keys`]).
    pub(crate) echoed: &'static str,
    pub(crate) jobs: u8,
    pub(crate) answer: &'static [&'static str],
    /// **A question the running line asks, and what the gate types back**: the cue it waits for
    /// after the echo, then the reply and Enter (milestone 892 (a std program at the prompt holds
    /// the network and a resolver): `jig install`'s yes). `None` for every line that asks nothing.
    pub(crate) asks: Option<(&'static str, &'static str)>,
}

/// **What the shell prints when a job it asked for never answered**, either half:
/// `components/src/swish.rs` prints the first when the progenitor could not build the job, and the
/// second is `swish::FAULTED_SENTENCE`, for a job that trapped first. Spelled here rather than
/// imported because xtask does not depend on `swish`, and a phrase is enough for a transcript.
pub(in crate::swish_check) const JOB_DID_NOT_RUN: [&str; 2] = [
    "could not spawn",
    "that command faulted and was killed before it answered",
];

/// A [`Line`], positionally, so the script reads as the prompt does. The name is provisional.
pub(in crate::swish_check) const fn line(
    jobs: u8,
    typed: &'static str,
    answer: &'static [&'static str],
) -> Line {
    Line {
        typed,
        echoed: typed,
        jobs,
        answer,
        asks: None,
    }
}

/// **A line that asks a question while it runs**, and the reply the gate types once the `cue`
/// appears (milestone 892): `jig install` of a package that asks the owner for grants. The name
/// is provisional.
pub(in crate::swish_check) const fn asked(
    jobs: u8,
    typed: &'static str,
    (cue, reply): (&'static str, &'static str),
    answer: &'static [&'static str],
) -> Line {
    Line {
        typed,
        echoed: typed,
        jobs,
        answer,
        asks: Some((cue, reply)),
    }
}

/// **A line typed with editing keys in it** (milestone 47 (navigation and naming), DECISIONS §227
/// (how Tab reaches the shell)): `typed` goes to the UART byte for byte, and `echoed` is what the
/// prompt then shows. A line ending in `^C` (`\x03`) is sent with no Enter, and must answer
/// nothing, because the shell discarded it. The name is provisional.
pub(in crate::swish_check) const fn keys(
    jobs: u8,
    typed: &'static str,
    echoed: &'static str,
    answer: &'static [&'static str],
) -> Line {
    Line {
        typed,
        echoed,
        jobs,
        answer,
        asks: None,
    }
}

/// Whether a scripted line is abandoned with `^C` rather than run.
pub(in crate::swish_check) fn interrupted_at_prompt(typed: &str) -> bool {
    typed.ends_with('\x03')
}
