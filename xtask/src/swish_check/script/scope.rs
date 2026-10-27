//! **The scoped block** (milestone 52, DECISIONS §160): `scope { commands }` puts back the
//! directory and the bound names its commands changed, on the way out, whether they worked or not.
//! It refuses before it runs what it could not put back, and `( ... )` is a refusal that names the
//! two forms that replaced it.
//!
//! Name: provisional (milestone 52, 2026-10-27 UTC).

use super::*;

/// The scoped-block rows of [`SWISH_CHECK_SCRIPT`], in table order.
pub(super) const LINES: &[Line] = &[
    // The block starts from the root so every `ls` below has one answer.
    line(0, "cd /", &[]),
    line(0, "mkdir scopes", &[]),
    // **A `cd` inside the block does not leak.** `pwd` inside says where the block stands, and the
    // `ls` after it lists the root, which holds `scopes`. A leak would list `scopes`'s own (empty)
    // contents.
    line(0, "scope { cd scopes ; pwd }", &["/scopes"]),
    line(0, "ls", &["scopes"]),
    // **A bound name does not leak.** Inside the block the name reaches the directory it was filed
    // for; after it, the name reaches nothing.
    line(0, "scope { bind scopes inner ; cd /inner ; pwd }", &["/scopes"]),
    line(0, "cd /inner", &["no such name in this directory"]),
    // **The block's status is its last command's**, so the connectors around it read the commands
    // inside it.
    line(0, "scope { cd scopes && echo kept } && echo after", &["kept", "after"]),
    // **A block that fails still puts everything back.** The second `cd` fails, the block says why
    // and answers with a failure, and the root is where this shell stands.
    line(
        0,
        "scope { cd scopes ; cd nowhere } || echo failed",
        &["no such name in this directory", "failed"],
    ),
    line(0, "ls", &["scopes"]),
    // **What a block could not put back is refused before anything in it runs**, and the refusal
    // names the isolation form. Nothing started: `jobs` lists nothing after it.
    line(
        0,
        "scope { cd scopes ; interrupt_heeder & }",
        &["cannot put back a background job", "swish -c"],
    ),
    line(0, "ls", &["scopes"]),
    // **`( ... )` has no meaning here**, and the refusal points at both forms.
    line(
        0,
        "(cd scopes && ls)",
        &["( ) groups nothing in this shell", "scope {", "swish -c"],
    ),
    line(0, "ls", &["scopes"]),
    // **The isolation form's spelling is reserved**, and the shell says the child is not built
    // rather than that the program does not exist.
    line(0, "swish -c ls", &["reserved for an isolated child", "not built yet"]),
];
