---
status: REFUSED
raised: 2026-08-31
refused_by: §87
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 199. GitHub shows this public repository as having no license (it does not; retracted)

Minted and retracted the same day, 2026-08-31. Found while checking whether the repository was public
for DECISIONS §135's (running GPL software is aggregation) distribution question. *(Number
provisional until the merge queue lands it.)*

Refused because the premise was false: the terms are ratified by DECISIONS §87, and GitHub
already shows them. One tool's field read as none, and nothing needs fixing.

In brief. `gh repo view nifeos/nife --json licenseInfo` returns none. The tree carries
`LICENSE-MIT`, `LICENSE-APACHE` and `license = "MIT OR Apache-2.0"` in the workspace manifest, and
DECISIONS §87 (MIT OR Apache-2.0, and why the GPL's lesson does not transfer) ratifies the choice.
GitHub's license detector does not recognize the dual-file Rust convention, so a stranger opening
this repository is shown a public project with no stated terms.

## Revisit

- **Condition.** GitHub's repository page stops showing both licenses, or shows only one of MIT and Apache-2.0, which misstates the dual license §87 ratified.

## What the block used to argue, kept for the record

Below is the reasoning as written, and it is wrong in its premise rather than its values: had the
sidebar been silent, this would all have held.

## Why it would have been worth a block rather than a shrug

AGENTS.md's third principle is that a newcomer must be able to succeed without asking anyone, and
its test is whether a competent stranger with only this repository can get to a passing build and a
correct mental model. License terms are the first thing a cautious stranger checks and the first
thing an employer's policy asks about, and here the answer at the place they look is silence. That
is the same defect class as a name that misleads: the reader meets it before they meet anything
else.

It is also newly relevant rather than merely tidy. §135 turned on who conveys what, and the honest
statement of this project's position is *source is public, no binaries are conveyed, the terms are
MIT OR Apache-2.0*. The third clause is the one GitHub is not carrying.

## What to check before choosing a fix

The usual remedies are a root `LICENSE` file or a `.github` setting, and **which one is right is not
obvious and should be measured rather than assumed**:

- A root `LICENSE` that merely points at the two files may still not be detected, since the detector
  matches known license texts rather than prose.
- Copying one license's full text to `LICENSE` would make GitHub display that one, which would
  misstate a dual license in the direction of whichever was copied. That is worse than silence.
- `README.md` almost certainly already states the terms; being right in the README and silent in the
  sidebar is exactly the split this milestone is about.

The likely honest answer is a root file carrying both texts with the `OR` stated first, checked
by actually looking at what the sidebar renders afterwards. **Verify by observation, not by
expectation**, since the detector's behavior is the whole question.

## BUGS

- This is cosmetic in the way a name is cosmetic, which is to say it is not, but it also blocks
  nothing and no gate will ever notice it.
- Nothing here checks the claim stays true. A license-visibility check would have to ask GitHub's
  API, which no gate in this tree does, so the fix is a one-time act that can silently regress.
- The dual-license convention is Rust's, and GitHub's detector is not going to change for us.
  Any fix is working around somebody else's heuristic and may break when that heuristic does.

## Index row

Retracted the day it was minted. The premise was false: the sidebar reads "Apache-2.0, MIT
licenses found" and `gh api repos/<slug>` returns a full license object. The `none` came from one `gh repo view --json licenseInfo` field, which was read as the world's answer without anyone
opening the page. Kept because the tooling defect is real and the next person will hit it.
