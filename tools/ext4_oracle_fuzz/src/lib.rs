//! **What the `ext4_server` fuzz targets share**: the generated ext4 corpus as copy-on-write
//! templates, a few fuzzer-chosen corruptions of one of them, and `ext4_server::Server` over the
//! result through the oracle's own `rsext4` adapter (`ext4_oracle_manifest::volume`).
//!
//! The corpus is what `tools/ext4_oracle/corpus/make-corpus.sh` builds, read from the directory
//! [`IMAGES_VARIABLE`] names (`script/fuzz` builds it and sets the variable). Each image is 64 MiB
//! and mostly zero, far past what a libFuzzer input should be, so an input never carries an image:
//! it chooses one, and the bytes it changes. An input is small, a run of it starts from an image
//! e2fsprogs wrote and `e2fsck` passed, and each corruption lands on a block that holds something.
//!
//! Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation. calef names things.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use ext4_oracle_manifest::volume::{self, Ext4Volume};
use ext4_server::{BlockDevice, ErrorNumber, Server};
use file_system_contract::{block, error, file_system};
use libfuzzer_sys::arbitrary::{Result, Unstructured};

/// The block the block capability serves, and the unit a template is stored in.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub const BLOCK: usize = block::BLOCK_BYTES;

/// **The environment variable naming the corpus directory**: every `*.img` in it is a template,
/// in name order.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub const IMAGES_VARIABLE: &str = "EXT4_ORACLE_FUZZ_IMAGES";

/// The most corruptions one input applies, and the most bytes each one writes.
const MAXIMUM_CORRUPTIONS: usize = 8;
const MAXIMUM_CORRUPTION_BYTES: usize = 8;

/// One corpus image: its size and its nonzero blocks. A block not stored reads as zeros.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub struct Template {
    size: u64,
    blocks: HashMap<u64, Box<[u8; BLOCK]>>,
    /// The stored blocks' indices, sorted: where a corruption may land.
    nonzero: Vec<u64>,
}

impl Template {
    fn read(path: &Path) -> Template {
        let bytes = std::fs::read(path)
            .unwrap_or_else(|e| refuse(&format!("cannot read {}: {e}", path.display())));
        let mut blocks = HashMap::new();
        for (index, chunk) in bytes.chunks(BLOCK).enumerate() {
            if chunk.iter().any(|&b| b != 0) {
                let mut whole = Box::new([0; BLOCK]);
                whole[..chunk.len()].copy_from_slice(chunk);
                blocks.insert(index as u64, whole);
            }
        }
        let mut nonzero: Vec<u64> = blocks.keys().copied().collect();
        nonzero.sort_unstable();
        Template {
            size: bytes.len() as u64,
            blocks,
            nonzero,
        }
    }
}

/// Stop the process with a message, for a run started without its corpus. Not a panic: a missing
/// corpus is a setup error, and libFuzzer would report a panic as a finding.
fn refuse(why: &str) -> ! {
    eprintln!(
        "ext4_oracle_fuzz: {why}. Build the corpus with tools/ext4_oracle/corpus/make-corpus.sh \
         and name its directory in {IMAGES_VARIABLE}, as script/fuzz does."
    );
    std::process::exit(2)
}

/// **Every template**, read once per process.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub fn templates() -> &'static [Template] {
    static TEMPLATES: OnceLock<Vec<Template>> = OnceLock::new();
    TEMPLATES.get_or_init(|| {
        let directory = std::env::var_os(IMAGES_VARIABLE)
            .unwrap_or_else(|| refuse(&format!("{IMAGES_VARIABLE} is not set")));
        let mut paths: Vec<_> = std::fs::read_dir(&directory)
            .unwrap_or_else(|e| refuse(&format!("cannot list {directory:?}: {e}")))
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "img"))
            .collect();
        paths.sort();
        if paths.is_empty() {
            refuse(&format!("{directory:?} holds no .img file"));
        }
        paths.iter().map(|p| Template::read(p)).collect()
    })
}

/// **A template, copy-on-write, as the block capability serves it**: whole blocks, reads only.
/// An input pays for the blocks it corrupted, not for the image.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub struct Image {
    template: &'static Template,
    changed: HashMap<u64, Box<[u8; BLOCK]>>,
}

impl BlockDevice for Image {
    fn read_block(
        &mut self,
        index: u64,
        out: &mut [u8; BLOCK],
    ) -> core::result::Result<(), ErrorNumber> {
        if index.saturating_mul(BLOCK as u64) >= self.template.size {
            return Err(ext4_server::INPUT_OUTPUT_ERROR);
        }
        match self
            .changed
            .get(&index)
            .or_else(|| self.template.blocks.get(&index))
        {
            Some(stored) => out.copy_from_slice(&stored[..]),
            None => out.fill(0),
        }
        Ok(())
    }

    fn size_bytes(&self) -> u64 {
        self.template.size
    }
}

/// **Draw an image from the input**: which template, then up to eight corruptions, each a few
/// bytes written at an offset inside one of its nonzero blocks.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub fn image(u: &mut Unstructured) -> Result<Image> {
    let all = templates();
    let template = &all[u.choose_index(all.len())?];
    let mut changed: HashMap<u64, Box<[u8; BLOCK]>> = HashMap::new();
    for _ in 0..u.int_in_range(0..=MAXIMUM_CORRUPTIONS)? {
        let index = template.nonzero[u.choose_index(template.nonzero.len())?];
        let offset = u.choose_index(BLOCK)?;
        let len = u
            .int_in_range(1..=MAXIMUM_CORRUPTION_BYTES)?
            .min(BLOCK - offset);
        let bytes = u.bytes(len)?;
        let whole = changed
            .entry(index)
            .or_insert_with(|| template.blocks[&index].clone());
        whole[offset..offset + bytes.len()].copy_from_slice(bytes);
    }
    Ok(Image { template, changed })
}

/// The server every target drives: `ext4_server` over the oracle's `rsext4` adapter.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub type Served = Server<Ext4Volume<Image>>;

/// **Mount an image and serve it**, or `None` for a mount `rsext4` refuses, which is an answer
/// and not a finding.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub fn serve(image: Image) -> Option<Served> {
    volume::mount(image).ok().map(Server::new)
}

/// A request word from fields as drawn: each field masked to its width, so any value is a word
/// the kernel could deliver.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub fn request(operation: u64, handle: u64, len: u64) -> u64 {
    file_system::request(
        operation & 0xff,
        handle & file_system::MAXIMUM_HANDLE,
        len & file_system::MAXIMUM_LENGTH_BYTES,
    )
}

/// **A reply's first word as a result**, checking the rule every reply keeps: a negative word is
/// an error word whose code is in `1..=MAXIMUM_ERRNO`.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub fn result(r0: i64) -> core::result::Result<u64, ErrorNumber> {
    if r0 >= 0 {
        return Ok(r0 as u64);
    }
    match error::code(r0) {
        Some(code) => Err(code),
        None => panic!("a reply word {r0} that is neither a value nor an error word"),
    }
}

/// **Whether every component of `path` is one name**: non-empty, not `.` or `..`, and free of NUL.
/// Spelled out here rather than taken from `subtree_scope`, so the targets hold the server to the
/// rule and not to its own reading of it.
///
/// Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation.
pub fn is_relative_path_of_names(path: &[u8]) -> bool {
    path.split(|&b| b == b'/')
        .all(|c| !c.is_empty() && c != b"." && c != b".." && !c.contains(&0))
}
