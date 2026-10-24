# What the gate found on its first runs

*Name: provisional. Named after the parent's own heading.*

Two findings from the first days of the gate [`notes/counted-claims.md`](../counted-claims.md)
describes. The second one caught the gate's own author.

## A crate no shard proved

The `harness-crates` derivation reads the tree, and `script/verify` reads a hand-kept table of
crates to prove. They disagreed by one. `multicast_dns_protocol` landed with milestone 55 (Time Machine) carrying
three harnesses and was never added to that table, so nothing proved them and nothing said so.

The suite went green *faster*, which `notes/verification.md` already names as the dangerous failure
mode. It simply arrived through the crate list rather than through the shard packer that note was
worried about.

That is the argument for deriving rather than maintaining, made by the mechanism on the day it was
built. The row was added. The omission is recorded in `script/verify`'s comment, where the next
person to edit that table will read it.

## A line that outgrew the renderer

Then came the documentation of this very check, in `notes/scripts.md`'s `script/lint` table row.
The addition took that row from 1835 bytes to 2108. That overflowed
`documentation::render::LINE_MAX`, which is 2048 because 1835 was the measurement it was sized
against.

The renderer truncated. The failure arrived as a `manual` render test asserting that no character
is dropped, quoting text three hundred lines further down the file. Nothing connected the two.

