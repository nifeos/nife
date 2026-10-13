# Spelled-out rulings

*An appendix to [`design/naming.md`](../naming.md), which is the rule. It holds rulings that spell
out a contraction nothing outside the tree owns, after the `receive` ruling in
[vocabulary-rulings.md](vocabulary-rulings.md), which sat in a document with no room left under the
prose budget. The file and its stem are provisional; naming is an architect's.*

## The tree spells `operation`, never `op`

calef, 2026-10-04 UTC, after the `receive` ruling: *"Seems like OP should be OPERATION and RECV
should be RECEIVE."*

> `operation` is spelled out in every identifier this tree owns (`OPERATION_READLINE`,
> `request_operation`, `operation(w0)`). `op` is not a ratified abbreviation for it. An identifier
> that mirrors an external API, or where `op` means something else, keeps its spelling.

The rule is the one the `receive` ruling applied: spell out a contraction nothing outside the tree
owns. The `OP_` prefix on request-code constants became `OPERATION_`. The decoder `op(w0)` became
`operation(w0)`. The names `req_op`, `fs_op`, `op_code`, `op_in`, `OPS`, `PROOF_OPS` and `ops`
followed. No constant value, opcode or wire layout changed.

Refused: keeping `OP_` because the prefix is short and every constant carries it, since length is
the cost the tree already pays for `RECEIVE_CAP`. Also refused: `opcode` or `OPCODE_`, which names
the number rather than the thing requested and would be a second ruling.

Kept, because someone else owns the spelling or the word means something else:

- `core::ops` and `std::ops` (`Deref`, `Range`), and every `no-op` or `no_op`, an English idiom.
- `unsafe_op_in_unsafe_fn`, a rustc lint.
- `op` as an instruction mnemonic in `script/stack-depth-check` and `script/fastpath-footprint`,
  and as the TFTP opcode field in `script/board-netboot`.
- DHCP's `op` field (`op = BOOTREQUEST` in `crates/virtio`).
- `is_op` and `is_diag_op` in `crates/grant_plan/src/line.rs`, and the `op` loop in a swish help
  test. Both mean a shell *operator* (`|`, `>`, `2>`).
- `vendor/redoxfs`, a permission-bit `op` that is not ours to edit.

`OPERATION_SENDTO` keeps the POSIX `SENDTO` spelling in its suffix, pending a separate ruling on
whether the tree spells that `SEND_TO`. This ruling does not touch it.

Closed records (`BUILT` roadmap blocks, `design/decisions/`, dated audit reports) keep the name they
used, so a grep for `OP_` finds them.

Live notes and roadmap documents spell the names out.

## The tree spells `capability`, never `cap`

calef, 2026-10-06 UTC: *"I don't think we should abbreviate capability as cap."* When the
maintainer proposed how to apply it, he answered: *"Record the ruling. We've almost got a quiet
tree so we can run that shortly."*

> New code spells `capability` out in every identifier. `cap` is not a ratified abbreviation for
> it, as a whole word or as a component (`cap`, `Cap`, `CAP`, `caps`, `cap_slot`, `SEND_CAP`,
> `irq_cap`). The existing names are renamed in one sweep when the tree is quiet.

The rule is the `operation` ruling's. Its exceptions are carried over as proposals, not yet ruled:
an identifier that mirrors a name a specification coined keeps it, and so does `cap` where it is
the English word for a ceiling. Comments and prose are outside the rule, since "a cap on lane
count" abbreviates nothing.

`script/lint` holds the line until the sweep, through `helpers/cap_abbreviation.py`: a ratchet
over Rust identifiers that fails when the count rises, 2,890 on the ruling's day. The census, the
kept names and a proposed spelling for each public name are in
[capability-worklist.md](capability-worklist.md). The proposals are not ruled. The ABI names
(`SEND_CAP`, `RECEIVE_CAP`, `CAP_INSERT`, `SYS_CAP_DELETE`) and the `Cap` type go last, as one batch.
