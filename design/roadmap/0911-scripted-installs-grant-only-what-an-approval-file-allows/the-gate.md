# What was built, and the gate row by row: milestone 911

An appendix of [milestone 911](../0911-scripted-installs-grant-only-what-an-approval-file-allows.md),
under §212 (a prose budget)'s cap. The block's Built section names the pieces. This file carries them with
their host tests, and then the Done-when rows row by row. The block owns the design; this file
only reads it.

## What was built

Lane `milestone/911-scripted-installs-approval-file` (#2853), 2026-10-28 (UTC). Per item of the
work:

1. The grant record (`crates/installed_programs`): `Granted` carries `Service`,
   `Directory(name, rights)` and `Installer` beside the network and the zones, each one `grant`
   line, sorted and once. The `answered` line (`answered console`, or `answered approval <name>
   sha256:<digest>`) is the how-it-was-answered column, one per generation: install and remove
   write it, and `installed_programs::answered` reads it; two are no single answer. The subset
   test is `covers` and `extras`, which never grant the intersection and name every extra grant.
   Host tests: `two_paths_write_one_record_and_their_answers_differ` (Done-when 4's bytes), the
   strict answered-line reads, the new kinds' round-trip, and the subset test's extras.
2. The approval file (`crates/approval_file`): the TOML grammar, read strictly, refusing at
   the line that fails; `compile` writes the canonical records, and `check` is item 6's host-side
   call. Host tests: the example file, thirteen refusals, the compile round-trip.
3. `--approved` on install and remove; an update is an install of a newer version (`jig` has
   no upgrade verb; that is 809's), and the flag is on it.
4. `Approve` and `Revoke` on the installer endpoint, and the delegated badge, with
   §270 (a package manager holds an installer endpoint, not the spawn endpoint) amended in place. The store sits at `activation/approvals/<name>`, read per request, so a
   revoke deletes and a re-approve replaces with no restart and no new endpoint. The delegated
   check runs a `Rollback` against the badge's approval, every row and its grants.
5. The one-run delegation: `--within <name>` on a console line
   (`grant_plan::delegated_within`), the endpoint badged with the approval's id.
6. The host-side check: `approval_file::check`, over item 1's `extras`; 909 calls it with the
   packages' notes' asks.
7. `--json`: one object per result or refusal, exit codes documented in `jig`'s header (0, 1,
   2), and a subset-test refusal naming each extra grant as data.

The seed writes the packages and the approval files (`xtask/src/disk.rs`). `servant`'s note asks
to run as a service. `seeker` at 0.1.0 and 0.2.0 carries two notes asking one zone apart.
`lab-runner` is the owner's file, which bounds every package the live generation holds; the seed
writes it edited too (one zone more allowed), and writes `runner`, the second approval.

The rows are the approvals topic (`xtask/src/swish_check/script/approvals.rs`), which
`script/swish-check` types in its second boot, on aarch64, riscv64 and x86_64, every leg, no
omits.

1. A scripted install succeeds with no prompt, and the package runs with exactly its asked
   grants: `jig install --approved ./lab-runner.toml ./seeker.nifepkg` asks nothing, which the
   answer's arrival proves, and `seeker` runs and prints its line.
2. An update whose manifest adds a zone fails, names the zone, and leaves the previous version
   installed and running: 0.2.0 asks `example.net` beside `nife.test`; the refusal names it in
   the block's shape, and `seeker` 0.1.0 still runs after it.
3. A delegated endpoint cannot install outside its file. It cannot install a listed package
   asking more than its entry. It cannot send `Approve`. Under `--within lab-runner`, the
   progenitor refuses `dialer`, which no entry bounds, and 0.2.0, which asks one zone more. It
   answers `jig approve --within lab-runner` with `NotServedHere`. It refuses a delegated removal
   of `seeker`, whose entry says `remove = false`, and a delegated reinstall of `servant`, whose
   entry says `update = false`. After `jig revoke lab-runner`, the progenitor answers the
   delegated install of `noteless` with `Revoked`, so the holder can install nothing.
4. An interactive yes and a scripted install write byte-identical grant records for the same
   grants. The host test `two_paths_write_one_record_and_their_answers_differ` proves the bytes.
   The gate runs both installs and runs the package under each. The record's own text is one
   encoding either way.
5. An edited file changes nothing until `jig approve` runs again, which shows only the changed
   grants; a delegation already running then installs within the new records, with no restart and
   no new endpoint: The stored records refuse 0.2.0. `jig approve ./lab-runner-edited.toml`
   prints `lab-runner, changed since the stored approval:` and one line, `looks up names in
   example.net`. The same delegated install then passes, against the same approval name and the
   same badge mechanism.
6. A refused scripted install exits with its documented code, and `--json` names each extra
   grant: `jig` documents the codes (0, 1, 2); the refusal's data form is the host-visible
   shape, and the gate asserts the refusal text.
7. A delegated holder rolls back to a generation within its approval, and the progenitor refuses
   a rollback to one holding a row outside it, naming it: the delegated rollback passes while
   every restored row lies in `lab-runner`. After the owner installs `dialer` interactively, the
   progenitor refuses the delegated rollback and names it; the live generation is unchanged.
8. A rollback leaves approvals unchanged: `jig approve` stores `runner` after the generations a
   later rollback restores, and `jig revoke` drops `lab-runner` before that rollback. The owner's
   rollback restores the packages and leaves both approvals exactly as they were: the revoked one
   still refuses, and the fresh one still allows, which its delegated install of `servant` proves.

Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file, for
the block's Built section's gate. calef names files.
