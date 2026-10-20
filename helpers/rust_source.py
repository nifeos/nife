"""Reading this tree's Rust source, for the `script/` entry points that count things in it.

**Why this file exists.** Three derivations were written twice, once in a gate and once in the
dashboard, because each lived inside a `#!/bin/sh` script's inline `python3` heredoc: the `unsafe`
census (`script/lint` and `script/metrics`), the comment-and-literal strip the code-line and
comment-line split is built on (the same pair), and the proof-harness count (`script/falsifications`,
`script/metrics`, and a third derivation inside `script/lint`). A gate that changed its definition
would leave the dashboard quietly asserting the old one, with both looking authoritative and nothing
firing. See milestone 236 (three derivations are copied between scripts, and nothing notices when
they drift).

**The premise that made the copies look unavoidable is false, and checking it is what shrank this
milestone.** `script/lint` and `script/metrics` both `cd` to the repository root before they
`exec python3`, so their heredocs have a stable working directory, and a heredoc with a stable
working directory can `sys.path.insert(0, 'helpers')` and import like anything else. Nothing about
being a shell entry point prevented it. That matters because the alternative on the table was a host
crate, which would have made three `script/` commands depend on a `cargo build` and changed what a
`script/` command is; this changes nothing a caller can see.

**What is deliberately NOT here, and it is the part worth reading.** The file *sourcing* stays in
each caller, and only the derivation over text is shared. `script/lint` walks `git ls-files` and
reads the working tree, because a gate is asked about the tree in front of it. `script/metrics`
streams blobs out of `git cat-file --batch` at eight historical revisions and never checks anything
out, because a report is asked about the past and AGENTS.md forbids the checkout that would be
needed. That difference is exactly the tolerance milestone 236's block warned a shared
implementation might dissolve, so it is kept where it belongs: in the input, not in the count.

**And the harness count is still derived twice, on purpose.** `script/lint` and
`script/falsifications` attribute each harness to a workspace package from `cargo metadata`, which
needs a checked-out workspace and a cargo that can parse it. `script/metrics` cannot have either: it
reads blobs from July, nothing is on disk, and its own header records that nothing there is allowed
to build. `harness_count` below is the pure-text derivation the report uses; `script/lint` keeps its
own and **compares the two**, which is the honest answer for a question two callers genuinely cannot
ask the same way.

Every function here takes `files`, an iterable of `(path, text)` pairs, with `path` relative to the
repository root and slash-separated.
"""

import pathlib
import re

# The comment and literal stripper. Rust source is not a regular language, so this is lexer-shaped
# rather than a parse, and every count built on it is honest rather than close: a naive grep for
# `unsafe {` over this tree scores 1071 against the census's 1057, and the fourteen it invents are
# all `unsafe {}` written inside a `//!` doc example in `intrusive`, `inter_process_communication`,
# `paging` and `user_mode_heap`. Those are documentation ABOUT unsafe, and a ceiling that counted
# them would fire when somebody improved a doc comment.
#
# Block comments are matched non-greedily and do NOT nest, which Rust's do. The tree has none
# nested; a nested one would end the strip early and could only ADD to a count, which fails loud
# rather than quiet.
#
# The raw-string body is `.*?` and NOT `(?:.|\n)*?`, which it was until 2026-10-07 (UTC). Under
# `re.S` a `.` already matches a newline, so the old alternation gave every newline two ways to
# match, and an unterminated `r"` followed by n newlines cost 2^n backtracking steps: 24 newlines
# took a second, and each one more doubled it. That is CodeQL alert 26 (py/redos), and the input
# is any Rust file a branch adds, so one bad file would hang `script/lint` rather than fail it.
# `_selftest` below holds the pathological case.
_NON_CODE = re.compile(
    r'/\*.*?\*/'                        # block comment
    r'|//[^\n]*'                        # line comment, doc comment included
    r'|r(#*)".*?"\1'                    # raw string, any hash count (re.S: `.` spans lines)
    r'|"(?:[^"\\\n]|\\.)*"'             # ordinary string
    r"|'(?:[^'\\]|\\.)'",               # char literal (a lifetime has no closing quote, so it is
    re.S)                               # left alone, which is what we want)


def strip_non_code(text):
    """Blank every comment and literal, keeping the line structure so line numbers still line up.

    Replacing with spaces rather than deleting is what makes `non_blank` on the result a **code
    line** count: a comment block and a multi-line string literal both become blank lines and
    contribute nothing, so it is a quantity nobody can move by writing prose.
    """
    return _NON_CODE.sub(lambda m: re.sub(r'[^\n]', ' ', m.group(0)), text)


def non_blank(text):
    """Lines with something other than whitespace on them."""
    return sum(1 for line in text.split('\n') if line.strip())


# --- the unsafe census (milestone 134) -----------------------------------------------------------
#
# Raised by calef on 2026-08-18 as one question: "how much unsafe code is there in a code base, and
# is that something we should be monitoring and driving in a particular direction over time?" See
# notes/register-of-measures.md and notes/unsafe-obligations.md.
#
# Shapes assumed, each checked against the real tree rather than against expectation:
#   - a block is `unsafe` then optional whitespace then `{`, MINUS the `unsafe extern "C" {` form,
#     which is a linkage declaration and not a block anybody reasons about (zero in the tree today,
#     subtracted anyway so the day one lands it does not read as unsafe code);
#   - a thread-safety claim is an `unsafe impl` of Send or Sync SPECIFICALLY, which is a different
#     animal from an `unsafe impl` of an unsafe trait (`GlobalAlloc`, `intrusive_fifo::Node`): the
#     trait is unsafe there because its own contract is, while `unsafe impl Send for T` is a
#     hand-written assertion that the compiler is wrong about T, and it is the most consequential
#     unsafe in this tree. The `[^{;]*?` tail keeps it inside one item header so a body cannot
#     donate a match.
#
# There is deliberately NO `unsafe fn` count here. `script/lint`'s `==> unsafe fn contracts` check
# derives one and prints it, and a second count of the same thing taken from a slightly different
# scope is the drift milestone 134 exists to stop.
UNSAFE_BLOCK = re.compile(r'\bunsafe\s*\{')
UNSAFE_EXTERN = re.compile(r'\bunsafe\s+extern\s+"[^"]*"\s*\{')
THREAD_SAFETY = re.compile(r'\bunsafe\s+impl\b[^{;]*?\b(?:Send|Sync)\s+for\b')
# Every `unsafe impl`, so the census can report the OTHER population too: an `unsafe impl` of an
# unsafe trait (`GlobalAlloc`, `intrusive_fifo::Node`, `ns16550::RegisterSpace`). Counted as all
# impls minus the thread-safety claims above, so the two numbers partition one population and
# cannot overlap (milestone 139 (drive the unsafe count down), 2026-10-07 UTC).
UNSAFE_IMPL = re.compile(r'\bunsafe\s+impl\b')
# A `static mut` declaration, counted for milestone 891 (a mutable static becomes an atomic, a lock
# or a recorded cell); the population's name is provisional. Not unsafe code itself, but every
# access to one is, and its soundness argument lives in a comment. The lookbehind keeps `&'static
# mut T`, a reference type, out: the lexer leaves a lifetime alone, so `'static mut` survives the
# strip and would otherwise match. A bare grep scored about 90 for that reason when the census found
# 52.
STATIC_MUT = re.compile(r"(?<!')\bstatic\s+mut\b")
# Test code is out of this one population and only this one, matching the census the milestone was
# raised from (notes/lint-census-2026-10-09.md). The system tests and the fixtures are harnesses
# whose single-threaded shape is the test's own premise, and the block's scope left them out. The
# `#[cfg(test)]` items elsewhere are not stripped: none declares a `static mut` today, and one that
# did would count, which is the loud direction.
STATIC_MUT_TEST_CODE = ('system_tests/', 'fixtures/')

# What is out of the census, and every exclusion has a reason rather than a convenience.
#
# `bench/host/` holds the Linux and macOS programs the cross-OS comparison runs: ~100 `unsafe`
# blocks of libc FFI that those operating systems require and that say nothing about nife's
# soundness. Counting them would move the census every time somebody added a comparison. `xtask/`,
# `tools/`, `fuzz/` and `helpers/` are build and image tooling that runs on the developer's Mac and
# cannot fault the kernel.
#
# `patches/` is excluded for a different reason and it is the one worth reading: it holds our
# platform layer for Rust's `std`, which DOES run on the machine, so this is a real hole rather than
# a tidy boundary. It is out because a ceiling asserts a direction, and that code's shape is
# upstream's rather than ours: it implements `std`'s internal interfaces and cannot be restructured
# to hold fewer unsafe blocks without diverging further from the crate we are trying to track. It
# also matches the `unsafe fn contracts` check's scope, so there is one answer to "which Rust is
# ours" instead of two. The 37 blocks it leaves uncounted are recorded in
# notes/register-of-measures.md's BUGS, where a reader meets them.
#
# The list is exclusions rather than inclusions on purpose: a new subsystem directory is counted by
# default, and being left out has to be somebody's decision.
HOST_ONLY = ('bench/host/', 'xtask/', 'tools/', 'fuzz/', 'helpers/', 'patches/')


# The counted-claim name `density` below is held to: `script/lint` registers it, and
# `script/metrics` reads each week's ceiling back by it, so the chart's line is the gate's.
UNSAFE_DENSITY_MARKER = 'unsafe-density-outside-arch'


def unsafe_census(files):
    """Every unsafe number this tree tracks, in one pass over the Rust that runs on nife.

    Returns `outside_arch`, `inside_arch`, `thread_safety`, `trait_claims`, `static_mut`,
    `code_lines` and `density`.

    **Density rather than a raw count, measured before it was chosen.** Outside
    `kernel/src/arch/` the block count went 171 to 747 between 2026-07-15 and 2026-08-18, almost all
    of it the system being BUILT rather than anything drifting, so a raw ceiling would have fired on
    nearly every lane. Per 10,000 code lines the same period reads 22.8, then 11.8, then 9.3,
    falling at every sample: the tree is getting proportionally safer, and holding that claim is
    what the ceiling is for. Per 10,000 rather than per 1,000 because the counted-claim markers read
    integers, and 93 keeps a digit that 9 would throw away.

    The denominator is the non-arch code only, so the density and its numerator describe the same
    code. Mixing arch lines in would let assembly-heavy months dilute a number that is deliberately
    not about assembly.
    """
    out = {'outside_arch': 0, 'inside_arch': 0, 'thread_safety': 0, 'trait_claims': 0,
           'static_mut': 0, 'code_lines': 0}
    for path, text in files:
        if path.startswith(HOST_ONLY):
            continue
        code = strip_non_code(text)
        blocks = len(UNSAFE_BLOCK.findall(code)) - len(UNSAFE_EXTERN.findall(code))
        claims = len(THREAD_SAFETY.findall(code))
        out['thread_safety'] += claims
        out['trait_claims'] += len(UNSAFE_IMPL.findall(code)) - claims
        if not path.startswith(STATIC_MUT_TEST_CODE):
            out['static_mut'] += len(STATIC_MUT.findall(code))
        if path.startswith('kernel/src/arch/'):
            out['inside_arch'] += blocks
        else:
            out['outside_arch'] += blocks
            out['code_lines'] += non_blank(code)
    # Truncated rather than rounded: a ceiling must never fail a tree that sits exactly on it.
    out['density'] = (10000 * out['outside_arch'] // out['code_lines']
                      if out['code_lines'] else 0)
    return out


# --- the unsafe census, split by trust boundary (milestone: unsafe census by trust boundary,
# provisional) -------------------------------------------------------------------------------------
#
# `unsafe_census` above answers "how much unsafe code is there in the tree", and one population
# mixes two things that mean opposite things in a capability microkernel: an `unsafe` block inside
# `kernel/src` runs with no confinement over it at all, and an `unsafe` block in a userspace program
# is confined by the same MMU-plus-capability-table mechanism that confines every other program.
# `notes/trusted-base.md` (the `maintainer/redleaf-comparison` lane, 2026-09-20, landing alongside
# this one) hand-computed a first split -- kernel/src/** total 577, "everything else that runs on
# nife" 561 -- and flagged in its own BUGS that nothing keeps it computed. This is that mechanism.
#
# **The split that hand computation did NOT make, and the one this file exists to make**: crates/
# is not one population. `crates/paging` and `crates/direct_memory_access_validator` were lifted out
# of `kernel/src` on purpose so a model checker could reach them (see the BUGS in
# notes/trusted-base.md, which names exactly this risk: "a tree can shrink [the kernel line count]
# by moving code out of `kernel/src` without reducing what anyone has to trust"). A crate that ships
# ONLY in the kernel binary is the trusted base wherever it lives on disk; a crate that ships ONLY
# in a userspace program is confined wherever it lives; and roughly half of `crates/` ships in BOTH,
# which this split reports as a bucket of its own rather than guessing.
#
# **Where a `crates/` member's side comes from.** `helpers/trust_boundary_crates.tsv`, one row per
# member, generated from `cargo metadata` by `helpers/trust_boundary.py`, whose docstring holds the
# rule. `script/lint` regenerates it and fails when the checked-in file differs, and the pre-push
# hook fails when a member has no row, so a crate added since the last `--write` cannot fall through
# unseen. `script/metrics` never runs cargo; it reads the file from the working tree and applies
# today's table to every historical week, the restatement trade `MILESTONE_STATUSES`/`NAME_STATUSES`
# make in that script.
#
# **A `shared` crate is not a hedge.** `environment_contract` and `clock_contract` are the clearest
# cases: the kernel builds the shared page (`PageBuilder`) and a userspace `std` program reads it
# back through the identical `unsafe fn new`/`from_raw_parts` accessor, so the SAME unsafe source
# genuinely executes with kernel privilege in one binary and under confinement in another. There is
# no single number that is not either an overcount (attribute it to both) or an undercount
# (attribute it to neither), so it is reported as its own line rather than folded into either side.
#
# **What is not attempted here.** Some of a `shared` crate's unsafe may in fact be reached only from
# the kernel's own `#[cfg(test)]` modules (several of the Cargo.toml comments say exactly this: "the
# kernel does no date arithmetic of its own; its tests predict what the command printed"), which
# would mean it never ships in the production kernel binary at all. Telling that apart from unsafe
# the kernel's real logic calls needs a source-level, per-call-site read of each of the (currently
# five) `shared` crates that carry any `unsafe`, which this pass did not do; it is recorded as
# future work rather than guessed at.


def _crate_buckets(path):
    """{crate dir: bucket} from `trust_boundary_crates.tsv`, retired rows included."""
    return {line.split('\t')[0]: line.split('\t')[1] for line in path.read_text().splitlines()
            if line and not line.startswith(('#', 'crate\t'))}


CRATE_BUCKETS = _crate_buckets(pathlib.Path(__file__).with_name('trust_boundary_crates.tsv'))
# The `crates/` members no binary on nife links (`stick_maker`, `board_console` and the like). They
# stay in `unsafe_census` (its `HOST_ONLY` is paths, and the lint ceiling keeps its meaning); this
# split leaves them out, because kernel privilege or userspace confinement has no answer for code
# that runs on neither.
HOST_TOOL_CRATES = frozenset(c for c, b in CRATE_BUCKETS.items() if b == 'host')

# Each of these is its OWN cargo workspace (an empty `[workspace]` in its own `Cargo.toml`), outside
# the main one, so it is matched by path. `std_exerciser`'s and `entropy_backend`'s headers give the
# reason they stay out: build-std flags and a `getrandom_backend` cfg the workspace crates must
# never inherit. By its own header each runs on nife at EL0 and never as kernel code:
# `redoxfs_server`'s `el0` feature is the filesystem server; `std_exerciser`, `cryptography_exerciser`,
# `pinned_tls_exerciser`, `jig` and `mdr` are std programs for the `*-unknown-nife` targets;
# `entropy_backend` is the `getrandom` backend in every userspace `std` program;
# `cryptography_provider` and `pinned_tls_client` are the TLS glue those programs link; `c_library` is
# the C library C programs link. `helpers/trust_boundary.py` reads each as a userspace root, and its
# `--check-rows` fails on a workspace of its own that this list and its host list both miss.
USERSPACE_ONLY_DIRS = ('redoxfs_server/', 'std_exerciser/', 'entropy_backend/',
                       'cryptography_exerciser/', 'cryptography_provider/', 'c_library/', 'jig/',
                       'mdr/', 'pinned_tls_client/', 'pinned_tls_exerciser/',
                       # `fs_server/` (2026-08-01 to 2026-08-25) and `fs-server/` (before that) are
                       # `redoxfs_server`'s own past names (its Cargo.toml header has the history);
                       # a historical week's tree carries whichever name was current, never both.
                       'fs_server/', 'fs-server/')

# `uefi_loader` is neither side: it runs once, before the kernel starts, with the full privilege of
# the pre-OS environment, to decide WHICH kernel image gets control. `notes/trusted-base.md` draws
# the trusted base at "the kernel plus the hardware" and means the isolation boundary the kernel
# enforces at runtime; `uefi_loader` has come and gone (its memory reclaimed) before that boundary
# exists, so it is a boot-chain-of-trust question rather than a runtime-isolation one, and folding it
# into either side would misstate which claim it backs. Reported as its own bucket; which claim it
# belongs to is an architect's to decide. A `crates/` member only it links is `boot_chain` in the
# table.
BOOT_CHAIN_DIRS = ('uefi_loader/',)


def trust_bucket(path):
    """Which trust-boundary population a source file belongs to, or `None` for one this split
    has no opinion about (already out of every census, e.g. `vendor/`, or excluded above as host
    tooling).

    `user/src/` is milestone 175's predecessor to `components/`/`fixtures/` (renamed 2026-09-13);
    a historical week's tree carries whichever name was current then, never both, so both are
    handled here rather than only in the caller.

    BUGS: `system_tests/` is in `unsafe_census` and in no bucket here (78 blocks on 2026-10-20
    UTC). It is a test kernel image: its code runs at EL1, but no shipped kernel links it, and
    whether test-only kernel code is trusted base is not decided. So the five buckets sum to less
    than the census by that much plus the host-tool crates.
    """
    if path.startswith(HOST_ONLY):
        return None
    if path.startswith(BOOT_CHAIN_DIRS):
        return 'boot_chain'
    if path.startswith(USERSPACE_ONLY_DIRS):
        return 'userspace'
    if path.startswith(('components/', 'fixtures/', 'user/src/')):
        return 'userspace'
    if path.startswith('kernel/src/'):
        return 'kernel'
    if path.startswith('crates/'):
        bucket = CRATE_BUCKETS.get(path.split('/')[1])
        if bucket == 'host':
            return None
        # A `crates/` directory the table has no row for: on today's tree `script/lint` refuses
        # that, so it is a crate renamed or removed since a historical week this split restates
        # (`script/metrics` can never run cargo against a historical revision). Counted rather than
        # dropped, so the gap is visible instead of a silent undercount.
        return bucket or 'unclassified'
    return None


def trust_boundary_census(files):
    """The unsafe census, split into kernel/userspace/shared/boot_chain/unclassified.

    Same input as `unsafe_census` (path, text pairs over the Rust that runs on nife, or once did),
    same stripper, same block pattern. Returns `<bucket>` and `<bucket>_code_lines` for each of
    `kernel`, `userspace`, `shared`, `boot_chain` and `unclassified`, plus `kernel_density` and
    `userspace_density` (blocks per 10,000 code lines, truncated like `unsafe_census`'s own): the
    two populations large enough and unambiguous enough to hold a ceiling, if one is ever wanted.
    `shared` and `boot_chain` get no density; see this file's own comment above for why a single
    number for either would be a guess.
    """
    buckets = ('kernel', 'userspace', 'shared', 'boot_chain', 'unclassified')
    out = {b: 0 for b in buckets}
    out.update({b + '_code_lines': 0 for b in buckets})
    for path, text in files:
        bucket = trust_bucket(path)
        if bucket is None:
            continue
        code = strip_non_code(text)
        blocks = len(UNSAFE_BLOCK.findall(code)) - len(UNSAFE_EXTERN.findall(code))
        out[bucket] += blocks
        out[bucket + '_code_lines'] += non_blank(code)
    for b in ('kernel', 'userspace'):
        cl = out[b + '_code_lines']
        out[b + '_density'] = (10000 * out[b] // cl) if cl else 0
    return out


# --- the proof-harness count, the text-only derivation -------------------------------------------
#
# The falsification record milestone 194 built and DECISIONS §134 ratified: what evidence each Kani
# harness carries that it can fail. `replayable` has a patch a script applies to turn the harness
# red; `attested` is a person who broke it and watched; `unfalsified` is the claim's honest
# denominator. A harness with no record at all counts as unfalsified, which is what it is.
#
# `unfalsifiable` is the fourth state (§134 (a harness carries a machine-replayable falsification
# record, or it is not evidence), amended 2026-10-06): somebody looked for a defect in
# the code under proof that could turn the harness red on its own property, found none, and wrote
# down why beside the record. It is counted apart from both sides, because it is neither evidence
# that the harness can fail nor "nobody tried yet", and `script/falsifications --check` refuses one
# without a written reason of real length, so it cannot be used to empty the unfalsified count.
#
# Name: ratified 2026-10-06 (calef, in session, relayed by the maintainer to the lane that built it,
# #1730: "unfalsifiable ratified."). Refused `exempt` (reads as a waiver from the rule rather than a
# finding about the harness), `examined` (says an effort happened and not what it found; `attested`
# already owns "a person did something"), `by design` (true of only one of the first two such
# harnesses, the other guards a future regression, and two tokens grep and align worse, `by hand`'s
# reason in §134), `unbreakable` (claims no change can break the property, when one of the two
# exists precisely because a future change could), and `vacuous` (a harness that cannot fail on
# today's code is not vacuous, and vacuity is already the `cover!` check's word). These were the
# proposing lane's alternatives; calef ratified over them without comment on any.
PROOF = re.compile(r'#\[kani::proof\b')
FALSIFICATION = re.compile(
    r'\bFalsification:\s+(replayable|attested|unfalsifiable|unfalsified)\b')

# Directories holding `#[kani::proof]` that belong to no workspace package, expressed as path
# prefixes because that is all a text-only derivation has. `helpers/` is the `kani-lint-shim` source
# `script/lint` compiles by hand; `patches/` is our `std` platform layer. `vendor/redoxfs` is the
# third such file and is already outside every caller's file set.
NOT_A_PACKAGE = ('helpers/', 'patches/')


def falsification_records(text):
    """The state of each `Falsification:` record that annotates a `#[kani::proof]`.

    Text-only, like everything else here, and the approximation is stated rather than hidden: a
    record is attributed to the **first attribute** that follows it, skipping the rest of its own
    comment run. That is what `script/falsifications` does with a real module walk, and it is exact
    for every shape this tree uses, because the block is defined as the comment run immediately
    above the attribute. A record separated from its attribute by a blank line would be missed here
    and by `script/falsifications` both, which is the same answer rather than a drift.
    """
    lines = text.split('\n')
    out = []
    for i, line in enumerate(lines):
        hit = FALSIFICATION.search(line)
        if hit is None:
            continue
        for j in range(i + 1, min(i + 40, len(lines))):
            s = lines[j].strip()
            if s.startswith('//'):
                continue
            if PROOF.search(s):
                out.append(hit.group(1))
            elif s.startswith('#['):
                continue  # another attribute between the comment and the one that decides
            break
    return out


def harness_count(files):
    """Proof harnesses and how many carry a falsification record, from the source text alone.

    `script/lint` and `script/falsifications` answer the same question by attributing each harness
    to a workspace package out of `cargo metadata`, which is the better derivation where it can run
    and is why they keep it. This one exists for `script/metrics`, which reads blobs from revisions
    that are not checked out and could not run cargo against them if it wanted to. `script/lint`
    runs both and fails on a disagreement, which is what keeps the two honest.
    """
    total = falsified = unfalsifiable = 0
    for path, text in files:
        if path.startswith(NOT_A_PACKAGE):
            continue
        # Stripped, so the sentence in `kernel/src/syscall.rs` explaining what a `#[kani::proof]`
        # is does not count as one. A raw grep counts 151 where the tree has 146.
        total += len(PROOF.findall(strip_non_code(text)))
        # Raw, because a falsification record lives in a doc comment by construction, and
        # attributed to what it annotates, because since milestone 305 a record may sit above a
        # kernel `#[test_case]` instead of a `#[kani::proof]`. `total` counts harnesses, so a
        # record counted here that belongs to a test would put two different populations in one
        # fraction: it inflates `falsified` and deflates `unfalsified` by the same amount, and
        # `script/lint`'s drift check against `script/falsifications --count` reports the
        # disagreement rather than either being wrong. Nine such records arrived at once and it
        # fired, which is the check working.
        kinds = falsification_records(text)
        falsified += sum(1 for kind in kinds if kind in ('replayable', 'attested'))
        unfalsifiable += sum(1 for kind in kinds if kind == 'unfalsifiable')
    return {'total': total, 'falsified': falsified, 'unfalsifiable': unfalsifiable,
            'unfalsified': max(total - falsified - unfalsifiable, 0)}


# --- does a diff change compiled code, or only its comments ----------------------------------------
#
# The question `script/verify --affected-since` and `script/falsifications --affected-since` ask of
# each changed `.rs` file in a harness crate's closure, so that a comment edit (a spelling sweep, a
# path rewritten inside a comment) does not replay every proof. Found 2026-10-06 (UTC) on #1734,
# which rewrote `proposals/<slug>.md` paths inside comments across many crates: its re-falsify leg
# reached every record and was cancelled at the 45-minute limit.
#
# **A lexer and not `strip_non_code` above**, because the two answer opposite questions. That one
# blanks string literals along with comments, which is right for counting code lines and wrong
# here: an edit to a string literal changes the compiled program and must count. Nor a regex over
# lines, which reads the `//` in `"http://..."` as a comment. `rustc -Z unpretty` was the other
# candidate and lost because it parses rather than lexes: it wants the crate's modules on disk at
# one revision, and here the base revision is a blob that is not checked out.
#
# The rule. Two texts are the same program when their token sequences are equal with comments
# removed. Whitespace and comments are both separators to rustc, so a comment is replaced by "a
# separation happened" and nothing else. Spacing is kept only where rustc keeps it, between two
# adjacent punctuation characters (`>>` against `> >`, rustc's `Spacing::Joint`); between two words
# a separation already makes two tokens, and next to a word it carries no meaning.
#
# **Doc comments (`///`, `//!`, `/** */`, `/*! */`) are removed too, by default, and that is a
# claim with two parts.** First, a doc comment can hold a doctest, and a doctest is not a Kani
# harness and not a falsification record: `cargo kani` never runs one, so editing it cannot change
# a proof's verdict. Second, a doc comment is a `#[doc = "..."]` attribute, which is metadata unless
# a macro reads it and makes it code (`clap`'s derive turns it into help text, `displaydoc` into a
# `Display` impl). The caller passes `docs_are_code=True` when the dependency graph holds a
# procedural macro outside `DOC_BLIND_PROC_MACROS`, so a new one arriving makes doc comments count
# until somebody reads it and adds it to the list. A `macro_rules!` that captures `$(#[$m:meta])*`
# and stringifies it can do the same, and nothing checks for that (BUGS in script/falsifications).

# The procedural macros in this tree's dependency graph that never read a `#[doc]` attribute into
# code, read from each crate's derive or attribute (2026-10-06, UTC): `serde_derive` reads
# `#[serde]`, `thiserror-impl` reads `#[error]`, `defmt-macros` reads its format strings, and
# `rustversion` reads the compiler version.
DOC_BLIND_PROC_MACROS = frozenset({'defmt-macros', 'rustversion', 'serde_derive', 'thiserror-impl'})


class LexError(ValueError):
    """The text is not lexable Rust (an unterminated literal or comment). Callers fail toward
    treating the file as changed code."""


def _word(c):
    return c == '_' or c.isalnum()


def code_tokens(text, docs_are_code=False):
    """The token sequence of Rust source with comments removed, literals kept byte for byte.

    Each punctuation token carries a trailing `+` when the next token is punctuation with nothing
    between them (joint), so the sequence distinguishes `>>` from `> >` the way rustc does.
    With `docs_are_code`, a doc comment stays in the sequence as a token of its own.
    """
    toks, i, n = [], 0, len(text)
    joint = False  # was the previous token punctuation with no separation since?

    def emit(tok, punct=False):
        nonlocal joint
        if punct and joint and toks:
            toks[-1] += '+'
        toks.append(tok)
        joint = punct

    def string_end(j):
        # j is just past the opening quote of an escaped (non-raw) string.
        while j < n:
            if text[j] == '\\':
                j += 2
            elif text[j] == '"':
                return j + 1
            else:
                j += 1
        raise LexError('unterminated string')

    while i < n:
        c = text[i]
        if c.isspace():
            joint = False
            i += 1
            continue
        if text.startswith('//', i):
            end = text.find('\n', i)
            end = n if end < 0 else end
            body = text[i:end]
            is_doc = (body.startswith('///') and not body.startswith('////')) or body.startswith('//!')
            if docs_are_code and is_doc:
                emit(body.rstrip())
            joint = False
            i = end
            continue
        if text.startswith('/*', i):
            depth, j = 1, i + 2
            while depth:
                if j >= n:
                    raise LexError('unterminated block comment')
                if text.startswith('/*', j):
                    depth, j = depth + 1, j + 2
                elif text.startswith('*/', j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            body = text[i:j]
            is_doc = ((body.startswith('/**') and not body.startswith('/***') and body != '/**/')
                      or body.startswith('/*!'))
            if docs_are_code and is_doc:
                emit(body)
            joint = False
            i = j
            continue
        if _word(c):
            j = i
            while j < n and _word(text[j]):
                j += 1
            word = text[i:j]
            # A literal prefix: b"..", c"..", b'x', and the raw forms r".." br#".."# cr"..".
            if word in ('r', 'br', 'cr') and j < n and text[j] in '"#':
                k = j
                while k < n and text[k] == '#':
                    k += 1
                if k < n and text[k] == '"':
                    close = '"' + '#' * (k - j)
                    end = text.find(close, k + 1)
                    if end < 0:
                        raise LexError('unterminated raw string')
                    emit(text[i:end + len(close)])
                    i = end + len(close)
                    continue
                # `r#ident`, a raw identifier: fall through and emit it as one word.
                if word == 'r' and k == j + 1 and k < n and _word(text[k]):
                    m = k
                    while m < n and _word(text[m]):
                        m += 1
                    emit(text[i:m])
                    i = m
                    continue
            if word in ('b', 'c') and j < n and text[j] == '"':
                end = string_end(j + 1)
                emit(text[i:end])
                i = end
                continue
            if word == 'b' and j < n and text[j] == "'":
                i = j  # the byte char is lexed as a char literal below, prefix included
                end = _char_end(text, i)
                if end is None:
                    raise LexError('a b prefix before something that is not a byte literal')
                emit('b' + text[i:end])
                i = end
                continue
            emit(word)
            i = j
            continue
        if c == '"':
            end = string_end(i + 1)
            emit(text[i:end])
            i = end
            continue
        if c == "'":
            end = _char_end(text, i)
            if end is not None:
                emit(text[i:end])
                i = end
                continue
            # A lifetime or a label: the quote and the name are one token.
            j = i + 1
            while j < n and _word(text[j]):
                j += 1
            emit(text[i:j])
            i = j
            continue
        emit(c, punct=True)
        i += 1
    return toks


def _char_end(text, i):
    """End of the char literal opening at `text[i] == "'"`, or None if it is a lifetime."""
    n = len(text)
    if i + 1 < n and text[i + 1] == '\\':
        end = text.find("'", i + 3)  # past the backslash and the escaped character
        if end < 0:
            raise LexError('unterminated char literal')
        return end + 1
    if i + 2 < n and text[i + 2] == "'":
        return i + 3
    return None


def comment_only_change(old, new, docs_are_code=False):
    """True when `old` and `new` are the same program and differ at most in comments and spacing.

    False when the token sequences differ, and False when either side does not lex, because a
    caller that cannot tell must run the proofs.
    """
    try:
        return code_tokens(old, docs_are_code) == code_tokens(new, docs_are_code)
    except LexError:
        return False


def _selftest():
    """The stripper's fixtures, run by `script/lint` before anything trusts a count built on it."""
    import time

    # Raw strings still strip whole, across lines and with hashes, and keep the line structure.
    src = 'let a = r#"x\n"y"\n"#; let b = 1;\n'
    out = strip_non_code(src)
    assert out.count('\n') == src.count('\n'), out
    assert 'x' not in out and 'y' not in out and 'let b = 1;' in out, out

    # Alert 26: an unterminated raw string over 27 newlines. The old pattern took about seven
    # seconds here on an M-series Mac and doubles with each newline; the fixed one is linear, so a
    # full second of slack cannot flake on a loaded runner and still fails the exponential version
    # instead of hanging the gate on it.
    started = time.monotonic()
    strip_non_code('r"' + '\n' * 27)
    elapsed = time.monotonic() - started
    assert elapsed < 1.0, f'raw-string strip took {elapsed:.2f}s: catastrophic backtracking is back'
    # A `static mut` counts once, a reference type spelled `&'static mut` and a comment about one do
    # not, and the test harnesses are out of that population.
    src = ("static mut A: u8 = 0;\nfn f(x: &'static mut u8) {}\n// static mut B\n"
           "pub(crate) static  mut C: u8 = 0;\n")
    assert unsafe_census([('k/a.rs', src), ('system_tests/b.rs', src)])['static_mut'] == 2
    print('rust_source selftest: ok')


if __name__ == '__main__':
    import sys
    if sys.argv[1:] == ['--selftest']:
        _selftest()
    else:
        sys.exit('usage: python3 helpers/rust_source.py --selftest')
