//! **The crate files the farm copies into `std` verbatim**, and where each one's source lives.
//!
//! Each file is named by its package and its path inside that package, never by a path in this
//! tree. The package's directory is the one `cargo metadata` gives for it (the parent of its
//! `manifest_path`), so a crate still under `crates/` is read from there, and a crate that has left
//! the tree and is pinned as a git or registry dependency is read from Cargo's checkout of that
//! pin. [`std_generate_modules`] copies these files and [`std_inputs_stamp`] hashes them, so both
//! read the same source.
//!
//! A child of `farm` (§282 (when a file is split, the parent becomes an index)): what was private
//! to `farm` is `pub(super)` here, which reaches the same modules.
//!
//! Name: provisional, minted 2026-11-11 (UTC) by lane/farm-sources-from-cargo-metadata, for the
//! module and the file.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::*;

/// One file the farm copies into `std`: the package that holds it, its path inside that package,
/// and its path under [`farm_std_src`].
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/farm-sources-from-cargo-metadata.
pub(super) struct CopiedFile {
    /// The package's name, as `cargo metadata` lists it.
    pub(super) package: &'static str,
    /// The file, relative to the package's directory.
    pub(super) file: &'static str,
    /// Where the copy goes, relative to [`farm_std_src`].
    pub(super) destination: &'static str,
}

/// Every file [`std_generate_modules`] copies, in the order it copies them.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/farm-sources-from-cargo-metadata.
pub(super) const COPIED_FILES: [CopiedFile; 14] = [
    CopiedFile {
        package: "abi",
        file: "src/lib.rs",
        destination: "sys/pal/nife/abi.rs",
    },
    // Each thread's cache in front of the heap (milestone 561 (a per-CPU allocator is what the
    // current-CPU page was for)), a sibling of it, so the two stay one grid.
    CopiedFile {
        package: "allocation_cache",
        file: "src/lib.rs",
        destination: "sys/alloc/nife/allocation_cache.rs",
    },
    // The net_stack socket-contract wire format, so the net PAL cannot drift from the server it
    // talks to.
    CopiedFile {
        package: "socket_contract",
        file: "src/lib.rs",
        destination: "sys/pal/nife/netproto.rs",
    },
    // The FS-service wire protocol (DECISIONS §27 (the filesystem service)), so `std::fs`'s PAL
    // (milestone 27 (Rust `std` on the native ABI), phase two) cannot drift from the server it
    // opens files through.
    CopiedFile {
        package: "filesystem_contract",
        file: "src/lib.rs",
        destination: "sys/pal/nife/fsproto.rs",
    },
    // The wall-clock contract (DECISIONS §43 (reading the clock is a page)), so the time PAL reads the clock page with the same
    // seqlock and the same layout the clock service publishes it with. A drift here would be a
    // torn read of a timestamp rather than a compile error.
    CopiedFile {
        package: "clock_contract",
        file: "src/lib.rs",
        destination: "sys/pal/nife/clockproto.rs",
    },
    // The entropy contract (DECISIONS §44), so the random PAL packs its requests and reads its
    // replies exactly the way the entropy service serves them. A drift here would be a program
    // reading the wrong bytes as a key.
    CopiedFile {
        package: "entropy_contract",
        file: "src/lib.rs",
        destination: "sys/pal/nife/entropyproto.rs",
    },
    // The inert-configuration contract (milestone 47's environment-variable fork, DECISIONS
    // §111), so `sys/env`'s seeding reads the config page with the same layout and the same
    // validated domains whoever assembles a page uses.
    CopiedFile {
        package: "environment_contract",
        file: "src/lib.rs",
        destination: "sys/pal/nife/envproto.rs",
    },
    // The argument page (milestone 205 (how a foreign program is told what to do), DECISIONS
    // §170), so `sys/args` reads the argv with the layout the shell assembles it in.
    CopiedFile {
        package: "program_argument_contract",
        file: "src/lib.rs",
        destination: "sys/pal/nife/argproto.rs",
    },
    // The byte-sink contract (milestone 50 (pipes and redirection: one sink protocol)), so `println!`'s framing and the classification of a
    // failed SEND are one definition shared with every sink and with the kernel-side tests. A
    // drift in `GONE` would be a program that keeps printing into a pipe whose reader has exited.
    CopiedFile {
        package: "byte_sink_contract",
        file: "src/lib.rs",
        destination: "sys/pal/nife/sinkproto.rs",
    },
    // The std runtime contract (milestone 595 (provisional)): the eight fixed slots and the three
    // shared pages, which `rt` re-exports. The progenitor places a std child's capabilities by
    // these numbers, so a drift here would be a program reading its heap budget out of the slot
    // its stdout went in.
    CopiedFile {
        package: "std_runtime_contract",
        file: "src/lib.rs",
        destination: "sys/pal/nife/runtimeproto.rs",
    },
    // The name resolver's client wire (milestone 384 (in a capability system the resolver is a
    // grant)), so `lookup_host` asks the resolver with the words it serves. Only `wire.rs`: the
    // rest of that crate needs `domain_name_system`, which the PAL cannot name. Milestone 801
    // (packages over the internet).
    CopiedFile {
        package: "domain_name_resolution_contract",
        file: "src/wire.rs",
        destination: "sys/pal/nife/resolveproto.rs",
    },
    // The timebase page (milestone 184 (extend the `std` port to x86_64)), so `rt::cntfrq` reads
    // the machine's rate at the address and with the magic the kernel writes it with. Generated
    // for every farm and compiled only on x86_64 and riscv64 (`sys/pal/nife/mod.rs` gates the
    // module), because the farm is one source tree for all three targets and aarch64 reads a
    // register instead.
    CopiedFile {
        package: "counter_frequency_contract",
        file: "src/lib.rs",
        destination: "sys/pal/nife/counterfreqproto.rs",
    },
    // The current-CPU page (milestone 812 (`std::thread::spawn` runs real threads in one address
    // space)): a thread's allowance, which is `available_parallelism`'s answer, at the layout the
    // kernel builds it with. It names one constant of the address-space map, so the map comes too,
    // and [`std_generate_modules`] points the name at it.
    CopiedFile {
        package: "address_space_map",
        file: "src/lib.rs",
        destination: "sys/pal/nife/addressmap.rs",
    },
    CopiedFile {
        package: "current_cpu_contract",
        file: "src/lib.rs",
        destination: "sys/pal/nife/currentcpuproto.rs",
    },
];

/// Each copied package's directory, from one `cargo metadata` run over this workspace, or why it
/// could not be found. Asked once per process: the stamp and the copy both read it.
pub(super) fn copied_package_directories() -> &'static Result<BTreeMap<String, PathBuf>, String> {
    static DIRECTORIES: OnceLock<Result<BTreeMap<String, PathBuf>, String>> = OnceLock::new();
    DIRECTORIES.get_or_init(|| {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let manifest = workspace_root().join("Cargo.toml");
        let out = Command::new(&cargo)
            .args(["metadata", "--format-version", "1", "--manifest-path"])
            .arg(&manifest)
            .output()
            .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let text = String::from_utf8(out.stdout)
            .map_err(|_| "cargo metadata printed something other than UTF-8".to_string())?;
        let names: Vec<&str> = COPIED_FILES.iter().map(|c| c.package).collect();
        package_directories(&text, &names)
    })
}

/// Each package of `names`, mapped to its directory (the parent of its `manifest_path`) in
/// `metadata`, the JSON `cargo metadata --format-version 1` prints.
///
/// A name with no package fails, and so does a name that two packages with different manifests
/// share (two versions of one crate in the graph), since the farm would not know which to copy.
pub(super) fn package_directories(
    metadata: &str,
    names: &[&str],
) -> Result<BTreeMap<String, PathBuf>, String> {
    let document = Json::parse(metadata)?;
    let Some(Json::Array(packages)) = document.field("packages") else {
        return Err("cargo metadata has no `packages` list".to_string());
    };
    let mut found: BTreeMap<String, PathBuf> = BTreeMap::new();
    for package in packages {
        let (Some(Json::String(name)), Some(Json::String(manifest))) =
            (package.field("name"), package.field("manifest_path"))
        else {
            continue;
        };
        if !names.contains(&name.as_str()) {
            continue;
        }
        let directory = Path::new(manifest)
            .parent()
            .ok_or_else(|| format!("{name}: manifest path {manifest} has no directory"))?
            .to_path_buf();
        if let Some(earlier) = found.get(name)
            && *earlier != directory
        {
            return Err(format!(
                "two packages are named {name} ({} and {}); the farm copies exactly one",
                earlier.display(),
                directory.display()
            ));
        }
        found.insert(name.clone(), directory);
    }
    if let Some(missing) = names.iter().find(|n| !found.contains_key(**n)) {
        return Err(format!(
            "no package named {missing} in this workspace's dependency graph"
        ));
    }
    Ok(found)
}

/// The name a stamped file is hashed under, so the stamp does not depend on where a checkout lives:
/// its path relative to the workspace root when it is inside the tree, as every file is today, or
/// `<package>/<path inside the package>` when it is in a package directory outside it.
pub(super) fn stamp_name(file: &Path, root: &Path, packages: &BTreeMap<String, PathBuf>) -> String {
    if let Ok(relative) = file.strip_prefix(root) {
        return relative.to_string_lossy().into_owned();
    }
    for (name, directory) in packages {
        if let Ok(relative) = file.strip_prefix(directory) {
            return format!("{name}/{}", relative.to_string_lossy());
        }
    }
    file.to_string_lossy().into_owned()
}

/// A JSON value, as much of one as reading `cargo metadata` needs: every value parses, and numbers
/// are kept as their text, since nothing here reads one.
#[derive(Debug, PartialEq)]
pub(super) enum Json {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Parse one JSON document, refusing anything after it but whitespace.
    pub(super) fn parse(text: &str) -> Result<Json, String> {
        let mut reader = JsonReader {
            bytes: text.as_bytes(),
            at: 0,
        };
        let value = reader.value()?;
        reader.whitespace();
        if reader.at != reader.bytes.len() {
            return Err(format!("JSON: trailing text at byte {}", reader.at));
        }
        Ok(value)
    }

    /// The value of `key` in an object, or `None` for a missing key or a value that is not one.
    pub(super) fn field(&self, key: &str) -> Option<&Json> {
        let Json::Object(fields) = self else {
            return None;
        };
        fields.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

/// The cursor [`Json::parse`] reads with.
struct JsonReader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl JsonReader<'_> {
    fn whitespace(&mut self) {
        while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn fail<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("JSON: {what} at byte {}", self.at))
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            self.fail("unknown word")
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.whitespace();
        match self.bytes.get(self.at) {
            None => self.fail("unexpected end"),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'"') => self.string().map(Json::String),
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                self.whitespace();
                if self.bytes.get(self.at) == Some(&b']') {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value()?);
                    self.whitespace();
                    match self.bytes.get(self.at) {
                        Some(b',') => self.at += 1,
                        Some(b']') => {
                            self.at += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return self.fail("expected , or ]"),
                    }
                }
            }
            Some(b'{') => {
                self.at += 1;
                let mut fields = Vec::new();
                self.whitespace();
                if self.bytes.get(self.at) == Some(&b'}') {
                    self.at += 1;
                    return Ok(Json::Object(fields));
                }
                loop {
                    self.whitespace();
                    if self.bytes.get(self.at) != Some(&b'"') {
                        return self.fail("expected a key");
                    }
                    let key = self.string()?;
                    self.whitespace();
                    if self.bytes.get(self.at) != Some(&b':') {
                        return self.fail("expected :");
                    }
                    self.at += 1;
                    fields.push((key, self.value()?));
                    self.whitespace();
                    match self.bytes.get(self.at) {
                        Some(b',') => self.at += 1,
                        Some(b'}') => {
                            self.at += 1;
                            return Ok(Json::Object(fields));
                        }
                        _ => return self.fail("expected , or }"),
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => {
                let start = self.at;
                while matches!(
                    self.bytes.get(self.at),
                    Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
                ) {
                    self.at += 1;
                }
                let text = std::str::from_utf8(&self.bytes[start..self.at])
                    .map_err(|_| "JSON: a number that is not text".to_string())?;
                Ok(Json::Number(text.to_string()))
            }
            Some(_) => self.fail("unexpected character"),
        }
    }

    /// A string, with the cursor on its opening quote.
    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(&byte) = self.bytes.get(self.at) else {
                return self.fail("unterminated string");
            };
            self.at += 1;
            match byte {
                b'"' => {
                    return String::from_utf8(out)
                        .map_err(|_| "JSON: a string that is not UTF-8".to_string());
                }
                b'\\' => {
                    let Some(&escape) = self.bytes.get(self.at) else {
                        return self.fail("unterminated escape");
                    };
                    self.at += 1;
                    let c = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode_escape()?,
                        _ => return self.fail("unknown escape"),
                    };
                    let mut buffer = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
                }
                _ => out.push(byte),
            }
        }
    }

    /// The character of a `\u` escape, with the cursor after the `u`; a surrogate pair is read as
    /// the one character it encodes.
    fn unicode_escape(&mut self) -> Result<char, String> {
        let high = self.four_hex()?;
        let code = if (0xD800..0xDC00).contains(&high) {
            if !self.bytes[self.at..].starts_with(b"\\u") {
                return self.fail("a lone surrogate");
            }
            self.at += 2;
            let low = self.four_hex()?;
            if !(0xDC00..0xE000).contains(&low) {
                return self.fail("a lone surrogate");
            }
            0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)
        } else {
            high
        };
        char::from_u32(code).map_or_else(|| self.fail("an invalid character"), Ok)
    }

    fn four_hex(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .and_then(|d| std::str::from_utf8(d).ok())
            .and_then(|d| u32::from_str_radix(d, 16).ok());
        match digits {
            Some(value) => {
                self.at += 4;
                Ok(value)
            }
            None => self.fail("a bad \\u escape"),
        }
    }
}
