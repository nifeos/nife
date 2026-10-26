#!/usr/bin/env python3
"""Replay a trace with some holdings removed, and report the new progenitor peak.
Usage: counterfactual.py TRACE ELF"""
import os, re, subprocess, sys
trace, elf = sys.argv[1], sys.argv[2]
pat = re.compile(r"^(\d+) P ([-+=]) slot=(\d+) used=(\d+) .*?(Rendezvous|PageFrame|MemoryRegion|ThreadControlBlock|AddressSpace|Virtio|Interrupt|Timer|DeviceFrame|\w+)\((.*?)\) rights=\S+ bt=\[(.*)\]")
ev = []
for l in open(trace):
    m = pat.match(l)
    if m:
        seq, op, slot, used, kind, oid, bt = m.groups()
        bt = [int(x, 16) for x in bt.split(", ") if x != "0"]
        ev.append([int(seq), op, int(slot), int(used), f"{kind}({oid})", bt])
addrs = sorted({a for e in ev for a in e[5]})
out = subprocess.run([os.environ.get("LLVM_SYMBOLIZER", "llvm-symbolizer"), "--obj=" + elf, "-f", "-C", "-i"],
                     input="\n".join(hex(a - 4) for a in addrs) + "\n", capture_output=True, text=True).stdout
sym = {}
for a, blk in zip(addrs, out.strip("\n").split("\n\n")):
    ls = blk.split("\n")
    sym[a] = [(ls[k], ls[k + 1]) for k in range(0, len(ls) - 1, 2)]
def lines(bt):
    r = []
    for a in bt:
        for fn, loc in sym.get(a, []):
            if "system_initializer/src/lib.rs" in loc:
                r.append(int(loc.split(":")[-2]))
    return r
for e in ev:
    e.append(lines(e[5]))

# Device grants: the seven boot-endowment slots that are Virtio/Interrupt/PageFrame minted before
# seq 25
# by the kernel on a gpu boot (12, 17..22).
DEVICE_SLOTS = {12, 17, 18, 19, 20, 21, 22}
launch_start = next((e[0] for e in ev if any(4282 <= n <= 4565 or 3080 <= n <= 3100 for n in e[6])), None)

def replay(drop):
    """drop(event, occupant) -> True to treat that occupant as absent at this moment."""
    occ = {}
    best = (0, None)
    for e in ev:
        seq, op, slot = e[0], e[1], e[2]
        if op in "-=":
            occ.pop(slot, None)
        if op in "+=":
            occ[slot] = e
        n = sum(1 for s, o in occ.items() if not drop(seq, s, o))
        if n > best[0]:
            best = (n, seq)
    return best

base = replay(lambda seq, s, o: False)
print(f"{trace}: baseline peak {base[0]} at seq {base[1]}; launch starts at seq {launch_start}")
gpu = any(e[4].startswith("Virtio(2)") for e in ev)
# O6: a separate process holds the device grants from the start of the boot and builds the
# graphical session; the progenitor holds none of them and does none of the launch.
def o6(seq, s, o):
    if o[0] < 25 and s in DEVICE_SLOTS and gpu:
        return True
    return launch_start is not None and o[0] >= launch_start
print("  O6 (devices and the launch live in another process):", replay(o6))
# Devices alone removed, launch kept in the progenitor (impossible as stated: the launch needs them;
# shown to separate the two contributions).
print("  devices removed outside the launch only:", replay(lambda seq, s, o: o[0] < 25 and s in DEVICE_SLOTS and gpu and (launch_start is None or seq < launch_start)))
print("  launch removed only:", replay(lambda seq, s, o: launch_start is not None and o[0] >= launch_start))
# O3a: the login block's inputs deleted as soon as build_child(login) returns (its TCB mint).
login_tcb = next((e[0] for e in ev if 2347 in e[6] and e[4].startswith("ThreadControlBlock")), None)
LOGIN_INPUTS = {2324, 2338, 2339, 2138, 2141, 5789, 4841}
def o3a(seq, s, o):
    if login_tcb is None or seq < login_tcb:
        return False
    return o[0] < login_tcb and o[0] > login_tcb - 600 and bool(set(o[6][:1]) & LOGIN_INPUTS)
print("  O3a (login inputs deleted when build_child(login) returns):", replay(o3a))
