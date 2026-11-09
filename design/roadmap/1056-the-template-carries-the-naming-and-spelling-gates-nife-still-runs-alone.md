---
status: NOT-STARTED
promoted_from: the-template-carries-the-naming-and-spelling-gates-nife-still-runs-alone
raised: 2026-11-05
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1056. The template carries the naming and spelling gates nife still runs alone

Raised by lane/record-follow-on-items. 
Reuse: the template's Rust lexer and word list (`helpers/rust_names.py` and
`helpers/naming_words.toml` in `nifeos/crate-workflows`), and nife's vendoring of them at a pinned
revision (`helpers/vendored_name_gates.py`, `vendor/crate-workflows.pin`). The checks themselves
exist in nife and move; the lane writes none from scratch.

## Background

A gate that is not specific to nife lives once, in the shared template of milestone 887 (crate
repositories share one CI template), and nife vendors it at a pinned revision. Package and crate
repositories call the template's workflows. nife's own gates (`script/names`, `Name:` blocks, the
roadmap, the decisions, the kernel and the image) stay in nife.

The template already holds the spelled-out words and the unit symbols. nife still runs these alone:

- the British-spelling check of `helpers/house_style.py`;
- the question-shape check of `helpers/name_checks.py`: a function that answers yes or no reads as
  a question (`is_`, `has_`, `can_`), as design/naming.md's rule says;
- `script/lint`'s naming checks 1 to 3: no `-d` suffix on a component or a crate, no "daemon", and
  one spelling for contract crates, `*_contract`.

So no gate holds a crate to them once it leaves nife, and none ever holds a package repository to them.

## The work

1. Move each shared check into the template as a self-contained helper, run in the template's
   `check / standalone` job and its own `script/lint`, with its word list apart from the code, at a
   ceiling of 0 for a crate repository.
2. nife vendors the moved helpers at the pinned revision, as it does the word list, and keeps only
   its ratchet halves (ceilings, kept lists, the hold on a branch to its merge base).
3. Milestone 887's gate table states where each of these checks lives.

## Open question

Milestone 887's table classes naming checks 1 to 3 as nife's own vocabulary. The no-"daemon" and
no-`-d` rules read as general naming rules; the `*_contract` spelling reads as nife's own. The
lane that builds this asks which of the three move, and moves the British-spelling and
question-shape checks either way.

## Done when

- A crate repository on the template fails a British spelling and a yes-or-no function that does
  not read as a question, and the template's canary shows each red on a planted break.
- nife runs the same checks from its vendored copy, and its own copies are gone.

## Index row

Naming and spelling gates that are not specific to nife still run only in nife. This moves them into the shared crate template so every crate repository fails the same breaks.
