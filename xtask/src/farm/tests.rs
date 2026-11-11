use super::exerciser::exerciser_target_directory;
use super::*;

/// The falsification of the shared link: two worktrees must never compute one toolchain name,
/// even when their directories share a basename, and no worktree may compute the legacy
/// `nife-dev` that lanes cut before 2026-10-10 still relink. Under the old code both roots
/// below answered `nife-dev`, which is the whole churn this replaces.
#[test]
fn two_worktrees_never_share_a_toolchain_name() {
    let a = toolchain_name_for(Path::new("/w/nife-worktrees/lane-a"));
    let b = toolchain_name_for(Path::new("/w/nife-worktrees/lane-b"));
    let main = toolchain_name_for(Path::new("/w/nife"));
    let clone = toolchain_name_for(Path::new("/tmp/stranger/nife"));
    for n in [&a, &b, &main, &clone] {
        assert!(n.starts_with(TOOLCHAIN_PREFIX), "{n}");
        assert_ne!(n, "nife-dev");
        assert!(
            n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "{n}"
        );
    }
    assert_ne!(a, b);
    assert_ne!(main, clone, "same basename, different checkout");
    assert!(a.starts_with("nife-dev-lane-a-"), "{a}");
    assert_eq!(
        a,
        toolchain_name_for(Path::new("/w/nife-worktrees/lane-a")),
        "stable across runs"
    );
}

/// The sweep removes a per-worktree link whose farm is gone and nothing else: not a live
/// worktree's link, not the legacy `nife-dev` even when it dangles, and not a real install.
#[cfg(unix)]
#[test]
fn the_sweep_removes_only_dangling_per_worktree_links() {
    use std::os::unix::fs::symlink;
    let base =
        std::env::temp_dir().join(format!("nife-toolchain-sweep-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir, live) = (base.join("toolchains"), base.join("live-farm"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(&live).unwrap();
    std::fs::create_dir_all(dir.join("nightly-2026-10-10-aarch64-apple-darwin")).unwrap();
    let gone = base.join("pruned-farm");
    symlink(&live, dir.join("nife-dev-live-00000001")).unwrap();
    symlink(&gone, dir.join("nife-dev-pruned-00000002")).unwrap();
    symlink(&gone, dir.join("nife-dev")).unwrap();
    symlink(&gone, dir.join("other-dangling")).unwrap();

    assert_eq!(sweep_dangling_links(&dir), ["nife-dev-pruned-00000002"]);
    for kept in [
        "nife-dev-live-00000001",
        "nife-dev",
        "other-dangling",
        "nightly-2026-10-10-aarch64-apple-darwin",
    ] {
        assert!(
            std::fs::symlink_metadata(dir.join(kept)).is_ok(),
            "{kept} must survive"
        );
    }
    assert!(sweep_dangling_links(&dir).is_empty(), "idempotent");
    let _ = std::fs::remove_dir_all(&base);
}

/// After the hard-link clone, rustc and its driver must be independent inodes and nothing else
/// may be, since copying `libLLVM` would add 140 MB per worktree for no benefit. The
/// driver's name carries a per-nightly hash, which is the case an exact-name match would miss.
#[cfg(unix)]
#[test]
fn the_farm_copies_rustc_and_its_driver_and_links_the_rest() {
    use std::os::unix::fs::MetadataExt;
    let base = std::env::temp_dir().join(format!("nife-farm-anchor-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (real, farm) = (base.join("real"), base.join("farm"));
    let files = [
        ("bin/rustc", true),
        ("bin/cargo", false),
        ("lib/librustc_driver-3d7ac1467fbd50fa.dylib", true),
        ("lib/libLLVM.dylib", false),
        ("lib/librustc-nightly_rt.asan.dylib", false),
    ];
    for d in ["bin", "lib"] {
        std::fs::create_dir_all(real.join(d)).unwrap();
        std::fs::create_dir_all(farm.join(d)).unwrap();
    }
    for (f, _) in files {
        std::fs::write(real.join(f), f).unwrap();
        std::fs::hard_link(real.join(f), farm.join(f)).unwrap();
    }
    unlink_sysroot_anchors(&farm).unwrap();
    for (f, copied) in files {
        let ino = |root: &Path| std::fs::metadata(root.join(f)).unwrap().ino();
        assert_eq!(ino(&real) != ino(&farm), copied, "{f}");
        assert_eq!(
            std::fs::read_to_string(farm.join(f)).unwrap(),
            f,
            "{f} keeps its bytes"
        );
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// The exerciser build's target dir is pinned, and an export that would move it is named
/// rather than obeyed or ignored. An export that already equals the pin is the no-op case:
/// announcing a "correction" that changes nothing would train readers to skim past the real
/// one.
#[test]
fn the_exerciser_target_directory_is_pinned_and_an_override_is_named() {
    let (pinned, notes) = exerciser_target_directory(None);
    assert!(notes.is_empty(), "nothing exported, nothing to correct");
    assert!(pinned.ends_with("std_exerciser/target"));

    let (_, notes) = exerciser_target_directory(Some(&pinned.display().to_string()));
    assert!(
        notes.is_empty(),
        "an export equal to the pin is already right"
    );

    let (still, notes) = exerciser_target_directory(Some("/nonexistent/elsewhere/target"));
    assert_eq!(still, pinned, "the pin does not follow the export");
    assert_eq!(notes.len(), 1);
    assert!(
        notes[0].contains("/nonexistent/elsewhere/target"),
        "the note names what it overrode"
    );
}

/// The copy into the patched std sysroot must drop a `# Examples` section and keep everything
/// else, including the `text` diagrams the protocol crates lead with. The two cases worth
/// pinning are the ones a naive line filter gets wrong: a hidden doctest line (`# use ...`)
/// looks exactly like a heading, and a section that runs to the end of the doc block has no
/// following heading to stop at.
#[test]
fn the_std_copy_drops_doc_examples_and_keeps_the_prose() {
    let src = "\
//! A contract.
//!
//! ```text
//!   a diagram
//! ```
//!
//! # Examples
//!
//! ```
//! # use entropy_contract::GET;
//! assert_eq!(GET, 1);
//! ```
//!
//! # Nothing here transforms a byte
//!
//! Prose that must survive.

/// An item.
///
/// # Examples
///
/// ```
/// let x = 1;
/// ```
pub const GET: u64 = 1;
";
    let got = strip_doc_examples(src);
    assert!(got.contains("a diagram"), "text blocks are documentation");
    assert!(got.contains("# Nothing here transforms a byte"));
    assert!(got.contains("Prose that must survive."));
    assert!(got.contains("/// An item."));
    assert!(got.contains("pub const GET: u64 = 1;"));
    assert!(!got.contains("# Examples"));
    assert!(
        !got.contains("entropy_contract"),
        "the copy is an inner module of std, where that crate does not exist"
    );
    assert!(
        !got.contains("let x = 1;"),
        "a trailing section, with no heading after it to stop at"
    );
}

/// A crate root split into module files generates the text it did as one file: milestone 845 (the
/// filesystem-protocol crate root is split along its seams) relies on this to split
/// `filesystem_contract` without changing what std builds.
#[test]
fn an_out_of_line_module_is_inlined_as_it_was_written_inline() {
    let split = "/// Docs.\npub mod fs;\n\nmod private;\npub const PAGE: usize = 4096;";
    let file = "pub const OPEN: u64 = 1;\n\n#[cfg(test)]\nmod tests;\n";
    let got = inline_modules(split, |n| (n == "fs").then(|| file.to_string())).unwrap();
    assert_eq!(
        got,
        "/// Docs.\npub mod fs {\n    pub const OPEN: u64 = 1;\n\n    #[cfg(test)]\n    mod tests;\n}\n\n\
         mod private;\npub const PAGE: usize = 4096;"
    );
    assert!(inline_modules("pub mod gone;", |_| None).is_err());
}

/// A copied crate that has left the tree resolves to Cargo's checkout of its pin, read from the
/// package's `manifest_path`, and is stamped under a name that does not say where that checkout
/// is. A crate still in the tree resolves to `crates/<name>` and keeps its tree-relative name, so
/// today's stamp is the one the hard-coded paths gave.
#[test]
fn a_copied_crate_resolves_through_its_manifest_path() {
    let metadata = r#"{"packages": [
        {"name": "abi", "version": "0.1.0", "manifest_path": "/w/nife/crates/abi/Cargo.toml",
         "targets": [{"kind": ["lib"], "edition": "2024"}], "rust_version": null},
        {"name": "counter_frequency_contract", "version": "0.1.0",
         "manifest_path": "/home/u/.cargo/git/checkouts/counter_frequency_contract-1a2b/3c4d5e6/Cargo.toml",
         "description": "a \"quoted\" word, a é and a 😀"},
        {"name": "uuid", "version": "1.28.0", "manifest_path": "/r/uuid-1.28.0/Cargo.toml"}
    ], "workspace_root": "/w/nife", "version": 1}"#;
    let directories = package_directories(metadata, &["abi", "counter_frequency_contract"])
        .expect("both packages are listed");
    assert_eq!(directories["abi"], Path::new("/w/nife/crates/abi"));
    let checkout =
        Path::new("/home/u/.cargo/git/checkouts/counter_frequency_contract-1a2b/3c4d5e6");
    assert_eq!(directories["counter_frequency_contract"], checkout);
    let root = Path::new("/w/nife");
    assert_eq!(
        stamp_name(&directories["abi"].join("src/lib.rs"), root, &directories),
        "crates/abi/src/lib.rs"
    );
    assert_eq!(
        stamp_name(&checkout.join("src/lib.rs"), root, &directories),
        "counter_frequency_contract/src/lib.rs"
    );
}

/// The farm refuses to guess: a package that is not in the graph, or a name two packages share,
/// fails with the name in the message.
#[test]
fn a_missing_or_doubled_package_is_refused() {
    let one = r#"{"packages": [{"name": "abi", "manifest_path": "/a/abi/Cargo.toml"}]}"#;
    let missing = package_directories(one, &["abi", "clock_contract"]).unwrap_err();
    assert!(missing.contains("clock_contract"), "{missing}");
    let two = r#"{"packages": [{"name": "abi", "manifest_path": "/a/abi/Cargo.toml"},
                              {"name": "abi", "manifest_path": "/b/abi/Cargo.toml"}]}"#;
    let doubled = package_directories(two, &["abi"]).unwrap_err();
    assert!(doubled.contains("two packages are named abi"), "{doubled}");
    assert!(package_directories("{\"packages\": [", &["abi"]).is_err());
}

/// The JSON reader reads what `cargo metadata` prints: nesting, every escape, a surrogate pair,
/// numbers and the three words, and refuses trailing text.
#[test]
fn the_json_reader_reads_cargo_metadata_shapes() {
    let value = Json::parse(r#" {"a": [1, -2.5e3, true, false, null, {}], "b": "x\n\t\\\/A😀"} "#)
        .expect("valid JSON");
    assert_eq!(
        value.field("a"),
        Some(&Json::Array(vec![
            Json::Number("1".into()),
            Json::Number("-2.5e3".into()),
            Json::Bool(true),
            Json::Bool(false),
            Json::Null,
            Json::Object(vec![]),
        ]))
    );
    assert_eq!(
        value.field("b"),
        Some(&Json::String("x\n\t\\/A\u{1F600}".into()))
    );
    assert!(Json::parse("{} x").is_err());
    assert!(Json::parse(r#""\ud83d""#).is_err());
}

/// In this tree every copied file resolves, through the workspace's own `cargo metadata`, to a
/// file that exists, so `std-src` can copy each one.
#[test]
fn every_copied_file_resolves_in_this_workspace() {
    let directories = copied_package_directories()
        .as_ref()
        .expect("cargo metadata lists every copied package");
    for copied in &COPIED_FILES {
        let file = directories[copied.package].join(copied.file);
        assert!(file.is_file(), "{}", file.display());
    }
}
