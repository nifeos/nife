# The terminal's sink adapter needed a new opcode, which was not known

*Name: provisional. After the opcode it is about.*

An appendix to [`notes/sink-protocol.md`](../sink-protocol.md), which states the result.

The plan said the adapter would be `file_sink`'s shape and that the work was rewiring the progenitor. Building
it found something else: `OPERATION_WRITE` reads from the client's output page, and there is exactly one
of those. The progenitor maps a single frame into `line_editor` read-only and into the shell read/write. A
second page-based client needs a second frame and a page index in every request, which is `filesystem_contract`'s
one-page-two-clients problem (DECISIONS §55, the reason the file behind a `>` is the shell itself)
arriving in a second contract.

So the terminal contract grew `OPERATION_PRINT`: up to eight bytes carried in the request's own words,
replied when they are on the wire. The adapter then needs no page at all, and `expand_output` is
shared with `OPERATION_WRITE`, so a newline from an adapter gets the same manners as a newline from the
shell. Two writers, one terminal, one set of manners, no second frame.

Eight rather than sixteen is the contract's request shape and not a choice: a served request arrives
through `receive_cap`, which hands the server a reply capability and two data words. `OPERATION_BYTES`
carries eight for the same reason, from the input direction, which is why the shape was already
there to copy.
