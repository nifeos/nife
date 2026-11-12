# Prior art

An appendix to [the milestone](../1110-subtree-scope-outside-review.md).

How the crate sits against what a reader already knows. Recent downloads are the last 90 days at
the time of the review.

| where | the same problem | what it answers | how this crate differs |
|---|---|---|---|
| Linux `openat2(2)` | `RESOLVE_BENEATH`, `RESOLVE_NO_SYMLINKS`, `RESOLVE_NO_XDEV` | `EXDEV` on an escape or a mount crossing, `ELOOP` on a symbolic link | the same two errnos for a mount and a link; `..` is `EINVAL` everywhere, where `openat2` allows a `..` that stays beneath |
| FreeBSD `O_RESOLVE_BENEATH`, Capsicum | a path may not escape the directory it is opened from | a capability-mode errno (unverified which) | the same rule, decided in one crate a server links rather than in the kernel |
| OpenBSD `unveil(2)` | a process sees only the paths it was unveiled | `ENOENT` for a hidden path | `Refusal::NotFound` for a directory the grant may not walk into follows the same reasoning: a holder that may not look must not learn the name is there |
| Fuchsia `fuchsia.io` | directory rights only attenuate down the tree | rights are a subset of the parent's | the walk's `attenuate` is the same intersection; the refusal of a step that comes up short (`Narrowed`) is this crate's addition |
| seL4 badges | the server knows which client sent a request | a badge word on the endpoint | `Bindings<B>` is the table a server keeps per badge; seL4 leaves that table to the server too |
| `cap-std` 4.0 (16.3M recent / 32.5M all-time) and `cap-primitives` (17.9M / 38.5M) | a sandboxed `Dir` whose paths cannot escape | a manual component-wise walk with `..` tracked inside the sandbox; symbolic links followed when their target stays beneath | one step at a time, as here; this crate never follows a link and never resolves `..` |
| `openat` 0.1 (266k / 1.4M) | relative opens from a directory handle | no escape check | nothing of this crate's job |
| `path-clean` 1.0 (9.4M / 40.6M) | a lexical `..` cleanup | a string | the thing this crate refuses to do: a name is decided against the tree, never rewritten |

Where this crate differs from all of them: the decision is a pure function over a `lookup` the
caller supplies, so a server with any node type uses it, and the refusals are Kani-proved with a
falsification patch each. What it costs: the caller must route every path and every handle through
it (one line of the module doc says so, and nothing can check it). Also, a client that writes `a/../b`
meets a refusal where Linux would accept it.
