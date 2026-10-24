//! **Writing the set onto a mounted volume, so that a failure part-way leaves the old file.**
//!
//! Each file is written to a temporary name beside its destination, flushed to the device, renamed
//! over the old one, and then read back and compared. A stick pulled out mid-write therefore holds
//! either last time's file or this time's, never half of one, and a write the device silently
//! mangled is reported rather than discovered at a firmware prompt.

//! # BUGS
//!
//! - **The read-back may be served from the host's cache**, so it proves the bytes reached the
//!   filesystem, not the flash. `File::sync_all` (`F_FULLFSYNC` on macOS, `fsync` elsewhere) asks
//!   the device to commit; ejecting is what guarantees it.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use crate::payload::File;

/// Where one file landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// Its full path on the volume.
    pub path: PathBuf,
    /// Bytes written.
    pub bytes: u64,
    /// Whether a file of that name was already there and has been replaced.
    pub replaced: bool,
}

fn destination(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |p, part| p.join(part))
}

/// How many bytes the set's files already occupy on the volume at `root`, which a copy frees.
pub fn replaced_bytes(root: &Path, files: &[File]) -> u64 {
    files
        .iter()
        .filter_map(|f| fs::metadata(destination(root, f.path)).ok())
        .map(|m| m.len())
        .sum()
}

/// Write one file safely (see the module documentation).
pub fn write_one(root: &Path, relative: &str, bytes: &[u8]) -> io::Result<Written> {
    let path = destination(root, relative);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("a payload path with no directory"))?;
    fs::create_dir_all(parent)?;
    let replaced = path.exists();

    // One temporary name per destination directory. FAT keeps long names, so it can be
    // descriptive; the leading "nife" means a leftover from an interrupted run is recognisable.
    let temporary = parent.join("nife-stick-maker.partial");
    {
        let mut out = fs::File::create(&temporary)?;
        out.write_all(bytes)?;
        out.sync_all()?;
    }
    fs::rename(&temporary, &path)?;

    let back = fs::read(&path)?;
    if back != bytes {
        return Err(io::Error::other(format!(
            "{} read back differently from what was written ({} bytes against {})",
            path.display(),
            back.len(),
            bytes.len()
        )));
    }
    Ok(Written {
        path,
        bytes: bytes.len() as u64,
        replaced,
    })
}

/// Write every file in the set and then the note, in that order, so a note never describes files
/// that are not there.
pub fn write_set(root: &Path, files: &[File], note: &str) -> io::Result<Vec<Written>> {
    let mut written = Vec::with_capacity(files.len() + 1);
    for file in files {
        written.push(write_one(root, file.path, file.bytes)?);
    }
    written.push(write_one(root, crate::payload::NOTE_PATH, note.as_bytes())?);
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stick_maker-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const SET: [File; 2] = [
        File {
            path: "EFI/BOOT/BOOTX64.EFI",
            bytes: b"new x86",
        },
        File {
            path: "EFI/BOOT/BOOTAA64.EFI",
            bytes: b"new arm",
        },
    ];

    #[test]
    fn writes_the_set_and_the_note_and_leaves_nothing_else() {
        let root = scratch("fresh");
        let written = write_set(&root, &SET, "note\n").unwrap();
        assert_eq!(written.len(), 3);
        assert!(written.iter().all(|w| !w.replaced));
        assert_eq!(
            fs::read(root.join("EFI/BOOT/BOOTX64.EFI")).unwrap(),
            b"new x86"
        );
        assert_eq!(fs::read(root.join("NIFE.TXT")).unwrap(), b"note\n");
        let mut names: Vec<String> = fs::read_dir(root.join("EFI/BOOT"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["BOOTAA64.EFI", "BOOTX64.EFI"],
            "no temporary left behind"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replaces_an_older_set_and_leaves_a_stranger_alone() {
        let root = scratch("refresh");
        fs::create_dir_all(root.join("EFI/BOOT")).unwrap();
        fs::write(root.join("EFI/BOOT/BOOTX64.EFI"), b"old, and longer").unwrap();
        fs::write(root.join("holiday.jpg"), b"somebody's photo").unwrap();
        assert_eq!(replaced_bytes(&root, &SET), 15);

        let written = write_set(&root, &SET, "note\n").unwrap();
        assert!(written[0].replaced);
        assert!(!written[1].replaced);
        assert_eq!(
            fs::read(root.join("EFI/BOOT/BOOTX64.EFI")).unwrap(),
            b"new x86"
        );
        assert_eq!(
            fs::read(root.join("holiday.jpg")).unwrap(),
            b"somebody's photo"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
