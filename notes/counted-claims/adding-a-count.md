# Counted claims: adding a new marked count, worked through

An appendix to [`notes/counted-claims.md`](../counted-claims.md), whose EXAMPLES keep the five
steps in short under the cap of §212 (a prose budget).

Take the tree's harness count as the worked example, from nothing to gated.

1. Decide the question, and write it as a sentence. Not "harnesses" but "how many Kani proof
harnesses the tree carries, which is what `script/verify` proves". If you cannot write the sentence
without an "or", you have two counts.

2. Write the derivation and check it against the real shapes. Not against what you expect the
shapes to be:

```sh
$ grep -rho '#\[kani::proof[a-z_]*' --include='*.rs' . | sort | uniq -c
 123 #[kani::proof
$ grep -rc '#\[kani::proof' --include='*.rs' crates | awk -F: '{s+=$2} END {print s}'
119
```

The gap between 123 and 119 is the whole reason for this step. Two of the four are in the vendored
RedoxFS, which this suite has never proved. Two are inside doc comments in
`helpers/kani-lint-shim/` that describe the attribute rather than use it. A derivation that counted
them would be confidently wrong, in the direction nobody checks.

3. Add the registry entry to the `==> counted claims` block in `script/lint`, with a docstring
saying what shape it assumes:

```python
REGISTRY = {
    'kani-harnesses': (
        'how many Kani proof harnesses the tree carries, which is what script/verify proves',
        lambda: sum(_harness_hits().values()),
    ),
}
```

4. Mark the sites. Put the marker after the number, on the same line:

```markdown
This project has **119 Kani harnesses** <!--count:kani-harnesses-->, and DECISIONS §14 says …
```

5. Run the gate. It prints one line on success and names both values on failure:

```
$ script/lint
==> counted claims
counted claims: 8 markers over 3 counts, all agreeing with the tree
```

```
$ script/lint          # after somebody lands a proof and does not touch the prose
==> counted claims
lint: a counted claim disagrees with the tree:
  notes/fuzzing.md:13: says 119, the tree says 120 (kani-harnesses: how many Kani proof harnesses
  the tree carries, which is what script/verify proves)

Fix the number, or fix the derivation in script/lint if the tree is right and the gate is asking
the wrong question. See notes/counted-claims.md.
```
