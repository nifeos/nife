# P2: later

An appendix to [the milestone](../1089-environment-contract-outside-review.md).

9. `Refused` implements neither `Display` nor `core::error::Error`.
   Finding: `src/lib.rs` line 170 to `:178`. A caller cannot use `?` into a `Box<dyn Error>` or print
   the refusal without matching. `core::error::Error` is stable in `core`, so `no_std` is no bar.
   Change: both impls, each variant's message naming the key and the domain constant.
   Door: two-way, additive. Effort: S.

10. Documentation that disagrees with the code. Finding: `MAXIMUM_TIME_ZONE_BYTES`'s doc says
    the longest IANA identifiers "run to about 33 bytes". The longest is 32, a backward link, and
    the longest canonical names are 30 in this host's tzdata. `notes/env-config.md` calls the crate
    name provisional while the crate's `Name:` block reads ratified.
    `system_tests/src/user/printenv_tests.rs` line 89 cites a test
    `a_zeroed_page_reads_as_no_configuration` that does not exist (the test is
    `a_zeroed_frame_carries_no_configuration`).
    `notes/bugs-without-milestones/kernel-abi-and-component-plumbing-1.md` line 13 says no manifest field
    declares the page; `grant_plan::Manifest::configuration` does. The `KNOWN_LOCALES` doc does not
    say that the page spells the codeset `UTF-8` while glibc's `locale -a` prints `utf8` (whether
    `setlocale` normalizes the two stays unverified here). Change: the five sentences. The nife-side
    rows belong to the move lane's pull request in nife. Door: two-way. Effort: S.

11. The reader's constructor takes a raw address where the family's newest member takes bytes.
    Finding: `ConfigPage::new(va: u64)` is `unsafe`, stores a raw pointer and is `Copy`, `Send` and
    `Sync` by `unsafe impl`. The whole burden sits on the caller's promise about a mapping.
    `program_argument_contract::ArgPage::parse(&[u8; PAGE_BYTES])` is safe, and every host test and
    doctest in this crate already holds a `[u8; PAGE_BYTES]` it casts to an address. The `Send` and
    `Sync` argument is sound as written (one writer, finished before the first reader, immutable
    bytes thereafter) and `notes/unsafe-obligations/ceilings.md` records it. Change: none to the
    family shape, which `clock_contract` and `counter_frequency_contract` share and whose consumers
    hold an address, not a slice. Recommended: a safe `fn read(page: &[u8; PAGE_CONTENT_BYTES]) ->
    ConfigurationView<'_>` or three free functions over a borrowed array, so a host program and the
    tests never write `unsafe`, with the address constructor kept for the mapped case. The
    maintainer can decide it. Door: two-way, additive. Effort: S.

12. The curated lists have no stated growth path for a stranger.
    Finding: the crate's `BUGS` says the lists "grow on demand" and points at §46 (thin primitives or whole subsystems; we write everything in between); a stranger with
    `TZ=Europe/Madrid` finds it refused and no instruction. Twenty zones, twelve locales and eleven
    terminal types is a demonstration-sized set.
    Change: the README's `BUGS` names the policy (a real IANA, glibc or terminfo name, added by a
    pull request that cites the source) and the alternative refused (compiling the full tables in,
    a dependency decision). No code change.
    Door: two-way. Effort: S, inside item 6.
