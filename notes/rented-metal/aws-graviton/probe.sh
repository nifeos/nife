#!/bin/bash
#
# Runs on the metal instance as root, fed over ssh by ./run (`sudo bash -s < probe.sh`). Everything
# it prints is the evidence for ../aws-graviton.md's four questions, so it prints each command
# before its output and its exit status after. It also leaves every raw ACPI table in
# /tmp/acpi-tables.tar for ./run to copy back.
#
# Written for Ubuntu 24.04 (the packages below); every read that matters comes from /sys, /proc or
# dmesg first, so a missing package loses a convenience, never an answer.
#
# Name: provisional (the AWS Graviton probe's lane, 2026-10-21 UTC), after what it does.

set -u
export DEBIAN_FRONTEND=noninteractive LC_ALL=C

section() { printf '\n===== %s  (%s)\n' "$1" "$(date -u +%FT%TZ)"; }
show() { printf '$ %s\n' "$*"; sh -c "$*" 2>&1; printf '[exit %s]\n' "$?"; }

section "packages"
show "apt-get -qq update"
show "apt-get -qq install -y acpica-tools pciutils gcc linux-tools-aws linux-tools-\$(uname -r) >/dev/null"

section "the machine"
show "uname -a"
show "cat /proc/cmdline"
show "lscpu"
show "cat /sys/devices/system/cpu/cpu0/regs/identification/midr_el1"
show "dmidecode -t bios -t system -t processor | head -60"
show "ls /sys/firmware/devicetree 2>&1; ls /sys/firmware/acpi/tables"

section "Q3: the exception level Linux was entered at"
# arm64 Linux prints "CPU: All CPU(s) started at EL2" (or EL1), and KVM says whether it found VHE.
show "dmesg | grep -E 'started at EL|VHE|Hyp mode|kvm \\[|nVHE|hyp'"

section "ACPI tables, raw and decompiled"
mkdir -p /tmp/acpi && cd /tmp/acpi || exit 1
cp /sys/firmware/acpi/tables/[A-Z]* . 2>/dev/null
tar cf /tmp/acpi-tables.tar -C /tmp/acpi . && chmod 644 /tmp/acpi-tables.tar
show "ls -l /tmp/acpi"
# APIC is the MADT's signature (GIC distributor, redistributors, ITS, one GICC per CPU). IORT says
# what sits between each PCIe root complex and the ITS: an SMMUv3 node, or nothing.
for t in IORT APIC GTDT SPCR DBG2 MCFG FACP; do
    [ -e "$t" ] || { echo "no $t table"; continue; }
    show "od -A x -t x1 -v $t"
    show "iasl -d $t >/dev/null && cat $t.dsl"
done

section "Q1: the SMMU, as the kernel found it"
show "dmesg | grep -iE 'smmu|iommu|iort|its'"
show "ls -l /sys/class/iommu/"
show "ls /sys/bus/platform/devices/ | grep -i smmu"
show "lspci -D -nn"
show "lspci -D -vvv -d 1d0f:"
# Amazon's PCI vendor is 1d0f: ENA and the EBS and instance-store NVMe controllers.
for d in /sys/bus/pci/devices/*; do
    printf '%s %s:%s driver=%s iommu_group=%s iommu=%s\n' "${d##*/}" \
        "$(cat "$d/vendor")" "$(cat "$d/device")" \
        "$(basename "$(readlink "$d/driver" 2>/dev/null)" 2>/dev/null)" \
        "$(basename "$(readlink "$d/iommu_group" 2>/dev/null)" 2>/dev/null)" \
        "$(basename "$(readlink "$d/iommu" 2>/dev/null)" 2>/dev/null)"
done
for g in /sys/kernel/iommu_groups/*; do
    [ -d "$g" ] || { echo "no IOMMU groups"; break; }
    printf '%s type=%s devices=%s\n' "${g##*/}" "$(cat "$g/type" 2>/dev/null)" "$(ls "$g/devices" | tr '\n' ' ')"
done
# The per-CPU count columns are dropped; what is left names each interrupt's controller and hwirq.
show "awk '/ITS|SMMU|smmu|nvme|ena/ { print \$1, \$(NF-3), \$(NF-2), \$(NF-1), \$NF }' /proc/interrupts"

section "Q2: the PMU and the cycle counter"
show "sysctl -w kernel.perf_event_paranoid=-1 kernel.perf_user_access=1"
show "dmesg | grep -iE 'perf|pmu|spe|cmn'"
show "ls /sys/bus/event_source/devices/"
show "perf stat -e cycles,instructions,cycles:u,cycles:k -- sh -c 'i=0; while [ \$i -lt 3000000 ]; do i=\$((i+1)); done'"
show "perf stat -a -e cycles -- sleep 1"

cat > /tmp/pmccntr.c <<'EOF'
/* What EL0 sees of the cycle counter, before and after perf grants access. */
#include <linux/perf_event.h>
#include <sched.h>
#include <setjmp.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>

#define READ(reg) ({ uint64_t v_; __asm__ volatile("isb; mrs %0, " #reg : "=r"(v_)); v_; })

static sigjmp_buf trapped;
static void on_sigill(int sig) { (void)sig; siglongjmp(trapped, 1); }

static int read_pmccntr(uint64_t *out) {
    if (sigsetjmp(trapped, 1)) return 0;
    *out = READ(pmccntr_el0);
    return 1;
}

static void measure(const char *when) {
    uint64_t c0, c1;
    if (!read_pmccntr(&c0)) { printf("%s: mrs pmccntr_el0 trapped (SIGILL)\n", when); return; }
    uint64_t freq = READ(cntfrq_el0), t0 = READ(cntvct_el0);
    for (volatile uint64_t i = 0; i < 300000000; i++) {}
    uint64_t t1 = READ(cntvct_el0);
    read_pmccntr(&c1);
    double secs = (double)(t1 - t0) / (double)freq;
    printf("%s: pmccntr_el0 advanced %llu over %.3f s of cntvct_el0 (cntfrq %llu Hz): %.1f MHz\n",
           when, (unsigned long long)(c1 - c0), secs, (unsigned long long)freq,
           (double)(c1 - c0) / secs / 1e6);
}

int main(void) {
    struct sigaction sa;
    memset(&sa, 0, sizeof sa);
    sa.sa_handler = on_sigill;
    sigaction(SIGILL, &sa, NULL);

    /* Linux emulates these reads at EL0 and sanitizes them, so a field it hides reads 0. */
    printf("MIDR_EL1 %016llx\n", (unsigned long long)READ(midr_el1));
    printf("ID_AA64PFR0_EL1 %016llx\n", (unsigned long long)READ(id_aa64pfr0_el1));
    printf("ID_AA64PFR1_EL1 %016llx\n", (unsigned long long)READ(id_aa64pfr1_el1));
    printf("ID_AA64ISAR0_EL1 %016llx\n", (unsigned long long)READ(id_aa64isar0_el1));
    printf("ID_AA64ISAR1_EL1 %016llx\n", (unsigned long long)READ(id_aa64isar1_el1));
    printf("ID_AA64MMFR0_EL1 %016llx\n", (unsigned long long)READ(id_aa64mmfr0_el1));
    printf("ID_AA64MMFR1_EL1 %016llx\n", (unsigned long long)READ(id_aa64mmfr1_el1));
    printf("ID_AA64MMFR2_EL1 %016llx\n", (unsigned long long)READ(id_aa64mmfr2_el1));
    printf("ID_AA64DFR0_EL1 %016llx\n", (unsigned long long)READ(id_aa64dfr0_el1));

    measure("no perf event open");

    /* config1 bit 0 asks for a 64-bit counter, which puts cycles on PMCCNTR_EL0; bit 1 asks for
     * EL0 reads, which kernel.perf_user_access=1 allows. Linux then sets PMUSERENR_EL0 while this
     * task runs. */
    struct perf_event_attr attr;
    memset(&attr, 0, sizeof attr);
    attr.type = PERF_TYPE_HARDWARE;
    attr.size = sizeof attr;
    attr.config = PERF_COUNT_HW_CPU_CYCLES;
    attr.config1 = 0x3;
    attr.exclude_kernel = 1;
    attr.exclude_hv = 1;
    int fd = (int)syscall(SYS_perf_event_open, &attr, 0, -1, -1, 0);
    if (fd < 0) { perror("perf_event_open"); return 1; }
    struct perf_event_mmap_page *pg = mmap(NULL, (size_t)sysconf(_SC_PAGESIZE), PROT_READ, MAP_SHARED, fd, 0);
    if (pg == MAP_FAILED) { perror("mmap"); return 1; }
    sched_yield();
    printf("perf user page: cap_user_rdpmc %u, index %u, pmc_width %u\n",
           (unsigned)pg->cap_user_rdpmc, pg->index, (unsigned)pg->pmc_width);
    measure("user-readable cycles event open");
    uint64_t count = 0;
    if (read(fd, &count, sizeof count) == (ssize_t)sizeof count)
        printf("perf's own count for this task at EL0: %llu\n", (unsigned long long)count);
    return 0;
}
EOF
show "gcc -O1 -o /tmp/pmccntr /tmp/pmccntr.c && taskset -c 1 /tmp/pmccntr"

section "done"
