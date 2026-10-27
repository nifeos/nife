//! **What a builtin has to say**: [`Say`], the value a builtin returns rather than printing, the
//! heap cap its `HeapFull` sentence names, and [`write_say`], which renders it.
//!
//! Name: provisional (milestone 884, 2026-10-12 UTC), for calef's naming pass.

use super::*;

/// What a builtin has to say. A value rather than a print, because the printing half belongs to the
/// interactive prompt and the navigating witness (which has no terminal) runs the same builtins.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Say {
    /// It worked and there is nothing to add.
    Nothing,
    /// The name could not be navigated, and nothing was sent.
    Refused(Refused),
    /// The filesystem refused, with this errno. Rendered by `filesystem_contract::dir::explain`, which keeps
    /// the sentence next to the decision that chose the number.
    Failed(i32),
    /// This shell holds no directory capability, so there is nothing to name.
    NoDirectory,
    /// The verb needs an operand and got none.
    NeedsAName,
    /// **`touch -t`'s operand did not parse as an RFC 3339 instant, or names one this wire cannot
    /// carry** (a pre-1970 instant: `filesystem_contract::fs::SETMTIME_AT`'s value is an unsigned
    /// seconds count). DECISIONS §112. Not a [`Say::Failed`], deliberately: this is refused before
    /// any wire request is made, so there is no errno the filesystem chose and `dir::explain`
    /// would have nothing true to say about it.
    NotAnInstant,
    /// **A designation this shell cannot back**, in `grant_plan`'s vocabulary. It arrives here
    /// because expansion happens *before* planning, so a pattern that matched nothing (or too much)
    /// is refused before there is a program to attribute it to, and [`echo`] can reach the same
    /// refusals with no program on the line at all.
    Cannot(Refusal),
    /// **`bind` refused**, in [`nav::BindRefused`]'s vocabulary: the name is already bound, the
    /// table is full, or (a two-grant shell only, not reachable from any real boot yet) the name
    /// collides with one of this shell's own grant labels. Distinct from
    /// [`Refused`](Say::Refused): that is a fact about the *path* being navigated, this is a fact
    /// about the *name being filed*, and the two vocabularies stay separate for the same reason
    /// [`nav::Refused`] and [`nav::BindRefused`] are separate types rather than one enum with two
    /// unrelated halves.
    CannotBind(nav::BindRefused),
    /// **The shell's heap could not hold what the line needed**, so nothing ran. The heap is capped
    /// at [`HEAP_MAX_BYTES`] and every allocation in the shell is fallible (calef's ruling, milestone
    /// 47 (navigation and naming)): the boot shell is the owner's console, and running out of
    /// memory there is a refusal with a sentence, never a panic.
    HeapFull,
}

/// **The cap on the shell's heap**: 32 KiB, eight pages, mapped from the shell's budget all at once
/// at `_start`, before anything else is carved from it (milestone 47 (navigation and naming),
/// calef's ruling of 2026-09-26).
///
/// Why this number. The heap holds what a line needs for as long as the line runs: a pipeline's
/// planned stages today, and later a name page for a set grant. Those are a few KiB, so 32 KiB is
/// several times the largest line. The cap exists so that a leak exhausts the heap, which refuses
/// the next line with [`Say::HeapFull`], long before it could reach the budget the shell builds its
/// children from: eight pages, and a few more for their page tables, of the budget's 128.
///
/// Name: provisional, milestone 47's allocator lane, 2026-09-26.
pub const HEAP_MAX_BYTES: u64 = 32 * 1024;

/// Write what a builtin had to say. Every line is a statement about a name or a capability, never
/// about a policy: `filesystem_contract::dir::explain` keeps the filesystem's half next to the decision that
/// chose the errno, so this function does not get to invent a friendlier word for a refusal.
pub fn write_say(s: Say, out: &mut dyn FnMut(&[u8])) {
    match s {
        Say::Nothing => {}
        Say::Refused(r) => {
            out(b"  ");
            out(r.message().as_bytes());
            out(b"\n");
        }
        Say::Failed(errno) => {
            out(b"  ");
            out(dir::explain(errno).as_bytes());
            out(b"\n");
        }
        Say::NoDirectory => {
            out(b"  this shell holds no directory capability; there is nothing here to name\n");
        }
        Say::NeedsAName => out(b"  name what you mean: this verb takes one\n"),
        Say::NotAnInstant => {
            out(b"  -t needs an RFC 3339 instant, e.g. 2030-01-01T00:00:00Z\n");
        }
        Say::Cannot(r) => {
            out(b"  ");
            out(r.message().as_bytes());
            out(b"\n");
        }
        Say::CannotBind(r) => {
            out(b"  ");
            out(r.message().as_bytes());
            out(b"\n");
        }
        Say::HeapFull => {
            out(b"  this shell's heap is full (its cap is ");
            write_num(HEAP_MAX_BYTES / 1024, out);
            out(b" KiB), so the line did not run and nothing was spawned\n");
        }
    }
}

#[cfg(test)]
mod tests;
