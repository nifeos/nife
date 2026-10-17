# Instructions for agents

Instructions an agent reads when one kind of work calls for them, each a directory holding a
`SKILL.md` in the open Agent Skills format (https://agentskills.io/specification).
`AGENTS.md` links every one with the moment to read it, so an agent that only reads files
reaches them by following links; `.claude/skills/` and `.agents/skills/` hold symlinks to these
directories, so tools that load skills find them too. They were moved whole out of `AGENTS.md`
on 2026-10-06 (UTC).

Name: ratified 2026-10-06 (calef, pull request #1726). Refused `notes/agents/` (read as a
roster of agents rather than instructions for them). His ruling: "notes/skills/". The six entry
names below were ratified the same day in the same place, in his words "The entry names are fine
as they are". This block covers them: `script/names` reads it for every entry here that holds
a `SKILL.md`.

`helpers/agents-index-check.py`, run by `script/lint`, fails when this table, the directories,
`AGENTS.md`'s links and the symlinks disagree.

| Name | Description | When to read it |
|---|---|---|
| [`maintainer`](maintainer/SKILL.md) | The maintainer and steward roles in nife: load before briefing a lane, launching or resuming developers, choosing how many lanes to run, gating or merging a lane's pull request, pruning worktrees, or reporting queue depth. Holds the top-up rule (when a developer finishes, launch the next work before writing the report; a conversation with an architect never blocks the queue) and what bounds lane count (the collision surface, memory, disk). A top-level session in this repository is the maintainer. Load the developer-lane skill as well, since a maintainer briefs against it. | Before you brief, gate or merge a lane, or clean up after one. |
| [`developer-lane`](developer-lane/SKILL.md) | Rules for a developer lane in nife: a subagent executing exactly one milestone in its own worktree and branch. Load first, before any work in a lane, and when briefing one. Covers the claim (script/claim, an empty commit and a draft pull request before any work), what a developer may never do (merge, mint, edit design/decisions/, design/ or AGENTS.md except its own roadmap block), never polling CI (end the turn with `WAITING [<owner>/<repo>:]<run_id>... on <what>`), when a lane stops, the report and its handoff, where identified work must go (a proposed milestone or a BUGS entry), and shared state (provisional section numbers, each worktree's own toolchain link, branches that hold knowledge). | Before any work in a lane. |
| [`decisions`](decisions/SKILL.md) | How nife decides, recommends, and takes a question to an architect. Load before choosing between options, making a recommendation, or deciding how much care a choice deserves; and when something may be an architect's call (a design fork, a wire format, a name, the syscall surface, a new dependency, a design/decisions/ section, a published number), when holding work for an architect, adding the needs-architect label, writing a "## What I need from you" comment or a PROPOSED decision file. Holds: move fast on what can be undone and be methodical on what cannot (the test is who else has already acted on it); elegance and performance beat implementation convenience (would I still choose this if both options were the same amount of work? if not, say the recommendation is about effort); the seven questions a fork must answer before it reaches an architect; and why requesting a review from calef silently does nothing. | Before you choose between options, recommend one, or hold work for an architect. |
| [`naming-authority`](naming-authority/SKILL.md) | Who names things in nife: load before minting, choosing or changing the name of a crate, a program, a module, a script or a public function, or before ratifying or performing a rename. Names are an architect's call: ship a provisional name, say so in the report, and never rename on your own initiative. Points to design/naming.md for the conventions. | Before you mint or change the name of a crate, program, module or public function. |
| [`worktree-commits`](worktree-commits/SKILL.md) | Committing in nife: load before committing, squashing, force-pushing or saving work aside in a lane worktree. Never use git stash (the stash stack is shared machine-wide by every lane), and squash against the base SHA recorded when the branch was cut, never against origin/main, which moves under a lane while it works. One purpose per commit, the message says why, commit early and push, curate before reporting. | Before you commit, squash or push in a worktree. |
| [`qemu-hygiene`](qemu-hygiene/SKILL.md) | QEMU hygiene in nife: load before running QEMU by hand (a demo, a boot, an interactive run) and after any session that ran it. Every interactive or demo run is bounded with helpers/qemu-bounded.sh, since timeout(1) is absent on macOS and perl alarm does not work on QEMU. Covers why halt() uses wfi, and how to find and kill a leaked QEMU without killing someone else's gate (pgrep -l qemu, walk the parent chain, lsof target/nifefs.img). | Before you run QEMU by hand, or after a session that did. |
