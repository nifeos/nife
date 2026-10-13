# A piped gate reports the pipe's status, not the gate's

An appendix to [`../scripts.md`](../scripts.md), which keeps the rule and the command. Name:
provisional; calef names files.

`script/lint | tail -30; echo $?` prints `tail`'s exit code. So does `| grep`, `| head`, and
every other filter used to make a long gate readable. The gate can fail and the shell will say `0`.

It is the worst kind of gate defect, because it fails in the safe-looking direction: a red gate
reporting green looks like a green one, and the point of `script/lint` is that nobody has to read
it.

Write it as a redirect, and read `$?` before anything else touches it:

```console
$ script/lint > /tmp/lint.txt 2>&1; echo "exit=$?"
exit=0
$ grep -iE "^error|PROBLEM" /tmp/lint.txt      # now filter, having already read the status
```

`set -o pipefail` fixes it, and `script/` entry points use it; it is off by default in an
interactive shell and most one-liners, which is where this bites.

Nothing gates this, and nothing could: the repository cannot inspect a shell pipeline. It is rung
four, recorded where somebody about to run a gate is already reading.
