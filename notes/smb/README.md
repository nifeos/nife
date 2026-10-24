# SMB: the appendices to notes/smb.md

[`notes/smb.md`](../smb.md) is the page to read: what a Mac mounted, the scale and reason of the
removal, the NTLM lesson, and what never worked. The SMB code was removed on 2026-08-30. The files
here keep the rest of the note as it stood at `685900ec`, the last commit that holds the code, so
read every present tense in them as "as of `685900ec`".

- [`ntlm-removal.md`](ntlm-removal.md): what went with the NTLM half, and why it is the lesson.
- [`adapter-as-it-stood.md`](adapter-as-it-stood.md): the adapter and its pieces.
- [`wire-decisions.md`](wire-decisions.md): every wire choice and its reason.
- [`apple-extensions.md`](apple-extensions.md): the `AAPL` create context and `FULL_SYNC`.
- [`throughput.md`](throughput.md): the 64 KiB transfer, measured.
- [`testing.md`](testing.md): the host tests, the gate and the commands.
- [`limitations.md`](limitations.md): the BUGS list in full.
- [`remaining-work.md`](remaining-work.md): the work lists as they stood.

Name: provisional, minted 2026-10-11 (UTC) by `lane/ten-longest-notes`, which split the parent under
§212 (a prose budget), for the directory and every stem in it. The directory follows the
`notes/<stem>/` appendix convention §212 set, so its name is the parent's stem. Each file is named
for its content. calef names directories and files, and `script/names --unratified` lists each stem.
