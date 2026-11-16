# P1: before the tag

An appendix to [the milestone](../1111-usb-outside-review.md).

## P1: before the first release

4. No README, no license files, no changelog, no stranger test.
   Finding: the crate directory holds `Cargo.toml`, `src/` and `falsifications/`. The scaffold
   expects `README.md` (badges, one line, EXAMPLES, "What is proven", "Building and testing",
   "Running the proofs"), `LICENSE-MIT`, `LICENSE-APACHE`, `CHANGELOG.md`, `SECURITY.md`,
   `CONTRIBUTING.md`, `CODEOWNERS`, `_typos.toml`, `deny.toml`, `rustfmt.toml` and
   `docs/stranger-test.md`. The crate-level documentation is good and can seed the README, but a
   crates.io reader never sees `lib.rs`.
   Evidence: `ls crates/usb`; `crate-workflows/scaffold/README-crate.md`.
   Change: fill in the scaffold. The README's first line says "host side" and "boot keyboard" so
   the `usb-device` and `usbd-hid` crowd leave at once. "What is proven" states the three bounds
   (25 configuration bytes, 20 events, 20 header bytes) and names what lies outside them (item 7).
   A "Where the table comes from" section restates the provenance position of milestone 1019 (the driver crates carry facts from their sources, not their text), so a
   docs.rs reader sees the license reasoning without opening a module. Then run the stranger test.
   Door: two-way. Effort: M.

5. The crate has no review package, no index row, and no ratification record for its public names.
   Finding: `notes/crate-reviews.md` has no `usb` row. The xHCI package covers the crate in one
   paragraph that records no name, no API summary, no proof list and no `script/crate-standalone`
   result for it. `script/names` lists one `Name:` block for the crate, so the eight public
   functions, four public types, one enum and nineteen public constants have no `provisional`,
   `recorded` or `ratified` state anywhere. Milestone 820 (`extensible_host_controller_interface`: proven xHCI logic, released on its own) moves the crate on the strength of a
   review that was never written.
   Evidence: `notes/crate-reviews.md` line 72 (the xHCI row); `notes/crate-reviews/README.md`
   ("a new package is a new file plus a row in the index").
   Change: add a `notes/crate-reviews/universal-serial-bus.md` in the index's package shape. It
   holds the name and version, API, proofs, dependencies (none), the naming-rules table with this
   review's helper results, and the Decision 1 table as proposed names. It also holds provenance
   (milestone 1019 item 2), the standalone result, and the recommendation. Add one row in the
   index.
   Door: two-way (a note); the names it tables are Decision 1's, decided provisionally. Effort: M.

6. The usage table stops at 0x63, so a non-US keyboard loses keys silently.
   Finding: `evdev_code` answers `None` for several usages. One is usage 0x64, the ISO key beside
   the left shift on UK, German and most European layouts (`Non-US \ and |`, `KEY_102ND`). The
   others are 0x65 (`Application`, `KEY_COMPOSE`), 0x67 (`Keypad =`, `KEY_KPEQUAL`), 0x68 to 0x73
   (`F13` to `F24`), and 0x87 to 0x8b (the Japanese `International1` to `International5` keys:
   `KEY_RO`, `KEY_KATAKANAHIRAGANA`, `KEY_YEN`, `KEY_HENKAN`, `KEY_MUHENKAN`). Verified by the scratch test: `changes` emits nothing
   for a press of 0x64. The usage names and `KEY_*` matches here come from memory. A lane must read
   them    from the two sources the module names before it adds any row. The documentation says the table
   covers 0x04 through 0x63 by design, so this is a scope choice and not a bug, but a stranger with
   a German keyboard will file it as one.
   Evidence: `crates/usb/src/boot_keyboard.rs` lines 93 to 96 and 121 to 222.
   Change: extend `TABLE` through at least 0x8b. Take each row from the Usage Tables and the
   header exactly as the existing rows came, with a row comment per entry. Leave a usage with no
   `KEY_*` match as a hole the comment names. The table becomes `[Option<u16>; N]` or keeps `u16`
   with 0 as "no code"; the first is clearer. Rerun the Linux cross-check and update the
   fingerprint, as `the_table_matches_its_linux_cross_check` instructs. Add 0x64 and 0x87 to
   `the_usage_table_matches_the_keycaps`.
   Door: two-way (adding rows is compatible). Effort: S.

7. The proofs leave the two-interface paths and the event order unproven, and the documentation
   does not say so.
   Finding: the walk harness stops at 25 bytes, the shortest configuration that reaches an
   endpoint. The paths that need two interfaces (the `without_endpoint` refusal, a candidate reset
   by a later interface, a `too_small` endpoint followed by an acceptable one) need at least 34
   bytes and are outside the proof. The `without_endpoint` path has no unit test either: the scratch
   test is the first time it ran, and it answered `NoInterruptIn { interface: 0 }` correctly. The
   report harness proves the bound of 20 events and nothing about their order or their
   correspondence to the two reports. The crate documentation says the harnesses "prove that for
   every input up to a bound" without naming the bounds.
   Evidence: `crates/usb/src/verification.rs` lines 14 to 30 and 44 to 54; `crates/usb/src/lib.rs`
   line 7; `crates/usb/src/descriptor.rs` lines 200 to 206 and 263 to 266.
   Change: a unit test for the `without_endpoint` path (the scratch configuration: a boot
   interface with zero endpoints, then a non-boot HID interface with one). A fourth harness shaped
   rather than fully symbolic. It fixes a 9-byte header and two 9-byte interface descriptors, and
   leaves the class, subclass, protocol and alternate-setting bytes symbolic. It adds two symbolic
   7-byte endpoint descriptors. The solver then reaches the second interface at under 50 bytes
   without the 177-second cost growing past the budget (unverified; measure). A fifth harness, or an assertion
   in the third, that every event's code is a modifier code or `evdev_code` of a usage present in
   exactly one of the two reports, and that modifier events precede the rest. Each new harness
   arrives with a falsification patch. One paragraph in the README and in `lib.rs` naming the three
   bounds and the two things outside them.
   Door: two-way. Effort: M.

8. A descriptor of a known type that falls short of its layout leaves the previous candidate
   open.
   Finding: the walk matches `INTERFACE if len >= INTERFACE_LEN` and `ENDPOINT if len >=
   ENDPOINT_LEN`; a type-4 descriptor of 2 to 8 bytes falls to the `_ => {}` arm and does not reset
   `candidate`. Scratch test: a boot interface with no endpoint, then a 4-byte interface
   descriptor for interface 1, then interface 1's endpoint 0x82, answers `Ok` with `interface: 0,
   endpoint: 0x82`. The driver would then send `SET_PROTOCOL` to interface 0 and read reports from
   interface 1's endpoint. Only a malformed device reaches this, and the module's stated policy is
   to refuse rather than trust; a short header is already `NotThatDescriptor`. Linux skips such a
   descriptor with a notice and does not reassign it; OpenBSD was not checked for this case.
   Evidence: `crates/usb/src/descriptor.rs` lines 222, 232 and 254 to 256; the test
   `a_short_descriptor_is_skipped_and_two_bytes_is_enough` (lines 477 to 491) pins the current
   behavior.
   Change: answer `Malformed { at }` for a type-4 or type-5 descriptor shorter than its layout.
   Rewrite the test to assert the refusal and keep its two-byte vendor descriptor case. A
   maintainer can loosen the stricter reading later.
   Door: two-way. Effort: XS.

9. A usage duplicated in the earlier report releases twice.    Finding: presses deduplicate
   (`!after.keys[..i].contains(&usage)`); releases do not.
   Scratch test: `before = [0x04, 0x04, 0, 0, 0, 0]`, `after = RELEASED` emits `(30, false)` twice.
   A consumer that counts held keys goes negative; the bound of 20 still holds.
   Evidence: `crates/usb/src/boot_keyboard.rs` lines 252 to 259 against 260 to 269.
   Change: the same `before.keys[..i]` guard on the release loop, and a test beside
   `a_duplicated_usage_presses_once` named `a_duplicated_usage_releases_once`.
   Door: two-way. Effort: XS.

10. No fuzz target, on the one crate in the pair whose whole input is a stranger's bytes.
    Finding: the template's `fuzz.yml` and weekly fuzz run exist for crates with no `unsafe`; this
    crate has none and no `fuzz/` directory. Kani covers 25 bytes; a fuzzer covers a real
    composite keyboard's 59 bytes and more.
    Evidence: `ls crates/usb`; `crate-workflows/scaffold/caller-workflows/fuzz.yml`.
    Change: two targets, `find_boot_keyboard` over arbitrary bytes and `changes` over two parsed
    reports asserting the same three properties the harnesses assert plus the count bound; the
    composite and mouse-first configurations from the tests as the seed corpus.
    Door: two-way. Effort: S.
