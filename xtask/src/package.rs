//! **`cargo xtask package`**: build a package from a reviewed recipe, and say what its digest is.
//!
//! The producer half of milestone 198 (a package manager, and the trivial install that makes a
//! second customer possible). DECISIONS §195 (a reviewed recipe vouches for a package) puts a
//! package's digest in a version-controlled recipe changed by human review; this turns such a
//! recipe into the one archive file DECISIONS §197 (a package is one archive file) rules a package
//! is. nife cannot build software, so a package is produced here, on a host with a
//! cross-toolchain, and a recipe names an architecture: three triples, three packages.
//!
//! # EXAMPLES
//!
//! ```text
//! $ cargo xtask package packages/uptime.recipe.toml
//! uptime 0.1.0 aarch64, 2 members, 4600 bytes
//!   uptime          4528 bytes  sha256:7d8e...c1
//!   uptime.licence    68 bytes  sha256:a3f0...9e
//! wrote target/packages/uptime-0.1.0-aarch64.nifepkg
//! digest sha256:9f2c...4b  (recorded in the recipe, and it matches)
//! package: PASS
//! ```
//!
//! A recipe with no `digest` key builds and prints the digest to paste in; one whose `digest`
//! disagrees with the bytes fails, because the reviewed line decides whether the bytes may run.
//!

//! # The recipe format
//!
//! TOML (calef, 2026-09-27; JSON and YAML refused). The extension `.recipe.toml` and every key
//! name are provisional.
//!
//! ```toml
//! name = "uptime"              # required, at most package_archive::NAME_LEN bytes
//! version = "0.1.0"            # required
//! architecture = "aarch64"     # required: aarch64, riscv64 or x86_64
//! digest = "sha256:9f2c..."    # optional: sha256: and 64 hex characters, checked against
//!                              # what was built
//!
//! [[member]]                   # members in order: the package's bytes depend on it
//! program = "uptime"           # a built ELF for that architecture, resolved from target/
//! as = "uptime"                # optional: the member's name in the package, when the ELF it
//!                              # packs is built under another name
//!
//! [[member]]
//! name = "uptime.licence"      # any file, by a path relative to the repository root
//! file = "LICENSE-MIT"
//! ```
//!
//! `program` spares a recipe the target triple and cargo profile, which are facts about this
//! checkout. An unknown key is refused, so a misspelt `digest` cannot pass review as a recipe that
//! records none. The parser is the host-only `toml` crate; the target reads the image's catalog
//! (`name sha256:<hex>` lines, below), never a recipe.
//!

//! # BUGS
//!
//! - **It builds nothing.** A `program` whose ELF is not in `target/` is an error naming the file,
//!   not a cargo invocation, so the tool cannot hide which binary it packed.
//! - **The catalog is a file in `target/` and an archive entry**, not a repository index. §195's
//!   per-source trust needs a catalog per source; the image's own is the only source there is.
//! - **A recipe cannot say where its source came from**: no upstream URL or tarball digest,
//!   because the only packages built here come from this repository.
//!
//! Installing a fetched package happens on the target; see notes/packages.md.

use std::path::PathBuf;

use package_archive::{Attributes, Package, package_size, sha256, write_package};

use crate::host::workspace_root;
use crate::{RISCV_TARGET, TARGET, X86_TARGET, profile_dir};

/// Where built packages land. Under `target/` because a package is an artifact, and because
/// nothing in this tree is ready to publish one.
const OUTPUT: &str = "target/packages";

/// A recipe, parsed. The parser is a pure function over the text, so a test can call it with a
/// string literal, which is the shape every parser in this tree that is worth testing has.
#[derive(Debug, PartialEq, Eq)]
struct Recipe {
    name: String,
    version: String,
    architecture: String,
    /// `(member name, where its bytes come from)`, in the order the recipe lists them, because the
    /// package's bytes are a function of that order and a reviewed digest depends on it.
    members: Vec<(String, Source)>,
    /// Read with `measured_boot::parse_digest`, the one parser every reader of a digest's text
    /// shares, so a recipe cannot record a digest the catalog's readers would refuse.
    digest: Option<measured_boot::Digest>,
}

#[derive(Debug, PartialEq, Eq)]
enum Source {
    /// A built ELF for the recipe's architecture, resolved from `target/`.
    Program(String),
    /// **A `std` program `cargo xtask std-exerciser` built** for the recipe's architecture,
    /// resolved from `std_exerciser/target/<arch>-unknown-nife/release/` (milestone 892 (a std
    /// program at the prompt holds the network and a resolver)). Optional where the exerciser is:
    /// the image's catalog leaves out a recipe whose `std` program was not built, so its package is
    /// in the image exactly when the gate's lines for it can run. Key name provisional.
    StdProgram(String),
    /// A path relative to the repository root.
    File(String),
}

/// `cargo xtask package <recipe>`: build it, check it against the recipe, and report.
pub(crate) fn package(recipe_path: Option<String>) -> bool {
    let Some(recipe_path) = recipe_path else {
        eprintln!("usage: cargo xtask package <recipe>");
        return false;
    };
    let root = workspace_root();
    let text = match std::fs::read_to_string(root.join(&recipe_path)) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("package: could not read {recipe_path}: {error}");
            return false;
        }
    };
    let built = match build(&root, &text) {
        Ok(built) => built,
        Err(complaint) => {
            eprintln!("package: {recipe_path}: {complaint}");
            return false;
        }
    };
    let parsed = Package::parse(&built.file).expect("build() read it back already");
    println!(
        "{} {} {}, {} members, {} bytes",
        parsed.name(),
        parsed.version(),
        parsed.architecture(),
        parsed.len(),
        built.file.len()
    );
    for index in 0..parsed.len() {
        let name = parsed.member_name(index).unwrap_or("");
        let len = parsed.member(index).map(<[u8]>::len).unwrap_or(0);
        let digest = parsed.member_digest(index).unwrap_or_default();
        println!("  {name:<24} {len:>9} bytes  {}", labeled(&digest));
    }
    if built.recorded {
        println!(
            "digest {}  (recorded in the recipe, and it matches)",
            built.digest
        );
    } else {
        println!(
            "digest {}  (the recipe records none; review it and add it)",
            built.digest
        );
    }
    match write_out(&root, &built) {
        Ok(written) => {
            println!("wrote {}", relative(&root, &written));
            println!("package: PASS");
            true
        }
        Err(complaint) => {
            eprintln!("package: {complaint}");
            false
        }
    }
}

/// A package built from a recipe and checked, not yet written anywhere.
pub(crate) struct Built {
    /// `name-version-architecture`, the file's stem and its catalogue name.
    pub(crate) stem: String,
    pub(crate) file: Vec<u8>,
    /// SHA-256 over the whole file, in its text form, `sha256:<64 lowercase hex>`.
    pub(crate) digest: String,
    /// Whether the recipe recorded a digest (which, if this returned at all, matched).
    recorded: bool,
}

/// Build the package a recipe describes, read it back with the target's parser, and check it
/// against the recipe's recorded digest. Writes nothing: a refusal here leaves the disk as it was.
pub(crate) fn build(root: &std::path::Path, text: &str) -> Result<Built, String> {
    let recipe = parse_recipe(text)?;

    // Read every member first, so a missing file is reported before anything is written.
    let mut bytes = Vec::new();
    for (name, source) in &recipe.members {
        let path =
            resolve(root, &recipe.architecture, source).map_err(|c| format!("{name}: {c}"))?;
        // **A program is packed stripped**, the same bytes the image packs (`read_stripped`).
        // Unstripped, `uptime` was 881,152 bytes against a job region of 40 pages (160 KiB): an
        // installed program is bytes something on the target must read into memory to build a
        // process from, and debug sections nobody on the target reads would have cost more than the
        // program. Found by rung 3a's consumer lane, 2026-09-24.
        let content = match source {
            Source::Program(_) | Source::StdProgram(_) => {
                crate::inspect::read_stripped(&path.display().to_string())
            }
            Source::File(_) => std::fs::read(&path),
        }
        .map_err(|e| format!("{name}: could not read {}: {e}", path.display()))?;
        bytes.push(content);
    }
    let members: Vec<(&str, &[u8])> = recipe
        .members
        .iter()
        .zip(&bytes)
        .map(|((name, _), content)| (name.as_str(), content.as_slice()))
        .collect();

    let attributes = Attributes {
        name: &recipe.name,
        version: &recipe.version,
        architecture: &recipe.architecture,
    };
    let mut file = vec![0u8; package_size(&members)];
    write_package(&attributes, &members, &mut file).map_err(|e| format!("refused: {e:?}"))?;

    // Read the package back with the target's own parser before anything is written. The producer
    // and the consumer share one definition of the format, and this is where that stops being a
    // claim: a writer that could emit a file its reader refuses would ship one.
    let parsed =
        Package::parse(&file).map_err(|e| format!("wrote a file its own reader refuses: {e:?}"))?;
    if let Err(error) = parsed.verify() {
        return Err(format!("member {} does not match its digest", error.index));
    }

    let measured = sha256(&file);
    let digest = labeled(&measured);
    // **The recorded digest is checked before anything is written**, which is the order §195 asks
    // for even though it costs a rebuild to find out. A package whose bytes do not reproduce the
    // reviewed line is one nothing accepts, so leaving it on disk beside a catalogue entry
    // vouching for it would be the tool disagreeing with itself.
    if let Some(recorded) = &recipe.digest
        && *recorded != measured
    {
        return Err(format!(
            "the recipe records {}, these bytes are {digest}: a rebuild that does not \
             reproduce the reviewed digest is the failure DECISIONS §195 exists to make visible; \
             nothing was written",
            labeled(recorded)
        ));
    }
    Ok(Built {
        stem: format!("{}-{}-{}", recipe.name, recipe.version, recipe.architecture),
        file,
        digest,
        recorded: recipe.digest.is_some(),
    })
}

/// Write a built package under `target/packages/` and its line into the host's catalogue there.
fn write_out(root: &std::path::Path, built: &Built) -> Result<PathBuf, String> {
    let output = root.join(OUTPUT);
    std::fs::create_dir_all(&output)
        .map_err(|e| format!("could not create {}: {e}", output.display()))?;
    let written = output.join(format!("{}.nifepkg", built.stem));
    std::fs::write(&written, &built.file)
        .map_err(|e| format!("could not write {}: {e}", written.display()))?;
    // The catalogue line is `measured_boot`'s manifest shape (a name, a space, 64 hex characters),
    // which is the format the progenitor already reads to decide whether a program may run. §195
    // makes the image's measurement table the first source of trust, so a package's entry looking
    // like an entry in that table is the point rather than a coincidence.
    let catalogue = output.join("catalog");
    append(&catalogue, &format!("{} {}\n", built.stem, built.digest))
        .map_err(|e| format!("could not write {}: {e}", catalogue.display()))?;
    Ok(written)
}

/// **The image's own package source** (rung 3a of milestone 198): build every recipe under
/// `packages/` whose architecture is `architecture`, write each package where the test's HTTP peer
/// serves it from, and return the catalogue lines for the image to carry.
///
/// This is DECISIONS §195's "the image's measured table becomes the first source" made literal: the
/// catalogue goes into the initial ramdisk archive *before* the measurement table is computed, so
/// the kernel's trust root vouches for the catalogue and the catalogue vouches for the package. A
/// package fetched over plain HTTP is then checked against a digest that never crossed the network.
///
/// It also means every archive build runs the producer end to end, which is the gate
/// notes/packages.md's BUGS said nothing ran.
pub(crate) fn image_catalogue(architecture: &str) -> Result<String, String> {
    let root = workspace_root();
    let dir = root.join("packages");
    let mut recipes: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("could not read {}: {e}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".recipe.toml"))
        })
        .collect();
    recipes.sort();
    let mut catalogue = String::new();
    for path in recipes {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        let recipe = parse_recipe(&text)?;
        if recipe.architecture != architecture {
            continue;
        }
        // A `std` program that was not built leaves its package out, and the gate's lines for it
        // are skipped the same way (`Source::StdProgram`).
        if recipe.members.iter().any(|(_, source)| {
            matches!(source, Source::StdProgram(name) if !std_program_path(&root, architecture, name).exists())
        }) {
            continue;
        }
        let built = build(&root, &text).map_err(|c| format!("{}: {c}", relative(&root, &path)))?;
        write_out(&root, &built)?;
        catalogue.push_str(&format!("{} {}\n", built.stem, built.digest));
    }
    // **`jig` is handed the catalog in one page** (milestone 809 (the package client becomes a
    // program); `system_initializer`'s `copy_catalog`), and the progenitor refuses to start it with
    // a catalog that does not fit. So a catalog that outgrows the page fails here, at the build,
    // naming the size, rather than as a `jig` that will not spawn. Strictly less than a page,
    // because the zero byte after the text is where it ends.
    if catalogue.len() >= 4096 {
        return Err(format!(
            "the {architecture} catalog is {} bytes, and jig is handed it in one 4096-byte page",
            catalogue.len()
        ));
    }
    Ok(catalogue)
}

/// **The image's index sources** (milestone 801 (packages over the internet)): the text
/// `package_index::Sources` reads, packed as `package_index::SOURCES_ENTRY` above the measurement
/// table, as the catalog is, so the kernel's trust root vouches for where the image says its index
/// is and which root that index's TLS must chain to.
///
/// **Two texts, chosen by `NIFE_INDEX_SOURCES`.** Unset or `test`, the image names the test peer:
/// `helpers/tls-peer`, which every runner forwards at 10.0.2.9:8443 and which presents a chain for
/// `basalt.test` from the committed test authority. Every check boots that one, because no check
/// may depend on a host outside the runner. `basalt` names the production hosts instead,
/// `package_index::BASALT_SOURCES`: `basalt.nifeos.org`, served by GitHub Pages, then
/// `basalt.crickertech.com`, both pinned to ISRG Root X1 (§250 (an image names its distribution's package index); calef's 2026-10-12 (UTC)
/// ruling on where it is served). One text, not both: `package_index::Sources` holds one root, and
/// the two hosts chain to different ones. Milestone 909 (basalt composes the bootable image from
/// its packages) is where basalt asks for `basalt` on the images it publishes. Anything else is
/// refused rather than read as `test`, so a typo cannot ship a test image as a real one.
///
/// The address after the test host is there because a guest under QEMU's slirp asks the host's
/// resolver, which knows no `.test` name: milestone 892 (a std program at the prompt holds the
/// network and a name resolver) measured it.
///
/// The variable `NIFE_INDEX_SOURCES` and its two values are provisional names, minted 2026-10-13
/// (UTC) by lane `lane/basalt-on-pages-and-releases`.
pub(crate) fn image_index_sources() -> Result<String, String> {
    match std::env::var("NIFE_INDEX_SOURCES").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("test") => test_index_sources(),
        Ok("basalt") => checked_index_sources(package_index::BASALT_SOURCES.to_string()),
        Ok(other) => Err(format!(
            "NIFE_INDEX_SOURCES is {other:?}; it may be unset, `test` or `basalt`"
        )),
        Err(e) => Err(format!("NIFE_INDEX_SOURCES: {e}")),
    }
}

/// The test peer's sources, pinned to the committed test authority.
fn test_index_sources() -> Result<String, String> {
    let root = workspace_root().join("pinned_tls_client/fixtures/pinned-root.der");
    let der =
        std::fs::read(&root).map_err(|e| format!("could not read {}: {e}", root.display()))?;
    let hex: String = der.iter().map(|b| format!("{b:02x}")).collect();
    let text = format!(
        "# The image's package index (milestone 801): helpers/tls-peer under every runner, pinned\n\
         # to the test authority in pinned_tls_client/fixtures. See xtask's image_index_sources.\n\
         repository basalt.test 8443 rolling 10.0.2.9\n\
         root {hex}\n"
    );
    checked_index_sources(text)
}

/// Either text, read back with the target's own reader before it is packed, as a package is.
fn checked_index_sources(text: String) -> Result<String, String> {
    package_index::Sources::parse(&text)
        .map_err(|e| format!("the index sources this build wrote are refused: {e:?}"))?;
    if text.len() >= 4096 {
        return Err(format!(
            "the index sources are {} bytes, and jig is handed them in one 4096-byte page",
            text.len()
        ));
    }
    Ok(text)
}

/// Append a catalogue line, replacing any earlier line for the same name so a rebuild does not
/// leave two answers to one question.
fn append(path: &std::path::Path, line: &str) -> std::io::Result<()> {
    let name = line.split(' ').next().unwrap_or_default();
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut text: String = existing
        .lines()
        .filter(|kept| kept.split(' ').next() != Some(name))
        .map(|kept| format!("{kept}\n"))
        .collect();
    text.push_str(line);
    std::fs::write(path, text)
}

fn relative(root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// A digest in the text form every catalog, recipe and activation row shares, `sha256:<hex>`.
fn labeled(digest: &measured_boot::Digest) -> String {
    String::from_utf8_lossy(&measured_boot::digest_text(digest)).into_owned()
}

/// Where a member's bytes live on this host.
fn resolve(root: &std::path::Path, architecture: &str, source: &Source) -> Result<PathBuf, String> {
    match source {
        Source::File(path) => Ok(root.join(path)),
        Source::StdProgram(name) => {
            let path = std_program_path(root, architecture, name);
            if path.exists() {
                Ok(path)
            } else {
                Err(format!(
                    "{} is not built; `cargo xtask std-exerciser` builds it",
                    relative(root, &path)
                ))
            }
        }
        Source::Program(name) => {
            let triple = match architecture {
                "aarch64" => TARGET,
                "riscv64" => RISCV_TARGET,
                "x86_64" => X86_TARGET,
                other => return Err(format!("unknown architecture {other}")),
            };
            let path = root.join(format!("target/{triple}/{}/{name}", profile_dir()));
            if path.exists() {
                Ok(path)
            } else {
                Err(format!(
                    "{} is not built; build it first (cargo xtask initial-ramdisk-{})",
                    relative(root, &path),
                    if architecture == "aarch64" {
                        "aarch64"
                    } else if architecture == "riscv64" {
                        "riscv"
                    } else {
                        "x86"
                    }
                ))
            }
        }
    }
}

/// Where `cargo xtask std-exerciser` leaves the `std` program `name` for `architecture`.
fn std_program_path(root: &std::path::Path, architecture: &str, name: &str) -> PathBuf {
    root.join(format!(
        "std_exerciser/target/{architecture}-unknown-nife/release/{name}"
    ))
}

/// Parse a recipe, or say which key could not be read and why.
///
/// A pure function over the text, so the interesting half is host-testable in milliseconds, which
/// is what `AGENTS.md` asks of every parser in this tree.
fn parse_recipe(text: &str) -> Result<Recipe, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| e.message().to_string())?;
    let string = |key: &str| -> Result<Option<String>, String> {
        match table.get(key) {
            None => Ok(None),
            Some(toml::Value::String(value)) if !value.is_empty() => Ok(Some(value.clone())),
            Some(_) => Err(format!("{key} is a non-empty string")),
        }
    };
    for key in table.keys() {
        if !["name", "version", "architecture", "digest", "member"].contains(&key.as_str()) {
            return Err(format!("unknown key {key}"));
        }
    }
    let name = string("name")?.ok_or("no name")?;
    let version = string("version")?.ok_or("no version")?;
    let architecture = string("architecture")?.ok_or("no architecture")?;
    let digest = match string("digest")? {
        None => None,
        Some(digest) => Some(measured_boot::parse_digest(&digest).map_err(|why| {
            format!("a digest is sha256: and 64 hex characters ({why:?}: {digest})")
        })?),
    };

    let mut members = Vec::new();
    let entries = match table.get("member") {
        None => &Vec::new(),
        Some(toml::Value::Array(entries)) => entries,
        Some(_) => return Err("member is an array of tables, [[member]]".to_string()),
    };
    for (index, entry) in entries.iter().enumerate() {
        let number = index + 1;
        let Some(entry) = entry.as_table() else {
            return Err(format!("member {number} is not a table"));
        };
        let field = |key: &str| entry.get(key).and_then(toml::Value::as_str);
        for key in entry.keys() {
            if !["program", "std_program", "name", "file", "as"].contains(&key.as_str()) {
                return Err(format!("member {number}: unknown key {key}"));
            }
        }
        if field("as").is_some_and(str::is_empty) {
            return Err(format!("member {number}: `as` is a non-empty name"));
        }
        match (
            (field("program"), field("std_program")),
            field("as"),
            field("name"),
            field("file"),
        ) {
            ((Some(program), None), as_name, None, None) => {
                // `as` renames the member: the ELF is built under one name and packaged under
                // another, which is how `greeting` at 0.2.0 packs the second fixture's bytes.
                let named = as_name.unwrap_or(program);
                members.push((named.to_string(), Source::Program(program.to_string())));
            }
            ((None, Some(program)), as_name, None, None) => {
                let named = as_name.unwrap_or(program);
                members.push((named.to_string(), Source::StdProgram(program.to_string())));
            }
            ((None, None), None, Some(member), Some(path)) => {
                members.push((member.to_string(), Source::File(path.to_string())));
            }
            _ => {
                return Err(format!(
                    "member {number} is either `program` or `std_program` alone (optionally `as` a \
                     member name) or `name` with `file`"
                ));
            }
        }
    }

    Ok(Recipe {
        name,
        version,
        architecture,
        members,
        digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECIPE: &str = r#"
# a comment
name = "uptime"
version = "0.1.0"
architecture = "aarch64"

[[member]]
program = "uptime"

[[member]]
name = "uptime.licence"
file = "LICENSE-MIT"
"#;

    /// Both texts an image can carry fit the page `jig` reads them from, and they differ where they
    /// must: the test peer is reached at a fixed address under its own authority, the production
    /// host by name under ISRG Root X1. Read without `NIFE_INDEX_SOURCES`, which tests share.
    #[test]
    fn both_index_sources_pack_and_name_different_roots() {
        let test = test_index_sources().unwrap();
        let basalt = checked_index_sources(package_index::BASALT_SOURCES.to_string()).unwrap();
        let test = package_index::Sources::parse(&test).unwrap();
        let basalt = package_index::Sources::parse(&basalt).unwrap();
        assert!(matches!(test.root, package_index::Root::Der(_)));
        assert_eq!(basalt.root, package_index::Root::Named("isrg-root-x1"));
        let first = |s: package_index::Sources<'_>| {
            s.repositories().next().unwrap().repository.host.to_string()
        };
        assert_eq!(first(test), "basalt.test");
        assert_eq!(first(basalt), "basalt.nifeos.org");
    }

    #[test]
    fn a_recipe_reads_as_what_it_says() {
        let recipe = parse_recipe(RECIPE).unwrap();
        assert_eq!(recipe.name, "uptime");
        assert_eq!(recipe.version, "0.1.0");
        assert_eq!(recipe.architecture, "aarch64");
        assert_eq!(recipe.digest, None);
        assert_eq!(
            recipe.members,
            vec![
                ("uptime".to_string(), Source::Program("uptime".to_string())),
                (
                    "uptime.licence".to_string(),
                    Source::File("LICENSE-MIT".to_string())
                ),
            ]
        );
    }

    #[test]
    fn a_missing_field_is_named() {
        for (text, missing) in [
            ("name = \"uptime\"", "no version"),
            ("version = \"0.1.0\"", "no name"),
            ("name = \"uptime\"\nversion = \"0.1.0\"", "no architecture"),
        ] {
            assert_eq!(parse_recipe(text).unwrap_err(), missing);
        }
    }

    #[test]
    fn a_digest_that_is_not_a_digest_is_refused() {
        // The one field where a typo would otherwise pass review and then fail a build on another
        // host, which is the failure §195's whole arrangement is trying not to have. The digest
        // goes before the first [[member]], where TOML puts a top-level key.
        let with = |digest: &str| {
            RECIPE.replacen(
                "[[member]]",
                &format!("digest = \"{digest}\"\n\n[[member]]"),
                1,
            )
        };
        assert_eq!(
            parse_recipe(&with("sha256:abc123")).unwrap_err(),
            "a digest is sha256: and 64 hex characters (NotHex: sha256:abc123)"
        );
        // DECISIONS §197's digest ruling (2026-10-07): bare hex, as recipes were written before
        // it, and a label this tool does not know are both refused by name, not read as SHA-256.
        let bare = "a".repeat(64);
        assert_eq!(
            parse_recipe(&with(&bare)).unwrap_err(),
            format!("a digest is sha256: and 64 hex characters (Unlabeled: {bare})")
        );
        let merkle = format!("merkle-sha256-8k:{bare}");
        assert_eq!(
            parse_recipe(&with(&merkle)).unwrap_err(),
            format!("a digest is sha256: and 64 hex characters (UnknownLabel: {merkle})")
        );
        assert_eq!(
            parse_recipe(&with(&format!("sha256:{bare}")))
                .unwrap()
                .digest,
            Some([0xaa; 32])
        );
    }

    #[test]
    fn a_key_nobody_can_read_names_itself() {
        // A misspelt key must not read as an absent one: a misspelt `digest` would otherwise be a
        // recipe that records no digest, which review would pass.
        assert_eq!(
            parse_recipe("name = \"uptime\"\nfetch = \"https://example.invalid\"\n").unwrap_err(),
            "unknown key fetch"
        );
        assert_eq!(
            parse_recipe(&RECIPE.replace("file = \"LICENSE-MIT\"", "")).unwrap_err(),
            "member 2 is either `program` or `std_program` alone (optionally `as` a \
             member name) or `name` with `file`"
        );
        assert_eq!(
            parse_recipe("name = \"\"\n").unwrap_err(),
            "name is a non-empty string"
        );
    }

    /// **`as` renames a program member** (milestone 614 (two installed versions of one program,
    /// each runnable, and a caller granted the one it needs)): the ELF is resolved under its own name
    /// and packaged under another, which is how `greeting` at 0.2.0 carries the second fixture's
    /// bytes under the member name `greeting`. A file member takes no `as`, because its `name` is
    /// already what the package calls it.
    #[test]
    fn a_program_member_can_be_packaged_under_another_name() {
        let text = "\
name = \"greeting\"
version = \"0.2.0\"
architecture = \"aarch64\"

[[member]]
program = \"greeting_v2\"
as = \"greeting\"
";
        let recipe = parse_recipe(text).unwrap();
        assert_eq!(
            recipe.members,
            vec![(
                "greeting".to_string(),
                Source::Program("greeting_v2".to_string())
            )]
        );
        let plain = text.replace("as = \"greeting\"\n", "");
        assert_eq!(
            parse_recipe(&plain).unwrap().members,
            vec![(
                "greeting_v2".to_string(),
                Source::Program("greeting_v2".to_string())
            )]
        );
        assert_eq!(
            parse_recipe(
                "name = \"x\"\nversion = \"1\"\narchitecture = \"aarch64\"\n\n[[member]]\nname \
                 = \"x\"\nfile = \"LICENSE-MIT\"\nas = \"y\"\n"
            )
            .unwrap_err(),
            "member 1 is either `program` or `std_program` alone (optionally `as` a \
             member name) or `name` with `file`"
        );
    }

    #[test]
    fn every_recipe_in_the_tree_parses() {
        // The recipes are read by every archive build, so a malformed one breaks the image rather
        // than this command; catching it here costs milliseconds.
        let dir = workspace_root().join("packages");
        let mut seen = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.to_string_lossy().ends_with(".recipe.toml") {
                parse_recipe(&std::fs::read_to_string(&path).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                seen += 1;
            }
        }
        assert!(seen > 0, "no recipes found");
    }
}
