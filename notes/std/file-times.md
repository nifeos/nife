# File times: `modified` and `set_times`

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

*An appendix to [`notes/std.md`](../std.md), which is the page to read. This file holds milestone
64's last pass: the mapping for each time call, why the handle shapes refuse, its EXAMPLES and its
BUGS.*

The records this file cites by number:

- milestone 64 (enough `std` to run somebody else's crate)
- milestone 47 (navigation and naming)
- §112 (`touch`'s two behaviors need two rights)


## File times: `modified` and `set_times` (milestone 64's last pass)

Both bind to the contract's `GETMTIME`, `SETMTIME` and `SETMTIME_AT` verbs (DECISIONS §112).

The mapping, verb by verb:

| `std` call | what it does here |
|---|---|
| `std::fs::metadata(p).modified()`, `DirEntry::metadata().modified()` | Bound. `stat` walks to the directory holding the name (asking `dir::READ`, as it always did), runs `OPEN`/`FSTAT`/`CLOSE` for the size and kind, then `GETMTIME` for the time. Works for a directory as for a file. |
| `std::fs::set_times(p, t)`, `set_times_nofollow`, `FileTimes::set_modified` | Bound, always to `SETMTIME_AT` with the caller's seconds, walking with `dir::WRITE \| dir::SETTIME`. Sub-second part truncated. |
| `File::metadata().modified()`, `Dir::metadata().modified()` | Refused, `Unsupported`: the contract asks by name and a handle has none. |
| `File::set_times`, `File::set_modified` | Refused, `Unsupported`, same reason. |
| `FileTimes::set_accessed` passed to `set_times` | Refused whole, `Unsupported`, and nothing is changed: no verb sets an access time. |
| `metadata(".")` / `metadata("/")`, `set_times(".")` | The granted directory itself has no name; `modified()` and `set_times` refuse. |
| `accessed()`, `created()` | Refused: no verb carries either. |
| `SETMTIME` (set to the server's own "now") | Not used by std, deliberately (below). |

Why the handle shapes refuse rather than remember the name. The PAL could record the name a `File`
was opened by and ask `GETMTIME` with it. The answer would be about whatever holds that name *now*.
After a rename it would report another file's time, after an unlink a `NotFound` for a file the
caller is holding open, and `set_times` the same way would stamp the wrong file and return `Ok`.
POSIX's `fstat` and `futimens` act on the inode, and a binding that silently acted on a name instead
is the answer-instead-of-refusing failure this note keeps recording. A handle-taking form is a wire
change and is proposed in design/roadmap/0504-an-mtime-for-an-open-file.md.

Why `set_times` never falls back to `SETMTIME`. A `SystemTime` from the caller is an assertion
about history even when it came from `SystemTime::now()`, and §112 put that authority behind
`dir::SETTIME`, separate from `dir::WRITE`. So a grant without `SETTIME` answers
`ReadOnlyFilesystem`. Falling back to `SETMTIME` would write the server's own "now" instead of the
time asked for and report success, and that "now" is not a time at all (next paragraph).

A refused `GETMTIME` does not fail `metadata()`. The size and the kind are still true, and a
`metadata()` that failed because one field could not be read would break every caller that only
wanted `is_dir()`. The reply word is kept and `modified()` reports it. Through a per-file grant the
caretaker refuses all three mtime verbs with `ENOTDIR` (it has no directory to resolve a name
under). `modified()` says that in its message rather than reporting `NotADirectory`, which would
read as a fact about the file.

EXAMPLES, as `std_exerciser` runs them on all three architectures:

```rust
let t = std::fs::metadata("motd")?.modified()?;          // the host tool's stamp: a real second
std::fs::write("f", b"x")?;
std::fs::set_times("f", FileTimes::new().set_modified(t))?; // SETMTIME_AT, needs WRITE | SETTIME
assert_eq!(std::fs::metadata("f")?.modified()?, t);       // whole seconds round-trip
let f = File::open("f")?;
assert_eq!(f.metadata()?.modified().unwrap_err().kind(), ErrorKind::Unsupported);
```

BUGS:

- A file nife writes while the clock reads unknown shows early 1970. Once the clock page knows the
  time, the FS server stamps from the wall clock. Before that every mutation stamps a counter
  (notes/touch.md's `BUGS`), and the contract says Unix seconds. So `modified()` of such a file is
  `UNIX_EPOCH` plus a few seconds. The PAL cannot tell, because `SETMTIME_AT` may legitimately assert
  a small number.
- A write only ever moves a timestamp forward. The engine (`vendor/redoxfs`'s `write_node` and
  `truncate_node`) never moves an mtime back. A file the host tool made keeps its host time through
  any write with an earlier stamp, which is every write while the clock reads unknown. A file that `set_times` moved to the future
  stays there. So `std_exerciser` asserts a write moving a *made* file forward, before the
  `set_times`, which is what the stamp can promise.
- Whole seconds only. The wire carries seconds, so `set_times` stores a zero nanosecond part and
  truncates. A second-granularity filesystem does the same on Unix and std documents
  precision as platform-dependent, so this is recorded rather than refused.
- A time past `i64::MAX` seconds is refused, though the server would accept it: `GETMTIME`'s
  reply word is signed, so the time could be written and never read back.
- `metadata` costs four messages plus the walk, where it cost three, and `DirEntry::metadata` of a
  directory now costs a walk where it used to cost nothing (it answered from the listing, with no
  time). The four messages are not atomic: a name replaced between `OPEN` and `GETMTIME` reports the
  new node's time with the old node's size.
- Not exercised through a narrowed grant from `std`. Every std test grants the mount root with every
  right, so the `ReadOnlyFilesystem` a grant without `SETTIME` answers is proved by
  `redoxfs_server`'s host tests (the rights table beside `settime_at`) rather than through this PAL.
  The same gap [the descent section](fs-descent.md#what-is-proven-and-what-is-not) records for
  `OPENDIR`, and closed the same way.
