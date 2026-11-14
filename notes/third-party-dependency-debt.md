# Third-party crates short of the bar

*Name: provisional, minted 2026-11-14 (UTC) by maintainer/third-party-dependencies-meet-the-bar.*

§46 (thin primitives, whole subsystems and everything in between) applies its four-part bar to
every third-party crate a nife image links, the ones already shipped included. A shipped crate short
of the bar stays in use, appears here as debt, and gets upstream contributions until it meets the
bar. Each entry below names a crate and the parts it fails. An entry leaves when the crate meets
the part, and the list is empty when every shipped crate meets the whole bar.

## What was measured

Measured 2026-11-14 (UTC) at the tip of the pull request that wrote this note.

- Which crates ship: `notes/dependency-standards-census/tools/classify.py` replays each image build's
  cargo flags through `cargo tree -e normal`, as the census did. A crate ships when a build that
  produces part of an image links it. The census measures the two vendored trees that ship as third-party code,
  `vendor/redoxfs` and `vendor/talc`, with them. `vendor/relibc` is a seed nife owns
  and is outside this list.
- Safety argument (part 1): `safety_comment_coverage` and `unsafe_fn_contracts` from the census's
  `measure.py`, plus the same test for `unsafe trait`. An entry fails when any unsafe block lacks a
  comment containing "safety", or any `unsafe fn` or `unsafe trait` lacks a `# Safety` section.
  The scan is textual over the whole package, not resolved against the features and targets nife
  builds, so a crate whose unsafe code sits behind a cfg nife never enables can appear in error.
  A hand read of smoltcp 0.14.0 found that its only unsafe code is in the `std` raw-socket files nife does
  not compile, so it meets the part. The scan skips unsafe code that a crate's macros generate.
  `libc` declares its functions in `extern` blocks, which this scan does not read as unsafe
  functions, and fails on the others.
- Three architectures (part 3): `cargo check --locked --offline` of the kernel, `components`,
  `fixtures` and `redoxfs_server` for the aarch64, riscv64 and x86_64 targets finished clean, so
  every crate those graphs link builds on all three. The `jig`, `c_library` and `uefi_loader` graphs
  need the std overlay or the loader's artifacts and this pass did not rebuild them, so their crates carry
  no entry for this part on that basis alone.
- Proofs (part 2) and maintained upstream (part 4) are judgments, not scans. The census's
  `registry.tsv` and `upstream-practices.tsv` hold the older evidence, and the pull request that
  contributes to a crate shows both parts for it.

## Debt

Sorted by crate name, then version. Each entry names the part it fails. The safety argument is the only part a scan covers.

- aes 0.8.4: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- aes 0.9.3: safety argument, lacking a comment on every unsafe block
- allocator-api2 0.4.0: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- argon2 0.4.1: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- argon2 0.6.0: safety argument, lacking a Safety section on every unsafe fn
- base16ct 1.0.0: safety argument, lacking a comment on every unsafe block
- bitflags 1.3.2: safety argument, lacking a comment on every unsafe block
- blake2 0.10.6: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe trait
- blake2 0.11.0: safety argument, lacking a comment on every unsafe block
- block-buffer 0.12.1: safety argument, lacking a comment on every unsafe block
- byteorder 1.5.0: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- chacha20 0.10.2: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- cipher 0.4.4: safety argument, lacking a comment on every unsafe block
- cmov 0.5.4: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- cpufeatures 0.2.17: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- cpufeatures 0.3.1: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- curve25519-dalek 5.0.0: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- generic-array 0.14.7: safety argument, lacking a comment on every unsafe block, a Safety section on every unsafe fn and a Safety section on every unsafe trait
- getrandom 0.2.17: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- getrandom 0.4.3: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- hash32 0.3.1: safety argument, lacking a comment on every unsafe block
- heapless 0.9.3: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- httparse 1.10.1: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- hybrid-array 0.4.14: safety argument, lacking a comment on every unsafe block
- hybrid-array 0.4.15: safety argument, lacking a comment on every unsafe block
- inout 0.1.4: safety argument, lacking a comment on every unsafe block
- inout 0.2.5: safety argument, lacking a comment on every unsafe block
- jiff 0.2.43: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- jiff-core 0.1.5: safety argument, lacking a comment on every unsafe block
- libc 0.2.191: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- libm 0.2.16: safety argument, lacking a comment on every unsafe block
- lock_api 0.4.14: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe trait
- lz4_flex 0.11.6: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- once_cell 1.21.4: safety argument, lacking a comment on every unsafe block
- poly1305 0.9.1: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- polyval 0.7.3: safety argument, lacking a Safety section on every unsafe fn
- r-efi 7.1.0: safety argument, lacking a comment on every unsafe block
- redox_syscall 0.9.5: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- redoxfs (vendored): safety argument, lacking a comment on every unsafe block, a Safety section on every unsafe fn and a Safety section on every unsafe trait
- scopeguard 1.2.0: safety argument, lacking a comment on every unsafe block
- seahash 4.1.0: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- sha2 0.11.1: safety argument, lacking a comment on every unsafe block
- stable_deref_trait 1.2.1: safety argument, lacking a Safety section on every unsafe trait
- subtle 2.6.1: safety argument, lacking a comment on every unsafe block
- talc (vendored): safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- tock-registers 0.12.0: safety argument, lacking a comment on every unsafe block
- uuid 1.29.0: safety argument, lacking a comment on every unsafe block and a Safety section on every unsafe fn
- zerocopy 0.8.63: safety argument, lacking a comment on every unsafe block, a Safety section on every unsafe fn and a Safety section on every unsafe trait
- zeroize 1.9.0: safety argument, lacking a comment on every unsafe block
- zeroize 1.9.1: safety argument, lacking a comment on every unsafe block
