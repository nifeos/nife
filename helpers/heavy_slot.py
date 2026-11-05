#!/usr/bin/env python3
"""Run a heavy job only when one of the machine's heavy-job slots is free.

    python3 helpers/heavy_slot.py <job> -- <command> [args...]
    python3 helpers/heavy_slot.py --held      # exit 0 when an ancestor of the caller holds a slot
    python3 helpers/heavy_slot.py --status
    python3 helpers/heavy_slot.py --pool kani --units <n> <job> -- <command> [args...]
    python3 helpers/heavy_slot.py --pool kani --size | --weight | --held

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
Two settings override it, the environment first. NIFE_HEAVY_SLOTS=<n> sets it for one process tree.
A per-machine setting is the file `heavy-slot-count` beside the slot directory (default
~/.nife/heavy-slot-count), holding one integer: `echo 10 > ~/.nife/heavy-slot-count`. It lives
outside every worktree, so every new lane and script reads it with no setup, and a host that does
not write it keeps the memory-based default. Remove the file to go back.

Two processes given different N each count only the slots they know. The file is read on every
call, and a waiter reads it again on every poll, so a change reaches a waiting job within
POLL_SECONDS and every job started later at once. A job that already holds a slot keeps it and is
unaffected. A process with NIFE_HEAVY_SLOTS exported keeps that value (the environment wins), so a
running lane that exported it must unset it, or the shell that started it must be restarted. Raise
N, not lower it, while jobs run: slots above a lowered N still count as held for the processes that
know them, and as free for the ones that do not.

A waiter prints a hint when a holder looks stuck: its process tree has used under 1 percent of the
time it has run in CPU (ps counts only live processes, so a job that ran many short children can
read low; the hint says "may"). `--status` marks a slot whose worktree path no longer exists as
orphaned, the shape a lint left by a pruned worktree takes.

The Kani pool. A heavy slot is one equal share, and a Kani run is not one share: `script/verify`
runs VERIFY_JOBS harnesses at once, and a harness is large and uneven. The 2026-11 host out-of-memory
dumps found three or four cbmc processes alive together at 22.2, 20.7, 7.5 and 3.4 GiB resident, so
one proof reaches 20 GiB and more, far past the 3.5 GiB that notes/verification/glob-proofs.md
records for glob. So Kani memory has its own pool beside the slots, in units of UNIT_GIB (2 GiB). The
pool is physical memory less KANI_RESERVE_GIB (12 GiB, for builds, QEMU and the system). A harness
weighs HARNESS_GIB (10 GiB by default, conservative: two of the largest proofs fit in the 44 GiB
pool of a 56 GiB host, and a run at VERIFY_JOBS=4 takes 40 of those GiB). A run takes the weight of
every harness it runs at once, all together or none, so it never holds some units while it waits for
the rest and two big runs cannot deadlock. The units are flock files in `kani/` under the slot
directory, taken as the slots are, so a killed run frees them with no cleanup. A request above the
pool is clamped to it, and the caller runs only as many harnesses at once as fit (`--size`).

  NIFE_KANI_HARNESS_GIB   the weight of one harness; lower it for a group of small proofs
  NIFE_KANI_UNITS         the pool size outright
  NIFE_VERIFY_MEMORY_BYTES  replaces physical memory in the sizing (a selftest picks a small machine)
  NIFE_KANI_POOL=off      no pool. It is also off under GITHUB_ACTIONS, where a runner is one job on
                          one machine sized by its workflow.

Python callers use `take_kani`, which holds the units until the process exits. `--status` lists every
holder with its lane, worktree and command line.

BUGS: a request for many units can be overtaken by smaller ones that fit sooner. The slots wait in
ticket order (below); the Kani units do not yet.

BUGS: the ancestor walk includes pid 1, which `script/test` is when it is a container's first
process. Two containers that share one slot directory and each run a heavy script as pid 1 cannot
tell their pid 1s apart, so one can run inside the other's slot. Give each container its own
NIFE_HEAVY_SLOT_DIR.

Order. A job waits for a slot in ticket order, through helpers/heavy_ledger.py: the job that began
waiting first starts first, and a slot freed while several wait goes to the oldest. The ledger counts
memory in bytes, every job asks for 8 GiB, and the capacity is the slot count times 8 GiB, so it
admits exactly as many jobs as there are slots. An admitted job still locks a slot file, so a helper
from before the ledger counts it, and a slot held without a ledger entry counts as one job.
"""

import errno
import fcntl
import os
import subprocess
import sys
import time

import heavy_ledger

GIB = 1 << 30
GIB_PER_SLOT = 8
POLL_SECONDS = 2
STUCK_SECONDS = 3600
UNIT_GIB = 2
KANI_RESERVE_GIB = 12
HARNESS_GIB = 10


def count_file():
    """The per-machine slot count file: `heavy-slot-count` beside the slot directory."""
    base = os.environ.get("NIFE_HEAVY_SLOT_DIR") or os.path.expanduser("~/.nife/heavy-slots")
    return os.path.join(os.path.dirname(os.path.abspath(base)), "heavy-slot-count")


def slot_count():
    override, source = os.environ.get("NIFE_HEAVY_SLOTS", ""), "NIFE_HEAVY_SLOTS"
    if not override and os.path.exists(count_file()):
        override, source = open(count_file()).read().strip(), count_file()
    if override:
        try:
            n = int(override)
        except ValueError:
            n = 0
        if n < 1:
            sys.exit(f"heavy-slot: {source}={override!r} must be an integer of at least 1")
        return n
    memory = os.sysconf("SC_PHYS_PAGES") * os.sysconf("SC_PAGE_SIZE")
    return max(1, memory // (GIB_PER_SLOT * GIB))


def slot_dir(pool=None):
    path = os.environ.get("NIFE_HEAVY_SLOT_DIR") or os.path.expanduser("~/.nife/heavy-slots")
    if pool:
        path = os.path.join(path, pool)
    os.makedirs(path, exist_ok=True)
    return path


def pool_size(pool):
    """Units in `pool`, or 0 when the pool is off. Only the kani pool exists."""
    if pool != "kani":
        sys.exit(f"heavy-slot: no pool named {pool!r} (the only one is kani)")
    if os.environ.get("GITHUB_ACTIONS") or os.environ.get("NIFE_KANI_POOL") == "off":
        return 0
    override = os.environ.get("NIFE_KANI_UNITS", "")
    if override:
        n = int(override)
        if n < 1:
            sys.exit(f"heavy-slot: NIFE_KANI_UNITS={override} must be at least 1")
        return n
    memory = int(os.environ.get("NIFE_VERIFY_MEMORY_BYTES") or
                 os.sysconf("SC_PHYS_PAGES") * os.sysconf("SC_PAGE_SIZE"))
    usable = memory / GIB - KANI_RESERVE_GIB
    return max(1, int(usable // UNIT_GIB))


def harness_units():
    """Units one harness weighs: HARNESS_GIB, or NIFE_KANI_HARNESS_GIB, rounded up."""
    gib = float(os.environ.get("NIFE_KANI_HARNESS_GIB") or HARNESS_GIB)
    if gib <= 0:
        sys.exit("heavy-slot: NIFE_KANI_HARNESS_GIB must be positive")
    return max(1, -(-int(gib * 1024) // int(UNIT_GIB * 1024)))


def kani_capacity(n):
    """Harnesses that fit in a pool of `n` units at once; at least one."""
    return max(1, n // harness_units())


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


def field(record, name):
    """One `name=value` field of a slot record; `cmd` is last and runs to the end of the line."""
    if name == "cmd":
        return record.split(" cmd=", 1)[1] if " cmd=" in record else ""
    for f in record.split(" cmd=")[0].split():
        if f.startswith(name + "="):
            return f[len(name) + 1:]
    return ""


def lane_of(worktree):
    """The branch checked out in the holder's worktree, or '' when it is gone."""
    if not worktree or not os.path.isdir(worktree):
        return ""
    out = subprocess.run(["git", "-C", worktree, "rev-parse", "--abbrev-ref", "HEAD"],
                         capture_output=True, text=True)
    return out.stdout.strip() if out.returncode == 0 else ""


def live_command(pid):
    out = subprocess.run(["ps", "-o", "args=", "-p", str(pid)], capture_output=True, text=True)
    return out.stdout.strip()


def describe(held):
    """One block per held slot: the record, then who owns it (lane, worktree) and what runs."""
    lines = []
    seen = set()
    for i, record in sorted(held.items()):
        pid = holder_pid(record)
        alive = pid > 0 and pid_alive(pid)
        note = "" if alive else " (that pid has exited; a process it started still holds the slot)"
        if record and record in seen:
            lines.append(f"  slot {i}: same holder as above")
            continue
        seen.add(record)
        lines.append(f"  slot {i}: {record.split(' cmd=')[0] or 'no record yet'}{note}")
        worktree = field(record, "worktree")
        lane = lane_of(worktree)
        if worktree and not os.path.isdir(worktree):
            lines.append(f"    ORPHANED: worktree {worktree} no longer exists")
        elif lane or worktree:
            lines.append(f"    lane {lane or '(unknown)'}  worktree {worktree}")
        cmd = (live_command(pid) if alive else "") or field(record, "cmd")
        if cmd:
            lines.append(f"    command {cmd}")
    return "\n".join(lines)


def to_seconds(text):
    """Seconds in a ps time such as `[[dd-]hh:]mm:ss[.ss]`; 0 when it does not parse."""
    try:
        days, _, rest = text.rpartition("-")
        total = 0.0
        for part in rest.split(":"):
            total = total * 60 + float(part)
        return total + 86400 * int(days or 0)
    except ValueError:
        return 0.0


def stuck_hint(held, table=None):
    """A line naming holders that may be stuck, or ''. A holder is flagged when its process tree has
    used under 1 percent CPU over more than STUCK_SECONDS of running. `table` is test input:
    {pid: (ppid, cpu_seconds, elapsed_seconds)}."""
    if table is None:
        table = {}
        out = subprocess.run(["ps", "-A", "-o", "pid=,ppid=,time=,etime="],
                             capture_output=True, text=True).stdout
        for line in out.split("\n"):
            f = line.split()
            if len(f) == 4 and f[0].isdigit() and f[1].isdigit():
                table[int(f[0])] = (int(f[1]), to_seconds(f[2]), to_seconds(f[3]))
    names = []
    for i, record in sorted(held.items()):
        pid = holder_pid(record)
        if pid not in table:
            continue
        tree, grew = {pid}, True
        while grew:
            grew = False
            for p, (parent, _, _) in table.items():
                if parent in tree and p not in tree:
                    tree.add(p)
                    grew = True
        elapsed = table[pid][2]
        if elapsed > STUCK_SECONDS and sum(table[p][1] for p in tree) < 0.01 * elapsed:
            names.append(f"slot {i} (pid {pid})")
    if not names:
        return ""
    return ("heavy-slot: " + ", ".join(names) + " may be stuck: under 1% CPU for over "
            f"{STUCK_SECONDS // 3600} hour. Check `--status`; kill the process if it is an orphan.")


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


def acquire_units(job, n, directory, units):
    """Block until `units` slots are locked at once, and return their fds. All or none."""
    units = min(units, n)
    announced = False
    while True:
        mine, held = try_slots(n, directory)
        if len(mine) >= units:
            keep = sorted(mine)[:units]
            for i, fd in mine.items():
                if i not in keep:
                    os.close(fd)
            for i in keep:
                record(mine[i], i, job, units)
            return [mine[i] for i in keep]
        for fd in mine.values():
            os.close(fd)
        if not announced:
            print(f"heavy-slot: {job} is waiting for {units} of {n} Kani units "
                  f"({directory}, {UNIT_GIB} GiB each); the rest are held by:\n{describe(held)}",
                  file=sys.stderr, flush=True)
            announced = True
        time.sleep(POLL_SECONDS)


def take_kani(harnesses, job):
    """In-process: hold the units of `harnesses` running at once until this process exits.

    Returns the units held, or 0 when an ancestor holds some or the pool is off. The caller runs no
    more than kani_capacity() harnesses at once."""
    n = pool_size("kani")
    if n == 0:
        return 0
    directory = slot_dir("kani")
    if held_by_ancestor(n, directory):
        return 0
    fds = acquire_units(job, n, directory, harnesses * harness_units())
    for fd in fds:
        os.set_inheritable(fd, True)
    return len(fds)


def acquire(job, n, directory):
    """Block until the ledger admits this job to one of `n` slots, and return the fds it holds: the
    slot's and its ledger entry's (helpers/heavy_ledger.py). The slot count is read again on every
    poll."""
    def waiting(held, ahead):
        line = f"heavy-slot: {job} is waiting for one of {n} heavy-job slots ({directory})"
        if ahead:
            line += f", behind {ahead} earlier in line"
        if held:
            line += f"; they are held by:\n{describe(held)}"
        print(line, file=sys.stderr, flush=True)
        hint = stuck_hint(held)
        if hint:
            print(hint, file=sys.stderr, flush=True)
    worktree = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    return heavy_ledger.acquire(job, heavy_ledger.SLOT_BYTES, slot_count, directory, try_slots,
                                record, waiting, POLL_SECONDS, worktree, sys.argv)


def record(fd, slot, job, units=1, command=None):
    worktree = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    started = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    cmd = " ".join(command or sys.argv).replace("\n", " ")
    line = f"pid={os.getpid()} job={job} units={units} worktree={worktree} started={started} cmd={cmd}\n"
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
    waiting = heavy_ledger.queue_lines(directory)
    if waiting:
        print(f"{len(waiting)} waiting, in the order they start:")
        print("\n".join(waiting))
    k = pool_size("kani")
    if k == 0:
        print("Kani pool: off")
        return
    kdir = slot_dir("kani")
    mine, kheld = try_slots(k, kdir)
    for fd in mine.values():
        os.close(fd)
    print(f"{k} Kani units of {UNIT_GIB} GiB in {kdir}, {len(kheld)} held; a harness weighs "
          f"{harness_units()} units, so {kani_capacity(k)} fit at once ({k * UNIT_GIB:.0f} GiB in all)")
    if kheld:
        print(describe(kheld))


def pool_main(argv):
    """`--pool kani` forms; argv is what follows the flag's value."""
    n = pool_size("kani")
    if argv == ["--size"]:
        print(kani_capacity(n) if n else "off")
        return
    if argv == ["--weight"]:
        print(harness_units())
        return
    if argv == ["--held"]:
        sys.exit(0 if n == 0 or held_by_ancestor(n, slot_dir("kani")) else 1)
    if len(argv) >= 4 and argv[0] == "--units" and argv[2:4] != [] and "--" in argv:
        units = int(argv[1])
        sep = argv.index("--")
        job, command = argv[2], argv[sep + 1:]
        if n:
            for fd in acquire_units(job, n, slot_dir("kani"), units):
                os.set_inheritable(fd, True)
        os.execvp(command[0], command)
    sys.exit("usage: heavy_slot.py --pool kani (--size | --held | --units <n> <job> -- <command>...)")


def main(argv):
    if argv == ["--status"]:
        status()
        return
    if argv[:2] == ["--pool", "kani"]:
        pool_main(argv[2:])
        return
    if argv == ["--held"]:
        sys.exit(0 if held_by_ancestor(slot_count(), slot_dir()) else 1)
    if len(argv) < 3 or argv[1] != "--":
        sys.exit("usage: heavy_slot.py <job> -- <command> [args...] | --held | --status")
    job, command = argv[0], argv[2:]
    for fd in acquire(job, slot_count(), slot_dir()):
        os.set_inheritable(fd, True)
    os.execvp(command[0], command)


if __name__ == "__main__":
    main(sys.argv[1:])
