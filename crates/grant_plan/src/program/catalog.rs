//! **The programs the shell can spawn, declared once**: the macro that turns the rows into the
//! `Prog` enum and its lookups, the compile-time checks on the rows, and the rows themselves.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// **Declare the programs the shell can spawn, once** (milestone 150; name provisional).
///
/// Each row is `Variant { id: N, name: "archive_name" }`, and from the rows this generates the
/// enum and everything that used to be a hand-maintained copy of it: [`Prog::name`], [`Prog::id`],
/// [`Prog::from_id`], [`Prog::from_name`], [`Prog::ALL`] and [`PROG_COUNT`]. Adding a program is a
/// row here plus its [`Prog::manifest`] arm, which the compiler demands (`E0004`); removing one is
/// deleting the row, and the compiler then points at every arm that named it.
///
/// **The id is written, never derived from position**, because it is a thing two programs agree
/// on (the shell sends it, the progenitor decodes it). A position-derived id would renumber every
/// later program the day an earlier one was removed. So removal leaves a hole, [`PROG_COUNT`] is
/// one past the highest id rather than the number of rows, and `the_wire_ids_already_shipped_never_move`
/// pins the ids that shipped before this table existed. Two rows with one id, or one name, fail
/// the build at the `const` assertions below rather than at a prompt.
///
/// A `macro_rules!` rather than a derive because DECISIONS §46 refuses a proc-macro dependency for
/// this, and a `const` table beside a hand-written enum would still be two lists nothing joins.
/// notes/adding-a-program/why-it-works-this-way.md has what else was considered.
macro_rules! programs {
    (
        $(#[$enum_meta:meta])*
        pub enum Prog {
            $(
                $(#[$meta:meta])*
                $variant:ident { id: $id:literal, name: $name:literal },
            )*
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum Prog {
            $(
                $(#[$meta])*
                $variant,
            )*
        }

        /// One past the highest wire id [`Prog::id`] can answer, which is the size of the table
        /// the progenitor indexes with it (`[Option<Elf>; PROG_COUNT]`). **Not the number of
        /// programs**: a removed program's id stays a hole rather than being reused, so the two
        /// differ as soon as one is removed, and [`Prog::from_id`] answers `None` for a hole.
        /// Computed from the declarations, so it cannot be forgotten; until milestone 150 it was a
        /// literal a person widened, and forgetting to was the one edit nothing caught.
        pub const PROG_COUNT: usize = one_past_highest(&[$($id),*]);

        const _: () = assert!(all_distinct_ids(&[$($id),*]), "two programs share a wire id");
        const _: () = assert!(all_distinct_names(&[$($name),*]), "two programs share a name");

        impl Prog {
            /// Every program, in declaration order. Sweep this rather than `0..PROG_COUNT`, which
            /// has holes once a program has been removed.
            pub const ALL: &'static [Prog] = &[$(Prog::$variant),*];

            /// Resolve a program by the name typed on the command line.
            ///
            /// Builtins are matched first by [`parse`](crate::parse), so a program named `help`, `echo`, `caps`
            /// or `time` would be unreachable. The program namespace must not contain any of those
            /// names.
            pub fn from_name(name: &[u8]) -> Option<Prog> {
                $(
                    if name == $name.as_bytes() {
                        return Some(Prog::$variant);
                    }
                )*
                None
            }

            /// The name the progenitor loads it by in the initial ramdisk (nifefs), and the shell
            /// prints. It is also the `[[bin]]` name in `components/` or `fixtures/`, which `xtask`
            /// checks every time it packs an archive.
            pub fn name(self) -> &'static str {
                match self {
                    $(Prog::$variant => $name,)*
                }
            }

            /// The stable wire id the shell sends and the progenitor decodes ([`spawnproto`](crate::spawnproto)).
            pub fn id(self) -> u64 {
                match self {
                    $(Prog::$variant => $id,)*
                }
            }

            /// The inverse of [`id`](Prog::id): the progenitor turns the wire id back into a
            /// program.
            pub fn from_id(id: u64) -> Option<Prog> {
                match id {
                    $($id => Some(Prog::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

/// [`PROG_COUNT`]'s arithmetic, `const` so the table size is known where the progenitor declares
/// its array.
pub(super) const fn one_past_highest(ids: &[u64]) -> usize {
    let mut next = 0;
    let mut i = 0;
    while i < ids.len() {
        if ids[i] as usize >= next {
            next = ids[i] as usize + 1;
        }
        i += 1;
    }
    next
}

pub(super) const fn all_distinct_ids(ids: &[u64]) -> bool {
    let mut i = 0;
    while i < ids.len() {
        let mut j = i + 1;
        while j < ids.len() {
            if ids[i] == ids[j] {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

pub(super) const fn all_distinct_names(names: &[&str]) -> bool {
    let mut i = 0;
    while i < names.len() {
        let mut j = i + 1;
        while j < names.len() {
            if same_bytes(names[i].as_bytes(), names[j].as_bytes()) {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
/// (design/naming/functions-that-answer-yes-or-no-worklist.md). It does not yet follow the Rust predicate rule
/// calef ratified 2026-09-24; recommended `bytes_eq`, because std's `ptr_eq` and `addr_eq` shape.
pub(super) const fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut k = 0;
    while k < a.len() {
        if a[k] != b[k] {
            return false;
        }
        k += 1;
    }
    true
}

programs! {
    /// A program the shell can spawn. The set is small and closed in phase 1; each variant carries a
    /// static [`Manifest`](crate::Manifest) and a stable wire id for [`spawnproto`](crate::spawnproto).
    ///
    /// This is deliberately an enum and not a string lookup at the grant boundary: the shell resolves
    /// a typed program once, and everything downstream (the manifest check, the wire id the progenitor decodes)
    /// speaks the type, not the name. A name that does not resolve is [`Refusal::NoSuchProgram`](crate::Refusal::NoSuchProgram), the
    /// "there is nothing there to name" shape of no-ambient-authority applied to programs themselves.
    pub enum Prog {
        /// Squares its integer argument and reports the answer. Needs no memory grant.
        LeastAuthorityDemo { id: 0, name: "least_authority_demo" },
        /// Spends a granted untyped budget: maps pages until the budget is exhausted and reports how
        /// many it got. The program that makes `--mem` *real* rather than parsed-and-ignored: the
        /// number it reports is the authority the command line handed it.
        MemoryGrantDepleter { id: 1, name: "memory_grant_depleter" },
        /// A long-running job that *heeds* the cooperative interrupt: it works forever, polling its
        /// interrupt flag between work units, and on `^C` cleans up and exits (DECISIONS §24 (interrupting the foreground process, two-tier and shell-held)). The
        /// cooperative tier made visible: the first `^C` stops it gracefully.
        InterruptHeeder { id: 2, name: "interrupt_heeder" },
        /// A runaway that ignores the interrupt entirely: a tight loop that never checks its flag. Only
        /// the forcible tier (the shell tearing its region down) ends it. The case the cooperative tier
        /// cannot reach, and the reason the second `^C` exists.
        InterruptIgnorer { id: 3, name: "interrupt_ignorer" },
        /// Print the wall-clock time (milestone 51, `components/src/date.rs`). It takes nothing from the
        /// command line: no argument, no memory, no file. **Its whole authority is a read-only mapping
        /// of the clock page, which the progenitor endows and this shell cannot**, and that asymmetry is why
        /// [`Manifest::clock`](crate::Manifest::clock) exists: the grant is real, it is just not something a person designates.
        /// The interactive boot starts a clock service and hands the progenitor the page, so `date` at the prompt
        /// prints a time; on a machine whose RTC the service did not believe it prints "the time is
        /// unknown: the machine has no clock it believes", which is the other true sentence.
        Date { id: 4, name: "date" },
        /// **Remove a name, and with `-r` the tree under it** (milestone 47 (navigation and naming),
        /// `components/src/rm.rs`).
        ///
        /// A **program, not a builtin**, and that is Unix's shape rather than a divergence from it.
        /// `cd`, `pwd` and `ls` are builtins here because the shell is rebinding what it already holds;
        /// `rm -r` is a destructive loop, not a rebinding. A builtin would run with the shell's **entire
        /// endowment**, while a program takes an explicit attenuated grant, so `caps rm -r logs` prints
        /// the subtree at risk before anything happens and a bug in the recursion can only reach what it
        /// was handed. See [`DirSpec`](crate::DirSpec).
        // `rm` stopped being a builtin in milestone 47's rmdir lane, which is what makes this
        // name reachable: a builtin would have shadowed it, because `parse` matches those first.
        Rm { id: 5, name: "rm" },
        /// **Count what arrives on its input** (milestone 50, `components/src/wc.rs`): lines, words and
        /// bytes, printed as one line of text.
        ///
        /// The first program that declares [`InputSpec::Required`](crate::InputSpec::Required), and the reason that spec exists.
        /// `wc` with nothing feeding it would block on a receive forever, and the shell can see that at
        /// the prompt: the manifest says it reads a stream, so a line that gives it none is
        /// [`Refusal::InputRequired`](crate::Refusal::InputRequired) before anything is spawned. Unix cannot make that check, because
        /// there fd 0 always exists and "nobody is ever going to write to it" is not a property of the
        /// command line.
        ///
        /// Its output is [`OutputSpec::Bytes`](crate::OutputSpec::Bytes) because it has to be: `wc`'s answer is text, so `echo a
        /// | wc | wc` composes, and a program that reported a number in a register could not be on the
        /// left of a pipe at all.
        ///
        /// And because it declares an input and declares no file, it is also the program that gives the
        /// **input operand** its meaning: `wc report.txt` is `wc < report.txt` with the operator left
        /// out, resolved by [`plan_against_with`](crate::plan_against_with) into a [`line::Source::File`](crate::line::Source::File). See that function for
        /// why what the child holds is narrower than a per-file capability rather than the same thing.
        Wc { id: 6, name: "wc" },
        /// **Render markdown for a terminal** (milestone 40, `components/src/mdr.rs`,
        /// notes/documentation.md).
        ///
        /// The same manifest as [`Prog::Wc`]: a stream in, a stream out, and nothing else. `doc
        /// notes/glob.md` reads like Unix's `man` and is not: the name is a designation the *shell*
        /// resolves against the directory it holds, and what arrives at the program is bytes. A viewer
        /// that opened the page it renders would be a viewer that could open any page.
        ///
        /// Name: provisional.
        Mdr { id: 7, name: "mdr" },
        /// **List the processes in the supervision domain it was spawned into** (milestone 126 (the `procps` package),
        /// `components/src/ps.rs`, notes/process-view.md).
        ///
        /// The reason [`Manifest::domain`](crate::Manifest::domain) exists, and the same asymmetry [`Prog::Date`] made for the
        /// clock: the grant is real and it is not something a person designates on the line. There is
        /// no `/proc` here to name and no pid space to scan, so what `ps` can see is decided entirely
        /// by which supervision endpoint the progenitor put in its capability table, and `caps ps` prints that.
        Ps { id: 8, name: "ps" },
        /// **Name the members of that same domain that match, and do nothing to them** (milestone 126,
        /// `components/src/pgrep.rs`, notes/process-view.md).
        ///
        /// [`Prog::Ps`]'s manifest exactly, down to the field, and that is the declaration doing the
        /// work rather than a coincidence. On Unix `pgrep` and `pkill` are one lookup with two endings,
        /// so a program that can find a process can end it. Here the two manifests being identical is
        /// the readable form of **a domain names its members and does not act on them** (calef,
        /// 2026-08-17): the finding program is granted no authority the listing program lacks, and there
        /// is no `Prog::Pkill` for it to be compared against, because a tid is a name and no method
        /// turns one into a capability.
        ///
        /// `ArgSpec::Forbidden` is [`Prog::Date`]'s deliberate under-declaration, for the same reason
        /// and with the same consequence: `pgrep` reads a state mask out of a register the shell cannot
        /// set, so at this prompt it takes the default and names every member. Nothing in this system
        /// delivers *bytes* from a command line to a program, so a pattern is not something the line can
        /// carry until [`ArgSpec`](crate::ArgSpec) grows the positional arity milestone 47 deferred; see `crates/pgrep`'s
        /// `BUGS`.
        Pgrep { id: 9, name: "pgrep" },
        // **There was a `Watch` here until milestone 281** (`watch` holds exactly what `ps` holds,
        // so it is nothing), which deleted `user/src/watch.rs` and `crates/watch` on 2026-09-13.
        // The reason it went is a test worth reusing rather than a one-off.
        //
        // **Two programs are two programs when they hold different authority.** `Watch`'s manifest was
        // [`Prog::Ps`]'s with one field changed, and the program held the same three slots from the same
        // named constants (`REPORT`, [`DOMAIN_SLOT`], [`DIAGNOSTICS_SLOT`]) while being, literally,
        // `ps`'s own loop. In a capability system that settles "one program or two" without appealing to
        // taste: there was no boundary there to draw. The refresh needed no capability of its own
        // either, because the interval was a yield-spin over the ambient monotonic counter and this
        // kernel has no timed wait, so even the least-authority argument for keeping them apart was
        // absent.
        //
        // That argued for folding it into `ps` as a flag, which is what milestone 281 was minted to do.
        // calef then took it one step further: the flag's entire content was a busy-wait over a table of
        // two columns that barely changes, so **deleting it buys the same simplification and costs
        // nothing anyone was using**. Milestone 282 (a thread's CPU time, and the `top` it makes possible), on DECISIONS
        // §150 (how does a thread's CPU time reach userspace?), adds per-thread scheduled CPU
        // time, and once there is something worth watching and something to rank by, the live view
        // is rebuilt properly as `top`. It arrives as an `abi::survey::record` value rather than as
        // the fourth `SURVEY` word this line used to name: calef replaced the widened register row
        // with a selector on 2026-09-21, inside the window §150 itself left open for exactly that.
        //
        // **Neither `watch` nor `crates/watch` was ever ratified**, and that was deliberate rather than
        // an oversight: calef declined to rule on both while this milestone might retire them, which it
        // did. The name was wrong on its own terms too. Upstream `watch` re-runs an arbitrary command,
        // which this one never could (spawning by name is the shell's own capability and is granted to
        // nothing the shell spawns), so the program was never in `watch`'s family. It was a very thin
        // member of `top`'s, which is the shape 282 makes it worth rebuilding as.
        /// **Print how long the ambient monotonic counter has been running** (milestone 126,
        /// `components/src/uptime.rs`, `crates/uptime`).
        ///
        /// [`Prog::LeastAuthorityDemo`]'s manifest, not [`Prog::Date`]'s: `user_mode_runtime::monotonic_nanos` is granted to
        /// **every** process unconditionally (`kernel/src/arch/*/timer.rs`'s documented, deliberate
        /// exception to DECISIONS §10's no-ambient-authority rule), so this program needed no clock
        /// capability, no domain, no memory, no file, nothing beyond the report channel every spawn
        /// carries. The one member of milestone 126's "machine-wide statistics" row that turned out to
        /// be pure wiring rather than a design fork; see design/roadmap/0126-who-else-is-running.md.
        Uptime { id: 10, name: "uptime" },
        /// **Print the inert-configuration page** (milestone 47's environment-variable fork, DECISIONS
        /// §111; `components/src/printenv.rs`).
        ///
        /// The reason [`Manifest::config`](crate::Manifest::config) exists, and the same asymmetry [`Prog::Date`] made for the
        /// clock: the grant is real and it is not something a person designates on the line. Before
        /// this program, the page existed and could be assembled and mapped
        /// (`crates/environment_contract`), but nothing in the shell's program table declared wanting one,
        /// so `caps`'s preview of it had nothing to show and no boot granted it outside a kernel test
        /// harness standing in for a std program. `date` before `Prog::Date` existed is the position
        /// this section of the roadmap named; `printenv` is this milestone's `date`.
        ///
        /// Takes no argument, no memory, no file: its whole authority is the read-only page. `TZ`,
        /// `LANG` and `TERM` print as `KEY=value` when the key is present and `KEY (unset)` when the
        /// page is valid but does not carry it (`environment_contract`'s validated-domain shape makes both
        /// states distinguishable from a page nobody has assembled at all, which reads as
        /// "no configuration was granted").
        ///
        /// Name: provisional, Unix's own for exactly this (`printenv(1)`/`env(1)` with no
        /// arguments): a term of art already right, per this tree's own naming convention for
        /// standard terms.
        Printenv { id: 11, name: "printenv" },
        /// **Print a version-4 UUID drawn from the entropy service** (milestone 111, `components/src/uuid.rs`).
        ///
        /// The reason [`Manifest::entropy`](crate::Manifest::entropy) exists, and [`Prog::Date`]'s asymmetry a fourth time: the
        /// grant is real and no token on the line designates it. Before this program the entropy
        /// service existed, answered, and was reachable only by something the *system* spawned
        /// (`credentialer` at boot, `disk_partitioner` under the kernel's own test harness), so a
        /// program that needed randomness could not be run by a person. `date` before [`Prog::Date`]
        /// existed is the position notes/entropy.md's `BUGS` described; this is that milestone's
        /// `date`.
        ///
        /// **It is `disk_partitioner`'s draw with the disk taken away**, which is what makes it an
        /// honest consumer rather than a demonstration. Both call
        /// `universally_unique_identifier::Uuid::v4_from_random` over sixteen bytes from the same
        /// service, because a GPT gives every partition a random unique id and neither crate will
        /// invent one. The partitioner
        /// needs a disk capability this shell does not hold and cannot attenuate; the sixteen bytes and
        /// the stamping are the half that does not, so this program is the part of that path a prompt
        /// can reach today.
        ///
        /// Takes no argument, no memory, no file: its whole authority is the client view of the entropy
        /// service. `OutputSpec::BytesAndDiagnostics` for [`Prog::Ps`]'s reason, and here the second
        /// stream carries the one sentence that matters: a boot with no entropy service must leave
        /// `uuid > id.txt` **empty** and say why on the terminal, because a file containing a
        /// predictable identifier is worse than a file containing nothing.
        ///
        /// Name: provisional. RFC 9562's own term for the object, which
        /// `crates/universally_unique_identifier` also uses (2026-10-06); the partition table says
        /// GUID only for the UEFI fields that carry one.
        Uuid { id: 12, name: "uuid" },
        /// **Rank the members of that same domain by the CPU time each has been scheduled for**
        /// (milestone 282 (a thread's CPU time, and the `top` it makes possible), `components/src/top.rs`, `crates/top`).
        ///
        /// [`Prog::Ps`]'s manifest plus one read-only page. Until milestone 126 (the `procps`
        /// package) built the machine statistics page it was `ps`'s field for field, and milestone
        /// 281 (`watch` holds exactly what `ps` holds)'s rule that **two programs are two programs
        /// when they hold different authority** made whether it was a program at all a live
        /// question. DECISIONS §225 (`free` sees the machine and your share) then put `tload`'s
        /// question in this program's summary, which needs the page, so the endowments now differ
        /// and the rule no longer argues for folding it into `ps`. Whether that settles it is
        /// calef's; `crates/top`'s module docs carry the argument.
        ///
        /// `ArgSpec::Forbidden` rather than an optional count, because there is no such spec: a
        /// `top N` would have to make the argument `Required`, and a `top` that cannot be typed
        /// bare is not a `top`. Every row is printed, ranked. Same boundary limitation
        /// [`Prog::Pgrep`] records for its missing pattern.
        ///
        /// Name: provisional.
        Top { id: 13, name: "top" },
        /// **Reach the network from the prompt** (milestone 590 (the booted system starts its
        /// network stack),
        /// `fixtures/src/network_echo_client.rs`): open a TCP connection to the QEMU runners'
        /// echo peer (10.0.2.9:7777, `helpers/qemu-runner-aarch64.sh`'s `guestfwd`), send a
        /// fixed line, and print what came back.
        ///
        /// The only program in this table that declares [`Manifest::network`](crate::Manifest::network), and a fixture rather
        /// than a tool: the echo peer exists only inside a QEMU runner, so the program is the proof
        /// that a person at a booted system's prompt can reach the network through the stack the
        /// progenitor built, and nothing more. The package client milestone 198 (a package manager)
        /// needs is the first real tool that will declare the same field.
        ///
        /// Name: provisional.
        NetworkEchoClient { id: 14, name: "network_echo_client" },
        /// **Try to reach the network without declaring it, and say what the kernel answered**
        /// (milestone 590 (provisional), `fixtures/src/unreachable_network_witness.rs`).
        ///
        /// The negative control for [`Prog::NetworkEchoClient`], and the reason it is a program
        /// rather than a sentence: its manifest declares nothing, it `CALL`s [`NETWORK_SLOT`](crate::NETWORK_SLOT) with a
        /// real socket-contract request anyway, and prints the error. A progenitor that endowed the
        /// network to every child rather than to the declaring one would turn its line from a
        /// refusal into a reply. `unwritable_clock_witness`'s shape, one authority over.
        ///
        /// Name: provisional.
        UnreachableNetworkWitness { id: 15, name: "unreachable_network_witness" },
        /// **The `std` demonstration program, spawned from the prompt** (milestone 595
        /// (provisional), the shell runs a `std` program). `std_exerciser/src/main.rs`, built with
        /// `-Zbuild-std` against nife's patched standard library rather than as a `[[bin]]` in
        /// `components/` or `fixtures/`, which is why it is in an archive only when `cargo xtask
        /// std-exerciser` ran first (`script/test` does).
        ///
        /// The first program this table names whose [`Manifest::startup`](crate::Manifest::startup) is [`StartupContract::Std`](crate::StartupContract::Std), and
        /// that is its whole reason to be here: it is how the progenitor's std layout is proven at
        /// the real prompt. Its transcript asserts the heap, the wall clock, randomness and the
        /// configuration page each arrived at the slot `std_runtime_contract` fixes, so a layout
        /// that put one in the wrong place fails a line rather than printing something plausible.
        ///
        /// **Named through this enum as an archive program, which settles nothing about §219 (how
        /// the shell names an installed program to the spawner)**, decided 2026-09-26 as option D.
        /// A program that arrives by option D carries its own manifest, and this field is what
        /// that manifest will say.
        StdExerciser { id: 16, name: "std_exerciser" },
        /// **How much memory the machine has, and how much of this prompt's budget is spent**
        /// (milestone 126, DECISIONS §225 (`free` sees the machine and your share), `components/src/free.rs`, `crates/free`).
        ///
        /// Two grants, and they are the ruling's two lines. [`Manifest::machine`](crate::Manifest::machine) is the machine
        /// statistics page, which owner policy grants by default and can withhold; [`Manifest::share`](crate::Manifest::share)
        /// is this prompt's job budget narrowed to `ENUMERATE`, which answers "yours". Neither can
        /// change anything: the page is read-only and the budget view cannot spend.
        ///
        /// Name: provisional, milestone 126's `free` lane, 2026-09-26. Upstream `procps`'s, for the
        /// program a reader types to ask this.
        Free { id: 17, name: "free" },
        /// **The machine's counters since boot**: run queue, memory, interrupts, context switches
        /// and the busy/idle split (milestone 126, DECISIONS §225, `components/src/vmstat.rs`,
        /// `crates/vmstat`).
        ///
        /// The machine statistics page and nothing else. Not `free`'s manifest, because it does not
        /// hold the budget view: `vmstat` describes the machine and says nothing about "yours".
        ///
        /// Name: provisional, milestone 126's `free` lane, 2026-09-26. Upstream `procps`'s.
        Vmstat { id: 18, name: "vmstat" },
        /// **Where this prompt's job budget went, by kind of kernel object** (milestone 126,
        /// DECISIONS §225, `components/src/slabtop.rs`, `crates/slabtop`).
        ///
        /// Upstream `slabtop` lists the kernel's slab caches. This kernel has none (milestone 14 (kernel objects from untyped)
        /// removed its heap and slab); kernel objects are carved from regions their holders own,
        /// so §225 turned the question into `MemoryRegion::USAGE` asked per object type. It holds
        /// the budget view alone, which is why it is not `free`: it does not see the machine.
        ///
        /// Name: provisional, milestone 126's `free` lane, 2026-09-26, flagged harder than the other
        /// two: the upstream name promises a kernel-wide cache view, and this is one budget's
        /// breakdown.
        Slabtop { id: 19, name: "slabtop" },
        /// **Take the screen and the keyboard, and run a prompt on them** (milestone 632
        /// (provisional), `components/src/graphical_terminal.rs`; calef's 2026-09-30 ruling that graphics is
        /// launched from the swish prompt rather than built at boot).
        ///
        /// The display stack a `graphical_terminal` spawn builds is the progenitor's job, exactly as a
        /// directory grant's caretaker is: the shell asks (`spawnproto::Wiring::graphics`, which
        /// carries no capability since milestone 715 (provisional)) and the progenitor builds
        /// `gpu_driver`, `display_terminal`, the line discipline and, when the boot has a
        /// keyboard, `keyboard_driver` from the devices it holds, then starts this program wired
        /// to the lot. With no keyboard the session's keystrokes come from the boot's own line
        /// discipline over the UART, which is milestone 192 (a keyboard on real silicon)'s option A
        /// at launch rather than at boot.
        ///
        /// Declares nothing a line designates, which is what makes every operator on it a refusal:
        /// `Words` output refuses `>` and the left of a `|` (a session is not a byte stream),
        /// `InputSpec::Forbidden` refuses the right of one, and a graphical terminal session reached any way
        /// but a plain line is a line this manifest turns away at the prompt.
        ///
        /// Name: ratified 2026-10-03 (calef, #1493). Refused `screen` (clashes with `SCREEN_BIT`,
        /// the screen-narrowed tail of §106 (an unredirected tail stage's output goes to the
        /// screen, not the shell), and with GNU screen), `gate` (the tree's word for a check that
        /// fails loudly, about 3,700 uses, and the x86 IDT's interrupt gate), `display_session` and
        /// `display_console` (`console` is the UART server). It names the whole stack
        /// (`gpu_driver`, `display_terminal` and the keyboard), so it survives any later change of
        /// VT engine behind `display_terminal`.
        GraphicalTerminal { id: 20, name: "graphical_terminal" },
        /// **Restart the machine** (milestone 805 (`reboot` at the prompt), DECISIONS §251
        /// (restarting the machine is a kernel object the progenitor hands out)),
        /// `components/src/reboot.rs`.
        ///
        /// The one program that declares [`Manifest::reboot`](crate::Manifest::reboot) and [`Manifest::sync`](crate::Manifest::sync). It syncs
        /// the writable filesystem through its sync-only capability, prints the answer, then
        /// invokes the reboot object, which does not return when the reset works. When the firmware
        /// refuses, it says so on its second stream.
        ///
        /// Name: provisional. A verb, which design/naming.md passes only as a term of art, like
        /// `bind`; every Unix since V7 has spelled this one `reboot`.
        Reboot { id: 21, name: "reboot" },
        /// **The package manager** (milestone 809 (the package client becomes a program)): it
        /// installs, removes and rolls back packages, and lists what the image vouches for.
        /// `jig/src/main.rs`, a `std` program built beside `std_exerciser`, which is why it is in
        /// an archive only when `cargo xtask std-exerciser` ran (`script/test` and CI do).
        ///
        /// The one program that declares [`Manifest::installer`](crate::Manifest::installer): the progenitor serves its
        /// requests on an endpoint that speaks install, remove and rollback and nothing else, and
        /// grants it only on a request from the owner's console (DECISIONS §270 (a package manager
        /// holds an installer endpoint, not the spawn endpoint)). It fetches a package over the
        /// network itself, so the progenitor parses no network input.
        ///
        /// Name: ratified 2026-10-06 (calef): the guide that makes flat-pack assembly come out the
        /// same every time. Refused `package`, `pkg`, `bpm`, `knap` and `flatpak`; `kit` and `cam`
        /// collide with shipped commands. Its verbs are apt's where apt has one
        /// (design/naming/command-line-rulings.md, 2026-10-10).
        Jig { id: 22, name: "jig" },
    }
}
