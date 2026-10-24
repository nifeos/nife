# What Kani could not do in `user/`, measured

*Name: provisional.*

An appendix to [`notes/user-proofs.md`](../user-proofs.md). It holds the three properties that were
written, run and abandoned before the one that shipped, what each shape cost, and why.

A proof that does not terminate is not a weaker proof, it is no proof. Three properties were
written, run and abandoned before the one that shipped. They are recorded because the shapes
generalize. A future lane that tries the obvious thing deserves the measurement rather than the
surprise.

| the property | the shape | what happened |
|---|---|---|
| the staged document fits the buffer | a bound on a **sum of 32 symbolic values** | no answer in 20 minutes, on CaDiCaL **or** Z3 |
| `Editor::insert_char` preserves the document invariant | a **symbolic index into a 3.5 KB struct** | CBMC ran out of memory in 3m23s |
| `wc::render` writes the digits of `v` | any claim about **values off a chain of 20 divisions** | no answer in 10 minutes |

The first is the instructive one. "The sum of at most 32 values, each at most 100, is at most 3232"
is arithmetic a child can check. It is also a cardinality argument, the family resolution-based SAT
is famously worst at. The solver has to rediscover counting from bit-level adders. Kani is a bounded
model checker over a SAT backend. This is not a gap in the tool so much as the wrong question to
bring it.

The second says the same thing about state size rather than arithmetic. A symbolic index into an
array of 32 hundred-byte rows is a case split the formula cannot afford. So the editor's editing
operations are out of reach as written. Restructuring the document so its lengths live apart from
its bytes would fix that. It would be a data-layout change made for the prover, which is a design
question and not a lane's to take.

The third is the trap most likely to catch the next person. `render` verifies instantly if the
only assertion is `n <= out.len()`, because `--slice-formula` throws the division chain away as
irrelevant. Add one assertion about a byte's *value*, and the whole chain becomes relevant. Then
the harness stops finishing. A fast harness is not evidence that the code is cheap to reason about.
It can be evidence that the assertion was not asking anything.

What Kani is good at here, and what shipped, is the opposite shape. It is bounded byte movement
with comparisons: no sums, no division, no symbolic indices into large objects.

## Two traps in printenv's harnesses

Both harnesses in `components/src/printenv.rs` are shaped around something Kani does badly.

A loop over a range whose endpoint is symbolic has no unwinding bound CBMC can find, and the harness
never terminates. So `push_never_writes_past_the_buffer_it_was_given` does not walk `0..start` and
`n..CAP`. Each walk spans the whole buffer and selects the interesting range inside. That costs `CAP`
iterations of a byte comparison and finishes in a tenth of a second. It was found the slow way.

A `cover` alone cannot go red. Kani 0.67.0 reports an unsatisfiable `cover` as "0 of 1 cover
properties satisfied" and still prints `VERIFICATION:- SUCCESSFUL`, with exit code zero. A `push`
that reserved its last byte (`*n + 1 < buf.len()`) kept `the_buffer_can_be_filled_exactly` green on
its `cover` alone (milestone 323 (the falsification record is incomplete in five ways, and each was
found by a different lane), 2026-10-05). The harness therefore also asserts the boundary: `push`
takes exactly as many bytes as fit, `min(CAP, start + bytes.len())`.

The unwind bound of four bytes is stated rather than raised. Raising it costs solver time and buys
nothing the argument in [notes/user-proofs.md](../user-proofs.md), "What is proved today", does not
already give.
