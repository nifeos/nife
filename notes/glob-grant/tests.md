# Globbing: what the tests prove, and from where

*Name: provisional, minted 2026-10-11 (UTC) by lane/fourth-ten-longest-notes.*

An appendix to [`notes/glob-grant.md`](../glob-grant.md), which states the conclusion.

## On the host, in milliseconds

`cargo test -p grant_plan -p fs_proto` proves these:

- The expander's set is what matched and not what did not.
- An empty match and an oversized one are refusals.
- A matched name too long to grant refuses the whole expansion rather than dropping one name.
- The dot rule holds, and a pattern may sit only in the last component.
- The planner grants the set the expander produced, unnarrowed and unreordered.
- A literal operand ignores any expansion offered with it.
- The set encoding round-trips, and refuses rather than truncating.
- The fixture is matched by the pattern it is staged for. That is what makes the kernel test's
  literal set provably the expansion.

The batching is proved on the host too, by `cargo test -p grant_plan -p swish -p fs_proto`
(milestone 109 (`xargs`)):

- The batches partition the match, with no name taken twice or dropped.
- A batch is the smallest names in order rather than the first the directory yielded.
- The watermark is exclusive, and `more` says whether a batch follows.
- A sweep of a directory that shrinks under it takes every name exactly once. That is the case a
  cursor would get wrong.
- An empty batch ends a sweep, but an empty *first* batch is still `NoMatch`.
- The unnameable-match refusal survives batching.
- The sweep's account and its three wordings are pinned.
- The `globmany` fixture is over the bound, and splits exactly where the swish-check gate's two
  literal strings say it does.

## At a real prompt, both ISAs

`script/swish-check` runs three lines. Unbatched, the pattern is refused. Batched, its second batch
is `m-08.txt m-09.txt m-10.txt`, which pins the resume rule from one line. An off-by-one, a batch
that restarted, and a batch that took the first eight the directory yielded all print something
else. And `xargs caps rm` shows the second batch's grant naming those three names and nothing more.

## In the guest, both ISAs

`glob_grant_tests` is one `#[test_case]`, because the two phases are one argument and their order is
load-bearing:

1. A real shell in a real `fs_subtree_caretaker` expands one pattern two ways and reports agreement,
   plus the three refusals that stop the agreement being vacuous.
2. `rm` is the attacker. Told to remove `gl-three.log` through the set capability: the file
   exists, sits one directory entry away from the two names in the set, and the caretaker one hop up
   holds a capability that could remove it. `ENOENT`. Nothing in `rm` decided not to try.
3. And the grant works: `rm` in namespace mode removes exactly the two names, which is what stops
   claim 2 being equally true of a capability that reaches nothing.

## From outside the guest entirely

`xtask::redoxfs_glob_grant_took_exactly_the_match` is a different process, on the host, with the
pinned engine, reading the image the run left behind. The two matched names are gone, the two
unmatched ones are still there, and the unmatched directory still holds its file. What the guest
displayed is what disappeared.

## The two code paths, as examples

Moved verbatim from the parent's EXAMPLES on 2026-10-13 (UTC) under §212 (a prose budget).

Expand once and grant what was shown, from the shell's side:

```rust
// components/src/swish/: one expander, two callers
let shown = nav.expand(b"gl-*.txt")?;              // what `echo` prints
let e = grant_plan::plan(&spec, holdings(&nav), Expansion::at(0, shown))?;
assert_eq!(e.dir.unwrap().names, shown);           // and what `rm` would hold
```

Wire a set grant and attack it:

```rust
// system_tests/src/user/glob_grant_tests.rs
let report = fs_service::start_granted_set(
    blk_server_image(),
    program("redoxfs_server").unwrap(),
    program("fs_nameset_caretaker").unwrap(),
    program("rm").unwrap(),
    fs_service::SetGrant {
        dir: tree::GLOBSET,
        names: &[(b"gl-one.txt", false), (b"gl-two.txt", false)],
        rights: dir::REMOVE,                       // take names out, and nothing else
        role: filesystem_contract::grant::spec(0, 0),         // no name: the operand is the namespace
        arg: 0,
        arg2: 0,
        stack_pages: 4,
    },
)?;
```
