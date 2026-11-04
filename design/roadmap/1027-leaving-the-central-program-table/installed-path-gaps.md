# Leaving the central program table: what an installed program cannot hold yet

[The proposal](../1027-leaving-the-central-program-table.md) holds the end state. This appendix takes
each grant the shipped tools declare and says what stops an installed program from holding it, what
code would let it, what approval rule fits, and what is risky.

## Two kinds of gap

`grant_plan::image_can_carry` in `crates/grant_plan/src/image.rs` refuses a manifest that declares
a file, a directory, an input, a short option, `interruptible`, a silent output or a second stream,
and every owner-granted field. Read against the callers, those refusals mix two different limits.

- **Policy:** what a vouched note may earn. The owner-granted fields are pure policy: the ruling of
  2026-11-02 (UTC) lets an owner's `grant` line earn each, and [the security
  appendix](security.md) bounds the console class.
- **Transport:** what the frames path can deliver. An image travels as page frames the shell
  copies from a file (`IMAGE_BIT`), and the progenitor carves its region from one image pool sized
  for a single largest image (`IMAGE_POOL_PAGES` in `crates/system_initializer/src/layout.rs`). The
  shell runs an image only on a plain line, one stage with no operator
  (`components/src/swish/capabilities.rs`, the plain-run check; `pipeline.rs` sets `image: false`
  for every stage). The progenitor refuses a held image (`spawn_loop.rs`, the `held && image`
  branch and its comment).

The end state's by-name path (part 5 in [the end state](end-state.md)) builds an archive program
the way a row builds today: from the parsed ELF the progenitor already holds, in the job pool,
held by the shell. So every transport gap below closes for base programs the moment they spawn by
name. The transport gaps stay open only for programs installed on a disk.

## Each grant

| grant | who declares it | what the frames path lacks | what would carry it | approval rule | risk |
|---|---|---|---|---|---|
| file (`FileGrantDeclaration::Required`) | no shipped program; only test manifests in `test_support.rs` and `planning/tests` | the caretaker wiring on the image path | the row path's file grant, reused | the person designates the file on the line, so the vouch suffices; unvouched bytes never (§219 (how the shell names an installed program to the spawner)) | low; nothing needs it |
| directory with a subtree option | `rm` | `run_image`'s `Wiring` sets `directory_grant` only for a words designation (a `std` program's names), never for a declared directory operand; `spawnproto` already orders image frames after the grant's data | `run_image` plans and sends the grant as `spawn` does; the progenitor builds the caretaker before an image job | the person names the directory and `caps` prints the subtree at risk before anything runs; vouch suffices | a replaced `rm` acts on whatever directory a person names, as on Unix; the grant stays as narrow as the line |
| short options | `rm` (`rfv`) | they ride inside the directory grant's words (`dir_grant(&g, e.short_options)` in `running.rs`), so they arrive with the directory grant and not alone | the directory grant above; a program with options and no directory would need its own carrier (inference: no such program exists) | options are not authority, except `subtree_option`, which `caps` prints | low |
| input | `wc`, `mdr`, `sink_consumer` | a pipe or `<` makes a line not plain | images as pipeline stages: a staging region per image stage, so a pool that holds more than one image | none; the line designates the stream | memory: two image stages need two staging regions, unmeasured |
| second stream (`BytesAndDiagnostics`) | `date`, `uuid`, `ps`, `pgrep`, `top`, `free`, `vmstat`, `slabtop`, `reboot` | `run_image`'s `Wiring` sets no `diagnostics`, `sink`, `source`, `interruptible` or `held`, so no endpoint follows for the second stream (`DIAG_BIT`) | `run_image` delegates the endpoint as `spawn` does; the progenitor reads the slot from the note | none; the stream exists because the program declares it (§67 (a program's second stream is a declaration)) | low; the smallest code change here, and it unlocks eight tools |
| silent output | `interrupt_heeder`, `interrupt_ignorer` | not stated in code beyond the refusal list; inference: the image path's answer reader expects bytes or a word | the image answer path accepting no output | none | low; fixtures only |
| interruptible | `interrupt_heeder`, `interrupt_ignorer` | an interruptible job is built from the shell's job budget (§309 (a supervised job's `ThreadControlBlock` goes to the shell that asked for it)), and an image's region comes from the image pool | carve an interruptible image's region from the shell's budget, sized by `image_region_pages` | none for the interrupt pair; it is the shell's | the budget must hold an image's region; unmeasured |
| held job, `&`, `jobs`, `fg` | every row the shell holds | the progenitor refuses `held && image` | the same region change as interruptible | none | as above |
| redirection `>` and `2>` | any tool a person redirects (`uuid > id.txt`) | not a plain line | the sink and diagnostics endpoints on the image path | none | low |
| verbs (`VERBS` note) | `jig`, and any replacement package manager | `endowed_image` refuses installed bytes with a verb table | `narrow_by_verb` reading the note's table at the shell and the progenitor, with the verb read from the argument page as `resolve` already does (migration step 7) | each verb's grants must lie within the note's manifest and its owner grants; the narrowing only removes | medium: a bug in narrowing hands a line every verb's grants, which is why the refusal exists |
| network, zones, service, directory by name | `jig`, `network_echo_client`, installed fixtures | carried today, by the owner's `grant` lines | already built (milestone 892 (a std program at the prompt holds the network and a resolver), milestone 911 (scripted installs grant only what the owner's approval file allows)) | the owner's grant at install, or the approval file | as ruled |
| owner-granted fields (`reboot`, `sync`, installer, catalog, index store, activation directory, console input, display) | `reboot`, `jig`, `graphical_terminal` | no note byte spells them, and `image_can_carry` refuses them | the `OWNER_GRANTS` note and an `OwnerGrants` vouch (migration steps 4 and 5) | an owner's `grant` line each, within [the security appendix](security.md)'s classes | per class: from a denial of service (`reboot`) to a console takeover (installer) |

## Which tools could move today

"Move" here means the row leaves the table and the program still runs at the prompt by its bare
name. Read strictly, no tool can move today without a loss.

- Three rows pass `image_can_carry` with their current manifest: `uptime`, `printenv` and
  `least_authority_demo`. `uptime`'s manifest is `NO_NOTE_MANIFEST` itself, and
  `packages/uptime.recipe.toml` already builds an installable `uptime`.
- Each would then run only when installed on a disk, through the live table. A boot with no
  writable filesystem would lose the name. The test boots that `script/swish-check` drives run
  `least_authority_demo 3 && echo yes` and similar plain lines, which the frames path serves, but a
  person's `printenv > env.txt` or `uptime | wc` would stop working.
- §229 (a bare name reaches an installed program) refuses to install a package whose program name
  equals a name the image carries, so the row and the install cannot both exist; the row must go
  first.

With the by-name path in place (step 10 of [the migration](migration.md)), every row moves with
no loss, because the archive still carries it. With only the second-stream
change on the frames path, eight tools become installable from a disk: `date`, `uuid`, `ps`,
`pgrep`, `top`, `free`, `vmstat` and `slabtop`. `rm`, `wc` and `mdr` need the directory or input
work as well.
