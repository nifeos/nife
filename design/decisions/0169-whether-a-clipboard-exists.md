---
status: DECIDED
raised: 2026-09-19
decided: 2026-10-28
ratified_by: calef
---

# 169. Whether a clipboard exists here, and what it is scoped to

Ruled 2026-10-28 (UTC): B, a clipboard scopes to a login session, per authenticated principal;
reading it costs the same proof anything under that subtree costs. Milestone 49 (users, login, and attribution)'s login-boot-wiring
must exist first; milestone 180 waits on both.

Raised 2026-09-19 by milestone 435 (forty-five milestones are gated on a decision nobody wrote
down)'s lane. That lane found milestone 180 (a clipboard in a system with no ambient authority)
gated on `DECISION` with no decision anywhere a reader can open. calef minted the block on
2026-08-26 after checking the roadmap for a gap and finding that nothing in `design/` or `notes/`
mentions a clipboard at all, not even as a named limitation. *(Section number provisional until the
merge queue lands it.)*

## What is being decided

Whether this system has a clipboard, and if so what it is scoped to. It is not a driver question and
not a wiring question. A clipboard is shared, mutable state that more than one principal reads and
writes. That is the shape this tree has a standing rule against.

## The tension, stated precisely

Every existing multi-client object here is minted, held and delegated explicitly. A directory
capability goes to the principal it names ([§47](0047-directory-rights.md), a directory capability
carries six rights, and a child can never exceed its parent). A channel endpoint is minted per
connection (`login_protocol::CONNECT`). The compositor's shared windows are each their own object
rather than one global surface every client can address.

A clipboard is the opposite of all of it. One slot, last writer wins, readable by whoever asks, with
the system deliberately not checking who "whoever" is. That is the entire feature, and it is not
incidental to it. **A capability-shaped clipboard that required proving you were the principal who
copied would not be a clipboard**, because the whole point is that the pasting program is not the
copying program and the system introduces them anyway.

So this cannot be scoped before the tension is ruled on. That is why it arrives as a decision rather
than as work.

## Prior art, read rather than recalled

- X11's `PRIMARY` and `CLIPBOARD` selections: ambient, the ownerless-buffer shape exactly.
- Wayland's data-device protocol: still ambient, but with an explicit offer and request handshake
  rather than X11's poll-on-demand. That is closer to this tree's delegate-explicitly grain without
  escaping the tension.
- OSC 52 in terminal emulators: the copying program names its destination, which is a targeted
  transfer rather than a clipboard.

Both of the first two are the shape to read before designing this, not to copy.

## The options

| | shape | cost |
|---|---|---|
| **A** | **One systemwide object**, held by whichever component owns the terminal or the compositor, offered to any client that asks. | What every other system calls a clipboard. It is ambient authority by definition, scoped only by "anything running on this machine can read the last thing copied". Allowed by AGENTS.md's ladder as a **named** exception, which means it has to be written down as an exception and as a foot gun where a reader meets it; an unmarked one reads as a design and the next person extends it. |
| **B** | **Scoped to a login session**, so a clipboard exists per authenticated principal and reading it costs the same proof anything else under that subtree costs. | Closest to this tree's grain, and it makes the exception small instead of absent. It means a clipboard cannot exist at all until milestone 49's login-boot-wiring piece does, which today it does not. |
| **C** | **Not a clipboard: a targeted transfer.** The copying program names its destination. | Refuses the shape that causes the tension, at the cost of not being what "select, then paste somewhere else" means to a person using the machine. |

**Recommendation: B, and A written down as what B is an exception to.** B keeps the feature people
actually want while making the ambient region exactly one principal wide. That is the same move
`SURVEY` made for enumeration (a shell sees its own children and nothing else) rather than a new
idea. C is the purest and answers a different question than the one a person is asking.

The effort note AGENTS.md asks for: B costs more than A and depends on a milestone that has not
landed. So if A is chosen for those reasons, it is an effort decision and should say so in those
words.

## How reversible it is

The scope is the expensive half. Whatever a clipboard is scoped to is what every program that uses
it is written against. Widening it later is easy, while narrowing it later breaks programs. That
argues for deciding narrow first, which is the argument for B independently of taste.

## What is blocked until this is answered

Milestone 180, entirely. It is the first record of the gap rather than a scoped increment, and
nothing can be scoped from it until this is ruled.

Downstream and waiting: real terminal use, which is the actual motivation. Copying a path from one
pane and an error message from a build is the same *would someone live in this* axis milestone 142
(a text display good enough) names for the display half. The sources are the terminal (142) and a
pointer for drag-to-select (179). The paste target is anything a shell or an editor accepts text
into.
