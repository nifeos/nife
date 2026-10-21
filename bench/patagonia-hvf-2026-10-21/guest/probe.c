// Freestanding QEMU-virt probe: EL2 under HVF, VHE, PMU cycle counter behavior, GIC/ITS, SMMU IDR0.
typedef unsigned long u64; typedef unsigned int u32;
#define R(r) ({ u64 v; __asm__ volatile("mrs %0, " #r : "=r"(v)); v; })
#define W(r, v) __asm__ volatile("msr " #r ", %0; isb" :: "r"((u64)(v)))
static volatile u32 *const uart = (u32 *)0x09000000;
static void puts(const char *s) { while (*s) *uart = *s++; }
static void hex(const char *l, u64 v) { puts(l); puts(" 0x"); for (int i = 60; i >= 0; i -= 4) *uart = "0123456789abcdef"[(v >> i) & 15]; *uart = '\n'; }
static void dec(const char *l, u64 v) { char b[24]; int n = 0; puts(l); puts(" "); do b[n++] = '0' + v % 10; while (v /= 10); while (n) *uart = b[--n]; *uart = '\n'; }
static inline u64 cnt(void) { u64 v; __asm__ volatile("isb; mrs %0, cntvct_el0" : "=r"(v)); return v; }

void main(void) {
    u64 el = R(CurrentEL) >> 2;
    dec("CurrentEL", el);
    if (el == 2) {
        hex("HCR_EL2", R(hcr_el2));
        hex("VPIDR_EL2", R(vpidr_el2));
        u64 h = R(hcr_el2);
        W(hcr_el2, h | (1UL << 34));               // E2H
        hex("HCR_EL2 after E2H set", R(hcr_el2));
        hex("CNTHCTL_EL2", R(cnthctl_el2));
        W(hcr_el2, h);
        // Drop to EL1 for the rest so the PMU is measured where a kernel runs.
        W(cnthctl_el2, 3); W(cntvoff_el2, 0);
        W(hcr_el2, 1UL << 31);                     // RW: EL1 is AArch64
        extern char el1_entry[];
        W(elr_el2, (u64)el1_entry); W(spsr_el2, 0x3c5);
        __asm__ volatile("mov x0, sp; msr sp_el1, x0; eret");
    }
    __asm__ volatile(".global el1_entry\nel1_entry:");
    dec("now at EL", R(CurrentEL) >> 2);
    hex("ID_AA64DFR0_EL1", R(id_aa64dfr0_el1));
    hex("PMCR_EL0 before", R(pmcr_el0));
    W(pmcr_el0, (1 << 0) | (1 << 2) | (1 << 6));   // E, C (reset), LC
    W(pmcntenset_el0, 1UL << 31);
    W(pmccfiltr_el0, 0);
    // Does it move, and at what rate against the 24 MHz generic timer?
    u64 c0 = R(pmccntr_el0), t0 = cnt();
    while (cnt() - t0 < 2400000) {}                // 100 ms
    u64 c1 = R(pmccntr_el0), t1 = cnt();
    dec("PMCCNTR delta over 100 ms", c1 - c0);
    dec("CNTVCT delta", t1 - t0);
    // Cost of one PMCCNTR read, and of one CNTVCT read, in generic-timer ticks per 10000 reads.
    u64 s = cnt(); for (int i = 0; i < 10000; i++) (void)R(pmccntr_el0); u64 e = cnt();
    dec("ticks for 10000 PMCCNTR reads", e - s);
    s = cnt(); for (int i = 0; i < 10000; i++) (void)cnt(); e = cnt();
    dec("ticks for 10000 CNTVCT reads", e - s);
    // Back-to-back PMCCNTR differences: the floor sel4bench subtracts as overhead.
    u64 mn = ~0UL, mx = 0;
    for (int i = 0; i < 1000; i++) { u64 a = R(pmccntr_el0), b = R(pmccntr_el0); u64 d = b - a; if (d < mn) mn = d; if (d > mx) mx = d; }
    dec("back-to-back PMCCNTR min", mn); dec("back-to-back PMCCNTR max", mx);
    // GIC and ITS
    volatile u32 *gicd = (u32 *)0x08000000;
    hex("GICD_TYPER", gicd[1]); hex("GICD_PIDR2", gicd[0xffe8 / 4]);
    volatile u64 *gits = (u64 *)0x08080000;
    hex("GITS_TYPER", gits[1]); hex("GITS_PIDR2", ((volatile u32 *)gits)[0xffe8 / 4]);
    volatile u32 *gicr = (u32 *)0x080a0000;
    hex("GICR_TYPER", *(volatile u64 *)(gicr + 2));
    puts("done\n");
    for (;;) __asm__ volatile("wfi");
}
