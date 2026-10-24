//! `std::env`'s variables on nife (milestone 64, rank 4 of the measured gap list).
//!
//! A nife process starts with an **empty** environment, seeded only from its own grants
//! ([`seed`]): an environment variable is not a capability, so nothing is inherited from the
//! spawner. `env::var("HOME")` is `None` because nobody gave this program a home, the same `None` a
//! Unix program gets for an unset variable. `env::vars()` is an empty iterator, never a panic:
//! std's `unsupported` backend panics there, and `Command::envs`, `dotenvy` and loggers call it.
//!
//! `set_var` and `remove_var` operate on a process-local table that `var` reads back. Nothing
//! leaves the process: nife has no `execve` carrying an environment, and spawn is by capability.

//! # `TZ`, `LANG` and `TERM` are seeded from a grant (milestone 47, DECISIONS §111)
//!
//! [`seed`] is called once, from `pal::nife::init`, before `main` runs. It reads the
//! inert-configuration page this process was granted (`rt::CONFIG_SLOT`, `rt::CONFIG_PAGE`), if
//! any, and pushes its keys into [`ENV`]. A program granted no such page is seeded with nothing,
//! so `env::var("TZ")` answers `Err` for a program nobody handed a timezone to. See
//! `environment_contract` for why this needs no seqlock and for the closed domains the keys are
//! validated against.

//! # BUGS
//!
//! - **Only the inert-configuration third is seeded.** `PATH` and `HOME` are **names** (directory
//!   capabilities wearing a string costume) and wait on a two-directory endowment; secrets are
//!   answered by an endpoint (§41 (the endpoint is the broker)) and never arrive as a string on
//!   this table. A program cannot be *handed* an arbitrary variable the way it can be handed `TZ`.
//! - **The table is per-process and dies with it.** There is no `/etc/environment`, no shell export
//!   that survives, and no way for a parent to hand one down.

use core::sync::atomic::{AtomicU8, Ordering};

use crate::ffi::{OsStr, OsString};
use crate::sync::Mutex;
use crate::sys::pal::nife::envproto::ConfigPage;
use crate::sys::pal::nife::rt;
use crate::{fmt, io, vec};

/// The process's variables. `Vec` rather than `HashMap` on purpose: an environment here holds what
/// the program itself put in it, which is a handful of entries, and a map would drag `RandomState`
/// (and therefore the entropy PAL) into the first `env::var` any program makes.
static ENV: Mutex<Vec<(OsString, OsString)>> = Mutex::new(Vec::new());

/// A snapshot of the process's variables, taken while the lock was held.
///
/// Snapshot rather than a live view, because std's `Env` outlives the borrow and a program is free
/// to `set_var` while iterating. Unix has the same hazard and answers it the same way.
pub struct Env {
    iter: vec::IntoIter<(OsString, OsString)>,
}

impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter.as_slice()).finish()
    }
}

impl Iterator for Env {
    type Item = (OsString, OsString);

    fn next(&mut self) -> Option<(OsString, OsString)> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// Every variable this process holds: empty until the program or [`seed`] sets one, and never a
/// panic (see the module docs).
pub fn env() -> Env {
    let snapshot = ENV.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Env { iter: snapshot.into_iter() }
}

pub fn getenv(k: &OsStr) -> Option<OsString> {
    let guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
    guard.iter().find(|(name, _)| name == k).map(|(_, value)| value.clone())
}

/// # Safety
///
/// Same contract as every other platform's: the caller must not be racing another thread that is
/// reading the environment.
pub unsafe fn setenv(k: &OsStr, v: &OsStr) -> io::Result<()> {
    let mut guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
    match guard.iter_mut().find(|(name, _)| name == k) {
        Some(entry) => entry.1 = v.to_owned(),
        None => guard.push((k.to_owned(), v.to_owned())),
    }
    Ok(())
}

/// # Safety
///
/// See [`setenv`].
pub unsafe fn unsetenv(k: &OsStr) -> io::Result<()> {
    let mut guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
    guard.retain(|(name, _)| name != k);
    Ok(())
}

/// Whether an inert-configuration page was granted: 0 = not yet asked, 1 = granted, 2 = not.
///
/// The probe must not touch the page, because a program granted none has no page mapped and a read
/// would fault. It invokes the slot with a method no object defines: an empty slot answers
/// `NoSuchSlot` (-1), and the config page's PageFrame answers `BadMethod` (-5), a refusal from a
/// real object. Cached, because a slot's contents are fixed at spawn on this ABI. The same probe as
/// `sys/time/nife.rs`'s for the clock.
static CONFIG_GRANTED: AtomicU8 = AtomicU8::new(0);

/// A method number no object type defines, so the invocation can only ever be refused. Same
/// constant `sys/time/nife.rs` uses for its own probe; not shared between the two files because
/// each PAL module is meant to be readable with no other file open.
const NO_SUCH_METHOD: u64 = 0xffff;

fn is_config_granted() -> bool {
    match CONFIG_GRANTED.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            // SAFETY: a plain syscall that cannot succeed; the kernel validates the slot.
            let r = unsafe { rt::invoke(rt::CONFIG_SLOT, NO_SUCH_METHOD, 0, 0, 0) };
            let ok = r != -1; // anything but "the slot is empty" means a capability is there
            CONFIG_GRANTED.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
            ok
        }
    }
}

/// Seed [`ENV`] from the inert-configuration page this process was granted, if any (milestone
/// 47's environment-variable fork, DECISIONS §111). Called once, from `pal::nife::init`, before
/// `main` runs; see the module docs.
///
/// A process granted no config page calls [`is_config_granted`] once, gets `false`, and returns
/// having touched nothing, so this is safe to call unconditionally at startup.
pub fn seed() {
    if !is_config_granted() {
        return;
    }
    // SAFETY: the loader maps the config page read-only at `rt::CONFIG_PAGE` alongside the
    // capability the probe just found in `rt::CONFIG_SLOT`, and nothing unmaps or writes it
    // (the page has exactly one writer, and it is not this process; see `environment_contract`'s docs).
    let page = unsafe { ConfigPage::new(rt::CONFIG_PAGE) };
    let mut guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
    for (key, value) in [("TZ", page.tz()), ("LANG", page.lang()), ("TERM", page.term())] {
        if let Some(v) = value {
            guard.push((OsString::from(key), OsString::from(v)));
        }
    }
}
