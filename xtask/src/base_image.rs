//! **`cargo xtask base-image`**: compose an architecture's bootable image from packages.
//!
//! Milestone 909 (basalt composes the bootable image from its packages). The image is the kernel
//! plus the initial ramdisk the kernel hands the progenitor. nife's own build packs that ramdisk
//! from the tree; this composes it instead from `.nifepkg` archives, each checked against the
//! catalog that vouches for it, so a binary no package carries cannot reach the image.
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
//! - **The base image names no approval file yet**: the definition sets no `approval` key, so the check runs only when one names it.
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
#[derive(Clone, Debug, PartialEq, Eq)]
struct Definition {
    /// The version every base package is built at (provisional: one version for the base, because
    /// the packages release and are installed together).
    version: String,
    /// The archive entries the image carries, by program name.
    programs: Vec<String>,
    /// The image's approval file, when the definition names one: a path under the tree root that
    /// [`compose`] checks every package against. `None` composes the image without the check, which
    /// is the state until the base image's approval policy is written (909 item 3's BUGS).
    approval: Option<String>,
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

/// **The definition a test image carries**: `packages/base-image.toml`'s programs plus the
/// `fixtures` package's, which no definition lists. The kernel's boot self-test runs two of them
/// and the shell's check types at the others, and a release image carries none.
///
/// `test` false returns the definition unchanged. A refusal says what could not be read; the
/// caller puts its verb in front of it.
fn with_test_programs(mut definition: Definition, test: bool) -> Result<Definition, String> {
    if test {
        definition
            .programs
            .extend(crate::archive::fixture_programs()?);
    }
    Ok(definition)
}

/// **Read the definition and locate the packages**, adding the fixtures package's programs to a
/// test image. `None` when a step refused, having said why.
fn plan(arguments: &Arguments, arch: &str, root: &Path) -> Option<Plan> {
    let definition = match read_definition(root) {
        Ok(definition) => definition,
        Err(complaint) => {
            eprintln!("base-image: {complaint}");
            return None;
        }
    };
    let definition = match with_test_programs(definition, arguments.test) {
        Ok(definition) => definition,
        Err(complaint) => {
            eprintln!("base-image: {complaint}");
            return None;
        }
    };

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
        if !crate::boot_check::boot_image(arch, runner, &kernel, &archive) {
            return false;
        }
    }
    true
}

/// **`cargo xtask base-packages`**: build every base package the image needs from this tree, into
/// `target/packages` with its catalog. The half of `base-image` a builder that composes elsewhere
/// runs first (milestone 909); it builds no image and no kernel.
///
/// `--test` adds the `fixtures` package, the one the composed test image's boot self-test runs and
/// no release image carries. A builder that composes with `--test` asks for the same set here; the
/// two flags are one decision, which is why [`with_test_programs`] is their one reader.
pub(crate) fn base_packages() -> bool {
    let (arch, test) = match parse_base_packages_arguments(std::env::args().skip(2).collect()) {
        Ok(arguments) => arguments,
        Err(complaint) => {
            eprintln!("base-packages: {complaint}");
            eprintln!("usage: cargo xtask base-packages <aarch64|riscv64|x86_64> [--test]");
            return false;
        }
    };
    let root = workspace_root();
    let definition = match read_definition(&root) {
        Ok(definition) => definition,
        Err(complaint) => {
            eprintln!("base-packages: {complaint}");
            return false;
        }
    };
    let definition = match with_test_programs(definition, test) {
        Ok(definition) => definition,
        Err(complaint) => {
            eprintln!("base-packages: {complaint}");
            return false;
        }
    };
    if !build_packages(&root, &arch, &definition, test) {
        return false;
    }
    println!(
        "base-packages: every package in {DEFINITION}{} for {arch} is in target/packages",
        if test {
            " and the test image's `fixtures`"
        } else {
            ""
        }
    );
    true
}

/// **`cargo xtask base-packages`, its arguments**: one architecture, and whether the packages are
/// for a test image. A pure function over the words, so the refusals are host-testable.
fn parse_base_packages_arguments(args: Vec<String>) -> Result<(String, bool), String> {
    let mut arch = None;
    let mut test = false;
    for word in args {
        match word.as_str() {
            "--test" => test = true,
            other if other.starts_with('-') => return Err(format!("unknown argument {other}")),
            other => {
                if arch.is_some() {
                    return Err(format!("one architecture only, not also {other}"));
                }
                arch = Some(other.to_string());
            }
        }
    }
    arch.map(|arch| (arch, test))
        .ok_or_else(|| "no architecture".to_string())
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
    let approval = match table.get("approval") {
        Some(toml::Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(_) => return Err("approval is a non-empty path".to_string()),
        None => None,
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
}

/// **Compose the archive's entries from packages**: for every program the definition names, the
/// member `bin/<program>` of the package that owns it, checked against the catalog.
/// **Compose one architecture's archive from the packages the definition names.** Each program
/// comes from its package's archive, checked against the catalog; when the definition names an
/// approval file, each package's manifest note is checked against it, and a package asking for more
/// than its entry allows refuses the composition, naming every extra grant (milestone 911 (scripted
/// installs grant only what the owner's approval file allows), 909 item 3).
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

    // The image's approval file, when the definition names one. Read and parsed before anything is
    // taken from an archive, so a file that does not parse refuses the composition rather than
    // leaving the check half-done.
    let approval_text = match &definition.approval {
        Some(relative) => {
            let path = root.join(relative);
            let text = std::fs::read_to_string(&path).map_err(|e| {
                format!(
                    "could not read the image's approval file {}: {e}",
                    path.display()
                )
            })?;
            Some((path, text))
        }
        None => None,
    };
    let approval = match &approval_text {
        Some((path, text)) => Some((
            path.as_path(),
            approval_file::File::parse(text).map_err(|e| format!("{}: {e:?}", path.display()))?,
        )),
        None => None,
    };

    let mut blobs = Vec::new();
    let mut asked: BTreeMap<String, Vec<Ask>> = BTreeMap::new();
    let mut used: BTreeMap<String, String> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
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
        let bytes =
            std::fs::read(&path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
        let package = Package::parse(&bytes)
            .map_err(|e| format!("{}: not a package: {e:?}", path.display()))?;
        let member = format!("bin/{program}");
        let content = (0..package.len())
            .find(|index| package.member_name(*index) == Some(member.as_str()))
            .and_then(|index| package.member(index))
            .ok_or_else(|| {
                format!(
                    "{stem} carries no `{member}`, so package `{owner}` does not own `{program}` \
                     as its definition says"
                )
            })?;
        blobs.push((program.clone(), content.to_vec()));
        asked
            .entry(owner.clone())
            .or_default()
            .extend(asks_of(content));
        if used.insert(stem.clone(), archive.digest.clone()).is_none() {
            order.push(stem);
        }
    }

    if let Some((path, approval)) = &approval {
        check_approval(path, approval, &asked)?;
    }

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
    })
}

/// **What one package asked for**, as an owned mirror of [`installed_programs::Granted`], so the
/// asks outlive the program bytes [`asks_of`] read them from.
#[derive(Debug, PartialEq, Eq)]
enum Ask {
    Network,
    Zone(String),
    Service,
}

/// **The asks one program's manifest note carries**: the network, each resolver zone, and the
/// service ask (milestone 911). Empty for a program with no note, which asks for none, the same
/// reading `system_initializer::asked_grants_of` gives at boot.
fn asks_of(program: &[u8]) -> Vec<Ask> {
    let Some(descriptor) = manifest_descriptor(program) else {
        return Vec::new();
    };
    let Ok(manifest) = capability_manifest_note::decode(descriptor) else {
        return Vec::new();
    };
    let mut asks = Vec::new();
    if manifest.network {
        asks.push(Ask::Network);
    }
    if capability_manifest_note::service(descriptor).unwrap_or(false) {
        asks.push(Ask::Service);
    }
    if let Ok(zones) = capability_manifest_note::zones(descriptor) {
        for zone in zones.iter() {
            if let Ok(text) = core::str::from_utf8(zone) {
                asks.push(Ask::Zone(text.to_string()));
            }
        }
    }
    asks
}

/// **A manifest note's descriptor inside an ELF's bytes**, found by its header and owner the way
/// [`crate::disk`]'s note rewriting finds one, or `None` when there is none. The note is the
/// authoritative record of a program's asks, so the check reads it rather than a static table that
/// a program's own build could disagree with.
fn manifest_descriptor(bytes: &[u8]) -> Option<&[u8]> {
    const HEADER: usize = 12;
    const NAME_FIELD: usize = 8;
    let name_length = (capability_manifest_note::OWNER.len() + 1) as u32;
    let last = bytes.len().checked_sub(HEADER + NAME_FIELD)?;
    for at in 0..=last {
        let header = &bytes[at..at + HEADER];
        if u32::from_le_bytes(header[0..4].try_into().unwrap()) != name_length
            || u32::from_le_bytes(header[8..12].try_into().unwrap())
                != capability_manifest_note::MANIFEST
        {
            continue;
        }
        let name = &bytes[at + HEADER..at + HEADER + capability_manifest_note::OWNER.len()];
        if name != capability_manifest_note::OWNER {
            continue;
        }
        let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let start = at + HEADER + NAME_FIELD;
        return bytes.get(start..start.checked_add(length)?);
    }
    None
}

/// **Refuse the composition when a package asks for a grant the image's approval file does not
/// allow**, naming every extra (milestone 911 (scripted installs grant only what the owner's
/// approval file allows)'s subset test, called once per package; milestone 909 (basalt composes the
/// bootable image from its packages) item 3).
fn check_approval(
    path: &Path,
    approval: &approval_file::File<'_>,
    asked: &BTreeMap<String, Vec<Ask>>,
) -> Result<(), String> {
    for (package, asks) in asked {
        let granted: Vec<installed_programs::Granted<'_>> = asks
            .iter()
            .map(|ask| match ask {
                Ask::Network => installed_programs::Granted::Network,
                Ask::Service => installed_programs::Granted::Service,
                Ask::Zone(zone) => installed_programs::Granted::Zone(zone.as_str()),
            })
            .collect();
        let extras = approval
            .check(package, &granted)
            .map_err(|e| format!("{}: {e:?}", path.display()))?;
        let mut named: Vec<String> = extras.iter().map(granted_name).collect();
        named.sort();
        named.dedup();
        if !named.is_empty() {
            return Err(format!(
                "package `{package}` asks for {}, which the image's approval file ({}) does not \
                 allow",
                named.join(", "),
                path.display()
            ));
        }
    }
    Ok(())
}

/// Name a grant the way a refusal should read it.
fn granted_name(grant: &installed_programs::Granted<'_>) -> String {
    use installed_programs::Granted;
    match grant {
        Granted::Network => "network".to_string(),
        Granted::Service => "service".to_string(),
        Granted::Installer => "the installer endpoint".to_string(),
        Granted::Zone("") => "zone \".\" (the root)".to_string(),
        Granted::Zone(zone) => format!("zone \"{zone}\""),
        Granted::Directory(name, rights) => format!("directory \"{name}\" ({})", rights.spell()),
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
    fn the_image_definition_may_name_an_approval_file() {
        let named = parse_definition(
            "version = \"0.1.0\"\nprograms = [\"swish\"]\n\
             approval = \"packages/base-image.approval.toml\"",
        )
        .unwrap();
        assert_eq!(
            named.approval.as_deref(),
            Some("packages/base-image.approval.toml")
        );
        assert_eq!(
            parse_definition("version = \"0.1.0\"\nprograms = [\"swish\"]")
                .unwrap()
                .approval,
            None
        );
        assert_eq!(
            parse_definition("version = \"0.1.0\"\nprograms = [\"swish\"]\napproval = 3")
                .unwrap_err(),
            "approval is a non-empty path"
        );
    }

    #[test]
    fn a_programs_note_names_its_asks() {
        let network =
            capability_manifest_note::Note::of(&grant_plan::Program::NetworkEchoClient.manifest());
        assert_eq!(asks_of(&network.0), [Ask::Network]);

        const LEN: usize = capability_manifest_note::serviced_len(&["example.net"]);
        const NOTE: usize = capability_manifest_note::serviced_note_len(&["example.net"]);
        let serviced: capability_manifest_note::ServicedNote<NOTE> =
            capability_manifest_note::ServicedNote::<NOTE>::of::<LEN>(
                &grant_plan::Program::NetworkEchoClient.manifest(),
                &["example.net"],
                true,
            );
        let asks = asks_of(&serviced.0);
        assert!(asks.contains(&Ask::Service));
        assert!(asks.contains(&Ask::Zone("example.net".to_string())));

        // A program with no note asks for nothing, the reading the boot path gives it.
        assert_eq!(asks_of(b"\x7fELF not really"), []);
    }

    #[test]
    fn a_package_asking_past_its_entry_is_refused() {
        let asked = BTreeMap::from([(
            "core-tools".to_string(),
            vec![Ask::Network, Ask::Zone("example.net".to_string())],
        )]);
        let nothing = approval_file::File::parse(
            "approval = \"test\"\n\n[package.core-tools]\nupdate = false\nremove = false\n",
        )
        .unwrap();
        let refusal = check_approval(Path::new("approval.toml"), &nothing, &asked).unwrap_err();
        assert!(
            refusal.contains("package `core-tools` asks for"),
            "{refusal}"
        );
        assert!(refusal.contains("network"), "{refusal}");
        assert!(refusal.contains("zone \"example.net\""), "{refusal}");

        let allowed = approval_file::File::parse(
            "approval = \"test\"\n\n[package.core-tools]\n\
             network = true\nzones = [\"example.net\"]\nupdate = false\nremove = false\n",
        )
        .unwrap();
        assert_eq!(
            check_approval(Path::new("approval.toml"), &allowed, &asked),
            Ok(())
        );
    }

    #[test]
    fn the_base_images_approval_file_parses() {
        // The first cut lives in the tree, so it is held to the grammar it will be read with. It is
        // not yet wired into `packages/base-image.toml`, so this is the only reader it has.
        let path = workspace_root().join("packages/base-image.approval.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        let file = approval_file::File::parse(&text).unwrap();
        assert_eq!(file.name(), "base-image");
        assert!(file.entry("jig").is_some(), "jig is a base package");
        let definition = read_definition(&workspace_root()).unwrap();
        let owners = program_owners(&workspace_root(), false).unwrap();
        for program in &definition.programs {
            let package = owners.get(program).unwrap();
            assert!(
                file.entry(package).is_some(),
                "every base package needs an entry: {package} (for {program})"
            );
        }
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
    fn base_packages_arguments_read_as_an_architecture_and_a_test_flag() {
        assert_eq!(
            parse_base_packages_arguments(vec!["aarch64".into()]).unwrap(),
            ("aarch64".into(), false)
        );
        assert_eq!(
            parse_base_packages_arguments(vec!["x86_64".into(), "--test".into()]).unwrap(),
            ("x86_64".into(), true)
        );
        assert_eq!(
            parse_base_packages_arguments(vec!["--test".into(), "riscv64".into()]).unwrap(),
            ("riscv64".into(), true)
        );
        assert_eq!(
            parse_base_packages_arguments(vec![]).unwrap_err(),
            "no architecture"
        );
        assert_eq!(
            parse_base_packages_arguments(vec!["aarch64".into(), "--from".into()]).unwrap_err(),
            "unknown argument --from"
        );
        assert_eq!(
            parse_base_packages_arguments(vec!["aarch64".into(), "riscv64".into()]).unwrap_err(),
            "one architecture only, not also riscv64"
        );
    }

    #[test]
    fn a_test_definition_carries_the_fixtures_package() {
        // `base-packages --test` and `base-image --test` ask for one set, so both read it here.
        let definition = parse_definition(DEFINITION).unwrap();
        assert_eq!(
            with_test_programs(definition.clone(), false).unwrap(),
            definition
        );
        let test = with_test_programs(definition, true).unwrap();
        assert!(test.programs.contains(&"progenitor".to_string()));
        assert!(test.programs.contains(&"kernel_test_subject".to_string()));
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
