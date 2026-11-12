# P2: later

An appendix to [the milestone](../1102-network-time-protocol-outside-review.md).

## P2: for later

16. `Timestamp::fraction` is `pub(crate)` while `seconds` is `pub`.
    Finding: a user who wants the fraction field writes `bits() as u32`; the documentation of
    `seconds` points at an accessor that exists but stays hidden.
    Evidence: `src/lib.rs` lines 157 to 166.
    Change: make `fraction` public. Door: two-way. Effort: XS.

17. Nothing uses the two low-entropy items of the public surface.
    Finding: `Query::new` (nonce equals the send time) and the low-bit randomizer (a dozen
    random bits) exist for a caller with no random source. No caller in the tree uses either; nife's
    client and the test server use `with_nonce`. Keeping them publishes a weaker path next to a free
    stronger one, and the documentation has to spend two paragraphs steering readers away from it.
    Evidence: `src/lib.rs` lines 218 to 236 and 613 to 622; `grep -rn "Query::new"` and a search for the low-bit randomizer
    finds no NTP caller.
    Change: Decision 3 asks whether both leave before the first publish. If they stay, `new` gains a
    `# Security` paragraph and the README's BUGS names it.
    Door: a public surface, two-way today. Effort: XS.

18. `Short` exposes its field while `Timestamp` and `Interval` hide theirs.
    Finding: `Short(pub u32)`; the other two fixed-point types go through `from_bits`/`bits` and
    `from_ticks`/`ticks`. The type's name is also a generic word (Decision 2).
    Evidence: `src/lib.rs` lines 280 to 296.
    Change: private field, `from_bits` and `bits`, with the three call sites in the test server
    updated. Door: two-way today. Effort: XS.

19. `dest` is an abbreviation in a public parameter.
    Finding: `Query::accept(&self, wire: &[u8], dest: Timestamp)`; the naming rules apply to
    parameters, and the RFC's word is "destination timestamp".
    Evidence: `src/lib.rs` line 691.
    Change: `destination`. Door: two-way (parameter names are not part of the ABI). Effort: XS.

20. The notes that describe the crate have drifted.
    Finding: `notes/ntp.md` says "21 host tests and 7 Kani harnesses". There are 25 and 7. The same
    note and `what-is-proved.md` describe the solver timings on one machine. This review's run of
    the bounded harness took 25 s rather than 9 s.
    Evidence: `notes/ntp.md` under "Why the protocol is a crate". `notes/ntp/what-is-proved.md`.
    Change: in nife, state the present counts without a date, as the documents rule requires. In the
    new repository, let the README state what `cargo test` prints rather than a count.
    Door: two-way. Effort: XS.

21. The `Reject` documentation overstates the check order.
    Finding: the `Reject` example says "the nonce check runs before any of the packet is believed";
    `accept` reads and judges version and mode first. The `accept` documentation has the order
    right. Nothing leaks that matters, but a stranger comparing the two will stop.
    Evidence: `src/lib.rs` lines 478 to 479 and 694 to 703.
    Change: "before anything in the packet beyond the two framing fields is believed".
    Door: two-way. Effort: XS.

22. `abs_ticks` does not take std's spelling.
    Finding: it returns `self.0.unsigned_abs()`, and the rule prefers std's spelling where std has
    one. No caller uses it.
    Evidence: `src/lib.rs` lines 342 to 345.
    Change: Decision 2 (drop it, or `unsigned_abs_ticks`). Door: public name, two-way today.
    Effort: XS.
