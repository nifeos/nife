//! `std::env::args()` on nife (milestone 205 (how a foreign program is told what to do),
//! DECISIONS §170).
//!
//! Reads the argument page the loader mapped read-only at `rt::ARGS_PAGE` (capability in
//! `rt::ARGS_SLOT`), in `program_argument_contract`'s layout, generated into this PAL as `argproto`.
//! Per §170 the arguments are bytes and carry no authority: what a path may reach was fixed before
//! the program started, by the directory in slot 4 alone.
//!
//! A program granted no argument page gets no arguments, not even `argv[0]`, and a page that does
//! not parse whole reads as no arguments too (`program_argument_contract`'s docs say why).

//! # Zero copies
//!
//! The page is read-only, mapped for the life of the process, and never written after assembly, so
//! each argument is an `&'static OsStr` borrowed straight off it. `OsStr` on nife is bytes, so any
//! byte string is a valid one, which is what makes `from_encoded_bytes_unchecked` sound here.

//! # BUGS
//!
//! - **Where the page sits and its layout are provisional** until calef rules on them
//!   (`design/roadmap/0672-the-argument-page-layout.md`). Both are generated from one crate,
//!   so a change there moves the loader and this reader together.

use crate::sys::pal::nife::argproto::ArgPage;
use crate::sys::pal::nife::rt;
use crate::ffi::{OsStr, OsString};
use crate::num::NonZero;
use crate::sync::OnceLock;
use crate::{fmt, slice};

/// A method number no object type defines, so the invocation can only ever be refused. The same
/// probe `sys/env/nife.rs` makes for the configuration page; not shared, because each PAL module is
/// meant to be readable with no other file open.
const NO_SUCH_METHOD: u64 = 0xffff;

static ARGS: OnceLock<Vec<&'static OsStr>> = OnceLock::new();

fn read() -> Vec<&'static OsStr> {
    // SAFETY: a plain syscall that cannot succeed; the kernel validates the slot. -1 is "the slot
    // is empty", and anything else means a capability is there.
    if unsafe { rt::invoke(rt::ARGS_SLOT, NO_SUCH_METHOD, 0, 0, 0) } == -1 {
        return Vec::new();
    }
    // SAFETY: the loader maps the argument page read-only at `rt::ARGS_PAGE` alongside the
    // capability the probe just found in `rt::ARGS_SLOT`, and nothing unmaps or writes it for the
    // life of the process.
    let page = unsafe { ArgPage::from_va(rt::ARGS_PAGE) };
    page.iter()
        // SAFETY: `OsStr` on nife is bytes, so every byte string is a valid encoding of one.
        .map(|a| unsafe { OsStr::from_encoded_bytes_unchecked(a) })
        .collect()
}

pub fn args() -> Args {
    Args { iter: ARGS.get_or_init(read).iter() }
}

pub struct Args {
    iter: slice::Iter<'static, &'static OsStr>,
}

impl !Send for Args {}
impl !Sync for Args {}

impl fmt::Debug for Args {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.iter.as_slice().fmt(f)
    }
}

impl Iterator for Args {
    type Item = OsString;

    fn next(&mut self) -> Option<OsString> {
        self.iter.next().map(|arg| arg.to_os_string())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.iter.len()
    }

    fn last(self) -> Option<OsString> {
        self.iter.last().map(|arg| arg.to_os_string())
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.iter.advance_by(n)
    }
}

impl DoubleEndedIterator for Args {
    fn next_back(&mut self) -> Option<OsString> {
        self.iter.next_back().map(|arg| arg.to_os_string())
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.iter.advance_back_by(n)
    }
}

impl ExactSizeIterator for Args {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.iter.is_empty()
    }
}
