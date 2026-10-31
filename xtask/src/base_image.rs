//! **`cargo xtask base-image`**: compose an architecture's bootable image from packages.
//!
//! Milestone 909 (basalt composes the bootable image from its packages). The image is the kernel
//! plus the initial ramdisk the kernel hands the progenitor. nife's own build has packed that
//! ramdisk from the tree since milestone 19f (run a real workload); this composes it instead from
//! `.nifepkg` archives, each checked against the catalog that vouches for it, so a binary no
//! package carries cannot reach the image.
//!
//! `packages/base-image.toml` lists the programs the image carries (milestone 611 (every program
//! and crate belongs to a package) owns the other direction: which package a program belongs to).
//! The list is the image's decision, not a package's: a package releases together, while an image
//! carries what it boots, and the two differ (the `boot` package's `uefi_loader` is a disk image
//! no initial ramdisk carries, and the `redoxfs` package also builds host tools).
//!
//! One-way, as every archive build already is: compose the archive, write the measurement manifest
//! the kernel build reads, then build the kernel. A kernel built before its archive has no trust
//! root and refuses to enter the progenitor.
//!
//! # EXAMPLES
//!
//! ```text
//! $ cargo xtask base-image aarch64
//! $ cargo xtask base-image riscv64 --from /tmp/basalt/rolling/targets \
//!       --catalog /tmp/basalt/rolling/metadata/stand-in-index --out /tmp/image-riscv
//! ```
//!
//! Without `--from`, the base packages are built from this tree first ([`build_packages`]), which
//! keeps nife's own image testable before basalt pins the commit.
//!
//! # BUGS
//!
//! - **The kernel is built here, not taken from a package.** It is firmware's entry, never an
//!   archive entry, and the `kernel` package declares crates and no programs, so this builds it
//!   from those crates. The binary claim is about the ramdisk, which is 909's "Done when".
//! - **The catalog lists only the packages the image used.** `jig` needs the fetched index for
//!   anything else, which is what `SOURCES_ENTRY` is for.
//! - **A package's digest is checked against the catalog it was handed.** That catalog's trust is
//!   the build that wrote it; the base packages record no digest of their own yet (909's BUGS).
//!
//! Name: provisional, minted 2026-10-28 (UTC) by lane milestone/909-basalt-composes-the-image.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use package_archive::Package;

use crate::host::workspace_root;

/// The file that says which programs an image carries.
const DEFINITION: &str = "packages/base-image.toml";

/// The catalog's file name in a packages directory, in `cargo xtask package`'s own shape.
const CATALOG: &str = "catalog";

/// The package whose programs a test image adds: `fixtures`, the test programs every archive
/// build packs. Its definition names its crate rather than 79 programs, so `--test` takes the
/// `[[bin]]` blocks `fixtures/Cargo.toml` declares (milestone 909).
const TEST_PACKAGE: &str = "fixtures";

/// What `packages/base-image.toml` says.
#[derive(Debug, PartialEq, Eq)]
struct Definition {
    /// The version every base package is built at (provisional: one version for the base, because
    /// the packages release and are installed together).
    version: String,
    /// The archive entries the image carries, by program name.
    programs: Vec<String>,
    /// The approval file the composition checks every package against, as a path from the
    /// workspace root (milestone 911 (scripted installs grant only what an approval file
    /// allows); provisional key name). The image carries the same file as a measured archive
    /// entry, so an unattended update on the booted system reads what the composition checked.
    approval: String,
}

/// `cargo xtask base-image`, its arguments parsed.
#[derive(Debug, Default, PartialEq, Eq)]
struct Arguments {
    arch: Option<String>,
    /// Where the packages are, when they were built elsewhere.
    from: Option<String>,
    catalog: Option<String>,
    out: Option<String>,
    boot: bool,
    /// Carry the `fixtures` package's programs too: the image a test boots, not the image a
    /// release ships.
    test: bool,
}

/// **Where a composed image comes from**: the definition it carries, the packages directory, that
/// directory's catalog, and where the kernel and archive are written. Name provisional
/// (2026-10-28 UTC).
struct Plan {
    definition: Definition,
    packages: PathBuf,
    catalog: PathBuf,
    out: PathBuf,
}

/// **Read the definition and locate the packages**, adding the fixtures package's programs to a
/// test image. `None` when a step refused, having said why.
fn plan(arguments: &Arguments, arch: &str, root: &Path) -> Option<Plan> {
    let mut definition = match read_definition(root) {
        Ok(definition) => definition,
        Err(complaint) => {
            eprintln!("base-image: {complaint}");
            return None;
        }
    };
    // A test image carries the fixtures package's programs as well: the kernel's boot self-test
    // runs two of them and the shell's check types at the others, and a release image carries none.
    if arguments.test {
        match crate::archive::fixture_programs() {
            Ok(programs) => definition.programs.extend(programs),
            Err(complaint) => {
                eprintln!("base-image: {complaint}");
                return None;
            }
        }
    }

    // The packages: built here from the tree, or named by the caller.
    let packages = match arguments.from.as_deref() {
        Some(dir) => PathBuf::from(dir),
        None => {
            if !build_packages(root, arch, &definition, arguments.test) {
                return None;
            }
            root.join("target/packages")
        }
    };
    let catalog = match arguments.catalog.as_deref() {
        Some(path) => PathBuf::from(path),
        None => packages.join(CATALOG),
    };
    let out = match arguments.out.as_deref() {
        Some(dir) => PathBuf::from(dir),
        None => root.join(format!("target/base-image-{arch}")),
    };
    Some(Plan {
        definition,
        packages,
        catalog,
        out,
    })
}

/// **`cargo xtask base-image`**: compose one architecture's kernel and initial ramdisk from
/// packages, and optionally boot it to the prompt.
pub(crate) fn base_image() -> bool {
    let arguments = match parse_arguments(std::env::args().skip(2).collect()) {
        Ok(arguments) => arguments,
        Err(complaint) => {
            eprintln!("base-image: {complaint}");
            return false;
        }
    };
    let Some(arch) = arguments.arch.as_deref() else {
        eprintln!(
            "usage: cargo xtask base-image <aarch64|riscv64|x86_64> [--from <dir>] [--catalog <path>] [--out <dir>] [--test] [--boot]"
        );
        return false;
    };
    let root = workspace_root();
    let Some(plan) = plan(&arguments, arch, &root) else {
        return false;
    };
    let Plan {
        definition,
        packages,
        catalog,
        out,
    } = plan;

    let composed = match compose(
        &root,
        arch,
        &packages,
        &catalog,
        &definition,
        arguments.test,
    ) {
        Ok(composed) => composed,
        Err(complaint) => {
            eprintln!("base-image ({arch}): {complaint}");
            return false;
        }
    };
    let archive = out.join("initrd.img");
    if let Some(parent) = archive.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!(
            "base-image ({arch}): cannot create {}: {e}",
            parent.display()
        );
        return false;
    }
    let sources = match crate::package::image_index_sources() {
        Ok(sources) => sources,
        Err(complaint) => {
            eprintln!("base-image ({arch}): {complaint}");
            return false;
        }
    };
    let Some((size, entries)) = crate::archive::write_archive_with(
        "base-image",
        arch,
        &archive.display().to_string(),
        "the composed image",
        &composed.blobs,
        &composed.catalog,
        &sources,
        Some(&composed.seed),
    ) else {
        return false;
    };
    println!(
        "{arch}: {} programs and {} archive entries from {} packages, {size} bytes",
        composed.blobs.len(),
        entries,
        composed.packages.len()
    );
    println!("wrote {}", archive.display());
    // The image's activation store, written beside the archive: the first installed-programs
    // table the composition wrote, measured by the kernel this build is about to make.
    let store = out.join("activation-store.img");
    if let Err(e) = std::fs::write(&store, &composed.seed) {
        eprintln!("base-image ({arch}): could not write {}: {e}", store.display());
        return false;
    }
    println!("wrote {}", store.display());
    for (stem, digest) in &composed.packages {
        println!("  {stem} {digest}");
    }

    // The kernel last: it reads the measurement manifest the archive build just wrote.
    let Some(kernel) = build_kernel(arch, &out) else {
        return false;
    };
    println!("wrote {}", kernel.display());

    if arguments.boot {
        let Some(runner) = runner(arch) else {
            return false;
        };
        if !crate::boot_check::boot_image(arch, runner, &kernel, &archive, Some(&store)) {
            return false;
        }
    }
    true
}

/// **`cargo xtask base-packages`**: build every base package the image needs from this tree, into
/// `target/packages` with its catalog. The half of `base-image` a builder that composes elsewhere
/// runs first (milestone 909); it builds no image and no kernel. `--test` packs the `fixtures`
/// package too, which the base definition's packages do not name, so a caller can compose a test
/// image from the packages it just built (`base-image --test --from` refuses an archive the
/// packages directory does not carry); the published repository carries no `fixtures` package,
/// which no release image boots.
pub(crate) fn base_packages() -> bool {
    let arguments: Vec<String> = std::env::args().skip(2).collect();
    let test = arguments.iter().any(|word| word == "--test");
    let Some(arch) = arguments.iter().find(|word| !word.starts_with('-')) else {
        eprintln!("usage: cargo xtask base-packages <aarch64|riscv64|x86_64> [--test]");
        return false;
    };
    let arch = arch.clone();
    let root = workspace_root();
    let definition = match read_definition(&root) {
        Ok(definition) => definition,
        Err(complaint) => {
            eprintln!("base-packages: {complaint}");
            return false;
        }
    };
    if !build_packages(&root, &arch, &definition, test) {
        return false;
    }
    let what = if test { " and fixtures" } else { "" };
    println!("base-packages: every package in {DEFINITION}{what} for {arch} is in target/packages");
    true
}

/// Parse the verb's arguments. A pure function over the words, so the refusals are host-testable.
fn parse_arguments(args: Vec<String>) -> Result<Arguments, String> {
    let mut parsed = Arguments::default();
    let mut rest = args.into_iter();
    while let Some(word) = rest.next() {
        match word.as_str() {
            "--boot" => parsed.boot = true,
            "--test" => parsed.test = true,
            "--arch" | "--from" | "--catalog" | "--out" => {
                let value = rest.next().ok_or_else(|| format!("{word} needs a value"))?;
                match word.as_str() {
                    "--arch" => parsed.arch = Some(value),
                    "--from" => parsed.from = Some(value),
                    "--catalog" => parsed.catalog = Some(value),
                    "--out" => parsed.out = Some(value),
                    _ => unreachable!(),
                }
            }
            other if parsed.arch.is_none() && !other.starts_with('-') => {
                parsed.arch = Some(other.to_string());
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(parsed)
}

/// Read `packages/base-image.toml`.
fn read_definition(root: &Path) -> Result<Definition, String> {
    let path = root.join(DEFINITION);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    parse_definition(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Parse the image definition, refusing a key nobody reads so a misspelt `programs` is not an
/// empty image.
fn parse_definition(text: &str) -> Result<Definition, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| e.message().to_string())?;
    for key in table.keys() {
        if !["version", "programs", "approval"].contains(&key.as_str()) {
            return Err(format!("unknown key {key}"));
        }
    }
    let version = match table.get("version") {
        Some(toml::Value::String(value)) if !value.is_empty() => value.clone(),
        Some(_) => return Err("version is a non-empty string".to_string()),
        None => return Err("no version".to_string()),
    };
    let approval = match table.get("approval") {
        Some(toml::Value::String(value)) if !value.is_empty() => value.clone(),
        Some(_) => return Err("approval is a non-empty string".to_string()),
        None => return Err("no approval".to_string()),
    };
    let programs = match table.get("programs") {
        Some(toml::Value::Array(entries)) => entries
            .iter()
            .map(|entry| {
                entry
                    .as_str()
                    .filter(|name| !name.is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| "programs is an array of non-empty names".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err("programs is an array of non-empty names".to_string()),
        None => return Err("no programs".to_string()),
    };
    if programs.is_empty() {
        return Err("programs is empty; an image carries at least a progenitor".to_string());
    }
    let mut sorted = programs.clone();
    sorted.sort();
    if let Some(pair) = sorted.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(format!("`{}` is listed twice", pair[0]));
    }
    Ok(Definition {
        version,
        programs,
        approval,
    })
}

/// **Which package each program belongs to**, read from `packages/*.package.toml`.
///
/// Only `name` and `programs` are read: `helpers/packages.py --check` is the format's strict
/// reader, and a second one here would be a second definition of the format. A program two
/// packages claim is refused, because the image would otherwise take whichever archive it read
/// last.
///
/// `test` adds the `fixtures` package's programs, which no definition lists: the package names its
/// crate and the manifest names its 79 programs, so they are read from `fixtures/Cargo.toml` by
/// the same reader every archive build uses ([`crate::archive::fixture_programs`]).
fn program_owners(root: &Path, test: bool) -> Result<BTreeMap<String, String>, String> {
    let dir = root.join("packages");
    let mut owners = BTreeMap::new();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("could not read {}: {e}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".package.toml"))
        })
        .collect();
    paths.sort();
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        let table: toml::Table = text
            .parse()
            .map_err(|e: toml::de::Error| format!("{}: {}", path.display(), e.message()))?;
        let Some(name) = table.get("name").and_then(toml::Value::as_str) else {
            continue;
        };
        let Some(toml::Value::Array(programs)) = table.get("programs") else {
            continue;
        };
        for program in programs.iter().filter_map(toml::Value::as_str) {
            if let Some(claimed) = owners.insert(program.to_string(), name.to_string()) {
                return Err(format!(
                    "`{program}` is declared by packages `{claimed}` and `{name}`"
                ));
            }
        }
    }
    if test {
        for program in crate::archive::fixture_programs()? {
            if let Some(claimed) = owners.insert(program.clone(), TEST_PACKAGE.to_string()) {
                return Err(format!(
                    "`{program}` is a `{TEST_PACKAGE}` program and is declared by package \
                     `{claimed}` too"
                ));
            }
        }
    }
    Ok(owners)
}

/// A package archive in the packages directory, read back once.
struct Archive {
    version: String,
    architecture: String,
    /// SHA-256 over the whole file, `sha256:<64 hex>`.
    digest: String,
}

/// **One package's archive for `arch`**, or why it is not there. Two files for one package and
/// architecture are refused rather than resolved by name order: a directory that holds two builds
/// of one version is a mistake and the image would silently take one.
fn find_archive(
    dir: &Path,
    packages: &BTreeMap<String, Vec<(String, Archive)>>,
    name: &str,
    arch: &str,
) -> Result<(String, Archive), String> {
    let entries = packages
        .get(name)
        .ok_or_else(|| format!("no archive for package `{name}` in {}", dir.display()))?;
    let mut matching = entries
        .iter()
        .filter(|(_, archive)| archive.architecture == arch)
        .map(|(stem, archive)| (stem.clone(), archive));
    let Some(first) = matching.next() else {
        return Err(format!(
            "package `{name}` has no archive for {arch} in {}",
            dir.display()
        ));
    };
    if let Some(second) = matching.next() {
        return Err(format!(
            "package `{name}` has two archives for {arch}: {} and {}",
            first.0, second.0
        ));
    }
    Ok((
        first.0.clone(),
        Archive {
            version: first.1.version.clone(),
            architecture: first.1.architecture.clone(),
            digest: first.1.digest.clone(),
        },
    ))
}

/// Read every `.nifepkg` in `dir`, grouped by package name.
fn read_archives(dir: &Path) -> Result<BTreeMap<String, Vec<(String, Archive)>>, String> {
    let mut packages: BTreeMap<String, Vec<(String, Archive)>> = BTreeMap::new();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("could not read {}: {e}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "nifepkg")
        })
        .collect();
    paths.sort();
    for path in paths {
        let bytes =
            std::fs::read(&path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
        let package = Package::parse(&bytes)
            .map_err(|e| format!("{} is not a package: {e:?}", path.display()))?;
        package.verify().map_err(|e| {
            format!(
                "{}: member {} does not match its digest",
                path.display(),
                e.index
            )
        })?;
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("{} has no name", path.display()))?
            .to_string();
        let expected = format!(
            "{}-{}-{}",
            package.name(),
            package.version(),
            package.architecture()
        );
        if stem != expected {
            return Err(format!(
                "{} is package `{expected}`; the file's name is the catalog's key",
                path.display()
            ));
        }
        let digest = crate::package::digest_label(&package_archive::sha256(&bytes));
        packages
            .entry(package.name().to_string())
            .or_default()
            .push((
                stem,
                Archive {
                    version: package.version().to_string(),
                    architecture: package.architecture().to_string(),
                    digest,
                },
            ));
    }
    Ok(packages)
}

/// Parse a catalog: `<stem> sha256:<64 hex>` lines, blank lines and `#` comments skipped, as
/// `cargo xtask package` writes it. Anything else is refused, because a line nobody reads is a
/// package nobody vouches for.
fn parse_catalog(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut lines = BTreeMap::new();
    for (number, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut words = line.split(' ');
        let (Some(stem), Some(digest), None) = (words.next(), words.next(), words.next()) else {
            return Err(format!(
                "line {}: {line:?} is not `<stem> sha256:<hex>`",
                number + 1
            ));
        };
        let digest = measured_boot::parse_digest(digest)
            .map_err(|why| format!("line {}: {why:?}: {digest}", number + 1))?;
        if lines
            .insert(stem.to_string(), crate::package::digest_label(&digest))
            .is_some()
        {
            return Err(format!("line {}: `{stem}` is listed twice", number + 1));
        }
    }
    Ok(lines)
}

/// What [`compose`] produced: the archive's entries and the catalog that goes into it.
struct Composed {
    /// `(entry name, bytes)`, in the order `packages/base-image.toml` lists them.
    blobs: Vec<(String, Vec<u8>)>,
    /// The catalog lines of exactly the packages the image used.
    catalog: String,
    /// `(stem, digest)` for each package the image used.
    packages: Vec<(String, String)>,
    /// The image's activation store, the first installed-programs table the composition wrote,
    /// whose bytes the kernel's measurement manifest names.
    seed: Vec<u8>,
}

/// **Compose the archive's entries from packages**: for every program the definition names, the
/// member `bin/<program>` of the package that owns it, checked against the catalog; and every
/// package's asks checked against the image's approval file (milestone 911's check, host-side).
/// The grant records the checks pass become the image's first installed-programs table.
fn compose(
    root: &Path,
    arch: &str,
    dir: &Path,
    catalog_path: &Path,
    definition: &Definition,
    test: bool,
) -> Result<Composed, String> {
    let owners = program_owners(root, test)?;
    let archives = read_archives(dir)?;
    let catalog_text = std::fs::read_to_string(catalog_path)
        .map_err(|e| format!("could not read {}: {e}", catalog_path.display()))?;
    let catalog =
        parse_catalog(&catalog_text).map_err(|e| format!("{}: {e}", catalog_path.display()))?;
    // The image's approval file (milestone 911 (scripted installs grant only what an approval
    // file allows)): every package's asks are checked against it, the file rides the image as a
    // measured archive entry, and its name and digest are the `answered` line's.
    let approval_path = root.join(&definition.approval);
    let approval_bytes = std::fs::read(&approval_path)
        .map_err(|e| format!("could not read {}: {e}", approval_path.display()))?;
    let approval_text = String::from_utf8(approval_bytes.clone()).map_err(|_| {
        format!(
            "{} is not UTF-8, and an approval file is text",
            approval_path.display()
        )
    })?;
    let approval = approval_file::File::parse(&approval_text).map_err(|e| {
        format!(
            "{}: line {}: {e:?}",
            approval_path.display(),
            e.at
        )
    })?;
    let approval_digest = measured_boot::sha256(&approval_bytes);

    let mut blobs = Vec::new();
    let mut used: BTreeMap<String, String> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    // The packages' bytes, read once per package and alive through the generation's write: the
    // grant record's zone names point into a note inside these bytes.
    let mut package_bytes: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut rows: Vec<installed_programs::Entry<'_>> = Vec::new();
    let mut grants: Vec<installed_programs::Grant<'_>> = Vec::new();
    let mut refusals: Vec<String> = Vec::new();
    for program in &definition.programs {
        let owner = owners.get(program).ok_or_else(|| {
            format!(
                "`{program}` is in no package's `programs` line; the package that packs it must \
                 name it in packages/<name>.package.toml"
            )
        })?;
        let (stem, archive) = find_archive(dir, &archives, owner, arch)?;
        if archive.version != definition.version {
            return Err(format!(
                "package `{owner}` {arch} is version {}, and the image is {}",
                archive.version, definition.version
            ));
        }
        let vouched = catalog.get(&stem).ok_or_else(|| {
            format!(
                "{} lists no `{stem}`; a package the catalog does not vouch for cannot enter the \
                 image",
                catalog_path.display()
            )
        })?;
        if *vouched != archive.digest {
            return Err(format!(
                "{stem}: the catalog records {vouched}, these bytes are {}",
                archive.digest
            ));
        }
        let path = dir.join(format!("{stem}.nifepkg"));
        let bytes = match package_bytes.get(stem.as_str()) {
            Some(bytes) => bytes,
            None => {
                let read = std::fs::read(&path)
                    .map_err(|e| format!("could not read {}: {e}", path.display()))?;
                package_bytes.insert(stem.clone(), read);
                package_bytes.get(stem.as_str()).unwrap()
            }
        };
        let package = Package::parse(bytes)
            .map_err(|e| format!("{}: not a package: {e:?}", path.display()))?;
        let member = format!("bin/{program}");
        let index = (0..package.len())
            .find(|at| package.member_name(*at) == Some(member.as_str()))
            .ok_or_else(|| {
                format!(
                    "{stem} carries no `{member}`, so package `{owner}` does not own `{program}` \
                     as its definition says"
                )
            })?;
        let content = package.member(index).ok_or_else(|| {
            format!("{stem}: member {index} has no bytes, which its parse should have refused")
        })?;
        let digest = package.member_digest(index).ok_or_else(|| {
            format!("{stem}: member {index} has no digest, which its parse should have refused")
        })?;
        // The asks the program's note declares, checked against the package's entry in the
        // image's approval file, before the program enters the image (milestone 911, item 6;
        // milestone 909, item 3). Every extra grant is named, so one edit to the file answers
        // every refusal at once.
        let carried = package_archive::Program {
            name: program,
            bytes: content,
            digest,
            member: index,
        };
        let asks: Vec<installed_programs::Granted<'_>> =
            program_asks::asked_grants_of(&carried).collect();
        check_asks(
            &approval,
            approval.name(),
            owner,
            program,
            &asks,
            &mut refusals,
        );
        rows.push(installed_programs::Entry {
            program,
            version: &definition.version,
            package: owner,
            digest,
            manager: installed_programs::OWNER,
        });
        grants.extend(program_asks::asked_grants(&carried));
        blobs.push((program.clone(), content.to_vec()));
        if used.insert(stem.clone(), archive.digest.clone()).is_none() {
            order.push(stem);
        }
    }
    if !refusals.is_empty() {
        return Err(format!(
            "the image's approval file {} does not allow what its packages ask:\n  {}",
            definition.approval,
            refusals.join("\n  ")
        ));
    }
    // The image's first installed-programs table, as the composition's answer to the checks that
    // just passed: the records an interactive install would write for the same grants, with the
    // approval's name and digest in the answered line.
    let answered = installed_programs::Answered::Approval(approval.name(), approval_digest);
    let generation = seed_generation(&rows, &grants, &answered)?;
    let mut seed = vec![0u8; activation_store::write_size(&[generation.len()])];
    activation_store::write(&[&generation], 1, &mut seed)
        .map_err(|e| format!("the image's activation store: {e:?}"))?;
    blobs.push((
        approval_file::IMAGE_ENTRY.to_string(),
        approval_bytes.clone(),
    ));

    let mut text = String::new();
    for stem in &order {
        text.push_str(&format!("{stem} {}\n", used[stem]));
    }
    Ok(Composed {
        blobs,
        catalog: text,
        packages: order
            .into_iter()
            .map(|stem| {
                let digest = used[&stem].clone();
                (stem, digest)
            })
            .collect(),
        seed,
    })
}

/// **One program's asks against its package's entry** (milestone 911's subset test,
/// `approval_file::File::check`): every grant the entry does not allow is named, with the package
/// and the program that asked, into `refusals`. The composition refuses once, at the end, so one
/// edit to the file answers every refusal at once.
fn check_asks(
    file: &approval_file::File<'_>,
    approval_name: &str,
    package: &str,
    program: &str,
    asks: &[installed_programs::Granted<'_>],
    refusals: &mut Vec<String>,
) {
    for extra in file.check(package, asks).unwrap_or_default() {
        refusals.push(format!(
            "refused: package `{package}` program `{program}` asks for {}, which \
             `{approval_name}` does not allow",
            spell_grant(&extra),
        ));
    }
}

/// One `Granted` in the words the approval file's keys spell, which is what an owner reads when
/// the refusal says what to allow.
fn spell_grant(what: &installed_programs::Granted<'_>) -> String {
    match what {
        installed_programs::Granted::Network => "the network".to_string(),
        installed_programs::Granted::Service => "the service (runs at every boot)".to_string(),
        installed_programs::Granted::Installer => "the installer".to_string(),
        installed_programs::Granted::Zone(zone) if zone.is_empty() => "the root zone".to_string(),
        installed_programs::Granted::Zone(zone) => format!("zone {zone}"),
        installed_programs::Granted::Directory(name, rights) => {
            format!("directory {name} ({})", rights.spell())
        }
    }
}

/// **The image's first installed-programs table**: one generation holding every program the image
/// carries as a row of the package it came from, the owner's grants beside them, and the
/// approval's answered line. The one writer an install writes records with
/// ([`installed_programs::with_entries_granting`]) writes it, so a composition's record is
/// byte-identical to a console's for the same grants; `image_carries` is false because these rows
/// ARE the image's programs, which no installer is taking a name from (DECISIONS §229 bounds a
/// package install, not the image's own first table).
fn seed_generation(
    rows: &[installed_programs::Entry<'_>],
    grants: &[installed_programs::Grant<'_>],
    answered: &installed_programs::Answered<'_>,
) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; 65536];
    loop {
        match installed_programs::with_entries_granting(
            "",
            || rows.iter().copied(),
            |_| false,
            || grants.iter().copied(),
            Some(answered),
            &mut out,
        ) {
            Ok(n) => {
                out.truncate(n);
                return Ok(out);
            }
            Err(refused) => {
                if refused.error == installed_programs::Error::TooSmall && out.len() < 1 << 20 {
                    out.resize(out.len() * 2, 0);
                    continue;
                }
                return Err(format!(
                    "the image's first installed-programs table: {:?}",
                    refused.error
                ));
            }
        }
    }
}

/// **Build every base package the image needs from this tree's own build**, then leave them in
/// `target/packages` with their catalog. `false` after saying what failed.
///
/// The programs' builds come first, because a package is packed from what the tree built:
/// `components` for the native programs, the `std` farm's TLS graph for `jig` and `mdr`, and the
/// RedoxFS server's workspace for the file server and `mkfs`. `test` adds `fixtures`, which the
/// composed test image carries and the release image does not.
fn build_packages(root: &Path, arch: &str, definition: &Definition, test: bool) -> bool {
    let Ok(triple) = crate::package::triple(arch) else {
        eprintln!("base-image: unknown architecture {arch}");
        return false;
    };
    let Ok(owners) = program_owners(root, test) else {
        eprintln!("base-image: could not read the package definitions");
        return false;
    };
    let mut packages: Vec<String> = definition
        .programs
        .iter()
        .filter_map(|program| owners.get(program).cloned())
        .collect();
    packages.sort();
    packages.dedup();

    // The base is built as one step, so the directory the catalog is written into starts empty:
    // an appended line for a package a previous run left there would read as two builds of it.
    if let Err(e) = crate::package::clear_output(root) {
        eprintln!("base-image: {e}");
        return false;
    }
    if !crate::package::build_base_programs(triple, arch, test) {
        return false;
    }
    for package in packages {
        let packed = if test && package == TEST_PACKAGE {
            match crate::archive::fixture_programs() {
                Ok(programs) => {
                    crate::package::package_members(&package, arch, &definition.version, &programs)
                }
                Err(complaint) => {
                    eprintln!("base-image: {complaint}");
                    return false;
                }
            }
        } else {
            crate::package::package_named(&package, arch, &definition.version)
        };
        if !packed {
            return false;
        }
    }
    true
}

/// Where the composed kernel is written, and the copy made there.
fn build_kernel(arch: &str, out: &Path) -> Option<PathBuf> {
    let triple = crate::package::triple(arch).ok()?;
    if !crate::cargo(&["build", "-p", "kernel", "--target", triple]) {
        eprintln!("base-image ({arch}): the kernel build failed");
        return None;
    }
    let built = workspace_root().join(format!("target/{triple}/{}/kernel", crate::profile_dir()));
    let kernel = out.join("kernel");
    if let Err(e) = std::fs::copy(&built, &kernel) {
        eprintln!(
            "base-image ({arch}): could not copy {} to {}: {e}",
            built.display(),
            kernel.display()
        );
        return None;
    }
    Some(kernel)
}

/// The runner script that boots an architecture's kernel, for `--boot`.
fn runner(arch: &str) -> Option<&'static str> {
    match arch {
        "aarch64" => Some(crate::RUNNER),
        "riscv64" => Some("helpers/qemu-runner-riscv64.sh"),
        "x86_64" => Some("helpers/qemu-runner-x86_64.sh"),
        other => {
            eprintln!("base-image: unknown architecture {other}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFINITION: &str = r#"
# what the image carries
version = "0.1.0"
programs = ["progenitor", "swish", "jig"]
"#;

    #[test]
    fn the_image_definition_reads_as_what_it_says() {
        let definition = parse_definition(DEFINITION).unwrap();
        assert_eq!(definition.version, "0.1.0");
        assert_eq!(definition.programs, ["progenitor", "swish", "jig"]);
    }

    #[test]
    fn a_definition_missing_its_parts_is_named() {
        assert_eq!(
            parse_definition("programs = [\"swish\"]").unwrap_err(),
            "no version"
        );
        assert_eq!(
            parse_definition("version = \"0.1.0\"").unwrap_err(),
            "no programs"
        );
        assert_eq!(
            parse_definition("version = \"0.1.0\"\nprograms = []").unwrap_err(),
            "programs is empty; an image carries at least a progenitor"
        );
        assert_eq!(
            parse_definition("version = \"0.1.0\"\nprograms = [\"a\", \"a\"]").unwrap_err(),
            "`a` is listed twice"
        );
        assert_eq!(
            parse_definition("version = \"0.1.0\"\nprograms = [\"a\"]\nmembers = [\"a\"]")
                .unwrap_err(),
            "unknown key members"
        );
    }

    #[test]
    fn a_catalog_is_checked_line_by_line() {
        let catalog = parse_catalog(
            "# a comment\n\nuptime-0.1.0-aarch64 sha256:\
             7d8e7d1f6e1a5d3c5b10b1f1e0b9a7c3d4e5f60718293a4b5c6d7e8f90123456\n",
        )
        .unwrap();
        assert_eq!(catalog.len(), 1);
        let hex = "a".repeat(64);
        assert_eq!(
            parse_catalog(&format!("uptime-0.1.0-aarch64 {hex}")).unwrap_err(),
            "line 1: Unlabeled: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(
            parse_catalog("uptime-0.1.0-aarch64").unwrap_err(),
            "line 1: \"uptime-0.1.0-aarch64\" is not `<stem> sha256:<hex>`"
        );
        let line = format!("uptime-0.1.0-aarch64 sha256:{hex}");
        assert_eq!(
            parse_catalog(&format!("{line}\n{line}")).unwrap_err(),
            "line 2: `uptime-0.1.0-aarch64` is listed twice"
        );
    }

    #[test]
    fn the_arguments_read_as_flags_and_a_bare_architecture() {
        assert_eq!(
            parse_arguments(vec!["aarch64".into(), "--boot".into()]).unwrap(),
            Arguments {
                arch: Some("aarch64".into()),
                boot: true,
                ..Arguments::default()
            }
        );
        assert_eq!(
            parse_arguments(vec![
                "riscv64".into(),
                "--from".into(),
                "/tmp/p".into(),
                "--catalog".into(),
                "/tmp/i".into(),
                "--out".into(),
                "/tmp/o".into(),
            ])
            .unwrap(),
            Arguments {
                arch: Some("riscv64".into()),
                from: Some("/tmp/p".into()),
                catalog: Some("/tmp/i".into()),
                out: Some("/tmp/o".into()),
                boot: false,
                test: false,
            }
        );
        assert_eq!(
            parse_arguments(vec!["--from".into()]).unwrap_err(),
            "--from needs a value"
        );
        assert_eq!(
            parse_arguments(vec!["aarch64".into(), "--nonsense".into()]).unwrap_err(),
            "unknown argument --nonsense"
        );
    }

    #[test]
    fn a_program_two_packages_claim_is_refused() {
        // The tree's own definitions: every claimed program has one owner. This is the invariant
        // the image depends on, checked here so a split that duplicates one fails in milliseconds.
        let owners = program_owners(&workspace_root(), false).unwrap();
        assert!(owners.contains_key("progenitor"));
        assert!(owners.contains_key("jig"));
        // A test image adds the fixtures package's programs, which no definition lists.
        assert!(!owners.contains_key("kernel_test_subject"));
        let test = program_owners(&workspace_root(), true).unwrap();
        assert_eq!(
            test.get("kernel_test_subject").map(String::as_str),
            Some("fixtures")
        );
    }
}
