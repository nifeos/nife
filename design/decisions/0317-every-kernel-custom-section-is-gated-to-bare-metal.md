---
status: DECIDED
raised: 2026-10-29
decided: 2026-10-29
ratified_by: calef
---

# 317. Every kernel custom-section link_section is gated to the bare-metal target

*Section number provisional; the merge queue mints it at merge. Ruled 2026-10-29 (UTC) in the
maintainer's session, relayed by the maintainer.*

The kernel compiles twice. The linker scripts (`kernel/link-*.ld`) place its custom sections for
bare metal; a host compile reads none of them. The hot path goes in `.text.hot.*` sections, and the
interrupt and secondary stacks get regions of their own. Those names mean something only to the
bare-metal scripts.

Rustc validates a custom section name against the object format while it codegens the item, so a
Mach-O host refuses an ELF name with `invalid Mach-O section specifier`. Every custom-section
`link_section` under `kernel/src` therefore carries
`#[cfg_attr(target_os = "none", unsafe(link_section = "..."))]`. That gate keeps the exact ELF name
for the linker scripts and drops the attribute on a host. `script/lint` fails an ungated one
(`helpers/lint_link_section_target.py`), so the next one cannot land.
[notes/kernel-proofs.md](../../notes/kernel-proofs.md) and
[notes/linker-scripts.md](../../notes/linker-scripts.md) carry the reasoning where a reader meets
the sections.

## What it refuses

An ungated custom section, which a host compile refuses outright. And a per-architecture gate, as
in `#[cfg(target_arch = "aarch64")]`. One target-wide gate excludes every host and keeps a single
attribute on each section.
