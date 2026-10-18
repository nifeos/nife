//! **The spawn protocol: the wire half of the shell-to-progenitor grant expression.** The shell
//! resolves a `run` into an [`Endowment`](crate::Endowment) but does not build the child: the
//! progenitor holds the initial ramdisk and the ELF loader. So the shell says what to spawn and
//! delegates the capabilities it grants over the same endpoint. A userspace protocol, not kernel
//! ABI: the kernel routes these words as any IPC and never reads them (§10 (process model:
//! capability-based)).
//!
//! 1. **Request.** Three words on the spawn endpoint: the program id, the integer argument, and the
//!    memory-grant page count ([`request`], [`prog_id`], [`arg`], [`mem_pages`]).
//! 2. **The directory grant**, if announced ([`Wiring::dir`]): [`GRANT_WORDS`] plain `SEND`s,
//!    before the capabilities, because they are data and mixing orders would put a `RECEIVE` where
//!    a `RECEIVE_CAP` belongs.
//! 3. **Delegation**, in a fixed order both sides read from the same [`Wiring`]: the supervised
//!    job's pair, the sink, the source, the diagnostic endpoint, the screen-narrowed tail's
//!    completion endpoint, the `--mem` untyped (split from the shell's own budget), the gpu's four
//!    and the keyboard's three, and the machine statistics page last. Order rather than tags; a
//!    promise nobody receives would deadlock both sides.
//! 4. **Outcome.** The progenitor builds, endows and starts the child, which answers on the result
//!    endpoint; if it cannot, it sends [`SPAWN_FAILED`] there, so the shell's single read completes.
//! 5. **Death.** For a killed child, `job_undertaker` sends [`JOB_FAULTED`] on that same read.
//! 6. **Reaped.** After every collect, `job_undertaker` sends [`reaped`] on the spawn endpoint
//!    through a copy badged [`UNDERTAKER_BADGE`], which tells the progenitor the job's memory and
//!    file-service window are back.
//!
//! # BUGS
//!
//! - Any promised message a caller withholds hangs the progenitor: claiming run-unvouched
//!   ([`Wiring::run_unvouched`]) without holding [`RUN_UNVOUCHED_SLOT`], or a shell that cannot
//!   retype a frame mid-request. Nothing lets either side abandon a half-sent request; today the
//!   boot shell is the only holder. A holder can also lend its presentation to another's claim.
//! - An image's manifest note ([`crate::image_manifest`]) reaches only what these words carry, so a
//!   note declaring a file, directory, input, option, job, second stream, silent output or `std` is
//!   refused ([`crate::image_can_carry`]). Its [`crate::Endowment`] names a stand-in row
//!   ([`crate::IMAGE_ROW`]); reading that row instead of the manifest is the foot gun.
//! - The shell reads the note twice; the progenitor judges what arrived and refuses a misfit with
//!   [`SPAWN_REFUSED_BY_MANIFEST`]. Only a plain line runs an image, never interruptibly.
//! - Every activation verb, [`Activation::Vouch`] included, is open to any spawn-endpoint holder,
//!   safe only while the owner's boot prompt is the one (§221); install, remove and rollback have
//!   [`INSTALLER_BADGE`], vouch has none.

/// The interruptible bit, packed into the high half of the page-count word so one `SEND` still
/// carries the whole request. `mem_pages` is a small count (`memory_grant_depleter`'s ceiling is
/// 64), so the low 32 bits hold it and this bit rides above.
const INTERRUPTIBLE_BIT: u64 = 1 << 32;

/// **A capability for the child's output slot follows** (milestone 50). Set by `>` and by every
/// stage of a `|` but the last: the shell delegates an endpoint and the progenitor puts it where the result
/// endpoint would have gone, so the child writes to a pipe or a file sink without knowing which.
const SINK_BIT: u64 = 1 << 33;

/// **A capability for the child's input slot follows** (milestone 50). Set by `<` and by every
/// stage of a `|` but the first.
const SOURCE_BIT: u64 = 1 << 34;

/// **A capability for the child's declared second output follows** (DECISIONS §67). Set for a
/// program whose manifest declares one, whether or not the line has a `2>` on it: the stream exists
/// because the program says so, and the operator only names where it goes.
///
/// Unlike [`SINK_BIT`] this does **not** say which slot: the progenitor reads that from the manifest, because
/// the slot is the program's declaration and not the shell's choice. What the wire says is only
/// "expect one more capability", which is what keeps the two sides in lockstep.
const DIAG_BIT: u64 = 1 << 35;

/// **A capability for a narrowed tail stage's completion signal follows** (DECISIONS §106). Set
/// when the shell decided this stage both writes and reads, and the line named neither `>` nor `|`
/// for its output (`Wiring::sink` is false and the plan says [`crate::line::Sink::Report`]): the
/// shell cannot be both this stage's feeder and its reader, so its primary output defaults to
/// `terminal_sink_caretaker` instead of the shell's own result endpoint, the same adapter a
/// declared second stream already reaches by default under DECISIONS §67.
///
/// What follows is not a sink capability (the progenitor already knows to build that default from its own
/// `term_sink`, unprompted, the same way it builds a diagnostic default). It is a **fresh
/// endpoint the shell minted and kept a copy of**, delegated so the progenitor can install it as this child's
/// DECISIONS §26 fault target in place of its own domain channel. The kernel then delivers the
/// child's exit there instead of to the progenitor's reaper, and the shell `RECEIVE`s it as its completion
/// signal instead of draining the child's bytes, which it no longer sees.
const SCREEN_BIT: u64 = 1 << 37;

/// **A directory grant follows, and the progenitor is to build a caretaker for it** (milestone 31 phase 3).
///
/// The odd one out on this word, because it announces **data rather than a capability**. Every other
/// bit here says "expect one more `SEND_CAP`"; this one says "expect two more `SEND`s", and the
/// reason is that the shell has nothing to delegate. A directory grant is delivered by a
/// `fs_subtree_caretaker`, the caretaker has to hold the file service to attenuate it, and **the
/// shell's file-service endpoint carries no `GRANT`**, so the shell could not hand one over if it
/// wanted to. What it can do is say what the grant *is*; the progenitor holds the endpoint and builds the rest.
///
/// See [`GRANT_WORDS`] for what the two messages carry and why they are opaque to this module.
const DIR_BIT: u64 = 1 << 36;

/// **A second directory grant follows, for the same confined program** (milestone 154,
/// design/roadmap/0154-multi-directory-namespace.md). Set only alongside [`DIR_BIT`]: a second
/// grant is meaningless without a first, the same way the kernel's `fs_service::TwoDirGrant`
/// (slot 0 grant A, slot 1 grant B) only exists in pairs.
///
/// **Following [`DIR_BIT`]'s own precedent rather than inventing a new shape**, as milestone
/// 154's roadmap block names as the open question this closes: another two [`GRANT_WORDS`]
/// messages follow the first pair, carrying the second caretaker's `START` words. Unlike
/// [`DIR_BIT`], the confined program's own `START` words are **not** repeated a second time: one
/// program is still being started, holding two narrowed endpoints (slot 0 grant A, slot 1 grant
/// B), the same delivery `fs_service::start_granted_two_dirs` already proved.
///
/// **Nothing on the shell side sets this bit yet, and the progenitor does not decode it.** No
/// manifest declares two directory operands, so no plan constructs a two-directory `Endowment`
/// (notes/two-trees.md's BUGS). This is the wire format alone, built ahead of an emitter the way
/// [`DIR_BIT`] itself once stated a grant nothing could construct yet.
const DIR2_BIT: u64 = 1 << 38;

/// **The executable's bytes follow, as page frames the caller owns** (DECISIONS §219 option D,
/// ruled by calef 2026-09-26). Set when the shell runs a program the image did not name by
/// [`Prog`](crate::Prog) id: an installed package's member, or a binary somebody just built.
///
/// **Word 0 changes meaning under this bit**: it carries the image's **length in bytes** instead
/// of a program id ([`image_len`]), and [`image_pages`] of that many `SEND_CAP`s follow, one frame
/// each, in page order, each tagged with its index. They come after the directory grant's data
/// messages and before every other delegated capability, so the data-before-capabilities order the
/// rest of this word keeps is kept here too. The shell sends each frame narrowed to `READ`, so the
/// progenitor can map it and can neither write it nor pass it on.
///
/// **The progenitor hashes its own copy**, never the caller's frames, because the caller keeps a
/// mapping of them and could change the bytes between a hash and a build. It maps each frame
/// through the loader's scratch window (`supervision_protocol::map_scratch`), copies it into a page of its own, and deletes
/// the capability before taking the next, so a request of any size costs its capability table one
/// transient slot. A digest found in the installed-programs table is vouched and runs with that entry's
/// manifest. A miss runs only for a caller that presents the run-unvouched capability
/// ([`RUN_UNVOUCHED_BIT`], §219's gate D2), with [`crate::UNVOUCHED_MANIFEST`]; any other miss is
/// refused with [`SPAWN_UNVOUCHED`].
///
/// Name: provisional (milestone 198 rung 3a, 2026-09-26), §219's own word for it.
const IMAGE_BIT: u64 = 1 << 39;

/// **Not a spawn: an edit to the installed-programs table** (milestone 198 (a package manager) rung 3a's
/// installer; DECISIONS §208 (installing a package is granting it, and the installed programs are
/// versioned)). When this bit is set the request asks the progenitor to install, remove or roll
/// back, word 1 is an [`Activation`] verb, the rest of word 2 is zero, and the progenitor answers
/// with exactly one message on the result endpoint: [`activation_reply`]'s two words.
///
/// **Why the progenitor serves it**, rather than an installer program: §208's argument for A3 was
/// that "the supervisor that performs a live swap and the thing that decides which version is
/// active are the same authority", and §219 already gave the progenitor the read half (it looks
/// every image's digest up in the live generation). It holds the file service with `WRITE`, the
/// image's catalogue in its archive, and the frame-staging path an image request built. The
/// *client* is a program, `jig` (milestone 809 (the package client becomes a program)), which
/// sends this request on its installer endpoint ([`INSTALLER_BADGE`]); the owner's console sends
/// it on the spawn endpoint for `vouch`.
///
/// What follows the request depends on the verb:
///
/// - For [`Activation::Install`], word 0 is the package file's length in bytes, and its frames follow
///   exactly as an image's do (`IMAGE_BIT`: [`image_pages`] `SEND_CAP`s, each narrowed to `READ`).
///   The progenitor copies them before it hashes, for the image path's reason.
/// - For [`Activation::Remove`], one data message follows, the program's name packed the way a
///   directory grant packs one (`filesystem_contract::grant::pack_name`: two words, then the
///   length). Opaque here for [`GRANT_WORDS`]'s reason.
/// - For [`Activation::Rollback`], nothing follows.
/// - For [`Activation::Vouch`], word 0 is the executable's length, its frames follow as an
///   install's do, and then the name it will be recorded under, as one data message packed as
///   [`Activation::Remove`]'s is. Frames before the name, because the frames are capabilities and
///   the name is data, and the progenitor stages the one before it reads the other.
///
/// Name: provisional (2026-09-26).
const ACTIVATION_BIT: u64 = 1 << 40;

/// **The caller presents the run-unvouched capability: one `SEND` on it follows the delegation**
/// (DECISIONS §219 gate D2, ruled by calef 2026-09-26). Meaningful with [`IMAGE_BIT`]; the
/// progenitor honours it on any request, so a caller that sets it is never left blocked in its
/// `SEND`.
///
/// **Why a message on a second endpoint rather than a capability on this one.** The capability is
/// granted without `GRANT`, so its holder cannot pass it on (§219 limitation 2), and for the same
/// reason it cannot ride a `SEND_CAP` here: the kernel refuses to delegate a capability that lacks
/// `GRANT`. What a holder *can* do with a `WRITE`-only endpoint is send on it. The progenitor holds
/// the only `READ` on that endpoint and takes one message from it, at a fixed point in the
/// exchange, while serving a request that claimed it. A message that arrives there came from a
/// holder, because nothing else can reach the endpoint.
///
/// **Where in the exchange**: after every delegated capability, the last thing a request sends. The
/// progenitor then knows the digest's verdict and the whole delegation before it decides. The word
/// sent is not read: the fact that it arrived is the whole of what it says.
///
/// The progenitor rather than the kernel decides what it means, which is why this is a bit on a
/// userspace word and not a method: DECISIONS §10 (process model: capability-based, microkernel)'s surface is unchanged. The kernel only routes a
/// `SEND` on an endpoint, as it routes every other.
///
/// Name: provisional (milestone 198 rung 3a, 2026-09-26).
const RUN_UNVOUCHED_BIT: u64 = 1 << 41;

/// **The line's argv follows as one frame** (milestone 205 (how a foreign program is told what to
/// do), DECISIONS §170 (how a foreign program is told what to do)). The shell assembles the words it was typed into one page in
/// `program_argument_contract`'s layout and `SEND_CAP`s it narrowed to `READ`, after the directory grant's
/// data and any image's frames, and before every other delegated capability.
///
/// **The progenitor copies it**, for [`IMAGE_BIT`]'s reason: the shell keeps a mapping of the frame.
/// It copies into a page carved from the child's own region, so the child's reclaim frees it, and
/// places that at `std_runtime_contract::ARGS_SLOT` mapped read-only at `ARGS_PAGE`. The bytes are
/// not read on the way, because §170 ruled that they carry no authority: what a word may reach is
/// the directory grant's business and nothing on this page can widen it.
///
/// A request that sets it for a program that does not run on the `std` layout has its frame taken
/// and dropped, so both sides stay in lockstep and the child gets nothing it could not read.
///
/// Name: provisional (milestone 205, 2026-09-26).
const ARGS_BIT: u64 = 1 << 42;
/// **The machine statistics page follows as one `SEND_CAP`, after every other delegated
/// capability** (milestone 126 (the `procps` package), DECISIONS §225 (`free` sees the machine and
/// your share)). Set by a shell that holds the page at [`MACHINE_PAGE_SLOT`] when the program's
/// manifest declares `machine`; the progenitor maps it read-only into the child and places it at
/// `crate::MACHINE_SLOT`, then deletes its copy.
///
/// **Why the page travels with the request rather than living in the progenitor.** The progenitor's
/// capability table peaks during the login block at one slot under the table's size
/// (`kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED`), and a page held for the life of the boot would
/// have spent that last slot; the first CI run that tried it measured the table full. The progenitor
/// hands the page to the shell before the login block and keeps no copy, so what reaches a program
/// is decided by what its session holds. That is also the shape §225's "granted to every login,
/// withholdable by the owner" reads as: a session without the page cannot pass it on.
///
/// **Bit 44, not the 42 calef ratified**: `ARGS_BIT` took 42 (milestone 205, #1394) and
/// `NAMESET_BIT` takes 43 (#1402) while this was in flight, and numbers stay provisional until the
/// queue lands them. Still in word 2's flag half, above the 32-bit page count.
///
/// Name: ratified 2026-09-27 (calef, #1360's table).
const MACHINE_BIT: u64 = 1 << 44;

/// **This is a graphical terminal launch** (milestone 632 (provisional), calef's 2026-09-30 ruling
/// that graphics is launched from the prompt rather than built at boot). **No capability follows
/// it** since milestone 715 (provisional): the spawn service holds the gpu's four (and a
/// keyboard's three) from boot, and builds the session's drivers from its own copies, the way it
/// already held the boot discipline's endpoint for the same session. Until 715 the shell held the
/// seven at [`SHELL_GPU_SLOT`] onward for the life of the boot and delegated them after this bit,
/// which lent the prompt `GRANT` on every device; the 2026-10-03 security audit's follow-up
/// narrowed that. A boot with no display answers [`SPAWN_NO_DISPLAY`].
///
/// Meaningful only for a `graphical_terminal` request, and the progenitor refuses any other program that sets
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

/// **Where a session holds the machine statistics page** (milestone 126, DECISIONS §225): `READ |
/// GRANT`, so it can delegate it with [`MACHINE_BIT`] and not write it. A named slot probed at
/// `_start`, for [`RUN_UNVOUCHED_SLOT`]'s reasons.
///
/// **Twenty, not the 21 calef ratified**: `crate::SHELL_CONFIGURATION_SLOT` took 21 (milestone 47 (navigation and naming)) while
/// this was in flight.
///
/// Name: ratified 2026-09-27 (calef, #1360's table).
pub const MACHINE_PAGE_SLOT: u64 = 20;

/// **Empty since milestone 715 (provisional)**: the spawn service holds the display devices, and no
/// builder places anything here. The block stays reserved, and the fence below stays, because a
/// probed slot is sound only where nothing allocates. What it was:
///
/// **Where a session holds the gpu's confined transport** (milestone 632 (provisional)), with its
/// three siblings beside it: the completion interrupt at 23, the DMA run at 24, the surface run at
/// 25, and, when a virtio keyboard exists, its transport at 26, event interrupt at 27 and DMA page
/// at 28. `WRITE | GRANT` on the transport, `READ | GRANT` on each interrupt, `READ | WRITE |
/// GRANT` on each page: exactly the rights the kernel granted the progenitor, which places them
/// here at the shell's build and keeps no copy, for [`MACHINE_PAGE_SLOT`]'s own reasons. A boot
/// with no GPU leaves all seven empty, and the shell's `_start` probe is what tells that apart.
///
/// **Twenty-two through twenty-eight**, the block between [`crate::SHELL_CONFIGURATION_SLOT`] (21) and
/// [`RUN_UNVOUCHED_SLOT`] (62): a named block rather than next-free numbering, because the shell
/// probes these slots rather than being told, and a probe is sound only for a slot nothing else
/// allocates into. One block of seven, contiguous, so the relation to the two slots that fence it
/// reads as two assertions rather than seven.
///
/// Name: ratified 2026-10-03 (calef, #1493), with slots 22 to 28 and the delegation order (after the
/// `--mem` untyped, before the machine statistics page).
pub const SHELL_GPU_SLOT: u64 = 22;

// The seven graphical slots are one contiguous block fenced by two named ones, and these two
// assertions are the fence: above the shell's configuration page, below the run-unvouched slot
// (and so below the kernel's fault slot, which each binary asserts against `abi` itself, for the
// reason `RUN_UNVOUCHED_SLOT`'s own doc gives). A slot outside the fence would be one a runtime
// allocation could land in, and a probe there would be sound only in a table that is almost full.
const _: () = assert!(SHELL_GPU_SLOT > MACHINE_PAGE_SLOT);
const _: () = assert!(SHELL_GPU_SLOT > crate::SHELL_CONFIGURATION_SLOT);
const _: () = assert!(SHELL_GPU_SLOT + 6 < RUN_UNVOUCHED_SLOT);

/// **The directory grant is a set of names, and the set follows as one frame** (milestone 205 (how
/// a foreign program is told what to do), §170 clauses 2 to 5). Meaningful with [`Wiring::dir`]: the
/// progenitor builds `fs_nameset_caretaker` rather than `fs_subtree_caretaker` for the grant, and
/// maps it the set, `filesystem_contract::nameset`'s encoding, read-only. The shell sends the frame
/// `READ`-only after the argv's, before every other delegated capability; the progenitor copies it,
/// for [`IMAGE_BIT`]'s reason, into a page from the job's region.
///
/// It is how a program that hears words gets exactly the names its words designated in the
/// shell's directory and no sibling of them. `rm *.txt` would ride it too; nothing sends it for
/// `rm` yet (`components/src/swish.rs`'s `dir_grant`).
///
/// Name: provisional (milestone 205, 2026-09-27).
const NAMESET_BIT: u64 = 1 << 43;

/// **Where a session holds the run-unvouched capability** (DECISIONS §219 gate D2): the slot the
/// progenitor places it in, `WRITE` only, in the boot shell and in `login`, and the slot `login`
/// delegates it from.
///
/// Sixty-two, the highest slot below the kernel's reserved fault slot (`abi::fault::FAULT_EP_SLOT`,
/// 63; `grant_plan` does not depend on `abi`, so each binary that reads this asserts the relation
/// itself). It was twenty-two until calef raised the table from 24 slots to 32 on 2026-09-27 (UTC),
/// thirty until milestone 754 (the capability table grows to 64 slots) raised it to 64 on 2026-10-04 (UTC), and it moved with the fault slot
/// both times so those assertions still hold. A named slot for the reason [`crate::NETWORK_SLOT`] is one: the holder probes it rather
/// than being told, and the probe is sound only at `_start`, before the process has allocated
/// anything, because a runtime allocation takes the first free slot and could land here only in a
/// table that is almost full.
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
    /// new generation whose entry for the name is that digest, marked
    /// `installed_programs::OWNER` in place of a package, so
    /// `package rollback` undoes it like any other edit. The bytes are not copied: a digest vouches
    /// for itself wherever the file is. They must parse as an executable, or nothing is written.
    ///
    /// **Who may ask** is whoever holds the spawn endpoint, which is the boot prompt and nothing
    /// else; §221 ruled that whoever holds that prompt is the owner. A session `login` builds holds
    /// no spawn endpoint and cannot reach its root to write `activation/` by hand either. **The
    /// installer endpoint refuses it** (milestone 809, [`INSTALLER_BADGE`]): a package manager may
    /// install what a catalog vouches for, and may not vouch for any bytes it likes.
    ///
    /// Name: provisional (milestone 198, lane `milestone/198-owner-console`, 2026-09-26).
    Vouch = 5,
}

impl Activation {
    fn from_word(w: u64) -> Option<Self> {
        match w {
            1 => Some(Self::Install),
            2 => Some(Self::Remove),
            3 => Some(Self::Rollback),
            5 => Some(Self::Vouch),
            _ => None,
        }
    }
}

/// Build an activation request. `len` is the file's length for [`Activation::Install`] and
/// [`Activation::Vouch`], whose frames follow, and ignored otherwise.
pub fn activation_request(verb: Activation, len: u64) -> (u64, u64, u64) {
    let w0 = if matches!(verb, Activation::Install | Activation::Vouch) {
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
    /// The image's catalogue does not vouch for these bytes (DECISIONS §195 (a reviewed recipe vouches for a package)), or they are not a
    /// package at all. Nothing was written.
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
    /// A verb this progenitor does not know, or a package larger than [`IMAGE_MAX_PAGES`].
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
    /// (`installed_programs::may_edit`; calef's ruling of 2026-10-06 on milestone 809: a manager
    /// "refuses to remove or roll back a row it did not install"). Nothing was written.
    /// Provisional.
    NotYours = 15,
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
pub const HOLDER_MAX: usize = 32;

/// **The two messages that follow a [`ActivationStatus::NameTaken`] reply on an installer
/// endpoint** (milestone 919): the name of the package that already holds the program, so the
/// owner is told what to remove first. Four little-endian words of name and its length. The spawn
/// endpoint sends no install, so only a package manager reads these. Name: provisional.
pub fn holder_words(holder: &str) -> [(u64, u64, u64); 2] {
    let mut bytes = [0u8; HOLDER_MAX];
    let len = holder.len().min(HOLDER_MAX);
    bytes[..len].copy_from_slice(&holder.as_bytes()[..len]);
    let word = |i: usize| {
        let mut w = [0u8; 8];
        w.copy_from_slice(&bytes[i * 8..i * 8 + 8]);
        u64::from_le_bytes(w)
    };
    [(word(0), word(1), len as u64), (word(2), word(3), 0)]
}

/// [`holder_words`] read back into `out`: the holder's name, or `""` for words that do not spell
/// one.
pub fn holder_from_words(words: [(u64, u64, u64); 2], out: &mut [u8; HOLDER_MAX]) -> &str {
    let [(a, b, len), (c, d, _)] = words;
    for (i, w) in [a, b, c, d].into_iter().enumerate() {
        out[i * 8..i * 8 + 8].copy_from_slice(&w.to_le_bytes());
    }
    let len = usize::try_from(len).map_or(HOLDER_MAX, |l| l.min(HOLDER_MAX));
    core::str::from_utf8(&out[..len]).unwrap_or("")
}

/// **The largest image `IMAGE_BIT` may carry, in pages** (4 MiB). A ceiling both sides read,
/// so the shell refuses a larger file before it sends anything and the progenitor never stages more
/// than its image pool can hold beside the child built from it. `uptime` is 22 pages stripped, and
/// `ripgrep` 14.1.1's image spans about 670 (`notes/ripgrep-on-nife.md`), which is the program this
/// was raised from 64 for (milestone 595 (the shell runs a `std` program), 2026-09-27).
///
/// What the number costs, because it is reserved rather than spent: the shell's budget carries one
/// image's worth of staging frames, and the progenitor's image pool two (its own copy, which §219
/// has it hash, and the child's pages) plus a `std` program's heap. That is
/// `IMAGE_MAX_PAGES * 3 + grant_plan::STD_REGION_PAGES` pages, about 13.5 MiB, set aside at boot
/// whether or not an image is ever run. See `IMAGE_POOL_PAGES` in `crates/system_initializer`.
///
/// Provisional, like the bit: the number is the pools' arithmetic, not a property of any program.
pub const IMAGE_MAX_PAGES: u64 = 1024;

/// **The budget the progenitor hands the boot shell**, in pages, which both read: the progenitor
/// splits it, and the shell prints it in `caps` because there is no call that says how much is left.
/// Moved here from two constants that had to be kept equal by hand (milestone 595 (the shell runs a
/// `std` program), 2026-09-27).
///
/// 128 pages for `--mem` grants, pipes and the shell's own windows, as it was from milestone 31 (a
/// capability shell), plus one image's staging frames ([`IMAGE_MAX_PAGES`]), since the shell
/// reads a file run by its path into frames split from this budget before it sends them.
///
/// Name: provisional.
pub const SHELL_BUDGET_PAGES: u64 = 128 + IMAGE_MAX_PAGES;

/// The page size an image is carried in. A frame is one page on every architecture this tree
/// builds for.
pub const IMAGE_PAGE: u64 = 4096;

/// **The two messages a [`Wiring::dir`] request is followed by**, in order, each three words:
///
/// 1. **the caretaker's `START` words**, which the progenitor passes to `fs_subtree_caretaker` verbatim: the
///    granted directory's name and the `filesystem_contract::dir` rights the subtree capability is to carry;
/// 2. **the confined program's `START` words**, which the progenitor passes to the program verbatim: for `rm`,
///    the operand's name and the options that were typed.
///
/// **This module does not decode either, deliberately.** They are `filesystem_contract::grant`'s packing, and
/// `grant_plan` has no non-dev dependency on `filesystem_contract` on purpose (its own manifest says why: the
/// shell must be able to check a command line without linking the filesystem contract). Passing them
/// through as opaque triples keeps that true, and it means a change to how a grant is packed is a
/// change in one crate rather than in the wire this one owns. The shell packs them; the progenitor forwards
/// them; nothing in between reads them.
///
/// Two messages rather than one because the two processes are started with different names: the
/// caretaker with the *directory*, the program with the *operand inside it*. Six words do not fit in
/// three.
pub const GRANT_WORDS: usize = 2;

/// **Where the shell's operators end up on the wire**, alongside the parts of the endowment that
/// were always here. `sink` and `source` are booleans rather than capabilities because the
/// capability travels separately, over `SEND_CAP`: this word only says whether to expect it.
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
    /// **A directory grant follows as two data messages** ([`GRANT_WORDS`]), and the progenitor is to build a
    /// `fs_subtree_caretaker` for it before it builds the child. The only entry here that announces
    /// data instead of a capability; see `DIR_BIT`.
    pub dir: bool,
    /// **A second directory grant follows, as two more data messages** (milestone 154). See
    /// `DIR2_BIT`. Meaningless unless `dir` is also set; nothing here enforces that on its own
    /// (the wire is one word, and a caller that set this without `dir` sent a request that was
    /// wrong before it left the shell).
    pub dir2: bool,
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
    /// **The line's argv follows as one `READ` frame** (DECISIONS §170). See `ARGS_BIT`.
    pub args: bool,
    /// **The directory grant is a set of names, which follows as one `READ` frame** (milestone
    /// 205). See [`NAMESET_BIT`].
    pub nameset: bool,
    /// **The machine statistics page follows as the last delegated capability** (milestone 126).
    /// See [`MACHINE_BIT`].
    pub machine: bool,
    /// **The gpu's four capabilities follow, and the graphical terminal session is to be built from them**
    /// (milestone 632 (provisional)). See [`GRAPHICS_BIT`].
    pub graphics: bool,
    /// **A virtio keyboard's three follow beside the gpu's four** (milestone 632 (provisional)).
    /// See [`KEYBOARD_BIT`]; meaningless unless `graphics` is also set.
    pub keyboard: bool,
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
    if w.dir {
        w2 |= DIR_BIT;
    }
    if w.dir2 {
        w2 |= DIR2_BIT;
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
    if w.args {
        w2 |= ARGS_BIT;
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
    (prog_id, arg, w2)
}

/// The whole wiring of a received request (word 2), so the progenitor reads it once rather than asking three
/// separate questions of the same word.
pub fn wiring(w2: u64) -> Wiring {
    Wiring {
        interruptible: is_interruptible(w2),
        sink: w2 & SINK_BIT != 0,
        source: w2 & SOURCE_BIT != 0,
        diagnostics: w2 & DIAG_BIT != 0,
        dir: w2 & DIR_BIT != 0,
        dir2: w2 & DIR2_BIT != 0,
        screen: w2 & SCREEN_BIT != 0,
        image: w2 & IMAGE_BIT != 0,
        run_unvouched: w2 & RUN_UNVOUCHED_BIT != 0,
        args: w2 & ARGS_BIT != 0,
        nameset: w2 & NAMESET_BIT != 0,
        machine: w2 & MACHINE_BIT != 0,
        graphics: w2 & GRAPHICS_BIT != 0,
        keyboard: w2 & KEYBOARD_BIT != 0,
    }
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
pub fn arg(w1: u64) -> u64 {
    w1
}

/// The memory-grant page count from a received request (word 2). Non-zero means one delegated
/// untyped capability follows the interrupt caps (if any) over `SEND_CAP` / `RECEIVE_CAP`.
pub fn mem_pages(w2: u64) -> u64 {
    w2 & 0xffff_ffff
}

/// Whether this is a supervised foreground job (word 2's high bit). When set, the delegation leads
/// with two caps: a job untyped (the progenitor builds the child from it; the shell keeps it to `DESTROY`) and
/// a shared job frame (the cooperative interrupt flag and the child's status).
pub fn is_interruptible(w2: u64) -> bool {
    w2 & INTERRUPTIBLE_BIT != 0
}

/// The data word carried alongside the delegated untyped in the `SEND_CAP`. It is not load-bearing
/// (the progenitor identifies the cap by the protocol position, not the tag), but a fixed marker makes a
/// misrouted message obvious in a trace. Its low bits echo the page count as a cheap cross-check.
pub const CAP_TAG: u64 = 0x6361_705f; // "cap_" little-endian-ish marker

/// The sentinel the progenitor sends on the result endpoint when it could not build the child, so the
/// shell's single read completes with a legible failure rather than blocking forever. Distinct
/// from any answer a real program would report (no phase-1 program returns `u64::MAX`).
pub const SPAWN_FAILED: u64 = u64::MAX;

/// **The word for a job the kernel killed** (milestone 235,
/// design/roadmap/0235-a-faulted-job-should-reach-the-prompt.md). Sent on the result endpoint by
/// `job_undertaker`, which is the process already holding the progenitor's supervision endpoint, once it has
/// collected the corpse.
///
/// It exists because a faulted job is the one outcome this protocol could not say. A child that
/// exits non-zero has answered; a child the progenitor could not build gets [`SPAWN_FAILED`]; a child the
/// kernel killed **sends nothing at all**, so the shell's single read had nothing to complete it
/// and the prompt never came back (measured 2026-09-02: `least_authority_demo` patched to trap, and
/// `script/swish-check` reporting "the prompt never came back to take `least_authority_demo 7`").
///
/// # Why the supervisor says it rather than the shell asking or the endpoint carrying it
///
/// DECISIONS §26 (the fault endpoint: thread death becomes a message a supervisor holds) delivers
/// every death to exactly **one** endpoint, so the three couplings milestone 235 named are a choice
/// of who holds that endpoint, and only one of them leaves the ordinary paths alone.
///
/// **The shell asking** loses first. A shell that asks has to decide *when* to ask, and with no
/// non-blocking receive in the ABI (`crates/system_initializer`'s own loop records that it has
/// none) that decision is a poll interval, which is a timeout wearing a different hat: it cannot
/// tell a slow job from a dead one, which is the thing the hang already could not tell.
///
/// **The endpoint carrying the death** loses on the ordinary path. Pointing a job's fault target at
/// the endpoint the shell reads would work for a fault, and §26.3 flows *exits* down the same
/// endpoint too, so every ordinary job would leave a second message on the shell's result endpoint
/// behind its answer and the next command's read would take it. It also moves collection into the
/// shell for every job, and takes every job out of the progenitor's supervision domain, which is what
/// `ps`/`pgrep` read (DECISIONS §106 already records that cost as acceptable for one narrow stage
/// and it is not acceptable for all of them).
///
/// **The supervisor telling** costs one capability and one word. `job_undertaker` already receives
/// the death, already collects the corpse, and its own `BUGS` section already recorded that it "has
/// no way to say anything". This is that sentence answered.
///
/// Distinct from [`SPAWN_FAILED`] because the two are different facts a person needs told apart:
/// nothing ran, versus something ran and died. It sits one below `u64::MAX` for the same reason
/// that one sits at it, and the same caveat applies: no program in this tree answers with either.
///
/// Name: provisional, like everything a lane mints: a word in a protocol is exactly the kind of
/// name an architect decides.
pub const JOB_FAULTED: u64 = u64::MAX - 1;

/// **The badge on `job_undertaker`'s copy of the spawn endpoint** (milestone 685 (a job is finished
/// when its memory is back), option A, ruled by calef 2026-10-06 UTC: "A looks right."). A message
/// on the spawn endpoint carrying this badge is a [`reaped`] message, never a request.
///
/// The shell's copy is unbadged and carries `WRITE` alone, and `BADGE` needs `GRANT`
/// (`abi::rendezvous::BADGE`), so nothing but the copy the progenitor minted for the undertaker can
/// send with this badge. The kernel delivers the badge on a plain `SEND` too (milestone 613
/// (a system log service: the in-memory half)'s amendment to §230 (badged endpoint capabilities)), in `x3` for `RECEIVE` and `RECEIVE_CAP`
/// alike, which is why the progenitor can check it on every receive of a request, not only the
/// first.
///
/// **The wire layout**, three words of a plain `SEND`:
///
/// ```text
///   w0  label   the badge the progenitor put on the job's supervision capability (DECISIONS §148
///               (resolves by asking the kernel)), which the kernel returned beside the death;
///               0 for a job built without one
///   w1  tid     the dead thread's id, as the kernel stamped it on the death message
///   w2  0       reserved
/// ```
///
/// The label is what the progenitor keys on, because it is a name the progenitor chose before the
/// job ran: the progenitor holds a job's thread capability, not its thread id, so a tid alone would
/// name nothing it knows. The tid rides along because the ruling asked for it and a reader of the
/// trace wants it. `w0` and `w1` are the two words a `RECEIVE_CAP` returns as data (`x0`, `x2`),
/// so a reaped message arriving where the progenitor expected a delegated capability is read
/// whole.
///
/// **Sent after the collect, never before**: the message means the region is already back in the
/// pool, which is the whole of its use. Name: provisional.
pub const UNDERTAKER_BADGE: u64 = 1;

/// **The "reaped" message for job `label`, thread `tid`** ([`UNDERTAKER_BADGE`] has the layout).
#[must_use]
pub const fn reaped(label: u64, tid: u64) -> (u64, u64, u64) {
    (label, tid, 0)
}

/// **The bit that marks a badge as an installer endpoint's** (milestone 809 (the package client
/// becomes a program), DECISIONS §270 (a package manager holds an installer endpoint, not the spawn
/// endpoint)). The progenitor badges a copy of the spawn endpoint [`installer_badge`] of the job's
/// label for a program that declares `Manifest::installer`, narrowed to `WRITE`, and places it at
/// `crate::INSTALLER_SLOT`. A request carrying such a badge is served only if it is an
/// [`Activation::Install`], [`Activation::Remove`] or [`Activation::Rollback`]; anything else is
/// answered [`ActivationStatus::NotServedHere`] with nothing read past its first message. The
/// answer goes to the holder's own reply endpoint, `crate::INSTALLER_REPLY_SLOT`, because the
/// shell is reading the result endpoint while the holder runs.
///
/// The wire after the first message is [`activation_request`]'s, unchanged: only the endpoint it
/// arrives on is new. A label is never `0` and stays far below this bit (a counter from 1), and
/// [`UNDERTAKER_BADGE`] is `1` without it, so the three kinds of sender never share a badge: the
/// shell's copy is unbadged.
///
/// **A holder that promises frames and does not send them holds the progenitor** until it dies:
/// its reaped message ends the wait, and the request is abandoned. One that blocks without dying
/// holds it for good, the exposure every promise in this protocol has.
///
/// Name: provisional (milestone 809).
pub const INSTALLER_BADGE: u64 = 1 << 62;

/// The installer endpoint's badge for the job labeled `label` ([`INSTALLER_BADGE`]).
#[must_use]
pub const fn installer_badge(label: u64) -> u64 {
    INSTALLER_BADGE | label
}

/// **The job label an installer endpoint's badge carries**, or `None` for a badge that is not one
/// ([`INSTALLER_BADGE`]).
#[must_use]
pub const fn installer_label(badge: u64) -> Option<u64> {
    if badge & INSTALLER_BADGE != 0 {
        Some(badge & !INSTALLER_BADGE)
    } else {
        None
    }
}

/// **Whether the installer endpoint serves this verb** ([`INSTALLER_BADGE`]): install, remove and
/// rollback, and never a vouch.
#[must_use]
pub const fn installer_serves(verb: Activation) -> bool {
    matches!(
        verb,
        Activation::Install | Activation::Remove | Activation::Rollback
    )
}

/// **The word for bytes nobody vouched for** (DECISIONS §219, milestone 198 rung 3a). Sent on the
/// result endpoint by the progenitor when an `IMAGE_BIT` request's digest is not in the
/// installed-programs table, in place of [`SPAWN_FAILED`], because "nothing was built" and "it was refused
/// as unvouched" are different facts a person needs told apart: the first is out of memory, the
/// second is a decision.
///
/// A miss gets it unless the caller presented the run-unvouched capability ([`Wiring::run_unvouched`],
/// §219's gate D2). A presented miss is built instead, with [`crate::UNVOUCHED_MANIFEST`]: only what
/// the caller delegated, and the clock and configuration pages.
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
/// (milestone 715 (provisional)). Since 715 the spawn service holds the display devices and the
/// shell holds none, so the shell cannot find out from its own slots that there is nothing to
/// launch a session on; it asks, and this is the answer. Distinct from [`SPAWN_FAILED`] for
/// [`SPAWN_UNVOUCHED`]'s reason: "this boot has no display" is a fact about the machine, and "the
/// session could not be built" (a second session, the job pool spent) is a failure, and a person
/// needs them told apart. Four below `u64::MAX`, for [`JOB_FAULTED`]'s reason.
///
/// Mechanism: ruled 2026-10-04 (UTC, calef): the shell learns "no display" from this result word,
/// not from a fact it is told at `_start`. Name: provisional (milestone 715, 2026-10-03); the
/// ruling covered the mechanism only.
pub const SPAWN_NO_DISPLAY: u64 = u64::MAX - 4;

/// The ack the progenitor sends on the result endpoint when a **supervised** (interruptible) child started
/// cleanly. An interruptible child reports its own progress and exit through the shared job frame,
/// not the result endpoint, so the progenitor sends this once as the go-ahead: the shell reads it, then begins
/// watching the job frame. `0` is distinct from [`SPAWN_FAILED`].
pub const SPAWN_OK: u64 = 0;

#[cfg(test)]
mod tests {
    use super::*;

    /// The reaped message carries the label and the thread id, in that order, and a zero third
    /// word, so it cannot be read as a delegated capability's.
    #[test]
    fn a_reaped_message_is_label_then_thread() {
        assert_eq!(reaped(6, 9), (6, 9, 0));
    }

    /// A holder's name survives the two messages whole, at the full width and empty.
    #[test]
    fn a_holder_name_round_trips() {
        let mut out = [0u8; HOLDER_MAX];
        for name in ["duet", "", "a-package-name-that-fills-it-all"] {
            assert_eq!(holder_from_words(holder_words(name), &mut out), name);
        }
        let mut out = [0u8; HOLDER_MAX];
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
        assert_eq!(arg(w1), 9);
        assert_eq!(mem_pages(w2), 16);
        assert_eq!(wiring(w2), Wiring::default());
    }

    #[test]
    fn interruptible_bit_survives_a_zero_page_count() {
        // The interrupt demonstrators take no --mem, so the flag must ride independent of the count.
        let (_, _, w2) = request(
            2,
            0,
            0,
            Wiring {
                interruptible: true,
                ..Wiring::default()
            },
        );
        assert_eq!(mem_pages(w2), 0);
        assert!(is_interruptible(w2));
    }

    #[test]
    fn no_grant_is_zero_pages() {
        let (_, _, w2) = request(0, 5, 0, Wiring::default());
        assert_eq!(mem_pages(w2), 0);
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
        for m in 0u32..1 << 14 {
            let b = |i: u32| m & (1 << i) != 0;
            let w = Wiring {
                interruptible: b(0),
                sink: b(1),
                source: b(2),
                diagnostics: b(3),
                dir: b(4),
                dir2: b(5),
                screen: b(6),
                image: b(7),
                run_unvouched: b(8),
                args: b(9),
                nameset: b(10),
                machine: b(11),
                graphics: b(12),
                keyboard: b(13),
            };
            let (_, _, w2) = request(3, 0, 64, w);
            assert_eq!(wiring(w2), w, "{w:?}");
            assert_eq!(mem_pages(w2), 64, "{w:?}");
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
            dir: true,
            dir2: true,
            screen: true,
            image: true,
            run_unvouched: true,
            args: true,
            nameset: true,
            machine: true,
            graphics: true,
            keyboard: true,
        };
        let (_, w1, w2) = request(3, 2, 64, all);
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

    /// **`DIR2_BIT` follows [`DIR_BIT`]'s own precedent**: it is a second bit, not a count, and it
    /// round-trips independent of whether `dir` itself is set (nothing here enforces the "meaningless
    /// without `dir`" rule from the wire alone; that is the emitter's obligation, stated in
    /// [`Wiring::dir2`]'s own doc).
    #[test]
    fn a_second_directory_grant_is_a_second_bit_not_a_count() {
        let (_, _, w2) = request(
            0,
            0,
            0,
            Wiring {
                dir: true,
                dir2: true,
                ..Wiring::default()
            },
        );
        let w = wiring(w2);
        assert!(w.dir);
        assert!(w.dir2);
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
        assert_eq!(SHELL_BUDGET_PAGES, 128 + IMAGE_MAX_PAGES);
    }
}
