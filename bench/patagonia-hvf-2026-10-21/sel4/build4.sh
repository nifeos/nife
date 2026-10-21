#!/bin/sh
set -eu
b() { dir=$1; dtb=$2; cpu=$3; shift 3; cd /w/$dir; rm -rf build && mkdir build && cd build
  ../init-build.sh -DPLATFORM=qemu-arm-virt -DAARCH64=TRUE -DARM_CPU=$cpu -DQEMU_GIC_VERSION=3 -DQEMU_DTB=/w/$dtb "$@" > configure.log 2>&1 && ninja > ninja.log 2>&1 && echo "$dir: $(ls images)"
  grep -E "^Kernel[A-Za-z0-9]*:|^(RELEASE|FASTPATH|HARDWARE|FAULT|ITERATIONS|MCS|PLATFORM|ARM_CPU|ARM_HYP|QEMU_GIC_VERSION|AllowUnstableOverhead):" CMakeCache.txt | sort > config.txt; }
bench="-DFASTPATH=TRUE -DITERATIONS=5 -DAllowUnstableOverhead=ON -DHARDWARE=OFF -DFAULT=OFF -DIRQUSER=OFF -DSCHED=OFF -DSIGNAL=OFF -DMAPPING=OFF -DSYNC=OFF"
b sl8 hvf-virt.dtb cortex-a53 $bench -DKernelArmExportVCNTUser=ON
b s8 hvf-virt.dtb cortex-a53 $bench
b t8 hvf-virt.dtb cortex-a53 -DSIMULATION=FALSE
b slhyp hvf-virt-el2.dtb cortex-a53 $bench -DARM_HYP=ON -DKernelArmExportVCNTUser=ON
b shyp hvf-virt-el2.dtb cortex-a53 $bench -DARM_HYP=ON
