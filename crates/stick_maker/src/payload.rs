//! **The boot files this program carries, and the note it leaves beside them.**
//!
//! The payload is a set of UEFI applications at the removable-media paths UEFI 2.10, 3.5.1.1 fixes
//! per architecture: `\EFI\BOOT\BOOTX64.EFI` for `x86_64`, `BOOTAA64.EFI` for aarch64 and
//! `BOOTRISCV64.EFI` for riscv64. Each firmware loads only its own name, so one stick boots on all
//! three and nobody picks a target (U1 in the roadmap block). Each file is one sealed application
//! holding its kernel and the archive that kernel measures, and the loader's build refuses a
//! mismatched pair (`uefi_loader/build.rs`), so this program cannot pair a kernel with the wrong
//! archive. See notes/boot-stick/the-program.md.

use measured_boot::{hex, sha256};

/// One embedded file, at the path it is written to on the stick.
#[derive(Debug, Clone, Copy)]
pub struct File {
    /// Relative, `/`-separated: `EFI/BOOT/BOOTX64.EFI`.
    pub path: &'static str,
    /// Its bytes.
    pub bytes: &'static [u8],
}

/// Where the note goes, at the stick's root.
pub const NOTE_PATH: &str = "NIFE.TXT";

/// The architecture a removable-media path boots, by the name the UEFI specification gives it.
pub fn architecture(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next()?.to_ascii_uppercase();
    match name.as_str() {
        "BOOTX64.EFI" => Some("x86_64"),
        "BOOTAA64.EFI" => Some("aarch64"),
        "BOOTRISCV64.EFI" => Some("riscv64"),
        _ => None,
    }
}

/// The bytes the whole set occupies.
pub fn total(files: &[File]) -> u64 {
    files.iter().map(|f| f.bytes.len() as u64).sum()
}

/// **The note written to `NIFE.TXT`**: what is on the stick, from which build, and the digest of
/// each file.
///
/// It exists because of a BUGS entry in notes/x86-uefi-boot.md: *a stale `.efi` on a stick is
/// silent*. A stick written by this program says what it carries in a file anyone can open on any
/// computer, and the digests let a bench session confirm the stick holds the build it thinks it
/// does (`shasum -a 256 /Volumes/NIFE/EFI/BOOT/*`).
pub fn note(files: &[File], build: &str) -> String {
    let mut text = String::new();
    text.push_str("nife boot stick\n");
    text.push_str(&format!("Written by stick_maker, build {build}.\n\n"));
    text.push_str(
        "Each machine's firmware finds its own file below and ignores the others, so this one\n\
         stick boots every architecture listed. Nothing on it needs configuring.\n\n",
    );
    for file in files {
        let digest = hex(&sha256(file.bytes));
        let digest = String::from_utf8_lossy(&digest).into_owned();
        text.push_str(&format!(
            "{:<8} {:<24} {:>10} bytes  sha256 {digest}\n",
            architecture(file.path).unwrap_or("?"),
            file.path,
            file.bytes.len(),
        ));
    }
    text.push_str(
        "\nSecure Boot must be off: these files are not signed.\n\
         To make another stick, run stick_maker again; it replaces these files and nothing else.\n",
    );
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_removable_media_name_names_its_architecture() {
        assert_eq!(architecture("EFI/BOOT/BOOTX64.EFI"), Some("x86_64"));
        assert_eq!(architecture("EFI/BOOT/BOOTAA64.EFI"), Some("aarch64"));
        assert_eq!(architecture("EFI/BOOT/BOOTRISCV64.EFI"), Some("riscv64"));
        // FAT is case-insensitive and so is the firmware's lookup.
        assert_eq!(architecture("efi/boot/bootx64.efi"), Some("x86_64"));
        assert_eq!(architecture("EFI/BOOT/BOOTIA32.EFI"), None);
    }

    #[test]
    fn the_note_carries_each_file_and_its_digest() {
        let files = [
            File {
                path: "EFI/BOOT/BOOTX64.EFI",
                bytes: b"abc",
            },
            File {
                path: "EFI/BOOT/BOOTAA64.EFI",
                bytes: b"",
            },
        ];
        let text = note(&files, "test-build");
        assert!(text.contains("build test-build"));
        // The FIPS 180-4 "abc" vector: the note's digest is the one `shasum -a 256` prints.
        assert!(text.contains(
            "x86_64   EFI/BOOT/BOOTX64.EFI              3 bytes  sha256 \
             ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        ));
        assert!(text.contains("aarch64  EFI/BOOT/BOOTAA64.EFI"));
        assert_eq!(total(&files), 3);
    }
}
