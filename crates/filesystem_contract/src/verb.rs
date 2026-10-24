use super::{dir, fs};

/// **What the length field of a request word counts**, which is the only thing about a request
/// a proxy has to understand in order to forward it faithfully.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operand {
    /// Nothing. The request names a handle and the length field is unused, so a caretaker
    /// forwards a length of **zero** rather than whatever the client happened to send. That is
    /// not tidiness: it means a client cannot smuggle a length into a verb that has none and
    /// have some later server give it a meaning.
    None,
    /// A **directory name**, `req_len` bytes at the start of the shared page, resolved under the
    /// request's handle. This is the operand a name filter applies to, and the only one:
    /// `fs_nameset_caretaker` asks its question exactly when this is the shape.
    Name,
    /// `req_len` bytes of **payload** that are not a directory name: file data
    /// ([`fs::READ`]/[`fs::WRITE`]), or an attribute name and value. Forwarded verbatim, and
    /// **never filtered**, because an attribute name is not a name in the namespace the
    /// capability narrows.
    Payload,
    /// Two directories and two names, [`fs::RENAME`] alone: the source is the request word's
    /// handle and length, the destination rides in the second word ([`fs::rename_dst`]), and
    /// both names sit back to back in the page. The only shape a proxy must rewrite rather than
    /// forward, because both handles are the client's.
    Rename,
}

/// One row: everything a proxy needs to know about one verb.
#[derive(Clone, Copy, Debug)]
pub struct Verb {
    /// The opcode. Rows are stored in opcode order and the ordering is asserted at compile time.
    pub operation: u64,
    /// Its name, for a diagnostic and for a test's failure message. Costs nothing at runtime in
    /// the caretakers, which never read it.
    pub name: &'static str,
    /// What the length field counts.
    pub operand: Operand,
    /// Whether the **second word** carries something the server reads (an offset, a size, a
    /// cursor, a rights mask, an attribute spec). When it does not, a caretaker forwards zero,
    /// for [`Operand::None`]'s reason.
    pub carries_w1: bool,
    /// Whether a successful reply is a **new handle**, which a proxy that renumbers handles must
    /// install in its own table before handing back.
    pub mints_handle: bool,
    /// The [`dir`] rights the server demands, **all of them**. Zero where the verb needs none
    /// ([`fs::CLOSE`], [`fs::FSTAT`]) or where the requirement is "any of" rather than "all of"
    /// (see [`Verb::needs_any`]).
    pub needs_all: u64,
    /// The rights where **any one** of them will do: only [`fs::OPEN`], which takes
    /// [`dir::READ`] or [`dir::WRITE`]. Kept apart from [`Verb::needs_all`] because
    /// `Rights::allows` is deliberately "all of", and folding an any-of requirement into it
    /// would be a hole rather than a simplification.
    pub needs_any: u64,
}

impl Verb {
    /// Whether the length field means anything, so a caretaker knows to forward it.
    pub const fn carries_len(&self) -> bool {
        !matches!(self.operand, Operand::None)
    }

    /// Whether the operand is a name in the namespace a caretaker narrows. **This is what a
    /// name filter keys off**, and the reason it is a table field rather than a list inside
    /// `fs_nameset_caretaker`: a name-taking verb added to the contract is filtered from the
    /// moment its row exists, instead of from the moment somebody remembers.
    pub const fn takes_name(&self) -> bool {
        matches!(self.operand, Operand::Name | Operand::Rename)
    }

    /// Whether the verb **mutates**, which is the one question a direction-carrying grant asks.
    /// A read-only per-file grant refuses exactly these with [`super::grant::EROFS`], so a
    /// mutating verb added to the contract is refused by a read-only grant from its first day
    /// rather than forwarded because nobody updated a list.
    pub const fn mutates(&self) -> bool {
        self.needs_all & (dir::WRITE | dir::CREATE | dir::REMOVE) != 0
    }
}

/// A row, spelled once so the table below reads as data.
const fn row(
    operation: u64,
    name: &'static str,
    operand: Operand,
    carries_w1: bool,
    mints_handle: bool,
    needs_all: u64,
) -> Verb {
    Verb {
        operation,
        name,
        operand,
        carries_w1,
        mints_handle,
        needs_all,
        needs_any: 0,
    }
}

/// The lowest opcode in the contract, and the first row's.
pub const FIRST: u64 = fs::OPEN;
/// The highest opcode in the contract, and the last row's. Raising it without adding a row is a
/// compile error.
pub const LAST: u64 = fs::SETMTIME_AT;

/// **Every verb the file-service contract carries, in opcode order.**
///
/// The rights column is the server's, copied from the doc comment on each opcode in [`fs`] and
/// checked against the server's behaviour by `redoxfs_server`'s own tests. It is here because a proxy
/// that knows a verb mutates can refuse it without having to know what mutating means.
pub const TABLE: [Verb; (LAST - FIRST + 1) as usize] = [
    // OPEN's requirement is "READ or WRITE", the one any-of in the contract, so it is the one
    // row built by hand rather than by `row`.
    // Its second word is the per-step rights request a relative path descends with (milestone
    // 606, ruling A), so a caretaker must forward it.
    Verb {
        operation: fs::OPEN,
        name: "OPEN",
        operand: Operand::Name,
        carries_w1: true,
        mints_handle: true,
        needs_all: 0,
        needs_any: dir::READ | dir::WRITE,
    },
    // The second word is an offset for both, which is why they carry it and TRUNCATE's size
    // rides there too: an offset-shaped quantity is not a payload length.
    row(fs::READ, "READ", Operand::Payload, true, false, dir::READ),
    row(
        fs::WRITE,
        "WRITE",
        Operand::Payload,
        true,
        false,
        dir::WRITE,
    ),
    row(fs::CLOSE, "CLOSE", Operand::None, false, false, 0),
    row(fs::FSTAT, "FSTAT", Operand::None, false, false, 0),
    row(
        fs::CREATE,
        "CREATE",
        Operand::Name,
        false,
        true,
        dir::CREATE,
    ),
    row(
        fs::TRUNCATE,
        "TRUNCATE",
        Operand::None,
        true,
        false,
        dir::WRITE,
    ),
    // OPENDIR and MKDIR carry the requested rights mask in the second word, which is what makes
    // them the two verbs that hand back *authority* rather than data.
    row(
        fs::OPENDIR,
        "OPENDIR",
        Operand::Name,
        true,
        true,
        dir::DESCEND,
    ),
    row(
        fs::READDIR,
        "READDIR",
        Operand::None,
        true,
        false,
        dir::ENUMERATE,
    ),
    row(
        fs::MKDIR,
        "MKDIR",
        Operand::Name,
        true,
        true,
        dir::CREATE | dir::DESCEND,
    ),
    // RENAME needs REMOVE on the source directory and CREATE on the destination. Both are in
    // `needs_all` because a proxy asks only "does this mutate", and it does, twice over.
    row(
        fs::RENAME,
        "RENAME",
        Operand::Rename,
        true,
        false,
        dir::REMOVE | dir::CREATE,
    ),
    row(
        fs::UNLINK,
        "UNLINK",
        Operand::Name,
        false,
        false,
        dir::REMOVE,
    ),
    row(fs::RMDIR, "RMDIR", Operand::Name, false, false, dir::REMOVE),
    // The extended-attribute verbs (milestone 57). Their operand is a **payload**, not a name:
    // an attribute name is not a name in the directory, so a name-set capability must not filter
    // it and a per-file capability must not compare it against the granted name. Getting that
    // wrong would have been the natural mistake here, which is why the distinction is a variant
    // of `Operand` rather than a comment.
    row(
        fs::GETXATTR,
        "GETXATTR",
        Operand::Payload,
        false,
        false,
        dir::READ,
    ),
    // The only attribute verb with a second word: the value's length and type code.
    row(
        fs::SETXATTR,
        "SETXATTR",
        Operand::Payload,
        true,
        false,
        dir::WRITE,
    ),
    row(
        fs::LISTXATTR,
        "LISTXATTR",
        Operand::None,
        false,
        false,
        dir::READ,
    ),
    row(
        fs::REMOVEXATTR,
        "REMOVEXATTR",
        Operand::Payload,
        false,
        false,
        dir::WRITE,
    ),
    // STATFS names a handle and nothing else: no operand, no second word, no right. The reply
    // fills the shared page, which costs a proxy nothing because the page is the one all three
    // parties already share (see `fs_subtree_caretaker`'s `PAGE_VA`).
    row(fs::STATFS, "STATFS", Operand::None, false, false, 0),
    // SYNC: any handle qualifies, and it needs `dir::WRITE`, for the reason `fs::SYNC` gives.
    row(fs::SYNC, "SYNC", Operand::None, false, false, dir::WRITE),
    // **The mtime verbs** (milestone 47's `touch`; DECISIONS §112). All three are name-taking,
    // resolved directly under a directory handle rather than against an open file handle,
    // because `touch` never opens what it acts on: `fs::CREATE` and `fs::UNLINK` already take
    // this shape for the same reason.
    row(
        fs::GETMTIME,
        "GETMTIME",
        Operand::Name,
        false,
        false,
        dir::READ,
    ),
    row(
        fs::SETMTIME,
        "SETMTIME",
        Operand::Name,
        false,
        false,
        dir::WRITE,
    ),
    // The only one of the three that carries a second word: the caller's asserted seconds
    // value, TRUNCATE's reason for putting an offset-shaped quantity in `w1` rather than the
    // length field. `needs_all` carries both rights, exactly as RENAME's does for its two: a
    // proxy that only asks "does this mutate" needs no finer answer, and the FS server is what
    // actually checks each bit.
    row(
        fs::SETMTIME_AT,
        "SETMTIME_AT",
        Operand::Name,
        true,
        false,
        dir::WRITE | dir::SETTIME,
    ),
];

/// **The row for an opcode, or `None` if the contract does not carry it.**
///
/// A caretaker's whole dispatch is this lookup plus the fields it returns, so an opcode a client
/// invents is refused at one site rather than falling through a match's catch-all.
pub const fn of(operation: u64) -> Option<&'static Verb> {
    if operation < FIRST || operation > LAST {
        return None;
    }
    Some(&TABLE[(operation - FIRST) as usize])
}

// **The compile-time half of the deliverable.** Adding an opcode to `fs` and forgetting a row
// here fails the build instead of producing a caretaker that silently does not offer it. A
// runtime test cannot do this job: the three caretakers are `no_std` binaries and a test that
// has to boot them would find the gap long after the commit that made it.
const _: () = assert!(
    TABLE.len() == (LAST - FIRST + 1) as usize,
    "every opcode between verb::FIRST and verb::LAST needs a row in verb::TABLE"
);
const _: () = assert!(
    in_opcode_order(&TABLE),
    "verb::TABLE is stored in opcode order and indexed by it; a row is out of place"
);

/// Does `table` hold one row per opcode from [`FIRST`], in order? A function rather than a loop
/// inside the `const` above, because a check that only ever meets a good table cannot show it
/// would refuse a bad one, and this one is fed a bad one in `tests.rs`.
pub(crate) const fn in_opcode_order(table: &[Verb]) -> bool {
    let mut i = 0;
    while i < table.len() {
        if table[i].operation != FIRST + i as u64 {
            return false;
        }
        i += 1;
    }
    true
}

/// **What a per-file grant answers for each verb** (`components/src/fs_file_caretaker.rs`).
///
/// This one caretaker needs a policy and the other two do not, and the asymmetry is the design
/// rather than an accident. `fs_subtree_caretaker` and `fs_nameset_caretaker` serve the
/// *directory* protocol their client speaks, so every verb means what it always meant and the
/// attenuation lives elsewhere (in the handle the server minted, and in a name filter). A file
/// capability is a **different protocol**: it has no namespace to enumerate, nothing to create a
/// name in, and exactly one handle. So "which verbs exist" is part of what the capability *is*,
/// and that belongs in a table somebody wrote on purpose.
pub mod file_grant {
    use super::super::grant;

    /// What the caretaker does with one verb.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Policy {
        /// **Answered here, from what this process holds.** [`super::fs::OPEN`] resolves the one
        /// granted name; [`super::fs::CLOSE`] is a truthful no-op, because the caretaker owns
        /// the underlying handle for its whole life and a client closing "its" handle must not
        /// be able to make the next request fail.
        Local,
        /// **Forwarded on the handle the caretaker opened at startup**, with the caller's handle
        /// substituted. A mutating verb ([`super::Verb::mutates`]) is refused with
        /// [`grant::EROFS`] when the grant carries no write direction, and the refusal is
        /// table-driven rather than listed, so a mutating verb added to the contract is refused
        /// by a read-only grant from the day its row exists.
        Forward,
        /// **Not offered, and the errno says why.** [`grant::ENOTDIR`] where the request does
        /// not *mean* anything through a file capability, which is a stronger and more useful
        /// statement than a refusal: a refusal implies a policy that could have said yes, and
        /// there is no policy here, only what the capability is.
        Refused(i32),
    }

    /// The policy for each verb, indexed exactly as [`super::TABLE`] is.
    ///
    /// # BUGS
    ///
    /// Every directory verb answers `ENOTDIR`, the same statement `CREATE` makes: a file
    /// capability is not a directory, so the request does not mean anything here. They
    /// answered `EBADF` until 2026-08-01, inherited from the catch-all this table replaced,
    /// which shared one arm with "you named a handle I never minted". Writing the rows down is
    /// what made the conflation visible, and the fix is that the two words now mean one thing
    /// each: `EBADF` is about the caller's handle, `ENOTDIR` is about the request.
    pub const POLICY: [Policy; super::TABLE.len()] = [
        Policy::Local,                   // OPEN: the one granted name, anything else ENOENT.
        Policy::Forward,                 // READ
        Policy::Forward,                 // WRITE, EROFS without the direction
        Policy::Local,                   // CLOSE
        Policy::Forward,                 // FSTAT
        Policy::Refused(grant::ENOTDIR), // CREATE: a file capability is not a directory.
        Policy::Forward,                 // TRUNCATE, EROFS without the direction
        // **All six say what `CREATE` says**: a file capability is not a directory, so "list it",
        // "make a name in it" or "take a name out of it" do not mean anything here. `EBADF` would
        // be about the caller's handle; `ENOTDIR` is about the request.
        Policy::Refused(grant::ENOTDIR), // OPENDIR
        Policy::Refused(grant::ENOTDIR), // READDIR
        Policy::Refused(grant::ENOTDIR), // MKDIR
        Policy::Refused(grant::ENOTDIR), // RENAME
        Policy::Refused(grant::ENOTDIR), // UNLINK
        Policy::Refused(grant::ENOTDIR), // RMDIR
        // Forwarded: an attribute is part of what a file *is* (see `xattr`'s header), so a
        // capability that may read the file may read what is attached to it. The two that mutate
        // are refused with `EROFS` through a read-only grant by `Policy::Forward`'s own rule.
        Policy::Forward, // GETXATTR
        Policy::Forward, // SETXATTR, EROFS without the direction
        Policy::Forward, // LISTXATTR
        Policy::Forward, // REMOVEXATTR, EROFS without the direction
        // STATFS is forwarded rather than refused, and it is the one directory-shaped question
        // that means something through a file capability: a program granted one file to write
        // still has to know whether its next write fits. The server takes any handle it minted,
        // so the caretaker's substituted file handle answers it.
        Policy::Forward, // STATFS
        // SYNC is forwarded for STATFS's reason (any handle qualifies, the question is about
        // the device) and refused with EROFS through a read-only grant by `Policy::Forward`'s
        // own rule, matching SYNC's own doc ("Rights: dir::WRITE").
        Policy::Forward, // SYNC
        // The mtime verbs (milestone 47, DECISIONS §112) are all name-taking, resolved under a
        // *directory* handle, exactly like CREATE/OPENDIR/UNLINK/RENAME above: a file
        // capability has no directory to resolve a name under. (`dir::SETTIME` never applies to a
        // file handle anyway: a file entry is attenuated to `READ | WRITE` when it is opened.)
        Policy::Refused(grant::ENOTDIR), // GETMTIME
        Policy::Refused(grant::ENOTDIR), // SETMTIME
        Policy::Refused(grant::ENOTDIR), // SETMTIME_AT
    ];

    /// `EBADF`: no such handle, and only that since 2026-08-01 (see [`POLICY`]'s BUGS).
    pub const EBADF: i32 = 9;

    /// The policy for an opcode, or `None` for an opcode the contract does not carry.
    pub const fn of(operation: u64) -> Option<Policy> {
        if operation < super::FIRST || operation > super::LAST {
            return None;
        }
        Some(POLICY[(operation - super::FIRST) as usize])
    }

    // A policy per verb, checked where a missing one is cheapest to find.
    const _: () = assert!(
        POLICY.len() == super::TABLE.len(),
        "every verb needs a per-file-grant policy; add the row next to the verb's"
    );
}

#[cfg(test)]
mod tests;
