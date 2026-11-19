# Decision B: which `talc` outside nife

An appendix to [the milestone](../1100-memory-region-heap-outside-review.md). It holds the case
behind Decision B, which the architect ruled on 2026-11-14 (UTC), option A.

Evidence the architect saw: `talc` 5.1.1 came out on 2026-09-09 and has 2.6 million downloads. One
maintainer wrote most of its commits. Pull request 57 has been open since 2026-09-24 with no
comments.

Background. nife builds `talc` from `vendor/talc`, the crates.io 5.1.1 package plus upstream pull
request 57, through `[patch.crates-io]` in each workspace root. A `[patch]` table does not travel
with a library crate: the crate's own repository, and any consumer whose root workspace lacks the
patch, builds the unpatched package. Pull request 57 fixes two fit checks that compared addresses
where they meant sizes. The first fixes undefined behavior that Miri reports through
`allocation_cache`'s host test, and both can hand out overlapping memory where addresses wrap, which
a 64-bit process never reaches in practice (unverified as a proof). The pull request has been open
for seven weeks with one commit and no maintainer comment. The step 9 appendix of milestone 1017
already rules that `redoxfs` takes `talc` as a git dependency pinned by `rev` with `version`,
divergence as a patch file in that repository, and that `talc` gets no repository of nife's own.
That ruling does not say what this crate does, and this crate is a library that other repositories
consume.

Recommendation first. Depend on crates.io `talc = "5.1.1"` in the crate's manifest, unpatched, and
state in the README what the package lacks and why it does not reach a 64-bit process. nife's own
root `[patch.crates-io]` keeps applying to nife's whole graph, this crate included, so every nife
image still builds the fixed source. The `redoxfs` repository carries the patch file that the ruling
gave it. When a `talc` release carries pull request 57, bump and delete the README sentence. This
keeps the crate publishable (crates.io publishes a git dependency only by its `version`, so a pin
would not reach a registry consumer anyway), costs no fork, and leaves the fixed source in every
build that ships.

Prior art. Rust std's wasm target depends on `dlmalloc` from crates.io and carries no patch; a fix
waits for a release. Debian and the BSD ports carry patches at the distribution, not in the library,
which is the shape nife's `[patch]` has today.

Concrete example. The crate repository's `check` runs its host tests (item 8) against the unpatched
5.1.1 and passes, because the wrapped-address case needs a gap that ends within a chunk of the top
of a 64-bit address space. basalt's image, built in nife's workspace, links the patched source. If
upstream stays silent past the next monthly `script/vendor-watch` report, the question returns as a
fork request, which this block does not make.

Not taken. A git pin on upstream at a revision carrying the fix (there is none; the pull request has
not merged). A fork under `nifeos` (refused in the step 9 appendix as needing its own approval). A
`[patch]` instruction in the README for consumers (an instruction a build cannot check).
