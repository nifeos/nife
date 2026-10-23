//! **The capability shell's logic, with the IO taken out.** `components/src/swish/` is the
//! program, holding the terminal, spawn and result channels, a budget and maybe a directory; this
//! crate is everything it decides or renders with no capability in hand, so it is host-tested.
//! Every renderer writes through `out: &mut dyn FnMut(&[u8])`, and a directory listing comes
//! through a second callback, `expand`, so [`echo`] and [`expansion`] are testable end to end.
//! See notes/shell.md, "The program and the crate", and notes/shell/the-swish-crate.md.
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
    Command, Endowment, Holdings, OutputDeclaration, ParsedInvocation, Program,
    ProgramArgumentDeclaration, Refusal, Streams, spawnproto,
};
pub use number::write_num;
pub use preview::{Vouched, write_capabilities, write_image_capabilities, write_preview};
pub use routing::{Route, route};
pub use say::{HEAP_MAX_BYTES, Say, write_say};
pub use sentences::{
    FAULTED_SENTENCE, NO_DISPLAY_SENTENCE, REFUSED_BY_MANIFEST_SENTENCE, UNVOUCHED_SENTENCE,
    USER_USAGE, VOUCH_USAGE, image_refusal_sentence, spawn_failure_sentence, write_help,
    write_outcome, write_pwd, write_refusal, write_vouch,
};
pub use status::{STATUS_WORD, Status};
pub use timing::{Untimed, write_duration, write_timing, write_untimed};
pub use words::{Piece, designation, echo, expansion, is_pattern, pieces, write_set};
