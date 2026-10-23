//! **What a command did**: [`Status`], which is what `$?` reports and what `&&` reads, and
//! [`STATUS_WORD`], the word that reads it.
//!
//! Name: provisional (milestone 884, 2026-10-12 UTC), for calef's naming pass.

/// **What a command did**, which is what `$?` reports and what `&&` reads (milestone 67 (`swish`
/// the language)).
///
/// [`Refused`](Status::Refused) is the shell declining the line, with nothing spawned and no
/// authority moved; [`Failed`](Status::Failed) is something attempted that did not work. It is the
/// shell's own reading of the line and designates nothing. A per-program status is DECISIONS §301
/// (exit status and output)'s, which milestone 172 (a capability-native subprocess primitive)
/// proposes and which is not built. See notes/swish-language.md, "Exit status: a refusal is not an error".
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Status {
    /// The command ran and the shell has nothing to report. `$?` is `0`.
    #[default]
    Ran,
    /// Something was attempted and did not work. `$?` is `1`.
    Failed,
    /// The shell would not run it, so nothing did. `$?` is `2`.
    Refused,
}

impl Status {
    /// Whether `&&` should carry on. Only [`Ran`](Status::Ran) is a yes.
    pub fn is_ok(self) -> bool {
        matches!(self, Status::Ran)
    }

    /// The number `$?` shows.
    pub fn code(self) -> u64 {
        match self {
            Status::Ran => 0,
            Status::Failed => 1,
            Status::Refused => 2,
        }
    }

    /// Read a status back out of [`code`](Status::code). The shell keeps it in an atomic cell,
    /// because every printer in the program can reach it and none of them holds a `&mut` to
    /// anything shared.
    pub fn from_code(code: u64) -> Status {
        match code {
            0 => Status::Ran,
            1 => Status::Failed,
            _ => Status::Refused,
        }
    }

    /// The number as bytes, which is `'static` because there are three of them.
    ///
    /// That is not a micro-optimization: a substituted word has to be a slice with the line's
    /// lifetime, and a `'static` slice unifies with any of them. The shell has had a capped heap
    /// since 2026-09-26, so what keeps a substitution one word is [`pieces`](crate::pieces).
    pub fn digits(self) -> &'static [u8] {
        match self {
            Status::Ran => b"0",
            Status::Failed => b"1",
            Status::Refused => b"2",
        }
    }
}

/// **The word that reads the last command's status.** Recognized where words are expanded, which
/// today is [`echo`](crate::echo).
///
/// It is spelled `$?` because that is the spelling every shell user already knows, and this project
/// does not respell a name a reader arrives with. It is **not** a variable: there is no variable
/// mechanism here at all (milestone 47 owns that, and studies it as "the same question wearing a
/// string costume"), so this is one word the expander knows, in the same category as a pattern.
pub const STATUS_WORD: &[u8] = b"$?";

#[cfg(test)]
mod tests;
