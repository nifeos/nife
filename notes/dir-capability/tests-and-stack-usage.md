# What the tests prove, from inside and outside the guest, and the stack measured

*Name: ratified 2026-10-18 (calef, "Approve as a group"). Refused `tests-and-stack`.*

An appendix to [notes/dir-capability.md](../dir-capability.md), moved there on 2026-10-11 (UTC)
under §212 (a prose budget). It holds the attacker runs, the host-side check of the image, and the
FS server's stack before and after `RENAME`.

## What the guest tests prove, and from where

Three `#[test_case]`s, one module for both ISAs rather than an aarch64 test with a riscv twin.
Nothing in them is architecture-specific, so the parity gate, §19 (architectural parity is a
tenet), is met by the same test running twice.

Each wires a `fs_subtree_caretaker` holding a capability to the fixture's `sub` with one rights set,
and runs the `ROLE_DIR_ATTACKER` role of `fs_test_client` against it. The image carries:

```text
  /            motd  scratch  sub/  other/
  /sub         inner  deeper/          <- the granted capability is here
  /sub/deeper  leaf
  /other       secret                  <- never reachable from the grant
```

The attacker is told nothing about its own grant beyond a run index. The index only keeps the names
it creates distinct across three runs that share one image. It attempts every verb and reports a
bitmap of what got through, and the kernel test asserts the *exact* expected set for that
configuration. So the specification lives in the test rather than in the program under test. The
three runs are each other's controls: a caretaker that refused everything fails the wide run, and
one that allowed everything fails the narrow ones.

| Run | Rights | Expected |
|---|---|---|
| read-only | `DESCEND｜READ｜ENUMERATE` | opened its own file, enumerated, descended |
| full | `ALL` | those three, plus created, wrote, renamed, made a directory |
| append-only | `READ｜WRITE｜CREATE` | opened its own, created, wrote |

The append-only run is the roadmap's motivating sentence made a test, and two rungs in it are the
interesting ones. `DESCEND` withheld while `CREATE` is held means it can make a file and cannot make
a directory. `REMOVE` withheld while `CREATE` is held is "add to this, destroy nothing" exactly: it
creates a name and then cannot move it, through the same code the full run moves it with.

What the attacker attempts, and what makes each attempt real. `motd` is in the granted directory's
parent, and `other/secret` is in its sibling. Both are on the image and one directory entry from the
caretaker, which could open either on any request it liked. So each refusal is a fact about the
capability rather than about the filesystem. The attacker tries `..` at every rights setting. It asks
for a right its capability does not carry, which must be refused, not quietly narrowed. It descends
asking for nothing and checks that the resulting capability can do nothing at all. And it guesses
handle numbers past anything the caretaker could have minted.

Two bits exist to stop the whole thing being vacuous. `OPENED_ITS_OWN` is the control. Without it,
every refusal above is equally consistent with a caretaker that answers no to everything or a grant
that reaches nothing. `GRANTED_ACCESS_FAILED` fires when the thing it *should* be able to do did not
work, so a capability that reaches nothing reports itself rather than passing as perfectly confined.
`ENUMERATED_A_STRANGER` catches a listing that contains a name from outside the grant. A listing is a
rendering of authority, and a stranger in one is an escape even though nothing was opened.

### The name-set run

A fourth test runs the third caretaker, `fs_nameset_caretaker`, which inspects a name on every
request. The subtree caretaker is covered by the three runs above, and the per-file caretaker by
its own attacker. The name-set test grants one name inside `sub` and leaves `deeper` out. Its
witness holds `dir::READ`, which `rm`'s own witness does not, so the naming question is asked
through verbs `rm` never sends. It must still read and set the attributes of the one file the set
names.

## And from outside the guest entirely

The bitmap is a statement by the thing being tested. The other kind of evidence is a different
process, on the host, with the pinned engine, reading the image the run left behind
(`xtask::redoxfs_subtree_was_confined`). It asserts:

1. every fixture name is still in the image root (a capability granted on `sub` can remove nothing
   above itself, so a missing name is an escape too);
2. no name of the attacker's making is in the root, which is the upward escape;
3. its creations are in `sub`, which stops claim 2 from being true of a capability that reaches
   nothing. And `sub` holds both a renamed name and an unrenamed one. That is the `REMOVE` rung
   witnessed from out here: one capability moved a name, and another, running the same code against
   the same directory, could not;
4. `other/secret` and `sub/inner` read back byte for byte.

A program that broke out and then lied about it would still have left the file on the disk.

## The FS server's stack, measured before and after

§27 (the filesystem service) records that this server's stack is sized by measurement rather than
chosen. `CREATE` and
`TRUNCATE` had added a level of tree recursion and left it 528 bytes short, which presented as a
mystery 900-second test. So the four verbs this milestone adds are exactly the kind of change that
warrants looking, and `RENAME` most of all: it is `find_node` twice, then `link_node` and
`remove_node`, all in one transaction.

The high-water mark went down, by 3,776 bytes on aarch64 and 3,696 on riscv64:

| | aarch64 | riscv64 | of a 397,312-byte grant |
|---|---|---|---|
| the four verbs, before `RENAME` | 135,744 | 135,856 | 34% |
| with `RENAME` | 131,968 | 132,160 | 33% |

Not attributed, deliberately. A verb that recurses more cannot make the deepest path shallower, so
this is the compiler laying the serve loop out differently, not `RENAME` being cheap. Measuring the
frame it costs would need an instrument aimed at that request rather than a per-boot maximum.
Milestone 37 (prove RedoxFS's crash consistency) saw the same shape (an 8 KiB drop it also declined
to explain), and §27 records both
numbers rather than a story. What matters for the gate is the headroom. Two thirds of the grant is
free either way, comfortably above the quarter-left floor the test fails under.
