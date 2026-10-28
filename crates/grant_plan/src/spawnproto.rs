//! **The spawn protocol: the wire half of the shell-to-progenitor grant expression.**
//!
//! The shell resolves a `run` into an [`Endowment`](crate::Endowment) but does not build the child:
//! the progenitor holds the initial ramdisk and the ELF loader. So the shell sends three request
//! words on the spawn endpoint ([`request`]), then any data messages, then the capabilities it
//! grants, in the fixed order both sides read from the same [`Wiring`], and the progenitor answers
//! on the result endpoint. A userspace protocol, not kernel ABI: the kernel routes these words as
//! any IPC and never reads them (§10 (process model: capability-based)).
//! notes/grant-expression/spawn-protocol.md has the exchange.

//! # BUGS
//!
//! - A withheld promised message hangs the progenitor ([`Wiring::run_unvouched`] claimed without
//!   [`RUN_UNVOUCHED_SLOT`], a frame not retyped); nothing can abandon a half-sent request.
//! - An image's manifest note reaches only what these words carry, and [`crate::image_can_carry`]
//!   refuses the rest; its [`crate::Endowment`] names the stand-in row [`crate::IMAGE_ROW`].
//! - The shell reads an image's note twice and the progenitor refuses a misfit with
//!   [`SPAWN_REFUSED_BY_MANIFEST`]; only a plain line runs an image, never interruptibly.
//! - Every activation verb, [`Activation::Vouch`] included, is open to any spawn-endpoint holder,
//!   safe only while the owner's boot prompt is the one (§221).

/// The interruptible bit, packed into the high half of the page-count word so one `SEND` still
/// carries the whole request. `mem_pages` is a small count (`memory_grant_depleter`'s ceiling is
/// 64), so the low 32 bits hold it and this bit rides above.
const INTERRUPTIBLE_BIT: u64 = 1 << 32;

/// **A capability for the child's output slot follows** (milestone 50). Set by `>` and by every
/// stage of a `|` but the last: the shell delegates an endpoint and the progenitor puts it where
/// the result endpoint would have gone, so the child writes to a pipe or a file sink without
/// knowing which.
const SINK_BIT: u64 = 1 << 33;

/// **A capability for the child's input slot follows** (milestone 50). Set by `<` and by every
/// stage of a `|` but the first.
const SOURCE_BIT: u64 = 1 << 34;

/// **A capability for the child's declared second output follows** (DECISIONS §67). Set for a
/// program whose manifest declares one, whether or not the line has a `2>` on it: the stream exists
/// because the program says so, and the operator only names where it goes.
///
/// Unlike [`SINK_BIT`] this does **not** say which slot: the progenitor reads that from the
/// manifest, because the slot is the program's declaration and not the shell's choice. What the
/// wire says is only "expect one more capability", which is what keeps the two sides in lockstep.
const DIAG_BIT: u64 = 1 << 35;

/// **A capability for a narrowed tail stage's completion signal follows** (DECISIONS §106). Set
/// when the shell decided this stage both writes and reads, and the line named neither `>` nor `|`
/// for its output: its primary output defaults to `terminal_sink_caretaker`, and what follows is a
/// fresh endpoint the shell minted and kept a copy of. The progenitor installs it as the child's
/// fault target under DECISIONS §26 (the fault endpoint), so the shell `RECEIVE`s the child's exit
/// there as its completion signal instead of draining bytes it no longer sees.
const SCREEN_BIT: u64 = 1 << 37;

/// **A directory grant follows, and the progenitor is to build a caretaker for it** (milestone 31
/// phase 3).
///
/// The one bit here that announces data rather than a capability: two `SEND`s ([`GRANT_WORDS`]),
/// not a `SEND_CAPABILITY`. The shell's file-service endpoint carries no `GRANT`, so it cannot delegate
/// what a `fs_subtree_caretaker` needs; it says what the grant is, and the progenitor builds the
/// rest.
///
/// Name: ratified 2026-10-18 (calef: "Approve the family").
const DIRECTORY_GRANT_BIT: u64 = 1 << 36;

/// **A second directory grant follows, for the same confined program** (milestone 154,
/// design/roadmap/0154-multi-directory-namespace.md). Set only alongside [`DIRECTORY_GRANT_BIT`].
///
/// Another two [`GRANT_WORDS`] messages follow the first pair, carrying the second caretaker's
/// `START` words; the program's own are not repeated, since one program starts holding two
/// narrowed endpoints, as `fs_service::start_granted_two_dirs` does. **Nothing sets this bit yet
/// and the progenitor does not decode it**: no manifest declares two directory operands
/// (notes/two-trees.md's BUGS).
///
/// Name: ratified 2026-10-18 (calef: "Approve the family").
const SECOND_DIRECTORY_GRANT_BIT: u64 = 1 << 38;

/// **The executable's bytes follow, as page frames the caller owns** (DECISIONS §219 option D,
/// ruled by calef 2026-09-26). Set when the shell runs a program the image did not name by
/// [`Program`](crate::Program) id: an installed package's member, or a binary somebody just built.
///
/// Word 0 is then the image's length in bytes ([`image_len`]), and [`image_pages`] `SEND_CAPABILITY`s
/// follow, one `READ` frame each in page order, tagged with its index, after the directory grant's
/// data and before every other capability. The progenitor hashes its own copy. A miss runs
/// only with [`RUN_UNVOUCHED_BIT`] and [`crate::UNVOUCHED_MANIFEST`], and is otherwise refused with
/// [`SPAWN_UNVOUCHED`]. See notes/grant-expression/spawn-protocol.md.
///
/// Name: provisional (milestone 198 rung 3a, 2026-09-26), §219's own word for it.
const IMAGE_BIT: u64 = 1 << 39;

/// **Not a spawn: an edit to the installed-programs table** (milestone 198 (a package manager) rung
/// 3a's installer; DECISIONS §208 (installing a package is granting it, and the installed programs
/// are versioned)). Word 1 is an [`Activation`] verb, the rest of word 2 is zero, and the
/// progenitor answers with one message on the result endpoint, [`activation_reply`]'s two words.
///
/// What follows depends on the verb: an install's or a vouch's frames as [`IMAGE_BIT`]'s, with word
/// 0 the file's length; for a vouch, then the name as one data message; for a remove, that name
/// message alone; for a rollback, nothing, with names packed by
/// `filesystem_contract::grant::pack_name`. notes/grant-expression/spawn-protocol.md says why.
///
/// Name: provisional (2026-09-26).
const ACTIVATION_BIT: u64 = 1 << 40;

/// **The caller presents the run-unvouched capability: one `SEND` on it follows the delegation**
/// (DECISIONS §219 gate D2, ruled by calef 2026-09-26). Meaningful with [`IMAGE_BIT`]; the
/// progenitor honours it on any request, so a caller that sets it is never left blocked in its
/// `SEND`.
///
/// A message rather than a capability, because the capability lacks `GRANT` and the kernel will not
/// delegate it. It is the last thing a request sends, and the word is not read: that it arrived on
/// an endpoint only holders can reach is the whole of what it says. See
/// notes/grant-expression/spawn-protocol.md.
///
/// Name: provisional (milestone 198 rung 3a, 2026-09-26).
const RUN_UNVOUCHED_BIT: u64 = 1 << 41;

/// **The line's argv follows as one frame** (milestone 205 (how a foreign program is told what to
/// do), DECISIONS §170 (how a foreign program is told what to do)). The shell assembles the words
/// it was typed into one page in `program_argument_contract`'s layout and `SEND_CAPABILITY`s it narrowed
/// to `READ`, after the directory grant's data and any image's frames, and before every other
/// delegated capability.
///
/// The progenitor copies it, unread, into a page of the child's region placed at
/// `std_runtime_contract::ARGS_SLOT` and mapped read-only at `ARGS_PAGE`: the bytes carry no
/// authority. A program not on the `std` layout has the frame taken and dropped.
///
/// Name: ratified 2026-10-18 (calef: "Approve the family").
const PROGRAM_ARGUMENTS_BIT: u64 = 1 << 42;
/// **The machine statistics page follows as one `SEND_CAPABILITY`, after every other delegated
/// capability** (milestone 126 (the `procps` package), DECISIONS §225 (`free` sees the machine and
/// your share)). Set by a shell that holds the page at [`MACHINE_PAGE_SLOT`] when the program's
/// manifest declares `machine`; the progenitor maps it read-only into the child and places it at
/// `crate::MACHINE_SLOT`, then deletes its copy.
///
/// The page travels with the request because the progenitor's capability table has no slot to
/// spare for it; notes/grant-expression/spawn-protocol.md has the measurement.
///
/// Name: ratified 2026-09-27 (calef, #1360's table).
const MACHINE_BIT: u64 = 1 << 44;

/// **This is a graphical terminal launch** (milestone 632 (provisional), calef's 2026-09-30 ruling
/// that graphics is launched from the prompt rather than built at boot). No capability follows it:
/// the spawn service holds the gpu's four and a keyboard's three from boot and builds the session's
/// drivers from its own copies. A boot with no display answers [`SPAWN_NO_DISPLAY`].
///
/// Meaningful only for a `graphical_terminal` request; the progenitor refuses any other that sets
/// it.
///
/// Name: ratified 2026-10-03 (calef, #1493). Refused one device-bundle bit covering both (it adds a
/// second way of saying what a request carries).
const GRAPHICS_BIT: u64 = 1 << 45;

/// **Unused since milestone 715 (provisional), and a request that sets it is refused.** It said a
/// virtio keyboard's three capabilities followed [`GRAPHICS_BIT`]'s four; the spawn service now
/// holds those itself and decides the arm from what the boot granted (milestone 192 (a keyboard
/// on real silicon)'s option A, the UART, when there is no keyboard). Kept rather than deleted
/// because bit 46 and its name are a ratified wire position, and whether to retire or reuse it is
/// an architect's call, recorded as a BUGS entry in milestone 715's block.
///
/// Name: ratified 2026-10-03 (calef, #1493), with bit 46 and its meaning only with bit 45.
const KEYBOARD_BIT: u64 = 1 << 46;

/// **The shell holds this job's region and its exit** (milestone 933 (every job is built the way a
/// supervised job is), ruled 2026-10-27 UTC). Two capabilities lead the delegation, after the
/// interrupt pair and before the sink: a region the progenitor builds the job from, so the shell
/// can tear it down with `DESTROY`, and an endpoint the shell receives on, which becomes the job's
/// supervision endpoint (badged with the job's label) and, unless [`SINK_BIT`] or [`SCREEN_BIT`]
/// says otherwise, the job's output. The progenitor answers once, [`held_answer`], then sends the
/// job's `ThreadControlBlock` as the one message after a good answer (§309 (a supervised job's `ThreadControlBlock` goes to the shell that asked for it)). It retains nothing,
/// and learns that the job's file-service window is free from a [`release`] message instead of
/// `job_undertaker`'s [`reaped`] one. With [`SCREEN_BIT`] set, no further endpoint follows and the
/// job's output defaults to the terminal's sink.
///
/// Name: provisional (milestone 933's lane, 2026-10-27 UTC).
const HELD_BIT: u64 = 1 << 47;

/// **Not a spawn: the shell says a job it held is gone** (milestone 933). Word 0 is the job's label
/// from [`held_answer`], word 1 is zero, and the rest of word 2 is zero. The progenitor frees the
/// job's window, resolver grants and installer reply as it does for a [`reaped`] message, and
/// answers nothing. The shell sends it after its `DESTROY` has returned the region, before its next
/// request, so the progenitor takes it between requests and never mid-request.
///
/// Name: provisional (milestone 933's lane, 2026-10-27 UTC).
const RELEASE_BIT: u64 = 1 << 48;

/// **Where a session holds the machine statistics page** (milestone 126, DECISIONS §225): `READ |
/// GRANT`, so it can delegate it with [`MACHINE_BIT`] and not write it. A named slot probed at
/// `_start`, for [`RUN_UNVOUCHED_SLOT`]'s reasons.
///
/// Name: ratified 2026-09-27 (calef, #1360's table).
pub const MACHINE_PAGE_SLOT: u64 = 20;

/// **Empty and reserved**: the spawn service holds the display devices, and no builder places
/// anything here. Slots 22 to 28 stay a named block between [`crate::SHELL_CONFIGURATION_SLOT`]
/// (21) and [`RUN_UNVOUCHED_SLOT`] (62), fenced below, because a probed slot is sound only where
/// nothing allocates. The seven held the gpu's transport, interrupt and two page runs and a virtio
/// keyboard's three, with the rights the kernel granted the progenitor.
///
/// Name: ratified 2026-10-03 (calef, #1493), with slots 22 to 28 and the delegation order (after
/// the `--mem` untyped, before the machine statistics page).
pub const SHELL_GPU_SLOT: u64 = 22;

// The seven graphical slots are one contiguous block fenced by two named ones, and these two
// assertions are the fence: above the shell's configuration page, below the run-unvouched slot
// (and so below the kernel's fault slot, which each binary asserts against `abi` itself, for the
// reason `RUN_UNVOUCHED_SLOT`'s own doc gives). A slot outside the fence would be one a runtime
// allocation could land in, and a probe there would be sound only in a table that is almost full.
const _: () = assert!(SHELL_GPU_SLOT > MACHINE_PAGE_SLOT);
const _: () = assert!(SHELL_GPU_SLOT > crate::SHELL_CONFIGURATION_SLOT);
const _: () = assert!(SHELL_GPU_SLOT + 6 < RUN_UNVOUCHED_SLOT);

/// **Where the boot shell holds its second mounted tree** (milestone 154's mechanism, milestone 834
/// (ioping on nife and Linux)'s use): the disk tree's endpoint, `WRITE` and no `GRANT`, the
/// read-only and uncached view of the NVMe disk calef ruled for the prompt (2026-10-18 21:36 UTC,
/// "Yes, read-only at prompt"). Empty on a machine with no NVMe disk, which the shell's `_start`
/// probe tells apart. Below the graphical block, so `caps`'s census runs from the configuration
/// page (21) straight to 62.
///
/// The kernel maps its window 0 onto the first tree's window 0 frame, sound because the shell has
/// one `CALL` in flight. Only the boot shell is given it; a `login` session holds one tree.
///
/// Name: provisional (lane `lane/ioping-direct-io`, 2026-10-18 UTC).
pub const MOUNTED_TREE_SLOT: u64 = 19;

const _: () = assert!(MOUNTED_TREE_SLOT < MACHINE_PAGE_SLOT);
const _: () = assert!(MOUNTED_TREE_SLOT < crate::SHELL_CONFIGURATION_SLOT);

/// **The name the second mounted tree appears at in the shell's one tree**: `/disk`. One component,
/// so `/disk/nvme0` is a directory one level into the mounted tree, which is what a grant needs.
///
/// **The progenitor routes a grant by it** (milestone 834): a directory grant whose name is
/// `disk/<directory>` is built against the disk tree, as `<directory>`. The shell can never plan a
/// first-tree grant with that first component, because the mount hides the first tree's own
/// `/disk`, so the name says which tree without a wire bit of its own.
///
/// Name: provisional (lane `lane/ioping-direct-io`, 2026-10-18 UTC).
pub const MOUNTED_TREE_NAME: &[u8] = b"disk";

/// **The rights the second mounted tree's root carries**: list, walk, read. The disk tree answers
/// nothing that writes, so a shell that asked for more could not `cd` there (`OPENDIR` refuses a
/// request it cannot meet in full). `filesystem_contract::dir`'s `ENUMERATE | READ | DESCEND`,
/// spelled as a number because this crate does not link the contract; a host test pins the two.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub const MOUNTED_TREE_RIGHTS: u64 = 0b10_0011;

/// **Which tree a directory grant's name is in, and its name there** (milestone 834): the second
/// tree, as `rest`, for `disk/<rest>`; the first, unchanged, for anything else.
///
/// ```
/// use grant_plan::spawnproto::{in_mounted_tree, MOUNTED_TREE_NAME};
/// assert_eq!(in_mounted_tree(b"disk/nvme0"), Some(&b"nvme0"[..]));
/// assert_eq!(in_mounted_tree(b"diskette"), None);
/// assert_eq!(in_mounted_tree(MOUNTED_TREE_NAME), None);
/// ```
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
pub fn in_mounted_tree(name: &[u8]) -> Option<&[u8]> {
    name.strip_prefix(MOUNTED_TREE_NAME)?
        .strip_prefix(b"/")
        .filter(|rest| !rest.is_empty())
}

/// **The directory grant is a set of names, and the set follows as one frame** (milestone 205 (how
/// a foreign program is told what to do), §170 clauses 2 to 5). Meaningful with
/// [`Wiring::directory_grant`]: the progenitor builds `fs_nameset_caretaker` rather than
/// `fs_subtree_caretaker` for the grant, and maps it the set, `filesystem_contract::nameset`'s
/// encoding, read-only. The shell sends the frame `READ`-only after the argv's, before every other
/// delegated capability; the progenitor copies it, for [`IMAGE_BIT`]'s reason, into a page from the
/// job's region. A program that hears words gets exactly the names its words designated and no
/// sibling; nothing sends it for `rm *.txt` yet.
///
/// Name: provisional (milestone 205, 2026-09-27).
const NAMESET_BIT: u64 = 1 << 43;

/// **Where a session holds the run-unvouched capability** (DECISIONS §219 gate D2): the slot the
/// progenitor places it in, `WRITE` only, in the boot shell and in `login`, and the slot `login`
/// delegates it from.
///
/// The highest slot below the kernel's reserved fault slot (`abi::fault::FAULT_EP_SLOT`, 63);
/// `grant_plan` does not depend on `abi`, so each binary that reads this asserts the relation. A
/// named slot, as [`crate::NETWORK_SLOT`] is: the holder probes it at `_start`, before it has
/// allocated anything, which is the only time a probe is sound.
///
/// Name: provisional.
pub const RUN_UNVOUCHED_SLOT: u64 = 62;

/// **What an activation request asks for** (see `ACTIVATION_BIT`). Provisional names, like the
/// bit's; the prompt spells them `jig install`, `jig remove`, `jig rollback` and `vouch`.
///
/// **Number 4 is a hole**: it was `Fetch`, the progenitor fetching a package by name, retired by
/// milestone 809 (the package client becomes a program) when `jig` took the fetch and the
/// progenitor stopped parsing network input. A request carrying 4 is answered
/// [`ActivationStatus::Unknown`], as any verb this side does not know is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Activation {
    /// Install every program of the package whose bytes follow (milestone 919 (a package carries
    /// several programs)): place each program's bytes where a person can run them, and record
    /// each one's digest in one new generation, which is made live. One refused program refuses
    /// the package, and nothing is written.
    Install = 1,
    /// Write a new generation without the named package and make it live. Its bytes stay on
    /// disk, which is what lets a rollback bring it back. **The name is a package's** (calef,
    /// 2026-10-14 (UTC), milestone 919: "jig remove should work on packages, not programs."): a
    /// bare name removes every row the package installed, at every version, with the default
    /// pointers of its programs; `package@version` removes that version's rows.
    Remove = 2,
    /// Make the generation numbered one below the live one live again. Nothing is rewritten.
    Rollback = 3,
    /// **The owner vouches for these bytes** (DECISIONS §221 (the boot prompt is the owner's
    /// console), ruling 1, and §195 (a reviewed recipe vouches for a package) clause 3: the owner
    /// may vouch for a digest no source carries). The progenitor hashes its own copy and writes a
    /// new generation whose entry for the name is that digest, marked `installed_programs::OWNER`,
    /// so `package rollback` undoes it. The bytes are not copied, and must parse as an executable.
    ///
    /// Only a spawn-endpoint holder may ask, which is the boot prompt and so the owner. The
    /// installer endpoint refuses it ([`INSTALLER_BADGE`]).
    ///
    /// Name: provisional (milestone 198, lane `milestone/198-owner-console`, 2026-09-26).
    Vouch = 5,
    /// **The owner's approval file, compiled and stored** (milestone 911 (scripted installs grant
    /// only what the owner's approval file allows); `jig approve <file>`). The frames are the
    /// compiled records (`installed_programs::approval`), whose header carries the approval's
    /// name and the file's digest the records bind to; the progenitor stores them beside the
    /// installed-programs table's generations, never as rows in them, and answers
    /// [`ActivationStatus::Done`]. Only an **undelegated** installer badge may send it: a badge
    /// that carries an approval is answered [`ActivationStatus::NotServedHere`], so no holder can
    /// widen its own approval or mint another.
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    Approve = 6,
    /// **The named approval is dropped** (milestone 911; `jig revoke <name>`). The name follows as
    /// one data message, packed as a [`Activation::Remove`]'s is. The progenitor deletes the
    /// store's file; every later request under a badge that carries the approval is answered
    /// [`ActivationStatus::Revoked`]. Only an undelegated installer badge may send it.
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    Revoke = 7,
}

impl Activation {
    fn from_word(w: u64) -> Option<Self> {
        match w {
            1 => Some(Self::Install),
            2 => Some(Self::Remove),
            3 => Some(Self::Rollback),
            5 => Some(Self::Vouch),
            6 => Some(Self::Approve),
            7 => Some(Self::Revoke),
            _ => None,
        }
    }
}

/// Build an activation request. `len` is the data length for [`Activation::Install`],
/// [`Activation::Vouch`] and [`Activation::Approve`], whose frames follow, and ignored otherwise.
pub fn activation_request(verb: Activation, len: u64) -> (u64, u64, u64) {
    let w0 = if matches!(
        verb,
        Activation::Install | Activation::Vouch | Activation::Approve
    ) {
        len
    } else {
        0
    };
    (w0, verb as u64, ACTIVATION_BIT)
}

/// **The verb, if this request is an activation request at all.** The progenitor asks this first,
/// before [`wiring`], because under `ACTIVATION_BIT` no other bit of word 2 means anything. A set
/// bit with a verb this side does not know is `Some(None)`: the caller answers
/// [`ActivationStatus::Unknown`] and reads nothing more.
pub fn activation(w1: u64, w2: u64) -> Option<Option<Activation>> {
    (w2 & ACTIVATION_BIT != 0).then(|| Activation::from_word(w1))
}

/// **How an activation request came out**: word 0 of the progenitor's one reply. Word 1 is the
/// generation that is live afterwards, whatever the status, so a refusal still says what is in
/// force. Provisional, like the verbs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActivationStatus {
    /// Done: word 1 is the generation now live.
    Done = 0,
    /// The image's catalogue does not vouch for these bytes (DECISIONS §195 (a reviewed recipe
    /// vouches for a package)), or they are not a package at all. Nothing was written.
    NotCataloged = 1,
    /// A vouched package with no program the installer can find. Nothing was written.
    NoProgram = 2,
    /// [`Activation::Remove`] named a package the live generation has no row of.
    NotInstalled = 3,
    /// [`Activation::Rollback`] from a generation with none below it.
    NoEarlier = 4,
    /// The file service refused a write, or this boot has none. What was written before the
    /// failure is described in `notes/packages.md`: never a `current` naming a generation that
    /// was not written whole.
    StoreFailed = 5,
    /// A verb this progenitor does not know, or a package larger than [`IMAGE_MAXIMUM_PAGES`].
    Unknown = 6,
    // 7, 8 and 9 were `NoSuchPackage`, `NoNetwork` and `FetchFailed`, the progenitor's fetch's
    // answers, retired with it by milestone 809. A fetch's failures are `jig`'s to say now.
    /// [`Activation::Vouch`] was sent bytes that do not parse as an executable, or a name the
    /// installed-programs table cannot record. Nothing was written.
    NotExecutable = 10,
    /// [`Activation::Install`] for a package one of whose programs has the name another
    /// installed package's program already has (`installed_programs::Error::Taken`; DECISIONS §229
    /// (how a bare name at the prompt reaches an installed program), B2; calef, 2026-10-14 (UTC),
    /// milestone 919: "If two packages offer the same program name, refuse the second package.
    /// Need to remove the first one before installing the second one."). The same package at
    /// another version is an upgrade and is not this. No generation was written. Word 2 names the
    /// program ([`activation_reply`]), and on an installer endpoint two messages follow naming
    /// the package that holds it ([`holder_words`]). Provisional.
    NameTaken = 11,
    /// [`Activation::Install`] for a package one of whose programs has the name of a program
    /// the image carries (`installed_programs::Error::ImageName`; DECISIONS §229, calef's ruling of
    /// 2026-09-27). Under §235 (the OS is built and updated from packages) a base program is
    /// updated through the boot slot, never by install. No generation was written. Provisional,
    /// like its number.
    ImageName = 12,
    /// [`Activation::Remove`] named `package@version`, the version holds one of its programs'
    /// default pointers, and more than one other version of that program remains: no ordering
    /// among live versions exists to pick a new default with (milestone 614 (two installed
    /// versions of one program, each runnable, and a caller granted the one it needs)). Nothing
    /// was written; the shell names the candidates from the live table, which it reads itself.
    /// Provisional, like its number.
    Ambiguous = 13,
    /// **The request arrived on the installer endpoint and is not one it serves** (milestone 809,
    /// [`INSTALLER_BADGE`]): a spawn, a vouch, or a verb this side does not know. Nothing was read
    /// past the first message and nothing was written. Provisional.
    NotServedHere = 14,
    /// **A removal or a rollback would undo a row another package manager installed**
    /// (`installed_programs::can_edit`; calef's ruling of 2026-10-06 on milestone 809: a manager
    /// "refuses to remove or roll back a row it did not install"). Nothing was written.
    /// Provisional.
    NotYours = 15,
    /// **The approval does not allow this** (milestone 911 (scripted installs grant only what the
    /// owner's approval file allows), the check in the installer): the package is outside the
    /// badge's approval, its asks exceed its entry, the entry's `remove` answer refuses a
    /// removal, or a rollback would restore a row outside it. Nothing was read past the frames
    /// and nothing was written. One message follows naming the package ([`holder_words`]).
    /// Provisional.
    NotApproved = 16,
    /// **The badge's approval is not stored** (milestone 911): it was revoked by
    /// [`Activation::Revoke`], or it was never approved. Nothing is read and nothing is written;
    /// the holder may do nothing under it until `jig approve` stores the name again. Provisional.
    Revoked = 17,
}

impl ActivationStatus {
    /// The status a reply's word 0 carries, or [`ActivationStatus::Unknown`] for a word that is
    /// none of them.
    pub fn from_word(w: u64) -> Self {
        match w {
            0 => Self::Done,
            1 => Self::NotCataloged,
            2 => Self::NoProgram,
            3 => Self::NotInstalled,
            4 => Self::NoEarlier,
            5 => Self::StoreFailed,
            10 => Self::NotExecutable,
            11 => Self::NameTaken,
            12 => Self::ImageName,
            13 => Self::Ambiguous,
            14 => Self::NotServedHere,
            15 => Self::NotYours,
            16 => Self::NotApproved,
            17 => Self::Revoked,
            _ => Self::Unknown,
        }
    }
}

/// The progenitor's reply to an activation request: the status, the generation live afterwards
/// (0 when there is none), and `member`: for [`ActivationStatus::NameTaken`] and
/// [`ActivationStatus::ImageName`], one more than the table-of-contents index of the member whose
/// program was refused, and 0 otherwise. The installer that sent the package names the program
/// from its own copy (`package_archive::program_name`), so the name does not travel (milestone
/// 919 (a package carries several programs)).
pub fn activation_reply(status: ActivationStatus, live: u32, member: u64) -> (u64, u64, u64) {
    (status as u64, u64::from(live), member)
}

/// **The longest holder name a [`ActivationStatus::NameTaken`] reply carries**, in bytes: the
/// width of a package header's name field (`package_archive::NAME_LEN`). A longer name, which only
/// a hand-written table holds, arrives cut at this width. Name: provisional, milestone 919's lane.
///
/// Name: ratified 2026-10-27 (#2797).
pub const HOLDER_MAXIMUM: usize = 32;

/// **The two messages that follow a [`ActivationStatus::NameTaken`] reply on an installer
/// endpoint** (milestone 919): the name of the package that already holds the program, so the
/// owner is told what to remove first. Four little-endian words of name and its length. The spawn
/// endpoint sends no install, so only a package manager reads these. Name: provisional.
pub fn holder_words(holder: &str) -> [(u64, u64, u64); 2] {
    let mut bytes = [0u8; HOLDER_MAXIMUM];
    let len = holder.len().min(HOLDER_MAXIMUM);
    bytes[..len].copy_from_slice(&holder.as_bytes()[..len]);
    let word = |i: usize| {
        let mut w = [0u8; 8];
        w.copy_from_slice(&bytes[i * 8..i * 8 + 8]);
        u64::from_le_bytes(w)
    };
    [(word(0), word(1), len as u64), (word(2), word(3), 0)]
}

/// **The trailer an install or a removal sends after its data**, the answer beside the record
/// (milestone 911 (scripted installs grant only what the owner's approval file allows), item 1):
/// how the answer was given. One message for the console's yes, three for an approval, whose
/// name and file digest the progenitor alone cannot know. The sender always sends it; the
/// progenitor reads it for every install and removal and writes the generation's `answered`
/// line from it, from the badge's approval instead when the request arrived on a delegated
/// endpoint.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub const ANSWERED_CONSOLE: u64 = 0;
/// The trailer's kind word for an approval's answer ([`answered_console`]).
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub const ANSWERED_APPROVAL: u64 = 1;

/// The console's one trailer message: the answer was the owner's yes at the prompt, or the
/// owner's own `--approved` file whose check `jig` ran (the console holds the authority either
/// way). A delegated request overrides it with the badge's approval.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[must_use]
pub const fn answered_console() -> (u64, u64, u64) {
    (ANSWERED_CONSOLE, 0, 0)
}

/// The approval's three trailer messages: the kind, the approval's name packed as a removal's
/// name is ([`filesystem_contract::grant::pack_name`], sixteen bytes at most), then the file's
/// digest the records bind to, thirty-two raw bytes over two messages. The name is the approval
/// file's own `approval` key; a longer name is refused at compile time, before this is built.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[must_use]
pub fn answered_approval(name: &str, digest: &[u8; 32]) -> [(u64, u64, u64); 3] {
    let (lo, hi) = pack_name(name.as_bytes());
    let word = |at: usize| {
        let mut w = [0u8; 8];
        w.copy_from_slice(&digest[at..at + 8]);
        u64::from_le_bytes(w)
    };
    [
        (ANSWERED_APPROVAL, lo, hi),
        (word(0), word(8), word(16)),
        (word(24), 0, 0),
    ]
}

/// The name packed the way a removal's name is: sixteen bytes, little-endian words, zero-padded
/// (`filesystem_contract::grant::pack_name`'s rule, spelled here because this crate does not link
/// the contract; the test pins the two).
fn pack_name(name: &[u8]) -> (u64, u64) {
    let mut bytes = [0u8; 16];
    let len = name.len().min(16);
    bytes[..len].copy_from_slice(&name[..len]);
    (
        u64::from_le_bytes(bytes[..8].try_into().unwrap_or([0; 8])),
        u64::from_le_bytes(bytes[8..].try_into().unwrap_or([0; 8])),
    )
}

/// [`pack_name`] read back: the length the words carry, or 0 for words that spell nothing.
fn unpack_name(lo: u64, hi: u64, out: &mut [u8; 16]) -> usize {
    let words = [lo.to_le_bytes(), hi.to_le_bytes()];
    let mut packed = [0u8; 16];
    packed[..8].copy_from_slice(&words[0]);
    packed[8..].copy_from_slice(&words[1]);
    let len = packed
        .iter()
        .rposition(|&b| b != 0)
        .map_or(0, |at| at + 1)
        .min(16);
    out[..len].copy_from_slice(&packed[..len]);
    len
}

/// [`answered_approval`] and [`answered_console`] read back: how the answer was given, or
/// `None` for words that spell neither kind. `out` holds the unpacked name.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub fn answered_from_trailer(
    (kind, lo, hi): (u64, u64, u64),
    more: &[(u64, u64, u64)],
    out: &mut [u8; 16],
) -> Option<Answered> {
    match kind {
        ANSWERED_CONSOLE if lo == 0 && hi == 0 => Some(Answered::Console),
        ANSWERED_APPROVAL if more.len() >= 2 => {
            let len = unpack_name(lo, hi, out);
            let mut digest = [0u8; 32];
            for (i, (a, b, c)) in more[..2].iter().enumerate() {
                for (at, w) in [a, b, c].into_iter().enumerate() {
                    let byte = i * 24 + at * 8;
                    if byte < 32 {
                        let take = 8.min(32 - byte);
                        digest[byte..byte + take].copy_from_slice(&w.to_le_bytes()[..take]);
                    }
                }
            }
            Some(Answered::Approval {
                name: *out,
                len,
                digest,
            })
        }
        _ => None,
    }
}

/// **How a trailer said the answer was given**, as the generation's `answered` line spells it
/// (`installed_programs::Answered`). The progenitor's reading of [`answered_from_trailer`].
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answered {
    /// The owner's console.
    Console,
    /// The approval named, bound to the file's digest.
    Approval {
        /// The name, as `out` was given room for it.
        name: [u8; 16],
        /// The name's length, at most sixteen bytes.
        len: usize,
        /// The file's digest the records bind to.
        digest: [u8; 32],
    },
}

/// [`holder_words`] read back into `out`: the holder's name, or `""` for words that do not spell
/// one.
pub fn holder_from_words(words: [(u64, u64, u64); 2], out: &mut [u8; HOLDER_MAXIMUM]) -> &str {
    let [(a, b, len), (c, d, _)] = words;
    for (i, w) in [a, b, c, d].into_iter().enumerate() {
        out[i * 8..i * 8 + 8].copy_from_slice(&w.to_le_bytes());
    }
    let len = usize::try_from(len).map_or(HOLDER_MAXIMUM, |l| l.min(HOLDER_MAXIMUM));
    core::str::from_utf8(&out[..len]).unwrap_or("")
}

/// **The largest image `IMAGE_BIT` may carry, in pages** (4 MiB). A ceiling both sides read, so the
/// shell refuses a larger file before it sends anything and the progenitor never stages more than
/// its image pool holds. `ripgrep` 14.1.1's image spans about 670 (`notes/ripgrep-on-nife.md`).
///
/// Reserved, not spent: `IMAGE_MAXIMUM_PAGES * 3 + grant_plan::STD_REGION_PAGES` pages, about 13.5
/// MiB, are set aside at boot for the shell's staging frames, the progenitor's copy, the child's
/// pages and a `std` heap (`IMAGE_POOL_PAGES` in `crates/system_initializer`). Provisional.
///
/// Name: ratified 2026-10-27 (#2797).
pub const IMAGE_MAXIMUM_PAGES: u64 = 1024;

/// **The budget the progenitor hands the boot shell**, in pages, which both read: the progenitor
/// splits it, and the shell prints it in `caps` because there is no call that says how much is
/// left.
///
/// 128 pages for `--mem` grants, pipes and the shell's own windows, plus one image's staging frames
/// ([`IMAGE_MAXIMUM_PAGES`]), since the shell reads a file run by its path into frames split from
/// this budget before it sends them, plus the regions of the jobs it holds
/// ([`crate::JOB_POOL_PAGES`], milestone 933 (every job is built the way a supervised job is)):
/// the same mix the progenitor's pool holds, now the shell's to split.
///
/// Name: provisional.
pub const SHELL_BUDGET_PAGES: u64 = 128 + IMAGE_MAXIMUM_PAGES + crate::JOB_POOL_PAGES;

/// The page size an image is carried in. A frame is one page on every architecture this tree
/// builds for.
pub const IMAGE_PAGE: u64 = 4096;

/// **The two messages after a [`Wiring::directory_grant`] request**, in order, three words each:
///
/// 1. **the caretaker's `START` words**, passed to `fs_subtree_caretaker` verbatim: the granted
///    directory's name and the `filesystem_contract::dir` rights the subtree capability carries;
/// 2. **the confined program's `START` words**, passed to the program verbatim: for `rm`, the
///    operand's name and the options that were typed.
///
/// Opaque here: they are `filesystem_contract::grant`'s packing, which this crate does not link.
/// Two messages because the two processes start with different names; six words do not fit in
/// three.
pub const GRANT_WORDS: usize = 2;

/// **Where the shell's operators end up on the wire**, alongside the parts of the endowment that
/// were always here. `sink` and `source` are booleans rather than capabilities because the
/// capability travels separately, over `SEND_CAPABILITY`: this word only says whether to expect it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Wiring {
    /// A foreground job the shell will supervise (DECISIONS §24). Two capabilities lead the
    /// delegation: a job untyped the child is built from so the shell can tear it down, then a
    /// shared job frame.
    pub interruptible: bool,
    /// The child's output slot is substituted (`>` or the left of a `|`).
    pub sink: bool,
    /// The child's input slot is filled (`<` or the right of a `|`).
    pub source: bool,
    /// The child declares a second output stream, so one more endpoint follows (DECISIONS §67).
    pub diagnostics: bool,
    /// **A directory grant follows as two data messages** ([`GRANT_WORDS`]), and the progenitor is
    /// to build a `fs_subtree_caretaker` for it before it builds the child. The only entry here
    /// that announces data instead of a capability; see `DIRECTORY_GRANT_BIT`.
    pub directory_grant: bool,
    /// **A second directory grant follows, as two more data messages** (milestone 154). See
    /// `SECOND_DIRECTORY_GRANT_BIT`. Meaningless unless `directory_grant` is also set, which
    /// nothing here enforces: a caller that set it alone sent a request wrong before it left.
    pub second_directory_grant: bool,
    /// **This stage's primary output defaults to `terminal_sink_caretaker`, and a fresh completion
    /// endpoint follows** (DECISIONS §106). See `SCREEN_BIT`. Mutually exclusive with `sink` in
    /// practice (a stage the shell delegated an explicit sink for has somewhere else to write), but
    /// nothing here enforces that; the two ride independent bits because both sides read one word.
    pub screen: bool,
    /// **The executable's bytes follow as frames, and word 0 is their length** (DECISIONS §219
    /// option D). See `IMAGE_BIT`.
    pub image: bool,
    /// **One `SEND` on the run-unvouched capability follows the delegation** (DECISIONS §219 gate
    /// D2). See `RUN_UNVOUCHED_BIT`.
    pub run_unvouched: bool,
    /// **The line's argv follows as one `READ` frame** (DECISIONS §170). See
    /// `PROGRAM_ARGUMENTS_BIT`.
    pub program_arguments: bool,
    /// **The directory grant is a set of names, which follows as one `READ` frame** (milestone
    /// 205). See [`NAMESET_BIT`].
    pub nameset: bool,
    /// **The machine statistics page follows as the last delegated capability** (milestone 126).
    /// See [`MACHINE_BIT`].
    pub machine: bool,
    /// **This is a graphical terminal launch, and no capability follows** (milestone 632
    /// (provisional)): the spawn service builds the session from its own copies. See
    /// [`GRAPHICS_BIT`].
    pub graphics: bool,
    /// **Unused, and a request that sets it is refused; no capability follows** (milestone 715
    /// (provisional)). See [`KEYBOARD_BIT`]; meaningless unless `graphics` is also set.
    pub keyboard: bool,
    /// **The shell holds this job's region and its exit** (milestone 933). Two capabilities follow
    /// the interrupt pair; see `HELD_BIT`.
    pub held: bool,
}

/// Build the three request words from a resolved endowment's parts.
pub fn request(prog_id: u64, arg: u64, mem_pages: u64, w: Wiring) -> (u64, u64, u64) {
    let mut w2 = mem_pages & 0xffff_ffff;
    if w.interruptible {
        w2 |= INTERRUPTIBLE_BIT;
    }
    if w.sink {
        w2 |= SINK_BIT;
    }
    if w.source {
        w2 |= SOURCE_BIT;
    }
    if w.diagnostics {
        w2 |= DIAG_BIT;
    }
    if w.directory_grant {
        w2 |= DIRECTORY_GRANT_BIT;
    }
    if w.second_directory_grant {
        w2 |= SECOND_DIRECTORY_GRANT_BIT;
    }
    if w.screen {
        w2 |= SCREEN_BIT;
    }
    if w.image {
        w2 |= IMAGE_BIT;
    }
    if w.run_unvouched {
        w2 |= RUN_UNVOUCHED_BIT;
    }
    if w.program_arguments {
        w2 |= PROGRAM_ARGUMENTS_BIT;
    }
    if w.nameset {
        w2 |= NAMESET_BIT;
    }
    if w.machine {
        w2 |= MACHINE_BIT;
    }
    if w.graphics {
        w2 |= GRAPHICS_BIT;
    }
    if w.keyboard {
        w2 |= KEYBOARD_BIT;
    }
    if w.held {
        w2 |= HELD_BIT;
    }
    (prog_id, arg, w2)
}

/// The whole wiring of a received request (word 2), so the progenitor reads it once rather than
/// asking three separate questions of the same word.
pub fn wiring(w2: u64) -> Wiring {
    Wiring {
        interruptible: is_interruptible(w2),
        sink: w2 & SINK_BIT != 0,
        source: w2 & SOURCE_BIT != 0,
        diagnostics: w2 & DIAG_BIT != 0,
        directory_grant: w2 & DIRECTORY_GRANT_BIT != 0,
        second_directory_grant: w2 & SECOND_DIRECTORY_GRANT_BIT != 0,
        screen: w2 & SCREEN_BIT != 0,
        image: w2 & IMAGE_BIT != 0,
        run_unvouched: w2 & RUN_UNVOUCHED_BIT != 0,
        program_arguments: w2 & PROGRAM_ARGUMENTS_BIT != 0,
        nameset: w2 & NAMESET_BIT != 0,
        machine: w2 & MACHINE_BIT != 0,
        graphics: w2 & GRAPHICS_BIT != 0,
        keyboard: w2 & KEYBOARD_BIT != 0,
        held: w2 & HELD_BIT != 0,
    }
}

/// **Whether a held job is also sent a view of the shell's budget** (milestone 933): when its
/// program declares [`Manifest::share`](crate::Manifest::share), the third capability of the
/// held group is the shell's budget narrowed to `ENUMERATE`, which becomes the job's share view
/// (`free`'s `Yours:`) in place of the progenitor's pool. Both sides ask this one function, so the
/// shell never sends a capability the progenitor does not read.
///
/// Name: provisional, minted 2026-10-27 (UTC) by lane milestone/933-every-job-built-like-a-supervised-job.
#[must_use]
pub const fn held_shares(program: crate::Program) -> bool {
    program.manifest().share
}

/// **The "release" request for job `label`** ([`RELEASE_BIT`]): the three words the shell sends
/// when it has torn down a job it held.
///
/// Name: provisional, minted 2026-10-27 (UTC) by lane milestone/933-every-job-built-like-a-supervised-job.
#[must_use]
pub const fn release(label: u64) -> (u64, u64, u64) {
    (label, 0, RELEASE_BIT)
}

/// **The label a release request names, if this first message is one.** The progenitor asks it
/// before [`activation`] and [`wiring`], because under the bit no other bit of word 2 means
/// anything.
///
/// Name: provisional, minted 2026-10-27 (UTC) by lane milestone/933-every-job-built-like-a-supervised-job.
#[must_use]
pub const fn released_label(w0: u64, w2: u64) -> Option<u64> {
    if w2 & RELEASE_BIT != 0 {
        Some(w0)
    } else {
        None
    }
}

/// **The progenitor's one answer to a held job**: `(SPAWN_OK, label, 0)` when the job started, and
/// `(failure, 0, 0)` with one of the sentinels otherwise (milestone 933). The label is the number
/// on the job's supervision capability; it comes back on the job's death message in the sixth
/// word, and the shell names the job to [`release`] by it.
///
/// Name: provisional, minted 2026-10-27 (UTC) by lane milestone/933-every-job-built-like-a-supervised-job.
#[must_use]
pub const fn held_answer(label: u64) -> (u64, u64, u64) {
    (SPAWN_OK, label, 0)
}

/// The image's length in bytes from a received request (word 0), when [`Wiring::image`] is set.
/// The same word [`prog_id`] reads otherwise; which one it is depends on word 2 alone.
pub fn image_len(w0: u64) -> u64 {
    w0
}

/// How many frames an image of `len` bytes travels in: one per started page. `0` for an empty
/// image, which the progenitor refuses rather than builds.
pub fn image_pages(len: u64) -> u64 {
    len.div_ceil(IMAGE_PAGE)
}

/// The program id from a received request (word 0).
pub fn prog_id(w0: u64) -> u64 {
    w0
}

/// The integer argument from a received request (word 1).
///
/// Name: ratified 2026-10-18 (calef: "Approve the family").
pub fn program_argument(w1: u64) -> u64 {
    w1
}

/// The memory-grant page count from a received request (word 2). Non-zero means one delegated
/// untyped capability follows the interrupt caps (if any) over `SEND_CAPABILITY` / `RECEIVE_CAPABILITY`.
///
/// Name: ratified 2026-10-18 (calef: "Approve the family").
pub fn memory_grant_pages(w2: u64) -> u64 {
    w2 & 0xffff_ffff
}

/// Whether this is a supervised foreground job (word 2's high bit). When set, the delegation leads
/// with two caps: a job untyped (the progenitor builds the child from it; the shell keeps it to
/// `DESTROY`) and a shared job frame (the cooperative interrupt flag and the child's status).
pub fn is_interruptible(w2: u64) -> bool {
    w2 & INTERRUPTIBLE_BIT != 0
}

/// The data word carried alongside the delegated untyped in the `SEND_CAPABILITY`. It is not load-bearing
/// (the progenitor identifies the cap by the protocol position, not the tag), but a fixed marker
/// makes a misrouted message obvious in a trace. Its low bits echo the page count as a cheap
/// cross-check.
///
/// Name: provisional, minted 2026-10-27 (UTC) by lane capability-sweep-1. Spelled out from
/// `CAPABILITY_TAG` under the capability ruling of 2026-10-06 (UTC).
pub const CAPABILITY_TAG: u64 = 0x6361_705f; // "cap_" little-endian-ish marker

/// The sentinel the progenitor sends on the result endpoint when it could not build the child, so
/// the shell's single read completes with a legible failure rather than blocking forever. Distinct
/// from any answer a real program would report (no phase-1 program returns `u64::MAX`).
pub const SPAWN_FAILED: u64 = u64::MAX;

/// **The word for a job the kernel killed** (milestone 235,
/// design/roadmap/0235-a-faulted-job-should-reach-the-prompt.md). Sent on the result endpoint by
/// `job_undertaker`, which already holds the progenitor's supervision endpoint, once it has
/// collected the corpse. A child the kernel killed sends nothing, so without this the shell's
/// single read would never complete.
///
/// Distinct from [`SPAWN_FAILED`]: nothing ran, versus something ran and died. One below
/// `u64::MAX`; no program answers with either. See notes/grant-expression/spawn-protocol.md.
///
/// Name: provisional, like everything a lane mints: a word in a protocol is exactly the kind of
/// name an architect decides.
pub const JOB_FAULTED: u64 = u64::MAX - 1;

/// **The badge on `job_undertaker`'s copy of the spawn endpoint** (milestone 685 (a job is finished
/// when its memory is back), option A, ruled by calef 2026-10-06 UTC: "A looks right."). A message
/// on the spawn endpoint carrying this badge is a [`reaped`] message, never a request. Only the
/// copy the progenitor minted can carry it, since the shell's is unbadged `WRITE` and `BADGE`
/// needs `GRANT`; the kernel delivers the badge on every receive, so each one is checked.
///
/// Three words of a plain `SEND`, sent after the collect, so the region is already back: `w0` the
/// label the progenitor put on the job's supervision capability (0 for none), which it keys on;
/// `w1` the dead thread's id; `w2` zero. See notes/grant-expression/spawn-protocol.md. Name:
/// provisional.
pub const UNDERTAKER_BADGE: u64 = 1;

/// **The "reaped" message for job `label`, thread `tid`** ([`UNDERTAKER_BADGE`] has the layout).
#[must_use]
pub const fn reaped(label: u64, tid: u64) -> (u64, u64, u64) {
    (label, tid, 0)
}

/// **The bit that marks a badge as an installer endpoint's** (milestone 809 (the package client
/// becomes a program), DECISIONS §270 (a package manager holds an installer endpoint, not the spawn
/// endpoint)). The progenitor badges a `WRITE` copy of the spawn endpoint [`installer_badge`] of
/// the job's label for a program declaring `Manifest::installer`, at `crate::INSTALLER_SLOT`, which
/// serves install, remove and rollback and answers [`ActivationStatus::NotServedHere`] otherwise.
///
/// Since milestone 911 (scripted installs grant only what the owner's approval file allows) a
/// **delegated** endpoint's badge also carries the approval the holder acts within
/// ([`DELEGATED_APPROVAL_SHIFT`]): the same endpoint object, badged for one job and one approval,
/// not a second one. Labels count from 1 and stay below 2³² ([`installer_badge`]'s doc), the
/// approval's id rides above them, and [`UNDERTAKER_BADGE`] is `1` without the bit, so no two
/// kinds of sender share a badge. A holder that promises frames and never sends them holds the
/// progenitor until its reaped message ends the wait, or for good if it never dies.
///
/// Name: provisional (milestone 809).
pub const INSTALLER_BADGE: u64 = 1 << 62;

/// **Where a delegated installer badge's approval id sits** (milestone 911): bit 32 up, thirty
/// bits of per-boot ids under [`INSTALLER_BADGE`]'s bit. A badge carries one approval or none;
/// the id is the progenitor's key for the name, and the store's file, read per request, is the
/// authority. The kernel writes the badge (§230 (badged endpoint capabilities)) and `BADGE` needs
/// `GRANT`, which a holder lacks, so no holder can forge or widen it.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub const DELEGATED_APPROVAL_SHIFT: u64 = 32;

/// The installer endpoint's badge for the job labeled `label` ([`INSTALLER_BADGE`]): the plain,
/// undelegated endpoint, whose requests are the owner's own.
#[must_use]
pub const fn installer_badge(label: u64) -> u64 {
    INSTALLER_BADGE | label
}

/// **The delegated installer endpoint's badge** (milestone 911): [`installer_badge`] of `label`,
/// and the approval id `approval` above [`DELEGATED_APPROVAL_SHIFT`]. A label at 2³² or an id at
/// 2³⁰ would collide, so a caller that minted one has already refused it; the progenitor's label
/// counts jobs from 1 and its ids count approvals from 1.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[must_use]
pub const fn delegated_installer_badge(label: u64, approval: u64) -> u64 {
    INSTALLER_BADGE | (approval << DELEGATED_APPROVAL_SHIFT) | label
}

/// **The job label an installer endpoint's badge carries**, or `None` for a badge that is not one
/// ([`INSTALLER_BADGE`]): the label below [`DELEGATED_APPROVAL_SHIFT`], whatever approval the
/// badge also carries.
#[must_use]
pub const fn installer_label(badge: u64) -> Option<u64> {
    if badge & INSTALLER_BADGE != 0 {
        Some(badge & !(INSTALLER_BADGE | (0x3fff_ffff << DELEGATED_APPROVAL_SHIFT)))
    } else {
        None
    }
}

/// **The approval id a delegated installer badge carries**, or `None` for the plain, undelegated
/// endpoint ([`delegated_installer_badge`]).
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[must_use]
pub const fn installer_approval(badge: u64) -> Option<u64> {
    let id = (badge & (0x3fff_ffff << DELEGATED_APPROVAL_SHIFT)) >> DELEGATED_APPROVAL_SHIFT;
    match (badge & INSTALLER_BADGE != 0, id) {
        (true, 1..) => Some(id),
        _ => None,
    }
}

/// **Whether the installer endpoint serves this verb** ([`INSTALLER_BADGE`]): install, remove,
/// rollback, and the owner's approve and revoke; never a vouch.
#[must_use]
pub const fn installer_serves(verb: Activation) -> bool {
    matches!(
        verb,
        Activation::Install
            | Activation::Remove
            | Activation::Rollback
            | Activation::Approve
            | Activation::Revoke
    )
}

/// **The word for bytes nobody vouched for** (DECISIONS §219, milestone 198 rung 3a). Sent on the
/// result endpoint when an `IMAGE_BIT` request's digest is not in the installed-programs table, in
/// place of [`SPAWN_FAILED`]: "nothing was built" is out of memory, and "refused as unvouched" is a
/// decision. A caller presenting [`Wiring::run_unvouched`] has the miss built instead, with
/// [`crate::UNVOUCHED_MANIFEST`]: only what it delegated, and the clock and configuration pages.
///
/// Two below `u64::MAX`, for [`JOB_FAULTED`]'s reason one below it. Name: provisional.
pub const SPAWN_UNVOUCHED: u64 = u64::MAX - 2;

/// **The progenitor's refusal of bytes whose own manifest forbids the request** (milestone 597,
/// provisional). The note is there and malformed, or it declares something an image request cannot
/// carry yet, or nobody vouched for the bytes and the note asks for what only a command line could
/// designate ([`crate::ImageRefusal`]), or the request's argument or `--mem` count does not fit
/// the manifest the child would be endowed with. Nothing is built.
///
/// A refusal and not a failure, for [`SPAWN_UNVOUCHED`]'s reason: nothing broke. Three below
/// `u64::MAX`, for [`JOB_FAULTED`]'s reason. Name: provisional.
pub const SPAWN_REFUSED_BY_MANIFEST: u64 = u64::MAX - 3;

/// **The spawn service's word for a `graphical_terminal` launch on a boot with no display**
/// (milestone 715 (provisional)). The shell holds no display device, so it asks, and this is the
/// answer. Distinct from [`SPAWN_FAILED`]: no display is a fact about the machine, and a session
/// that could not be built (a second session, the job pool spent) is a failure. Four below
/// `u64::MAX`, for [`JOB_FAULTED`]'s reason.
///
/// Mechanism: ruled 2026-10-04 (UTC, calef): the shell learns "no display" from this result word,
/// not from a fact it is told at `_start`. Name: provisional (milestone 715, 2026-10-03); the
/// ruling covered the mechanism only.
pub const SPAWN_NO_DISPLAY: u64 = u64::MAX - 4;

/// **The progenitor's word for a program this image does not carry**: the initial ramdisk has no
/// entry under the program's name, so there was nothing to build. Distinct from [`SPAWN_FAILED`]
/// for [`SPAWN_UNVOUCHED`]'s reason: an image built without `jig` answered `jig` with "out of
/// memory", which sent a person looking at the job pool for a fault in how the image was built.
/// Five below `u64::MAX`, for [`JOB_FAULTED`]'s reason.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/image-needs-jig.
pub const SPAWN_NOT_IN_IMAGE: u64 = u64::MAX - 5;

/// The ack the progenitor sends on the result endpoint when a **supervised** (interruptible) child
/// started cleanly. An interruptible child reports its own progress and exit through the shared job
/// frame, not the result endpoint, so the progenitor sends this once as the go-ahead: the shell
/// reads it, then begins watching the job frame. `0` is distinct from [`SPAWN_FAILED`].
pub const SPAWN_OK: u64 = 0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mounted_trees_rights_are_list_walk_and_read() {
        use filesystem_contract::dir;
        assert_eq!(
            MOUNTED_TREE_RIGHTS,
            dir::ENUMERATE | dir::READ | dir::DESCEND
        );
    }

    /// The reaped message carries the label and the thread id, in that order, and a zero third
    /// word, so it cannot be read as a delegated capability's.
    #[test]
    fn a_reaped_message_is_label_then_thread() {
        assert_eq!(reaped(6, 9), (6, 9, 0));
    }

    /// **A release names the job, and only a release is one** (milestone 933). Every wiring flag
    /// at once is still a spawn, and a release with a zero label still reads as a release, so the
    /// progenitor's first test of a request never mistakes one for the other.
    #[test]
    fn a_release_is_its_label_and_only_a_release_is_one() {
        let (w0, w1, w2) = release(41);
        assert_eq!(released_label(w0, w2), Some(41));
        assert_eq!(w1, 0);
        assert_eq!(released_label(0, release(0).2), Some(0));
        assert_eq!(activation(w1, w2), None, "a release is not an activation");
        let (_, _, spawn) = request(3, 0, 64, Wiring::default());
        assert_eq!(released_label(41, spawn), None);
        assert_eq!(held_answer(7), (SPAWN_OK, 7, 0));
    }

    /// **An approval is one of the activation verbs, and only the ones it serves are served**
    /// (milestone 911 (scripted installs grant only what the owner's approval file allows)):
    /// `Approve` and `Revoke` read back, the request words carry the records' length for the
    /// one that has frames, and the installer endpoint serves both beside install, remove and
    /// rollback, never a vouch.
    #[test]
    fn approve_and_revoke_are_verbs_the_installer_serves() {
        let (w0, w1, w2) = activation_request(Activation::Approve, 512);
        assert_eq!(w0, 512, "the records' length travels");
        assert_eq!(activation(w1, w2), Some(Some(Activation::Approve)));
        let (r0, w1, w2) = activation_request(Activation::Revoke, 0);
        assert_eq!(activation(w1, w2), Some(Some(Activation::Revoke)));
        assert_eq!(r0, 0, "a revoke carries no length");
        assert!(installer_serves(Activation::Approve));
        assert!(installer_serves(Activation::Revoke));
        assert!(installer_serves(Activation::Install));
        assert!(!installer_serves(Activation::Vouch));
    }

    /// **The statuses the delegated check answers** (milestone 911): `NotApproved` for an
    /// install, removal or rollback the approval does not allow, `Revoked` for one whose
    /// approval is gone, and a word this reader does not know is `Unknown`.
    #[test]
    fn not_approved_and_revoked_read_back() {
        assert_eq!(
            ActivationStatus::from_word(16),
            ActivationStatus::NotApproved
        );
        assert_eq!(ActivationStatus::from_word(17), ActivationStatus::Revoked);
        assert_eq!(ActivationStatus::from_word(18), ActivationStatus::Unknown);
    }

    /// **A delegated badge carries its job and its approval, and neither can be read as the
    /// other** (milestone 911): the label survives the approval's bits, an undelegated badge
    /// carries no approval, and the kernel's own one-word badge (`reaped`) reads as neither.
    #[test]
    fn a_delegated_badge_carries_its_job_and_its_approval() {
        let delegated = delegated_installer_badge(7, 3);
        assert_eq!(installer_label(delegated), Some(7));
        assert_eq!(installer_approval(delegated), Some(3));
        let plain = installer_badge(7);
        assert_eq!(installer_label(plain), Some(7));
        assert_eq!(installer_approval(plain), None);
        assert_eq!(installer_approval(reaped(9, 9).0), None);
        assert_eq!(installer_label(reaped(9, 9).0), None);
        // The two forms are the same endpoint object: one badge bit, one endpoint.
        assert_eq!(
            delegated & !INSTALLER_BADGE,
            (3 << DELEGATED_APPROVAL_SHIFT) | 7
        );
    }

    /// **The answered trailer round-trips**, console and approval alike, and the packing is the
    /// removal name's rule (`filesystem_contract::grant::pack_name`), which this crate spells
    /// because it does not link the contract. A kind word this reader does not know spells
    /// nothing.
    #[test]
    fn an_answered_trailer_round_trips() {
        use filesystem_contract::grant;
        let (lo, hi) = grant::pack_name(b"lab-runner").unwrap();
        assert_eq!(
            pack_name(b"lab-runner"),
            (lo, hi),
            "the packing is the removal name's"
        );
        assert_eq!(answered_console(), (ANSWERED_CONSOLE, 0, 0));
        let mut out = [0u8; 16];
        assert_eq!(
            answered_from_trailer(answered_console(), &[], &mut out),
            Some(Answered::Console)
        );
        let digest = [0x5au8; 32];
        let trailer = answered_approval("lab-runner", &digest);
        assert_eq!(trailer[0].0, ANSWERED_APPROVAL);
        assert_eq!(trailer.len(), 3);
        match answered_from_trailer(trailer[0], &trailer[1..], &mut out) {
            Some(Answered::Approval {
                name,
                len,
                digest: got,
            }) => {
                assert_eq!(&name[..len], b"lab-runner");
                assert_eq!(got, digest);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(answered_from_trailer((9, 0, 0), &[], &mut out), None);
    }

    /// A holder's name survives the two messages whole, at the full width and empty.
    #[test]
    fn a_holder_name_round_trips() {
        let mut out = [0u8; HOLDER_MAXIMUM];
        for name in ["duet", "", "a-package-name-that-fills-it-all"] {
            assert_eq!(holder_from_words(holder_words(name), &mut out), name);
        }
        let mut out = [0u8; HOLDER_MAXIMUM];
        let words = [(u64::MAX, 0, 99), (0, 0, 0)];
        assert_eq!(
            holder_from_words(words, &mut out),
            "",
            "not UTF-8 spells nothing"
        );
    }

    #[test]
    fn request_round_trips() {
        // 6, not 1: word zero must come back verbatim, and an id of 1 cannot tell verbatim from a
        // hardcoded answer.
        let (w0, w1, w2) = request(6, 9, 16, Wiring::default());
        assert_eq!(prog_id(w0), 6);
        assert_eq!(program_argument(w1), 9);
        assert_eq!(memory_grant_pages(w2), 16);
        assert_eq!(wiring(w2), Wiring::default());
    }

    #[test]
    fn interruptible_bit_survives_a_zero_page_count() {
        // The interrupt demonstrators take no --mem, so the flag must ride independent of the
        // count.
        let (_, _, w2) = request(
            2,
            0,
            0,
            Wiring {
                interruptible: true,
                ..Wiring::default()
            },
        );
        assert_eq!(memory_grant_pages(w2), 0);
        assert!(is_interruptible(w2));
    }

    #[test]
    fn no_grant_is_zero_pages() {
        let (_, _, w2) = request(0, 5, 0, Wiring::default());
        assert_eq!(memory_grant_pages(w2), 0);
    }

    /// **The fourteen flags are independent of each other and of the page count** (milestone 50 (pipes
    /// and redirection), §67 (a program's second stream is a declaration)'s fourth, milestone 31 (a
    /// capability shell) phase 3's fifth, DECISIONS §106 (the `terminal_sink_caretaker` narrowing)'s
    /// sixth, milestone 154 (a process that holds two directory capabilities)'s seventh, §219's
    /// image and its gate D2, §170's argv and name set, milestone 126's machine page, and
    /// milestone 632 (provisional)'s gpu and keyboard). They
    /// share one word, and what the progenitor reads
    /// next off the endpoint depends on all of them, so a bit that bled into another would make the
    /// progenitor take a capability for a data word (or the reverse) and hang rather than fail.
    #[test]
    fn the_wiring_flags_do_not_collide() {
        for m in 0u32..1 << 15 {
            let b = |i: u32| m & (1 << i) != 0;
            let w = Wiring {
                interruptible: b(0),
                sink: b(1),
                source: b(2),
                diagnostics: b(3),
                directory_grant: b(4),
                second_directory_grant: b(5),
                screen: b(6),
                image: b(7),
                run_unvouched: b(8),
                program_arguments: b(9),
                nameset: b(10),
                machine: b(11),
                graphics: b(12),
                keyboard: b(13),
                held: b(14),
            };
            let (_, _, w2) = request(3, 0, 64, w);
            assert_eq!(wiring(w2), w, "{w:?}");
            assert_eq!(memory_grant_pages(w2), 64, "{w:?}");
        }
    }

    /// **The three sentinels are distinct from each other** (milestone 235). They share one word
    /// on one endpoint and the shell tells them apart by value alone, so a collision would make
    /// "nothing was built", "it ran and died" and "it started" the same message.
    #[test]
    fn the_result_sentinels_do_not_collide() {
        let all = [SPAWN_FAILED, JOB_FAULTED, SPAWN_UNVOUCHED, SPAWN_OK];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    /// **Word 0 is a length under [`IMAGE_BIT`], and it survives the round trip whole** (§219 D).
    /// 89,168 is `uptime`'s stripped size, which is not a page multiple, so the frame count has to
    /// round up: a request that rounded down would send one frame fewer than the progenitor waits
    /// for, and both sides would hang.
    #[test]
    fn an_image_request_carries_its_length_and_rounds_up_to_frames() {
        let (w0, _, w2) = request(
            89_168,
            0,
            0,
            Wiring {
                image: true,
                ..Wiring::default()
            },
        );
        assert!(wiring(w2).image);
        assert_eq!(image_len(w0), 89_168);
        assert_eq!(image_pages(image_len(w0)), 22);
        assert_eq!(image_pages(4096), 1);
        assert_eq!(image_pages(4097), 2);
        assert_eq!(image_pages(0), 0);
    }

    /// **An activation request is told apart from every spawn by one bit, and its verbs and
    /// statuses round-trip** (milestone 198 rung 3a). A spawn request with every wiring flag set is
    /// still not an activation request, which is what keeps the progenitor from reading a spawn's
    /// program id as a package length.
    #[test]
    fn an_activation_request_is_not_a_spawn_and_round_trips() {
        let all = Wiring {
            interruptible: true,
            sink: true,
            source: true,
            diagnostics: true,
            directory_grant: true,
            second_directory_grant: true,
            screen: true,
            image: true,
            run_unvouched: true,
            program_arguments: true,
            nameset: true,
            machine: true,
            graphics: true,
            keyboard: true,
            held: true,
        };
        let (_, w1, w2) = request(3, 2, 64, all);
        assert_eq!(released_label(3, w2), None, "a spawn is never a release");
        assert_eq!(activation(w1, w2), None);
        for verb in [
            Activation::Install,
            Activation::Remove,
            Activation::Rollback,
            Activation::Vouch,
        ] {
            let (w0, w1, w2) = activation_request(verb, 90_491);
            assert_eq!(activation(w1, w2), Some(Some(verb)));
            assert_eq!(
                w0,
                if matches!(verb, Activation::Install | Activation::Vouch) {
                    90_491
                } else {
                    0
                }
            );
        }
        assert_eq!(activation(9, ACTIVATION_BIT), Some(None));
        // The retired fetch's number is a verb nobody knows now.
        assert_eq!(activation(4, ACTIVATION_BIT), Some(None));
        for status in [
            ActivationStatus::Done,
            ActivationStatus::NotCataloged,
            ActivationStatus::NoProgram,
            ActivationStatus::NotInstalled,
            ActivationStatus::NoEarlier,
            ActivationStatus::StoreFailed,
            ActivationStatus::Unknown,
            ActivationStatus::NotExecutable,
            ActivationStatus::NameTaken,
            ActivationStatus::ImageName,
            ActivationStatus::Ambiguous,
            ActivationStatus::NotServedHere,
            ActivationStatus::NotYours,
        ] {
            let (w0, w1, _) = activation_reply(status, 7, 0);
            assert_eq!(ActivationStatus::from_word(w0), status);
            assert_eq!(w1, 7);
        }
    }

    /// **The installer endpoint's badge names its job and no other sender's** (milestone 809), and
    /// it serves install, remove and rollback, never a vouch.
    #[test]
    fn an_installer_badge_carries_its_job_and_is_no_other_senders() {
        assert_eq!(installer_label(installer_badge(7)), Some(7));
        assert_eq!(installer_label(0), None, "the shell's copy is unbadged");
        assert_eq!(installer_label(UNDERTAKER_BADGE), None);
        assert!(installer_serves(Activation::Install));
        assert!(installer_serves(Activation::Remove));
        assert!(installer_serves(Activation::Rollback));
        assert!(!installer_serves(Activation::Vouch));
    }

    /// **`SECOND_DIRECTORY_GRANT_BIT` follows [`DIRECTORY_GRANT_BIT`]'s own precedent**: it is a
    /// second bit, not a count, and it round-trips whether or not `directory_grant` is set; the
    /// "meaningless without it" rule is the emitter's ([`Wiring::second_directory_grant`]).
    #[test]
    fn a_second_directory_grant_is_a_second_bit_not_a_count() {
        let (_, _, w2) = request(
            0,
            0,
            0,
            Wiring {
                directory_grant: true,
                second_directory_grant: true,
                ..Wiring::default()
            },
        );
        let w = wiring(w2);
        assert!(w.directory_grant);
        assert!(w.second_directory_grant);
    }

    /// The replies a spawn can make are one family near the top of the word, each its own value,
    /// and none of them is a thing a child could report as its own result.
    #[test]
    fn the_spawn_sentinels_are_distinct_values_at_the_top_of_the_word() {
        let all = [
            SPAWN_FAILED,
            JOB_FAULTED,
            SPAWN_UNVOUCHED,
            SPAWN_REFUSED_BY_MANIFEST,
            SPAWN_NO_DISPLAY,
            SPAWN_NOT_IN_IMAGE,
        ];
        for (i, a) in all.iter().enumerate() {
            assert!(*a > u64::MAX - 8, "{a:#x}");
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_eq!(SPAWN_OK, 0);
    }

    /// The shell's budget is its own 128 pages and one image's staging frames.
    #[test]
    fn the_shell_budget_is_its_windows_and_one_staged_image() {
        assert_eq!(
            SHELL_BUDGET_PAGES,
            128 + IMAGE_MAXIMUM_PAGES + crate::JOB_POOL_PAGES
        );
    }
}
