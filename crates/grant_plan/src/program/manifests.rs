//! **What each program is granted**: `Prog::verbs` and `Prog::manifest`, one arm per program.
//! A child file of `program`, not a module of its own, for the reason that module's header gives.
//!
//! Moved out of the crate's root unchanged by milestone 842 (the grant-plan crate root is split
//! along its seams), as §272 (the grant-plan crate root splits into private modules) lays out; the
//! root re-exports every public item at its old path.
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

use crate::{
    DIAGNOSTICS_SLOT, DirectoryGrantDeclaration, FileGrantDeclaration, Flags,
    IndexStoreDeclaration, InputDeclaration, Manifest, MemoryGrantDeclaration, NO_FLAGS,
    OutputDeclaration, Prog, ProgramArgumentDeclaration, Runtime, VerbGrant, WordGrant, verbs,
};

impl Prog {
    /// **What each verb of this program is granted, when its manifest is narrowed by the line's
    /// first word** (milestone 809 (the package client becomes a program), its option V2): empty
    /// for every program but one, which is then endowed exactly [`manifest`](Prog::manifest) as
    /// always. [`narrow_by_verb`](crate::narrow_by_verb) reads it, and the shell and the progenitor both call that.
    pub const fn verbs(self) -> &'static [VerbGrant] {
        match self {
            Prog::Jig => verbs::JIG_VERBS,
            _ => &[],
        }
    }

    /// **The zone this program may resolve names in, when its manifest is narrowed to hold the
    /// network** (milestone 892 (a std program at the prompt holds the network and a resolver)):
    /// dotted text, empty for the root zone, which is every name. `None` for every program that
    /// resolves nothing, which is every program but one.
    ///
    /// On this table rather than in [`Manifest`], so installed bytes can never carry one: an image
    /// has no `Prog`, and its note has no field to ask with. Whether a zone is a manifest's word or
    /// the owner's is milestone 892's open fork for every other program.
    ///
    /// `jig` holds the root zone, calef's ruling Q5 on #1884 (2026-10-10 UTC): it reaches whatever
    /// host an owner's repository or a listed location names.
    ///
    /// Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC).
    /// The pull request records no other name considered.
    pub const fn resolver_zone(self) -> Option<&'static str> {
        match self {
            Prog::Jig => Some(""),
            _ => None,
        }
    }

    /// The program's declared endowment: what the shell must (and must not) grant it.
    pub const fn manifest(self) -> Manifest {
        match self {
            Prog::LeastAuthorityDemo => least_authority_demo_manifest(),
            Prog::MemoryGrantDepleter => memory_grant_depleter_manifest(),
            Prog::InterruptHeeder => interrupt_heeder_manifest(),
            Prog::InterruptIgnorer => interrupt_ignorer_manifest(),
            Prog::Date => date_manifest(),
            Prog::Rm => rm_manifest(),
            Prog::Wc => wc_manifest(),
            Prog::Mdr => mdr_manifest(),
            Prog::Ps => ps_manifest(),
            Prog::Pgrep => pgrep_manifest(),
            Prog::Top => top_manifest(),
            Prog::Printenv => printenv_manifest(),
            Prog::Uptime => uptime_manifest(),
            Prog::NetworkEchoClient => network_echo_client_manifest(),
            Prog::UnreachableNetworkWitness => unreachable_network_witness_manifest(),
            Prog::Uuid => uuid_manifest(),
            Prog::Free => free_manifest(),
            Prog::Vmstat => vmstat_manifest(),
            Prog::Slabtop => slabtop_manifest(),
            Prog::GraphicalTerminal => graphical_terminal_manifest(),
            Prog::Reboot => reboot_manifest(),
            Prog::StdExerciser => std_exerciser_manifest(),
            Prog::Jig => jig_manifest(),
        }
    }
}

/// The manifest of `least_authority_demo`, [`Prog::LeastAuthorityDemo`].
const fn least_authority_demo_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Required,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        // A least_authority_demo answers with a number in a register, which is older than the sink
        // contract and still the right shape for one integer. It is also why `least_authority_demo 9 >
        // out.txt` is refused: there are no bytes to put in the file.
        output: OutputDeclaration::Words,
        input: InputDeclaration::Forbidden,
        reports: true,
        // A least_authority_demo finishes in one step; there is no long computation to interrupt, so it
        // is granted no interrupt channel. The shell waits for its result and no ^C tier
        // applies (DECISIONS §24).
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

/// The manifest of `memory_grant_depleter`, [`Prog::MemoryGrantDepleter`].
const fn memory_grant_depleter_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        // A budget between 1 and 64 pages. The lower bound makes "memory_grant_depleter with no --mem"
        // a refusal (it exists to spend memory); the upper bound is a sanity ceiling the
        // shell's own budget can actually back.
        mem: MemoryGrantDeclaration::Required { min: 1, max: 64 },
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        // The page count it managed to map, in a register. Same reason as least_authority_demo's.
        output: OutputDeclaration::Words,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// The two interrupt demonstrators. Both run until interrupted, take no argument and no
// memory grant, and report through the shared job frame rather than the result endpoint
// (so `reports` is false: they hold no result cap). `interruptible` is what makes the
// shell wire the two-tier ^C path and hold the region for a forcible teardown.
/// The manifest of `interrupt_heeder`, [`Prog::InterruptHeeder`].
const fn interrupt_heeder_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        // It reports through the shared job frame and holds no output capability at all,
        // which is a third thing a slot can be and the reason `OutputDeclaration` has three
        // variants: `Silent` is not "bytes nobody reads", it is no slot.
        output: OutputDeclaration::Silent,
        input: InputDeclaration::Forbidden,
        reports: false,
        interruptible: true,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

/// The manifest of `interrupt_ignorer`, [`Prog::InterruptIgnorer`].
const fn interrupt_ignorer_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::Silent,
        input: InputDeclaration::Forbidden,
        reports: false,
        interruptible: true,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// `date` declares an empty grant expression, and that is the interesting part: its
// authority (a read-only mapping of the clock page) is not something the command line
// can hand over, so there is nothing to type and nothing to forbid getting wrong.
// `FileGrantDeclaration::Forbidden` and `MemoryGrantDeclaration::Forbidden` are the load-bearing half: a clock
// reader has no use for either, so naming one is refused rather than granted-and-
// ignored. It reports through the shared result endpoint, one framed line of text.
//
// `ProgramArgumentDeclaration::Forbidden` is a **deliberate under-declaration**: `date` reads three
// registers (format, UTC offset, provenance), and a manifest that could express that
// needs positional arity, which milestone 47 explicitly defers. So the shell spawns it
// with the defaults (`Human`, UTC, no provenance line), which is the `date` a person
// typing `date` wants; the selectors arrive when `ProgramArgumentDeclaration` grows.
/// The manifest of `date`, [`Prog::Date`].
const fn date_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        // **`date` was speaking the sink contract before the contract existed**, because
        // its hand-rolled framing is bit for bit a `BYTES` message (notes/sink-protocol.md
        // on why `OPERATION_BYTES` is zero). So it is the first program that can be piped, and it
        // needed one change to be one: an end-of-stream message, without which a reader
        // waits forever on a producer that has already exited.
        //
        // **And the first program to declare a second stream** (DECISIONS §67), because it
        // is the program whose loss motivated the decision: "the time is unknown" is a
        // complaint about the run and it travelled on the same endpoint as the answer, so
        // `date > when.txt` on a clockless machine wrote it into the file. Declaring the
        // second output is what lets `2>` name where it goes, and what makes the default
        // (the shell prints it) a different place from where the answer went.
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        // **The one program in this table that declares a clock**, and the reason the field
        // exists. Nothing on the command line designates it, so the progenitor reads this to decide
        // which children get the read-only mapping (milestone 51's wiring, notes/date.md).
        clock: true,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **The first program endowed a directory**, and the first with options. It takes no
// integer and no memory: what it needs is the authority to take a name out of the
// directory that holds it, which is a capability and not a number.
//
// `subtree_flag: Some(b'r')` is the whole of "`rm -r` hands over more than `rm`". A
// grant without it cannot descend or enumerate, so the recursion is not merely disabled
// by a branch in the program: the capability to walk was never handed over, and a `rm`
// that tried anyway would be told there is no such name.
/// The manifest of `rm`, [`Prog::Rm`].
const fn rm_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Required {
            subtree_flag: Some(b'r'),
        },
        // In `rmopt`'s bit order, which is what that module pins.
        flags: Flags::new(b"rfv"),
        // `-v` prints names, and it prints them the way `date` prints a time. Piping `rm
        // -rv logs | wc` is therefore expressible; whether it is a good idea is the user's
        // business and not the manifest's.
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **The consumer**, and the only program that declares an input. Everything else about
// it is empty: no argument, no memory, no file, no directory, no options. What it does
// is entirely decided by what is in its input slot, which is the point of having it.
/// The manifest of `wc`, [`Prog::Wc`].
const fn wc_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::Bytes,
        // **The barrier**, and the only one in this tree. `wc` prints nothing until end of
        // stream, which is what lets this shell feed it and read it back with the one wait
        // point a process has. See [`InputDeclaration::Required::writes_while_reading`]: a filter
        // that answered as it read could not be run this way at all.
        input: InputDeclaration::Required {
            writes_while_reading: false,
        },
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **The viewer**, whose manifest is "a stream in, a stream out" like `wc`'s, and handed
// bytes like every other stage. The one place it parts from `wc` is the field milestone
// 50 added: `wc` absorbs the whole stream before it emits (`writes_while_reading:
// false`), while `mdr` renders as it reads and so writes while it is still reading. That
// difference is not decoration. It is exactly why `mdr` can deadlock a rendezvous
// pipeline where `wc` never does (notes/documentation.md's BUGS section), and the planner can
// only account for it if the manifest declares it. No memory grant, because the renderer
// never allocates.
/// The manifest of `mdr`, [`Prog::Mdr`].
const fn mdr_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Required {
            writes_while_reading: true,
        },
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **`ps`: a stream out, a domain in, and nothing else** (milestone 126).
//
// Every designated grant is `Forbidden`, which is the row worth reading: there is no
// file, no directory, no memory and no argument, so nothing a person could type widens
// what this program sees. Its whole authority is `domain`, which the progenitor endows and the
// command line cannot name, exactly as `date`'s clock is.
//
// `OutputDeclaration::BytesAndDiagnostics` because a refusal and a listing must not travel on
// the same stream: `ps > running.txt` on a domain it was not granted has to leave the
// file empty and put the reason on the terminal, which is the whole of DECISIONS §67
// applied to a monitor. `InputDeclaration::Forbidden` because a snapshot reads nothing.
/// The manifest of `ps`, [`Prog::Ps`].
const fn ps_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: true,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **`pgrep`: `ps`'s manifest, field for field, and the sameness is the claim.**
//
// A reader comparing these two blocks should find nothing to compare, because the
// program that *finds* a process is granted exactly what the program that *lists* them
// is: one supervision endpoint, `ENUMERATE`, and no way to name anything outside the
// domain. On Unix `pgrep` and `pkill` differ only in what they do with the answer, and
// there is no such pair here: a tid is a name, and killing a live child is
// `MemoryRegion::DESTROY` on its region, held by whoever spawned it (DECISIONS §24).
//
// `ProgramArgumentDeclaration::Forbidden` is `date`'s deliberate under-declaration, not an oversight.
// `pgrep` reads a state mask out of the integer-argument register, and a *pattern* is
// bytes, which nothing in this system carries from a command line to a program. So the
// prompt sends the default and `pgrep` names every member; see `crates/pgrep`'s `BUGS`
// for why that is a property of the boundary rather than of this program.
/// The manifest of `pgrep`, [`Prog::Pgrep`].
const fn pgrep_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: true,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **`top`: `ps`'s manifest and the machine statistics page.** Ranking still costs no
// authority: the CPU figures are a second walk of the same endpoint under the same
// `ENUMERATE`. What costs a grant is the machine line under the summary, which is what
// became of `tload` (DECISIONS §225), and it is a read-only page.
//
// `clock: false` and it is worth a line, because a `top` looks like it needs one: the
// uptime in its summary is the ambient monotonic counter every process already holds,
// which is the same finding `uptime` made when it turned out to need no capability.
/// The manifest of `top`, [`Prog::Top`].
const fn top_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: true,
        config: false,
        entropy: false,
        network: false,
        machine: true,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **The one program in this table that declares the inert-configuration page.** Same
// asymmetry as `date`'s clock: nothing on the command line designates it, so this is
// the progenitor's to endow and this field is how the progenitor decides which children get it.
/// The manifest of `printenv`, [`Prog::Printenv`].
const fn printenv_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
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
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **`least_authority_demo`'s manifest, not `date`'s.** `uptime` reads `user_mode_runtime::monotonic_nanos`,
// which is granted to every process unconditionally, so there is no capability here to
// declare: no clock, no domain, no memory, no file. `OutputDeclaration::Bytes` rather than
// `BytesAndDiagnostics` because the program cannot fail (the counter is always there to
// read), so there is nothing a second stream would ever carry; `wc`'s manifest makes the
// same call for the same reason.
/// The manifest of `uptime`, [`Prog::Uptime`].
const fn uptime_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **The one program in this table that declares the network** (milestone 590
// (provisional)). `uuid`'s block one service over, with one difference: a socket client
// mints the page it trades bytes with the stack through, so it needs a budget to mint
// it from, and that is `--mem` rather than something the network grant carries. Four
// pages is the frame and the page tables to map it; sixteen is generous for a program
// that maps one page.
/// The manifest of `network_echo_client`, [`Prog::NetworkEchoClient`].
const fn network_echo_client_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Required { min: 4, max: 16 },
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: true,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **Declares nothing, deliberately**: `uptime`'s manifest, field for field. The program
// exists to be the child the progenitor must not hand the network to.
/// The manifest of `unreachable_network_witness`, [`Prog::UnreachableNetworkWitness`].
const fn unreachable_network_witness_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **The one program in this table that declares the entropy service** (milestone 111).
// `printenv`'s block one authority over, and `ps`'s output shape: every designated
// grant is `Forbidden`, so nothing a person could type widens what this program
// reaches, and its whole authority is `entropy`, which the progenitor endows and the command line
// cannot name.
//
// `OutputDeclaration::BytesAndDiagnostics` is load-bearing rather than copied. `uuid` has
// exactly one way to fail (it was spawned holding no entropy capability), and the
// failure must not travel on the byte stream: `uuid > id.txt` on a boot with no entropy
// service has to leave the file empty and put the reason on the terminal, because a
// predictable identifier written into a file is worse than an empty file. That is
// DECISIONS §67 applied to a program whose whole product is unpredictability.
/// The manifest of `uuid`, [`Prog::Uuid`].
const fn uuid_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: true,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **`free`: the machine page and the budget view, and nothing that can act.** The
// two grants are §225's two lines, and both are views: `READ` on a page nobody but
// the kernel writes, and `ENUMERATE` on a budget, which cannot spend it.
/// The manifest of `free`, [`Prog::Free`].
const fn free_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: true,
        share: true,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **`vmstat`: the machine page alone.** Its rates are per second since boot, and the
// seconds come from the ambient monotonic counter, `uptime`'s finding, so no clock.
/// The manifest of `vmstat`, [`Prog::Vmstat`].
const fn vmstat_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: true,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **`slabtop`: the budget view alone**, which is the difference from `free`.
/// The manifest of `slabtop`, [`Prog::Slabtop`].
const fn slabtop_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: true,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

/// The manifest of `graphical_terminal`, [`Prog::GraphicalTerminal`].
const fn graphical_terminal_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        // A session reports one word when it ends (it ran, or it was interrupted); there is
        // no byte stream to redirect, and `graphical_terminal > out.txt` is refused for the same reason
        // `least_authority_demo 9 > out.txt` is.
        output: OutputDeclaration::Words,
        input: InputDeclaration::Forbidden,
        reports: true,
        // Not the §24 supervised shape in this first cut: the session ends on its own
        // (`^C` reaches it through its terminal, in either arm) rather than through a job
        // frame the shell watches. Milestone 632's block records the cost.
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

// **`reboot`: the reboot object, and nothing a line can designate.** No argument, no
// file, no memory. Its output is a report of what the sync answered, and its second
// stream is for the one failure it can see, a firmware that refused: the reason of §67
// (a program's second stream is a declaration, not a number), so `reboot > out.txt`
// still puts the refusal in front of a person. Not interruptible, because the
// supervised shape builds a child with no capabilities, and there is nothing to
// interrupt: it either resets the machine or says why not, at once.
/// The manifest of `reboot`, [`Prog::Reboot`].
const fn reboot_manifest() -> Manifest {
    Manifest {
        arg: ProgramArgumentDeclaration::Forbidden,
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        config: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: true,
        sync: true,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Native,
    }
}

/// The manifest of `std_exerciser`, [`Prog::StdExerciser`].
const fn std_exerciser_manifest() -> Manifest {
    Manifest {
        // **The line's words are its argv** (milestone 205, DECISIONS §170): `std_exerciser
        // one 'two words'` prints what `std::env::args()` yielded, which is the transcript
        // proving the page reaches `std`.
        arg: ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        // No directory. §170 ruled that a word's bytes carry no authority and that the
        // directories granted on the line bound what they reach; this program is spawned
        // holding none and runs its offline transcript, the one that asserts the most slots.
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        // std's `stdout` and `stderr` both send on `std_runtime_contract::STDOUT_SLOT` in
        // the sink framing, so the program is one byte stream and `>` and `|` work on it.
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        // The three `std` needs to answer `SystemTime::now()`, `SystemRng` and
        // `env::var("TZ")` rather than refuse them, and the three its transcript asserts.
        clock: true,
        domain: false,
        config: true,
        entropy: true,
        network: false,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        runtime: Runtime::Std,
    }
}

/// The manifest of `jig`, [`Prog::Jig`].
const fn jig_manifest() -> Manifest {
    Manifest {
        // **Its words are its verb and operands** (milestone 205, DECISIONS §170): `jig
        // install downloads/noteless.nifepkg` designates the file read-only, as `std_grep`'s
        // words do, and a word with no `/` is a package name, which designates nothing.
        arg: ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
        mem: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        dir: DirectoryGrantDeclaration::Forbidden,
        flags: NO_FLAGS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        // TLS to the index (milestone 801): a certificate's dates are checked against the
        // clock, and a handshake takes randomness.
        clock: true,
        domain: false,
        config: false,
        entropy: true,
        // `install <name>` fetches the package from the package source itself.
        network: true,
        machine: false,
        share: false,
        reboot: false,
        sync: false,
        // The request to install, remove or roll back, and the catalog it resolves a
        // package name against.
        installer: true,
        catalog: true,
        // The index copy it writes at `update` and resolves a name against after that
        // (milestone 801 (packages over the internet), ruling I2).
        indexes: IndexStoreDeclaration::ReadWrite,
        runtime: Runtime::Std,
    }
}
