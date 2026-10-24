#!/bin/sh
# measure.sh: the host-reordering measurement, as one bounded job.
#
#   measure.sh <litmus-scale> <weakening> <suite-deadline-seconds>
#
# 1. Builds the litmus guest at <litmus-scale> and runs it on an aarch64 and a riscv64 guest, four
#    cores each, under multi-threaded TCG. Prints its RESULT lines.
# 2. Applies weaken-spin.sh <weakening> (release, both, or none) and runs the kernel suite's
#    aarch64 and riscv64 legs, `script/test --arch aarch64,riscv64 --no-host`, again and again
#    until <suite-deadline-seconds> after the script started, printing one SUITE line per run.
#
# A scale of 0 skips step 1, and a deadline of 0 skips step 2.
#
# It needs the project's QEMU on PATH and the toolchain script/bootstrap installs. Logs land in
# target/host-reordering/. See notes/benchmarks/host-reordering-under-tcg.md.
set -u
cd "$(dirname "$0")/../.." || exit 1
# shellcheck source=helpers/qemu-path.sh
. helpers/qemu-path.sh
start=$(date +%s)
scale="${1:?usage: measure.sh <litmus-scale> <release|both|none> <suite-deadline-seconds>}"
weakening="${2:?}"
deadline="${3:?}"
out=target/host-reordering
mkdir -p "$out"

echo "HOST $(uname -m) $(nproc) cpus; $(qemu-system-aarch64 --version | head -1)"
grep -m1 -i 'model name' /proc/cpuinfo 2>/dev/null || true
lscpu 2>/dev/null | grep -i -m2 'model name\|vendor' || true

litmus=tools/host_reordering_litmus
elf() { echo "$litmus/target/$1/release/host_reordering_litmus"; }
[ "$scale" -eq 0 ] || {
for target in aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf; do
    LITMUS_SCALE="$scale" cargo build --quiet --release --target "$target" \
        --manifest-path "$litmus/Cargo.toml" || exit 1
done
helpers/qemu-bounded.sh 900 qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 \
    -smp 4 -m 256M -display none -serial stdio -semihosting -accel tcg,thread=multi \
    -kernel "$(elf aarch64-unknown-none-softfloat)" | tee "$out/litmus-aarch64.log"
helpers/qemu-bounded.sh 900 qemu-system-riscv64 -machine virt -cpu rv64 -smp 4 -m 256M \
    -bios none -display none -serial stdio -accel tcg,thread=multi \
    -kernel "$(elf riscv64imac-unknown-none-elf)" | tee "$out/litmus-riscv64.log"
}
[ "$deadline" -gt 0 ] || exit 0

if [ "$weakening" != none ]; then
    "$litmus/weaken-spin.sh" "$weakening" || exit 1
fi

i=0
while :; do
    now=$(date +%s)
    if [ $((now - start)) -ge "$deadline" ]; then
        break
    fi
    i=$((i + 1))
    t0=$now
    log="$out/suite-$i.log"
    # shellcheck disable=SC2065 # script/test is the suite, not test(1)
    script/test --arch aarch64,riscv64 --no-host > "$log" 2>&1
    status=$?
    secs=$(($(date +%s) - t0))
    echo "SUITE weakening=$weakening run=$i status=$status seconds=$secs"
    if [ "$status" -ne 0 ]; then
        grep -E -i 'FAIL|panicked|PANIC|timed out|hung' "$log" | head -20 | sed 's/^/  | /'
    fi
done
echo "SUITE-DONE weakening=$weakening runs=$i"
