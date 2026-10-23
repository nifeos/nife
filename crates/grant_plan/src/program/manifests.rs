//! **What each program is granted**: `Program::verbs` and `Program::manifest`, one arm per program.
//! A child file of `program`, not a module of its own, for the reason that module's header gives.
//!
//! The crate root re-exports every public item (§272 (the grant-plan crate root splits into
//! private modules)).
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

use crate::{
    DIAGNOSTICS_SLOT, DirectoryGrantDeclaration, FileGrantDeclaration, IndexStoreDeclaration,
    InputDeclaration, Manifest, MemoryGrantDeclaration, NO_SHORT_OPTIONS, OutputDeclaration,
    Program, ProgramArgumentDeclaration, ShortOptions, StartupContract, VerbGrant, WordGrant,
    verbs,
};

impl Program {
    /// **What each verb of this program is granted, when its manifest is narrowed by the line's
    /// first word** (milestone 809 (the package client becomes a program), its option V2): empty
    /// for every program but one, which is then endowed exactly [`manifest`](Program::manifest) as
    /// always. [`narrow_by_verb`](crate::narrow_by_verb) reads it, and the shell and the progenitor both call that.
    pub const fn verbs(self) -> &'static [VerbGrant] {
        match self {
            Program::Jig => verbs::JIG_VERBS,
            _ => &[],
        }
    }

    /// **The zone this program may resolve names in, when its manifest is narrowed to hold the
    /// network** (milestone 892 (a std program at the prompt holds the network and a resolver)):
    /// dotted text, empty for the root zone, which is every name. `None` for every program that
    /// resolves nothing, which is every program but one.
    ///
    /// On this table rather than in [`Manifest`], so installed bytes, which have no `Program`, can
    /// never carry one; whether a zone is a manifest's word or the owner's is milestone 892's open
    /// fork. `jig` holds the root zone (calef's ruling Q5 on #1884, 2026-10-10 UTC).
    ///
    /// Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC).
    /// The pull request records no other name considered.
    pub const fn resolver_zone(self) -> Option<&'static str> {
        match self {
            Program::Jig => Some(""),
            _ => None,
        }
    }

    /// The program's declared endowment: what the shell must (and must not) grant it.
    pub const fn manifest(self) -> Manifest {
        match self {
            Program::LeastAuthorityDemo => least_authority_demo_manifest(),
            Program::MemoryGrantDepleter => memory_grant_depleter_manifest(),
            Program::InterruptHeeder => interrupt_heeder_manifest(),
            Program::InterruptIgnorer => interrupt_ignorer_manifest(),
            Program::Date => date_manifest(),
            Program::Rm => rm_manifest(),
            Program::Wc => wc_manifest(),
            Program::Mdr => mdr_manifest(),
            Program::Ps => ps_manifest(),
            Program::Pgrep => pgrep_manifest(),
            Program::Top => top_manifest(),
            Program::Printenv => printenv_manifest(),
            Program::Uptime => uptime_manifest(),
            Program::NetworkEchoClient => network_echo_client_manifest(),
            Program::UnreachableNetworkWitness => unreachable_network_witness_manifest(),
            Program::Uuid => uuid_manifest(),
            Program::Free => free_manifest(),
            Program::Vmstat => vmstat_manifest(),
            Program::Slabtop => slabtop_manifest(),
            Program::GraphicalTerminal => graphical_terminal_manifest(),
            Program::Reboot => reboot_manifest(),
            Program::StdExerciser => std_exerciser_manifest(),
            Program::Jig => jig_manifest(),
        }
    }
}

/// The manifest of `least_authority_demo`, [`Program::LeastAuthorityDemo`].
const fn least_authority_demo_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Required,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
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
        configuration: false,
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
    }
}

/// The manifest of `memory_grant_depleter`, [`Program::MemoryGrantDepleter`].
const fn memory_grant_depleter_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        // A budget between 1 and 64 pages. The lower bound makes "memory_grant_depleter with no --mem"
        // a refusal (it exists to spend memory); the upper bound is a sanity ceiling the
        // shell's own budget can actually back.
        memory_grant: MemoryGrantDeclaration::Required { min: 1, max: 64 },
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        // The page count it managed to map, in a register. Same reason as least_authority_demo's.
        output: OutputDeclaration::Words,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        configuration: false,
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
    }
}

// The two interrupt demonstrators. Both run until interrupted, take no argument and no
// memory grant, and report through the shared job frame rather than the result endpoint
// (so `reports` is false: they hold no result cap). `interruptible` is what makes the
// shell wire the two-tier ^C path and hold the region for a forcible teardown.
/// The manifest of `interrupt_heeder`, [`Program::InterruptHeeder`].
const fn interrupt_heeder_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        // It reports through the shared job frame and holds no output capability at all,
        // which is a third thing a slot can be and the reason `OutputDeclaration` has three
        // variants: `Silent` is not "bytes nobody reads", it is no slot.
        output: OutputDeclaration::Silent,
        input: InputDeclaration::Forbidden,
        reports: false,
        interruptible: true,
        clock: false,
        domain: false,
        configuration: false,
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
    }
}

/// The manifest of `interrupt_ignorer`, [`Program::InterruptIgnorer`].
const fn interrupt_ignorer_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::Silent,
        input: InputDeclaration::Forbidden,
        reports: false,
        interruptible: true,
        clock: false,
        domain: false,
        configuration: false,
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
    }
}

/// The manifest of `date`, [`Program::Date`]: an empty grant expression, since its authority (the
/// clock page) is not something the line can hand over, so naming a file or memory is refused.
/// `ProgramArgumentDeclaration::Forbidden` is a deliberate under-declaration: `date` reads three
/// registers (format, UTC offset, provenance), which would need positional arity, so the shell
/// spawns it with the defaults (`Human`, UTC, no provenance line).
const fn date_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        // Its framing is a sink-contract `BYTES` message ending in end-of-stream
        // (notes/sink-protocol.md). The second stream (DECISIONS §67) carries "the time is
        // unknown", so `date > when.txt` on a clockless machine leaves the file without it.
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
        configuration: false,
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
    }
}

// **The first program endowed a directory**, and the first with options. It takes no
// integer and no memory: what it needs is the authority to take a name out of the
// directory that holds it, which is a capability and not a number.
//
// `subtree_option: Some(b'r')` is the whole of "`rm -r` hands over more than `rm`". A
// grant without it cannot descend or enumerate, so the recursion is not merely disabled
// by a branch in the program: the capability to walk was never handed over, and a `rm`
// that tried anyway would be told there is no such name.
/// The manifest of `rm`, [`Program::Rm`].
const fn rm_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Required {
            subtree_option: Some(b'r'),
        },
        // In `rmopt`'s bit order, which is what that module pins.
        short_options: ShortOptions::new(b"rfv"),
        // `-v` prints names, and it prints them the way `date` prints a time. Piping `rm
        // -rv logs | wc` is therefore expressible; whether it is a good idea is the user's
        // business and not the manifest's.
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        configuration: false,
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
    }
}

// **The consumer**, and the only program that declares an input. Everything else about
// it is empty: no argument, no memory, no file, no directory, no options. What it does
// is entirely decided by what is in its input slot, which is the point of having it.
/// The manifest of `wc`, [`Program::Wc`].
const fn wc_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
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
        configuration: false,
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
    }
}

// **The viewer**, whose manifest is "a stream in, a stream out" like `wc`'s, and handed
// bytes like every other stage. Like `wc` it absorbs the whole stream before it emits
// (`writes_while_reading: false`): CommonMark's grammar needs the whole document, so `mdr`
// reads to the end before it renders, and cannot deadlock a rendezvous pipeline by writing
// while its writer is still waiting to be read. A `std` program (milestone 784 (reuse what
// the tree wrote), item (f2)): its input arrives at `std_runtime_contract::STDIN_STREAM_SLOT`
// and its heap is the job's region. No memory grant beyond that.
/// The manifest of `mdr`, [`Program::Mdr`].
const fn mdr_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Required {
            writes_while_reading: false,
        },
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        configuration: false,
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
        startup: StartupContract::Std,
    }
}

/// The manifest of `ps`, [`Program::Ps`] (milestone 126 (the `procps` package)): every designated
/// grant `Forbidden`, so nothing typed widens what it sees, and its whole authority is `domain`. A
/// second stream (DECISIONS §67) so that `ps > running.txt` without a domain leaves the file empty
/// and puts the reason on the terminal.
const fn ps_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: true,
        configuration: false,
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
    }
}

/// The manifest of `pgrep`, [`Program::Pgrep`]: `ps`'s field for field, and the sameness is the
/// claim. Finding a process grants nothing listing does; killing a live child is
/// `MemoryRegion::DESTROY` on its region, held by whoever spawned it (DECISIONS §24 (interrupting
/// the foreground process)). `ProgramArgumentDeclaration::Forbidden` as for `date`: the state mask
/// register keeps its default, so it names every member (`crates/pgrep`'s `BUGS`).
const fn pgrep_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: true,
        configuration: false,
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
/// The manifest of `top`, [`Program::Top`].
const fn top_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: true,
        configuration: false,
        entropy: false,
        network: false,
        machine: true,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        activation_directory: false,
        console_input: false,
        startup: StartupContract::Native,
    }
}

// **The one program in this table that declares the inert-configuration page.** Same
// asymmetry as `date`'s clock: nothing on the command line designates it, so this is
// the progenitor's to endow and this field is how the progenitor decides which children get it.
/// The manifest of `printenv`, [`Program::Printenv`].
const fn printenv_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
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
    }
}

// **`least_authority_demo`'s manifest, not `date`'s.** `uptime` reads `user_mode_runtime::monotonic_nanos`,
// which is granted to every process unconditionally, so there is no capability here to
// declare: no clock, no domain, no memory, no file. `OutputDeclaration::Bytes` rather than
// `BytesAndDiagnostics` because the program cannot fail (the counter is always there to
// read), so there is nothing a second stream would ever carry; `wc`'s manifest makes the
// same call for the same reason.
/// The manifest of `uptime`, [`Program::Uptime`].
const fn uptime_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        configuration: false,
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
    }
}

// **The one program in this table that declares the network** (milestone 590
// (provisional)). `uuid`'s block one service over, with one difference: a socket client
// mints the page it trades bytes with the stack through, so it needs a budget to mint
// it from, and that is `--mem` rather than something the network grant carries. Four
// pages is the frame and the page tables to map it; sixteen is generous for a program
// that maps one page.
/// The manifest of `network_echo_client`, [`Program::NetworkEchoClient`].
const fn network_echo_client_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Required { min: 4, max: 16 },
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        configuration: false,
        entropy: false,
        network: true,
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
    }
}

// **Declares nothing, deliberately**: `uptime`'s manifest, field for field. The program
// exists to be the child the progenitor must not hand the network to.
/// The manifest of `unreachable_network_witness`, [`Program::UnreachableNetworkWitness`].
const fn unreachable_network_witness_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        configuration: false,
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
    }
}

/// The manifest of `uuid`, [`Program::Uuid`] (milestone 111 (a shell that can endow a child with
/// entropy)): every designated grant `Forbidden`, and its whole authority `entropy`. Its one
/// failure, no entropy capability, goes on the second stream (DECISIONS §67), so `uuid > id.txt`
/// leaves the file empty rather than predictable.
const fn uuid_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        configuration: false,
        entropy: true,
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
    }
}

// **`free`: the machine page and the budget view, and nothing that can act.** The
// two grants are §225's two lines, and both are views: `READ` on a page nobody but
// the kernel writes, and `ENUMERATE` on a budget, which cannot spend it.
/// The manifest of `free`, [`Program::Free`].
const fn free_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        configuration: false,
        entropy: false,
        network: false,
        machine: true,
        share: true,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        activation_directory: false,
        console_input: false,
        startup: StartupContract::Native,
    }
}

// **`vmstat`: the machine page alone.** Its rates are per second since boot, and the
// seconds come from the ambient monotonic counter, `uptime`'s finding, so no clock.
/// The manifest of `vmstat`, [`Program::Vmstat`].
const fn vmstat_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        configuration: false,
        entropy: false,
        network: false,
        machine: true,
        share: false,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        activation_directory: false,
        console_input: false,
        startup: StartupContract::Native,
    }
}

// **`slabtop`: the budget view alone**, which is the difference from `free`.
/// The manifest of `slabtop`, [`Program::Slabtop`].
const fn slabtop_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        configuration: false,
        entropy: false,
        network: false,
        machine: false,
        share: true,
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        activation_directory: false,
        console_input: false,
        startup: StartupContract::Native,
    }
}

/// The manifest of `graphical_terminal`, [`Program::GraphicalTerminal`].
const fn graphical_terminal_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
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
        configuration: false,
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
    }
}

// **`reboot`: the reboot object, and nothing a line can designate.** No argument, no
// file, no memory. Its output is a report of what the sync answered, and its second
// stream is for the one failure it can see, a firmware that refused: the reason of §67
// (a program's second stream is a declaration, not a number), so `reboot > out.txt`
// still puts the refusal in front of a person. Not interruptible, because the
// supervised shape builds a child with no capabilities, and there is nothing to
// interrupt: it either resets the machine or says why not, at once.
/// The manifest of `reboot`, [`Program::Reboot`].
const fn reboot_manifest() -> Manifest {
    Manifest {
        program_argument: ProgramArgumentDeclaration::Forbidden,
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        clock: false,
        domain: false,
        configuration: false,
        entropy: false,
        network: false,
        machine: false,
        share: false,
        reboot: true,
        sync: true,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        activation_directory: false,
        console_input: false,
        startup: StartupContract::Native,
    }
}

/// The manifest of `std_exerciser`, [`Program::StdExerciser`].
const fn std_exerciser_manifest() -> Manifest {
    Manifest {
        // **The line's words are its argv** (milestone 205, DECISIONS §170): `std_exerciser
        // one 'two words'` prints what `std::env::args()` yielded, which is the transcript
        // proving the page reaches `std`.
        program_argument: ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        // No directory. §170 ruled that a word's bytes carry no authority and that the
        // directories granted on the line bound what they reach; this program is spawned
        // holding none and runs its offline transcript, the one that asserts the most slots.
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
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
        configuration: true,
        entropy: true,
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
        startup: StartupContract::Std,
    }
}

/// The manifest of `jig`, [`Program::Jig`].
const fn jig_manifest() -> Manifest {
    Manifest {
        // **Its words are its verb and operands** (milestone 205, DECISIONS §170): `jig
        // install downloads/noteless.nifepkg` designates the file read-only, as `std_grep`'s
        // words do, and a word with no `/` is a package name, which designates nothing.
        program_argument: ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
        memory_grant: MemoryGrantDeclaration::Forbidden,
        file: FileGrantDeclaration::Forbidden,
        directory: DirectoryGrantDeclaration::Forbidden,
        short_options: NO_SHORT_OPTIONS,
        output: OutputDeclaration::Bytes,
        input: InputDeclaration::Forbidden,
        reports: true,
        interruptible: false,
        // TLS to the index (milestone 801): a certificate's dates are checked against the
        // clock, and a handshake takes randomness.
        clock: true,
        domain: false,
        configuration: false,
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
        // What is installed, read beside the index copy beneath it: `list -u` (milestone 809).
        activation_directory: true,
        // The owner's yes at `install` (milestone 892 (a std program at the prompt holds the
        // network and a resolver)), read from the console while the line runs.
        console_input: true,
        startup: StartupContract::Std,
    }
}
