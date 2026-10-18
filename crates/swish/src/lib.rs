//! **The capability shell's logic, with the IO taken out.** `components/src/swish.rs` is the
//! program, holding the terminal, spawn and result channels, a budget and maybe a directory; this
//! crate is everything it decides or renders with no capability in hand, so it is host-tested. If
//! a function here needed a capability, it would not be here.
//!
//! - [`route`] says what kind of thing was typed, answering the prefix words (`caps`, `time`)
//!   before the line is split on operators, or `caps date | wc` would lose everything after the
//!   pipe.
//! - [`write_duration`], [`write_timing`] and [`write_untimed`] render a duration, or one of the
//!   three reasons there is none.
//! - [`is_pattern`] and [`expansion`] decide whether a word is a designation to resolve or text to
//!   print, and which word on a line expands.
//! - Every sentence the user reads: [`write_say`], [`write_refusal`], [`write_outcome`],
//!   [`write_preview`], [`write_holdings`], [`write_help`], [`write_pwd`], [`write_num`].
//!
//! Every renderer takes `out: &mut dyn FnMut(&[u8])`: the program passes its terminal `print`, a
//! test a collector, a guest witness a buffer. A directory listing comes through a second callback,
//! `expand`, since only reading the directory needs a capability, so [`echo`] and [`expansion`] are
//! testable end to end against a fake directory.
//!
//! # BUGS
//!
//! `builtin` and `dispatch_one` (every arm is a capability invocation or a print), `run` (two
//! already-tested `grant_plan` calls around the spawn choice) and `spawn` and below (capability
//! movement) stay in the program; [`route`] lifts the decision they sit under.
//!
//! Name: ratified 2026-08-01 (calef), replacing `shell`. Refused `shell` (a category), `capsh`
//! (Linux's libcap ships one) and `sheesh` (dated, and an interjection). A swish goes through the
//! net touching nothing; the crate takes the program's name because it is the program's logic.

#![no_std]

pub mod bare;
pub mod complete;
pub mod jig_versions;
pub mod sequence;

mod apropos;
mod batching;
mod endowment;
mod number;
mod preview;
mod routing;
mod say;
mod sentences;
mod status;
#[cfg(test)]
mod test_support;
mod timing;
mod words;

#[cfg(test)]
extern crate std;

pub use apropos::{write_apropos, write_found};
pub use batching::{Sweep, write_batch, write_sweep};
use endowment::write_config_values;
pub use endowment::{write_census, write_holdings};
use environment_contract::ConfigPage;
use filesystem_contract::dir;
use grant_plan::expand::{Expansion, NameSet};
use grant_plan::line::{self, Line};
use grant_plan::nav::{self, Refused};
use grant_plan::{
    Command, Endowment, Holdings, OutputDeclaration, Prog, ProgramArgumentDeclaration, Refusal,
    RunSpec, Streams, spawnproto,
};
pub use number::write_num;
pub use preview::{Vouched, write_capabilities, write_image_capabilities, write_preview};
pub use routing::{Route, route};
pub use say::{HEAP_MAX_BYTES, Say, write_say};
pub use sentences::{
    FAULTED_SENTENCE, NO_DISPLAY_SENTENCE, REFUSED_BY_MANIFEST_SENTENCE, UNVOUCHED_SENTENCE,
    USER_USAGE, VOUCH_USAGE, image_refusal_sentence, write_help, write_outcome, write_pwd,
    write_refusal, write_vouch,
};
pub use status::{STATUS_WORD, Status};
pub use timing::{Untimed, write_duration, write_timing, write_untimed};
pub use words::{Piece, designation, echo, expansion, is_pattern, pieces, write_set};
