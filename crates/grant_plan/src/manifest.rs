//! **What a program may be granted, as types**: the output and input specs, option letters,
//! the slots the progenitor endows at, `Manifest`, `StartupContract`, and the region a `std` image is built
//! in.
//!
//! The crate root re-exports every public item (§272 (the grant-plan crate root splits into
//! private modules)).
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

use crate::spawnproto;

/// **What a program's output slot carries.** Declared per program because this system has two
/// conventions for "a program said something" and they are not interchangeable; see
/// [`Manifest::output`].
///
/// Name: ratified 2026-10-18 (calef: "Approve the family"), with the suffix `Declaration`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputDeclaration {
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
    /// [`Bytes`](OutputDeclaration::Bytes) **and a second byte stream** for this program's own
    /// diagnostics, in the capability table slot named here (DECISIONS §67).
    ///
    /// This is what `2>` binds to: the program says where its second stream goes, `caps` prints it,
    /// and a program that says nothing has no second stream to name. The slot is high and out of
    /// the way so it is the same in every wiring; see notes/program-manifest.md ("The named slots").
    BytesAndDiagnostics {
        /// The capability table slot the diagnostic endpoint is inserted into.
        slot: u64,
    },
}

impl OutputDeclaration {
    /// **Whether `>` and `|` have anything to substitute**, which is what the operators' check asks.
    /// A declared second stream does not change the answer: the first stream is still the sink
    /// contract, and a redirection still names where *it* goes.
    pub fn is_byte_stream(self) -> bool {
        matches!(
            self,
            OutputDeclaration::Bytes | OutputDeclaration::BytesAndDiagnostics { .. }
        )
    }

    /// The capability table slot this program's declared second stream lands in, or `None` for a program that
    /// declares none. `Some` is the whole of what makes `2>` legal on a command line.
    pub fn diagnostics_slot(self) -> Option<u64> {
        match self {
            OutputDeclaration::BytesAndDiagnostics { slot } => Some(slot),
            _ => None,
        }
    }
}

/// **Whether a program reads an input stream**, milestone 50's `<` and the right-hand end of `|`.
///
/// The asymmetry with [`OutputDeclaration`] is deliberate and it is a fact about the two directions.
/// Output has three shapes because the system grew two of them before the sink contract existed.
/// Input has one shape, because nothing read a stream at all until this milestone: the source
/// contract is the sink contract received rather than sent, and there was never a chance for a
/// second convention to establish itself.
///
/// Name: ratified 2026-10-18 (calef: "Approve the family"), with the suffix `Declaration`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputDeclaration {
    /// The program reads nothing. Giving it an input is [`Refusal::InputForbidden`](crate::Refusal::InputForbidden) rather than a
    /// capability it would never receive on, because a slot nobody reads is authority that moved
    /// for no reason.
    Forbidden,
    /// The program reads an input stream and cannot do anything without one. A line that gives it
    /// none is [`Refusal::InputRequired`](crate::Refusal::InputRequired) **at the prompt**, which is a check Unix cannot make.
    Required {
        /// **Whether this program's output may begin before its input has ended.** `wc` counts to
        /// end of stream and then prints one line, so it is `false`; a filter that transforms as it
        /// reads is `true`.
        ///
        /// It decides whether this shell can run a line at all: a `SEND` blocks on its one receiver,
        /// so a shell feeding a stage cannot also receive from it. A `false` stage is a barrier that
        /// absorbs the whole stream first, and a chain needs one ([`check_chain`](crate::check_chain)).
        /// See notes/pipes/one-wait-point.md.
        writes_while_reading: bool,
    },
}

/// A program that takes no short options. Spelled once so a manifest reads as a declaration rather
/// than as an empty literal repeated six times.
///
/// Name: ratified 2026-10-18 (calef: "Yes").
pub const NO_SHORT_OPTIONS: ShortOptions = ShortOptions::new(b"");

/// **The most short options one program may declare.** Sixteen, which is far above the three `rm`
/// declares and well inside the 64 bits of [`Endowment::short_options`](crate::Endowment::short_options). Not [`MAX_SHORT_OPTIONS`](crate::MAX_SHORT_OPTIONS), which bounds
/// what one line may type rather than what a program may accept.
///
/// A bound at all because a manifest is now also bytes a program carries (milestone 597,
/// provisional: a program carries its manifest in an ELF note), and a decoded manifest has to hold
/// its letters by value rather than point at a `'static` string the program's bytes are not.
///
/// Name: ratified 2026-10-18 (calef: "Yes").
pub const MAX_DECLARED_SHORT_OPTIONS: usize = 16;

/// **The short options a program declares, as their letters in bit order** ([`Manifest::short_options`]).
///
/// A value rather than a `&'static [u8]` since milestone 597 (provisional): a manifest read out
/// of a program's own bytes has no `'static` string to point at, so the letters are held inline.
/// Every constructor checks the same three things, so a manifest compiled in and a manifest decoded
/// from a note cannot disagree about what a well-formed option set is: at most [`MAX_DECLARED_SHORT_OPTIONS`]
/// letters, each an ASCII letter or digit, and no letter twice (a repeated letter would give one
/// option two bits, and [`plan`](crate::plan) reads the first).
///
/// Name: ratified 2026-10-18 (calef: "Yes").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShortOptions {
    letters: [u8; MAX_DECLARED_SHORT_OPTIONS],
    len: u8,
}

impl ShortOptions {
    /// The option set `letters` spells, or `None` if it breaks one of the three rules above.
    pub const fn try_new(letters: &[u8]) -> Option<Self> {
        if letters.len() > MAX_DECLARED_SHORT_OPTIONS {
            return None;
        }
        let mut out = [0u8; MAX_DECLARED_SHORT_OPTIONS];
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
/// One constant every declarer shares: the manifest declares whether there is a second stream, and
/// a program that declares none has no slot. Eight, above every ordinary grant, placed by
/// `abi::thread_control_block::CAP_INSERT`'s explicit target. See notes/program-manifest.md ("The
/// named slots").
pub const DIAGNOSTICS_SLOT: u64 = 8;

/// **The capability table slot a process-domain capability lands in** (milestone 126), for the programs that
/// declare [`Manifest::domain`].
///
/// Seven, just below [`DIAGNOSTICS_SLOT`], placed for that constant's reason. A program spawned
/// without the declaration holds an empty slot here and is refused by the kernel rather than shown
/// an empty domain.
pub const DOMAIN_SLOT: u64 = 7;

/// **The capability table slot a client view of the entropy service lands in** (milestone 111), for
/// the programs that declare [`Manifest::entropy`].
///
/// Nine, one past [`DIAGNOSTICS_SLOT`], placed for that constant's reason and above the shell's own
/// seven positional capabilities. A program spawned without the declaration holds an empty slot
/// here: its first `CALL` answers `abi::Error::NoSuchSlot`, which `entropy_contract::delivered`
/// reads as `None`. See notes/program-manifest.md ("The named slots").
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
/// values a child declaring [`Manifest::configuration`] would read rather than only that it would read
/// some.
///
/// The progenitor places the **same frame** it endows every such child with here, `READ` only, and
/// maps it at [`SHELL_CONFIGURATION_VIRTUAL_ADDRESS`], so the preview cannot drift from the child.
/// A shell built by anything else (a `login` session, a kernel test role) finds it empty and says it
/// cannot see the values. See notes/program-manifest.md ("The named slots").
///
/// Name: ratified 2026-10-18 (calef: "Approve the family").
pub const SHELL_CONFIGURATION_SLOT: u64 = 21;

/// **Where the boot shell's view of the inert-configuration page is mapped**, read-only
/// ([`SHELL_CONFIGURATION_SLOT`]). One page above the shell's clock (`0x00d0_0000`), so both share one page
/// table and this mapping costs the builder no table page of its own. A constant here rather than
/// one in each binary, because the progenitor maps it and the shell reads through it, and rule 7 of
/// the codebase rules says an address two binaries agree on is a crate's.
///
/// Name: ratified 2026-10-18 (calef: "Approve the family", with `VIRTUAL_ADDRESS` spelled out).
pub const SHELL_CONFIGURATION_VIRTUAL_ADDRESS: u64 = 0x0000_0000_00d0_1000;

/// **Where a program that declares [`Manifest::installer`] finds the installer endpoint**
/// (milestone 809 (the package client becomes a program), DECISIONS §270 (a package manager holds
/// an installer endpoint, not the spawn endpoint)). Fifteen, one past [`SYNC_SLOT`], for
/// [`ENTROPY_SLOT`]'s reasons, and above `std_runtime_contract`'s fixed low slots, so the same
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
/// from the line, an unvouched child gets the two read-only pages and none of the rest. Everything
/// else it holds is what the caller delegated, the output alone (slot 0, then the clock page and
/// the configuration page, for a native child). The bytes' own declaration is not consulted, and
/// [`image_manifest`](crate::image_manifest) refuses one that asks the line to designate more.
///
/// Name: provisional (milestone 198 rung 3a, 2026-09-26).
pub const UNVOUCHED_MANIFEST: Manifest = Manifest {
    program_argument: ProgramArgumentDeclaration::Forbidden,
    memory_grant: MemoryGrantDeclaration::Forbidden,
    file: FileGrantDeclaration::Forbidden,
    directory: DirectoryGrantDeclaration::Forbidden,
    short_options: NO_SHORT_OPTIONS,
    output: OutputDeclaration::Bytes,
    input: InputDeclaration::Forbidden,
    reports: true,
    interruptible: false,
    clock: true,
    domain: false,
    configuration: true,
    entropy: false,
    network: false,
    machine: false,
    share: false,
    reboot: false,
    sync: false,
    installer: false,
    catalog: false,
    indexes: IndexStoreDeclaration::Forbidden,
    activation_directory: false,
    console_input: false,
    startup: StartupContract::Native,
};

/// **[`UNVOUCHED_MANIFEST`] for bytes built against nife's `std`** (milestone 205 (how a foreign
/// program is told what to do)): the same two pages, in the `std` layout (clock at slot 5,
/// configuration at slot 7), plus the line's words at slot 8, which §170 ruled carry no authority.
/// No entropy, network or domain, whatever the note asks. Name: provisional.
pub const UNVOUCHED_STD_MANIFEST: Manifest = Manifest {
    program_argument: ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
    startup: StartupContract::Std,
    ..UNVOUCHED_MANIFEST
};

/// A program's expectation about the integer argument (`least_authority_demo 9`'s `9`).
///
/// Name: ratified 2026-10-18 (calef: "Approve the family"), with the suffix `Declaration`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProgramArgumentDeclaration {
    /// The program consumes an argument; omitting it is [`Refusal::ProgramArgumentRequired`](crate::Refusal::ProgramArgumentRequired).
    Required,
    /// The program takes no argument; supplying one is [`Refusal::ProgramArgumentForbidden`](crate::Refusal::ProgramArgumentForbidden).
    Forbidden,
    /// **The program hears the line's words as its argv, as bytes** (milestone 205 (how a foreign
    /// program is told what to do), DECISIONS §170). Every word after the program's name, up to the
    /// first operator, reaches it unquoted after `argv[0]`, unclassified, since §170 ruled these
    /// bytes carry no authority; such a manifest declares no positional grant. A word that names
    /// something is granted it as the [`WordGrant`] says ([`expand::Designation`](crate::expand::Designation)).
    /// Only a `std` program can read an argv ([`argv`](crate::argv)). Name: provisional (2026-09-26).
    Words(WordGrant),
}

impl ProgramArgumentDeclaration {
    /// Whether the program hears the line as its argv ([`ProgramArgumentDeclaration::Words`]).
    pub const fn hears_words(&self) -> bool {
        matches!(self, ProgramArgumentDeclaration::Words(_))
    }
}

/// **What a word that names an existing file is granted** (§170 (how a foreign program is told what
/// to do) clause 2, and clause 3's create): §170's `words`, carried inside
/// [`ProgramArgumentDeclaration::Words`] so a program that hears no words cannot declare one.
///
/// Every variant grants the named names and nothing else in their directory, with the walk below a
/// named directory (`DESCEND`). An unvouched program gets [`WordGrant::ReadOnly`] whatever it
/// declares (clause 4); a mark that widens a word is unruled and unbuilt. §170's "the file's
/// directory" is not a variant: a line names the directory, `.` included (calef's N1 ruling,
/// 2026-09-27T06:27Z: `rg pattern .`).
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
///
/// Name: ratified 2026-10-18 (calef: "Approve the family"), with the suffix `Declaration`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemoryGrantDeclaration {
    /// The program takes no memory grant; supplying one is [`Refusal::MemoryGrantForbidden`](crate::Refusal::MemoryGrantForbidden).
    Forbidden,
    /// The program must be granted a budget in `[min, max]` pages. Omitting it is
    /// [`Refusal::MemoryGrantRequired`](crate::Refusal::MemoryGrantRequired); out of range is [`Refusal::MemoryGrantOutOfRange`](crate::Refusal::MemoryGrantOutOfRange).
    Required { min: u64, max: u64 },
}

/// A program's expectation about a **file grant**, milestone 31 (a capability shell: designation is authorization) phase 2.
///
/// The manifest declares the direction and the command line designates the file, so `wc
/// report.txt` reads and `tee report.txt` writes with no flag either way. This enum also decides
/// that a bare token is a file at all. One file, not a list: a program needing two needs a manifest
/// that says so. See notes/program-manifest.md ("The file endowment declares a direction").
///
/// Name: ratified 2026-10-18 (calef: "Approve the family"), with the suffix `Declaration`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileGrantDeclaration {
    /// The program takes no file; naming one is [`Refusal::FileForbidden`](crate::Refusal::FileForbidden).
    Forbidden,
    /// The program is granted exactly one file, `writable` or not. Omitting it is
    /// [`Refusal::FileRequired`](crate::Refusal::FileRequired).
    Required { writable: bool },
}

/// A program's expectation about a **directory grant**, milestone 47's `rm -r`.
///
/// The rung above [`FileGrantDeclaration`]: the program is granted a capability to the directory
/// that holds what the command names, because removing a name is an operation on that directory.
/// The planned grant attenuates it to the names designated ([`DirectoryGrant`](crate::DirectoryGrant)).
/// The fields say what the program will do, not the filesystem contract's rights: the shell
/// translates, so a command line is checked without linking the filesystem.
///
/// Name: ratified 2026-10-18 (calef: "Approve the family"), with the suffix `Declaration`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DirectoryGrantDeclaration {
    /// The program takes no directory.
    Forbidden,
    /// The program is granted a capability to the directory that holds what the command names,
    /// carrying the authority to **take names out of it** and nothing else.
    ///
    /// `subtree_option`, when the option it names is on the command line, widens that to walking and
    /// listing what is underneath, which is what a recursive removal needs at every level. The
    /// widening is the point rather than a convenience: **typing `-r` visibly hands over more**, and
    /// a program run without it holds a capability that cannot even see a subdirectory, let alone
    /// descend into one.
    ///
    /// Name: ratified 2026-10-18 (calef: "Yes"). This covers the `subtree_option` field.
    Required { subtree_option: Option<u8> },
}

/// A program's SHILL-style manifest: the endowment it declares it expects. The shell checks the
/// command's grants against this at spawn, so a mismatch is a refusal at the prompt rather than a
/// mystery hang inside a program that did not get what it needed. See notes/program-manifest.md.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Manifest {
    pub program_argument: ProgramArgumentDeclaration,
    pub memory_grant: MemoryGrantDeclaration,
    /// A per-file grant, milestone 31 phase 2. See [`FileGrantDeclaration`] for why the direction lives here
    /// and the name lives on the command line.
    pub file: FileGrantDeclaration,
    /// A per-**directory** grant, milestone 47's `rm -r`. See [`DirectoryGrantDeclaration`]: the program is handed a
    /// capability to the directory holding what the command names, because what it does (taking a
    /// name out) is an operation on a directory and not on a file.
    pub directory: DirectoryGrantDeclaration,
    /// **The short options this program accepts**, as their letters, in the order their bits are
    /// numbered ([`Endowment::short_options`](crate::Endowment::short_options)). Empty for a program that takes none, which is every program
    /// but `rm` today.
    ///
    /// The manifest owns this so the shell refuses `-q` at the prompt rather than pass a letter
    /// along to be ignored. An option is not authority by itself, but `subtree_option` lets one
    /// widen a grant, and that widening is checked and printed before anything is spawned.
    ///
    /// Name: ratified 2026-10-18 (calef: "Yes").
    pub short_options: ShortOptions,
    /// **What this program's output slot carries** (milestone 50). A slot is a capability, but what
    /// travels over it is a convention, and `>` and `|` only make sense for one of them: a program
    /// that answers with a number in a register cannot have a file put behind it, because a file
    /// sink would receive a word it has no way to read as bytes.
    ///
    /// Declaring it is what lets `least_authority_demo 5 > out.txt` be [`Refusal::NotAByteStream`](crate::Refusal::NotAByteStream) at the prompt
    /// instead of a file full of nothing. Unix has no equivalent because on Unix every program's
    /// stdout is bytes by construction; here the register fastpath is real and older than the sink
    /// contract, so the two conventions coexist and the manifest is where they are told apart.
    pub output: OutputDeclaration,
    /// **Whether this program reads an input stream** (milestone 50 (pipes and redirection)). See [`InputDeclaration`]: a program
    /// that declares [`InputDeclaration::Required`] and is given nothing would block forever on a receive,
    /// and that is a mistake the shell can catch at the prompt.
    pub input: InputDeclaration,
    /// Endowed with the shared result endpoint (so it can report back). Every phase-1 program
    /// reports; the field exists so a program that does not can drop the channel it never uses.
    pub reports: bool,
    /// Granted a per-job interrupt channel so `^C` can reach it (DECISIONS §24). A
    /// long-running or interactive program declares this and the shell wires the two-tier interrupt
    /// path for it; a program that finishes in one step (`least_authority_demo`) declares `false` and is simply
    /// waited on. "Granted by default to interactive programs" is expressed here, per program.
    pub interruptible: bool,
    /// **Endowed a read-only mapping of the wall clock** (milestone 51's wiring), with no object
    /// through which to set or propose the time (DECISIONS §43). The first of the fields for what a
    /// command line cannot designate; see notes/program-manifest.md ("What the line cannot name")
    /// and notes/date.md.
    pub clock: bool,
    /// **Endowed a view over the supervision domain it is spawned into** (milestone 126,
    /// notes/process-view.md).
    ///
    /// [`clock`](Manifest::clock)'s family. It lands in [`DOMAIN_SLOT`] carrying `READ`, which is
    /// what `abi::rendezvous::SURVEY` takes; without it the kernel refuses the survey, and `ps`
    /// prints that apart from an empty domain.
    pub domain: bool,
    /// **Endowed a read-only mapping of the inert-configuration page** (milestone 47's
    /// environment-variable fork, DECISIONS §111; `crates/environment_contract`).
    ///
    /// [`clock`](Manifest::clock)'s family: a read-only mapping of the three inert values
    /// (`TZ`, `LANG`, `TERM`), with no object through which to change what a shell hands its
    /// children. [`Program::Printenv`](crate::Program::Printenv) declares it.
    ///
    /// Name: ratified 2026-10-18 (calef: "Approve the family").
    pub configuration: bool,
    /// **Endowed a client view of the entropy service** (milestone 111; DECISIONS §44,
    /// `crates/entropy_contract`, notes/entropy.md).
    ///
    /// [`clock`](Manifest::clock)'s family, as an endpoint rather than a page. It lands in
    /// [`ENTROPY_SLOT`] carrying `WRITE` alone, the right to `CALL`: no other client's request, and
    /// no `GRANT` to pass it on. Without it the first `CALL` fails and
    /// `entropy_contract::delivered` reports `None`. Ambient entropy would be ambient authority
    /// (DECISIONS §44); see notes/program-manifest.md ("What the line cannot name").
    pub entropy: bool,
    /// **Endowed a client view of the network stack** (milestone 590 (provisional), the booted
    /// system starts its network stack; `crates/socket_contract`, `components/src/net_stack.rs`).
    ///
    /// [`entropy`](Manifest::entropy)'s shape, one service over: `WRITE` at [`NETWORK_SLOT`], the
    /// right to `CALL` the socket contract, and the declaration is the whole of who may reach the
    /// network from this prompt. It reaches no other program's sockets, each being its own
    /// capability (§255 (each socket is its own capability)), and the stack holds
    /// `socket_contract::NO_LISTEN_GRANT`, so a declaring program can connect out and cannot listen.
    ///
    /// **Provisional field name.**
    pub network: bool,
    /// **Endowed the machine statistics page** (milestone 126, DECISIONS §225 part 2;
    /// `crates/machine_statistics_contract`): a read-only mapping at that crate's `PAGE_VA`, and
    /// the frame's capability at [`MACHINE_SLOT`] carrying `READ`.
    ///
    /// [`clock`](Manifest::clock)'s family. §225 grants it to every login by default, withholdable
    /// by the owner (`system_initializer::GRANT_MACHINE_PAGE`): the shell holds it at
    /// [`spawnproto::MACHINE_PAGE_SLOT`] and sends it with the request, so a withheld session's
    /// programs say they cannot see the machine.
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
    /// The authority to restart the machine: the kernel grants the progenitor the one there is.
    /// Without the declaration the kernel refuses the invoke with `abi::Error::NoSuchSlot`. It syncs
    /// nothing (§251); the program syncs through [`Manifest::sync`]. Exactly one program declares
    /// it, and no image can ([`image_can_carry`](crate::image_can_carry)). See
    /// notes/program-manifest.md ("What the line cannot name").
    ///
    /// **Provisional field name.**
    pub reboot: bool,
    /// **Endowed a sync-only capability to the file server** (milestone 805, the 2026-10-06
    /// amendment to §251), at [`SYNC_SLOT`]: an endpoint badged and bound by
    /// `filesystem_contract::fs::BIND_SYNC`, so it answers `fs::SYNC` and refuses every other
    /// verb. It names no directory and reaches no file. The progenitor makes one for the job from
    /// a client window the job holds until it is reaped, places it, and keeps no copy.
    ///
    /// Only `reboot` declares it, and no image can ([`image_can_carry`](crate::image_can_carry)),
    /// because a sync stalls every client's queue.
    ///
    /// Name: ratified 2026-10-06 (calef, #1783: "Approve sync for the file-server
    /// request and its capability, and flush for the device cache only.").
    pub sync: bool,
    /// **Endowed the installer endpoint** (milestone 809 (the package client becomes a program),
    /// DECISIONS §270 (a package manager holds an installer endpoint, not the spawn endpoint)), at
    /// [`INSTALLER_SLOT`], and the endpoint its answers come back on at [`INSTALLER_REPLY_SLOT`].
    ///
    /// The progenitor's spawn endpoint, badged for this job and narrowed to `WRITE`: it serves
    /// install, remove and rollback and refuses every other request, so the holder can change what
    /// is installed and cannot run anything. Only the owner's console grants it (DECISIONS §221 (the
    /// boot prompt is the owner's console)); no image can declare it. See notes/program-manifest.md.
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
    /// `std_runtime_contract::INDEX_SOURCES_PAGE`. The progenitor installs from the copy a package
    /// manager writes there (ruling I2; see notes/program-manifest.md). Only the owner's console
    /// grants it and no image may declare it; a line that designates its own directory
    /// (`jig install ./x.nifepkg`) gets that one instead, the `std` layout having one directory slot.
    ///
    /// **Provisional field name.**
    pub indexes: IndexStoreDeclaration,
    /// **Endowed the activation directory, read only** (milestone 809 (the package client becomes
    /// a program)): a directory grant on `activation/`, at the `std` layout's one directory slot in
    /// place of the index store. It holds the installed-programs table (`installed_programs`) and,
    /// beneath it, the index store, so one grant shows what is installed and what the index copy
    /// offers. `jig list -u` holds it, to say which installed packages the index offers newer.
    ///
    /// Read only, and still the owner's console's to grant, as [`indexes`](Manifest::indexes) is.
    /// No image or note may carry it, and a verb holding it holds no index store
    /// ([`narrow_by_verb`](crate::narrow_by_verb)), since there is one slot.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane `milestone/809-jig-list-u`. Not
    /// `activation`, which was the draft spelling of [`installer`](Manifest::installer).
    pub activation_directory: bool,
    /// **Whether the program's standard input is the owner's console** (milestone 892 (a std
    /// program at the prompt holds the network and a resolver); calef, 2026-10-19 (UTC), on #2405:
    /// "a"). The spawn service places the terminal endpoint, badged
    /// `line_editor::proto::STDIN_BADGE`, at `std_runtime_contract::STDIN_SLOT`, and the program reads
    /// the lines typed while it runs: what `jig install` asks the owner's yes through. Only a
    /// program in this table may hold it, as only the owner's console grants the installer: no
    /// image may declare it ([`image_can_carry`](crate::image_can_carry)) and no note may carry it.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
    pub console_input: bool,
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
/// Name: ratified 2026-10-18 (calef: "Approve the family"), with the suffix `Declaration`.
/// The variants are provisional (lane `milestone/801-jig-installs-from-the-index`, 2026-10-12 UTC).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexStoreDeclaration {
    /// None of it. Every program but `jig`.
    Forbidden,
    /// The copies read, the sources page mapped.
    ReadOnly,
    /// The copies read and written.
    ReadWrite,
}

impl IndexStoreDeclaration {
    /// The rights the store's directory is opened with, as `filesystem_contract::dir`'s bits:
    /// [`WordGrant::ReadOnly`]'s for reading, and [`WordGrant::Create`]'s for writing, since the
    /// first `update` creates the copy. `None` for [`IndexStoreDeclaration::Forbidden`].
    pub const fn rights(self) -> Option<u64> {
        match self {
            IndexStoreDeclaration::Forbidden => None,
            IndexStoreDeclaration::ReadOnly => Some(WordGrant::ReadOnly.rights()),
            IndexStoreDeclaration::ReadWrite => Some(WordGrant::Create.rights()),
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
    /// `std_runtime_contract`'s fixed slots, its shared pages, and a heap carved from
    /// [`STD_REGION_PAGES`]. A program built with `-Zbuild-std` for a `*-unknown-nife`
    /// target.
    Std,
}

/// **The region the progenitor builds a `std` program in, and then hands it as its heap**, in pages
/// (milestone 595 (provisional)). The shell prints it in `caps` and the progenitor spends it, so it
/// is here, where both read it.
///
/// One region, so one reclaim returns all of it: the child is built from it first, and the same
/// region, narrowed to `WRITE`, is its allocator's budget at
/// `std_runtime_contract::MEMORY_REGION_SLOT`. 256 pages of heap, the test harness's
/// `BUDGET_PAGES`, plus 128 for the build; not a measured high-water mark. See
/// notes/grant-expression/grant-plan-crate.md ("The `std` region").
///
/// Name: provisional.
pub const STD_REGION_PAGES: u64 = 256 + 128;

/// **The region a named `std` program is built in**, in pages: [`STD_REGION_PAGES`], plus one
/// package's worth ([`spawnproto::IMAGE_MAX_PAGES`]) for a program that declares
/// [`Manifest::installer`] (milestone 809 (the package client becomes a program)).
///
/// A package manager reads a whole package into one run of its own pages before it sends it a page
/// at a time, so the run is its buffer and its frames. It comes from the progenitor's image pool,
/// which is sized for a package's bytes. See notes/grant-expression/grant-plan-crate.md ("The `std`
/// region").
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
