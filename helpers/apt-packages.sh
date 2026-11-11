#!/bin/sh
# Print the apt packages of one group in helpers/apt-packages.txt, or of every group, one per line
# and without repeats.
#
#     helpers/apt-packages.sh                 every package
#     helpers/apt-packages.sh qemu-runtime    one group's packages
#
# script/ci-qemu, script/bootstrap and `helpers/owned-image-probe.sh packages` read the list through
# this, so what they install and what the probe checks are one list. Exits 1 for an unknown group.
#
# Name: provisional, minted by lane/owned-image-packages-probe.

set -eu

list="$(dirname "$0")/apt-packages.txt"
group=${1:-}
out=$(awk -v group="$group" '
    /^#/ || NF == 0 { next }
    group == "" || $1 == group { found = 1; for (i = 2; i <= NF; i++) if (!seen[$i]++) print $i }
    END { if (!found) exit 1 }
' "$list") || {
    echo "apt-packages: no group '$group' in $list" >&2
    exit 1
}
echo "$out"
