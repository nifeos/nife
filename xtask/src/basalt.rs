//! **basalt's published packages, fetched by digest for the gates** (milestone 810 (`ripgrep` is
//! packaged in basalt and installed with `jig`), DECISIONS §297 (software reaches a nife system as a
//! package `jig` installs from basalt) item 3).
//!
//! nife builds none of the foreign programs [`PACKAGES`] names (`rg`, `ioping`, `hackbench`,
//! `speedtest1`); its tests take basalt's published package instead (calef, 2026-10-13 UTC: *"1 is
//! preferable because that tests jig and basalt's delivery mechanism as well."*). First-party
//! `rmle` and `demos` are pinned for `jig install`; nife's tests run their own builds (milestone
//! 909 (basalt composes the bootable image from its packages) item 5).
//!
//! Name: provisional, `cargo xtask basalt-fetch`, minted 2026-10-13 (UTC) by lane
//! `lane/ripgrep-from-basalt`.

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

//! # Environment
//!
//! When [`crate::disk::BASALT_REPOSITORY`] names a repository, as basalt's own `packages` workflow
//! does with the one it just built, nothing is fetched: that repository's packages are accepted
//! under its own index and their programs taken from it. Without a network a local run says what
//! it could not fetch, succeeds, and skips every line and test of basalt's programs. In CI (`CI`
//! set) a failed fetch fails the row.

use std::path::{Path, PathBuf};

use crate::host::workspace_root;

/// **The basalt commit and Release nife's tests take their packages from**: two lines,
/// `commit <40 hex digits>` and `release <tag>`. The file is `xtask/basalt-pin`; its name and
/// format are provisional.
const PIN: &str = include_str!("../basalt-pin");

/// The packages nife's tests install, by their basalt recipe's file name.
const PACKAGES: [&str; 6] = ["ripgrep", "ioping", "hackbench", "sqlite", "rmle", "demos"];

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
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let (_, _, triple, want) = HOST_RIPGREP
        .iter()
        .find(|(o, a, _, _)| *o == os && *a == arch)
        .ok_or_else(|| format!("ripgrep publishes no {os} {arch} build this pins"))?;
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
fn download(url: &str) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new("curl")
        .args(["-sSfL", "--retry", "3", "--max-time", "120", url])
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
    fn a_recipe_without_every_digest_is_refused() {
        let missing = RECIPE.replace("x86_64 =", "x86_65 =");
        assert!(parse_recipe(&missing).unwrap_err().contains("x86_64"));
        let short = RECIPE.replace("0001", "1");
        assert!(parse_recipe(&short).unwrap_err().contains("64 hex"));
        assert!(parse_recipe("name = \"x\"\nversion = \"1\"\n").is_err());
    }
}
