#!/bin/sh
# boot.sh IMAGE LOG SECONDS [el2]: boot under HVF on the shared machine string, stop at a verdict or the bound.
img=$1 log=$2 secs=$3 m=virt,accel=hvf,gic-version=3,iommu=smmuv3,secure=off
[ "${4:-}" = el2 ] && m=$m,virtualization=on
qemu-system-aarch64 -machine $m -cpu host -smp 1 -m 1024 -nographic -kernel "$img" > "$log" 2>&1 & p=$!
n=0; while [ $n -lt "$secs" ]; do sleep 5; n=$((n+5))
  grep -q -E "All is well in the universe|halting|Test suite failed|\"error\"|^All benchmarks|benchmark finished|FAILED|seL4 called fail" "$log" && sleep 3 && break; done
kill $p 2>/dev/null; wait $p 2>/dev/null; echo "$log after ${n}s"
