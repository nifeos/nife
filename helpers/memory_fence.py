#!/usr/bin/env python3
"""Run a command inside a memory limit the host survives, and say in the live log what it used.

    python3 helpers/memory_fence.py [--reserve-mib N] [--interval S] -- <command>...
    python3 helpers/memory_fence.py --selftest

Name: provisional, minted 2026-11-16 (UTC) by milestone/1117-prove-shards-fit-the-runner; calef has
not ruled on it.

What it is for. A hosted runner has about 16 GB and no limit of its own below that. When the CBMC
processes of a prove shard grow past it, the kernel's out-of-memory killer takes whatever it picks,
the runner's own agent included, and GitHub reports "The runner has received a shutdown signal".
The job then sits until its 25-minute limit and is cancelled about five minutes after that, with no
memory record, because the step that printed one never ran. Milestone 1117 (the prove shards fit
the hosted runner's memory) found 51 merge-group shard jobs lost that way between 2026-11-14 and
2026-11-16 (UTC).

This puts the command in its own cgroup, capped at the host's MemTotal less a reserve (1536 MiB by
default) with no swap. A command that grows past the cap loses its largest process, which is a CBMC
proving one harness: Kani reports that harness as failed, the shard fails in minutes with its log,
and the runner lives. While the command runs, a line every --interval seconds (15 by default) gives
the cgroup's use, the host's available memory and each running CBMC with the harness it proves,
read from the goto file it was given. At the end it prints the cgroup's peak and the number of
out-of-memory kills, and for each kill it names the harness that was largest just before it, as a
GitHub error annotation. A kill makes the exit status 1 even if the command reported success.

Without a cgroup v2 memory controller or a password-free sudo, it runs the command bare, says so,
and still prints the samples.

BUGS: the harness a kill names is the CBMC that was largest at the last one-second tick before the
kill was counted. The kernel kills the largest process in the cgroup, so this is right unless a
second CBMC outgrew it inside that second.

BUGS: the reserve is a number, not a measurement of what the runner's own processes need. The
runner agent and its helpers used about 300 MiB in milestone 1117's samples; 1536 MiB leaves room
for them and the page cache.
"""

import os
import re
import signal
import subprocess
import sys
import tempfile
import threading
import time

MEBIBYTE = 1 << 20
CGROUP_ROOT = os.environ.get("NIFE_MEMORY_FENCE_ROOT", "/sys/fs/cgroup")
# The selftest points the root at a plain directory and writes it without sudo.
USE_SUDO = "NIFE_MEMORY_FENCE_ROOT" not in os.environ
MEMINFO = os.environ.get("NIFE_MEMORY_FENCE_MEMINFO", "/proc/meminfo")


def say(*parts):
    print("memory fence:", *parts, flush=True)


def meminfo_mib(field):
    try:
        with open(MEMINFO) as f:
            for line in f:
                if line.startswith(field + ":"):
                    return int(line.split()[1]) // 1024
    except OSError:
        pass
    return None


def harness_of(path):
    """`verification::two_live_asids_are_distinct` from a Kani goto file's name, or None.

    Kani names each harness's goto file `<crate>-<hash>_<v0 symbol>.out`. The v0 symbol is
    `_R` and its path tags (`Nv`, `Nt`), the crate root `Cs<disambiguator>_<len><crate>`, then one
    `<len><identifier>` per module and the harness itself; an identifier that starts with a digit
    or an underscore gets a `_` after its length.
    """
    base = os.path.basename(path)
    if not base.endswith(".out"):
        return None
    m = re.search(r"_R[NvtCs]*?C(?:s[0-9A-Za-z]*_)?", base)
    if not m:
        return None
    rest, idents = base[m.end():-len(".out")], []
    while rest:
        n = re.match(r"(\d+)_?", rest)
        if not n:
            return None
        size = int(n.group(1))
        ident = rest[n.end():n.end() + size]
        if len(ident) != size:
            return None
        idents.append(ident)
        rest = rest[n.end() + size:]
    if len(idents) < 2:
        return None
    return "::".join(idents[1:])


def read_int(path, default=None):
    try:
        with open(path) as f:
            return int(f.read().split()[0])
    except (OSError, ValueError, IndexError):
        return default


def read_events(cgroup):
    events = {}
    try:
        with open(os.path.join(cgroup, "memory.events")) as f:
            for line in f:
                key, _, value = line.partition(" ")
                events[key] = int(value)
    except (OSError, ValueError):
        pass
    return events


def cbmc_processes():
    """{pid: (harness, rss MiB)} for every running cbmc on the host."""
    found = {}
    for pid in os.listdir("/proc") if os.path.isdir("/proc") else ():
        if not pid.isdigit():
            continue
        try:
            with open("/proc/%s/comm" % pid) as f:
                if f.read().strip() != "cbmc":
                    continue
            with open("/proc/%s/cmdline" % pid, "rb") as f:
                argv = f.read().decode(errors="replace").split("\0")
            rss = 0
            with open("/proc/%s/status" % pid) as f:
                for line in f:
                    if line.startswith("VmRSS:"):
                        rss = int(line.split()[1]) // 1024
        except (OSError, ValueError):
            continue
        name = next((h for h in map(harness_of, argv) if h), None) or "cbmc %s" % pid
        found[int(pid)] = (name, rss)
    return found


def write(path, text):
    if USE_SUDO:
        subprocess.run(["sudo", "-n", "tee", path], input=text.encode(), stdout=subprocess.DEVNULL,
                       check=True)
    else:
        with open(path, "w") as f:
            f.write(text)


def make_fence(reserve_mib):
    """The path of a new capped cgroup and its cap in MiB, or (None, the reason there is none)."""
    # A selftest root is a plain directory, so the fixture fences on any host.
    if USE_SUDO and not sys.platform.startswith("linux"):
        return None, "not Linux"
    try:
        with open(os.path.join(CGROUP_ROOT, "cgroup.controllers")) as f:
            if "memory" not in f.read().split():
                return None, "no cgroup v2 memory controller"
    except OSError:
        return None, "no cgroup v2 hierarchy at " + CGROUP_ROOT
    total = meminfo_mib("MemTotal")
    if total is None:
        return None, "no MemTotal in " + MEMINFO
    if USE_SUDO and subprocess.run(["sudo", "-n", "true"], stderr=subprocess.DEVNULL).returncode:
        return None, "no password-free sudo to make a cgroup"
    cgroup = os.path.join(CGROUP_ROOT, "nife-memory-fence-%d" % os.getpid())
    cap = total - reserve_mib
    try:
        if USE_SUDO:
            subprocess.run(["sudo", "-n", "mkdir", cgroup], check=True)
        else:
            os.mkdir(cgroup)
        write(os.path.join(cgroup, "memory.max"), "%d\n" % (cap * MEBIBYTE))
        if USE_SUDO or os.path.exists(os.path.join(cgroup, "memory.swap.max")):
            write(os.path.join(cgroup, "memory.swap.max"), "0\n")
    except (OSError, subprocess.CalledProcessError) as e:
        return None, "could not make %s (%s)" % (cgroup, e)
    return (cgroup, cap), None


def remove_fence(cgroup):
    try:
        if os.path.exists(os.path.join(cgroup, "cgroup.kill")):
            write(os.path.join(cgroup, "cgroup.kill"), "1\n")
        if USE_SUDO:
            subprocess.run(["sudo", "-n", "rmdir", cgroup], stderr=subprocess.DEVNULL)
        else:
            subprocess.run(["rm", "-rf", cgroup])
    except (OSError, subprocess.CalledProcessError):
        pass


def run(command, reserve_mib, interval):
    fence, why = make_fence(reserve_mib)
    cgroup, cap = fence if fence else (None, None)
    if cgroup:
        say("%s holds the command, capped at %d MiB (MemTotal less %d) with no swap"
            % (os.path.basename(cgroup), cap, reserve_mib))
        join = "echo $$ | sudo -n tee \"$0\" >/dev/null && exec \"$@\"" if USE_SUDO else \
               "echo $$ > \"$0\" && exec \"$@\""
        argv = ["sh", "-c", join, os.path.join(cgroup, "cgroup.procs")] + command
    else:
        say("not fenced (%s); running the command bare" % why)
        argv = command
    child = subprocess.Popen(argv)

    def forward(signum, _frame):
        child.send_signal(signum)
    for s in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(s, forward)

    kills, largest, done = [], [None], threading.Event()
    seen = [0]

    def watch():
        last_print = time.monotonic()
        while not done.wait(1.0):
            procs = cbmc_processes()
            events = read_events(cgroup) if cgroup else {}
            if events.get("oom_kill", 0) > seen[0]:
                for _ in range(events["oom_kill"] - seen[0]):
                    kills.append(largest[0])
                seen[0] = events["oom_kill"]
            if procs:
                largest[0] = max(procs.values(), key=lambda p: p[1])
            if time.monotonic() - last_print >= interval:
                last_print = time.monotonic()
                used = read_int(os.path.join(cgroup, "memory.current")) if cgroup else None
                line = time.strftime("%H:%M:%S", time.gmtime())
                if used is not None:
                    line += " fence %d of %d MiB," % (used // MEBIBYTE, cap)
                line += " host available %s MiB" % meminfo_mib("MemAvailable")
                if procs:
                    line += "; " + ", ".join("%s %d MiB" % p for p in
                                             sorted(procs.values(), key=lambda p: -p[1]))
                say(line)
    watcher = threading.Thread(target=watch)
    watcher.start()
    status = child.wait()
    done.set()
    watcher.join()

    if cgroup:
        events = read_events(cgroup)
        for _ in range(events.get("oom_kill", 0) - seen[0]):
            kills.append(largest[0])
        peak = read_int(os.path.join(cgroup, "memory.peak"))
        say("peak %s MiB of a cap of %d MiB; %d out-of-memory kill(s)"
            % (peak // MEBIBYTE if peak is not None else "unknown", cap, len(kills)))
        for victim in kills:
            what = "%s at %d MiB" % victim if victim else "a process the samples did not see"
            print("::error title=out of memory::the memory fence's cap of %d MiB killed %s; it and "
                  "the CBMCs in the last sample beside it needed more memory than this runner has"
                  % (cap, what), flush=True)
        remove_fence(cgroup)
        if kills and status == 0:
            status = 1
    return status if status >= 0 else 128 - status


def selftest():
    fail = []

    def check(cond, what):
        if not cond:
            fail.append(what)

    goto = ("/t/target/kani/x86_64-unknown-linux-gnu/debug/build/address_space_identifier/7cd88fff52cbe1cd/out/"
            "address_space_identifier-7cd88fff52cbe1cd__RNvNtCsf7qcLhVz8dM_24address_space_identifier"
            "12verification27two_live_asids_are_distinct.out")
    check(harness_of(goto) == "verification::two_live_asids_are_distinct", "names a two-level harness")
    check(harness_of("x-1_RNvNtNtCsAb_1a1b2_9c.out") == "b::9c", "reads a `_` after a length")
    check(harness_of("--json-ui") is None, "ignores an argument that is not a goto file")
    check(harness_of("x_RNvCsAb_3abc5bad.out") is None, "refuses a length past the end")

    here = os.path.abspath(__file__)
    with tempfile.TemporaryDirectory() as tmp:
        env = dict(os.environ, NIFE_MEMORY_FENCE_ROOT=os.path.join(tmp, "none"))
        r = subprocess.run([sys.executable, here, "--", "sh", "-c", "exit 3"], env=env,
                           capture_output=True, text=True)
        check(r.returncode == 3 and "not fenced" in r.stdout, "runs bare and keeps the exit status")

        root = os.path.join(tmp, "cgroup")
        os.mkdir(root)
        with open(os.path.join(root, "cgroup.controllers"), "w") as f:
            f.write("cpu memory pids\n")
        meminfo = os.path.join(tmp, "meminfo")
        with open(meminfo, "w") as f:
            f.write("MemTotal:       16384000 kB\nMemAvailable:   15000000 kB\n")
        env = dict(os.environ, NIFE_MEMORY_FENCE_ROOT=root, NIFE_MEMORY_FENCE_MEMINFO=meminfo)
        # The command plays the kernel: it reports a peak and, for the second case, a kill.
        report = ('for f in %s/nife-memory-fence-*; do echo 1073741824 > "$f/memory.peak"; '
                  'printf "oom 0\\noom_kill %%s\\n" "$1" > "$f/memory.events"; done; exit "$2"' % root)
        r = subprocess.run([sys.executable, here, "--", "sh", "-c", report, "report", "0", "0"],
                           env=env, capture_output=True, text=True)
        check(r.returncode == 0, "a clean fenced run exits 0 (%d: %s)" % (r.returncode, r.stderr))
        check("capped at 14464 MiB" in r.stdout, "the cap is MemTotal less the reserve")
        check("peak 1024 MiB of a cap of 14464 MiB; 0 out-of-memory kill(s)" in r.stdout,
              "prints the peak")
        check([n for n in os.listdir(root) if n.startswith("nife-memory-fence-")] == [],
              "removes its cgroup")
        r = subprocess.run([sys.executable, here, "--", "sh", "-c", report, "report", "1", "0"],
                           env=env, capture_output=True, text=True)
        check(r.returncode == 1, "a kill fails a run that reported success")
        check("1 out-of-memory kill(s)" in r.stdout and "::error title=out of memory::" in r.stdout,
              "a kill is counted and annotated")
        r = subprocess.run([sys.executable, here, "--", "sh", "-c", report, "report", "0", "4"],
                           env=env, capture_output=True, text=True)
        check(r.returncode == 4, "keeps a failing exit status")
    if fail:
        sys.exit("memory_fence selftest: " + "; ".join(fail))
    print("memory_fence selftest: ok")


def main(argv):
    if argv == ["--selftest"]:
        return selftest()
    reserve, interval = 1536, 15.0
    while argv and argv[0] != "--":
        if argv[0] == "--reserve-mib" and len(argv) > 1:
            reserve, argv = int(argv[1]), argv[2:]
        elif argv[0] == "--interval" and len(argv) > 1:
            interval, argv = float(argv[1]), argv[2:]
        else:
            sys.exit(__doc__.split("\n\n")[1])
    if len(argv) < 2:
        sys.exit(__doc__.split("\n\n")[1])
    sys.exit(run(argv[1:], reserve, interval))


if __name__ == "__main__":
    main(sys.argv[1:])
