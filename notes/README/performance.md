# Notes index: Performance

Part of [the notes index](../README.md), which says how to add a line.

- [Where the kernel suite's time goes](../kernel-suite-time.md): per-test and per-module medians on each architecture from 30 merge-group runs, and what the riscv64 suite's growth consists of.
- [Benchmarks with teeth](../benchmarks.md): the current numbers and their gates, with the dated history in `notes/benchmarks/` appendices.
- [The instruction clock](../instruction-clock.md): timing claims measured in instructions retired, not wall time.
- [What fio needs from nife's C library](../fio-on-nife.md): milestone 833 (the same storage benchmark on nife and Linux, by porting real fio)'s list of missing headers and functions, found by compiling fio 3.40. Name provisional.
- [The bench runbook: which machine, in what order, and what an evening buys](../bench-runbook.md).
- [The bench boots](../bench-boots.md): what `disk_throughput`, `network_bench`, `storage_bench` and `xhci_port_probe` print, and why. Name provisional.
- [Taking a benchmark on radon](../footprint-perturbation.md): running the cache-footprint experiments on the small-cache board.
- [The workload that does not stop](../soak.md): a sustained multicore workload whose threads never migrate.
- [A kernel-initiated reboot on every board](../board-reboot.md).
- [The multicore defect-discovery curve](../multicore-defect-curve.md): milestone 201 (is multicore reliability converging)'s data, the format a soak appends to, and every multicore defect so far. Name provisional.
- [The multi-tasking workload benchmark](../job-mix.md): an AIM7-style workload for the process-versus-event kernel question.
- [Cycle counters on RISC-V, and why nothing here has measured one](../riscv-cycle-counters.md).
- [Does the TSC tick at a constant rate under TCG?](../tsc-under-tcg.md). Name provisional.
- [Running under virtualization on Apple Silicon](../virtualization.md): running the kernel on the Mac's core via HVF.
- [The HVF leg](../hvf-leg.md): the aarch64 suite on the physical Apple Silicon core.
- [The CPU-model matrix](../cpu-models.md): the RISC-V suite run across five QEMU CPU models.
