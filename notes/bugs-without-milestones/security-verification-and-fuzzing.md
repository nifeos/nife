# Security, verification and fuzzing

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  0442-...:124 and design/roadmap/0501-a-tls-client-that-speaks-to-one-pinned-peer.md | pinned-key model has no rotation story; the pin works only through cross-signatures that expire 2032-09-02 | A rotation plan for the pinned TLS root before 2032 | M  |
|  components/src/credentialer.rs | no rate limit or lockout; a guess costs one Argon2id derivation | credentialer: rate limit and lockout on verify | M  |
|  components/src/fs_file_caretaker.rs, fs_nameset_caretaker.rs, fs_subtree_caretaker.rs | no host fuzz target reaches the caretakers' decode (inline in EL0 binary) | Extract caretaker decode into a host crate and fuzz it | M  |
|  components/src/login.rs | user_timetable_keeper failing half way leaves splits in the user's budget; measured-boot verdict spelled locally, not via measured_boot::verdict | login: give back splits on keeper failure; use measured_boot::verdict | S  |
|  components/src/login_audit_receiver.rs | audit trail drained and discarded; no recorder (login_audit_recorder rename promised); roadmap 0480 (log channel per request) is REFUSED, so nothing carries it | login_audit_recorder: keep and surface the login audit | M  |
|  components/src/user_timetable_keeper.rs | durable timetable holds no clock, so every calendar line in a durable schedule never fires | Durable schedules get a clock | M  |
|  crates/login_contract/src/lib.rs | a terminal holder that exits without logging out wedges login until it restarts | login: recover when the terminal holder dies | S  |
|  cryptography_exerciser/src/main.rs (coverage bullet) | No ECDSA or RSA signature vector, so certificate verification is unexercised. | Signature vectors in the cryptography exerciser | S  |
|  cryptography_exerciser/src/main.rs (feature-detection bullet) | A crate that picks an implementation by asking the CPU dies in ring 3 on x86_64 (chacha20 AVX2); nothing warns. | A gate that finds run-time CPU feature detection in userspace crates | M  |
|  design/decisions/0060-fuzzing-the-parsers.md | Fuzz coverage is stated not measured, and the corpus is not kept so each CI sweep restarts shallow. | Measure fuzz coverage and persist the corpus | M  |
|  design/decisions/0250-images-name-one-package-index-and-the-bytes-may-live-anywhere.md (CA, root rotation, renewal bullets) | Nothing checks the index host's chain to ISRG X1, the X1 cross-sign expiry (2032-09-02) that stops a pinned client, or the nifeos.org renewal (2027-10-03). | An expiry tripwire for the pinned root and the domain | S  |
|  design/roadmap/0142-a-text-display-worth-living-in.md | Vendored fonts and the 140 KB atlas are not checked against upstream's release archive; registration is by hand | Check vendored fonts against upstream archive | S  |
|  design/roadmap/0155-user-provisioning.md | No real path for an operator to hold the tool's capabilities for any identity but the demo one; the directory capability is the whole file-service root, no principal-tree parent (merged two) | Provisioning for real identities under a principal tree | M  |
|  design/roadmap/0228-close-what-we-claim-is-closed.md | nothing checks that the closed counter bits (PMUSERENR_EL0, CR4.PCE) stay closed; a later write elsewhere goes unseen | A check that the closed counter bits stay closed | S  |
|  design/roadmap/0246-the-refusal-path-of-measured-boot-is-tested-by-nothing.md | login.rs runs verify_in_manifest and folds three outcomes into None; the host-tested verdict() is not used there | login uses the tested measured-boot verdict | S  |
|  design/roadmap/0288-host-tests-that-assume-an-aarch64-host.md | committed seed bytes are not checked against the generator that claims to produce them (fuzz/seeds/README BUGS) | Check seeds against their generator | S  |
|  design/roadmap/0288-host-tests-that-assume-an-aarch64-host.md | riscv64 fuzz seed never parsed by anything, only compared byte-wise to the aarch64 seed | Parse every architecture's fuzz seed | S  |
|  design/roadmap/0303-x86-64-fs-disk.md | an unclaimed virtio function is refused one DMA at a time, not confined once; proposal `an-unclaimed-function-behind-the-iommu.md` never became a milestone (0436 lists the dangling path) | Put every enumerated PCI function in an empty IOMMU domain at boot | M  |
|  design/roadmap/0442-a-crypto-provider-that-builds-on-bare-metal.md | TLS handshake cost on boards with no hardware crypto, and the portable-code penalty on aarch64/riscv64, are unmeasured | Measure a TLS handshake on each architecture | M  |
|  fixtures/src/confined_syscall_fuzzer.rs | GO never fuzzed; no thread or timer objects made | Fuzzer exercises GO and deterministic thread/timer | M  |
|  notes/cryptography-provider.md | Nothing measures timing, so soft fallbacks (all that runs on x86_64) could be non-constant-time and pass every vector | Measure constant-time behavior of the soft crypto fallbacks | M  |
|  notes/futex.md | Futex word read/clear store walks page tables without the revocation lock; a proposal exists (`proposals/a-futex-word-is-walked-and-touched-under-the-revocation-lock.md`) but is not a milestone | Walk and touch the futex word under the revocation lock | S  |
|  notes/fuzzing.md | GPT target fixes 512-byte blocks and barely reaches `check_backup` (head/tail split "worth doing; not done") | GPT fuzz: 4K blocks and a real backup header | S  |
|  notes/fuzzing.md | `device_tree_blob_walk`, `elf_parse`, GPT target assert nothing beyond "returned"; wrong answers invisible | Give the asserting-nothing fuzz targets a property | M  |
|  notes/fuzzing.md | Mutational fuzzing cannot reach deep DTB nesting; needs a grammar-based generator and a fuzzer-chosen needle | Grammar-based fuzz input for `device_tree_blob_walk` | M  |
|  notes/interactive-stack-swap.md | `OPERATION_QUIESCE` is served to any holder of the terminal endpoint; an app can freeze its terminal | Restrict `OPERATION_QUIESCE` to the supervisor | M  |
|  notes/kernel-proofs.md | `user/` parsers of untrusted input and `xtask` are out of the prover's reach | Bring `user/` parsers under Kani | L  |
|  notes/kernel-proofs.md | `kernel/src/arch/` is almost wholly unproved; the asm boundary is untouched | Kani harnesses across the arch asm seam | L  |
|  notes/kernel-proofs.md | `x86_64/machine.rs` ACPI parsing and `crates/machine_discovery` have no harnesses and no verify row (0423 covers the pointer reader only) | Prove machine_discovery and the x86_64 ACPI parse | M  |
|  notes/kernel-proofs/stubbing-an-instruction.md | x86_64 stub harness is `unfalsified` (no x86_64 Kani host); `sbi::call` has no harness and needs hart-mask/cpu::id models | Falsify the x86_64 stub harness and model sbi::call | M  |
|  notes/ntlm.md | `NTLM_PROOF` has no rate limit and no incidental one | Rate-limit NTLM proofs | S  |
|  notes/register-of-measures/unsafe-series.md (also notes/project-metrics/limitations.md, notes/register-of-measures.md) | `patches/std-nife/overlay/` (37 unsafe blocks, 15 without a lint-form SAFETY comment) is outside the unsafe census and outside clippy; the entry says it wants a lane | Bring the std platform overlay under the unsafe census and `undocumented_unsafe_blocks` | M  |
|  notes/session-identity.md (hook and permission bullets) | No harness-neutral hook puts a session on the App identity automatically; nothing checks App permissions against the documented table | Wire the App identity hook for Claude Code and opencode; check granted permissions | M  |
|  notes/x86-port.md | `rdtsc` is readable from ring 3 by inheritance, not decision; closing needs a coarse time source first | Decide and close ring-3 `rdtsc` on x86_64 | M  |
|  roadmap 0595 | installer does not check entropy readiness verdict; a condemned source reaches progenitor | The installer checks the entropy verdict | S  |
|  roadmap 0806 | login blames its stuck terminal on "no wait-any" though notification objects landed | Login uses notification objects | M  |
|  roadmap 0823 | audit candidate list came from grep; a server naming windows another way is missed | Audit window servers from the capability-receive graph | M  |
|  roadmap 0849 | ad hoc script path (subset of shell's grants, no block) not measured against installed path; milestone named "candidate" | The ad hoc script path is measured | M  |
