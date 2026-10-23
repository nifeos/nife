# The `grant_plan` crate: how a line becomes a grant

An appendix to [`notes/grant-expression.md`](../grant-expression.md). That note carries the idea;
this one carries the design of `crates/grant_plan`, the crate that holds the logic, where its doc
comments link here. Each section names the module it covers.

## The grammar the parser accepts (`lib.rs`)

```text
<prog> [-abc] [--mem N] [token ...]
caps [command]
time <command>
help
echo <text>
```

`caps` and `time` are prefix words: their operand is a whole command line, operators included,
kept unsplit for the shell to act on. A first word that is not a builtin is a program invocation.
Everything after it is the `--mem N` budget grant plus positional tokens the parser leaves
unclassified. Which token is the argument and which the file is the manifest's job.

Short options are checked against the program's manifest at the prompt, because one of them can
widen a grant: `rm -r logs` hands over the subtree that `rm logs` does not. A bare program name
spawns it, and a bare token in a file position designates the file. Whether it is read or written
lives in the manifest.

The crate performs no IO and makes no syscalls, so all of this is tested on the host (DECISIONS §7
(testing: QEMU harness + host-testable crates)). A program that needs something the command did not
grant fails legibly ("you hold no such capability"), not with a Unix-flavored `EPERM`. `spawnproto`
is the word layout of the shell-to-progenitor spawn protocol. It is a userspace protocol (DECISIONS
§21 (the terminal is a userspace component)): the kernel routes the words and never reads them.

## The operators (`line.rs`)

[`notes/pipes.md`](../pipes.md) has the grammar and the three rules that are not Unix's. Three
details of the splitter are recorded here.

- The `2` of `2>` is the familiar spelling, not a file descriptor. It counts only at a word
  boundary, so `wc2>f` is the word `wc2` and an output redirection, and `least_authority_demo 2 >
  out` keeps its argument.
- A redirection's name may be quoted, and the quotes are off by the time anything downstream sees
  it, because what is designated is the name and not the spelling.
- `2>` binds only to a stream the program's manifest declares (`OutputDeclaration`).

### BUGS

- A line has one diagnostic stream, minted once and given to every stage that declares one. So a
  pipeline carrying `2>` where only some stages declare one is refused.
- `<<` is refused as `Refusal::NoRedirectTarget`, a message about a missing name where the mistake
  was a missing feature.
- Quoting makes an operator an ordinary byte, and nothing puts one back. A quoted redirection name
  is exempt from `Refusal::PatternInRedirect`: `> "*.txt"` writes a file called `*.txt`.

## The `std` region (`manifest.rs`)

`STD_REGION_PAGES` is the region the progenitor builds a `std` program in and then hands it as its
heap. The shell prints it in `caps` and the progenitor spends it, so it lives in this crate, where
both read it.

It is one region rather than a heap beside a job, and the reason is the reap. A region that has been
`SPLIT` cannot be `DESTROY`ed until its children are. A heap split off the job's region would stop
`job_undertaker` reclaiming the job, and a heap split off the pool beside it would be a second region
nothing reclaims at all. So the child's address space, image, stack and TCB come out of the region
first. The same region, narrowed to `WRITE`, is the budget its allocator maps from, so its heap is
what the build left and one reclaim returns all of it.

The size is the 256 pages of heap the kernel test harness gives every `std` program (`BUDGET_PAGES`
in `system_tests/src/user/std_service.rs`, the budget `std_exerciser`'s transcript is proven under).
Another 128 pages cover the build: about 40 of image, 32 of stack, the page tables for the windows a
`std` child touches, and a caretaker if a directory grant rides along. It is not a measurement of any
program's high-water mark, and it is nowhere near what `rg` over a real tree needs. A budget a person
sizes at the prompt would take `--mem`'s shape, which no `std` manifest declares: §170 (how a foreign
program is told what to do) gave a `std` program its words and not a budget.

A program that declares `installer` gets one package's worth more (`named_std_region_pages`). A
package manager reads a whole package before it sends it, because it checks the digest first. What
it sends are frames of its own: it maps one run of pages, reads the package into it, and hands the
progenitor a page of that run at a time (`abi::page_frame::SLICE`). So the run is both its buffer and
its frames. It lives in the program's own region, built from the progenitor's image pool rather than
the job pool, because what it holds is a package's bytes, exactly what that pool is sized for. The
progenitor's copy of the same package is carved beside it. The shell's own staging pages are left for
a file run by its path.
