#!/bin/sh
# Turn sel4bench's ipc app into a loop-averaged one timed by CNTVCT_EL0. Run from the sel4bench-manifest root.
set -eu
h=projects/sel4bench/libsel4benchsupport/arch_include/arm/arch/ipc.h
m=projects/sel4bench/apps/ipc/src/main.c
cat >> "$h" <<'EOT'

/* loop-averaged variant: the architected generic timer, read at EL0 */
#undef READ_COUNTER_BEFORE
#undef READ_COUNTER_AFTER
#define READ_COUNTER_BEFORE(x) __asm__ volatile("isb; mrs %0, cntvct_el0; isb" : "=r"(x))
#define READ_COUNTER_AFTER(x) __asm__ volatile("isb; mrs %0, cntvct_el0; isb" : "=r"(x))
EOT
python3 - "$m" <<'EOP'
import sys
p = sys.argv[1]
s = open(p).read()
n0 = s.count("#define WARMUPS RUNS")
s = s.replace("#define WARMUPS RUNS", "#define WARMUPS LOOP_N\n#define LOOP_N 10000")
# Only the four call and reply-recv generators: stamp before the first pass and after the last.
old_b = "        READ_COUNTER_BEFORE(start); \\\n        bench_func(ep, tag"
new_b = "        if (i == 0) { READ_COUNTER_BEFORE(start); } \\\n        bench_func(ep, tag"
old_a = "        READ_COUNTER_AFTER(end); \\\n    } \\\n    COMPILER_MEMORY_FENCE(); \\\n"
new_a = "        if (i == WARMUPS - 1) { READ_COUNTER_AFTER(end); } \\\n    } \\\n    COMPILER_MEMORY_FENCE(); \\\n"
assert n0 == 1 and s.count(old_b) == 2, (n0, s.count(old_b))
s = s.replace(old_b, new_b)
k = s.count(old_a)
s = s.replace(old_a, new_a, 2)
assert k >= 2, k
open(p, "w").write(s)
EOP
grep -n "LOOP_N\|i == 0\|WARMUPS - 1" "$m"
