---
status: NOT-STARTED
promoted_from: report-qemu-plic-enable-recheck-upstream
raised: 2026-10-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 798. Report QEMU's PLIC enable-write defect upstream

Raised by the lane for the provisional milestone "a USB keystroke never strands `line_editor`"
(`lane/usb-keyboard-lost-wakeup`, #1657) on 2026-10-05 (UTC), after finding that the riscv64
swish-check keystroke stall was an emulator defect that nife's PLIC driver happened to expose. Title
and slug are drafts.

Reuse: the upstream fix already exists and is reused rather than rewritten: LIU Xu's
unmerged qemu-devel patch of 2026-03-25 (below). nife writes no patch of its own. Searched QEMU
master and the qemu-devel archive for an existing fix or report, and nife's own tree for an earlier
workaround record (none before #1657).

## The defect

QEMU 11.1.1's `hw/intc/sifive_plic.c` re-evaluates which contexts should see an external interrupt
(`sifive_plic_update`) after a priority write, a threshold write, a completion and a rising source
line. **It does not after a write to an enable word.** The enable branch of `sifive_plic_write`
stores the word and returns. So a source that went pending while it was disabled stays pending,
enabled, above threshold and undelivered until some unrelated event calls `sifive_plic_update`.

The specification has the PLIC notify a target whenever a source is pending, enabled and above the
threshold. A real part evaluates that continuously, so the defect is the emulator's.

nife hit it because `plic::enable`, which is also a userspace driver's `Interrupt::ACK`, wrote the
priority before the enable bit, so the ACK's only re-evaluation ran while the bit was clear. #1657
reordered the two writes. That works around the defect in nife. Any other guest that masks a PLIC
source in its handler and unmasks it with nothing but an enable write is still exposed. Whether
Linux's `irq-sifive-plic.c` is such a guest was not checked.

## The minimal reproducer

Any level source the guest can raise by hand works. The 16550's transmit-empty interrupt is the
one nife's test uses:

1. Enable source 10 (the UART on `virt`) for a context with priority 1 and threshold 0.
2. Raise the line (set `IER.ETBEI`), take the interrupt, claim it, clear its enable bit, complete.
3. Lower the line (`IER = 0`), then raise it again. It is now pending while disabled.
4. Set the enable bit. Nothing else.
5. `sip.SEIP` stays clear and no interrupt is taken, although source 10 is pending, enabled and
   above threshold. Any priority or threshold write now delivers it at once.

In nife that sequence is the kernel test, red with the old write order and deterministic:

```sh
git apply kernel/falsifications/sched.tests.an_interrupt_raised_while_its_line_is_masked_is_delivered_at_the_ack.patch
cargo xtask test --arch riscv64 --test an_interrupt_raised_while_its_line_is_masked
git apply -R kernel/falsifications/sched.tests.an_interrupt_raised_while_its_line_is_masked_is_delivered_at_the_ack.patch
```

The symptom that found it, measured on patagonia with the old order: 10 stalls in 300 boots of
swish-check's USB keystrokes alone (riscv64, TCG, four harts). The harness that counted them is
short enough to keep here. It boots the `--features shell` riscv64 kernel with `qemu-xhci` and
`usb-kbd`, types `echo hello` through the monitor, and calls a boot a stall when `hello` does not
come back within 15 s:

```python
#!/usr/bin/env python3
# usage: usbloop.py <worktree> <boots> <parallel> <key_delay_ms>
import os, socket, subprocess, sys, threading, time, concurrent.futures as cf

WT, N, PAR, DELAY = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]) / 1000
KEYS = ["e", "c", "h", "o", "spc", "h", "e", "l", "l", "o", "ret"]
DROP = ("NIFE_KEYBOARD", "NIFE_GPU", "NIFE_GPU_MON", "NIFE_NET", "NIFE_DISK", "NIFE_NVME")

def boot(i):
    sock = f"/tmp/usbloop-{os.getpid()}-{i}.sock"
    env = {k: v for k, v in os.environ.items() if k not in DROP}
    env.update(NIFE_USB_KEYBOARD="1", NIFE_USB_KEYBOARD_OPTS=",usb_version=1", NIFE_SCREEN_MON=sock,
               NIFE_INITIAL_RAMDISK=os.path.join(WT, "target/initial-ramdisk-riscv.img"))
    p = subprocess.Popen(["helpers/qemu-bounded.sh", "120", "helpers/qemu-runner-riscv64.sh",
                          "target/riscv64imac-unknown-none-elf/debug/kernel"], cwd=WT, env=env,
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    buf, lock = bytearray(), threading.Lock()
    def rd():
        while (b := p.stdout.read1(4096)):
            with lock: buf.extend(b)
    threading.Thread(target=rd, daemon=True).start()
    def text():
        with lock: return buf.decode("utf-8", "replace").replace("\r", "")
    def wait(pred, secs):
        end = time.time() + secs
        while time.time() < end:
            if pred(text()): return True
            time.sleep(0.1)
        return False
    verdict = "noprompt"
    if wait(lambda s: s.endswith("$ ") and "usb       : a keyboard on port" in s, 60):
        mark = len(text())
        for k in KEYS:
            with socket.socket(socket.AF_UNIX) as s:
                s.connect(sock); s.sendall(f"sendkey {k}\n".encode())
            time.sleep(DELAY)
        verdict = "ok" if wait(lambda s: "echo hello\nhello\n" in s[mark:], 15) else "STALL"
    p.terminate()
    subprocess.run(["pkill", "-f", sock])
    return verdict

with cf.ThreadPoolExecutor(PAR) as ex:
    res = {}
    for v in ex.map(boot, range(N)):
        res[v] = res.get(v, 0) + 1
print(res)
```

Run it from a worktree after `script/swish-check --arch riscv64` has built the image, as
`usbloop.py <worktree> 300 6 200`. It kills each QEMU it started by its socket name, and
`pgrep -l qemu` afterwards should be empty.

## The fix already exists upstream, unmerged

Checked 2026-10-05 (UTC): the defect is still present on QEMU master at d7a65d1793d6 (2026-10-03).
The fix is not new. LIU Xu posted the identical one line, a `sifive_plic_update(plic)` call after
the enable store, to qemu-devel on 2026-03-25 as "[PATCH qemu] hw/intc: Call sifive_plic_update()
after writing interrupt enable" (Message-ID `<177442359063.1954.8266696018975379698-0@git.sr.ht>`).
It got no replies and was not merged, and it was sent without copying the RISC-V maintainers or
the qemu-riscv list, which is the likely reason nobody saw it.

## nife sends no patch

QEMU's `docs/devel/code-provenance.rst` declines contributions derived from AI tools, and
`checkpatch` enforces it. Everything in this tree that touches the defect was written by an agent,
so nife does not send a patch, not even a one-line one. What it can send is a bug report and test
evidence, and calef sends it himself.

The evidence exists. A qtest reproducer that needs no guest fails 3 runs in 3 on master and passes
3 in 3 with LIU Xu's fix applied, and the rest of `qtest-riscv64` stays green (17 OK) with the fix.
The reproducer, both build logs, a draft GitLab issue and a draft reply to LIU Xu's thread are in
`~/projects/qemu-upstream-report/` on patagonia, outside the tree on purpose. They are not part
of nife, and committing them here would make them look like a contribution.

## Why this waits on calef

Both remaining actions leave the machine under calef's name, and neither can be taken back. That
puts them on the irreversible list in `AGENTS.md`:

1. File the GitLab issue, with the qtest reproducer and its results.
2. Reply on LIU Xu's thread with the same evidence, copying the RISC-V maintainers and qemu-riscv,
   so his patch reaches the people who can merge it.

When his patch lands and nife's pinned QEMU (`.qemu-version`) moves past it, the write order in
`plic::enable` stops mattering under the emulator. It stays as it is: a real PLIC does not care
about the order either.

## BUGS

- The qtest materials live on one machine, outside any repository. If patagonia loses them before
  calef files the issue, they have to be rebuilt from this file's reproducer steps.
- Nobody has measured what an extra `sifive_plic_update` per enable write costs. It is believed
  negligible, since the same call already runs on every change of a source line.

## Index row

QEMU 11.1.1's PLIC does not re-evaluate pending external interrupts after an enable write, which stalled a riscv64 keystroke and was exposed by nife's PLIC driver. The block reports it upstream and reuses an existing unmerged qemu-devel fix rather than writing one.
