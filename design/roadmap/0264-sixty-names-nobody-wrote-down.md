---
status: BUILT
raised: 2026-09-05
built: 2026-09-05
---
# 264. Sixty names the history cannot justify, and the research that would let calef rule on them

It gated on nothing while it ran, which is `script/names --unratified`'s own design: a worklist
rather than a wall, so an unratified name never fails anybody's build. `script/names --unratified`
now reports zero unrecorded, which was this block's own proof condition and is checkable by
anyone who doubts the paragraph.

## The measurement that scoped it

`script/names --unratified`, 2026-09-05: 100 unratified of 204, and the tool already sorts them
into two kinds that are two different jobs.

| | provisional | **unrecorded** | recorded |
|---|---|---|---|
| programs | 11 | **22** | 3 |
| crates | 10 | **8** | 2 |
| packages | 0 | **5** | 1 |
| scripts | 10 | **25** | 3 |
| | 31 | **60** | 9 |

**A provisional name is prepared and a ratification is a read.** A lane coined it, argued it, and
recorded its refusals, so calef weighs an argument. Two were ratified on 2026-09-05 in one question
each (`script/board-netboot` and `bench/xenon-netboot/`), and both went the lane's way because the
argument arrived with the name.

An unrecorded name has no argument at all. `script/names <name>` prints what the history does and
does not say, and for these it says nothing usable. Bringing calef a name with no argument is what
AGENTS.md refuses in "a fork reaches calef with its questions already answered": his attention is
the scarcest thing here, and spending it on lookups anybody could run is the failure that section
exists to name.

This milestone is those lookups. It converts 60 unanswerable names into recorded ones plus a
small set of genuinely open questions, each arriving with its options.

## What to produce, per name

A provenance block where a reader meets the name, which is milestone 115's mechanism and the
shape `script/names` already checks for. `script/board-netboot`'s header is the worked example: what
the name claims, why, and what was refused and why each lost, because "the refusals are the
valuable half" is that script's own line.

Three outcomes, and the split should be stated per name rather than assumed:

- `recorded`, where the history justifies it and nobody had written the sentence down. A large
  share of the 60 is this, and it is nearly free: `wc`, `rm`, `date`, `login`, `clock`, `entropy`,
  `ntp`, `hello` and `doc` are terms a reader knows from outside this project, which AGENTS.md calls
  the best names available. The acronym rule settled 2026-09-05 governs them, and where an acronym
  survives, the block says why its expansion teaches nothing.
- **`provisional`**, where the history does not justify it and the lane has a case to make. `outlaw`,
  `chatty`, `flaky`, `heeder`, `spinner`, `soaker` and `budgeter` are coinages nobody argued for in
  writing. **Do not invent a justification for a name that has none**: say what the history says,
  propose one, and let calef rule.
- A rename proposed, where the research shows the name is wrong. Propose it; do not do it.
  Renaming is a naming decision with extra steps and is an architect's.

## Two rules that govern this and were both set the same day

An acronym is spelled out unless its expansion teaches nothing (AGENTS.md's naming section,
2026-09-05). Several of these will turn on it, and `dma`, `dtb`, `gpt`, `ipc` and `asid` were
deratified by that rule, so a name carrying one of them needs the question asked rather than assumed.

And the seventh question applies to a rename as much as to a fork: would we still choose this if
both options cost the same? A name kept because renaming is work is an effort argument, and it has to
say so in those words.

## What this milestone is not

**It is not a renaming sweep.** The deliverable is the record, and a rename is an outcome the record
may recommend. Milestone 63 did the renaming; this writes down why the survivors survived.

**And it does not touch the 31 provisional ones.** Those are an architect's to ratify and are
already prepared; adding to their argument is not this lane's business.

## The proof that this milestone worked

`script/names --unratified` reports zero unrecorded, with every one of the 60 either recorded
with its reason or moved to provisional with a case. The count is the deliverable and it is
mechanically checkable, which is unusual for a documentation milestone and worth having.

## What it found

The counts, of the 60. 28 became `recorded`, 32 became `provisional`, and 5 of the 32 carry a
proposed rename. The tree-wide table moved from 9 recorded to 37 and from 60 unrecorded to 0; the
provisional tier grew from 31 to 63, which is the point rather than a side effect, since a
provisional name is one calef can rule on in a read and an unrecorded one is not.

The number that is honest rather than flattering: 10. Ten of the sixty had nothing in the tree
justifying them beyond the date they appeared, so the case beside them is this lane's own reading and
is labeled as such. They are `budgeter`, `heeder`, `spinner`, `outlaw`, `chatty`, `flaky`, `worker`,
`driver`, `gates` and the `user` package. The BUGS section below predicted this and was right about
its existence; the size is smaller than the prediction implied.

### The shape: a third of the worklist was a sorting bug, not a research gap

**21 of the 60 already held a complete argument, refusals included, and said "provisional" in their
own prose while carrying `unrecorded` as their status token.** `script/names` was built with two
states and gained `recorded` and `provisional` afterwards, precisely because "nobody can say why this
is called that" and "here is the argument, nobody signed it" are different amounts of calef's time.
Nothing swept the blocks written before the third state existed, so they kept saying the old word.

That is worth more than the twenty-one sentences it cost to fix. The worklist over-reported its
hardest tier by roughly a third, which made the backlog look worse than it was, and it interleaved
prepared names with unprepared ones in the one report whose entire job is telling them apart. A
reader of `--unratified` on 2026-09-04 could not see which twenty-one were a read and which were a
research task, which is the failure the three states were introduced to prevent.

**The general form, and the one to watch for elsewhere: a vocabulary change swept the tool and not
the records the tool reads.** `script/names --check` gates on the presence of a block and on its
first token parsing, never on the token being the right one, and the header says so under BUGS. So
nothing could have caught this, and nothing would have caught it next time either.

### The second shape: a name is usually argued by the lane that refused it for something else

The 28 `recorded` outcomes were not found in the places one would look. Almost none came from an
introducing commit or from a milestone block about the thing itself. They came from lanes arguing
about a different name and reaching for this one as evidence:

- Milestone 63 settled `credentialer` by departing from the `clock`/`entropy` resource pattern and
  saying why the departure was earned, which is an argument for the pattern it departed from. That
  one sentence recorded two names it was not about.
- `system_initializer`'s ratification refused `system_builder` twice to protect the phrase
  `builder.rs` uses about itself, so the tree reasoned about `builder` by twice declining to give it
  away.
- DECISIONS §134, naming `script/falsifications`, wrote down the `script/` family's verb-and-noun
  split that nobody had recorded and listed `names`, `citations`, `decisions`, `roadmap`, `coverage`
  and `bench` as the evidence the family existed. Six names settled by a decision about a seventh.
- `undefined-behavior-check`'s ratification established the `-check` suffix and cited `qemu-check`
  and `shell-check` by name while doing it.

So `script/names --refused` is also an index of implicit justifications, and it is currently read
as only half of that. The refusals are the valuable half for the question "has this been proposed
before"; they turn out to be the valuable half for "why is this called that" as well, because the
place a name gets argued hardest is the moment somebody wants it for something else.

### The third shape: the backlog is almost exactly "named before the mechanism existed"

Of the 60, 33 were introduced between 2026-07-14 and 2026-08-03, and milestone 115 built
`script/names` on 2026-08-04. Every one of the later entries is either a chip driver from the RISC-V
board work or one of the 21 mis-tokened blocks above. The unrecorded tier is therefore not a habit
this project still has; it is the sediment from before it had the habit, plus a token nobody swept.

## BUGS

- **A recorded name is not a ratified one**, and this milestone deliberately does not reduce what
  calef owes. It changes 60 unanswerable questions into a smaller number of answerable ones, which
  is the whole of the claim.
- Some of the 60 will have no discoverable reason. A name coined in a lane that recorded nothing
  leaves a history that says only when it appeared. Saying so is the honest outcome and is what
  `unrecorded`'s own vocabulary is for; a plausible reconstruction presented as history would be
  worse than the gap.
- **`script/names` sorts by exposure and this block does not argue with that ordering.** Programs a
  person types come before scripts, and scripts before crates, which is the tool's judgment rather
  than a measured one.
- It touches ~60 files across `user/src/`, `crates/`, `script/` and `Cargo.toml`s, so it collides
  with almost anything. It should run when the tree is quiet, the same scheduling constraint
  milestone 91 carries.

## Follow-on

Five renames are proposed and none is performed, which is the split this block set out: milestone 63
did the renaming and this writes down why the survivors survived. Each case lives in the provenance
block beside the name it is about, which is milestone 115's mechanism and the reason none of these
needs a file of its own; what is listed here is where to read it and what happens if calef says no.

- **Done.** `crates/cred` is `crates/credentialer`, renamed 2026-09-08. This lane proposed
  `credential`; calef chose `credentialer`, which is better for a reason the proposal missed.
  AGENTS.md's naming section: *"a crate and a program may share a name, and it says something when
  they do: the crate is that program's logic, lifted out so it can be host-tested and Kani-reachable
  while the program keeps the IO."* This crate describes itself in exactly those terms and names
  `components/src/credentialer.rs` as its service, so `credential` would have fixed the contraction while
  creating a fourth shape the tree does not have: a crate named for the subject beside a program
  named for the actor, with nothing saying they are one thing lifted apart. `credential_proto` is
  untouched, because the wire is about credentials rather than about the servicer, the same shape as
  `fs_proto` beside a server.
- **Milestone 285.** `crates/user_rt` is `crates/user_mode_runtime`, ratified by calef
  2026-09-13, and `crates/user_heap` is `crates/user_mode_heap` with it. This entry asked only that
  the `rt` half be spelled out, as `user_runtime`, and named the circle underneath it: the `user_`
  half was on the record only because `user_heap`'s ratification cited *this crate* as establishing
  the prefix. calef broke the circle rather than the contraction, ruling on the prefix itself,
  which this entry had said was worth more than one name and was right about. `user_runtime` lost to
  the measurement: `user` means *a person* several hundred times in this tree, so the shorter prefix
  would have fixed the abbreviation and kept the ambiguity.
- **Done.** `user/src/driver.rs` is `serial_driver`, ratified by calef 2026-09-08. It was the
  fourth driver in that directory and the only unqualified one, beside `block_driver`, `gpu_driver`
  and `keyboard_driver`, and `keyboard_driver` was the precedent since it was `kbd` until
  2026-08-27. It drives the NS16550 for interrupt-driven console input. `uart_driver` lost on the
  2026-09-05 acronym test: *universal asynchronous receiver-transmitter* teaches a reader almost
  nothing about a program that reads console input, where `serial` is the plain word for the same
  wire. `console_driver` stays refused because `console` is already a program, and a driver named
  for another program invites a reader to think it drives that program rather than a device.
- **Milestone 296.** `script/initboot` wants the hyphen every sibling has, as `init-boot`, in
  that script's header. It is
  the one entry point of fifty-two that runs two words together, and it survived milestone 63's
  2026-08-01 hyphenation sweep rather than being exempted by it. `cargo xtask initboot` is likewise
  the only run-together subcommand beside `shell-check`, `board-console`, `initrd-aarch64` and
  `std-src`, so the change is two strings. The reversible half of the naming rule, and cheap.
  It got neither hyphen nor rename. Put to calef on 2026-09-14, he asked what the thing does,
  and the answer was nothing `--features shell` does not: the two features selected the same six
  `cfg` sites and a build of each carried the same 3,109 symbols at the same total size. He ruled it
  deleted, so the script, the subcommand and the feature are all gone and this bullet is closed by
  subtraction rather than by spelling. The header that carried this argument went with the script;
  it is in git and quoted in 296's block.
- **Recorded.** **`shell-check` wanted a name that is not one hyphen from `shellcheck`**, and got
  one; the argument is in `script/swish-check`'s header. `shellcheck` is the shell linter `script/lint` itself runs, so two names
  differ by punctuation while naming unrelated things. Compounding it, design/naming.md's BUGS already
  records that `shell` no longer names any program here, because the shell is `swish`. The case is
  for `swish-check`, naming the program actually driven, with `prompt-check` the alternative if he
  would rather name the surface. Two loose ends the rename carries without closing: the cargo feature
  is spelled `shell`, and `cargo xtask shell-check` is the same string one level along.
  Ratified 2026-09-23: calef ruled `swish-check`, and the script, the subcommand and the xtask
  module were renamed the same day. `prompt-check` was refused for naming a surface rather than the
  binary. The cargo feature is still spelled `shell` and was deliberately left, since it is not the
  name that misleads a reader at the command line. This bullet keeps the old name because it is the
  record of why the name lost.
- **Milestone 388.** The 2026-09-05 acronym test reaches at least seven of the sixty and this
  milestone deliberately settled none of them, because design/naming.md already says the sweep is
  its own milestone and one lane spelling one acronym out would leave a program disagreeing with a
  crate calef ratified. Each affected block asks the question and records the answer as open.
  Numbered on 2026-09-19 by milestone 433's drain of the pile.
- **Milestone 386.** The 21-block sorting bug this milestone found was invisible to every gate,
  because `script/names --check` validates that a block's leading token parses and never that it is
  the right one. A block saying "provisional" or "not yet put to calef" in prose while its token
  says `unrecorded` is mechanically detectable, and it is a false entry in the one record whose job
  is saying what is prepared. Numbered on 2026-09-19 by milestone 433's drain of the pile, by which
  time the 21 instances had been cleared and one `unrecorded` name of 222 was left, so what is
  numbered is the regression guard.
- **Recorded.** **`fixtures/src/hello.rs` carries the `init_boot` role on aarch64 and the name does not
  say so**, in that program's own block. Not proposed as a rename: `hello` is right for what it was
  and the gap is that the thing grew, which is a fact to record rather than a naming error.
- **Recorded.** The retired `gates` script was DECISIONS §134's one clear exception, a noun that
  acts where the family's rule gives nouns to things that report. Stated in its header rather than
  resolved, because both verb forms were worse: `check` collides with the `-check` suffix eight
  scripts already carry, and `gate` singular would have named one of three. Closed by milestone 286,
  which retired the script into `script/ci-build` rather than ruling its name.
- **Recorded.** `flaky` borrows a word it then contradicts, in its own block. The field's term
  means a test that fails intermittently; this program fails deterministically, once, by attempt
  number, so a reader who knows the word arrives with the wrong model. No rename proposed, because
  the replacement is a design question rather than a spelling one, and `chatty` beside it raises the
  same adjective question, which is one ruling rather than two.

The 31 names that were already `provisional` on 2026-09-05 are untouched on purpose: they are
prepared, and adding to their argument is not this milestone's business.

## Index row

100 of 204 names are unratified, and the 60 unrecorded ones have no argument at all. Bringing
calef a name with no argument is what the seven questions refuse, so this does the lookups and
leaves him a prepared set
