# `static_elf` against the `elf` and `object` crates

*Name: provisional, minted by lane 1088-review-package. The file compares the crate with two
crates.io readers, beside the package for `static_elf` that the same lane writes next.*

The architect asked whether `crates/static_elf` should give way to a dependency before it leaves
this repository. This page measures two candidates against it: `elf` 0.8.0 (cole14/rust-elf, safe
Rust, `no_std`, no allocator needed) and `object` 0.41.0 (gimli-rs/object). It applies the test of
[§46 (we take what meets our bar, and write with a reason)](../../design/decisions/0046-dependency-rule.md).
It follows the comparison of
[milestone 784 (reuse what the tree wrote)](../../design/roadmap/0784-reuse-what-the-tree-wrote.md),
whose appendix `elf-against-three-readers.md` ruled to keep ours, and it repeats the measurements
against the current crate and the current releases. Every result below comes from code run for this
page in a throwaway crate outside the tree. No dependency entered the tree, and no file of the crate
changed.

## Recommendation

Keep `static_elf`. Neither candidate meets the bar of §46 today, and both would leave the larger
half of the crate's work, the loader policy, to a wrapper that this tree writes anyway.

- `object` fails condition 3 for the kernel and the loaders: it does not link on any of the three
  bare-metal targets without a global allocator, and none of `kernel`, `uefi_loader`,
  `system_initializer` or `measured_boot` defines one.
- `elf` passes conditions 1 and 3, but it is thin on 2 and 4. It ships no proofs, it has had no
  release since May 2025, and its note reader still misreads or swallows the cases the milestone
  784 probe found.
- A switch to either removes about 250 of the crate's 525 shipping lines, and the eleven dependents
  see no change only if a crate named `static_elf` stays as the wrapper.

What would change the recommendation:

- `elf` publishes a release with the unreleased fixes on its main branch and accepts a note-reader
  fix and our harnesses, and an architect judges the maintenance condition met.
- `object` offers a `read_core` build that links without an allocator.
- An architect decides that the loader front half should be third-party code on the strength of
  exposure alone, and accepts a wrapper for the policy.

Any of these is a ruling under rule 6 (taking a third-party dependency is an architect's call), with
`needs-architect`. This lane takes none of them.

## 1. What each reader checks by itself

Our crate refuses a file unless it is a little-endian ELF64 `ET_EXEC` for the build's machine.
It also refuses more than 64 program headers, a writable-and-executable segment, two segments on
one page, an entry outside an executable segment's file bytes, a segment outside the file, and
address overflow. It reads notes with a bounded walk that refuses padding past the segment and a duplicate.

The corpus below has 60 malformed images. They cover truncated headers, huge `e_phoff`, odd
`e_phentsize` and `e_phnum`, offset and size overflow, address wrap, overlapping and
write-and-execute segments, bad `p_align`, other file types, a foreign machine and hostile notes.
Each reader ran on each image, and a case counts as refused when the reader returns an error from
the header parse, from the segment data accessor or from the note iterator.

| reader | refused of 60 | at the header parse | at segment data | at the note walk |
|---|---|---|---|---|
| `static_elf` | 38 | 38 | 0 | 0 |
| `elf` 0.8.0 | 26 | 18 | 6 | 2 |
| `object` 0.41.0 | 33 | 19 | 5 | 9 |

`static_elf` reports the note errors at the note call, so its 38 count the header parse only, and
it refuses the note cases separately through `Elf::note`.

Fourteen images are accepted silently by both candidates and refused by ours. Thirteen of the
fourteen are loader policy, which a wrapper would write:

- a foreign machine, `ET_DYN` and `ET_REL`;
- a writable-and-executable segment and a segment with no permission;
- two segments on one page, and two with the identical range;
- `vaddr + memsz` wrapping, `memsz` of `u64::MAX`, `vaddr` of `u64::MAX`, and `memsz` below `filesz`;
- more than 64 program headers, and no program header at all.

The fourteenth is a program header table at an unaligned offset. Neither candidate checks the entry
against a segment either, so the no-header case passes both.

The reverse also holds. Both candidates refuse an `e_phentsize` of 57 and ours accepts it, because
ours reads the first 56 bytes of each entry. The ELF specification allows a larger entry size, so
this is a difference of policy and not a defect, and the wrapper would choose which to keep.

What each candidate does well:

- Both bound every table read by the slice length, and both return an error for `e_phoff` near
  `u64::MAX`.
- Both refuse ELFCLASS32, a big-endian file and `EI_VERSION` 0 at the header.
- `object` refuses `e_phnum` of 65535 with a message about section headers. It does not refuse 65.

Note handling, from the 784 probe repeated on the current releases:

| case | `static_elf` | `elf` | `object` |
|---|---|---|---|
| `p_align` 1 | finds the note | returns bytes shifted by three | finds the note |
| `p_align` 16 | refuses | returns `None`, no error | refuses |
| last note padded past the segment | refuses | accepts | accepts |
| descriptor past `p_filesz` | refuses | returns `None` | refuses |
| trailing garbage after a note | refuses | returns the wrong note's bytes | refuses |

The `elf` main branch contains a merge from September 2025 titled "note: allow notes with
alignment 0". It is not in 0.8.0, so the table does not show it. I did not build main.

Our crate also reads a note from a head buffer without the whole image (`NoteSegments::from_head`
and `NoteSearch`), which `components` and `program_asks` call. `elf`'s streaming reader needs its
`std` feature. `object`'s streaming path needs `std` for its cache. I did not build a no-`std`
streaming wrapper for either, so I cannot say how much of that API a switch would keep.

`object` requires the input slice to be aligned to the header type. The corpus used heap vectors,
which are aligned. I did not check whether the loaders always hold an ELF at an aligned address, so
a switch could add an alignment requirement that the call sites do not meet.

## 2. Panics and overflow

The corpus ran every image in all three readers under `catch_unwind`, in a crate built with
`overflow-checks = true`. Result: 62 cases (the 60 above, one valid image and one note image),
no panic and no overflow in any of the three.

I also read both crates' source for the program-header and note paths, the code those images reach.

- `elf` 0.8.0 declares `#![forbid(unsafe_code)]`. Outside `#[cfg(test)]` modules, `segment.rs`,
  `note.rs`, `file.rs`, `elf_bytes.rs`, `parse.rs` and `endian.rs` hold no `unwrap` and no `expect`.
  Its parse functions use `checked_*` arithmetic and return `IntegerOverflow`.
- `object` 0.41.0 holds its `unsafe` in `src/pod.rs` (nine uses that cast bytes to header structs) and
  two `transmute` calls in `read_cache.rs`, which needs `std`. The `pod.rs` uses carry a comment that
  names the alignment and size checks. The two `transmute` calls carry none that my grep found. The
  program-header and note paths in `read/elf/` hold no `unwrap`, `expect` or `unsafe`.
- Neither crate is panic-free by proof. A corpus of 62 images shows no panic, and it proves nothing
  about image 63. The Kani rows below carry the stronger claim.

A one-hour fuzz of each reader, with a coverage-guided fuzzer, is not part of this page. The 784 probe
ran one and found no panic in either.

## 3. Kani

Our nine harnesses cover four claims. A note walk never panics and stays inside its segment. A
program header names a whole header inside the table. The address check never overflows and yields
an in-bounds range. The page range is ordered and panic free. They pass, and each of the nine
fails under its falsification patch (`script/crate-standalone static_elf`, 2026-11-19, nine of
nine).

I ported the nearest public function of each candidate into a harness crate outside the tree, with
the 784 input sizes.

| property | `elf` 0.8.0 | `object` 0.41.0 |
|---|---|---|
| segment bytes lie inside the file | not run; 549 s in the 784 probe | fails on an empty segment; holds with `p_filesz > 0`, 0.95 s |
| one note never panics | holds, 18.7 s | holds, 1.5 s |
| a note walk stays inside its segment and ends | holds, 168 s | holds, 23.6 s |
| a hostile header never panics | did not finish in about 20 minutes on a loaded host; 692 s in the 784 probe | holds, 13.7 s |
| address overflow, page range | no such function | no such function |

`object` returns an empty slice for a zero-size segment without looking at the offset, so a
`PT_LOAD` with no bytes and a `p_offset` past the end passes. It reads nothing, so it is no memory
fault, and our policy refuses the case, as in the corpus above. Seven of the nine candidate
runs finished, so those paths are inside bounded model checking at these sizes. The two `elf` runs
that did not finish, I stopped. Neither crate ships a
harness, so a switch means contributing ours (the step §46 expects) or carrying them in a wrapper
crate. The address and page-range claims have no counterpart in either crate and stay with our
wrapper.

## 4. Dependency cost

| | `static_elf` | `elf` 0.8.0 | `object` 0.41.0 |
|---|---|---|---|
| transitive crates | none | none | `memchr` 2.8.3 |
| license | MIT OR Apache-2.0 | MIT/Apache-2.0 | Apache-2.0 OR MIT |
| `rust-version` | 1.88 | 1.81 | 1.85 |
| three bare-metal targets | builds | builds | links only with a global allocator |
| `unsafe` | none | forbidden | `pod.rs`, `read_cache.rs` |

Size of a probe loader that links the candidate plus the shared policy, built `panic = "abort"` for
each target (text bytes, then rodata bytes):

| target | `static_elf` | `elf` | `object`, with a refusing allocator |
|---|---|---|---|
| aarch64 | 9024, 1110 | 11984, 940 | 8360, 1000 |
| riscv64 | 7638, 1221 | 10350, 956 | 7178, 1013 |
| x86_64 | 11407, 937 | 12719, 772 | 10655, 884 |

`elf` is larger than ours by 1.3 to 3.0 KB of text. `object` is smaller by 0.5 to 0.8 KB, but only
because the probe gave it a global allocator that refuses every request. Without one it fails to
link with "no global memory allocator found but one is required", because `object`'s `read_core`
declares `extern crate alloc`. It uses the allocator for section names in `read/elf/file.rs`, a path
the loader does not call. I did not test whether a build that strips that path links without the
allocator, and I did not measure the kernel or the loaders themselves. The probe is a stand-in, so
read the sizes as a comparison between three parsers and not as the effect on the kernel image.

## 5. Maintenance, and what a switch costs

Releases and activity, read from crates.io and GitHub on 2026-11-19:

| | `elf` | `object` |
|---|---|---|
| latest release | 0.8.0, 2025-05-14 | 0.41.0, 2026-11-10 |
| releases in the last 12 months | none | six or more (0.37.3 to 0.41.0) |
| last push to the default branch | 2025-12-15 | 2026-11-13 |
| open pull requests | 8, the oldest from 2025-03 | 2 |
| open issues | 13 | 32 |
| recent downloads | 4.0 million | 152 million |

Issues and pull requests recently opened on `elf`: an issue opened in 2026-06 closed the same day;
two pull requests opened in 2025-09 merged in 2025-11; three pull requests opened since 2026-08 are
still open. On `object`, maintainer pull requests merge the day they open, and the outside ones in
the last month merged within two days or closed. I counted from the lists and read no thread, so I
cannot say how upstream treats a larger contribution.

AI-assisted submissions: neither repository carries a `CONTRIBUTING` file, a policy file or a note
about generated code at its root, and a search of each repository for the words "LLM" and "AI
generated" found none. That is an absence, not a statement of acceptance. I did not ask either
maintainer. The rule on dependencies we fix wants an upstream that accepts our submissions, so an
architect would first need a question put to each maintainer.

Lines. The crate holds 525 lines of shipping code (blank lines and comments excluded, tests and
harnesses excluded): `parse.rs` 187, `note.rs` 220, `segment.rs` 56, `error.rs` 46, `machine.rs` 13,
`lib.rs` 3. A switch retires the field readers and the note walk. The 784 probe wrapper holds
the loader policy plus glue to one candidate. It comes to 165 lines for `elf` and 150 for `object`,
and it leaves out the 17-variant error type, `Segment`, `PageBytes`, the machine constants and the
note search, about 115 lines in the crate today. A wrapper would run to about 270 lines. A switch
removes about 250 of 525, a little under half, which agrees with the 280 of 490 that milestone 784
measured. The loader policy stays ours either way.

Dependents. Eleven crates depend on `static_elf` by `Cargo.toml`: `jig`, `uefi_loader`,
`components`, `fuzz`, `system_tests`, `measured_boot`, `system_initializer`, `program_asks`,
`process_builder`, `fixtures` and `kernel`. A grep of the other crates finds these counts. The name `Elf` appears on 255
lines in 56 files, and `Elf::parse` on 110 lines in 42 files. `NATIVE_MACHINE` appears on 36 lines
in 21 files, `Segment` on 29 lines in 8, `PageBytes` on 34 lines in 10, and the note types on about 90 lines in
8. These counts are upper bounds, because shared words such as `Elf` and `READABLE` match other
code. If a crate named `static_elf` stays as the wrapper with today's public names, the dependents
change nothing and their lines do not move. If they take `elf` or `object` directly, each repeats
the policy or calls the wrapper, so every one of the 56 files changes.

## The bar of §46, applied

The test runs in order. Rule 1 does not apply: an ELF file is outside nife's ABI and its
capabilities, and other systems read it. Rule 2 asks whether a candidate meets the bar.

| condition | `elf` | `object` |
|---|---|---|
| 1. safety argument at every `unsafe` use | met, none permitted | `pod.rs` met; `read_cache.rs` not found |
| 2. proofs matching ours | none upstream; ours hold on the nearest function | none upstream; one counterexample on an empty segment |
| 3. all three architectures | builds on all three | builds, but links only with an allocator the kernel lacks |
| 4. maintained | no release in 18 months, 8 open pull requests | active |

Neither meets all four. Rule 3 then says to contribute upstream until one does. The distance for
`elf` is a release, a note-reader fix and the harnesses. The distance for `object` is an
allocator-free `read_core`, which is a change to the crate's layering, and one whose acceptance I
cannot predict. Rule 4 permits writing with a stated reason. The reasons that hold today are
that bringing `object` to the bar needs a restructuring upstream may not take, and that the loader
policy remains ours under every option. They do not include cost, since `elf` is closer to the bar
than writing is to done.

Decision 0146 says this tree writes parsers like `gpt`, `elf`, `dtb` and `nifefs` and does not take
them. That sentence was written for tar, and its reasoning was that a complete specification makes a
proof settle correctness. It predates the amendment of 2026-11-14, which allows taking a crate that
meets the bar in any layer, so the sentence is context and not a bar. Milestone 784 ruled to keep our
ELF reader on the measurements above, and these measurements give no reason to reverse it.

## What I could not verify

- The `elf` main branch. I measured the 0.8.0 release only, so I cannot say whether the unreleased
  note-reader fixes close the gaps in the note table.
- Whether either upstream accepts AI-assisted submissions, and how either treats a large change.
  The repositories state no policy, and I did not contact a maintainer.
- Binary size inside the kernel and the loaders. The probe measures the three parsers in a stand-in
  loader, and a wrapper's own size sits on top.
- Whether every loader holds the ELF at an aligned address, which `object` needs.
- Whether a no-`std` streaming note reader exists for either candidate.
- Whether a build of `object` that removes the section-name path links without an allocator.
- The two `elf` harnesses marked above, which I stopped under load. The 784 probe finished both.
- Fuzzing of the current releases, and any Miri run. I ran the corpus and the Kani harnesses.
- Mutation testing and the stranger test of the crate itself. They belong to the package.
- The 62-image corpus is small. A pass shows no panic on those images and no more.
