# Milestone 47 appendix: `touch` and globbing

An appendix to [milestone 47 (navigation and naming)](../0047-navigation-and-naming.md). The page is
complete without it. It holds how `touch` works and what rights it needs, and how a glob becomes a
grant.

## `touch`. See notes/touch.md.

### Creating a file

`touch <name>` makes an empty file if the name is not there and does nothing if it is, using
`fs_proto::fs::CREATE`. It is a builtin in `mkdir`'s category, because it needs no more than
`CREATE` on the directory the shell already holds, which is `mkdir`'s right and not a new grant.
`rm` is a program because `-r` is a destructive recursive walk that should not run with the shell's
whole endowment. `touch` recurses over nothing and destroys nothing, so that reason does not apply.

### Setting the modification time

Bare `touch` on an existing name sets its mtime to now; `touch -t` sets it to whatever you say. They
are two rights, per DECISIONS §112 (`touch`'s two behaviors need two rights). The second is the
ability to lie about history, which matters for anything reasoning from mtime, backups included.
That is the asymmetry of §43 (reading the clock is a page) one level down: reading is harmless,
setting is an authority. POSIX's `utime()` draws the same line, requiring only write permission to
set the current time but ownership to set an arbitrary one. Plain `WRITE` covers "now". A separate
right, `dir::SET_MODIFICATION_TIME_AT` (provisional), covers "arbitrary". It is the seventh rung of the directory
rights ladder of DECISIONS §47 (a directory capability carries six rights), as extended by §112.

RedoxFS tracks the mtime, and `fs_proto` exposes it as three verbs: `fs::GETMTIME`, `fs::SETMTIME`
and `fs::SETMTIME_AT` (all three provisional names). They are three verbs rather than one with a
flag, because `file_system_contract::verb::TABLE` encodes one fixed rights requirement per opcode.
`GETMTIME` and `SETMTIME` need `dir::READ` and `dir::WRITE` respectively, resolved directly under a
directory handle like `UNLINK` (neither opens what it acts on). `SETMTIME_AT` needs
`dir::WRITE | dir::SET_MODIFICATION_TIME_AT`, with the caller's asserted seconds riding in the second word, for
`TRUNCATE`'s reason for using `w1` over the length field. `SYNC` also has a row in that table, so a
program confined to a subtree can `SYNC` through its caretaker.

`touch -t` takes an RFC 3339 instant (`date`'s own output format), not Unix's compact
`[[CC]YY]MMDDhhmm[.ss]`. That is an earned divergence: the tree has an RFC 3339 parser (`calendar`)
and no reason to build a second date grammar for one flag; see notes/touch.md's `BUGS`.

`kernel::user::shell_navigation_tests` proves the two-right split against a real,
narrower-than-`dir::ALL` grant (`TOUCH_NOW_NEEDS_ONLY_WRITE` / `TOUCH_AT_REFUSED_WITHOUT_SETTIME`)
and the round trip against a real command line (`TOUCH_MTIME_ADVANCED` /
`TOUCH_AT_ROUND_TRIPPED`), on both ISAs.

## Globbing, which decides how every multi-file operation grants

See notes/glob.md and notes/glob-grant.md. A glob is an enumeration, and the rights ladder already
separates `enumerate` out ([mkdir-and-rm.md](mkdir-and-rm.md)), so matching is the easy part. The
question is what a match grants. For `rm *.txt` with five hundred hits:

| Answer | Verdict |
|---|---|
| Grant 500 file capabilities | Honest, and it exhausts capability slots |
| Grant the directory plus a name list | Cheap, and it over-grants catastrophically: `rm` could touch anything in that directory, which is the thing this whole model refuses |
| Make `rm` a builtin so the shell deletes and nothing is granted | Dodges the question, and costs `rm` as a program |
| A directory capability attenuated to a name set | The principled one, and the one built |

So `rm *.txt` grants a directory capability attenuated to the matched name set, served by
`components/src/fs_nameset_caretaker.rs`. Nothing is new in the kernel, and the attenuation is
checkable from outside the confined program.

The expansion you see is the grant. `echo *.txt` prints literally the authority that `rm *.txt`
would transfer, because the matched set *is* the namespace the caretaker serves. Unix cannot make
that claim, since `rm`'s authority never came from the command line at all.

The shell expands, before planning the grant, as in Unix. So `grant_plan` sees the expanded set
rather than the pattern, since the endowment is the set. `plan_against` fills its slots by index
and takes an `Expansion` keyed to that index, and `DirGrant.names` is a set: a literal operand is
the set of one.

The rules that follow:

- `fs_nameset_caretaker` is its own caretaker, not a mode of `fs_file_caretaker` or
  `fs_subtree_caretaker`. `fs_file_caretaker` serves the *file* protocol, and
  `fs_subtree_caretaker`'s design property is that it performs no checks at all, which a name filter
  (on seven name-taking verbs) would end. The grants also differ: a name rides in registers, a set
  needs a frame.
- An empty match is a refusal, zsh's answer. Passing the pattern through, as bash does, would build
  a grant whose namespace is a name nobody has, which acquires a referent the moment somebody
  creates that file.
- `ARG_MAX` is eight names, set by the shell's stack rather than by reasoning: sixteen ran it off the
  bottom of its stack planning one grant. Exceeding the bound is a loud refusal at the prompt, never
  a truncation.
- `ARG_MAX` is a capability limit rather than a buffer limit: you cannot hand a child a hundred
  thousand capabilities. It wants Unix's answer, batching, which milestone 109 (`xargs`: batching a
  grant too large to hand over) provides as a shell prefix word.
- `xargs <program>` still refuses past the first batch, because the shell cannot ask init to mint a
  per-batch caretaker. That missing delegation chain belongs to this milestone, and 109's block says
  so.
- Qualifiers (`*(.)`, `*(om[1])`, `*(Lm+1)`) and `**` are out, for notes/glob.md's reasons, which
  are authority questions. Qualifiers need type, mtime and size *per candidate*, so one `enumerate`
  becomes N `FSTAT` calls and needs a read right beyond enumerate.
- Completion is also an enumeration, so the completion menu is a rendering of your authority and
  cannot offer a path no capability reaches.

Tests: `grant_plan` and `fs_proto` host suites; `kernel::user::glob_grant_tests` on both ISAs (a real
shell expanding one pattern two ways, then `rm` as its own attacker behind a real
`fs_nameset_caretaker`); and `xtask::redoxfs_glob_grant_took_exactly_the_match` reading the image
from outside the guest.
