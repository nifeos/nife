# Vocabulary

*An appendix to [`design/naming.md`](../naming.md). It holds the words the tree gives one meaning,
and the test each one sets for words beyond it.*

## Words with one meaning here

| Word | Means | Test it sets |
|---|---|---|
| half | one side of a two-way split | a branch of a split with three or more is an arm, part, piece or leg; `script/lint` refuses a count above two before "halves" |
| `cpu` | the unit the scheduler runs a thread on, one hardware thread as Linux and POSIX count them | `core` means the physical core and never a cpu; a GPU or other device processor takes its own name (`gpu`); a devicetree `cpus` node and RISC-V `hart` in RISC-V-only code keep their owners' words |
| identity | the string a client presents (`alice`) | the identity string names a principal's subtree, per §117 (a principal\'s subtree is named by its identity string) |
| `principal` | the authenticated actor that results, holding a fresh capability set | `login` turns an identity into a principal; one word for both would lose the difference `login` performs |
| `user` | a person | user mode is `user_mode_` (`user_mode_runtime`) |
| `keeper` | holds something alive and restarts nothing | `supervisor` restarts what faults; `caretaker` serves narrowed access |
| `sink` | the end of a stream nobody reads further | name a program for a terminus only if no capability grant could end it |
| grant | authority a program receives (`FileGrant`) | what a program asks for is a declaration (`FileGrantDeclaration`); not every `Spec` is one, so `RunSpec` is `ParsedInvocation` |
| contract | an agreement between two programs | the `_contract` crate suffix; a type that selects which agreement applies may say so (`StartupContract`) |
| option | a letter or word on a command line, as POSIX says | a flag is a bit, so `options`, never `flags` |
| `nife` | the project | lowercase everywhere, prose included |

## A terminus that is structural, or merely current

Ask whether the name claims an end of stream that the design guarantees, or one that only reflects
what nobody has built yet. `audit_sink` discarded each login record because printing it needed a
capability nobody had granted. One grant would have made it keep records, and its name would then
lie, so it is `login_audit_receiver`: true today, and true when it records.

`file_sink` passes the test. Its client holds a capability over which no message but append is
expressible, and no grant could change that. `byte_sink_contract` names what it carries, and
`terminal_sink_caretaker` hands out a sink that cannot read.

## Stems the tree keeps

- `login` names the whole family (`login`, `login_contract`, `login_test_client`, the kernel's
  `login_service`). It does not authenticate, since `credentialer` checks the secret, but `login`
  is the field's word for the role, and a reader arriving from Unix lands in the right place. The
  program's docs say where it departs from Unix. Refused: `authenticator`, which names the half it
  does not do, and `powerbox`, a term of art almost no reader knows.
- `system_installer` installs the whole system and `jig` installs packages. A distribution prefix
  (`basalt_installer`) would go false, because the program copies whatever image the stick booted.
- `Guid` and `Uuid` name different things. A GUID is mixed-endian on disk where RFC 9562's UUID is
  big-endian, so one word for both would assert a byte order the code does not produce.
- `session` carries two senses in this repository: a login session in code, and an agent session in
  the process documents. Nothing in code confuses them.

## Choosing a word

- Avoid a word whose kernel meaning differs from yours: `schedule`, `swap`, `map`, `console`,
  `argument`, `tick`, `probe`, `frame`, `flag`. "Argument" is always qualified, as in program
  argument.
- When one word names two things, give each a qualifier: `state_handoff`, a stack frame or a page
  frame.
- Prefer the word the field already uses: `pending` (`PendingReaps`) over `due`, `operands`
  (`TouchOperands`) over `args`, `mount` (`MountedTree`) over `SecondDir`.
- A suffix a reader knows from another family claims membership in it. `SETMTIME_AT` suggested
  POSIX's `*at()` calls and was not one.
- A term coined here stays when the document that uses it defines it on the spot ("lift" in
  `notes/interleaving`). A rename would trade it for a vaguer word.
- Spell every name and word the American way: `color`, `catalog`. `helpers/house_style.py` refuses
  new British forms.
