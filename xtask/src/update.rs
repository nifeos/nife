//! **A slot is updated with the composed image**: `cargo xtask update-boot`, the third part of
//! milestone 976 (an OS update is a set of packages that lands through a slot).
//!
//! `confirm-boot` stages a copy of the image it just installed. This gate stages the file that
//! `cargo xtask base-image x86_64 --release` writes instead: the loader, the kernel and the archive
//! that basalt's composer built from packages, whose catalog lists exactly the packages the image
//! used. Then it runs `confirm-boot`'s three boots unchanged (`confirm::confirm_an_upgrade`).
//!
//! # What it asserts
//!
//! Everything `confirm-boot` asserts: boot 2 chooses slot 1, spends its one try, comes up and
//! confirms; boot 3 chooses slot 1 with no tries left. And one thing more, from boot 2's `jig list`:
//! the lines of the composed archive's own catalog appear, and no line of a package the installing
//! image vouches for and the composed one does not. A slot that held the install image again would
//! pass `confirm-boot` and fail here.

//! Name: `update-boot` is provisional, minted 2026-11-01 (UTC) by lane
//! lane/976-part-3-the-update-boot-gate. It follows `confirm-boot` and `rollback-boot`, the gates
//! it shares its boots with, and says what the slot receives. calef has not ruled on it.
//!
//! # BUGS
//!
//! - **Version 1 slot staging by the host.** The composed file goes into slot 1 through
//!   `rollback::stage_an_upgrade`, the version 1 header, written by the gate. This proves the slot
//!   path takes a composed image and a boot confirms it; it does not prove a device updater, which
//!   does not exist yet (parts 4 and 5 of the milestone).
//! - `jig list` prints names and versions, not digests, so the catalog is checked by what the shell
//!   shows of it.
//! - Everything `confirm-boot` lists: x86_64 only, one OVMF, no power cut, several minutes under
//!   TCG, so it is not in `script/test`'s default legs and no workflow runs it.

use crate::confirm::{Upgrade, confirm_an_upgrade};

/// The architecture every line here is about.
const ARCHITECTURE: &str = "x86_64";

/// **The gate.** `true` only if the composed image, staged into slot 1, was confirmed by its own
/// boot and then chosen again with no tries left, with its own catalog on the console.
pub(crate) fn update_boot() -> bool {
    confirm_an_upgrade("update-boot", the_composed_upgrade)
}

/// **Compose the image and say what its catalog must show.** Runs once the install image is built,
/// because the catalog it is told apart from is the one that image vouches for.
fn the_composed_upgrade() -> Option<Upgrade> {
    let Some(composed) = crate::base_image::compose_release_x86_64() else {
        eprintln!("update-boot: the composed image could not be built");
        return None;
    };
    let installing = crate::package::image_catalog(ARCHITECTURE)
        .inspect_err(|complaint| eprintln!("update-boot: {complaint}"))
        .ok()?;
    let composed_catalog = composed_catalog(&composed.join("initrd.img"))
        .inspect_err(|complaint| eprintln!("update-boot: {complaint}"))
        .ok()?;
    let (catalog_lines, not_in_catalog) = catalog_expectations(&composed_catalog, &installing);
    if catalog_lines.is_empty() {
        eprintln!(
            "update-boot: the composed archive's catalog lists no package, so it shows nothing"
        );
        return None;
    }
    Some(Upgrade {
        boot_file: Some(composed.join("esp/EFI/BOOT/BOOTX64.EFI")),
        catalog_lines,
        not_in_catalog,
    })
}

/// The catalog entry of a composed archive, which is what `jig list` prints on the machine that
/// boots it.
fn composed_catalog(archive: &std::path::Path) -> Result<String, String> {
    let bytes =
        std::fs::read(archive).map_err(|e| format!("could not read {}: {e}", archive.display()))?;
    let fs = nifefs::Fs::parse(&bytes)
        .map_err(|e| format!("{} does not parse: {e:?}", archive.display()))?;
    fs.read(package_archive::CATALOG)
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .ok_or_else(|| {
            format!(
                "{} carries no {}",
                archive.display(),
                package_archive::CATALOG
            )
        })
}

/// **What `jig list` must and must not print**, from the composed catalog and the installing
/// image's: the first in catalog order, the second the lines only the installing image has.
fn catalog_expectations(composed: &str, installing: &str) -> (Vec<String>, Vec<String>) {
    let shown = |catalog: &str| -> Vec<String> { catalog.lines().filter_map(listed).collect() };
    let must = shown(composed);
    let must_not = shown(installing)
        .into_iter()
        .filter(|line| !must.contains(line))
        .collect();
    (must, must_not)
}

/// **The line `jig list` prints for a catalog line**: two spaces, the package's name and version.
/// `jig`'s own parse of a stem (`name-version-arch`), which that program keeps private.
fn listed(line: &str) -> Option<String> {
    let stem = line.split(' ').next()?;
    let (name, version) = stem
        .strip_suffix(ARCHITECTURE)?
        .strip_suffix('-')?
        .rsplit_once('-')?;
    Some(format!("  {name} {version}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalog_line_is_listed_as_jig_prints_it() {
        assert_eq!(
            listed("uptime-0.1.0-x86_64 sha256:00").as_deref(),
            Some("  uptime 0.1.0")
        );
        assert_eq!(listed("# a comment"), None);
        assert_eq!(listed("uptime-0.1.0-aarch64 sha256:00"), None);
    }

    #[test]
    fn the_expectations_separate_the_two_images() {
        let composed = "a-0.1.0-x86_64 sha256:1\nb-0.2.0-x86_64 sha256:2\n";
        let installing = "a-0.1.0-x86_64 sha256:1\nc-0.3.0-x86_64 sha256:3\n";
        let (must, must_not) = catalog_expectations(composed, installing);
        assert_eq!(must, ["  a 0.1.0", "  b 0.2.0"]);
        assert_eq!(must_not, ["  c 0.3.0"]);
    }
}
