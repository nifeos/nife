# The system initializer's page budgets

An appendix of [the system initializer note](../system-initializer.md). Every budget below is a
constant in `crates/system_initializer/src/layout.rs` or `children.rs`, and each was counted or
measured rather than chosen. A spawner states the budget it hands, even where the kernel's test
wiring states the same number. No crate could carry a number that both the kernel and a user-mode
spawner read.

## Stacks

`CHILD_STACK_PAGES` is twelve. Eight faulted twenty-four bytes below the lowest page once `2>` put a
second `FileOut`, each with a 256-byte staging buffer held by value, on the shell's `run_pipeline`
frame. Twelve leaves four pages of headroom, because each smaller raise bought exactly enough and
the next change found the wall again. The cost is 48 KiB of address space per child.

`SHELL_STACK_PAGES` is twenty-one, for the shell alone. A file's name may be 255 bytes (milestone
946), an `Endowment` holds four file grants, and a debug build keeps a copy of one in a frame for
every argument and return that carries it. Measured on `script/swish-check`'s aarch64 leg on
2026-10-24 (UTC): twelve faults on the boot's first pipeline, sixteen in `installed/jig rollback`,
seventeen passes both boots. Twenty-one is that and the four pages of headroom `CHILD_STACK_PAGES`
keeps. The planner (`grant_plan`) is compiled optimized even in a debug build, which took 14 KiB off
its largest frame. The shell is built from the progenitor's
own pages once a boot. The kernel's `pipeline_service::SHELL_EXTRA_STACK` must stay level with it:
a test wiring with less headroom finds faults the boot does not have.

`CARETAKER_STACK_PAGES` is four. The caretaker has no allocator and no recursion, and its one
large frame is a sixteen-entry handle table and, in `fs_nameset_caretaker`, the two
`grant::MAX_NAME` name buffers a rename compares. Its name set is a static, not a local.

`NET_STACK_STACK_PAGES` and `RESOLVER_STACK_PAGES` are nine: the kernel harness's eight extra pages
and the one every process gets, since `build_child` counts the whole stack. The resolver's debug
build faulted below three on its first resolve (2026-10-06 UTC).

## Job regions

`JOB_REGION_PAGES` is forty-eight. It holds one spawned program's image, its stack, a thread control
block, an address-space root, the intermediate tables for the windows a child touches, and the
mapping records of §13 (capability revocation and untyped reclamation). Under the address-space map
a child's image and stack sit in the second gigabyte and its pair pages in the first. So a child
with any window mapped walks two gigabyte-level tables. On `x86_64` every child also pays three
tables for its timebase page, and `top`'s debug image is nineteen pages. At forty-one the `x86_64`
leg of `script/swish-check` refused to build it, and at forty-eight that leg passes every line.

`DIR_JOB_REGION_PAGES` is ninety-eight: the program, the caretaker that carries its grant, and the
two endpoints between them. The caretaker is a second address space with its own tables and stack,
and each of the two spaces pays one more gigabyte-level table. A region too small fails inside
`build_child`, which the prompt reports as "could not spawn" with no way to tell it from an empty
pool.

`JOBS_BUDGET_PAGES` is six job regions and one `std` region (`grant_plan::STD_REGION_PAGES`). It is
small on purpose. `script/swish-check` runs more jobs through it than it could hold without the
regions coming back, so the gate proves that a bounded budget is enough. Widening it silently
retires that gate. The `std` region is there so a `std` job does not need every native job before
it to be reclaimed first.

`GRAPHICAL_TERMINAL_SESSION_PAGES` is 544, out of the job pool once for the life of a session. While
one runs, the 128 pages left hold two native jobs, and a `std` job or a second session is refused.
The count from the constants (four program-shaped children, the discipline and its blob, two map
budgets, nine endpoints and pages) came to 457 and was wrong. Bisected on 2026-10-03 (UTC) under
`script/swish-check`'s graphical launch with a keyboard, aarch64 needed 490 to 493 pages and riscv64
494 to 495. 528 was the riscv64 figure plus about seven percent. Sixteen more pay for
`display_terminal`'s bold and italic faces from `glyph_atlas`, which raised its riscv64 image from
193 to 209 pages. At 528 CI's riscv64 keyboard launch could not spawn (run 39378568861). `x86_64`
has no graphical leg, so its need is unmeasured and ungated, and that leg's arrival must bisect
again.

## The progenitor's own budgets

`PROGENITOR_RESERVED_PAGES` is 128. It pays for the page tables reaching the loader's scratch
window, which are the progenitor's own mappings and must never come out of a child's region. Tearing
that region down would free tables under a window the progenitor never unmaps. The loader's cursor
wraps inside `supervision_protocol::SCRATCH_WINDOW`, so those tables cost at most
`SCRATCH_TABLE_PAGES` however long the machine runs, and an assertion holds that half spoken for.
The other half builds `job_undertaker` (its image, twelve stack pages, tables, address space and
thread) and the tables under `ACTIVATION_FS_VA` and `IMAGE_STAGING_VA`.

`IMAGE_POOL_PAGES` holds the largest image twice, with a `std` program's heap beside it. One copy is
the child's pages, and the other is the progenitor's own, which §219 (how the shell names an
installed program to the spawner) has it hash. It is apart from the job pool so that the job
pool's gate keeps its meaning. Named programs never carve from it, and an image never carves from
the job pool.

The image is held three times while a child is built: the shell's frames, the progenitor's copy,
and the child's pages. Hashing each byte as the loader copies it into the child would drop the
copy and halve the pool. That needs building the child from the caller's still-mapped frames and a
loader that hashes as it fills, which is more work than the copy.

Everything the job pool and the image pool can have built at once must fit in half of the scratch
window. Every page the progenitor builds for a job stays mapped there until the job's region is
destroyed. The other half is left for the boot servers' pages, which are never given back. Those
were not measured; the whole archive is under 2,400 pages of file, so half is generous. A pool grown
past it fails the build rather than a spawn.

## The login stack

`LOGIN_CONSTRUCTION_PAGES` is 768 plus one durable session's budget. The 768 covers `login`'s
one-time costs (128 and 32 pages) and a handful of sessions at 64 and 64 pages each that never log
out. `login` splits the durable budget at start-up whenever it is handed the timetable programs,
before it serves anyone, so it is derived from `login_contract::durable::BUDGET_PAGES`.
`CRED_BUDGET_PAGES` and `CRED_STACK_PAGES` match `kernel::user::credential_service`, where they were
sized from `credentialer`'s Argon2id cost.
