# What six real records found

*Name: provisional. Named after the main page's own heading.*

An appendix to [`notes/falsification.md`](../falsification.md), which states each finding.

The six were chosen against the cases that would embarrass the mechanism rather than a toy.
Milestone 191 (did the proofs catch the bugs?) named `capability::subset_is_reflexive` as a
tautology and the `paging` harnesses as per-ISA duplicates, so those are where a falsification
convention has to earn its keep.

### The capability crate's central theorem is blind to the predicate underneath it

`derive_never_widens_rights` is the crate's flagship: *"the central theorem, on the real
`CapabilityTable::derive`."* Applying the single most plausible defect in `Rights::is_subset_of`,
its two operands swapped (`other.0 & !self.0` for `self.0 & !other.0`), and running the whole crate:

```
Complete - 10 successfully verified harnesses, 2 failures, 12 total.
Verification failed for - verification::subset_matches_allows
Verification failed for - verification::from_bits_cannot_forge_a_right
```

Ten of twelve stayed green, including the central theorem. The reason is worth stating plainly
because it generalizes: `derive_never_widens_rights` states its property *through the predicate it
is testing*. It asserts `derived.rights.is_subset_of(src_rights)` while `derive` guards on
`rights.is_subset_of(src.rights)`, so a consistently wrong `is_subset_of` satisfies both sides and
the harness cannot see the break. It is load-bearing against the **guard** being dropped, which is
what its recorded falsification does, and blind to the comparison the guard calls.

That is a real coverage gap in the crate this whole project rests on, and it was invisible to
everything: the harness passes, `kani::cover!` would pass, the tests pass, mutation testing does not
reach `cfg(kani)` code. It took applying a defect and looking at which harnesses noticed.

**Not fixed here, deliberately.** Closing it means rewriting the harness to state the property in
terms that do not call the predicate (comparing raw bits, or asserting against `allows`), and this
lane was measuring the proofs rather than rewriting them. It wants a lane, and the proposal is in
milestone 194's report.

### `subset_is_reflexive` is falsifiable and still adds nothing

It can be turned red: drop the `!` from `is_subset_of` and `a.is_subset_of(a)` fails. So it is not
decorative in the strict sense, and §134's "no plausible implementation error breaks it" is slightly
too strong.

The sharper statement is the one the sweep can support: its falsification set is a strict subset of
another harness's. Every mutation of `is_subset_of` that reaches it also reaches
`subset_matches_allows`, and the swapped-operand case above reaches `subset_matches_allows` and not
it. A harness whose red cases are a subset of another's adds no discrimination to the suite. Nothing
here proposes deleting it; the record is what lets somebody make that argument with evidence instead
of taste.

### The same shape, one crate along

`filesystem_contract::a_grandchild_is_bounded_by_the_root` is `attenuate_never_widens` at depth two.
`attenuate` is a single `&`, so the only claim the second harness adds is that AND is associative,
and there is no plausible defect that breaks it while leaving the depth-one harness green. Both go
red on the same one-character patch (`&` becoming `|`). Recorded in the patch prose rather than
acted on.

### The patch path was wrong, and `paging` broke it on first contact

§134 (a harness carries a machine-replayable falsification record) was first spelled was
`crates/<crate>/falsifications/<harness_fn_name>.patch`, which assumes a harness function name is
unique inside its crate. In `paging` it is not. Six properties are stated once per ISA, in
`aarch64.rs`, `sv39.rs` and `x86_64.rs`, under six shared function names:

```
paging::{aarch64,sv39,x86_64}::verification::distinct_pages_take_distinct_paths
paging::{aarch64,sv39,x86_64}::verification::index_is_always_in_bounds
paging::{aarch64,sv39,x86_64}::verification::the_indices_and_offset_tile_the_address
paging::{aarch64,sv39,x86_64}::verification::the_leaf_keeps_address_and_permissions_apart
paging::{aarch64,sv39,x86_64}::verification::the_two_halves_are_disjoint
paging::{aarch64,sv39,x86_64}::verification::the_user_va_gate_admits_only_the_aligned_low_half
```

Eighteen of that crate's twenty-six harnesses, three to a filename. `cargo kani --harness
index_is_always_in_bounds` cannot separate them either, so the sweep could not have run one of them
if the file had existed.

calef amended §134 on 2026-08-31: the module path is always included, with no branch. Refused, and
it is the refusal worth keeping, was *unqualified when unique and module-qualified when not*, which
is a branch keyed on an **unstable** property, since a harness added elsewhere would retroactively
invalidate an existing path. That is the same shape as the two-tier program-naming rule calef
rejected on 2026-08-01, one domain over.

The amendment buys more than uniqueness, and this is why it was the right answer rather than the
adequate one. The qualified path is Kani's own fully qualified harness name with the separators
changed, so the sweep now filters with `--harness <qualified> --exact` instead of a substring match
on a bare function name. Under the old filter, `paging`'s three `index_is_always_in_bounds`
harnesses would all have run, and "one of them went red" would have proved nothing about the one
being falsified. The path and the filter are one fact written twice, which is the shape of a
convention that cannot drift.

`script/falsifications --check` keeps a collision check, but it now guards this script's own
module-path tracking rather than the tree. Two harnesses cannot share a qualified name in Rust, so
if two ever compute the same patch path, the brace counting that derives module paths is wrong and
the sweep would prove the wrong harness.
