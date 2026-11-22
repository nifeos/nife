# The verb table: adding a verb reaches the proxies, or it fails the build

An appendix to [notes/fs-server.md](../fs-server.md). It holds milestone 61 (the caretakers: one
verb table, and names that say what you get): why the table exists, what each row says, and the one
row that carries a security property.

## Why it exists

Three caretakers proxy this contract over three different narrowings: `fs_file_caretaker`,
`fs_subtree_caretaker` and `fs_nameset_caretaker`. Each had a hand-written `match` over the
opcode. Nothing made a `match` and this contract agree, so the failure mode was silent. A verb added
to the contract was simply absent from a caretaker, and the capability quietly was not there.

That happened. Milestone 57 (partitioning and formatting a real drive, and extended attributes)
added the four extended-attribute verbs. None of the three caretakers was taught them, and nothing
failed. It took a reader asking why there were three of these to find it.

## The table

`file_system_contract::verb::TABLE` is one row per opcode:

| field | what it answers |
|---|---|
| `operand` | what the request word's length field counts: nothing, a **directory name**, a **payload**, or a rename's two of each |
| `carries_w1` | whether the second word means something the server reads |
| `mints_handle` | whether a success is a new handle a renumbering proxy must install |
| `needs_all` / `needs_any` | the `dir` rights this server will demand. Kept apart because `Rights::allows` is "all of", and `OPEN`'s "read **or** write" folded into it would refuse capabilities this server accepts |

`verb::of(op)` is the whole of a caretaker's dispatch. The completeness check is a `const assert!`,
so adding an opcode to `fs` without a row here is a compile error. That is the deliverable: forgetting
now fails the build, rather than producing a capability that is quietly missing.

## It shares the dispatch, never the attenuation

The three caretakers narrow by different means and stay three programs. `fs_subtree_caretaker`
performs no checks at all. `fs_nameset_caretaker` filters a name on every verb whose operand is one.
`fs_file_caretaker` translates between two protocols, and has a per-verb policy of its own
(`verb::file_grant::POLICY`). A table lookup that picks a length or a zero cannot refuse anything.
That is why it can be shared by a program whose whole property is that it has no branch to get wrong.

## The row that carries a security property

The `Operand::Name` versus `Operand::Payload` split is the row that carries a security property. An
attribute name is a payload, so the four attribute verbs pass `fs_nameset_caretaker`'s filter without
being compared against the set. A directory name never does. Both directions of getting that wrong
are real. Filter the attribute name, and a program cannot read its own file's attributes. Stop
filtering a directory name, and a set capability escapes its set.
