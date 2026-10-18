# A set grant at the prompt: how a matched pattern reaches the progenitor

Status: DECIDED. calef ruled on 2026-09-27 at 15:14Z (UTC), on #1402: option A as built by
milestone 205 (how a foreign program is told what to do), which is this note's option 3 with the
copy. The set travels as one read-only frame the shell owns (`spawnproto::NAMESET_BIT`, `1 << 43`,
name still provisional), and the progenitor copies it into a page from the job's region before
`fs_nameset_caretaker` maps it, so the shell's write access reaches nothing. The ruling's option B,
this note's option 2 (the set as data words), lost because it would be a second way to move bytes
to the progenitor, beside the frames `IMAGE_BIT` already uses. It landed before milestone 599 (a
frame per filesystem client channel)'s production pool, so the race this note's costs section
names is live at the prompt. The line is `std_grep needle docs > out.txt`: the shell, draining the
job into `out.txt`, writes the file's bytes through the file service's one shared frame while the
job's nameset caretaker forwards a checked name through it. A write landing between the caretaker's
re-staging and the server's read opens a name the set never approved. Reachable, not observed;
milestone 205's block records it, and 599's production pool closes it. What follows is the analysis
as it was written.

Written 2026-09-26 (UTC) by milestone 47 (navigation and
naming)'s lane `milestone/47-navigation`. It answers the item milestone 47's block and milestone 109
(`xargs`) both carry: "the shell cannot ask the progenitor to mint a per-batch caretaker." It is a
change to `spawnproto`, which the shell and the progenitor both read, so it is an architect's call.
The file name is provisional.

## The premise, checked, and it is wider than the record says

The record says `xargs <program>` stops after batch one. The cause reaches further. At the real
prompt, any pattern that matches more than one name is refused before anything spawns, with or
without `xargs`:

- `components/src/swish/running.rs`, `dir_grant`: `g.names.only()` is `None` for a set, and the shell answers
  "a set of names is delivered by a nameset caretaker, and the progenitor builds the subtree one;
  name a single file".
- `crates/grant_plan/src/spawnproto.rs`: `DIRECTORY_GRANT_BIT` announces two more `SEND`s carrying the caretaker's
  and the child's three start words. A single name fits in two words. A set does not, and no bit
  says one follows.
- The set caretaker exists and is proven (`components/src/fs_nameset_caretaker.rs`,
  `system_tests/src/user/glob_grant_tests.rs`), but only a kernel test harness wires it. The progenitor
  builds `fs_subtree_caretaker` and nothing else.

So `rm *.txt` over two files is refused at a keyboard today, and `xargs rm` stops at batch one for
the same reason: every batch is a set.

## What the caretaker needs that a subtree grant does not

`fs_nameset_caretaker` starts with the directory's name and rights in its three start words (the
subtree caretaker's shape) plus a read-only frame at `SET_VA` holding the encoded set
(`filesystem_contract::nameset`, `BYTES` = 137 for eight names of up to sixteen bytes). It copies the
set into a local at startup and never reads the frame again.

The progenitor must therefore receive up to 137 bytes of set per spawn, and build that frame.

## The options

| | What travels | Cost | Where it fails |
|---|---|---|---|
| **1. Leave it** | Nothing | Zero | A pattern at the prompt designates one name or is refused. `xargs <program>` never runs a batch |
| **2. The set as data words** (`SET_BIT`, provisional): with `DIRECTORY_GRANT_BIT`, up to six more `SEND`s of three words (24 bytes each) carry the encoded set | The progenitor decodes and checks the encoding (`nameset::count`), writes it into a frame retyped from the caretaker's region, maps it read-only at `SET_VA`. One transient slot for the frame, freed after the build, as `build_child` already does for segments | The shell cannot change the set after sending it. `DIRECTORY_GRANT_BIT`'s own "the protocol carries data, not capabilities" precedent, and the one this block's environment pricing (2026-08-18) already costed |
| **3. The set as a frame the shell owns** (§219 (how the shell names an installed program to the spawner) option D's shape): one `SEND_CAP` of a frame the shell wrote | One page from the shell's budget per spawn, one transient slot in the progenitor | The shell keeps write access, so the progenitor must copy before mapping or the caretaker's startup copy races the shell. That copy makes it option 2 with a page in the middle |
| **4. Batches of one**, no wire change: `xargs` plans one name per batch, delivered by the subtree caretaker that already works | One spawn per name. Unix's `xargs -n 1` | Works today for `xargs rm`. Does nothing for a plain `rm *.txt`, and it is a stopgap: if options 2 and 4 cost the same, 2 wins. It is recommended only as effort, and says so |

Recommendation: option 2, because the carrier is data the progenitor checks rather than a page
somebody else can still write. It keeps the progenitor's slot arithmetic where it is: no permanent
slot, one transient.

## Costs that must be measured, not asserted

- The progenitor's capability table. `kernel::cap::CAPABILITY_TABLE_PEAK_MEASURED` is 23 of 32
  since calef raised the table on 2026-09-27 (it was 23 of 24 when this was written, and the doc
  then said the next addition should buy a slot back). Option 2 adds one *transient* slot on a
  directory-granted spawn. Whether that spawn path's peak is below the login block's peak is
  unmeasured; `script/swish-check` fails loudly on this, as it did twice before.
- Shared-page audit finding 1 becomes reachable. `notes/shared-page-audit.md` records that one
  FS frame is shared read-write by every client and that the set caretaker checks a name, then the
  FS server reads it again from that frame. It is unreachable today because "the shell is the only
  holder". A prompt-built set caretaker running beside another writer of that frame (a `>` file
  caretaker in the same pipeline, say) makes the window live. The fix, a frame per client channel,
  is that note's proposal A, filed by this lane and since promoted to milestone 599 (a frame per
  filesystem client channel). Option 2 should wait for it or land with it.
- The progenitor must carry `fs_nameset_caretaker`'s ELF, parsed from the measured archive as
  `care_elf` is. Its cost in the progenitor's own pages is unmeasured.

## What this does not decide

The eight-name bound (`MAX_NAMES`), which stays a stack decision in the shell; `mv *.txt dir/`, which
a set capability cannot express by design (`notes/glob-grant.md`); and the provisional name
`SET_BIT`.

## The limits 2b asks for, proposed

calef's ruling asked the building lane to propose the page limit, its memory cost, and a new
`grant::MAX_NAME`. They are proposed here, before 2b is built, so the numbers can be ruled on first.

`grant::MAX_NAME`: 252 bytes. That is RedoxFS's own limit (`DIR_ENTRY_MAX_LENGTH` in
`vendor/redoxfs/src/lib.rs`). Unix's 255 would let a grant name a file the filesystem can never
hold, and 16 is today's two-register limit, which 2b's page removes.

The page encoding: a two-byte count, then per name one length byte, one byte of flags (the
directory bit a listing observed), and the name. A name of `n` bytes costs `n + 2`.

The limit: two pages, 8 KiB. That holds about 585 names of twelve bytes, and 32 of the longest.
Over it the shell refuses, names the limit, and suggests `xargs`, as the ruling says. It never
truncates.

| Who | What it costs | Why that is affordable |
|---|---|---|
| The shell | 8 KiB of its heap while it plans and sends | The heap is capped at 32 KiB and a line holds nothing else that large |
| The progenitor | Two transient pages copied from the shell's, checked, then mapped into the caretaker | From the job's own region, so the job's reap returns them. One transient capability slot, since the copy maps one frame at a time as `blobs` already does |
| The set caretaker | Two read-only pages, and no local copy | It reads the progenitor's copy, which nothing else can write |

One page would be simpler and would hold about 290 short names. The case for two is a directory of
a few hundred files, which is where `rm *.log` stops being rare. Either number is a constant and
cheap to change until something depends on it.
