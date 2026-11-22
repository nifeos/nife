---
status: NOT-STARTED
raised: 2026-11-22
milestone_dependencies: 946
decision_dependencies: 307
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1122. Every name a start or activation message carries travels on a page

The rule in force, ruled 2026-11-22 (UTC): a name has one path and one limit. Every name that a
start message or an activation message carries travels on a page, and its bound is
`grant::MAXIMUM_NAME_BYTES` (252). No name rides two argument words, and no 16-byte bound sits
between a name and the program that reads it. The branch is `lane/names-ride-on-a-page`, and the
number is provisional.

What drives it: four programs refuse a name the file system holds without complaint. `cd` refuses
a directory over 16 bytes, `login` gives no subtree to an identity over 16 bytes, `jig install`
refuses a package over 16 bytes, and `vouch` refuses a file over 16 bytes. Milestone 31 (a
capability shell: designation is authorization) put a name in two words as a demonstrator's
limit. Milestone 946 (a granted file name can be longer than sixteen bytes) moved a file's name to
a page and left everything else on the words.

What it costs: one mapped page for each process that starts with a name. On aarch64
`bench/baseline-aarch64.txt` puts a page map near 491 ticks and a whole spawn near 11,800, so the
page is about 4 percent of a spawn.

Reuse: the one-name set that `rm` already falls back to (`components/src/swish/running.rs`,
`dir_grant`), `filesystem_contract::nameset`'s encoding and the page it travels on, and the frames
`vouch` already sends its file on. No new tool and no new crate.

## What moves

- A caretaker's granted directory. `grant_plan::navigation::MAXIMUM_DIRECTORY_NAME_BYTES` goes, so
  `cd` takes any name up to 252 bytes and `PathRefusal::DirectoryNameTooLong` goes with it. The
  directory reaches `fs_subtree_caretaker`, `fs_nameset_caretaker`, `client_windows::bound_channel`
  and the mounted grant on the caretaker's page. The kernel's own builders in
  `kernel/src/user/fs_service` (`directory_grant.rs`, `set_grant.rs`) write the same page.
- `rm`'s operand. It always travels as a set, which is the path a long operand takes today. `rm`
  stops reading a name from its words.
- A `login` identity's subtree. `mint` takes any identity up to `login_contract::MAXIMUM_IDENTITY`
  (64), which the page holds.
- `fs_file_caretaker`'s granted file, and `kernel/src/user/fs_service/file_grant.rs`, which builds it.
- The activation messages. `jig remove`'s name, `vouch`'s name and the approval file named in an
  answered approval travel on the page or frames the request already uses. This is milestone 963 (a
  package name longer than sixteen bytes installs and is removed). §307 (how long a name may be)
  rule 5 then has nothing to hold, and `jig install` checks 252.
- The private `pack_name` in `crates/program_table/src/spawn_protocol.rs`. It truncates to 16 bytes
  and does not refuse, against the contract's rule that nothing truncates a name. It goes with
  the trailer field it fills.
- Last, `grant::MAXIMUM_WORDS_NAME_BYTES`, `grant::pack_name`, `grant::unpack_name`,
  `grant::fits_words`, `grant_plan::MAXIMUM_WORDS_NAME_BYTES`, their tests and the Kani proof
  `a_packed_name_comes_back_whole` with its falsification patch.

## The path's representation

`DirectoryPath` holds eight components of 16 bytes by value, 137 bytes in all. At 252 bytes a
component the same layout is about 2 KiB. The shell carries one in every `ShellHoldings`, one in
each of four `BindEntry` values, and several in an `Endowment`, on a stack that milestone 1090 (an
outside review of `filesystem_contract`) found tight.

The recommendation is a packed buffer, as `nameset` packs a set. The buffer holds each component as a
length byte and its bytes, one after another, in one buffer of 512 bytes, with the depth still at most
eight. A path is then about 530 bytes. Its cost is a second bound beside the name's, the bytes of
the whole path, as `nameset::MAXIMUM_NAME_SET_BYTES` is beside `MAXIMUM_NAMES`. Two components of
252 bytes fit, or eight of 60. A path over it is a refusal at the prompt, never a truncation. The
fixed 2 KiB layout needs no new bound and is simpler to index. Its stack cost rules it out.
The bound is internal to the shell and can move later without a wire change.

## Exit

On aarch64, riscv64 and x86_64, the suite uses names of 17, 64 and 252 bytes. `cd` enters the
directory and `rm` removes a file in it. `login` gives the identity its subtree, up to the
identity's own 64. `jig install` and `jig remove` install and remove the package, and `vouch`
records the file. `rg
'MAXIMUM_WORDS_NAME_BYTES|pack_name|fits_words'` finds nothing in the tree.

## Index row

Every name a start or activation message carries travels on a page, with one bound of 252 bytes.
So `cd`, `rm`, `login`, `jig install`, `jig remove` and `vouch` take any name the file system holds,
and nothing on the way cuts a name short.
