# Done when

An appendix to [the milestone](../1112-user-mode-runtime-outside-review.md).

- `MappedWindow::read` and `write` accept only the crate's sealed integer types, `check` refuses a
  misaligned offset, and a host test exercises both refusals.
- No barrier arm in the crate carries `nomem`, and each barrier's comment says why.
- `script/crate-standalone user_mode_runtime` passes its manifest, files and citations steps, and
  the externalized-crates lint's ceiling for the crate is zero.
- `cargo test` runs on the host with tests for the window, the frame, `Delivered::decode`, the
  three child stubs decoded against `abi`, the nanosecond conversion and the `link.ld` base. The
  coverage exemptions file names each file that can run only on a target, with its reason.
- `link.ld` reaches every consumer through the crate's `build.rs`, no build script in nife names
  `crates/user_mode_runtime/link.ld`, and the base test lives in this crate.
- `cntfrq_checked` returns `None` for a zero register on aarch64.
- The aarch64 child stub derives every immediate from `abi`.
- The sources carry no section, milestone or `notes/` citation, and the two notes live in the
  repository's `docs/`.
- The template's acronym gate passes with a ceiling of zero.
- `Frame` has `len`, and `payload_read` clamps to `DATA_MAXIMUM`.
- The rename lane has landed the names Decision 1 decides, and `script/names
  user_mode_runtime` prints `ratified` for every public name.
- `cargo check`, `cargo clippy -- -D warnings` and `cargo doc` with warnings denied are clean on
  all three targets and on the host at the commit the move cites.
