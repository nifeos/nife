#!/usr/bin/env python3
"""Heavy jobs wait in ticket order for memory in a machine-wide ledger.

    python3 helpers/heavy_ledger.py --selftest

helpers/heavy_slot.py calls `acquire` to take a heavy-job slot; nothing else calls this directly.

Name: provisional, minted 2026-11-05 (UTC) by lane/heavy-ledger-in-ticket-order; calef has not
ruled on it. So are the `ledger/` directory under the slot directory and NIFE_HEAVY_AGING_SECONDS.

What it is. Step 1 of design/roadmap/1044-heavy-jobs-wait-in-order-for-measured-memory.md: the
ledger that later steps fill with measured weights. A job asks for a number of bytes and waits in
ticket order until the bytes fit. The machine's capacity is its slot count times SLOT_BYTES, and
every job asks for SLOT_BYTES, so the ledger admits as many jobs as the slots do. What it adds is
the order: a job that started waiting first starts first.

The files, all under `ledger/` in the slot directory:

  ledger.lock      a short lock every decision is made under
  next-ticket      the counter tickets come from
  queue/<ticket>   one per waiter, locked with flock(2) by the waiter while it waits
  holders/<ticket> one per admitted job, locked by the job (and inherited across exec) while it runs

The kernel keeps the ledger honest as it does the slots. A file whose lock nobody holds belongs to a
process that has exited, so the next decision removes it, with no pid to guess at and no cleanup
step. A record line has the same `key=value` fields as a slot record, `cmd=` last.

The decision. Each waiter, on each poll, takes the ledger lock and walks the queue in ticket order.
A waiter that fits in the free bytes, with an old slot free for it, may start, and its bytes count
as taken for the waiters behind it. A waiter that does not fit is passed over, so a smaller job can
start beside it, until it has waited AGING_SECONDS; then nobody behind it starts until it does.
Every waiter computes the same walk and admits only itself. A request above the capacity is clamped
to it, so every request can start once the machine is empty.

The old slots. Lanes cut before the ledger keep running the old helpers/heavy_slot.py for days. So
an admitted job also locks one old slot file and writes the old record in it, which is what an old
waiter counts and what `--held` and `--status` read. And a slot held by a process with no ledger
entry counts as a holder of SLOT_BYTES here. Both sides therefore see every job.

BUGS: an old-code waiter takes a free slot without a ticket, so it can still overtake a ledger
waiter.

BUGS: the capacity is the caller's slot count, read per process, so two processes with different
slot counts still decide against different capacities. Step 8 of the proposal moves the capacity
into one machine-local file.
"""

import fcntl
import os
import sys
import time

GIB = 1 << 30
SLOT_BYTES = 8 * GIB
AGING_SECONDS = 600
TICKET_DIGITS = 12


def aging_seconds():
    """AGING_SECONDS, or NIFE_HEAVY_AGING_SECONDS (a selftest makes it short)."""
    return float(os.environ.get("NIFE_HEAVY_AGING_SECONDS") or AGING_SECONDS)


def directories(slot_directory):
    """The ledger directory under `slot_directory`, with its queue and holders made."""
    base = os.path.join(slot_directory, "ledger")
    for sub in ("queue", "holders"):
        os.makedirs(os.path.join(base, sub), exist_ok=True)
    return base


def lock(base):
    """Take the ledger lock; closing the returned fd releases it."""
    fd = os.open(os.path.join(base, "ledger.lock"), os.O_RDWR | os.O_CREAT, 0o644)
    fcntl.flock(fd, fcntl.LOCK_EX)
    return fd


def next_ticket(base):
    """The next ticket, under the ledger lock."""
    path = os.path.join(base, "next-ticket")
    try:
        ticket = int(open(path).read().strip() or 0)
    except FileNotFoundError:
        ticket = 0
    with open(path + ".new", "w") as f:
        f.write(f"{ticket + 1}\n")
    os.replace(path + ".new", path)
    return ticket


def write_record(fd, fields, command):
    line = " ".join(f"{k}={v}" for k, v in fields.items())
    line += " cmd=" + " ".join(command).replace("\n", " ") + "\n"
    os.ftruncate(fd, 0)
    os.lseek(fd, 0, os.SEEK_SET)
    os.write(fd, line.encode())


def parse(text):
    """{field: value} from one record line; `cmd` runs to the end of the line."""
    head, _, cmd = text.strip().partition(" cmd=")
    fields = dict(f.split("=", 1) for f in head.split() if "=" in f)
    fields["cmd"] = cmd
    return fields


def entry(base, sub, ticket, fields, command):
    """Create `sub/<ticket>`, lock it, write its record and return the fd (under the ledger lock)."""
    path = os.path.join(base, sub, f"{ticket:0{TICKET_DIGITS}d}")
    fd = os.open(path, os.O_RDWR | os.O_CREAT, 0o644)
    fcntl.flock(fd, fcntl.LOCK_EX)
    write_record(fd, dict(ticket=ticket, **fields), command)
    return fd


def live(base, sub):
    """The records in `sub` whose owner still holds its lock, oldest ticket first. A file nobody
    holds is removed: its owner has exited. Called under the ledger lock."""
    out = []
    directory = os.path.join(base, sub)
    for name in sorted(os.listdir(directory)):
        path = os.path.join(directory, name)
        try:
            fd = os.open(path, os.O_RDWR)
        except FileNotFoundError:
            continue
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            record = parse(os.read(fd, 4096).decode(errors="replace"))
            if "ticket" in record:
                out.append(record)
        else:
            os.unlink(path)
        finally:
            os.close(fd)
    return out


def admissible(waiters, free_bytes, free_slots, now, aging):
    """The tickets that may start now, in order.

    `waiters` is the queue in ticket order, each a record with `ticket`, `bytes` and `since` (epoch
    seconds it joined the queue). Each admitted waiter takes its bytes and one slot from what the
    waiters behind it see. A waiter that does not fit is passed over until it has waited `aging`
    seconds; then the walk stops there, so nobody behind it starts first."""
    out = []
    for w in waiters:
        need = int(w["bytes"])
        if need <= free_bytes and free_slots > 0:
            out.append(int(w["ticket"]))
            free_bytes -= need
            free_slots -= 1
        elif now - float(w["since"]) >= aging:
            break
    return out


def free_space(capacity, holders, held_slots):
    """Bytes not reserved: the capacity less every ledger holder's bytes, less SLOT_BYTES for each
    held old slot that no ledger holder owns (a job started by a helper from before the ledger)."""
    owned = {h.get("slot") for h in holders}
    legacy = sum(1 for i in held_slots if str(i) not in owned)
    return capacity - sum(int(h["bytes"]) for h in holders) - legacy * SLOT_BYTES


def acquire(job, request, slot_count, slot_directory, try_slots, record, waiting, poll, worktree,
            command):
    """Block until this job is admitted, and return the fds it holds (its old slot and its ledger
    entry). The caller makes them inheritable and execs the job.

    `slot_count()` is read on every poll. `try_slots(n, directory)` and `record(fd, slot, job)` are
    helpers/heavy_slot.py's. `waiting(held, ahead)` prints the one waiting message, given the old
    slots' records and how many tickets wait ahead of this one."""
    base = directories(slot_directory)
    held_lock = lock(base)
    try:
        ticket = next_ticket(base)
        mine = entry(base, "queue", ticket,
                     dict(pid=os.getpid(), job=job, bytes=request, worktree=worktree,
                          since=f"{time.time():.3f}"), command)
    finally:
        os.close(held_lock)
    announced = False
    while True:
        n = slot_count()
        capacity = n * SLOT_BYTES
        held_lock = lock(base)
        try:
            free, held = try_slots(n, slot_directory)
            holders = live(base, "holders")
            waiters = live(base, "queue")
            for w in waiters:
                w["bytes"] = min(int(w["bytes"]), capacity)
            space = free_space(capacity, holders, held)
            if ticket in admissible(waiters, space, len(free), time.time(), aging_seconds()):
                slot = min(free)
                for i, fd in free.items():
                    if i != slot:
                        os.close(fd)
                record(free[slot], slot, job)
                admitted = entry(base, "holders", ticket,
                                 dict(pid=os.getpid(), job=job, bytes=min(request, capacity),
                                      slot=slot, worktree=worktree,
                                      since=f"{time.time():.3f}"), command)
                os.unlink(os.path.join(base, "queue", f"{ticket:0{TICKET_DIGITS}d}"))
                os.close(mine)
                return [free[slot], admitted]
            for fd in free.values():
                os.close(fd)
            ahead = sum(1 for w in waiters if int(w["ticket"]) < ticket)
        finally:
            os.close(held_lock)
        if not announced:
            waiting(held, ahead)
            announced = True
        time.sleep(poll)


def queue_lines(slot_directory):
    """`--status` lines for the waiters, oldest ticket first; empty when nobody waits."""
    base = directories(slot_directory)
    held_lock = lock(base)
    try:
        waiters = live(base, "queue")
    finally:
        os.close(held_lock)
    now = time.time()
    return [f"  ticket {w['ticket']}: {w.get('job', '?')} pid {w.get('pid', '?')}, waiting "
            f"{now - float(w['since']):.0f} s, worktree {w.get('worktree', '?')}" for w in waiters]


def selftest():
    """The decision against fixtures: no processes, no files, milliseconds."""
    s = SLOT_BYTES

    def w(ticket, size, since=100.0):
        return {"ticket": ticket, "bytes": size, "since": since}

    def expect(what, got, want):
        if got != want:
            sys.exit(f"heavy_ledger selftest: {what}: got {got}, want {want}")

    expect("first come, first served", admissible([w(1, s), w(2, s), w(3, s)], s, 1, 101, 600), [1])
    expect("each admission takes its bytes from those behind it",
           admissible([w(1, s), w(2, s), w(3, s)], 2 * s, 3, 101, 600), [1, 2])
    expect("each admission takes a slot from those behind it",
           admissible([w(1, s), w(2, s)], 4 * s, 1, 101, 600), [1])
    expect("a young big waiter lets a small one start beside it",
           admissible([w(1, 4 * s), w(2, s)], 2 * s, 2, 101, 600), [2])
    expect("an aged big waiter holds the queue for itself",
           admissible([w(1, 4 * s), w(2, s)], 2 * s, 2, 701, 600), [])
    expect("an aged waiter that fits starts",
           admissible([w(1, 2 * s), w(2, s)], 2 * s, 2, 701, 600), [1])
    expect("nobody waits, nobody starts", admissible([], 2 * s, 2, 101, 600), [])
    expect("a held slot no ledger holder owns counts as one slot's bytes",
           free_space(3 * s, [{"bytes": s, "slot": "0"}], [0, 1]), s)
    expect("a ledger holder's own slot is not counted twice",
           free_space(3 * s, [{"bytes": s, "slot": "0"}], [0]), 2 * s)
    expect("a record parses, its command last",
           parse("ticket=7 pid=1 job=lint bytes=8 cmd=sh -c a=b c"),
           {"ticket": "7", "pid": "1", "job": "lint", "bytes": "8", "cmd": "sh -c a=b c"})
    print("heavy_ledger selftest: ok")


if __name__ == "__main__":
    if sys.argv[1:] == ["--selftest"]:
        selftest()
    else:
        sys.exit("usage: heavy_ledger.py --selftest")
