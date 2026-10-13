# Vocabulary rulings

*An appendix to [`design/naming.md`](../naming.md), which is the rule. This file holds the
individual words calef has ruled on and the tests they set: halves and arms, abbreviations we
receive, structural termini, identity and principal, the `login` stem, and the casing of `nife`. It
exists to verify or challenge the main page, and a reader who only needs to name, ratify or rename
something should not have to open it. The directory `design/naming/` and this file's stem are
provisional names; naming is an architect's.*

## A half implies two; a third of anything is an arm

calef, 2026-09-19, reading a lane's workflow that called macOS, Linux and Windows each a "half" of
one program. A count above two in front of the word is not a strong claim or a loose one. It is
arithmetic that cannot be true, and a reader who meets it stops trusting the sentence around it.
(This section writes that shape as "three <halves>" wherever it must show it. `script/lint` gates on
the literal, and a rule whose own text trips its gate is a rule nobody can land.)

"Half" is for a genuine two-way split and is often exactly right.
This tree has honest halves everywhere: a crate's pure half and its host-tools half, the aarch64
half and riscv64 half of milestone 74 (cycle counters). For one branch of a split with three or
more, this tree's own word is arm. `components/src/console.rs` speaks of "its x86 arm", and the boot
ladder, the console server and the swish-check legs all read that way. "Part", "piece" and "leg" are
the other honest choices; a "leg" in this tree already means one architecture's run of a gate.

What is gated and what is not. `script/lint` reads only the shape that cannot be argued with: a
count word immediately in front of the word ("three h...", "four h...", and so on). It does not
judge a "half" whose siblings are a paragraph away, because that needs a reader. A gate that guesses
at prose is how this tree lost three checks. The sweep that came with the rule fixed four and left
the honest halves alone.

## An abbreviation we receive rather than author

calef, 2026-09-13, asking what it would take to rename `initrd` to `initial_ramdisk`. The answer is
that it cannot complete, and the reason generalises past this one word.

The acronym rule points at it, correctly. *An acronym is spelled out unless its expansion teaches
nothing* (2026-09-05). *Initial ramdisk* teaches a great deal: it says the thing is RAM-resident and
readable before storage exists. That is the entire point and is not recoverable from the five
letters. By that test `initrd` should go, the same way `dma` went.

*Corrected 2026-09-24: §154 (calef, 2026-09-18) superseded the 2026-09-05 wording quoted above. The
test is now whether the expansion is a phrase people actually say, and it is stated on the
[main page](../naming.md). "Initial ramdisk" is said, so the verdict on `initrd` is unchanged.*

It cannot, because about ninety of its 1,302 occurrences are somebody else's spelling:

| what | count | whose |
|---|---|---|
| `"linux,initrd-start"` | 19 | the Devicetree spec's property name, in a blob QEMU generates |
| `"linux,initrd-end"` | 8 | the same |
| `-initrd` | 64 | QEMU's command-line flag |

The kernel finds the region by parsing that property; every run passes that flag. Rename our 840
identifiers and the tree says `initial_ramdisk` in the code and `initrd` at the two points where a
reader most needs the words connected. Those are where we read the property, and where we launch the
machine. A rename that cannot reach the boundary makes a newcomer learn two words instead of one,
which is the opposite of what the acronym rule is for.

This is the `Guid` case one level out. `crates/gpt` keeps `Guid` rather than following
`user/src/uuid.rs`'s ratification, because the name is load-bearing at an interface boundary. A GUID
is mixed-endian on disk where RFC 9562's UUID is big-endian. So the two words name different things,
and collapsing them would assert a byte order the code does not produce. `initrd` is the same shape
with the authority reversed: not a distinction we are preserving, but a name we do not own.
*(Those paths are `crates/globally_unique_identifier_partition_table` and `components/src/uuid.rs`
today; `Guid` is still `Guid`.)*

So the rule this adds: the acronym test applies to names this tree
authors. Where a name arrives across an interface somebody else defines, the tree keeps their
spelling. It pays the cost at the reader's expense once, in an expansion written where the reader
meets it. `crates/user_rt/src/initrd.rs` carries that expansion as of 2026-09-13.

*Corrected 2026-09-24: that file is `crates/user_mode_runtime/src/initrd.rs` today, and its header
still opens with the expansion.*

*Overruled 2026-10-08 (UTC), #1842: the foreign spelling stops at the boundary.*

And the defect the pricing found was not the name. `initrd` appeared about 1,300 times and was
expanded in full exactly once, in `crates/dtb` (`crates/device_tree_blob` since 2026-09-19). That is
a crate about device trees rather than the one named for the thing. The abbreviation was never the
problem; an unexplained abbreviation was, and that is rung three rather than a sweep.

## A terminus that is structural, or one that is merely current

calef, 2026-09-13, asking after ruling `audit_sink` -> `login_audit_receiver`: *"Are there other
sinks that should be named receivers?"* The sweep found three and renamed none of them, which is why
the distinction is written down here rather than left in one block.

The test, in one question: does the name claim an end-of-stream that is a property of the design, or
one that is an accident of what has not been built yet?

`components/src/audit_sink.rs` (now `components/src/login_audit_receiver.rs`) receives one message
per successful login on `login`'s `AUDIT` endpoint and discards it. "Sink" was accurate about today and wrong about the program. The discard
exists because printing the record would need a `WRITE` view of the terminal, and handing that to a
third process was refused *for now*. The moment somebody grants it, the program keeps records and
its name says it does not. A name that has to change when a capability is granted is naming the gap
rather than the thing.

*The rename is performed, and the program's `Name:` block reads `ratified 2026-09-13`.*

The three that survived the same question, and each for its own reason:

| Name | Why the terminus is structural |
|---|---|
| `byte_sink_protocol` | A wire contract named for what it carries. It makes no disposal claim at all |
| `terminal_sink_caretaker` | It holds the terminal endpoint, which also carries `OP_READLINE`, and hands out a sink that **cannot read**. `sink` names what it hands out, `caretaker` names what it is. calef already caught this class once here, ratifying the longer form over `terminal_sink` on 2026-08-03 |
| `sink` (the program) | Not a terminus at all. Three roles, and `ROLE_FILE` is a real file behind a sink: the process can open, read, write at offsets, truncate and stat, while its client can only say *here are sixteen bytes, append them*. Renaming it `receiver` would name one end of a three-role program |

The third row's program no longer exists in that form, and the ruling is unaffected. Milestone 292
(three programs wearing one name) split it into `sink_transcript_writer`, `file_sink` and
`file_source` on 2026-09-14. The row's *reason* was that one name covered three jobs, and that
reason retired itself. What survived the split is the answer. `file_sink`'s terminus is structural:
its client holds a capability over which no message but *append* is expressible, and no grant
anybody could make would change that. That is the strongest form of this test passing, and it is why
`sink` stays the contract's word rather than becoming `receiver`.

The second half of the ruling is the part that is easy to lose. `audit_sink` failed on two counts
and only one of them is about "sink". The `audit` half promised a record that does not exist. That
is `flaky`'s fault (borrowed recognition the program contradicts) applied to a payload rather than
to a behaviour. A reader meeting `audit_sink` in a process listing concludes the system records
logins. Nothing does.

So `receiver` won because it is true in both states: it receives today and it will receive when it
records. `login_audit_recorder` is then an honest successor rather than a correction. That successor
is written into the program's own block as a condition rather than left to whoever notices. It is
the shape of §71 (a limitation is promoted), borrowed for a name: say what would change the answer,
beside the thing it would change.

What this does not license. It is not an argument against `sink`, which is this tree's word for the
end of a stream nobody reads further and is right three times out of four. It is an argument against
naming a program after a state that a single capability grant would end.

## An identity is what you present; a principal is what you become

`principal` is ratified (calef, 2026-09-14) as this tree's term of art for an authenticated actor
holding a capability set. It was the last word in the login vocabulary with no ruling. It is
ratified as a term, not as a filename: `script/names` walks crates, programs and modules, so nothing
gates this and the record is the gate.

Why it needed settling at all. `components/src/login.rs` could not be named until the words it
operates on were. Asked what that program authenticates, the honest answer turned out to be
*nothing*. It holds `WRITE` on the credential service's verify endpoint and relays, and
`components/src/credentialer.rs` is what checks the secret. What `login` does is mint a session's
worth of capabilities on the answer. So the sentence the program needs a name for is *turns an
identity into a principal*, and two of those three words were unsettled.

The four words, and why only one was open. Measured on 2026-09-14, tree-wide:

| word | code | prose | already names |
|---|---|---|---|
| `identity` | 546 | 362 | `identity_provisioner`, `MAX_IDENTITY`, `identity_hint` |
| `session` | 256 | 472 | `session_reviver` |
| `credential` | 163 | 129 | `credential_proto`, `credentialer`, `credentialer_test_client` |
| `principal` | 36 | 46 | **nothing** |

`identity` is fixed by an interface rather than by taste. DECISIONS §117 (a principal's subtree is
named by its identity string) names a principal's subtree by the identity string used directly, with
no lookup table, and `MAX_IDENTITY` caps it on the wire. `credential` was ratified with
`credentialer` on 2026-08-01. `user` means a person, which calef settled when the `user_` prefix
became `user_mode_`. It is otherwise spoken for: 1079 occurrences in code, essentially all of them
the kernel's `user::` module or the `user_mode_` prefix. That left `principal`, which the tree leans
on for the thing that matters most and had never given a name to.

The distinction the ratification keeps. An identity is the string a client presents (`alice`,
`bob`). A principal is the authenticated actor that results, holding a fresh capability set.
Collapsing them into one word was considered and refused. It is cheaper to read, and it loses
exactly the difference `login` exists to perform. That is the difference between what you present
and what you become. The attribution model of §109 (attribution is a property of a channel) is
written in the second word, not the first ("nameable only by the principal that established it").
Two successful logins are two different endpoint *objects* rather than two views of one.

One ambiguity recorded rather than fixed. `session` carries two senses in this repository: a login
session, and an agent session in AGENTS.md and the process notes. The prose count above is mostly
the second. Nothing in code confuses them, and no rename is proposed here. A reader of the process
docs should know the word is doing two jobs.

## The `login` stem stays

Ratified 2026-09-15 by calef, for the whole family: `components/src/login.rs`,
`crates/login_protocol`, `fixtures/src/login_test_client.rs`, and the kernel's `login_service` and
`login_tests`. About 700 occurrences across 94 files keep the word.

The case against was real, which is why this was parked on 2026-09-14 rather than signed. Two facts
undercut the reason first recorded for the name.

- Nothing types `login`. The kernel starts it, and only other programs reach it, by `CONNECT` on its
  front door. So an argument resting on a person meeting the Unix name did not hold.
- And it does not authenticate. It relays to `credentialer`, which checks the secret, and what it
  does itself is turn an identity into a principal (the section above).

By the test of milestone 63 (directory and package names), which refused to name the credential
service for its resource because it "never hands you a credential", a login service never hands you
a login.

Why the stem stays anyway. `login` is the field's name for this role whoever speaks it, and a reader
arriving from Unix lands in the right place. The program's docs say plainly where it departs
(capabilities instead of a mutated user ID). And the stem is carried by `login_protocol`, a wire
vocabulary two programs agree on, which is the expensive kind of name to move.

Considered and refused, as a program name:

- `authenticator` names the half this program does not do.
- `principal_minter` and `session_granter` are accurate and are new words for what everyone already
  calls logging in.
- `powerbox` is the right term of art for the pattern and one almost no reader would recognise.

Milestone 265 (`_proto` is a truncation) accepted a second rename of `login_protocol` if the stem
moved. There is none.

## The casing of `nife`, considered and settled

Raised 2026-08-15, the day of the rename: should prose write `Nife` (ordinary proper noun) or `NiFe`
(the chemically exact form, how Suess and Edison's batteries spell it)? Lowercase `nife` everywhere,
kept. Identifiers are lowercase regardless (crates, the triple, the repo), so any other choice
splits the spelling per context and drifts to three forms in practice. The camel seam in `NiFe` also
fights the ratified pronunciation (said like *knife*) by inviting "nye-fee".

The refusal record matters more than the choice. The sial/sima rule applies, since a name that needs
a casing note is the pronunciation-note tax in different clothes. The chemistry lives in the
README's one line, which is where it costs nothing.

## `cpu` is the scheduling unit; `core` is the physical core

calef, 2026-10-03 UTC, proposed by the maintainer and accepted ("Record the cpu ruling and start the
rename lane"):

> `cpu` names the unit the scheduler runs a thread on (one hardware thread, as Linux and POSIX count
> them). `core` is reserved for the physical core, for when topology needs it, and is not used to mean
> a cpu. A GPU or other device processor is named for what it is (`gpu`, ...), never `cpu`.

The tree already used `cpu` about ten to one (`MAX_CPUS`, `online_cpus`, `PerCpu`, `on_cpu`), and
`cpu_set` (ratified 2026-08-14) and `arch::cpu_start` are ratified with it. The rule follows the
tree, and the lane `cpu-not-core` renamed the identifiers that had drifted to `core`.

Considered and refused:

- `core` for the scheduling unit. With simultaneous multithreading (SMT) a physical core holds
  several cpus, so `core` would be wrong the first time topology matters. It also collides with
  Rust's `core` crate, so every `core::` path would read as a possible cpu reference.
- `processor`. A GPU is a processor too, which is why the ruling defines `cpu` as the unit the
  scheduler places threads on and not as "a processor".

Kept on purpose: names that arrive from someone else's interface (device-tree `cpus` nodes, vendor
clock names, RISC-V `hart` in RISC-V-only code, firmware and QEMU flags).

## The tree spells `receive`, never `recv`

calef, 2026-10-03 UTC, reviewing `recv_request` ("rule that the tree spells it receive"):

> `receive` is spelled out in every identifier this tree owns (`receive_request`, `RECEIVE_CAP`,
> `ipc_receive`, `OP_RECEIVE`). `recv` is not a ratified abbreviation. An identifier that mirrors
> an external API it binds to keeps the external spelling.

The governing rule is the one already applied to `user_rt` (now `user_mode_runtime`) and `cred_proto`
(now `credential_proto`): spell out a contraction nothing outside the tree owns. About 1,400
occurrences moved. No opcode, syscall number or wire value changed.

Refused: keeping `recv` because POSIX spells it so, since a syscall this tree defines is its own
name; and `rcv` or `rx`, which are different contractions the tree would have to ratify.

Kept, because someone else owns the spelling: `std::sync::mpsc` (`recv_timeout`, `RecvTimeoutError`),
the `std` overlay's `UdpSocket::recv` and `recv_from`, smoltcp's `recv_slice` and `can_recv`, Python's
`recvfrom`, and seL4's `ReplyRecv` and `seL4_Recv`. Mangled symbol patterns follow the names they
mangle. `script/lint`'s `recv` is a Python local for a method's receiver, a different word.

Closed records (`BUILT` roadmap blocks, `design/decisions/`, dated audit reports) keep the name they
used, so a grep for `recv` finds them.

The `operation` ruling of 2026-10-04, the same rule applied to `op`, is in
[spelled-out-rulings.md](spelled-out-rulings.md).

## We abbreviate on the command line

calef, 2026-10-06 UTC, while ratifying `jig` in PR #1756:

> We abbreviate on the command line.

Its first application, `jig`'s verbs (2026-10-10), is in
[command-line-rulings.md](command-line-rulings.md). Each abbreviation is still a name to ratify.

## A keeper holds something alive and does not restart it

calef, 2026-10-06 UTC, ratifying `user_timetable_keeper` over `session`.

| word | means | for this program |
|---|---|---|
| `keeper` | holds something alive, restarts nothing | ratified |
| `supervisor` | restarts what faults | refused; if it ever restarts the timetable, it becomes `user_timetable_supervisor` |
| `caretaker` | serves narrowed access | refused: it serves none |

## `system_installer`, not `basalt_installer`

calef, 2026-10-06 UTC: "system_installer ratified." It names what is installed, the whole system,
apart from `jig`, which installs packages. Refused `basalt_installer`: the program copies whatever
image the stick booted, so a distribution prefix would go false.
