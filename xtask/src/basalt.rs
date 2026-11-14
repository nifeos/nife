//! **basalt's published packages, fetched by digest for the gates** (milestone 810 (`ripgrep` is
//! packaged in basalt and installed with `jig`), DECISIONS §297 (software reaches a nife system as a
//! package `jig` installs from basalt) item 3). Name: provisional, `cargo xtask basalt-fetch`,
//! minted 2026-10-13 (UTC) by lane `lane/ripgrep-from-basalt`.
//!
//! nife builds none of the foreign programs [`PACKAGES`] names (`rg`, `ioping`, `hackbench`,
//! `speedtest1`); its tests take basalt's published package instead (calef, 2026-10-13 UTC: *"1 is
//! preferable because that tests jig and basalt's delivery mechanism as well."*). First-party
//! `demos` is pinned for `jig install`; nife's tests run their own builds (milestone
//! 909 (basalt composes the bootable image from its packages) item 5).

//! # What is pinned
//!
//! A basalt commit and the Release it published ([`PIN`], the file `xtask/basalt-pin`), and no
//! digest. basalt builds each package from its `recipes/` and publishes one per architecture on a
//! GitHub Release, with an index at `basalt.nifeos.org`. Each package's SHA-256 is read from the
//! recipe at the pinned commit (`recipes/<name>.toml`'s `[package_digest]`, which basalt's build
//! refuses to publish unless the package matches; milestone 917 (basalt's packages build
//! reproducibly, and the recipe records each package's digest)). A republish needs one edit here,
//! the pin, and never a digest.

//! # What it writes
//!
//! For each package of [`PACKAGES`] and each architecture, this:
//!
//! 1. fetches the package from the Release, refused unless `package_index::accept`, the
//!    installer's own decision, takes it under the recipe's SHA-256;
//! 2. writes it into a repository in basalt's layout under `target/basalt/`, whose index
//!    [`crate::disk`] hands to the test index host for `script/swish-check`'s `jig install` lines;
//! 3. writes its program member to `target/basalt/programs/<triple>/<program>`, which the kernel
//!    harness's archive and the shell check's disk carry for runs by path.

//! # The host's reference `rg`
//!
//! It also fetches ripgrep's own 14.1.1 for the host, pinned by SHA-256 per host
//! ([`HOST_RIPGREP`]). `script/swish-check` compares the guest's walk against what it prints, and
//! `walk_pricing` holds it to the fixture's lines.

//! # Built from the candidate tree, in the merge queue
//!
//! `cargo xtask basalt-build <architecture>...` builds the same packages from this checkout's
//! committed tree instead of fetching them (milestone 1115 (the gate builds basalt's delivered
//! programs from the candidate tree)). It checks out basalt at the commit [`PIN`] names into a
//! scratch directory and runs that commit's `helpers/packages.py build` against this checkout, for
//! the architectures named and the packages of [`PACKAGES`], into [`built_repository`]. Every
//! program links nife's standard library and the C programs nife's C library, so a program built
//! this way reads the candidate's wire formats. The four merge-group rows that run basalt's
//! programs (`swish-check`, `swish-check-x86_64`, `rg-threads`, `rg-threads-x86_64`) each build the
//! architectures they run and hand the result to `basalt-fetch` with
//! [`crate::disk::BASALT_REPOSITORY`]. A change that breaks a wire format a delivered program reads
//! lands in one pull request, with the rebuilt program tested in its merge group.
//!
//! The build root is a directory of its own for each run, under the system's temporary directory,
//! so two runs on one host do not share basalt's fixed `/tmp/basalt-build`. The scratch checkout and
//! the build root are removed when the run ends. The packages differ from the published ones: they
//! are built from another tree, at another path. basalt's `packages` workflow tests the published
//! bytes, and runs nife's `swish-check` rows on them, at every pin bump.

//! # Environment
//!
//! When [`crate::disk::BASALT_REPOSITORY`] names a repository, as basalt's own `packages` workflow
//! does with the one it just built and nife's merge-group rows do with [`built_repository`],
//! nothing is fetched: that repository's packages are accepted under its own index and their
//! programs taken from it. Without a network a local run says what it could not fetch, succeeds,
//! and skips every line and test of basalt's programs. In CI (`CI` set) a failed fetch fails the
//! row.

//! # BUGS
//!
//! - **Only the merge queue and a push to `main` build from the tree.** A pull request's `full-ci`
//!   run and a local `script/ci-build` fetch the published programs, so a change that breaks a
//!   delivered program's wire format is red there and green in its merge group. To see what the
//!   queue sees, run `cargo xtask basalt-build aarch64 riscv64 x86_64` and set
//!   `NIFE_BASALT_REPOSITORY` to the directory it prints.
//! - **The digest check's refusal is read as success.** basalt's build fails (exit status 1) when a
//!   package differs from the digest its recipe records, which a package built from any tree but
//!   the one basalt pins always does. At the pinned basalt commit `--update-digests` refuses a build
//!   of fewer than every architecture, so it cannot set the check aside. The build writes its index
//!   only after every package is built and packed, so [`build`] takes a failed run whose fresh index
//!   lists every package it asked for as a built one. A basalt flag that skips the check would
//!   remove the inference.
//! - **The build root is set by assigning `BUILD_ROOT`**, a module-level name in basalt's
//!   `helpers/packages.py`, after importing it, because the script takes no option for it. A
//!   basalt change that renames it fails the build at once and says so. Its `fetch` is wrapped
//!   the same way, to try a download again when the route to a source host drops for a moment.
//! - **`xtask/basalt-pin` does not follow basalt.** It names basalt commit `155eba5`, whose own
//!   pin is a nife commit of 2026-10-17. The merge-group rows run that commit's recipes and build
//!   script, and every other gate fetches that commit's published programs. A recipe basalt adds
//!   or changes reaches nife only when someone moves the pin by hand.
//! - Each build stages a nife tree at a new path, and `cargo xtask std-src` links a toolchain for
//!   it (`rustup toolchain link`). The link dangles once the build root is removed, and the next
//!   `std-src` on the host sweeps it.

use std::path::{Path, PathBuf};

use crate::host::workspace_root;

/// **The basalt commit and Release nife's tests take their packages from**: two lines,
/// `commit <40 hex digits>` and `release <tag>`. The file is `xtask/basalt-pin`; its name and
/// format are provisional.
const PIN: &str = include_str!("../basalt-pin");

/// The packages nife's tests install, by their basalt recipe's file name.
const PACKAGES: [&str; 5] = ["ripgrep", "ioping", "hackbench", "sqlite", "demos"];

/// The architectures every recipe records a digest for.
const ARCHITECTURES: [&str; 3] = ["aarch64", "riscv64", "x86_64"];

/// The root name github.com's certificate chain is carried under in nife's images
/// (`package_index::BASALT_SOURCES`'s `location-root`), which basalt's index lines name too.
const RELEASE_ROOT: &str = "sectigo-root-e46";

/// The channel basalt's repository is laid out under (`package_index::PROVISIONAL_CHANNEL`).
const CHANNEL: &str = "rolling";

/// **ripgrep 14.1.1 for the host, from its own Release** (`BurntSushi/ripgrep`), by host OS and
/// architecture: the asset's target triple and the SHA-256 its `.sha256` file published, read
/// 2026-10-13 (UTC). A host not listed has no reference `rg`, and the walk's expected answer falls
/// back to whatever `rg` is on `PATH`, whose version the staging line prints.
const HOST_RIPGREP: [(&str, &str, &str, &str); 4] = [
    (
        "macos",
        "aarch64",
        "aarch64-apple-darwin",
        "24ad76777745fbff131c8fbc466742b011f925bfa4fffa2ded6def23b5b937be",
    ),
    (
        "macos",
        "x86_64",
        "x86_64-apple-darwin",
        "fc87e78f7cb3fea12d69072e7ef3b21509754717b746368fd40d88963630e2b3",
    ),
    (
        "linux",
        "aarch64",
        "aarch64-unknown-linux-gnu",
        "c827481c4ff4ea10c9dc7a4022c8de5db34a5737cb74484d62eb94a95841ab2f",
    ),
    (
        "linux",
        "x86_64",
        "x86_64-unknown-linux-musl",
        "4cf9f2741e6c465ffdb7c26f38056a59e2a2544b51f7cc128ef28337eeae4d8e",
    ),
];

/// The ripgrep version [`HOST_RIPGREP`] names, which must be the one basalt's recipe builds.
const HOST_RIPGREP_VERSION: &str = "14.1.1";

/// **The repository the gates read**: the one [`crate::disk::BASALT_REPOSITORY`] names, or the
/// one `cargo xtask basalt-fetch` writes, `target/basalt`.
pub(crate) fn repository() -> PathBuf {
    std::env::var_os(crate::disk::BASALT_REPOSITORY)
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target/basalt"))
}

/// The repository's index, which exists only once a fetch succeeded or basalt handed one over.
pub(crate) fn repository_index() -> PathBuf {
    repository().join(CHANNEL).join("metadata/stand-in-index")
}

/// **`rg` for `triple`, taken from basalt's package**: present only after `cargo xtask
/// basalt-fetch`. Never part of the base image (calef, 2026-10-07 UTC); the kernel harness's
/// archive and the shell check's disk carry it when it is here.
pub(crate) fn ripgrep_elf(triple: &str) -> PathBuf {
    program_elf(triple, "rg")
}

/// **`ioping` for `triple`, taken from basalt's package** (milestone 906 (ioping is packaged in
/// basalt, and its test installs it with `jig`)): present only after `cargo xtask basalt-fetch`, on
/// [`ripgrep_elf`]'s terms. The kernel harness's archive carries it when it is here.
pub(crate) fn ioping_elf(triple: &str) -> PathBuf {
    program_elf(triple, "ioping")
}

/// **`hackbench` for `triple`, taken from basalt's package** (milestone 907 (hackbench is packaged
/// in basalt, and its test installs it with `jig`)), on [`ioping_elf`]'s terms.
pub(crate) fn hackbench_elf(triple: &str) -> PathBuf {
    program_elf(triple, "hackbench")
}

/// **SQLite's `speedtest1` for `triple`, the program of basalt's `sqlite` package** (milestone 897
/// (SQLite is packaged in basalt)), on [`ioping_elf`]'s terms.
pub(crate) fn speedtest1_elf(triple: &str) -> PathBuf {
    program_elf(triple, "speedtest1")
}

/// Where [`take_programs`] writes a package's program member for `triple`.
fn program_elf(triple: &str, program: &str) -> PathBuf {
    workspace_root().join(format!("target/basalt/programs/{triple}/{program}"))
}

/// **The host's reference `rg`**, ripgrep's own 14.1.1 build for this host, present only after
/// `cargo xtask basalt-fetch`. `walk_pricing` reads the same path.
pub(crate) fn host_ripgrep() -> PathBuf {
    workspace_root().join("target/ripgrep/host/rg")
}

/// `cargo xtask basalt-fetch`: fetch and check everything above. Returns whether the run passed.
pub(crate) fn fetch() -> bool {
    let in_ci = std::env::var_os("CI").is_some();
    let mut ok = true;
    let packages = match std::env::var_os(crate::disk::BASALT_REPOSITORY) {
        Some(_) => take_programs(&repository()),
        None => fetch_pinned().and_then(|()| take_programs(&repository())),
    };
    for result in [packages, fetch_host_ripgrep()] {
        match result {
            Ok(said) => println!("basalt-fetch: {said}"),
            Err(why) if in_ci => {
                eprintln!("basalt-fetch: {why}");
                ok = false;
            }
            Err(why) => eprintln!(
                "basalt-fetch: {why}; the lines and tests of basalt's programs skip without it \
                 and say so"
            ),
        }
    }
    ok
}

/// **Where `cargo xtask basalt-build` writes the repository it built**, in basalt's layout, which
/// the merge-group rows name in [`crate::disk::BASALT_REPOSITORY`].
pub(crate) fn built_repository() -> PathBuf {
    workspace_root().join("target/basalt-built")
}

/// The program `python3 -I -c` runs: basalt's `helpers/packages.py` imported from the directory
/// its first argument names, its `BUILD_ROOT` set to the second, and its own `main` run on the
/// rest. The module's `if __name__` guard keeps the import from running anything.
///
/// Its `fetch` is wrapped to try a download five times, waiting 2, 4, 8 and 16 seconds between
/// tries, because hosted runners lose the route to a source host for a few seconds at a time: the
/// first merge group to run this build failed two of four rows on `[Errno 101] Network is
/// unreachable` from `git.kernel.org`, while the other two rows fetched the same file in the same
/// minute. The recipe's digest, checked after every try, still decides whether the bytes are taken.
const PACKAGES_WITH_BUILD_ROOT: &str = r#"
import pathlib, sys, time
helpers, root = sys.argv[1], sys.argv[2]
sys.path.insert(0, helpers)
import packages
if not isinstance(getattr(packages, "BUILD_ROOT", None), pathlib.Path):
    sys.exit("basalt-build: helpers/packages.py has no BUILD_ROOT path to set")
if not callable(getattr(packages, "fetch", None)):
    sys.exit("basalt-build: helpers/packages.py has no fetch to wrap")
packages.BUILD_ROOT = pathlib.Path(root)
fetch_once = packages.fetch
def fetch_with_retries(url, path, want):
    for wait in (2, 4, 8, 16):
        try:
            return fetch_once(url, path, want)
        except OSError as error:
            print(f"basalt-build: {url}: {error}; trying again in {wait} seconds", flush=True)
            time.sleep(wait)
    return fetch_once(url, path, want)
packages.fetch = fetch_with_retries
sys.argv = ["packages.py", *sys.argv[3:]]
packages.main()
"#;

/// `cargo xtask basalt-build <architecture>...`: build [`PACKAGES`] for those architectures from
/// this checkout's committed tree with basalt's build at [`PIN`], into [`built_repository`]. Returns
/// whether every package was built.
pub(crate) fn build() -> bool {
    let named: Vec<String> = std::env::args().skip(2).collect();
    let architectures = match parse_architectures(&named) {
        Ok(a) => a,
        Err(why) => {
            eprintln!("basalt-build: {why}");
            eprintln!("usage: cargo xtask basalt-build <aarch64|riscv64|x86_64>...");
            return false;
        }
    };
    let nanoseconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let scratch = std::env::temp_dir().join(format!(
        "nife-basalt-build-{}-{nanoseconds}",
        std::process::id()
    ));
    let result = build_in(&scratch, &architectures);
    if let Err(e) = std::fs::remove_dir_all(&scratch) {
        eprintln!("basalt-build: could not remove {}: {e}", scratch.display());
    }
    match result {
        Ok(said) => {
            println!("basalt-build: {said}");
            true
        }
        Err(why) => {
            eprintln!("basalt-build: {why}");
            false
        }
    }
}

/// The architectures `named` lists, in [`ARCHITECTURES`] order, refused when empty or when one is
/// not an architecture basalt builds.
fn parse_architectures(named: &[String]) -> Result<Vec<&'static str>, String> {
    if named.is_empty() {
        return Err("no architecture named".to_owned());
    }
    if let Some(bad) = named.iter().find(|n| !ARCHITECTURES.contains(&n.as_str())) {
        return Err(format!(
            "`{bad}` is not one of {}",
            ARCHITECTURES.join(", ")
        ));
    }
    Ok(ARCHITECTURES
        .into_iter()
        .filter(|a| named.iter().any(|n| n == a))
        .collect())
}

/// [`build`]'s work, with `scratch` holding basalt's checkout and its build root.
fn build_in(scratch: &Path, architectures: &[&str]) -> Result<String, String> {
    let (commit, _) = parse_pin(PIN)?;
    let basalt = scratch.join("basalt");
    std::fs::create_dir_all(&basalt).map_err(|e| format!("{}: {e}", basalt.display()))?;
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(&basalt)
            .args(args)
            .status()
            .map_err(|e| format!("git: {e}"))?
            .success()
            .then_some(())
            .ok_or_else(|| format!("`git {}` failed in {}", args.join(" "), basalt.display()))
    };
    git(&["init", "-q"])?;
    git(&[
        "fetch",
        "-q",
        "--depth",
        "1",
        "https://github.com/nifeos/basalt",
        commit,
    ])?;
    git(&["checkout", "-q", "--detach", "FETCH_HEAD"])?;
    let repository = built_repository();
    if repository.exists() {
        std::fs::remove_dir_all(&repository)
            .map_err(|e| format!("{}: {e}", repository.display()))?;
    }
    let triples: Vec<String> = architectures
        .iter()
        .map(|a| format!("{a}-unknown-nife"))
        .collect();
    let status = std::process::Command::new("python3")
        .args(["-I", "-c", PACKAGES_WITH_BUILD_ROOT])
        .arg(basalt.join("helpers"))
        .arg(scratch.join("root"))
        .arg("build")
        .arg(workspace_root())
        .arg(&repository)
        .arg("--triples")
        .args(&triples)
        .arg("--only")
        .args(PACKAGES)
        .status()
        .map_err(|e| format!("python3: {e}"))?;
    let index_path = repository.join(CHANNEL).join("metadata/stand-in-index");
    let index = std::fs::read_to_string(&index_path).map_err(|e| {
        format!(
            "basalt's build at {commit} exited {status} and wrote no index ({}: {e})",
            index_path.display()
        )
    })?;
    let missing = missing_stems(&index, architectures);
    if !missing.is_empty() {
        return Err(format!(
            "{} lists no package for {}",
            index_path.display(),
            missing.join(", ")
        ));
    }
    let checked = if status.success() {
        "every package matched its recipe's digest"
    } else {
        "the digest check refused the packages, as it refuses any tree but the nife commit basalt pins"
    };
    Ok(format!(
        "{} packages built from this tree with basalt {}; {checked}; repository {}",
        PACKAGES.len() * architectures.len(),
        &commit[..7],
        repository.display()
    ))
}

/// Each `<package>-*-<architecture>` that `index` has no line for, for [`PACKAGES`] and
/// `architectures`. A stem is `<recipe name>-<version>-<architecture>`, and every recipe's name is
/// its file's stem.
fn missing_stems(index: &str, architectures: &[&str]) -> Vec<String> {
    let stems: Vec<&str> = index.lines().filter_map(|l| l.split(' ').next()).collect();
    let mut missing = Vec::new();
    for package in PACKAGES {
        for architecture in architectures {
            let found = stems.iter().any(|stem| {
                stem.strip_prefix(package)
                    .and_then(|rest| rest.strip_prefix('-'))
                    .and_then(|rest| rest.strip_suffix(architecture))
                    .is_some_and(|version| version.ends_with('-') && version.len() > 1)
            });
            if !found {
                missing.push(format!("{package} on {architecture}"));
            }
        }
    }
    missing
}

/// The index lines for [`PACKAGES`] at [`PIN`]: each recipe is read from basalt at the pinned
/// commit, and each package's line is `<stem> <recipe's digest> <root>=<Release URL>`.
fn pinned_index() -> Result<String, String> {
    let (commit, release) = parse_pin(PIN)?;
    let mut lines = String::new();
    for name in PACKAGES {
        let url =
            format!("https://raw.githubusercontent.com/nifeos/basalt/{commit}/recipes/{name}.toml");
        let text = String::from_utf8(download(&url)?).map_err(|_| format!("{url}: not UTF-8"))?;
        let recipe = parse_recipe(&text).map_err(|why| format!("{url}: {why}"))?;
        for (architecture, digest) in recipe.digests {
            let stem = format!("{}-{}-{architecture}", recipe.name, recipe.version);
            lines.push_str(&format!(
                "{stem} {digest} {RELEASE_ROOT}=https://github.com/nifeos/basalt/releases/download/{release}/{stem}.nifepkg\n"
            ));
        }
    }
    Ok(lines)
}

/// `(commit, release tag)` from `xtask/basalt-pin`'s text, refused unless the commit is 40
/// lowercase hex digits and the tag is basalt's `rolling-<date>-<first seven of the commit>`.
fn parse_pin(text: &str) -> Result<(&str, &str), String> {
    let field = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix(' '))
            .map(str::trim)
            .ok_or_else(|| format!("xtask/basalt-pin has no `{key}` line"))
    };
    let (commit, release) = (field("commit")?, field("release")?);
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!(
            "xtask/basalt-pin: `{commit}` is not a full lowercase commit id"
        ));
    }
    let shape_ok = release.strip_prefix("rolling-").is_some_and(|rest| {
        rest.len() == "YYYY-MM-DD-".len() + 7
            && rest.ends_with(&commit[..7])
            && rest.as_bytes()[10] == b'-'
    });
    if !shape_ok {
        return Err(format!(
            "xtask/basalt-pin: release `{release}` is not `rolling-<date>-{}`",
            &commit[..7]
        ));
    }
    Ok((commit, release))
}

/// What nife reads of a basalt recipe.
#[derive(Debug, PartialEq)]
struct Recipe {
    name: String,
    version: String,
    /// `(architecture, "sha256:<hex>")`, in [`ARCHITECTURES`] order.
    digests: Vec<(&'static str, String)>,
}

/// The package name, version and `[package_digest]` of a basalt recipe. Only those are read, so the
/// rest of a recipe is basalt's to change.
fn parse_recipe(text: &str) -> Result<Recipe, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| e.message().to_owned())?;
    let string = |t: &toml::Table, key: &str| match t.get(key) {
        Some(toml::Value::String(value)) => Ok(value.clone()),
        _ => Err(format!("no string `{key}`")),
    };
    let Some(toml::Value::Table(recorded)) = table.get("package_digest") else {
        return Err("no [package_digest] table".to_owned());
    };
    let mut digests = Vec::new();
    for architecture in ARCHITECTURES {
        let digest =
            string(recorded, architecture).map_err(|why| format!("[package_digest]: {why}"))?;
        let hex_ok = digest.strip_prefix("sha256:").is_some_and(|h| {
            h.len() == 64
                && h.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        });
        if !hex_ok {
            return Err(format!(
                "{architecture}: `{digest}` is not `sha256:<64 hex digits>`"
            ));
        }
        digests.push((architecture, digest));
    }
    Ok(Recipe {
        name: string(&table, "name")?,
        version: string(&table, "version")?,
        digests,
    })
}

/// Fetch every package [`pinned_index`] names into `target/basalt`, keeping a copy already there
/// that the index still accepts, and write the index beside them.
fn fetch_pinned() -> Result<(), String> {
    let pinned = pinned_index()?;
    let index = package_index::Index::parse(&pinned).map_err(|e| {
        format!(
            "the index built from basalt's recipes has line {} unreadable",
            e.number
        )
    })?;
    let root = workspace_root().join("target/basalt").join(CHANNEL);
    let targets = root.join("targets");
    std::fs::create_dir_all(&targets).map_err(|e| format!("{}: {e}", targets.display()))?;
    for entry in index.entries() {
        let file = targets.join(format!("{}.nifepkg", entry.stem));
        let kept =
            std::fs::read(&file).is_ok_and(|bytes| package_index::accept(&entry, &bytes).is_ok());
        if kept {
            continue;
        }
        let location = entry
            .locations()
            .next()
            .ok_or_else(|| format!("{} lists no location to fetch it from", entry.stem))?;
        let url = format!(
            "https://{}:{}{}",
            location.host, location.port, location.path
        );
        let bytes = download(&url)?;
        package_index::accept(&entry, &bytes)
            .map_err(|why| format!("{url}: refused under its recipe's digest ({why:?})"))?;
        std::fs::write(&file, &bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    let metadata = root.join("metadata");
    std::fs::create_dir_all(&metadata).map_err(|e| format!("{}: {e}", metadata.display()))?;
    std::fs::write(metadata.join("stand-in-index"), pinned)
        .map_err(|e| format!("{}: {e}", metadata.display()))
}

/// Accept each package `repository`'s index lists, by that index's digest, and write every program
/// it carries to [`ripgrep_elf`]'s directory for its architecture.
fn take_programs(repository: &Path) -> Result<String, String> {
    let index_path = repository.join(CHANNEL).join("metadata/stand-in-index");
    let text = std::fs::read_to_string(&index_path)
        .map_err(|e| format!("{}: {e}", index_path.display()))?;
    let index = package_index::Index::parse(&text)
        .map_err(|e| format!("{} line {} is unreadable", index_path.display(), e.number))?;
    let mut taken = Vec::new();
    for entry in index.entries() {
        let file = repository
            .join(CHANNEL)
            .join("targets")
            .join(format!("{}.nifepkg", entry.stem));
        let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let installable = package_index::accept(&entry, &bytes)
            .map_err(|why| format!("{}: refused by its index ({why:?})", file.display()))?;
        let architecture = entry.stem.rsplit('-').next().unwrap_or_default();
        let dir = workspace_root().join(format!(
            "target/basalt/programs/{architecture}-unknown-nife"
        ));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for program in installable.programs() {
            std::fs::write(dir.join(program.name), program.bytes)
                .map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        taken.push(entry.stem);
    }
    Ok(format!(
        "{} accepted from {}",
        taken.join(", "),
        repository.display()
    ))
}

/// Fetch ripgrep's own build for this host into [`host_ripgrep`], unless a copy is there already.
fn fetch_host_ripgrep() -> Result<String, String> {
    let rg = host_ripgrep();
    if rg.exists() {
        return Ok(format!("host rg already at {}", rg.display()));
    }
    let (operating_system, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let (_, _, triple, want) = HOST_RIPGREP
        .iter()
        .find(|(o, a, _, _)| *o == operating_system && *a == arch)
        .ok_or_else(|| format!("ripgrep publishes no {operating_system} {arch} build this pins"))?;
    let name = format!("ripgrep-{HOST_RIPGREP_VERSION}-{triple}");
    let url = format!(
        "https://github.com/BurntSushi/ripgrep/releases/download/{HOST_RIPGREP_VERSION}/{name}.tar.gz"
    );
    let bytes = download(&url)?;
    let got = measured_boot::digest_text(&package_archive::sha256(&bytes));
    if got[b"sha256:".len()..] != *want.as_bytes() {
        return Err(format!("{url}: SHA-256 is not the pinned {want}"));
    }
    let dir = rg.parent().expect("host_ripgrep has a parent");
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tarball = dir.join(format!("{name}.tar.gz"));
    std::fs::write(&tarball, &bytes).map_err(|e| format!("{}: {e}", tarball.display()))?;
    let unpacked = std::process::Command::new("tar")
        .arg("xzf")
        .arg(&tarball)
        .args(["--strip-components=1", "-C"])
        .arg(dir)
        .arg(format!("{name}/rg"))
        .status()
        .is_ok_and(|s| s.success());
    let _ = std::fs::remove_file(&tarball);
    if !unpacked || !rg.exists() {
        return Err(format!("could not unpack rg from {url}"));
    }
    Ok(format!(
        "host rg {HOST_RIPGREP_VERSION} at {}",
        rg.display()
    ))
}

/// The bytes at `url`, by `curl`, following redirects (a Release asset answers with one). Nothing
/// fetched is trusted until its caller checks a pinned digest, so the transport only has to arrive.
/// `--retry` alone retries only timeouts and HTTP 5xx, not a reset connection (curl exit 35 or 56),
/// so `--retry-all-errors` is what lets one reset not fail a required check.
fn download(url: &str) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new("curl")
        .args([
            "-sSfL",
            "--retry",
            "3",
            "--retry-all-errors",
            "--max-time",
            "120",
            url,
        ])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "could not fetch {url}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    /// The checked-in pin is one the fetch accepts, so a hand edit that breaks its shape fails
    /// here and not in a CI row that needs the network.
    #[test]
    fn the_checked_in_pin_parses() {
        parse_pin(PIN).expect("xtask/basalt-pin");
    }

    #[test]
    fn a_pin_needs_a_full_commit_and_the_tag_basalt_names() {
        let ok = format!("commit {COMMIT}\nrelease rolling-2026-10-17-0123456\n");
        assert_eq!(parse_pin(&ok), Ok((COMMIT, "rolling-2026-10-17-0123456")));
        for bad in [
            format!(
                "commit {}\nrelease rolling-2026-10-17-0123456\n",
                &COMMIT[..39]
            ),
            format!("commit {COMMIT}\nrelease rolling-2026-10-17-abcdef0\n"),
            format!("commit {COMMIT}\nrelease latest\n"),
            format!("commit {COMMIT}\n"),
        ] {
            assert!(parse_pin(&bad).is_err(), "{bad}");
        }
    }

    const RECIPE: &str = r#"
# comment with name = "wrong"
name = "ripgrep"
version = "14.1.1"

[[patch]]
crate = "ignore"
version = "0.4.23"

# note
[package_digest]
aarch64 = "sha256:0000000000000000000000000000000000000000000000000000000000000001"
riscv64 = "sha256:0000000000000000000000000000000000000000000000000000000000000002"
x86_64 = "sha256:0000000000000000000000000000000000000000000000000000000000000003"
"#;

    #[test]
    fn a_recipe_gives_its_name_version_and_one_digest_per_architecture() {
        let recipe = parse_recipe(RECIPE).unwrap();
        assert_eq!(
            (recipe.name.as_str(), recipe.version.as_str()),
            ("ripgrep", "14.1.1")
        );
        let digests: Vec<_> = recipe.digests.iter().map(|(a, d)| (*a, &d[70..])).collect();
        assert_eq!(
            digests,
            [("aarch64", "1"), ("riscv64", "2"), ("x86_64", "3")]
        );
    }

    #[test]
    fn architectures_are_named_from_the_three_basalt_builds() {
        let names = |n: &[&str]| n.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            parse_architectures(&names(&["riscv64", "aarch64"])),
            Ok(vec!["aarch64", "riscv64"])
        );
        assert!(parse_architectures(&[]).is_err());
        assert!(parse_architectures(&names(&["arm64"])).is_err());
    }

    #[test]
    fn an_index_missing_a_package_or_an_architecture_is_named() {
        let full: String = PACKAGES
            .iter()
            .map(|p| format!("{p}-1.0-x86_64 sha256:00\n"))
            .collect();
        assert!(missing_stems(&full, &["x86_64"]).is_empty());
        assert_eq!(
            missing_stems(&full, &["aarch64", "x86_64"]).len(),
            PACKAGES.len()
        );
        let without_sqlite = full.replace("sqlite-1.0-x86_64", "rmle-1.0-x86_64");
        assert_eq!(
            missing_stems(&without_sqlite, &["x86_64"]),
            ["sqlite on x86_64"]
        );
    }

    #[test]
    fn a_recipe_without_every_digest_is_refused() {
        let missing = RECIPE.replace("x86_64 =", "x86_65 =");
        assert!(parse_recipe(&missing).unwrap_err().contains("x86_64"));
        let short = RECIPE.replace("0001", "1");
        assert!(parse_recipe(&short).unwrap_err().contains("64 hex"));
        assert!(parse_recipe("name = \"x\"\nversion = \"1\"\n").is_err());
    }
}
