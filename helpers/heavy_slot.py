#!/usr/bin/env python3
"""Run a heavy job only when one of the machine's heavy-job slots is free.

    python3 helpers/heavy_slot.py <job> -- <command> [args...]
    python3 helpers/heavy_slot.py --held      # exit 0 when an ancestor of the caller holds a slot
    python3 helpers/heavy_slot.py --status

A script does not call this directly. It sources helpers/heavy-slot.sh, which re-executes the script
under this. <job> names the job in what a waiter prints (`lint`, `test`, `mutation`).

Name: provisional, minted 2026-10-19 (UTC) by lane/heavy-job-slots; calef has not ruled on it. So
are NIFE_HEAVY_SLOTS and NIFE_HEAVY_SLOT_DIR.

Why this exists. On 2026-10-19 (UTC) calef's 16 GB Mac hung twice with no kernel panic and had to be
forced off, while several lanes ran full `script/lint` runs, a mutation run, cold builds and QEMU
suites at once. Each lane's own discipline was fine; nothing bounded their sum. calef approved a
machine-wide cap the same day. This is that cap: at most N heavy jobs run on the machine, however
many lanes and worktrees exist.

The mechanism. N slot files under NIFE_HEAVY_SLOT_DIR (default ~/.nife/heavy-slots, outside every
worktree so all of them contend for the same ones). A job holds a slot by holding flock(2) on its
file, and the locked descriptor is inherited across exec into the job itself. The kernel drops a
flock when the last descriptor on it closes, so a job that crashes or is killed with SIGKILL frees
its slot with no cleanup step; there is no pid file to go stale. A mkdir lock with a pid check (the
other shape this needs on macOS, which has no flock(1)) has to guess whether a recorded pid is still
its holder, and that guess is what this avoids.

The descriptor is inherited by everything the job starts, so a child that outlives the job (a QEMU a
wrapper failed to kill) keeps the slot until it exits. That is deliberate: the machine is still
carrying that load. A waiter reports it as such.

Nesting. `script/ci-build` runs `script/test`; the pre-push hook runs `script/lint`. The inner job
must not take a second slot, and with N = 1 it must not wait for the slot its own parent holds, or it
deadlocks. So `--held`, which helpers/heavy-slot.sh asks before taking a slot, walks the caller's
ancestors and answers yes if any of them is the recorded holder of a slot that is in fact held; the
caller then runs inside that slot. That survives an environment the parent scrubbed, which an
environment-variable marker would not.

The default N is one slot per 8 GiB of physical memory, at least 1, so a 16 GiB machine runs two.
NIFE_HEAVY_SLOTS overrides it. Two processes given different N each count only the slots they know.

BUGS: the ancestor walk includes pid 1, which `script/test` is when it is a container's first
process. Two containers that share one slot directory and each run a heavy script as pid 1 cannot
tell their pid 1s apart, so one can run inside the other's slot. Give each container its own
NIFE_HEAVY_SLOT_DIR.

BUGS: waiters are not served in order. Each polls every POLL_SECONDS and the first to find a free
slot takes it, so a run can be overtaken by one that started later.
"""

import errno
import fcntl
import os
import subprocess
import sys
import time

GIB = 1 << 30
GIB_PER_SLOT = 8
POLL_SECONDS = 2


def slot_count():
    override = os.environ.get("NIFE_HEAVY_SLOTS", "")
    if override:
        n = int(override)
        if n < 1:
            sys.exit(f"heavy-slot: NIFE_HEAVY_SLOTS={override} must be at least 1")
        return n
    memory = os.sysconf("SC_PHYS_PAGES") * os.sysconf("SC_PAGE_SIZE")
    return max(1, memory // (GIB_PER_SLOT * GIB))


def slot_dir():
    path = os.environ.get("NIFE_HEAVY_SLOT_DIR") or os.path.expanduser("~/.nife/heavy-slots")
    os.makedirs(path, exist_ok=True)
    return path


def parent_table():
    """{pid: ppid} for every process, from one `ps` listing (no /proc on macOS)."""
    out = subprocess.run(["ps", "-A", "-o", "pid=,ppid="], capture_output=True, text=True).stdout
    parent = {}
    for line in out.split("\n"):
        fields = line.split()
        if len(fields) == 2:
            parent[int(fields[0])] = int(fields[1])
    return parent


def chain(parent, pid):
    """The pids from `pid` up to the root of `parent`, `pid` first. Pid 1 is included.

    Pid 1 is a real ancestor: when `script/test` is a container's first process, the holder the
    slot records is pid 1, and a walk that stopped before it never found the holder in its own
    family, so the script waited for its own lock for ever.
    """
    seen = []
    while pid > 0 and pid not in seen:
        seen.append(pid)
        pid = parent.get(pid, 0)
    return seen


def ancestors():
    """This process's ancestor pids (its parent first, pid 1 included)."""
    return set(chain(parent_table(), os.getppid()))


def read_holder(fd):
    os.lseek(fd, 0, os.SEEK_SET)
    return os.read(fd, 4096).decode(errors="replace").strip()


def holder_pid(record):
    for field in record.split():
        if field.startswith("pid="):
            try:
                return int(field[4:])
            except ValueError:
                return 0
    return 0


def try_slots(n, directory):
    """Try every slot once. Returns (fds we locked, {slot: record} for those held by others)."""
    mine, held = {}, {}
    for i in range(n):
        fd = os.open(os.path.join(directory, f"slot-{i}"), os.O_RDWR | os.O_CREAT, 0o644)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as e:
            if e.errno not in (errno.EWOULDBLOCK, errno.EAGAIN):
                raise
            held[i] = read_holder(fd)
            os.close(fd)
            continue
        mine[i] = fd
    return mine, held


def describe(held):
    lines = []
    for i, record in sorted(held.items()):
        pid = holder_pid(record)
        alive = pid > 0 and pid_alive(pid)
        note = "" if alive else " (that pid has exited; a process it started still holds the slot)"
        lines.append(f"  slot {i}: {record or 'no record yet'}{note}")
    return "\n".join(lines)


def pid_alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def held_by_ancestor(n, directory):
    mine, held = try_slots(n, directory)
    for fd in mine.values():
        os.close(fd)
    family = ancestors()
    return any(holder_pid(r) in family for r in held.values())


def acquire(job, n, directory):
    """Block until a slot is ours, and return its fd."""
    announced = False
    while True:
        mine, held = try_slots(n, directory)
        if mine:
            slot, fd = min(mine.items())
            for other in mine.values():
                if other != fd:
                    os.close(other)
            record(fd, slot, job)
            return fd
        if not announced:
            print(f"heavy-slot: {job} is waiting for one of {n} heavy-job slots "
                  f"({directory}); they are held by:\n{describe(held)}", file=sys.stderr, flush=True)
            announced = True
        time.sleep(POLL_SECONDS)


def record(fd, slot, job):
    worktree = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    started = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    line = f"pid={os.getpid()} job={job} worktree={worktree} started={started}\n"
    os.ftruncate(fd, 0)
    os.lseek(fd, 0, os.SEEK_SET)
    os.write(fd, line.encode())


def status():
    n, directory = slot_count(), slot_dir()
    mine, held = try_slots(n, directory)
    for fd in mine.values():
        os.close(fd)
    print(f"{n} heavy-job slots in {directory}, {len(held)} held")
    if held:
        print(describe(held))


def main(argv):
    if argv == ["--status"]:
        status()
        return
    if argv == ["--held"]:
        sys.exit(0 if held_by_ancestor(slot_count(), slot_dir()) else 1)
    if len(argv) < 3 or argv[1] != "--":
        sys.exit("usage: heavy_slot.py <job> -- <command> [args...] | --held | --status")
    job, command = argv[0], argv[2:]
    os.set_inheritable(acquire(job, slot_count(), slot_dir()), True)
    os.execvp(command[0], command)


if __name__ == "__main__":
    main(sys.argv[1:])
