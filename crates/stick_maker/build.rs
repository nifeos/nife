//! **Embed the boot files in the program**, so the download is one file.
//!
//! `NIFE_STICK_PAYLOADS` names a directory laid out the way the stick will be (`EFI/BOOT/*.EFI`),
//! which `cargo xtask stick` builds. Every file under `EFI/BOOT/` is embedded at its relative path.
//! Each is already sealed, a UEFI loader carrying its kernel and the archive that kernel measures
//! (`uefi_loader/build.rs` refuses a pair that does not match), so nothing here pairs anything.
//!
//! Unset, the program builds with no boot files and refuses to write a stick, so `cargo test` and
//! `script/lint` work with no environment. See notes/boot-stick/the-program.md.

use std::path::{Path, PathBuf};
use std::{env, fs};

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if let Ok(relative) = path.strip_prefix(root) {
            let relative: Vec<String> = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.push((relative.join("/"), path));
        }
    }
}

fn main() {
    println!("cargo::rerun-if-env-changed=NIFE_STICK_PAYLOADS");
    println!("cargo::rerun-if-env-changed=NIFE_STICK_BUILD");

    let mut files = Vec::new();
    if let Some(root) = env::var_os("NIFE_STICK_PAYLOADS").filter(|v| !v.is_empty()) {
        let root = PathBuf::from(root);
        let boot = root.join("EFI").join("BOOT");
        assert!(
            boot.is_dir(),
            "NIFE_STICK_PAYLOADS={} has no EFI/BOOT directory; `cargo xtask stick` builds one",
            root.display()
        );
        println!("cargo::rerun-if-changed={}", boot.display());
        collect(&root, &boot, &mut files);
        files.sort();
        assert!(
            !files.is_empty(),
            "NIFE_STICK_PAYLOADS={} holds no boot files",
            root.display()
        );
    }

    let mut generated = String::from("/// The boot files, at the paths they are written to.\n");
    generated.push_str("pub static FILES: &[File] = &[\n");
    for (relative, path) in &files {
        println!("cargo::rerun-if-changed={}", path.display());
        generated.push_str(&format!(
            "    File {{ path: {relative:?}, bytes: include_bytes!({:?}) }},\n",
            path.display().to_string()
        ));
    }
    generated.push_str("];\n");
    let build = env::var("NIFE_STICK_BUILD").unwrap_or_else(|_| "unlabelled".to_owned());
    generated.push_str(&format!(
        "/// Which build of nife the files came from.\npub static BUILD: &str = {build:?};\n"
    ));

    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("embedded.rs");
    fs::write(out, generated).expect("the build directory is writable");
}
