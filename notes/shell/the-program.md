# The shell: what the program holds, and its known gaps

*An appendix to [`notes/shell.md`](../shell.md), which is the page to read. It holds design detail
from `components/src/swish/` that the code's comments link here rather than carry. The directory and
the stem are provisional names; naming is calef's.*

## Granting only from what it holds

The shell grants nothing it does not hold. `prog --mem N` splits N pages from its own budget at slot
3. A bare name in a file position resolves only if the progenitor granted the shell a directory
(`DIR_TERMINAL`). There is no `run` verb and no `file:` prefix: a bare name spawns, and the manifest
declares which direction a file token goes ([grant-expression.md](../grant-expression.md)).

`caps` prints the shell's whole endowment. `caps <command>` previews exactly what a command would get,
including the one authority no token designates: the clock a manifest such as `date`'s declares.

## Walking a path across two trees

A path is resolved as a path before anything is opened, and then opened one `OPENDIR` at a time from
the root of the tree that serves it. That is milestone 154's one-tree presentation in one place. `..`
from a mount point is its parent in the first tree, because that is what the path says, and a path
through the mount point is opened on the second tree's endpoint and no other. A bound name misdirects
a path and cannot manufacture a boundary, since `..` past it is the same arithmetic a direct walk
would do. A position inside a mounted tree is never deeper than the path it came from, so the walk's
fixed capacity is never at risk.

## The known gaps, at length

The code keeps one line per entry in its `BUGS` sections. The longer account:

- A child holding the result endpoint can send `SPAWN_FAILED`, `SPAWN_NO_DISPLAY` or a wrong exit
  status about itself, though never about another job. Every `RECEIVE` returns the sender's badge in
  `x3`, so a badged copy of the endpoint per sender would close this at no new authority.
- Only a supervised job can run in the background. It is the one kind of job the shell holds
  capabilities for: the region it was built from, its shared frame and its TCB. Every other program
  is built in the progenitor's pool, answers on the one result rendezvous every job shares, and is
  reaped by `job_undertaker`. The shell would hold nothing to wait on or stop it with, and its
  answer would be read as the next command's.
- `user suspend` does not send `login_contract::SUSPEND`, the word that ends a durable session at
  once, because the shell holds no capability to `login`'s front door. On the boot that runs this
  prompt that costs nothing yet: `login` is started there with no schedule archive, so it holds no
  durable session to end. When it does, the shell needs `WRITE` on that front door beside its root.
  Until then the session ends at the suspended identity's next login attempt.
