#!/bin/sh
#
# Run a command with a hard time limit, and actually kill it.
#
#     helpers/qemu-bounded.sh 10 qemu-system-aarch64 -machine virt ...
#
# # Why this exists
#
# The obvious trick, `perl -e 'alarm N; exec @ARGV' <cmd>`, **does not work on QEMU.**
# QEMU installs its own SIGALRM handler (it uses timers internally), so the alarm is
# swallowed and the process runs forever.
#
# We found this out the hard way: eleven abandoned QEMU processes accumulated over a day
# of development, burning a combined 729% CPU, the oldest with almost eight hours of CPU
# time on it. Every "bounded" run had leaked.
#
# QEMU *does* honour SIGTERM, so that's what we use: start the child, start a killer in
# the background, and make sure the killer dies with us.
#
# Note the stdin plumbing on the child: a backgrounded command's stdin is otherwise redirected to
# /dev/null by the shell (POSIX), which silently breaks piping input to QEMU's serial port.
# We found that the hard way trying to drive the milestone-10 shell from a pipe.
#
# **It goes through fd 3, because `<&0` only works in bash.** POSIX assigns /dev/null to an
# asynchronous list's stdin *before* its explicit redirections, so under a literal shell `<&0`
# duplicates /dev/null onto itself. bash (macOS's /bin/sh) special-cases an explicit stdin
# redirection and skips the /dev/null; dash (Debian and Ubuntu's /bin/sh, so every CI runner) does
# not. So on patagonia keystrokes reached QEMU and in CI they never did: `cargo xtask uefi-boot`
# typed `echo typed on the wire` at a prompt that was on the screen and got no echo, twice in a row
# on 2026-09-19 (pull request #985), while passing every local run. Checked directly with macOS's
# own /bin/dash: `printf 'hi\n' | dash -c 'cat <&0 & wait'` prints nothing, and the same through a
# saved descriptor, `exec 3<&0; cat <&3 3<&- & wait`, prints `hi`. The descriptor is saved before
# the list is backgrounded, so the /dev/null assignment cannot reach it, and closed in the child so
# QEMU does not inherit a stray fd.
#
# # The bound is not the only way this script's job ends (milestone 226)
#
# The original killer slept for the whole bound and then fired. That bounds a run which is
# *allowed to finish*; it does nothing about a run whose wrapper is killed. When a session
# dies, or a harness is killed with `pkill -f`, or somebody hits Ctrl-C, the wrapper goes
# and **QEMU is inherited by pid 1 and runs forever**. It then holds the write lock on a
# disk image, and the *next* run fails with QEMU's `Failed to get "write" lock`, which names
# a file and nothing about the process holding it. Milestone 127's EL2 lane lost time to
# that twice in one session before recognising the shape.
#
# So the killer now has two reasons to fire, and they are the two ways this run can be over:
#
# 1. **The bound expired.** The original job, unchanged in effect.
# 2. **The wrapper is gone.** The killer polls `kill -0` on the wrapper's pid once a second
#    and kills the child as soon as that fails. This is what covers a SIGKILLed wrapper, a
#    dead session, and a closed terminal, none of which run any trap anywhere.
#
# and it also fires on its own death:
#
# 3. **The killer itself was signalled** (SIGTERM or SIGHUP, which is what `pkill -f
#    qemu-bounded` and a hangup send). Its trap kills the child on the way out, so the one
#    process that knows the child's pid never takes that knowledge with it.
#
# **SIGINT is deliberately not in that trap list**, because listing it would be a lie: a
# shell puts an asynchronous subshell's SIGINT to ignore before the subshell can trap it,
# so the trap would never run. Ctrl-C is covered anyway, and better: the wrapper dies of it,
# the killer survives it, and reason 2 fires within a second.
#
# The parent traps too, which is not redundant with reason 2. It makes the ordinary Ctrl-C
# and `kill` cases immediate instead of costing up to a second, and it is the only path that
# can also stand the killer down rather than leaving it to notice.
#
# # BUGS
#
# **A SIGKILL to the killer defeats all of it, and nothing on macOS can fix that.** SIGKILL
# is not trappable, and macOS has no `prctl(PR_SET_PDEATHSIG)`, so a supervisor that is shot
# in the head cannot hand off. `kill -9` on a whole process group is the realistic way to hit
# this. That case is what the lock diagnostic below is for: it cannot prevent the orphan, but
# it makes the next run say who is holding the image instead of naming a file.
#
# **The parent-alive poll can be fooled by pid reuse.** If the wrapper dies abnormally and its
# pid is reused within the bound, the killer waits out the full bound instead of firing early,
# which is exactly the old behaviour and never worse than it. It cannot go the other way: a
# live wrapper's pid is not reused.
#
# **The poll costs a `sleep` and a `kill -0` per second**, so a very long bound spends a few
# hundred system_calls. That is not measurable next to an emulator.
#
# **The lock diagnostic only inspects arguments that look like disk images** (`*.img`,
# `*.qcow2`, `*.raw`, and any `file=` field of a comma-separated option), and only reports
# holders that have the file open for **writing**. A lock held on something named differently is
# not reported, and neither is a reader, which is deliberate: the message is about a write lock,
# and reporting readers made it fire on every green `script/test` that overlapped another lane,
# because the three legs share one read-only OVMF firmware file.

set -e

SECONDS_LIMIT="$1"
shift

# Report who has a disk image open. Called only after a failure, so the happy path pays
# nothing for it, and it prints nothing at all when there is nothing to say.
#
# This exists because QEMU's own message for the case names the file and not the holder,
# and `lsof` answers the real question in one call. It reports rather than kills: a process
# holding the image may be another lane's gate in flight, and AGENTS.md's rule is to walk the
# parent chain UP before deciding a QEMU is a leak (a maintainer killed a lane's mid-suite
# emulator by skipping that on 2026-08-15).
report_lock_holders() {
    command -v lsof >/dev/null 2>&1 || return 0
    for arg in "$@"; do
        # `-drive if=none,file=x.img,format=raw` hides the path in one field of one argument.
        rest="$arg"
        while [ -n "$rest" ]; do
            field="${rest%%,*}"
            case "$rest" in
                *,*) rest="${rest#*,}" ;;
                *) rest="" ;;
            esac
            case "$field" in
                file=*) path="${field#file=}" ;;
                *.img | *.qcow2 | *.raw) path="$field" ;;
                *) continue ;;
            esac
            [ -f "$path" ] || continue
            # Only holders with the file open for **writing**, which is what the lock is about.
            # Reporting every reader cries wolf on a green run: `script/test`'s three legs share
            # one read-only OVMF firmware file, so a run that overlaps another lane's found a
            # second QEMU on it every time, said so, and was wrong to. `lsof -F` is the machine
            # readable form: `p<pid>` starts a process, `a<mode>` gives the access mode of the
            # descriptor before it, and `u` or `w` there means somebody can write.
            holders="$(lsof -Fpfa -- "$path" 2>/dev/null |
                awk '/^p/ { pid = substr($0, 2) } /^a/ { if (substr($0, 2) ~ /[wu]/) print pid }' |
                sort -u | tr '\n' ' ')"
            [ -n "$holders" ] || continue
            echo "qemu-bounded.sh: $path is still open by another process:" >&2
            for h in $holders; do
                echo "qemu-bounded.sh:   $(ps -o pid=,ppid=,lstart=,comm= -p "$h" 2>/dev/null)" >&2
            done
            echo "qemu-bounded.sh: if the command above failed with 'Failed to get \"write\" lock', that is why." >&2
            echo "qemu-bounded.sh: walk the parent chain (ps -o pid,ppid,command) before killing: a QEMU whose" >&2
            echo "qemu-bounded.sh: parent is a live harness is somebody's gate, not a leak." >&2
        done
    done
}

# The child and everything beneath it, leaves included. Signalling only `$CHILD` bounds the process
# this script started, and that is QEMU only when the command execs it. `cargo xtask shell` is
# cargo, then xtask, then the runner that execs QEMU; `helpers/qemu-runner-x86_64.sh` is a shell
# with QEMU beneath it. In both, a TERM to the child alone ends the child and leaves QEMU running
# under pid 1 (found 2026-09-24: three bounded `cargo xtask shell` runs in a row, three orphans).
# So the list is taken before anything is signalled, because a killed parent re-parents its
# children to pid 1 and `pgrep -P` then cannot find them. `echo` before recursing, not after:
# the loop variable is global in sh, and the recursive call overwrites it.
descendants() {
    for d in $(pgrep -P "$1" 2>/dev/null); do
        echo "$d"
        descendants "$d"
    done
}
# shellcheck disable=SC2329 # called from the trap strings below, which shellcheck cannot see
signal_tree() {
    # shellcheck disable=SC2046
    kill -"$1" $(descendants "$2") "$2" 2>/dev/null
}

exec 3<&0
"$@" <&3 3<&- &
CHILD=$!
exec 3<&-
PARENT=$$

# The killer. Detached, so it survives even if the shell is in a pipeline whose reader
# (`head`, say) exits early. That exact case is what leaked processes before.
#
# Its output goes to /dev/null rather than to the pipeline, which is not tidiness either: a
# background subshell inherits the pipe's write end, and so does every `sleep` it leaves
# behind, so a reader waiting for EOF waits for them too. See the stand-down comment below,
# which is the same bug approached from the other side.
(
    # `wait` on a killed sleep returns non-zero, and the poll's whole job is to survive that.
    set +e
    SLEEPER=""
    # Signalled: kill the child on the way out. Nothing else knows its pid.
    trap 'kill "$SLEEPER" 2>/dev/null; signal_tree TERM "$CHILD"; exit 0' TERM HUP
    # Stood down by the parent, which means the child finished on its own.
    trap 'kill "$SLEEPER" 2>/dev/null; exit 0' USR1

    waited=0
    while [ "$waited" -lt "$SECONDS_LIMIT" ]; do
        # `sleep 1 &` then `wait`, not a plain `sleep`: a shell runs a trap only after the
        # foreground command it is in finishes, so a plain sleep would swallow the signal for
        # up to a second.
        sleep 1 &
        SLEEPER=$!
        wait "$SLEEPER"
        # Reason 2: the wrapper is gone, so this run is over whatever the bound said.
        kill -0 "$PARENT" 2>/dev/null || break
        waited=$((waited + 1))
    done

    TREE="$(descendants "$CHILD") $CHILD"
    # shellcheck disable=SC2086
    kill -TERM $TREE 2>/dev/null
    sleep 2
    # shellcheck disable=SC2086
    kill -KILL $TREE 2>/dev/null
    exit 0
) >/dev/null 2>&1 &
KILLER=$!

# Signalled while waiting: take the child down now rather than making the killer notice.
trap 'signal_tree TERM "$CHILD"; kill -TERM "$KILLER" 2>/dev/null; exit 143' TERM HUP INT

set +e
wait "$CHILD"
STATUS=$?
set -e

# Stand the killer down, now that the child has finished on its own.
#
# **USR1 first, and that is not tidiness.** A bare `kill "$KILLER"` sends TERM, whose trap
# above kills the child (harmless, it is already dead) but which historically ended the
# subshell and orphaned the `sleep` inside it, and that sleep kept running for the whole
# bound holding the pipe's write end, so a piped reader blocked until the bound expired
# however quickly the guest finished. Found on 2026-08-18 by milestone 38, whose Linux
# comparison runs a guest that powers itself off in about fifteen seconds and whose every
# round nevertheless took the full five minutes. Both traps now kill the sleeper by pid, and
# the killer's output no longer touches the pipe at all, so this is belt and braces; USR1
# still says the true thing, which is "you are not needed", not "die".
kill -USR1 "$KILLER" 2>/dev/null || true
pkill -P "$KILLER" 2>/dev/null || true
kill "$KILLER" 2>/dev/null || true

if [ "$STATUS" -ne 0 ]; then
    report_lock_holders "$@"
fi

exit "$STATUS"
