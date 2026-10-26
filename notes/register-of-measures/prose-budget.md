# The prose budget

*An appendix to [the register of measures](../register-of-measures.md). The name is provisional.*

No document in scope is over 3,000 words, and `script/lint` keeps it that way. The cap is
[§212 (a prose budget)](../../design/decisions/0212-a-prose-budget-for-every-document.md), and the
gate is the prose ratchet, `helpers/prose_ratchet.py`. No row of `design/prose-baseline.tsv`
holds a word ceiling, so a document over the cap fails unless it carries a marked `prose-budget`
exception. The measure has no chart: at zero, the gate is the whole record (calef, 2026-10-14
UTC).

## Scope and count

The scope is every `.md` directly under `design/`, `design/decisions/`, `design/roadmap/`,
`design/roadmap/proposals/`, `notes/`, `notes/coes/` and `briefs/`, plus `AGENTS.md`, plus their
appendices and `design/tenets/`. That is the ruling's own evidence scope with the appendices §212
puts under the same cap. It is one definition, `documents()` in `helpers/prose_ratchet.py`. Words
are the whitespace-separated tokens over the whole file, less a generated table (`budget_words`), so
each appendix counts as its own document.

## BUGS

- A word cap rewards moving prose rather than cutting it, as §212's own `BUGS` says. Every file can
  pass while the tree's total grows, and nothing here measures whether anybody reads the words.
