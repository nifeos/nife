//! **What a program may be granted, as types**: the output and input specs, option letters,
//! the slots the progenitor endows at, `Manifest`, `StartupContract`, and the region a `std` image is built
//! in.
//!
//! Moved out of the crate's root unchanged by milestone 842 (the grant-plan crate root is split
//! along its seams), as §272 (the grant-plan crate root splits into private modules) lays out; the
//! root re-exports every public item at its old path.
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

use crate::spawnproto;

/// **What a program's output slot carries.** Declared per program because this system has two
/// conventions for "a program said something" and they are not interchangeable; see
/// [`Manifest::output`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputSpec {
    /// Nothing. The program holds no output capability (the interrupt demonstrators report through
    /// a shared frame). Redirecting it is [`Refusal::NotAByteStream`](crate::Refusal::NotAByteStream), and so is piping it, because
    /// there is no slot to substitute.
    Silent,
    /// One or more raw `u64` answers on the result endpoint, read by the shell and rendered by it.
    /// Older than the sink contract and still right for an integer; not redirectable.
    Words,
    /// The sink contract (`crates/byte_sink_contract`): self-framing byte messages ending in `OPERATION_EOF`. The
    /// only output that `>` and `|` can substitute, because it is the only one whose meaning does
    /// not depend on who is reading it.
    Bytes,
    /// [`Bytes`](OutputSpec::Bytes) **and a second byte stream** for this program's own
    /// diagnostics, in the capability table slot named here (DECISIONS §67).
    ///
    /// This is what `2>` binds to, and the slot number is why it is a declaration rather than
    /// Unix's fd 2 with a capability underneath. Nothing in this system agrees in advance that any
    /// number means anything; the program says where its second stream goes, `caps` prints it, and a
    /// program that says nothing has no second stream to name.
    ///
    /// **The slot is high and out of the way on purpose**, which is `abi::fault::FAULT_EP_SLOT`'s
    /// reasoning applied a second time: a child's ordinary grants fill from zero upward and how many
    /// there are depends on the wiring (`date` gets a clock from the progenitor and none from the guest test
    /// harness), so a low number would move under the program and it would probe the wrong slot. A
    /// number nothing else can reach is the same number in every wiring.
    BytesAndDiagnostics {
        /// The capability table slot the diagnostic endpoint is inserted into.
        slot: u64,
    },
}

impl OutputSpec {
    /// **Whether `>` and `|` have anything to substitute**, which is what the operators' check asks.
    /// A declared second stream does not change the answer: the first stream is still the sink
    /// contract, and a redirection still names where *it* goes.
    pub fn is_byte_stream(self) -> bool {
        matches!(
            self,
            OutputSpec::Bytes | OutputSpec::BytesAndDiagnostics { .. }
        )
    }

    /// The capability table slot this program's declared second stream lands in, or `None` for a program that
    /// declares none. `Some` is the whole of what makes `2>` legal on a command line.
    pub fn diagnostics_slot(self) -> Option<u64> {
        match self {
            OutputSpec::BytesAndDiagnostics { slot } => Some(slot),
            _ => None,
        }
    }
}

/// **Whether a program reads an input stream**, milestone 50's `<` and the right-hand end of `|`.
///
/// The asymmetry with [`OutputSpec`] is deliberate and it is a fact about the two directions.
/// Output has three shapes because the system grew two of them before the sink contract existed.
/// Input has one shape, because nothing read a stream at all until this milestone: the source
/// contract is the sink contract received rather than sent, and there was never a chance for a
/// second convention to establish itself.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputSpec {
    /// The program reads nothing. Giving it an input is [`Refusal::InputForbidden`](crate::Refusal::InputForbidden) rather than a
    /// capability it would never receive on, because a slot nobody reads is authority that moved
    /// for no reason.
    Forbidden,
    /// The program reads an input stream and cannot do anything without one. A line that gives it
    /// none is [`Refusal::InputRequired`](crate::Refusal::InputRequired) **at the prompt**, which is a check Unix cannot make.
    Required {
        /// **Whether this program's output may begin before its input has ended.**
        ///
        /// `wc` counts to end of stream and then prints one line, so it is `false`. A renderer or a
        /// filter that transforms as it reads is `true`, and the difference is not a performance
        /// note: it decides whether **this shell** can run the program at all.
        ///
        /// A sink message is a rendezvous `SEND` and a process has exactly one wait point (no
        /// select and no poll. Milestone 106 (a wait that ends on either the interrupt or the
        /// deadline) added a timer and milestone 151 (notification objects) a notification binding, but a `SEND` still
        /// blocks on its one receiver alone). So a shell that is feeding a stage cannot also be
        /// receiving from it, and a stage that writes while it reads blocks against a shell that is
        /// still writing. Both stop, and the prompt never comes back.
        ///
        /// A program that declares `false` is a **barrier**: it absorbs the whole stream before it
        /// answers, which lets the shell finish feeding before anything comes back. One barrier
        /// anywhere in a chain is enough, which is why `mdr page.md | wc` runs and `mdr page.md`
        /// does not. See [`check_chain`](crate::check_chain) and notes/pipes.md.
        writes_while_reading: bool,
    },
}

/// A program that takes no short options. Spelled once so a manifest reads as a declaration rather
/// than as an empty literal repeated six times.
pub const NO_FLAGS: Flags = Flags::new(b"");

/// **The most short options one program may declare.** Sixteen, which is far above the three `rm`
/// declares and well inside the 64 bits of [`Endowment::flags`](crate::Endowment::flags). Not [`MAX_FLAGS`](crate::MAX_FLAGS), which bounds
/// what one line may type rather than what a program may accept.
///
/// A bound at all because a manifest is now also bytes a program carries (milestone 597,
/// provisional: a program carries its manifest in an ELF note), and a decoded manifest has to hold
/// its letters by value rather than point at a `'static` string the program's bytes are not.
pub const MAX_DECLARED_FLAGS: usize = 16;

/// **The short options a program declares, as their letters in bit order** ([`Manifest::flags`]).
///
/// A value rather than a `&'static [u8]` since milestone 597 (provisional): a manifest read out
/// of a program's own bytes has no `'static` string to point at, so the letters are held inline.
/// Every constructor checks the same three things, so a manifest compiled in and a manifest decoded
/// from a note cannot disagree about what a well-formed option set is: at most [`MAX_DECLARED_FLAGS`]
/// letters, each an ASCII letter or digit, and no letter twice (a repeated letter would give one
/// option two bits, and [`plan`](crate::plan) reads the first).
///
/// Name: provisional (milestone 597, 2026-09-26).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Flags {
    letters: [u8; MAX_DECLARED_FLAGS],
    len: u8,
}

impl Flags {
    /// The option set `letters` spells, or `None` if it breaks one of the three rules above.
    pub const fn try_new(letters: &[u8]) -> Option<Self> {
        if letters.len() > MAX_DECLARED_FLAGS {
            return None;
        }
        let mut out = [0u8; MAX_DECLARED_FLAGS];
        let mut i = 0;
        while i < letters.len() {
            let l = letters[i];
            if !l.is_ascii_alphanumeric() {
                return None;
            }
            let mut j = 0;
            while j < i {
                if letters[j] == l {
                    return None;
                }
                j += 1;
            }
            out[i] = l;
            i += 1;
        }
        Some(Self {
            letters: out,
            len: letters.len() as u8,
        })
    }

    /// [`try_new`](Self::try_new) for a manifest written in source, where a bad option set is a
    /// compile error rather than a value.
    pub const fn new(letters: &[u8]) -> Self {
        match Self::try_new(letters) {
            Some(f) => f,
            None => panic!("an option set is at most 16 distinct ASCII letters or digits"),
        }
    }

    /// The letters, in bit order.
    pub const fn letters(&self) -> &[u8] {
        self.letters.split_at(self.len as usize).0
    }
}

/// **The capability table slot a declared diagnostic stream lands in** (DECISIONS §67), for the programs that
/// declare one at all.
///
/// It is a constant here rather than a number each manifest picks, and that is not a retreat to an
/// ambient convention: what the manifest declares is **whether** there is a second stream, and the
/// slot is a placement detail every declarer may as well share. What matters is that a program that
/// declares nothing has no slot and no stream, which is where the model differs from fd 2.
///
/// **Eight**, which is above every ordinary grant (a child's are inserted from zero and the most
/// any manifest asks for is four) and below `abi::fault::FAULT_EP_SLOT` at fifteen. Out-of-the-way
/// is load-bearing: how many low slots a child gets depends on the wiring, so a low number would
/// move under the program. `abi::thread_control_block::CAP_INSERT`'s explicit target is what puts it there, and it
/// exists already, for the fault endpoint, for exactly this reason.
pub const DIAGNOSTICS_SLOT: u64 = 8;

/// **The capability table slot a process-domain capability lands in** (milestone 126), for the programs that
/// declare [`Manifest::domain`].
///
/// Placed rather than taken from the next free slot, for exactly the reason above: how many low
/// slots a child gets depends on what else the line granted it, and `ps` reads a fixed number. Seven
/// sits below [`DIAGNOSTICS_SLOT`] and above every ordinary grant, so the two named slots read as one
/// small block a person can hold in their head.
///
/// A number is not a right. What it means to hold this slot is decided by the capability the progenitor puts
/// in it, and a program spawned without the declaration holds an empty slot here and is refused by
/// the kernel rather than shown an empty domain.
pub const DOMAIN_SLOT: u64 = 7;

/// **The capability table slot a client view of the entropy service lands in** (milestone 111), for
/// the programs that declare [`Manifest::entropy`].
///
/// Nine, one past [`DIAGNOSTICS_SLOT`], so the three named slots stay one small contiguous block a
/// person can hold in their head. Placed rather than taken from the next free slot for the reason
/// the other two are: how many low slots a child gets depends on what else the line granted it, and
/// a program that reads a fixed number needs that number not to move.
///
/// **It clears the shell's own table as well as a child's**, which the other two named slots never
/// had to: the progenitor hands the shell up to seven positional capabilities (slots 0 through 6), so a
/// number below seven would have collided with the shell's clock on a boot with a filesystem and a
/// second directory grant. Nine is above every one of them and far below
/// `abi::fault::FAULT_EP_SLOT`.
///
/// A number is not a right. What it means to hold this slot is decided by the capability the progenitor puts
/// in it, and a program spawned without the declaration holds an empty slot here: its first `CALL`
/// answers `abi::Error::NoSuchSlot`, which `entropy_contract::delivered` reads as `None` rather than
/// as a count.
pub const ENTROPY_SLOT: u64 = 9;

/// **The capability table slot a client view of the network stack lands in** (milestone 590
/// (provisional)), for the programs that declare [`Manifest::network`].
///
/// Ten, one past [`ENTROPY_SLOT`], for that constant's reasons: a named slot rather than the next
/// free one, above the shell's own seven positional capabilities, and far below
/// `abi::fault::FAULT_EP_SLOT`. A program spawned without the declaration holds an empty slot here,
/// and its first `CALL` answers `abi::Error::NoSuchSlot`; `unreachable_network_witness` is the
/// program that shows it at the prompt.
pub const NETWORK_SLOT: u64 = 10;

/// **Where the boot shell holds the inert-configuration page** (milestone 47 (navigation and
/// naming), DECISIONS §111 (inert configuration is a validated page)), so `caps` can print the
/// values a child declaring [`Manifest::config`] would read rather than only that it would read
/// some.
///
/// The progenitor places the **same frame** it endows every such child with here, `READ` only, and
/// maps it at [`SHELL_CONFIG_VA`]. The same frame is the point: a preview that printed values from a
/// copy of the defaults would agree with the child by coincidence and drift from it the first time
/// either changed, while a second holder of one page cannot disagree with the first.
///
/// Twenty-one, one under `spawnproto::RUN_UNVOUCHED_SLOT`, for that constant's reason: a named
/// slot is probed at `_start`, before the shell has allocated anything, so the slot holds exactly
/// what its builder placed. A shell built by anything else (a `login` session, a kernel test role)
/// finds it empty and says it cannot see the values, which is true.
///
/// Name: provisional (milestone 47, 2026-09-26).
pub const SHELL_CONFIG_SLOT: u64 = 21;

/// **Where the boot shell's view of the inert-configuration page is mapped**, read-only
/// ([`SHELL_CONFIG_SLOT`]). One page above the shell's clock (`0x00d0_0000`), so both share one page
/// table and this mapping costs the builder no table page of its own. A constant here rather than
/// one in each binary, because the progenitor maps it and the shell reads through it, and rule 7 of
/// the codebase rules says an address two binaries agree on is a crate's.
///
/// Name: provisional (milestone 47, 2026-09-26).
pub const SHELL_CONFIG_VA: u64 = 0x0000_0000_00d0_1000;

/// **Where a program that declares [`Manifest::installer`] finds the installer endpoint**
/// (milestone 809 (the package client becomes a program), DECISIONS §270 (a package manager holds
/// an installer endpoint, not the spawn endpoint)). Fifteen, one past [`SYNC_SLOT`], for
/// [`ENTROPY_SLOT`]'s reasons, and above `std_runtime_contract`'s ten fixed slots, so the same
/// number in either layout. `WRITE` alone: the holder may send a request and may not receive
/// another's or pass the endpoint on.
///
/// Name: provisional (milestone 809).
pub const INSTALLER_SLOT: u64 = 15;

/// **Where the same program finds the endpoint the progenitor answers its requests on**
/// (milestone 809). `READ` alone. A request on the spawn endpoint is answered on the shell's
/// result endpoint, which the shell is reading while this program runs, so an installer request
/// is answered on an endpoint of the holder's own, made from its job's region when it is built.
///
/// Name: provisional (milestone 809).
pub const INSTALLER_REPLY_SLOT: u64 = 16;

/// **Where a program that declares [`Manifest::catalog`] finds the catalog page's capability**
/// (milestone 809), `READ`, beside its mapping at `std_runtime_contract::CATALOG_PAGE`. The
/// mapping is what the program reads; the slot is what it probes before reading, so a copy run
/// without the grant says so instead of faulting on an unmapped page. Seventeen, one past
/// [`INSTALLER_REPLY_SLOT`].
///
/// Name: provisional (milestone 809).
pub const CATALOG_SLOT: u64 = 17;

/// **Where a program that holds [`Manifest::indexes`] finds the index sources page's capability**
/// (milestone 801 (packages over the internet)), `READ`, beside its mapping at
/// `std_runtime_contract::INDEX_SOURCES_PAGE`, for [`CATALOG_SLOT`]'s reason. Eighteen, one past
/// it. The index store's directory grant is at `std_runtime_contract::FS_DIR_SLOT`, the `std`
/// layout's one directory slot.
///
/// Name: provisional 2026-10-12 (UTC), lane `milestone/801-jig-installs-from-the-index`.
pub const INDEX_SOURCES_SLOT: u64 = 18;

/// **Where a program that declares [`Manifest::machine`] finds the machine statistics page's
/// capability** (milestone 126 (the `procps` package), DECISIONS §225 (`free` sees the machine and
/// your share)). Eleven, one past [`NETWORK_SLOT`], for that constant's reasons. The page itself is
/// mapped at `machine_statistics_contract::PAGE_VA`; the capability is here so `caps` has a slot to
/// print and so the grant is visible in the table.
///
/// Name: ratified 2026-09-27 (calef, #1360's table).
pub const MACHINE_SLOT: u64 = 11;

/// **Where a program that declares [`Manifest::share`] finds its `ENUMERATE` view of this prompt's
/// job budget** (milestone 126, DECISIONS §225). Twelve, one past [`MACHINE_SLOT`].
///
/// Name: ratified 2026-09-27 (calef, #1360's table).
pub const SHARE_SLOT: u64 = 12;

/// **Where a program that declares [`Manifest::reboot`] finds the reboot object** (milestone 805,
/// DECISIONS §251). Thirteen, one past [`SHARE_SLOT`], for [`ENTROPY_SLOT`]'s reasons: a named slot
/// rather than the next free one, and far below `abi::fault::FAULT_EP_SLOT`. `WRITE` alone, so the
/// holder can invoke it and cannot hand it on.
///
/// Name: provisional (milestone 805).
pub const REBOOT_SLOT: u64 = 13;

/// **Where a program that declares [`Manifest::sync`] finds its sync-only capability**
/// (milestone 805). Fourteen, one past [`REBOOT_SLOT`], for [`ENTROPY_SLOT`]'s reasons. `WRITE`
/// alone, the right to `CALL` the file server.
///
/// Name: ratified 2026-10-06 (calef, #1783: "Approve sync for the file-server
/// request and its capability, and flush for the device cache only.").
pub const SYNC_SLOT: u64 = 14;

/// **What the progenitor endows a child whose bytes nobody vouched for**, when the caller presented
/// the run-unvouched capability (DECISIONS §219 gate D2, ruled by calef 2026-09-26: *"Yes, allow
/// the clock and config pages."*).
///
/// The ruling in one value. Of the authorities the progenitor endows from a manifest rather than
/// from the line (`clock`, `domain`, `config`, `entropy`, `network`, and since milestone 805
/// `reboot`), an unvouched child gets the two read-only pages and none of the rest: never the
/// process domain, the network, entropy, the reboot object or the file service. Everything else it holds is what the caller delegated, which is the output
/// alone: [`image_manifest`](crate::image_manifest) refuses unvouched bytes whose note asks the line to designate more.
///
/// Where the two pages land, for a native child: slot 0 the output, slot 1 the clock page (mapped
/// read-only at the address `date` reads), slot 2 the configuration page (at the address
/// `printenv` reads). The bytes' own declaration is not consulted; a program asking for more finds
/// the slot empty.
///
/// Name: provisional (milestone 198 rung 3a, 2026-09-26).
pub const UNVOUCHED_MANIFEST: Manifest = Manifest {
    arg: ArgSpec::Forbidden,
    mem: MemSpec::Forbidden,
    file: FileSpec::Forbidden,
    dir: DirSpec::Forbidden,
    flags: NO_FLAGS,
    output: OutputSpec::Bytes,
    input: InputSpec::Forbidden,
    reports: true,
    interruptible: false,
    clock: true,
    domain: false,
    config: true,
    entropy: false,
    network: false,
    machine: false,
    share: false,
    reboot: false,
    sync: false,
    installer: false,
    catalog: false,
    indexes: IndexStore::Forbidden,
    startup: StartupContract::Native,
};

/// **[`UNVOUCHED_MANIFEST`] for bytes built against nife's `std`** (milestone 205 (how a foreign
/// program is told what to do)): the same two pages, in the `std` layout (clock at slot 5,
/// configuration at slot 7), plus the line's words at slot 8, which §170 ruled carry no authority.
/// No entropy, network or domain, whatever the note asks. Name: provisional.
pub const UNVOUCHED_STD_MANIFEST: Manifest = Manifest {
    arg: ArgSpec::Words(WordGrant::ReadOnly),
    startup: StartupContract::Std,
    ..UNVOUCHED_MANIFEST
};

/// A program's expectation about the integer argument (`least_authority_demo 9`'s `9`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArgSpec {
    /// The program consumes an argument; omitting it is [`Refusal::ArgRequired`](crate::Refusal::ArgRequired).
    Required,
    /// The program takes no argument; supplying one is [`Refusal::ArgForbidden`](crate::Refusal::ArgForbidden).
    Forbidden,
    /// **The program hears the line's words as its argv, as bytes** (milestone 205 (how a foreign
    /// program is told what to do), DECISIONS §170). Everything typed after the program's name, up
    /// to the first operator, reaches it as one argument per word with its quotes taken off,
    /// preceded by the name itself as `argv[0]`. Nothing is classified: an option letter the shell
    /// has never heard of is the program's business, not a refusal, because §170 ruled that these
    /// bytes carry no authority. What a word may reach is decided by the grants alone, so a
    /// manifest declaring this declares no positional grant (`a_program_that_hears_words_places_no_positional_grant`).
    ///
    /// Only a program on the `std` layout can read an argv ([`StartupContract::Std`]); see [`argv`](crate::argv) for how
    /// the page is assembled. Name: provisional (2026-09-26).
    ///
    /// **A word that names something here is granted it, as the [`WordGrant`] says** (§170 clauses
    /// 2 to 5, milestone 205's designation half): see [`expand::Designation`](crate::expand::Designation) for which words those are.
    Words(WordGrant),
}

impl ArgSpec {
    /// Whether the program hears the line as its argv ([`ArgSpec::Words`]).
    pub const fn hears_words(&self) -> bool {
        matches!(self, ArgSpec::Words(_))
    }
}

/// **What a word that names an existing file is granted** (§170 (how a foreign program is told what
/// to do) clause 2, and clause 3's create), declared by a program that hears words. The field §170
/// calls the manifest's `words`, carried inside [`ArgSpec::Words`] so a program that hears no words
/// cannot declare one.
///
/// Every variant grants the named names and nothing else in their directory, with the walk below a
/// named directory (`filesystem_contract`'s `DESCEND`), because a program handed `src` means to
/// read what is in it. **An unvouched program gets [`WordGrant::ReadOnly`] whatever it declares**
/// (clause 4): [`UNVOUCHED_STD_MANIFEST`] is read-only. A mark on a word that widens it is clause 4's
/// other half, and its spelling is not ruled, so none is built.
///
/// §170's third shape, "the file's directory", is not a variant: a line says it by naming the
/// directory, `.` included (calef's N1 ruling, 2026-09-27T06:27Z: `rg pattern .`).
///
/// Name: provisional (2026-09-27).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WordGrant {
    /// Read, list and walk what was named. `rg`'s.
    ReadOnly,
    /// And write what was named.
    ReadWrite,
    /// And write it, and create a name that does not resolve yet (clause 3: `cc -o main`). A word
    /// that names nothing is still designated, as a name the program may create.
    Create,
}

impl WordGrant {
    /// The rights a caretaker asks the file service for, as `filesystem_contract::dir`'s bits. Spelled
    /// as numbers here because this crate does not link the filesystem contract (see its
    /// `Cargo.toml`); `word_rights_are_the_filesystem_contracts` pins them to its constants.
    pub const fn rights(self) -> u64 {
        const ENUMERATE: u64 = 1 << 0;
        const READ: u64 = 1 << 1;
        const WRITE: u64 = 1 << 2;
        const CREATE: u64 = 1 << 3;
        const DESCEND: u64 = 1 << 5;
        let read = ENUMERATE | READ | DESCEND;
        match self {
            WordGrant::ReadOnly => read,
            WordGrant::ReadWrite => read | WRITE,
            WordGrant::Create => read | WRITE | CREATE,
        }
    }
}

/// A program's expectation about a memory grant (`--mem N`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemSpec {
    /// The program takes no memory grant; supplying one is [`Refusal::MemForbidden`](crate::Refusal::MemForbidden).
    Forbidden,
    /// The program must be granted a budget in `[min, max]` pages. Omitting it is
    /// [`Refusal::MemRequired`](crate::Refusal::MemRequired); out of range is [`Refusal::MemOutOfRange`](crate::Refusal::MemOutOfRange).
    Required { min: u64, max: u64 },
}

/// A program's expectation about a **file grant**, milestone 31 phase 2.
///
/// **The manifest declares the direction; the command line designates the file.** That split is the
/// SHILL shape and it is deliberate: a program knows whether it needs to write (that is a property of
/// what it does), while *which* file is the human's business and belongs on the command line. So
/// `wc report.txt` reads and `tee report.txt` writes, with no flag either way, and the authority is
/// still exactly what the line says because the program's half is fixed and published.
///
/// This enum is also what decides that a bare token *is* a file at all. Milestone 47 removed the
/// `file:` prefix on the finding that it announced the visible half of the grant and was silent on
/// the half that matters: `least_authority_demo 5 extra` is refused because `least_authority_demo`'s manifest says `Forbidden`,
/// not because of any prefix. The manifest was doing all the work.
///
/// One file, not a list. A program that needs two files needs a manifest that says so, and that is a
/// later widening (`Required { count }`) rather than something to leave ambiguous now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileSpec {
    /// The program takes no file; naming one is [`Refusal::FileForbidden`](crate::Refusal::FileForbidden).
    Forbidden,
    /// The program is granted exactly one file, `writable` or not. Omitting it is
    /// [`Refusal::FileRequired`](crate::Refusal::FileRequired).
    Required { writable: bool },
}

/// A program's expectation about a **directory grant**, milestone 47's `rm -r`.
///
/// [`FileSpec`] narrows a directory capability down to one file. This is the rung above: the program
/// is granted a capability to **a whole directory**, because what it does cannot be expressed as one
/// file. `rm` is the case that forced it: removing a name is an operation on the *directory that
/// holds it*, and no per-file capability carries the authority to do it.
///
/// # The grant is the directory the name is in, and that is wider than the name
///
/// `rm logs/old` hands `rm` a capability to `logs`, not to `old`, because that is what the
/// filesystem contract can express: `UNLINK` resolves a name under a directory handle. So the
/// program could remove any name in that directory, and it removes the one it was told. That is a
/// real over-grant, it is declared rather than hidden, and it is the same gap globbing closes: the
/// answer is a directory capability attenuated to a **name set** (the caretaker `fs_file_caretaker`
/// already is for one name), which is a later lane. Until then the honest statement is that `rm` is
/// granted the directory, and `caps rm -r logs` prints exactly that before anything happens.
///
/// # Why the rights are not a mask here
///
/// The fields say what the program will *do*, not what the wire calls it. `filesystem_contract::dir`'s
/// six-rung ladder is the filesystem contract's vocabulary, and translating into it belongs to the
/// shell, which holds both contracts; keeping it out of `grant_plan` is [`MAX_FILE_NAME`](crate::MAX_FILE_NAME)'s rule, that a
/// command line can be checked without linking the filesystem.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DirSpec {
    /// The program takes no directory.
    Forbidden,
    /// The program is granted a capability to the directory that holds what the command names,
    /// carrying the authority to **take names out of it** and nothing else.
    ///
    /// `subtree_flag`, when the option it names is on the command line, widens that to walking and
    /// listing what is underneath, which is what a recursive removal needs at every level. The
    /// widening is the point rather than a convenience: **typing `-r` visibly hands over more**, and
    /// a program run without it holds a capability that cannot even see a subdirectory, let alone
    /// descend into one.
    Required { subtree_flag: Option<u8> },
}

/// A program's SHILL-style manifest: the endowment it declares it expects. The shell checks the
/// command's grants against this at spawn, so a mismatch is a refusal at the prompt rather than a
/// mystery hang inside a program that did not get what it needed. See notes/program-manifest.md.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Manifest {
    pub arg: ArgSpec,
    pub mem: MemSpec,
    /// A per-file grant, milestone 31 phase 2. See [`FileSpec`] for why the direction lives here
    /// and the name lives on the command line.
    pub file: FileSpec,
    /// A per-**directory** grant, milestone 47's `rm -r`. See [`DirSpec`]: the program is handed a
    /// capability to the directory holding what the command names, because what it does (taking a
    /// name out) is an operation on a directory and not on a file.
    pub dir: DirSpec,
    /// **The short options this program accepts**, as their letters, in the order their bits are
    /// numbered ([`Endowment::flags`](crate::Endowment::flags)). Empty for a program that takes none, which is every program
    /// but `rm` today.
    ///
    /// The manifest owns this for the same reason it owns the file's direction: which options exist
    /// is a property of what the program does, and the shell must be able to refuse `-q` at the
    /// prompt rather than pass a letter along for the program to ignore. An option is **not**
    /// authority by itself, which is why the parser may collect letters before it knows the
    /// program; what makes one matter here is that `subtree_flag` lets one *widen a grant*, and
    /// that widening is checked and printed before anything is spawned.
    pub flags: Flags,
    /// **What this program's output slot carries** (milestone 50). A slot is a capability, but what
    /// travels over it is a convention, and `>` and `|` only make sense for one of them: a program
    /// that answers with a number in a register cannot have a file put behind it, because a file
    /// sink would receive a word it has no way to read as bytes.
    ///
    /// Declaring it is what lets `least_authority_demo 5 > out.txt` be [`Refusal::NotAByteStream`](crate::Refusal::NotAByteStream) at the prompt
    /// instead of a file full of nothing. Unix has no equivalent because on Unix every program's
    /// stdout is bytes by construction; here the register fastpath is real and older than the sink
    /// contract, so the two conventions coexist and the manifest is where they are told apart.
    pub output: OutputSpec,
    /// **Whether this program reads an input stream** (milestone 50). See [`InputSpec`]: a program
    /// that declares [`InputSpec::Required`] and is given nothing would block forever on a receive,
    /// and that is a mistake the shell can catch at the prompt.
    pub input: InputSpec,
    /// Endowed with the shared result endpoint (so it can report back). Every phase-1 program
    /// reports; the field exists so a program that does not can drop the channel it never uses.
    pub reports: bool,
    /// Granted a per-job interrupt channel so `^C` can reach it (DECISIONS §24). A
    /// long-running or interactive program declares this and the shell wires the two-tier interrupt
    /// path for it; a program that finishes in one step (`least_authority_demo`) declares `false` and is simply
    /// waited on. "Granted by default to interactive programs" is expressed here, per program.
    pub interruptible: bool,
    /// **Endowed a read-only mapping of the wall clock** (milestone 51's wiring).
    ///
    /// The odd one out in this struct, and deliberately: every other field is about something the
    /// command line can designate, and this is about something it cannot. A clock is not a name a
    /// person types, so there is no token to place and no refusal to write; what the manifest is for
    /// here is telling **The progenitor** which children to endow, and telling a person reading `caps date`
    /// that the authority exists at all.
    ///
    /// It is a *read-only* mapping and nothing else, which is the whole of DECISIONS §43's split
    /// made concrete: a program declaring this can read the time and has no object through which it
    /// could set or propose one. See notes/date.md.
    pub clock: bool,
    /// **Endowed a view over the supervision domain it is spawned into** (milestone 126,
    /// notes/process-view.md).
    ///
    /// [`clock`](Manifest::clock)'s twin, and for the same reason: a process domain is not a name a
    /// person types, so there is no token to place and no refusal to write. What this field does is
    /// tell **The progenitor** which children to endow, and tell a person reading `caps ps` that the authority
    /// exists and how wide it is.
    ///
    /// It lands in [`DOMAIN_SLOT`] carrying `READ`, which is what `abi::rendezvous::SURVEY` takes. A
    /// program that does not declare it holds nothing there, and the kernel refuses its survey
    /// loudly instead of answering with an empty list: **that difference is the whole feature**, and
    /// `ps` prints the two apart.
    pub domain: bool,
    /// **Endowed a read-only mapping of the inert-configuration page** (milestone 47's
    /// environment-variable fork, DECISIONS §111; `crates/environment_contract`).
    ///
    /// [`clock`](Manifest::clock)'s twin again, for the identical reason: `TZ`/`LANG`/`TERM` are
    /// not something a command line designates, so there is no token to place and no refusal to
    /// write. What this field does is tell **The progenitor** which children to endow, and tell a person
    /// reading `caps printenv` that the authority exists at all. Before this field existed, the
    /// page was granted **unconditionally** to a `std` program by a kernel test harness standing
    /// in for a real customer (`system_tests/src/user/std_service.rs`); this is the manifest declaration
    /// that section's own "Not yet built" line named, now with a real, `no_std`, shell-spawnable
    /// customer ([`Program::Printenv`](crate::Program::Printenv)) declaring it.
    ///
    /// It is a *read-only* mapping, the same rung [`clock`](Manifest::clock) uses: a program
    /// declaring this can read the three inert values and has no object through which it could
    /// change what a shell hands its children. **Provisional field name.**
    pub config: bool,
    /// **Endowed a client view of the entropy service** (milestone 111; DECISIONS §44,
    /// `crates/entropy_contract`, notes/entropy.md).
    ///
    /// The fourth member of [`clock`](Manifest::clock)'s family and the first one that is an
    /// *endpoint a service answers on* rather than a page: randomness is not a name a person types,
    /// so there is no token to place and no refusal to write, and what this field does is tell
    /// **The progenitor** which children to endow and tell a person reading `caps uuid` that the program
    /// draws random numbers at all.
    ///
    /// It lands in [`ENTROPY_SLOT`] carrying `WRITE`, which on an endpoint is the right to `CALL`
    /// and nothing else: a declaring program may ask the service for bytes and may not receive
    /// another client's request, and it holds no `GRANT`, so it cannot hand randomness to anything
    /// it spawns. A program that does not declare it holds an empty slot there and its first
    /// `CALL` comes back as a kernel error, which `entropy_contract::delivered` reports as `None`
    /// rather than as a short reply: **that difference is the whole feature**, and it is what
    /// `disk_partitioner` already turns into `R_NO_ENTROPY` instead of writing a GPT full of
    /// counter-derived ids.
    ///
    /// **Ambient entropy would be ambient authority.** A shell that handed every child a random
    /// source would make a program's dependence on randomness invisible, which is the one property
    /// the service was built to have (DECISIONS §44).
    pub entropy: bool,
    /// **Endowed a client view of the network stack** (milestone 590 (provisional), the booted
    /// system starts its network stack; `crates/socket_contract`, `components/src/net_stack.rs`).
    ///
    /// [`entropy`](Manifest::entropy)'s shape exactly, one service over: an endpoint the progenitor
    /// holds because it built the service, placed at [`NETWORK_SLOT`] carrying `WRITE` for a child
    /// that declares it, and at no slot for one that does not. A network is not a name a person
    /// types, so nothing on the command line can ask for it; the declaration is the whole of who
    /// may reach the network from this prompt, and `caps` prints it before anything runs.
    ///
    /// `WRITE` is the right to `CALL` the socket contract. It is not `READ` (which would let a
    /// client take another client's request off the endpoint) and not `GRANT` (which would let it
    /// hand the network to anything it spawns). **It reaches no other program's sockets**: each
    /// socket is a capability the stack hands back when it is opened (§255 (each socket is its own
    /// capability)), so this front door makes sockets and names none. Until milestone 649 (every
    /// client of a network stack shares its socket numbers) a socket was a small integer every
    /// client shared, and this comment said two declaring programs could each operate the other's
    /// sockets. No inbound authority rides with it: the stack the progenitor
    /// builds holds `socket_contract::NO_LISTEN_GRANT`, so a declaring program can connect out and
    /// cannot listen.
    ///
    /// **Provisional field name.**
    pub network: bool,
    /// **Endowed the machine statistics page** (milestone 126, DECISIONS §225 part 2;
    /// `crates/machine_statistics_contract`): a read-only mapping at that crate's `PAGE_VA`, and
    /// the frame's capability at [`MACHINE_SLOT`] carrying `READ`.
    ///
    /// [`clock`](Manifest::clock)'s family again: how the box is doing is not something a command
    /// line designates, so this tells the progenitor which children to endow and tells a person
    /// reading `caps free` that the program sees the machine. §225 ruled it granted to every login
    /// by default and withholdable by the owner. The page travels with the session: the shell
    /// holds it at [`spawnproto::MACHINE_PAGE_SLOT`] and sends it with the request, so a session
    /// the owner withheld it from (`system_initializer::GRANT_MACHINE_PAGE`) spawns programs that
    /// print that they cannot see the machine rather than a machine of zero bytes.
    ///
    /// **Provisional field name.**
    pub machine: bool,
    /// **Endowed a view of this prompt's job budget, narrowed to `ENUMERATE`** (milestone 126,
    /// DECISIONS §225 part 1), at [`SHARE_SLOT`]: the region every job this shell runs is carved
    /// from, which `abi::memory_region::USAGE` can ask what was spent from and on what, and which
    /// the holder cannot spend, split or destroy. `free`'s "yours" line and `slabtop`'s table.
    ///
    /// **Provisional field name.**
    pub share: bool,
    /// **Endowed the reboot object** (milestone 805 (`reboot` at the prompt), DECISIONS §251
    /// (restarting the machine is a kernel object the progenitor hands out)), at [`REBOOT_SLOT`].
    ///
    /// The authority to restart the machine. The kernel grants the progenitor the one there is, and
    /// the progenitor endows it to a declaring child and to nothing else, so what this field does is
    /// tell **The progenitor** which children to endow and tell a person reading `caps reboot` that
    /// the program can restart the machine. A program spawned without the declaration holds an
    /// empty slot there, and the kernel refuses its invoke with `abi::Error::NoSuchSlot`.
    ///
    /// **It does not sync anything.** The kernel syncs nothing (§251), and the progenitor does
    /// not either: the program syncs for itself through [`Manifest::sync`], which it declares
    /// beside this.
    ///
    /// **Exactly one program declares it, and no image can** ([`image_can_carry`](crate::image_can_carry)), so "only one
    /// program can restart the machine" is a fact about this table rather than about what somebody
    /// installed. An unvouched child never gets it ([`UNVOUCHED_MANIFEST`]).
    ///
    /// **Provisional field name.**
    pub reboot: bool,
    /// **Endowed a sync-only capability to the file server** (milestone 805, the 2026-10-06
    /// amendment to §251), at [`SYNC_SLOT`]: an endpoint badged and bound by
    /// `filesystem_contract::fs::BIND_SYNC`, so it answers `fs::SYNC` and refuses every other
    /// verb. It names no directory and reaches no file. The progenitor makes one for the job from
    /// a client window the job holds until it is reaped, places it, and keeps no copy.
    ///
    /// Only `reboot` declares it, and no image can ([`image_can_carry`](crate::image_can_carry)): a sync stalls every
    /// client's queue, which is a lever this tree has handed out only to the program about to
    /// restart the machine. A boot with no writable filesystem leaves the slot empty, and a boot
    /// whose file server could not make one refuses the spawn rather than start `reboot` unable to
    /// sync.
    ///
    /// Name: ratified 2026-10-06 (calef, #1783: "Approve sync for the file-server
    /// request and its capability, and flush for the device cache only.").
    pub sync: bool,
    /// **Endowed the installer endpoint** (milestone 809 (the package client becomes a program),
    /// DECISIONS §270 (a package manager holds an installer endpoint, not the spawn endpoint)), at
    /// [`INSTALLER_SLOT`], and the endpoint its answers come back on at [`INSTALLER_REPLY_SLOT`].
    ///
    /// The installer endpoint is the progenitor's spawn endpoint, badged for this one job and
    /// narrowed to `WRITE`: on it the progenitor serves `spawnproto`'s activation request for
    /// install, remove and rollback, and refuses every other request, `vouch` and a spawn
    /// included. So the holder can change what is installed and cannot run anything, and it holds
    /// no `GRANT`, so it cannot hand the endpoint to anything it spawns. **Only the owner's console
    /// grants it**: the progenitor places it only for a request that arrived on the unbadged spawn
    /// endpoint, which the boot prompt alone holds (DECISIONS §221 (the boot prompt is the owner's
    /// console)), so a `login` session cannot pass it on. No image can declare it
    /// ([`image_can_carry`](crate::image_can_carry)).
    ///
    /// **Provisional field name** (the block's draft was `activation`).
    pub installer: bool,
    /// **Endowed a read-only copy of the image's package catalog** (milestone 809), mapped at
    /// `std_runtime_contract::CATALOG_PAGE`: the `<stem> <digest>` lines the image vouches for,
    /// `package_archive::CATALOG`, which the kernel's measurement of the archive covers. What a
    /// package manager resolves a name against and lists, until an index replaces it. Knowledge
    /// and not authority: the progenitor checks every install against its own copy.
    ///
    /// **Provisional field name.**
    pub catalog: bool,
    /// **Endowed the machine's index store** (milestone 801 (packages over the internet), and
    /// item 6 of milestone 809 (the package client becomes a program)): a directory grant on
    /// `activation/indexes`, placed where a line's own directory grant would go, and a read-only
    /// copy of the image's index sources (`package_index::SOURCES_ENTRY`) mapped at
    /// `std_runtime_contract::INDEX_SOURCES_PAGE`. The directory holds the index copy a package
    /// manager writes, which the progenitor then installs from: ruling I2 of 2026-10-06 (calef, "I
    /// would think jig"), under which the progenitor trusts the copy in a directory only the owner's
    /// console grants. See [`IndexStore`].
    ///
    /// Only the owner's console grants it, as [`installer`](Manifest::installer), and no image may
    /// declare it ([`image_can_carry`](crate::image_can_carry)). A line that designates a
    /// directory of its own (`jig install ./x.nifepkg`) is given that one instead: the `std` layout
    /// has one directory slot.
    ///
    /// **Provisional field name.**
    pub indexes: IndexStore,
    /// **Which runtime contract the program was built against**, and so where it expects each
    /// capability to be (milestone 595 (provisional)). See [`StartupContract`].
    ///
    /// Not an authority, and so not a row in `caps`: the same grants land in different slots. It
    /// is here because the progenitor has to know it before it builds the child, and the manifest
    /// is the one declaration that travels with a program, whether it comes from the archive or,
    /// under DECISIONS §219's option D, from a package.
    ///
    /// Name: ratified 2026-10-18 (calef: "Yes").
    pub startup: StartupContract,
}

/// **How much of the machine's index store a program holds** (milestone 801 (packages over the
/// internet)); [`Manifest::indexes`] says what the store is. `jig update` writes the copy, and
/// `jig list` and `jig install <name>` only read it, so a verb chooses between the two
/// ([`crate::verbs::ManifestAuthorities::INDEXES_WRITE`]).
///
/// Name: provisional 2026-10-12 (UTC), lane `milestone/801-jig-installs-from-the-index`, as are
/// the variants.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexStore {
    /// None of it. Every program but `jig`.
    Forbidden,
    /// The copies read, the sources page mapped.
    ReadOnly,
    /// The copies read and written.
    ReadWrite,
}

impl IndexStore {
    /// The rights the store's directory is opened with, as `filesystem_contract::dir`'s bits:
    /// [`WordGrant::ReadOnly`]'s for reading, and [`WordGrant::Create`]'s for writing, since the
    /// first `update` creates the copy. `None` for [`IndexStore::Forbidden`].
    pub const fn rights(self) -> Option<u64> {
        match self {
            IndexStore::Forbidden => None,
            IndexStore::ReadOnly => Some(WordGrant::ReadOnly.rights()),
            IndexStore::ReadWrite => Some(WordGrant::Create.rights()),
        }
    }
}

/// **Where a program expects its capabilities**: in the order its spawner lists them, or at the
/// slots nife's `std` fixes (milestone 595 (provisional)).
///
/// A native program reads its capabilities by position, and the progenitor fills them from slot 0
/// upward in the order `spawn_service` documents. A `std` program cannot: the code reading its
/// slots is the standard library, written once for every program, so each authority has a fixed
/// slot and an empty slot means "not given" (`crates/std_runtime_contract`). The two layouts
/// disagree about slot 0 (the output, or the heap's budget), so a `std` program built in the native
/// layout would take its stdout endpoint for an allocator and fail on its first `println!`.
///
/// Name: ratified 2026-10-18 (calef: "Yes"). The variants `Native` and `Std` keep their names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StartupContract {
    /// Capabilities in order, the output first. Every program in `components/` and `fixtures/`.
    Native,
    /// `std_runtime_contract`'s eight fixed slots, its three shared pages, and a heap carved from
    /// [`STD_REGION_PAGES`]. A program built with `-Zbuild-std` for a `*-unknown-nife`
    /// target.
    Std,
}

/// **The region the progenitor builds a `std` program in, and then hands it as its heap**, in pages
/// (milestone 595 (provisional)). The shell prints it in `caps` and the progenitor spends it, so it
/// is here, where both read it.
///
/// **One region, not a heap beside a job**, and the reason is the reap rather than tidiness. A
/// region that has been `SPLIT` cannot be `DESTROY`ed until its children are, so a heap split off
/// the job's region would stop `job_undertaker` reclaiming the job; and a heap split off the pool
/// beside it would be a second region nothing reclaims at all. So the child's address space, image,
/// stack and TCB come out of this region first, and the same region, narrowed to `WRITE`, is the
/// budget at `std_runtime_contract::MEMORY_REGION_SLOT` its allocator maps from. Its heap is what
/// the build left, and one reclaim returns all of it.
///
/// Sized as the 256 pages of heap the kernel test harness has given every `std` program since
/// milestone 27 (Rust `std` on the native ABI), `BUDGET_PAGES` in `system_tests/src/user/std_service.rs`, which is the budget
/// `std_exerciser`'s transcript is proven under, plus 128 for the build: about 40 pages of image,
/// 32 of stack, the page tables for the windows a `std` child touches, and a caretaker if a
/// directory grant rides along. It is not a measurement of any program's high-water mark, and it is
/// nowhere near what `rg` over a real tree needs. A budget a person sizes at the prompt would be
/// `--mem`'s shape, which no `std` manifest declares yet; §170 (how a foreign program is told what
/// to do) gave a `std` program its words and not a budget.
///
/// Name: provisional.
pub const STD_REGION_PAGES: u64 = 256 + 128;

/// **The region a named `std` program is built in**, in pages: [`STD_REGION_PAGES`], plus one
/// package's worth ([`spawnproto::IMAGE_MAX_PAGES`]) for a program that declares
/// [`Manifest::installer`] (milestone 809 (the package client becomes a program)).
///
/// A package manager reads a whole package before it sends it (it checks the digest first), and
/// what it sends are frames of its own: it maps one run of pages, reads the package into it, and
/// hands the progenitor a page of that run at a time (`abi::page_frame::SLICE`). So the run is
/// both its buffer and its frames, and it lives in the program's own region, which is built from
/// the progenitor's image pool rather than the job pool: what it holds is a package's bytes,
/// exactly what that pool is sized for, and the progenitor's copy of the same package is carved
/// beside it. The shell's own staging pages ([`spawnproto::SHELL_BUDGET_PAGES`]) are left for a
/// file run by its path.
///
/// Name: provisional.
pub const fn named_std_region_pages(m: &Manifest) -> u64 {
    STD_REGION_PAGES
        + if m.installer {
            spawnproto::IMAGE_MAX_PAGES
        } else {
            0
        }
}

/// **The region a file run by its path is built in**, in pages: what a named program of the same
/// runtime gets ([`STD_REGION_PAGES`], or `native` for the progenitor's native job), plus the
/// image's own pages (milestone 595 (the shell runs a `std` program), 2026-09-27).
///
/// A named program's region was sized around its image, and every one in the archive is small. A
/// file is not: `rg`'s image is nearly twice a `std` region by itself, so a fixed size would either
/// refuse it or leave it no heap. Adding the image keeps what a program has left after its build
/// the same whichever way it arrived, at the price of counting a small image's pages twice.
///
/// Name: provisional.
pub const fn image_region_pages(image_pages: u64, startup: StartupContract, native: u64) -> u64 {
    image_pages
        + match startup {
            StartupContract::Std => STD_REGION_PAGES,
            StartupContract::Native => native,
        }
}

#[cfg(test)]
mod tests;
