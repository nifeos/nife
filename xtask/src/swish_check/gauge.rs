//! **The gauges the kernel prints into a transcript, taken out as it arrives** (`script/swish-check`).
//! The progenitor-stack and capability-slot gauges speak from the kernel's idle loop, between a
//! prompt and the next line's echo, and every reader in [`super`] assumes those two are adjacent.
//! See notes/swish-check-harness.md, "The gauges".

#[allow(unused_imports)]
use super::*;

/// **Takes the kernel's progenitor-stack and capability-slot gauges out of the transcript as it
/// arrives**, and keeps each with the offset it was removed at.
///
/// Both land between `$ ` and the next line's echo, which every reader assumes are adjacent, so
/// the gauges never reach them. Streaming and prefix-stable: the waits use the output's length as
/// a cursor into growing text, so a tail that could start a gauge line is held until it is one
/// (dropped, newline and all) or is not (emitted), and output is only ever appended to. Only a
/// whole kernel line is removed; a gauge a userspace writer shuffled into is left and fails the
/// run. The raw transcript keeps both gauges for their own checks. See
/// notes/swish-check-harness.md, "The gauges".
pub(super) struct GaugeFilter {
    pending: String,
    needles: &'static [&'static str],
}

impl Default for GaugeFilter {
    fn default() -> Self {
        Self::with_needles(&Self::NEEDLES)
    }
}

impl GaugeFilter {
    /// What `kernel::progenitor_stack::announce` prints first, its leading indent included: the
    /// kernel's own prefix for the line. Name provisional.
    pub(super) const NEEDLE: &'static str = "  progenitor stack:";
    /// What `kernel::cap::report_peak` prints first, its leading indent included.
    pub(super) const SLOT_NEEDLE: &'static str = "  capability slots:";
    /// Every kernel line this filter takes out. Both start with the kernel's two-space indent,
    /// which the bare-prompt exception in [`GaugeFilter::feed`] relies on.
    pub(super) const NEEDLES: [&'static str; 2] = [Self::NEEDLE, Self::SLOT_NEEDLE];

    /// A filter for other kernel lines with the same shape (milestone 342's flood probe).
    pub(super) fn with_needles(needles: &'static [&'static str]) -> Self {
        GaugeFilter {
            pending: String::new(),
            needles,
        }
    }

    /// Feed the next chunk. Text that is certainly not a gauge is appended to `out`; each whole
    /// gauge line is pushed to `gauges` with the length `out` had when it was removed.
    pub(super) fn feed(
        &mut self,
        chunk: &str,
        out: &mut String,
        gauges: &mut Vec<(usize, String)>,
    ) {
        self.pending.push_str(chunk);
        loop {
            if let Some(at) = self
                .needles
                .iter()
                .filter_map(|n| self.pending.find(n))
                .min()
            {
                out.push_str(&self.pending[..at]);
                match self.pending[at..].find('\n') {
                    Some(nl) => {
                        let line = self.pending[at..at + nl].trim().to_string();
                        gauges.push((out.len(), line));
                        self.pending.drain(..at + nl + 1);
                    }
                    None => {
                        self.pending.drain(..at);
                        return;
                    }
                }
            } else {
                // Hold back the longest tail that is a proper prefix of the needle, except the
                // space of a bare `$ `: the needle starts with the kernel's indent, so without
                // this the prompt every wait below looks for would never be emitted whole.
                let mut keep = self
                    .needles
                    .iter()
                    .filter_map(|needle| {
                        (1..needle.len())
                            .rev()
                            .find(|&n| self.pending.ends_with(&needle[..n]))
                    })
                    .max()
                    .unwrap_or(0);
                let before = &self.pending[..self.pending.len() - keep];
                let after_dollar = if before.is_empty() {
                    out.ends_with('$')
                } else {
                    before.ends_with('$')
                };
                if keep > 0 && after_dollar {
                    keep -= 1;
                }
                let cut = self.pending.len() - keep;
                out.push_str(&self.pending[..cut]);
                self.pending.drain(..cut);
                return;
            }
        }
    }
}

/// One segment of a gauge sentence: literal text, or one of its numbers. A number is a wildcard
/// (matched as "one or more digits") rather than a fixed value, because the value is not known
/// ahead of time: [`degauge`] is asking "is a gauge here at all", not "is this exact gauge here".
#[derive(Clone, Copy)]
pub(super) enum GaugeSeg {
    Lit(&'static str),
    Num,
}

/// Every sentence `kernel::progenitor_stack::announce` and `kernel::cap::announce_peak` can print,
/// grepped from those two functions verbatim (2026-09-27). Longer variants first, so a `BELOW` or
/// `ABOVE` sentence is matched whole rather than leaving its tail as unmatched noise once the
/// shorter, common prefix has already been consumed. See [`degauge`].
pub(super) const GAUGE_TEMPLATES: &[&[GaugeSeg]] = &[
    &[
        GaugeSeg::Lit("  progenitor stack: "),
        GaugeSeg::Num,
        GaugeSeg::Lit(" of "),
        GaugeSeg::Num,
        GaugeSeg::Lit(" bytes at peak, "),
        GaugeSeg::Num,
        GaugeSeg::Lit(" spare, BELOW the "),
        GaugeSeg::Num,
        GaugeSeg::Lit("-byte floor in kernel/src/progenitor_stack.rs"),
    ],
    &[
        GaugeSeg::Lit("  progenitor stack: "),
        GaugeSeg::Num,
        GaugeSeg::Lit(" of "),
        GaugeSeg::Num,
        GaugeSeg::Lit(" bytes at peak, "),
        GaugeSeg::Num,
        GaugeSeg::Lit(" spare"),
    ],
    &[
        GaugeSeg::Lit("  capability slots: "),
        GaugeSeg::Num,
        GaugeSeg::Lit(" of "),
        GaugeSeg::Num,
        GaugeSeg::Lit(" at peak, ABOVE the "),
        GaugeSeg::Num,
        GaugeSeg::Lit(" recorded in kernel/src/cap.rs"),
    ],
    &[
        GaugeSeg::Lit("  capability slots: "),
        GaugeSeg::Num,
        GaugeSeg::Lit(" of "),
        GaugeSeg::Num,
        GaugeSeg::Lit(" at peak"),
    ],
];

/// Try `template` starting at `chars[start]`, tolerating intruder characters wedged between its
/// own the way [`find_marker`] tolerates them in a flat needle, up to the same
/// [`SWISH_CHECK_MARKER_SLACK`] budget. `None` if the template does not fit in the budget or runs
/// off the end of `chars`. On success, returns where the match ended and, for every position from
/// `start` to that end, whether it belongs to the gauge (`true`) or is an intruder byte that must
/// be left alone (`false`).
pub(super) fn gauge_template_match(
    chars: &[char],
    start: usize,
    template: &[GaugeSeg],
) -> Option<(usize, Vec<bool>)> {
    let mut mask = Vec::new();
    let mut j = start;
    let mut skipped = 0usize;
    for seg in template {
        match *seg {
            GaugeSeg::Lit(word) => {
                for want in word.chars() {
                    loop {
                        if j >= chars.len() {
                            return None;
                        }
                        if chars[j] == want {
                            mask.push(true);
                            j += 1;
                            break;
                        }
                        if skipped >= SWISH_CHECK_MARKER_SLACK {
                            return None;
                        }
                        mask.push(false);
                        skipped += 1;
                        j += 1;
                    }
                }
            }
            GaugeSeg::Num => {
                let mut got_digit = false;
                loop {
                    if j >= chars.len() {
                        if got_digit {
                            break;
                        }
                        return None;
                    }
                    if chars[j].is_ascii_digit() {
                        mask.push(true);
                        got_digit = true;
                        j += 1;
                    } else if got_digit {
                        // The number ended: this character belongs to whatever comes next, not to
                        // the digit run, so it is left for the following segment to see.
                        break;
                    } else if skipped >= SWISH_CHECK_MARKER_SLACK {
                        return None;
                    } else {
                        mask.push(false);
                        skipped += 1;
                        j += 1;
                    }
                }
            }
        }
    }
    Some((j, mask))
}

/// Delete the best (fewest intruder characters) occurrence of any [`GAUGE_TEMPLATES`] template
/// from `text`. `None` if no template appears at all within budget.
pub(super) fn strip_one_gauge(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut best: Option<(usize, usize, Vec<bool>, usize)> = None;
    for template in GAUGE_TEMPLATES {
        let GaugeSeg::Lit(first_word) = template[0] else {
            unreachable!("every gauge template starts with a literal");
        };
        let first_char = first_word.chars().next().expect("non-empty literal");
        for start in 0..chars.len() {
            if chars[start] != first_char {
                continue;
            }
            if let Some((end, mask)) = gauge_template_match(&chars, start, template) {
                let skipped = mask.iter().filter(|kept| !**kept).count();
                if best.as_ref().is_none_or(|(_, _, _, s)| skipped < *s) {
                    best = Some((start, end, mask, skipped));
                }
            }
        }
    }
    best.map(|(start, end, mask, _)| {
        let mut out = String::with_capacity(text.len());
        out.extend(&chars[..start]);
        for (offset, keep) in mask.iter().enumerate() {
            if !keep {
                out.push(chars[start + offset]);
            }
        }
        // A newline right where the gauge ended is the kernel's, so it goes with the gauge;
        // left behind, it splits the echo the gauge was spliced into.
        let rest = if chars.get(end) == Some(&'\n') {
            end + 1
        } else {
            end
        };
        out.extend(&chars[rest..]);
        out
    })
}

/// **Takes a gauge's own characters out of `text` even when a second writer spliced them in one at
/// a time**, and returns everything else exactly where it was.
///
/// Interim, until §175 (where the kernel's own output goes once userspace owns the console) is
/// built: a kernel that no longer writes the UART directly has nothing left here to splice.
/// Interleaving can destroy a known string but never manufacture one, so matching the gauge's own
/// words (numbers as wildcards) has no false positives worth the name. Unlike [`find_marker`], this
/// deletes the matched characters, so the caller's exact-match waits still recognize the rest. See
/// notes/swish-check-harness.md, "The gauges".
pub(super) fn degauge(text: &str) -> String {
    let mut out = text.to_string();
    // Bounded rather than "until none found": a gate must not hang on a text that somehow keeps
    // offering a match. A boot does not print more than a handful of gauge lines.
    for _ in 0..64 {
        match strip_one_gauge(&out) {
            Some(next) => out = next,
            None => break,
        }
    }
    out
}

/// **Takes out the prompt lines the console drew a second time beneath a gauge.** `filtered` is
/// [`GaugeFilter`]'s output and `gauges` the offsets it removed lines at.
///
/// A flush between a typed line's last echo and its Enter leaves `$ line\n$ line\n`, and
/// [`swish_check_answer`] would read the first copy as silent. Only an exact copy is removed: the
/// `$ ` line ending at a gauge's offset, followed there by the same line and its line end. A prefix
/// redraw (`$ pack\n$ packages/...\n`) and any other kernel line stay.
/// `no_script_types_a_silent_line_twice_in_a_row` keeps every script free of the one shape this
/// could mistake for a redraw. See notes/swish-check-flake.md, "The noteless signature", and
/// notes/swish-check-harness.md.
pub(super) fn without_redraws(filtered: &str, gauges: &[(usize, String)]) -> String {
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    let mut offsets: Vec<usize> = gauges.iter().map(|(at, _)| *at).collect();
    offsets.dedup();
    for at in offsets {
        let Some(before) = filtered.get(..at).and_then(|b| b.strip_suffix('\n')) else {
            continue;
        };
        let start = before.rfind('\n').map_or(0, |i| i + 1);
        let partial = &before[start..];
        if !partial.starts_with("$ ") {
            continue;
        }
        let copy = &filtered[at..];
        if copy.starts_with(partial) && copy[partial.len()..].starts_with('\n') {
            cuts.push((start, at));
        }
    }
    let mut out = String::with_capacity(filtered.len());
    let mut from = 0;
    for (start, end) in cuts {
        if start >= from {
            out.push_str(&filtered[from..start]);
            from = end;
        }
    }
    out.push_str(&filtered[from..]);
    out
}

/// Which typed command a gauge removed at `at` belongs to: the last `$ ` line before it, skipping
/// the bare prompt the gauge usually follows, since that prompt is the *next* command's.
pub(super) fn gauge_follows(transcript: &str, at: usize) -> &str {
    let before = transcript[..at].trim_end_matches("$ ");
    before
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("$ ").filter(|c| !c.trim().is_empty()))
        .unwrap_or("(boot)")
}

#[cfg(test)]
mod tests;
