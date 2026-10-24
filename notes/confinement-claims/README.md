# Confinement claims: the appendices to notes/confinement-claims.md

[`notes/confinement-claims.md`](../confinement-claims.md) is the page to read: the table of what a
confined component cannot do, which test proves each row, and what each pass found in summary. The
files here hold each pass in full.

- [`unstated-claims.md`](unstated-claims.md): the six limits the system does not claim.
- [`breaking-the-harnesses.md`](breaking-the-harnesses.md): what the falsifications of milestone 202 (every confinement test is a ritual until somebody
  breaks the confinement and watches it fail) found.
- [`breaking-the-kernel-tests.md`](breaking-the-kernel-tests.md): what those of milestone 305 (the six kernel confinement rows get a
  falsification a machine can replay) found.
- [`which-assertion-fires.md`](which-assertion-fires.md): the sweep of milestone 307 (which assertion actually
  fires when a confinement claim is broken) over every row.
- [`adversarial-pass-2026-09-21.md`](adversarial-pass-2026-09-21.md): risk 7's adversarial pass.
- [`limitations.md`](limitations.md): the BUGS list in full.
- [`the-rights-gate-tests.md`](the-rights-gate-tests.md): how the rendezvous rights-gate and retype tests are shaped.

Name: provisional, for the directory and every stem in it. The directory follows the
`notes/<stem>/` appendix convention §212 (a prose budget) set, so its name is the parent's stem. Each file is named
for its content. calef names directories and files, and `script/names --unratified` lists each stem.
