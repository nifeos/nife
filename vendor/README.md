# Vendored upstream code

Engines we pin by carrying the source in-tree, per milestone 32's vendored-engine discipline:
pin a version, carry patches, record divergence. Vendoring (rather than a registry or git
dependency) is what lets the pin carry a patch and keeps the build hermetic.

One directory here is not a pin: `relibc/`, a seed nife owns and does not track (below). It lives
here because it is somebody else's code in shape, and every gate already treats `vendor/` that way.

## redoxfs 0.9.1

The on-disk engine for milestone 32's FS server (design/roadmap/0032-redoxfs-fs-server.md; the audit that chose
and priced it is notes/redoxfs-audit.md).

- **Source:** the published crates.io package `redoxfs-0.9.1.crate`,
  sha256 `a66d0c043a5768739851a7e5775192a70fdffdbf3418c22fd7927415d41a87c3`,
  upstream git sha `473b4baeb041ebe14504f30693393b1cae52558c` (see `.cargo_vcs_info.json`).
- **Divergence from the published package, exhaustively:**
  1. `src/filesystem.rs`, `src/record.rs`: the two `use alloc::vec::Vec` imports that fix the
     bit-rotted no_std build (three E0425 sites). The same fix as
     `patches/redoxfs-no-std-vec-import.patch` (written against upstream master); each site is
     marked with a `cricker-os pin divergence` comment. Drops when upstream ships it.
  2. `Cargo.toml`: an empty `[workspace]` table at the top, marking this crate as its OWN workspace
     root (also commented as a pin divergence in the file). This is what keeps it out of the
     cricker-os workspace, so our `-D warnings` clippy gate and `cargo fmt` never touch upstream
     code we do not own, and its default features (which pull `fuser`, a macFUSE build on macOS)
     never ride into our builds. `tools/redoxfs_host` is a separate own-workspace crate for the
     same reason; a cricker-os *member* cannot depend on an in-tree crate that another workspace
     owns without a "multiple workspace roots" error, so both live outside.
  3. `src/header.rs`: `Header::update_hash` made `pub`. **This is the first divergence that changes
     what upstream OFFERS rather than fixing a build, and the distinction is worth keeping**, because
     the two age differently: 1 and 2 drop when upstream fixes them, while this one must be
     re-applied forever and can conflict if upstream changes the method.

     Why it was taken: `Header::new` is `#[cfg(feature = "std")]` purely because it calls
     `uuid::Uuid::new_v4()`, so a `no_std` caller cannot use it. Every `Header` field is already
     `pub`, so we can BUILD one and source the uuid from our own entropy service; what we could not
     do is finish it, because an unhashed header is an invalid filesystem. So this exposes an
     existing method rather than adding an API.

     Chosen over adding a `new_with_uuid` constructor deliberately (calef, 2026-08-03: the minimum
     viable divergence). A visibility change on an existing method is the smallest thing that
     unblocks `mkfs` on the target, and far less likely to conflict on a pin bump than a new
     constructor whose name and signature upstream might choose differently.

     **It is not sufficient, and nothing outside this directory calls it today** (milestone 57's
     write half, 2026-08-03). The premise above is that a `no_std` caller can build a header and
     therefore make a filesystem, and the second half does not follow. Making a filesystem is
     `FileSystem::create_reserved`, which lays down the tree list, the allocation list and the root
     node through `Transaction::write_block` and `FileSystem::reset_allocator`; **both are private,
     `Transaction::new` is `pub(crate)`, `sync_block`'s `AllocCtx` parameter names a trait the crate
     does not export, and three of `FileSystem`'s fields are `pub(crate)` so the struct cannot even
     be built from outside.** A caller holding a finished `Header` has nowhere to put it. That is
     what divergence 4 is for. This one is kept rather than reverted because it was a deliberate
     call and reverting one is calef's to make; it is a one-line drop whenever he wants it.
  4. `src/header.rs`, `src/filesystem.rs`: **the uuid becomes an argument, and the creation path
     builds for `no_std`.** `Header::new_with_uuid(size, uuid)` and
     `FileSystem::create_reserved_with_uuid(.., uuid)` hold what used to be `Header::new`'s and
     `create_reserved`'s bodies; the `std` entry points keep their signatures and pass
     `Uuid::new_v4()`, so no upstream caller changes. The encryption branch stays behind `std`
     (`Salt::new` and `Key::new` are `getrandom` too) and a `no_std` create with a password returns
     `ENOSYS` rather than quietly making an unencrypted filesystem.

     This is the same shape upstream already uses one line away: `create` takes `ctime` as a
     parameter because a `no_std` engine has no clock, and now takes the disk id as one because it
     has no randomness. The caller that supplies it is `mkfs`, which holds an entropy endpoint;
     **no randomness enters vendored code.**

     Ages like divergence 3 (re-applied forever, can conflict on a pin bump), and is the one most
     worth upstreaming: `patches/redoxfs-no-std-create-uuid.patch` is the submission, and it applies
     to the published 0.9.1 with zero fuzz. Approved by calef 2026-08-03.
  5. `src/lib.rs`, `src/record.rs`, `src/htree.rs`, `src/node.rs`: **the record level is lowered to
     1, and the constant is split in two.** Milestone 138 step 1, 2026-08-18. `RECORD_LEVEL` (the
     level a new file is *created* at) goes from 5 to 1, so a record is 8 KiB rather than 128 KiB;
     a new `RECORD_LEVEL_MAX`, still 5, is what the two `BlockTrait::empty` guards compare against
     and what sizes `RECORD_SIZE` and the lz4 scratch buffer. `node.rs` gains a comment only.

     **Why the value.** Every file request in this system carries at most one 4 KiB page, so a
     128 KiB record fetched 32 blocks to serve one and rewrote all 32 to change one. Measured on
     milestone 38's harness, six interleaved rounds on a quiet machine: a 4 KiB read goes from
     1,458 us to 284 us (**5.1x**) and a 4 KiB write from 2,400 to 797 us (**3.0x**). Level 1
     rather than 0 because RedoxFS compresses a record only when it is larger than one block:
     level 0 gives up lz4 for 8.7% more read speed and roughly double the space overhead
     (+38% against +19% on text). notes/benchmarks.md has the sweep and the two-term model.

     **Why the split, which is the part that is not about speed.** `record_level` is a per-node
     field in the on-disk format, so the level an image was written at is a property of that image
     and not of the code reading it. Upstream needed one constant only because the created level
     and the largest readable level were the same number by construction; lowering the first
     without separating the second would make every record stored above it answer `ENOENT` on an
     image that was perfectly good. The split costs one constant and makes the change reversible:
     nothing at any level from 0 to 5 becomes unreadable, and the next change of `RECORD_LEVEL`
     cannot orphan what this one wrote. It is also half of what a genuine per-file level needs,
     since the guards already compare against a maximum.

     **Ages like divergences 3 and 4** (re-applied forever, can conflict on a pin bump). The value
     is ours and upstream has no reason to want it. **The split alone plausibly is upstreamable**
     and there is no `patches/` entry for it yet: that directory is for patches written to be
     submitted, and nobody has written this one or opened the merge request. Recorded here rather
     than left implied, because a divergence with an upstreaming story and no submission is a thing
     a reader should be told about rather than discover.
  6. `src/node.rs`: **the level-4 record count is 8 * 256^4, not 12 * 256^4.** 2026-10-04, calef's
     ruling ("Can we patch our redox?"). `NodeLevel::new` bounds its last level at `12 * NUM^4`
     records, but `NodeLevelData::level4` holds eight pointers, so a write or truncate past about
     4.03 PiB (128 KiB records; 8 * 256^4 records at any record size) indexed `level4[8]` and
     panicked with `index out of bounds: the len is 8 but the index is 8`. With 8, those offsets
     return `None`, which `transaction.rs` already turns into `ERANGE`. The 4 PiB figure on the
     `level4` field's doc comment already says eight. Found by the `redoxfs_server` fuzz target (PR #1597).
     Checked against upstream HEAD `b87b0976ee12` (2026-10-04): the constant is still 12 there.
     Test: `node::node_level_ends_where_level4_ends`, which fails on the unpatched constant.
     **Falsification: attested 2026-10-04, not replayable by `script/falsifications`.** That script
     replays records only for workspace packages and skips `vendor/`; `vendor/redoxfs`,
     `redoxfs_server` and `tools/redoxfs_host` are each their own workspace, so no package that
     can reach `NodeLevel` is in its scope, and a patch file under `vendor/` would also fail
     `script/vendor-verify`. Replay by hand: set `L4` back to `12 * NUM * NUM * NUM * NUM` and run
     `cargo test --manifest-path vendor/redoxfs/Cargo.toml --lib --no-default-features --features
     std node_level`; it fails on `NodeLevel::new(end).is_none()`.

     **Not sent upstream, and the reason is policy rather than doubt about the fix.** Redox's
     CONTRIBUTING.md refuses LLM-generated contributions, and this change was written by an agent,
     so we carry it. `redoxfs_server`'s `MAX_FILE_END` cap answers `EFBIG` well before this
     engine's `ERANGE` can be reached, so the patch is defence in depth for the service and the
     real fix for any other caller of the engine.

     **At the next bump:** read `NodeLevel::new` first. If upstream changed the constant to 8 (or
     restructured the function), drop this divergence and its test with it; otherwise re-apply the
     one-line change and keep the test. Ages like divergences 3 to 5 if upstream never fixes it.

- Everything else is byte-identical to the published package, including files we do not use
  (`Makefile`, `test.sh`, upstream CI configs) and `Cargo.lock`.
- **Proved rather than asserted, since 2026-07-30:** `script/vendor-verify` fetches the published
  tarball, checks its sha256 against `redoxfs.pin`, applies `redoxfs.divergence.patch`, and requires
  the result to be byte-for-byte the tracked contents of this directory. The two items above **are**
  that patch. After a deliberate change, regenerate with `script/vendor-verify --write-patch` and
  extend the list here; anything else is drift.
- **A correction, because the first run of that check found one.** This file used to carry a third
  divergence: "`Cargo.lock` present and committed. The published library package ships without one."
  That was wrong twice over. The published package *does* ship a lockfile, and ours was not
  upstream's: deleting it and letting cargo regenerate re-resolved 25 dependencies to whatever was
  current that day (`syn` split across 2.x and 3.x, `jiff` 0.2.31 to 0.2.35, `proc-macro-error2`
  gone). Nobody had touched the filesystem code, but nobody could have proved that either, which is
  the whole problem. Upstream's lockfile is restored, `--no-default-features --locked` builds green
  on both bare targets against it, and the claim is now checkable.
- **License:** upstream's own `LICENSE` (MIT), unchanged. The cricker-os dual-license terms do
  not apply inside this directory.
- **Feature use here:** the kernel-facing consumer (phase 2's FS server) builds it
  `--no-default-features` (pure no_std core); `tools/redoxfs_host` builds it with `std` only,
  deliberately not `fuse`, so host mkfs/inspection needs no macFUSE. The FS server still only ever
  opens an existing image (roadmap §32, port plan item 4); since divergence 4 the *creation* path
  builds for `no_std` as well, and `mkfs` is the one program that uses it. What stays std-gated
  is the randomness: `FileSystem::create`, `create_reserved`, `Header::new` and the encryption
  branch, all because they invent a value rather than take one.
- **Kept honest by:** `cargo xtask test` runs the host round-trip test (`cargo test --manifest-path
  tools/redoxfs_host/Cargo.toml`) and builds the no_std core for both bare-metal targets
  (`cargo build --manifest-path vendor/redoxfs/Cargo.toml --no-default-features --target ...`), so
  the pin cannot bit-rot silently. `script/lint` and `script/fmt` gate the host tool by the same
  `--manifest-path`, since it is outside the main workspace their `--workspace`/`--all` sweeps see.
  `script/vendor-verify` asks the different question those cannot: not "does it still build" but
  "is this tree what we say it is".

## relibc, seeded at 893a3b9133ac (not a pin)

The C library's relibc half, for milestone 835 (a C library, stage 1: files, clock and memory) and
§265 (a C library started from relibc, whose Rust platform layer holds the capabilities). The crate
that compiles it, and nife's own half (the platform layer, `malloc`, the start of a C program), is
[`c_library/`](../c_library/README.md).

- **Source:** `gitlab.redox-os.org/redox-os/relibc` at commit
  `893a3b9133ac2fb3089f71b02d5b61d145d97968` (2026-10-07), MIT (`relibc/LICENSE`, "Copyright (c)
  2018 Redox OS"), seeded 2026-10-10 (UTC). Every seeded file's first line says so.
- **Not a pin, deliberately.** notes/c-library.md measured it: Redox does not accept
  LLM-generated contributions, so a nife platform layer can never go upstream, and tracking
  upstream would mean rebasing a private fork whose divergence outgrows what it patches. So there
  is no `.pin`, `script/vendor-verify` and `script/vendor-watch` do not cover it, and an upstream
  fix is ported by hand from reading, with a commit naming the upstream one. A bug nife finds in
  relibc's generic code goes back as a bug report.
- **What was taken:** relibc's root modules (`c_str`, `io`, `sync`, `fs`, `error`, `out` and the
  rest), its `Pal` and `PalSignal` traits, its types, and the header modules a stage-1 program
  reaches: `assert`, `ctype`, `dirent`, `errno`, `fcntl`, `float`, `getopt`, `inttypes`, `langinfo`,
  `limits`, `locale`, `malloc`, `math`, `signal`, `stdio`, `stdlib`, `string`, `strings`,
  `sys_mman`, `sys_stat`, `sys_time`, `sys_types`, `sys_uio`, `sys_utsname`, `sys_wait`, `time`,
  `unistd`, `utime`, `wchar`, `wctype`, and the `bits_*` type modules. Its static headers
  (`include/`) and its one C file (`c/stdlib.c`).
- **What was left behind:** `redox-rt`, `platform/redox`, `platform/linux`, `ld_so` (the dynamic
  linker and TLS), `start.rs` and `crt0`, dlmalloc, `pthread` (stage 2, milestone 836), sockets and
  `netdb` (stage 3, milestone 837), `spawn` (milestone 838), terminals, `crypt`, `regex`, and every
  header module above not listed. The thirty-odd crates relibc depends on are gone too except
  `libm`; c_library/README.md says what replaced each.
- **How it diverges:** every edit says `nife:` where it is, with its reason. The kinds: relibc's
  Linux `cfg` arms are extended to `target_os = "nife"` (nife takes Linux's generic C ABI values);
  `#[thread_local]` statics become single cells (one thread); the `syscall()` function and the
  Linux signal trampoline are not built (no syscalls, §31 rule 1); functions that need what stage 1
  lacks were removed (`pthread_*`, pseudo-terminals, `crypt`, `alarm`, timers); time zones are UTC
  on `crates/calendar`; `printf` reads a `double` vararg as the soft-float ABI passes it; and
  `getopt_long_only`, which relibc lacks, was added.
- **The headers** in `relibc/include/` are generated from the modules by cbindgen 0.29.0, as relibc
  generates them, and committed. `helpers/c-library-headers.sh` regenerates them; no build runs it.
- **Gates: a dated exception (2026-10-10, UTC).** Like everything under `vendor/`, the seed is
  outside `script/lint`'s unsafe census, its `unsafe fn` contract check, its citation and
  house-style checks and `cargo fmt`, and about 580 of its `unsafe fn`s carry no `# Safety`
  section. That is a hole, not a design: calef ruled on #1896 (2026-10-10) that the seed comes under
  the unsafe census and the contract check, in milestone 868 (relibc's seed comes under the unsafe
  gates). This entry is removed when it lands. The code that is nife's, `c_library/`, is under
  every gate now.
- **`math.h`** is relibc's own Rust `math` module over the `libm` crate (calef, #1896). relibc
  offers it behind its opt-in `math_libm` feature; its default build compiles openlibm instead
  (`USE_RUST_LIBM` empty in its Makefile), which was not seeded.

## Bumping a pin

**Nothing here bumps itself, and that is deliberate.** A newer version is not automatically a better
one, and each pin above was taken on purpose. What the tree does do, since milestone 203, is refuse
to let a gap go unnoticed: `script/vendor-watch` asks crates.io and upstream git what has landed
since, the monthly `vendor watch` workflow runs it, and `upstream-status.md` in this directory is
the answer. `script/vendor-verify` is the complement and answers a different question: not "has
upstream moved" but "is this tree what we say it is".

When the watch says upstream has moved and somebody decides to follow it, this is the job. **None of
it is mechanical**, which is why nothing automates past the first step:

1. Raise `version`, `url` and `sha256` in the pin. (`script/vendor-watch --write` does this much,
   and stops there.)
2. Re-apply the divergences. Numbers 1, 2 and 6 above drop the day upstream ships them; 3, 4 and 5 are
   re-applied forever and are where the work is.
3. Regenerate the patch: `script/vendor-verify --write-patch`.
4. Extend this file's divergence list, which claims to be exhaustive and has been wrong once.
5. Run `script/test` and milestone 37's crash injector. **The store's safety claim is the thing a
   bump risks**, so a green build is not the bar.

### BUGS

- **A bump is allowed to fail, and that is a result rather than a defect.** Divergences 3, 4 and 5
  can stop applying outright if upstream restructures what they touch. The honest outcome then is a
  decision rather than a fix: take the new version and rewrite the divergences, stay put and record
  why, or fork permanently and stop pretending the pin tracks upstream. Which of the three is right
  is calef's call, not a lane's, so a lane that meets this writes it up and stops.
- **The divergence list above is not exhaustive, and it says it is.** Found 2026-08-31 by milestone
  203's lane, which had to read the patch to report which files upstream had also touched.
  `redoxfs.divergence.patch` modifies eight files; the five numbered divergences account for seven.
  The eighth is `src/transaction.rs`, where `err` is renamed `_err` in the lz4 decompression failure
  path so the no-`log` build has no unused-variable warning. It is small and it ages like
  divergence 1 (it drops the day upstream writes it that way), but an "exhaustive" list with an
  entry missing is the same defect this file already recorded once about the lockfile: nobody
  edited the filesystem, and nobody could prove it either. Whoever takes the next bump should
  number it and say where it came from; this lane did not, because inventing provenance for a
  change it did not make would be worse than naming the gap.
- **Should RedoxFS become its own repository? Not yet** (calef asked 2026-10-04 UTC; the
  maintainer's answer). Six divergences, about 140 changed lines, fixes that land atomically with
  their callers and one CI is cheaper than a second repo. Revisit at about ten divergences, at a
  pin bump that conflicts badly, or when another project wants the fork; because upstream refuses
  LLM-generated contributions, the divergences are likely to grow rather than drop.
- **The watch reports; it cannot decide.** A prompt nobody acts on is the same silence with more
  steps, and nothing in this tree measures whether anyone acted.
- **`script/vendor-watch` speaks GitLab and nothing else.** RedoxFS is the only vendored engine, so
  a second forge gets support the day a second pin needs one. An unrecognised `vcs_url` says so and
  exits rather than guessing.
- **The git half compares against a branch head, which moves.** So "37 commits behind" is a fact
  about the day it was generated, and `upstream-status.md` carries no date by design (a timestamp
  would make every monthly run a diff, and therefore a pull request). Read it as "as of the last
  time this file changed".
