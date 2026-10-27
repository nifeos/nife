//! **The scoped block** (milestone 52, DECISIONS §160): `scope { commands }` runs its commands, then
//! puts back the part of this shell's own state that they changed. The reading and the refusals
//! are `swish::scope`'s, host-tested; this is the running, since the state is the shell's.
//!
//! What is put back is what this shell holds as values: where it stands and the names `bind`
//! filed. It holds no variables or options yet, so there is nothing else to put back. The one
//! thing it holds that is a capability and not a value, a background job, is a change the block
//! refuses before it runs ([`Refusal::ScopeCannotRestore`]). The restoring happens whatever the
//! commands did, because every command here ends by returning: a failed command is a status, so
//! the way out of the block is the same on every path.
//!
//! Name: provisional (milestone 52, 2026-10-27 UTC).

use super::*;

/// **Run a scoped block**: `tail` is the `{ ... }` after the word `scope`.
///
/// The block's status is its last command's, so `scope { cd x && ls } || echo no` asks about the
/// listing. Nothing in the block runs unless the whole block is allowed to.
pub(super) fn scope_block(nav: &mut Nav, tail: &[u8]) {
    let commands = match swish::scope::check(tail) {
        Ok(commands) => commands,
        Err(r) => return say(Say::Cannot(r)),
    };
    let stood = nav.holds.cwd;
    let names = nav.holds.binds;

    dispatch_line(nav, commands);

    // The block's own answer, taken before the restoring can change `LAST`.
    let answer = last();
    nav.holds.binds = names;
    let mut restored = true;
    if nav.holds.cwd != stood {
        let said = nav.return_to(stood);
        if said != Say::Nothing {
            restored = false;
            say(said);
            print(b"  the block moved this shell and could not move it back: it stays where the block left it\n");
        }
    }
    let code = if restored { answer } else { Status::Failed };
    CURRENT.store(code.code(), core::sync::atomic::Ordering::Relaxed);
}
