# The stdin contract, proposed

An appendix of [milestone 892 (a std program at the prompt holds the network and a name
resolver)](../0892-a-std-program-at-the-prompt-holds-the-network-and-a-resolver.md), kept apart
under the word cap of §212 (a prose budget).

calef chose a byte source, then its shape: (a), on #2405, 2026-10-19 (UTC). It is built as
written here. Names are provisional.

- (a) Recommended: a cooked read on a badged terminal copy. A new `line_editor::proto` opcode,
  `OPERATION_READ_LINE_BYTES`, asks for up to eight bytes of the current line. The terminal runs its
  own line discipline (echo, editing, `^C`), and once a line is complete it answers r0 = count and
  r1 = the bytes, packed as `OPERATION_READRAW` packs them. Later calls drain the rest of the line,
  `\n` included. A count of 0 is end of input (`^D`). No page is involved. The copy carries a
  second badge value, `STDIN_BADGE`, on which the terminal serves this opcode and nothing else. It
  follows the raw-reader badge of milestone 709 (a graphical terminal session on the no-keyboard arm
  holds only the raw half of the boot discipline). std reads it at
  `std_runtime_contract::STDIN_SLOT`, slot 29, after the resolver slots 22 to 28. The spawn service places it only for a table program whose
  manifest sets `Manifest::console_input`, which no note can spell yet (as with `installer`). That is
  `jig` today. §270 (a package manager holds an installer endpoint, not the spawn endpoint) lets an
  owner's `grant` line give it to installed bytes, and that half is unbuilt. The shell is parked on the job's result while the job runs, so the two never read
  at once.
- (b) Push. The shell feeds `byte_sink_contract` frames into an endpoint at that slot, as a
  pipe's upstream stage does. No new opcode. But the shell cannot read the console and wait on the
  job's result at once (one wait point), and a job that never reads strands the typed line.
- (c) Raw. std gets milestone 709's raw-reader badge, turns raw mode on, and echoes and assembles
  lines itself. No new opcode, but every std program carries a line editor in its PAL, and leaving raw
  mode discards whatever was typed under it.

Recommended: (a). It is the only option where `read_line` behaves as a person expects (echoed and
editable) with no shell in the path.
