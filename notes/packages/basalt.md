# basalt

*Name: ratified 2026-10-05 (calef, the distribution's name, reserved by milestone 120 (nife and the
organization)). Refused `travertine` (needs a geologist's footnote), `magma` (molten rock, not a
rock), `tufa` (confused with `tuff`), `scoria` (also means slag), `flowstone` (a commercial
product), `tephra` (loose fragments, not a rock), `obsidian` (the notes app), `pumice` (reads as
light). Each reason is in [basalt.md](basalt.md).*

*The name of nife's first-party distribution, the `nifeos/basalt` repository that milestone 120 (nife and the organization)
reserved. The ruling and its refusals are recorded in [this directory's index](README.md), where
`script/names` reads them. What basalt assembles is decided in
[§151 (repository goal is independent release)](../../design/decisions/0151-repository-goal-is-independent-release.md) and built by milestone 198 (a package manager).*

## Why the name stands

calef doubted it on 2026-10-05 because the distribution is bumped daily and "basalt" suggested
something slow. He kept it after a review of the alternatives, for two reasons.

- No rock says "daily" to a reader who is not a geologist, so there was nothing better to trade it
  for.
- A solid name on a rolling distribution promises what rolling-release users want: that it does not
  break.

The geology also holds up. Basalt is fine-grained because it cools fast; granite is the slow one.

## Refused

| Name | Why |
|---|---|
| `travertine` | Limestone still forming at hot springs. The best "fresh" candidate, but its meaning needs a geologist's footnote, the same tax that retired `sial`. |
| `magma` | calef's own idea. It is molten rock, not a rock, so it breaks milestone 120 (nife and the organization)'s convention that a distribution is a rock (an aggregate of minerals). From memory, unverified: the Magma computer algebra system and the Linux Foundation's Magma mobile core also use it. |
| `tufa` | Confused with `tuff`, and the crate name is taken. |
| `scoria` | Also means slag, and the crate name is taken. |
| `flowstone` | FlowStone is a commercial product. |
| `tephra` | Loose fragments, not a rock. Apache Tephra exists. |
| `obsidian` | The notes app. |
| `pumice` | Reads as "light", and is milestone 120 (nife and the organization)'s example of a third party's distribution. |
