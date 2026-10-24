//! The documentation store (milestone 40) and the tree-wide search built on it.
//!
//! `manual` builds the index the guest reads and prints what it costs; `apropos` points the
//! same index and the same reader at this repository instead of at what the image installs.

use crate::host::workspace_root;

// ---- the documentation store (milestone 40) -------------------------------------------------

/// **What each package's documentation is**: a bundle is a package's pages plus the index shard
/// over them, installed as a unit at `doc/<bundle>/` in the filesystem image, with `doc/bundles`
/// listing the names (milestone 40's "installed by the package that owns it", minus a package
/// manager).
///
/// A bundle names paths that already exist, because **a note copied into a crate directory would be
/// a second copy that can drift**. The store is a build artifact, and a page that has moved fails
/// the build rather than shipping stale.
const DOC_BUNDLES: &[(&str, &[&str])] = &[
    ("manual", &["notes/documentation.md"]),
    ("swish", &["notes/pipes.md", "notes/line-discipline.md"]),
    // `notes/capabilities.md` answers what this system's central word means, and three stranger
    // runs (milestone 117) found it unreachable by following the tree. It is the kernel's own
    // document, so it is in the kernel's bundle; `script/apropos` is the other half, for a reader
    // with a checkout.
    (
        "kernel",
        &[
            "notes/ipc-naming.md",
            "notes/stack.md",
            "notes/capabilities.md",
        ],
    ),
    ("glob", &["notes/glob.md"]),
];

/// Where the store is staged on the host before it is imported into the filesystem image.
///
/// The last component is [`documentation::index::STORE_DIR`] rather than the literal `doc`, because the
/// guest opens that name and this writes it: it is a thing two programs agree on, so it is a
/// constant in the crate they share.
fn doc_store_path() -> std::path::PathBuf {
    workspace_root()
        .join("target/redoxfs-tree")
        .join(documentation::index::STORE_DIR)
}

/// What one bundle cost, so the numbers in notes/documentation.md are measured rather than estimated.
pub(crate) struct Shard {
    bundle: &'static str,
    pages: usize,
    terms: usize,
    postings: usize,
    /// Bytes of markdown.
    source: usize,
    /// Bytes of index.
    index: usize,
}

/// Build the store into `target/redoxfs-tree/doc`, ready for `mkredoxfs`'s `import`.
///
/// Returns one [`Shard`] per bundle. `None` means a listed page is missing, which is a build
/// failure rather than a warning: a store that quietly ships without a page is a manual with a
/// missing chapter and nothing to say so.
pub(crate) fn doc_store() -> Option<Vec<Shard>> {
    let root = doc_store_path();
    let _ = std::fs::remove_dir_all(&root);
    if std::fs::create_dir_all(&root).is_err() {
        eprintln!("doc-store: cannot create {}", root.display());
        return None;
    }

    let mut shards = Vec::new();
    let mut names = String::new();
    for (bundle, pages) in DOC_BUNDLES {
        let dir = root.join(bundle);
        if std::fs::create_dir_all(&dir).is_err() {
            eprintln!("doc-store: cannot create {}", dir.display());
            return None;
        }
        // The page's name in the store is its basename, because the store is where a reader types
        // it: `cd doc/glob` then `doc glob.md`. Its *path* in the index keeps the whole repository
        // path, so a search result says where the page came from.
        let mut loaded: Vec<(String, String, Vec<u8>)> = Vec::new();
        for page in *pages {
            let src = workspace_root().join(page);
            let Ok(bytes) = std::fs::read(&src) else {
                eprintln!("doc-store: {bundle} lists {page}, which does not exist");
                return None;
            };
            let base = std::path::Path::new(page)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(page);
            if std::fs::write(dir.join(base), &bytes).is_err() {
                eprintln!("doc-store: cannot write {base}");
                return None;
            }
            let title = documentation::index::title_of(&bytes)
                .unwrap_or(base)
                .to_string();
            loaded.push(((*page).to_string(), title, bytes));
        }

        let sources: Vec<documentation::index::Source<'_>> = loaded
            .iter()
            .map(|(path, title, bytes)| documentation::index::Source {
                path,
                title,
                text: bytes,
            })
            .collect();
        let index = documentation::index::build(&sources);
        let header =
            documentation::index::Header::parse(&index[..documentation::index::PAGE]).ok()?;
        if std::fs::write(dir.join(documentation::index::SHARD), &index).is_err() {
            eprintln!(
                "doc-store: cannot write {bundle}/{}",
                documentation::index::SHARD
            );
            return None;
        }
        shards.push(Shard {
            bundle,
            pages: header.pages as usize,
            terms: header.terms as usize,
            postings: header.postings as usize,
            source: loaded.iter().map(|(_, _, b)| b.len()).sum(),
            index: index.len(),
        });
        names.push_str(bundle);
        names.push('\n');
    }

    // The manifest a reader (and, when it exists, a guest-side `apropos`) uses to find the shards.
    // A file rather than a directory listing, because **there is no directory iteration in this
    // system** and adding one would be adding authority: a program that can list a directory can
    // discover what it was not given. See notes/documentation.md.
    if std::fs::write(root.join(documentation::index::MANIFEST), names).is_err() {
        eprintln!("doc-store: cannot write the bundle manifest");
        return None;
    }
    Some(shards)
}

/// `cargo xtask manual`: build the store and print what it costs, then answer a query against it.
///
/// The query at the end is not a demo. It is the only thing that proves the reader and the writer
/// agree, and it runs the **same** `no_std` lookup the guest runs, over the same bytes, through the
/// same one-page-at-a-time [`documentation::index::Pages`] interface. Only the IO differs.
pub(crate) fn manual_store(term: Option<String>) -> bool {
    let Some(shards) = doc_store() else {
        return false;
    };
    println!("documentation store: {}", doc_store_path().display());
    println!();
    println!(
        "  {:<10} {:>5} {:>7} {:>8} {:>9} {:>8} {:>6}",
        "bundle", "pages", "terms", "postings", "markdown", "index", "probes"
    );
    let (mut src, mut idx) = (0usize, 0usize);
    for s in &shards {
        // A lookup is a binary search over index PAGES, so its cost is the log of how many pages the
        // term table occupies, plus the one read that finishes inside a page. This is the number the
        // layout exists to keep small, so it is the number the build prints.
        let per = documentation::index::PAGE / documentation::index::TERM_REC;
        let term_pages = s.terms.div_ceil(per).max(1) as u64;
        let probes = 64 - (term_pages - 1).leading_zeros().min(63) + 1;
        println!(
            "  {:<10} {:>5} {:>7} {:>8} {:>9} {:>8} {:>6}",
            s.bundle, s.pages, s.terms, s.postings, s.source, s.index, probes
        );
        src += s.source;
        idx += s.index;
    }
    println!();
    println!("  {src} bytes of markdown, {idx} bytes of index");

    let Some(term) = term else {
        return true;
    };
    println!();
    println!("search: {term}");

    // **The bundles come from the manifest the build just wrote**, not from `DOC_BUNDLES`, because
    // that is what the guest reads and the whole value of this query is that it takes the guest's
    // path. A store whose manifest disagreed with the table would answer differently at the prompt
    // than it does here, and this is where that would show.
    let Ok(manifest) = std::fs::read(doc_store_path().join(documentation::index::MANIFEST)) else {
        eprintln!("doc-store: the bundle manifest is not there");
        return false;
    };
    let mut ranked = documentation::index::Ranked::new();
    let mut bad = Vec::new();
    documentation::index::bundles(&manifest, |bundle| {
        let name = String::from_utf8_lossy(bundle).to_string();
        let Ok(bytes) = std::fs::read(
            doc_store_path()
                .join(&name)
                .join(documentation::index::SHARD),
        ) else {
            bad.push(format!("{name}: no shard"));
            return;
        };
        if let Err(e) = documentation::index::search(
            bundle,
            term.as_bytes(),
            &mut documentation::index::Slice(&bytes),
            &mut ranked,
        ) {
            bad.push(format!("{name}: {e:?}"));
        }
    });
    for b in &bad {
        println!("  {b}");
    }

    for f in ranked.results() {
        println!(
            "  {:>4}  {:<28}  {:<46}  {}",
            f.count,
            String::from_utf8_lossy(f.location()),
            String::from_utf8_lossy(f.title()),
            String::from_utf8_lossy(f.origin())
        );
    }
    if ranked.offered() == 0 {
        println!("  nothing in the store says that");
    } else if ranked.offered() > ranked.results().len() {
        println!(
            "  {} of {} pages, strongest first",
            ranked.results().len(),
            ranked.offered()
        );
    }
    bad.is_empty()
}

// ---- the tree-wide search (milestone 40, script/apropos) -----------------------------------

/// **The same index, the same reader, pointed at this repository instead of at the image.**
/// Searches for `term` and returns false on a usage error or when a printed result's path was too
/// long to hold.
///
/// Milestone 117's stranger runs found pages that nothing a person would type led to. Milestone
/// 40's inverted index, shard-merging reader and ranking were pointed only at the pages the image
/// installs; this points the same `documentation::index::build` and `search` at the whole checkout,
/// deliberately not a second implementation, so a layout defect shows in both places. A result
/// names a **path in this repository**, because that is what a person with a checkout opens. See
/// notes/documentation/repository-search.md.
pub(crate) fn tree_apropos(term: Option<String>) -> bool {
    let Some(term) = term else {
        eprintln!("usage: script/apropos <word>");
        eprintln!("       searches every markdown page in this repository, and every crate's and");
        eprintln!("       program's own module documentation, and says where each one lives.");
        return false;
    };

    let sections = tree_sections();
    if sections.is_empty() {
        eprintln!("apropos: found no documentation to index, which means this is not a checkout");
        return false;
    }

    let mut ranked = documentation::index::Ranked::new();
    let mut pages = 0usize;
    let mut bytes = 0usize;
    let mut long = Vec::new();
    for shelf in &sections {
        pages += shelf.docs.len();
        bytes += shelf.docs.iter().map(|d| d.text.len()).sum::<usize>();
        // A path the record cannot hold is reported rather than silently shortened, because the
        // path is the whole answer here: a result a reader cannot open is worse than no result.
        for d in &shelf.docs {
            if d.path.len() > documentation::index::PATH_MAX {
                long.push(d.path.clone());
            }
        }
        let sources: Vec<documentation::index::Source<'_>> = shelf
            .docs
            .iter()
            .map(|d| documentation::index::Source {
                path: &d.path,
                title: &d.title,
                text: &d.text,
            })
            .collect();
        let shard = documentation::index::build(&sources);
        if let Err(e) = documentation::index::search(
            shelf.name.as_bytes(),
            term.as_bytes(),
            &mut documentation::index::Slice(&shard),
            &mut ranked,
        ) {
            eprintln!("apropos: {}: {e:?}", shelf.name);
            return false;
        }
    }

    println!("{pages} pages, {bytes} bytes of documentation in this repository");
    println!();
    println!("searching for: {term}");
    println!();
    for f in ranked.results() {
        // **The origin, not the location.** A guest result names `doc/<bundle>/<page>`, because
        // that is what a shell there can designate; the reader of this command holds a checkout, so
        // the openable name is the path the page came from.
        println!(
            "  {:>5}  {:<48}  {}",
            f.count,
            String::from_utf8_lossy(f.origin()),
            String::from_utf8_lossy(f.title())
        );
    }
    if ranked.offered() == 0 {
        println!("  nothing in this repository says that");
    } else if ranked.offered() > ranked.results().len() {
        println!();
        println!(
            "  {} of {} pages, strongest first",
            ranked.results().len(),
            ranked.offered()
        );
    }
    // **Only a path this search printed can fail it.** A path a reader is shown but cannot open is
    // worse than no result, so it exits 1 and is named; an over-long path that never reached the
    // output is a fact about the corpus and gets one line, so the tool does not fail on its own
    // README example.
    let max = documentation::index::PATH_MAX;
    let shown: Vec<&String> = long
        .iter()
        .filter(|p| {
            ranked
                .results()
                .iter()
                .any(|f| p.as_bytes().starts_with(f.origin()) && f.origin().len() == max)
        })
        .collect();
    for p in &shown {
        eprintln!(
            "apropos: {p} is longer than the {max} bytes a page record holds, so the result above \
             that names it is truncated"
        );
    }
    if long.len() > shown.len() {
        eprintln!(
            "apropos: {} other paths are longer than the {max} bytes a page record holds; none \
             is in these results",
            long.len() - shown.len()
        );
    }
    shown.is_empty()
}

/// One document offered to the tree index: where it lives, what it is called, and its text.
struct Doc {
    /// Path relative to the workspace root, which is the openable name a result prints.
    path: String,
    title: String,
    text: Vec<u8>,
}

/// One shard of the tree index, named for the part of the tree it covers.
struct Shelf {
    name: &'static str,
    docs: Vec<Doc>,
}

/// The corpus, in shards, because the merge across shards is what the reader does.
///
/// The shard names are the reader's map of the tree and are the only new vocabulary here; they are
/// the directories a person already sees.
fn tree_sections() -> Vec<Shelf> {
    let root = workspace_root();
    let mut out = Vec::new();

    // The markdown, in the four places this project keeps it. The repository root is included
    // because `README.md` is where a stranger starts, and a search that could not return the front
    // page would be odd about it.
    for (shard, dir, recurse) in [
        ("notes", "notes", false),
        ("decisions", "design/decisions", false),
        ("roadmap", "design/roadmap", false),
        ("design", "design", false),
        ("guides", "", false),
    ] {
        let mut docs = Vec::new();
        collect_markdown(&root.join(dir), &root, &mut docs, recurse);
        if !docs.is_empty() {
            out.push(Shelf { name: shard, docs });
        }
    }

    // **And the module documentation, which is the finding this exists for.** A stranger could not
    // find `crates/abi/src/lib.rs`, and no markdown page is going to fix that, because the document
    // it wants *is* that file's header. A `//!` block is markdown already, so it indexes as a page
    // with no conversion and no copy: the result names the source file, which is the thing to open.
    for (shard, dir, file) in [
        ("crates", "crates", "src/lib.rs"),
        ("components", "components/src", ""),
        ("fixtures", "fixtures/src", ""),
    ] {
        let mut docs = Vec::new();
        collect_module_docs(&root.join(dir), &root, file, &mut docs);
        if !docs.is_empty() {
            out.push(Shelf { name: shard, docs });
        }
    }

    out
}

/// Every `.md` file directly in `dir`, as `(path relative to the root, title, bytes)`.
fn collect_markdown(
    dir: &std::path::Path,
    root: &std::path::Path,
    out: &mut Vec<Doc>,
    recurse: bool,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut found: Vec<std::path::PathBuf> = entries.flatten().map(|e| e.path()).collect();
    // Sorted, so two runs on one checkout offer the same pages in the same order and a tie between
    // two pages breaks the same way twice. `Ranked` keeps ties in offer order by design.
    found.sort();
    for path in found {
        if path.is_dir() {
            if recurse {
                collect_markdown(&path, root, out, recurse);
            }
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel = rel.display().to_string();
        let title = documentation::index::title_of(&bytes)
            .unwrap_or(&rel)
            .trim()
            .to_string();
        out.push(Doc {
            path: rel,
            title,
            text: bytes,
        });
    }
}

/// The `//!` header of every Rust file under `dir`, as a page.
///
/// `file` is the path inside each subdirectory to read (`src/lib.rs` for a crate), or empty to read
/// the `.rs` files in `dir` itself (the programs).
fn collect_module_docs(
    dir: &std::path::Path,
    root: &std::path::Path,
    file: &str,
    out: &mut Vec<Doc>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut found: Vec<std::path::PathBuf> = entries.flatten().map(|e| e.path()).collect();
    found.sort();
    for entry in found {
        // A program is a `.rs` file or, since milestone 844 (the shell program is split along its
        // seams), a directory whose crate root is `main.rs`. The directory's name is the program's.
        let program_dir = file.is_empty() && entry.join("main.rs").is_file();
        let path = if program_dir {
            entry.join("main.rs")
        } else if file.is_empty() {
            if entry.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            entry
        } else {
            entry.join(file)
        };
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let doc = module_doc(&src);
        // A file whose header is a line or two says nothing worth ranking, and indexing it would
        // put noise in front of the pages that do. Three hundred bytes is about a paragraph.
        if doc.len() < 300 {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel = rel.display().to_string();
        // **Named for what it is, not for its first heading.** A crate header rarely opens with a
        // level-one heading, and falling back to the path would print the path twice. `crate abi`
        // is what a reader calls the thing.
        let what = if file.is_empty() { "program" } else { "crate" };
        let name = if program_dir {
            path.parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("?")
        } else if file.is_empty() {
            path.file_stem().and_then(|s| s.to_str()).unwrap_or("?")
        } else {
            path.parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("?")
        };
        out.push(Doc {
            path: rel,
            title: format!("{what} {name}"),
            text: doc.into_bytes(),
        });
    }
}

/// The leading `//!` block of a Rust file, with the markers stripped, which is markdown.
fn module_doc(src: &str) -> String {
    let mut out = String::new();
    for line in src.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("//!") {
            out.push_str(rest.strip_prefix(' ').unwrap_or(rest));
            out.push('\n');
        } else if t.is_empty() && out.is_empty() {
            // Leading blank lines before the header, which nothing in this tree writes but which
            // cost nothing to tolerate.
            continue;
        } else if !out.is_empty() {
            // The header ends at the first line that is not part of it. Attributes and `use` lines
            // below are code, and a searcher that indexed them would rank a crate by its imports.
            break;
        }
    }
    out
}
