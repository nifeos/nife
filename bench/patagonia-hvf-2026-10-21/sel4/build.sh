#!/bin/sh
# Inside trustworthysystems/sel4: sel4test (master manifest) and sel4bench (11d0fc2c) for qemu-arm-virt, GICv3, with the DTB HVF's machine dumps.
set -eu
plat="-DPLATFORM=qemu-arm-virt -DAARCH64=TRUE -DARM_CPU=cortex-a57 -DQEMU_GIC_VERSION=3 -DQEMU_DTB=/w/hvf-virt.dtb"
if [ ! -d /w/t/.repo ]; then mkdir -p /w/t && cd /w/t && repo init -q -u https://github.com/seL4/sel4test-manifest.git && repo sync -q -j4; fi
cd /w/t && repo manifest -r -o /w/sel4test-pinned.xml
rm -rf /w/t/build && mkdir /w/t/build && cd /w/t/build
../init-build.sh $plat -DSIMULATION=FALSE > configure.log 2>&1 && ninja > ninja.log 2>&1 && ls images
cd /w/src && repo manifest -r -o /w/sel4bench-pinned.xml
rm -rf /w/src/build && mkdir /w/src/build && cd /w/src/build
../init-build.sh $plat -DFASTPATH=TRUE -DHARDWARE=TRUE -DFAULT=TRUE -DITERATIONS=5 > configure.log 2>&1 && ninja > ninja.log 2>&1 && ls images
grep -E "^Kernel[A-Za-z0-9]*:|^(RELEASE|FASTPATH|HARDWARE|FAULT|ITERATIONS|MCS|PLATFORM|ARM_CPU|QEMU_GIC_VERSION):" CMakeCache.txt | sort > /w/sel4bench-config.txt
