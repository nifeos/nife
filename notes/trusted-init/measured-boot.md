# Measured boot: the crate's digest, its text and the loader's verdict

An appendix to [notes/trusted-init.md](../trusted-init.md), which carries the hash and why it is
SHA-256, written here. This file carries the rest of the design behind `crates/measured_boot`,
whose doc comments link here.

## One implementation, both sides

The build (xtask, on the host) and the kernel (`no_std`, at boot) hash through this one crate. There
is exactly one definition of the measurement, so a host and target disagreement cannot quietly
become a boot failure. The risk it trades for is a buggy implementation agreeing with itself. That
is why the tests are the published FIPS 180-4 and NIST CAVS vectors, not self-consistency checks.
BLAKE3 and Ed25519-signed images are follow-ups recorded in DECISIONS §26 (the fault endpoint)'s
milestone-22 phase B block, and each is more code inside the boundary.

The hasher streams rather than hashing one slice. The kernel hashes a mapped region it must not
copy, since there is no heap to copy a 300 KiB ELF into. A test can then hash the real bytes with
one byte flipped without allocating a mutated copy.

The digest comparison is a plain `==`. The expected value is public: it is printed in the build
output and sits in the kernel's `.rodata`. There is no secret for a timing side channel to leak.

## A digest's type does not carry its algorithm

DECISIONS §197 (a package is one archive file) holds the digest ruling (calef, 2026-10-07 UTC).
The algorithm lives in the text form, `sha256:<hex>`, because text is what leaves a program: a
catalog line, a recipe, an activation row. In memory a `Digest` is only ever a whole-file SHA-256.

A Merkle root, when a loader that pages binaries in on demand wants one, is a second fact beside
this one. It is checked by a different procedure, so it gets its own label (`merkle-sha256-8k:` is
the example the ruling gave) and its own type. An enum with one variant now would make every
comparison in the trust root, the activation table and the installer match on a variant that cannot
be anything else. On the day a second variant arrived, it would let a Merkle root reach a check
that only knows how to hash a whole file. Two types make that state unrepresentable; one enum makes
it a runtime match somebody has to remember.

`sha256` is the container-image convention's name for the algorithm. OCI and Docker digests are
spelled this way, so a stranger reading a catalog line already knows what `sha256:` means and can
check the hex with `sha256sum`. Every line written before a second label arrives still reads.

The parser refuses bare hex rather than reading it as SHA-256, because a reader that guessed the
algorithm would be the format change the label exists to avoid. It refuses an unknown label by
name, so a reader meeting a newer digest says so instead of comparing it as the old kind. A Merkle
root compared as a whole-file hash would be a guaranteed mismatch reported as tampering. Bare hex
still has one job: the kernel's boot diagnostics, `sealed_pair` and the stick's payload note print
it for a person comparing against `shasum -a 256`.

## The manifest and the progenitor's table

The manifest is one entry per line: a name, one space, and the digest's text form. `cargo xtask`
writes it. `kernel/build.rs` reads it into `Measurement`s, and the progenitor reads it out of the
archive at boot. The progenitor cannot allocate, so the parser yields borrowed names and never
builds a collection. An image's package catalog is the same format with package stems for names.
Blank lines, `#` comments and trailing space are skipped, because they appear in files people edit
or diff. Any other line that does not parse is reported with its text, and the build makes that a
hard error. A caller that skips errors still fails closed: a name it cannot parse is a name it will
not find, and an unfound name is `Unmeasured`.

The kernel's trust root is compiled into the kernel image, which works because the kernel is not in
the archive it measures. The progenitor is in that archive, so its own table cannot be compiled into
it without building userspace twice. The build packs the table as an ordinary archive entry,
`program_measurements`, and the kernel's trust root names it
([chain-past-the-progenitor.md](chain-past-the-progenitor.md)). The kernel vouches for the table
exactly as it vouches for the progenitor, so the progenitor's refusals are as trustworthy as the
progenitor is. The progenitor looks a name up before hashing, so a name the table does not mention
is a refusal that costs nothing to reach.

## The loader's verdict

One rule produces both fields of `Verdict`: nothing runs that the table will not vouch for. So
`elf` is `None` whenever the entry is missing, refused or not an ELF. The two `None` answers that
matter are not the same answer, which is why the type is a struct rather than an `Option`. With
`unvouched: false` the archive did not have the program; a build left it out, and the loader
carries on. With `unvouched: true` the archive had it and the measurement refused it, which is a
security event the boot reports out loud.

A program the table vouches for that is not an ELF is not a refusal. The measurement did its job,
and not being a loadable program is a build defect, not a substitution.

`Default` is "the archive did not have it", derived on purpose (milestone 246 (measured boot's refusal path is tested by nothing, and one mutant turns it off)). It is the fail-safe
value: no image means nothing runs, so a default-constructed `Verdict` vouches for nothing. It also
makes `verdict` mutable. `cargo mutants`' only operator on a function returning a struct replaces
the body with `Default::default()`, which without the derive does not compile and is scored
unviable. The tool would then report nothing about the one decision in the crate that says whether
unmeasured code may run. With the derive, that mutant is the dangerous wrong answer, a refusal
reported as an absence, and the tests kill it on every run.
