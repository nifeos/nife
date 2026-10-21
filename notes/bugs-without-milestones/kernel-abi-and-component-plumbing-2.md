# Kernel, ABI and component plumbing (part 2 of 2)

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  roadmap 0632 | failed-build path may keep pages; whether half-built drivers exit on a swept endpoint never forced | A failed session build is proven to free its pages | S  |
|  roadmap 0632 | a graphical session cannot be interrupted (no ^C escalation, hung session holds region to reboot) | A graphical session can be killed | M  |
|  roadmap 0636 | `current_cpu_protocol` reader forms `&AtomicU64` over a read-only mapped page | The cpu-page reader stops forming a reference | S  |
|  roadmap 0685 | job that answers then never exits holds a short pool's wait; `^C` cannot reach it | A waiting spawn request is interruptible | M  |
|  roadmap 0720 | semihosting `exit` paths are bare wfi/hlt loops with no `HaltReason` | Semihosting exit paths carry a HaltReason | S  |
|  roadmap 0761 | even capability slots straddle a cache line; align table to 64 | Align the capability table to a cache line | S  |
|  roadmap 0761 | capability read and grant are two critical sections; a revocation sweep can fall between (copy stays filed) | Close the derive-versus-revoke window | M  |
|  roadmap 0762 | `DeviceFrame` through `MAP_INTO` is covered by construction but not driven | Drive DeviceFrame through MAP_INTO | S  |
|  roadmap 0766 | ~30 other `[T; MAX_CPUS]` statics share cache lines; Apple 128-byte lines; unmeasured | Measure false sharing in the per-CPU statics | M  |
|  roadmap 0779 | fuzzer had no space root after parking on GO in rva23s64 CI, cause unknown (flake) | Find why a parked fuzzer lost its space root | M  |
|  roadmap 0848 | a server that never closes a dead client's session holds its charge until revoked | Reclaim a dead client's session charge | M  |
|  roadmap 0892 | program whose note asks zones without the network runs with no resolver; refusal is silent | Refuse zones without network by name | S  |
|  roadmap 0915 | correctable machine check recovery not built (every MC panics) | Recover from correctable machine checks | L  |
|  roadmap 0922 | path component max 16 bytes blocks 20-byte `least_authority_demo` by path; lift `MAX_NAME` | Lift filesystem_contract MAX_NAME | M  |
|  system_tests/src/user/running_space_tests.rs (32 frames) + scratch_window_tests.rs | first spawned program of a boot keeps 32 frames, unexplained (guessed: scratch-window last-level tables) | Explain the 32 frames kept by the first spawn in a boot | S  |
