# The inflow of 2026-10-14, triaged

Lane maintainer/mutation-inflow-2026-10-14-triage, 2026-10-14 (UTC), for fatal risk 3 (the tests do
not test). Name provisional. Milestone 740 (the survivors a merged pull request adds are checked
against a triage record) runs the weekly `inflow against the triage record` job. It failed on
[38654094250](https://github.com/nifeos/nife/actions/runs/38654094250), the 2026-10-14 census at
`deff22e9f`. Every key on its list now has a row in `notes/project-metrics/mutation-triage.csv`.

## What was on the list

Against the 2026-10-12 census, 45 keys were new on lines a merged pull request wrote. Two already
had rows (`abi`'s `admitted` and `e1000e`'s `tx_word`, both `| with ^`), so 43 were untriaged. Two
more keys were new on older lines, so the job reported them and did not fail on them. One of those,
`documentation`'s `table_widths` `> with >=`, already had a row. The other, `portable_executable`'s
`write_pe` `| with ^`, is triaged here too, so 44 rows are added.

Each crate was re-run with `cargo mutants -p <crate> --file <path> --re <the functions>`, on the
lane's worktree (macOS, base `571da1a2e`), after the new tests. Each row's disposition is what that
run showed.

## The rows

| crate | keys | killed | equivalent | gap |
|---|---|---|---|---|
| `filesystem_protocol` | 12 | 12 | 0 | 0 |
| `grant_plan` | 7 | 7 | 0 | 0 |
| `abi` | 5 | 4 | 1 | 0 |
| `swish` | 4 | 4 | 0 | 0 |
| `package_index` | 4 | 4 | 0 | 0 |
| `socket_protocol` | 3 | 3 | 0 | 0 |
| `portable_executable` | 3 | 0 | 3 | 0 |
| `documentation` | 2 | 1 | 1 | 0 |
| `uefi_loader` | 1 | 0 | 1 | 0 |
| `name_resolution_protocol` | 1 | 0 | 1 | 0 |
| `http_response` | 1 | 1 | 0 | 0 |
| `e1000e` | 1 | 0 | 1 | 0 |
| total | 44 | 36 | 8 | 0 |

No survivor was a bug. The kills are shapes "What the tool has taught" already names.

- A function nothing in the crate called. `filesystem_protocol`'s `walk::rg_match_path` is the list
  the kernel harness and the host's `rg` both compare against, so a wrong path would agree on both
  sides. `grant_plan`'s `IndexStore::rights` and `Prog::resolver_zone` have no caller in the tree
  yet. Each gets a test that spells its answers out.
- A constant checked only where the host never builds. `abi`'s `rights::ALL` and `SPEND_ONLY` are
  asserted against the kernel's twin at compile time, in a kernel build. A host test pins them as
  numbers, and `rights_are_distinct_single_bits` covers `BIND` with the other four rights.
- Tests that compare values only with each other. `socket_protocol`'s ephemeral tests ask that no
  port repeat, which any range passes. A new one walks 49152 to 65535 and the wrap.
- A boundary no fixture met: a head that fills `http_response`'s buffer to the byte, a five- and a
  six-digit port, a short even run of hex, a slot numbered 10, and a table column of width zero.
- Output no test read. `swish`'s index-store rows in `caps jig` get their first assertion.

The equivalents are `|` against `^` over disjoint constants (`abi`, `e1000e`,
`name_resolution_protocol`, and `portable_executable` twice), plus three with their own argument:

- `portable_executable`'s `put64` writes `ImageBase` 0 into a buffer made of zeros, and it has no
  other call.
- `documentation`'s `write_table_rule` `+= with *=` has two copies. The zero-width table kills the
  `+= *width` copy. The survivor is `+= 3` between columns. The column already holds the margin by
  then, and `close_line` only asks whether it is past zero.
- `uefi_loader`'s `Walk::prop` `> with >=` refuses a property that ends exactly at the structure
  block's end one token early. The walk refuses the next token with the same `Truncated` error,
  since no room remains for `END`.

## BUGS

- A direct call with slot 10 kills `swish`'s `write_slot` key. No preview row passes 10
  today, so the test guards the padding for the first row that does.
- `put64` has one call, which writes a zero. It is kept because it states the header field. A
  second call would make its mutant killable.
