#!/usr/bin/env python3
"""Every userspace `asm!` block that traps to the kernel declares every register the kernel writes.

    helpers/syscall_asm.py              # script/lint: fail on a trap that under-declares
    helpers/syscall_asm.py --selftest   # fixtures; script/lint

Added 2026-10-05 by lane/hvf-spawn-destroy-gone; the file name is provisional and calef has not
ratified it. A shared python module under `helpers/`, outside `script/names`' scope, so its
provenance is this paragraph.

# WHY

The kernel answers every syscall in the first argument register and may write the next five
(`kernel/src/syscall.rs` `dispatch`, `frame.set_arg`; the sixth is milestone 105 (the two forks)'s
dead child's label). An `asm!` block that declares fewer outputs
than that promises the compiler those registers survive the trap, and an optimiser is entitled to
keep a live value in one. `user_mode_runtime::yield_now` declared none. In the job mix's spawn job
the compiler loaded `DESTROY`'s five arguments before a yield and trapped with them after it, so
the second `DESTROY` went out as `invoke(0, 4)`, a `CALL` on the report endpoint, and came back
`Gone`. It failed 19 of 20 HVF sweeps and wedged the twentieth (notes/job-mix/spawn-job-miscompiled-yield.md).
Nothing about it was visible in the source: the defect was a missing declaration, and a missing
thing is what a reader does not see. Hence a gate rather than a comment.

# WHAT IT CHECKS

Every tracked `.rs` file outside `kernel/` and `vendor/` (the kernel's own traps are EL1 calls into
firmware, a different contract). For each `asm!(` whose template has `svc`, `ecall` or `syscall`,
the block must name as an output (`out`, `lateout`, `inout`, `inlateout`) all six words of the
syscall ABI: `x0..x5`, `a0..a5`, or `rdi rsi rdx r10 r8 r9`, and on `x86_64` also `rcx` and `r11`,
which the instruction itself writes.

# WHAT IT CANNOT SEE (recorded where a reader meets it)

- A trap in `global_asm!`, a `.s` file, or a C file. None exists outside the kernel today; a
  hand-written assembly stub allocates its own registers, so the compiler hazard does not apply.
- A template built by a macro (`concat!`) is not parsed. One would read as having no trap.
- It checks declarations, not what the kernel writes. If the kernel ever answers in a seventh
  register, this list and `trap6` in `crates/user_mode_runtime` change together.
"""
import os
import re
import subprocess
import sys
import tempfile

REQUIRED = {
    'svc': {'x0', 'x1', 'x2', 'x3', 'x4', 'x5'},
    'ecall': {'a0', 'a1', 'a2', 'a3', 'a4', 'a5'},
    'syscall': {'rdi', 'rsi', 'rdx', 'r10', 'r8', 'r9', 'rcx', 'r11'},
}
TRAP = re.compile(r'"\s*(svc|ecall|syscall)\b')
OUTPUT = re.compile(r'\b(?:out|lateout|inout|inlateout)\(\s*"(\w+)"\s*\)')
ASM = re.compile(r'\basm!\s*\(')
SKIP = ('kernel/', 'vendor/')


def blocks(text):
    """Yield (line, body) for each `asm!( ... )`, matching parentheses outside string literals."""
    for m in ASM.finditer(text):
        i, depth, in_str = m.end(), 1, False
        while i < len(text) and depth:
            c = text[i]
            if in_str:
                if c == '\\':
                    i += 1
                elif c == '"':
                    in_str = False
            elif c == '"':
                in_str = True
            elif c == '(':
                depth += 1
            elif c == ')':
                depth -= 1
            i += 1
        yield text.count('\n', 0, m.start()) + 1, text[m.end():i]


def findings(path, text):
    out = []
    for line, body in blocks(text):
        t = TRAP.search(body)
        if not t:
            continue
        missing = REQUIRED[t.group(1)] - set(OUTPUT.findall(body))
        if missing:
            out.append(f'{path}:{line}: `{t.group(1)}` does not declare '
                       f'{", ".join(sorted(missing))} as outputs')
    return out


def run(root):
    files = subprocess.run(['git', '-C', root, 'ls-files', '*.rs'], capture_output=True, text=True,
                           check=True).stdout.split()
    files = sorted(set(files))  # an unmerged path is listed once per stage
    bad, traps = [], 0
    for f in files:
        if f.startswith(SKIP):
            continue
        with open(os.path.join(root, f), encoding='utf-8') as fh:
            text = fh.read()
        traps += sum(1 for _, b in blocks(text) if TRAP.search(b))
        bad += findings(f, text)
    return bad, traps


def selftest():
    good_arm = 'asm!("svc #0", in("x8") n, inlateout("x0") a => b, lateout("x1") _, out("x2") _,\n' \
               '     inout("x3") c, lateout("x4") _, lateout("x5") _, options(nostack))'
    # The 2026-10-05 defect, verbatim in shape: no output at all.
    old_yield = 'asm!("svc #0", in("x8") abi::SYS_YIELD, options(nostack, nomem));'
    # Declares the result but not the four words a RECEIVE writes.
    half = 'asm!("ecall", in("a7") n, inlateout("a0") s => r, in("a1") m, options(nostack))'
    # Every word, but not the instruction's own clobbers.
    x86 = 'asm!("syscall", in("rax") n, inlateout("rdi") a => b, lateout("rsi") _, lateout("rdx") _,\n' \
          '     lateout("r10") _, lateout("r8") _, options(nostack))'
    not_a_trap = 'asm!("mrs {}, cntvct_el0", out(reg) t, options(nomem, nostack)); let s = "svc";'
    paren_in_string = 'asm!("svc #0 // )", in("x8") n, options(nostack))'
    cases = [
        (good_arm, 0), (old_yield, 1), (half, 1), (x86, 1), (not_a_trap, 0), (paren_in_string, 1),
    ]
    ok = True
    for src, want in cases:
        got = len(findings('t.rs', src))
        if got != want:
            print(f'syscall_asm selftest: expected {want} finding(s), got {got} for: {src}',
                  file=sys.stderr)
            ok = False
    with tempfile.TemporaryDirectory() as d:
        subprocess.run(['git', 'init', '-q', d], check=True)
        for name, src in [('a/x.rs', old_yield), ('kernel/k.rs', old_yield), ('a/y.rs', good_arm)]:
            os.makedirs(os.path.dirname(os.path.join(d, name)), exist_ok=True)
            with open(os.path.join(d, name), 'w', encoding='utf-8') as fh:
                fh.write(src)
        subprocess.run(['git', '-C', d, 'add', '.'], check=True)
        bad, traps = run(d)
        # line-citation: exception. Reason: the fixture expects the checker's own path:line report form
        if len(bad) != 1 or not bad[0].startswith('a/x.rs:1') or traps != 2:
            print(f'syscall_asm selftest: tree walk found {bad}, {traps} traps', file=sys.stderr)
            ok = False
    return 0 if ok else 1


def main(argv):
    if argv[1:] == ['--selftest']:
        return selftest()
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
    bad, traps = run(root)
    if bad:
        print('lint: a userspace trap under-declares the registers the kernel writes:',
              file=sys.stderr)
        for b in bad:
            print('  ' + b, file=sys.stderr)
        print('  The compiler may keep a live value in an undeclared register across the trap. '
              'Route the call through `trap6` (crates/user_mode_runtime), or declare all six words. '
              'notes/job-mix/spawn-job-miscompiled-yield.md has the miscompilation this caught.',
              file=sys.stderr)
        return 1
    print(f'syscall asm: {traps} userspace traps, every one declares all six result registers')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
