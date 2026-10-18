//! **`jig`, the package manager** (milestone 809 (the package client becomes a program)).
//!
//! ```text
//! jig update                   fetch the image's index over HTTPS and keep a copy of it
//! jig list                     the packages the index copy offers, or else the image's catalog
//! jig install <path>           install a package file already on this system
//! jig install <name>[@<ver>]   fetch a package by name and install it
//! jig remove <package>         a new generation without the package's programs
//! jig rollback                 the generation below the live one becomes live
//! ```
//!
//! **`update` and the index copy** (milestone 801 (packages over the internet), item 6 of
//! milestone 809): the image names its index's addresses and the root their TLS must chain to
//! (`package_index::Sources`, at `std_runtime_contract::INDEX_SOURCES_PAGE`). `update` fetches
//! the index from the first that answers, pinned to that root, keeps this architecture's lines,
//! and writes them to `package_index::IMAGE_INDEX_COPY` in the index store, the directory the
//! owner's console grants at `std_runtime_contract::FS_DIR_SLOT`. After that a name is looked up
//! in the copy first, fetched from the entry's locations or the repository's own `targets/` over
//! HTTPS, and judged by the entry's digest; the progenitor installs it on the same copy's word
//! (ruling I2). Before any `update`, a name is looked up in the image's catalog and fetched from
//! the LAN package source, as since milestone 198 (a package manager)'s rung 3a.
//!
//! The verbs are apt's where apt has one (calef, 2026-10-10 (UTC): "Lets use apt's verbs.";
//! design/naming/command-line-rulings.md). A word with a `/` in it is a file, by the prompt's rule
//! for a command word (DECISIONS §219 (how the shell names an installed program to the spawner)).
//!
//! The progenitor stays the installer (DECISIONS §208 (installing a package is granting it, and the
//! installed programs are versioned)) and checks every package against its own copy of the catalog.
//! This is the client. It asks on an installer endpoint that serves install, remove and rollback
//! and nothing else (DECISIONS §270 (a package manager holds an installer endpoint, not the spawn
//! endpoint)), with `spawnproto`'s activation request unchanged. It fetches a package itself, so
//! the progenitor parses no network input, and checks it before sending: it must be the package
//! asked for, and the catalog must vouch for its digest (`package_archive::installable_as`).
//!
//! The package is read into one run of this program's pages and handed over a page at a time with
//! `abi::page_frame::SLICE`, so the buffer and the frames are the same memory.
//!
//! notes/packages/jig.md has what each verb holds, how an install travels, and the BUGS.
//!
//! Name: ratified 2026-10-06 (calef). See `grant_plan::Prog::Jig`.

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream, ToSocketAddrs};
use std::process::ExitCode;

use entropy_backend as _;
use grant_plan::spawnproto::{self, Activation, ActivationStatus as S};
use grant_plan::{CATALOG_SLOT, INDEX_SOURCES_SLOT, INSTALLER_REPLY_SLOT, INSTALLER_SLOT};
use package_index::{
    Configured, Entry, IMAGE_INDEX_COPY, Index, Miss, PATH_MAX, Root, STAND_IN_INDEX_FILE, Source,
    Sources,
};
use pinned_tls_client::listed::{self, root_der};
use pinned_tls_client::{PinnedPeer, Session};
use socket_contract::fixture::{PACKAGE_PEER_HOST, PACKAGE_PEER_IP, PACKAGE_PEER_PORT};
use user_mode_runtime as rt;

// **What these bytes ask for when they are run by path, as installed bytes** (milestone 597 (a
// program carries its manifest in an ELF note)): `jig`'s row without the three grants no image may
// hold. The image's own `jig` is endowed from `grant_plan`'s table and never reads this; a copy run
// from a file is, and it finds no installer endpoint and says so, which is how `script/swish-check`
// shows a program without the grant is refused. The note cannot carry `installer` or `catalog`
// (`manifest_note::encode` refuses both at compile time), and a `std` image cannot carry the network.
manifest_note::carry!(grant_plan::Manifest {
    network: false,
    installer: false,
    catalog: false,
    indexes: grant_plan::IndexStoreDeclaration::Forbidden,
    ..grant_plan::Prog::Jig.manifest()
});

/// **Where the staging run is mapped**: one package, read here and handed on a page at a time. A
/// window of this program's own, so a pair page (`address_space_map`'s rule 7), clear of everything
/// a loader maps into a `std` program. Four MiB, `spawnproto::IMAGE_MAX_PAGES` pages.
const STAGING_VA: u64 = address_space_map::pair_page(0x0400_0000);

/// This machine's architecture, as a package stem spells it.
const ARCHITECTURE: &str = std::env::consts::ARCH;

const USAGE: &str =
    "  jig update | jig list | jig install <file or name> | jig remove <package> | jig rollback";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["update"] => update(),
        ["list"] => list(),
        ["install", path] if path.contains('/') => install_file(path),
        ["install", name] => install_name(name),
        ["remove", package] => remove(package),
        ["rollback"] => rollback(),
        _ => {
            println!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// **The image's catalog**, as this program was handed it: the text before the page's first zero
/// byte. `None` for a `jig` that was not granted one, which is told so rather than faulting on an
/// unmapped page.
fn catalog() -> Option<&'static str> {
    if !rt::is_granted(CATALOG_SLOT) {
        return None;
    }
    // SAFETY: the progenitor maps the catalog's page read-only at `CATALOG_PAGE` whenever it
    // places the page's capability at `CATALOG_SLOT` (`system_initializer`'s `StdLayout`), and the
    // mapping lives as long as this process.
    let page = unsafe {
        std::slice::from_raw_parts(
            std_runtime_contract::CATALOG_PAGE as *const u8,
            spawnproto::IMAGE_PAGE as usize,
        )
    };
    let end = page.iter().position(|&b| b == 0).unwrap_or(page.len());
    std::str::from_utf8(&page[..end]).ok()
}

/// **`jig list`**: every package this image's catalog vouches for on this architecture, one per
/// line, as `<name> <version>`.
fn list() -> ExitCode {
    if let Some(copy) = index_copy() {
        let Ok(index) = Index::parse(&copy) else {
            return refuse("the index copy cannot be read; run jig update");
        };
        println!("  packages the index offers (the copy jig update wrote):");
        for entry in index.entries() {
            if let Some((name, version)) = name_and_version(entry.stem) {
                println!("  {name} {version}");
            }
        }
        return ExitCode::SUCCESS;
    }
    let Some(catalog) = catalog() else {
        println!("  this jig holds no catalog; only the image's own jig is handed one");
        return ExitCode::FAILURE;
    };
    println!("  packages this image vouches for (its catalog; no index yet):");
    for line in catalog.lines() {
        let Some(stem) = line.split(' ').next() else {
            continue;
        };
        if let Some((name, version)) = name_and_version(stem) {
            println!("  {name} {version}");
        }
    }
    ExitCode::SUCCESS
}

/// A stem's name and version, when it is this architecture's.
fn name_and_version(stem: &str) -> Option<(&str, &str)> {
    stem.strip_suffix(ARCHITECTURE)?
        .strip_suffix('-')?
        .rsplit_once('-')
}

/// **A package's bytes, in a run of this program's own pages**, ready to hand on a page at a time.
struct Staged {
    /// The run's capability, with `GRANT`, which `SLICE` needs.
    run: u64,
    len: u64,
    /// What the run was sized for, in bytes; `len` is at most this.
    capacity: u64,
}

impl Staged {
    /// Retype a run for `len` bytes from this program's region and map it at [`STAGING_VA`].
    fn new(len: u64) -> Result<Staged, &'static str> {
        let pages = spawnproto::image_pages(len);
        if pages == 0 || pages > spawnproto::IMAGE_MAX_PAGES {
            return Err("that package is empty, or larger than a package may be (4 MiB)");
        }
        let run = rt::retype_page_frame_run(std_runtime_contract::MEMORY_REGION_SLOT, pages);
        let Ok(run) = u64::try_from(run) else {
            return Err("this jig could not set aside the pages for that package");
        };
        if !rt::map_page_frame(
            run,
            STAGING_VA,
            true,
            std_runtime_contract::MEMORY_REGION_SLOT,
        ) {
            rt::cap_delete(run);
            return Err("this jig could not map the pages for that package");
        }
        Ok(Staged {
            run,
            len,
            capacity: len,
        })
    }

    fn bytes(&mut self) -> &mut [u8] {
        // SAFETY: `new` mapped `image_pages(capacity)` pages read/write at `STAGING_VA`, which
        // nothing else in this process uses, and `len` is at most `capacity`.
        unsafe { std::slice::from_raw_parts_mut(STAGING_VA as *mut u8, self.len as usize) }
    }
}

/// **The progenitor's answer to one activation request**: the status, the generation live
/// afterwards, the member a refused install's program is (one more than its table-of-contents
/// index, 0 for none), and the package already holding that program's name, for `NameTaken`.
struct Reply {
    status: S,
    live: u64,
    member: u64,
    holder: String,
}

/// **Send one activation request on the installer endpoint and read its answer**: the frames
/// of `staged` for an install, the packed name for a removal, nothing for a rollback.
fn ask(verb: Activation, staged: Option<&Staged>, name: Option<&str>) -> Reply {
    let len = staged.map_or(0, |s| s.len);
    let (w0, w1, w2) = spawnproto::activation_request(verb, len);
    rt::send(INSTALLER_SLOT, w0, w1, w2);
    if let Some(st) = staged {
        for i in 0..spawnproto::image_pages(st.len) {
            match u64::try_from(rt::slice_page_frame(st.run, i, 1)) {
                Ok(page) => {
                    rt::send_capability(INSTALLER_SLOT, page, abi::rights::READ, i);
                    rt::cap_delete(page);
                }
                // A plain message where a frame was promised keeps both sides in step: the
                // progenitor counts a missing frame and refuses the package.
                Err(_) => {
                    rt::send(INSTALLER_SLOT, 0, 0, 0);
                }
            }
        }
    }
    if let Some(name) = name {
        let (lo, hi) = filesystem_contract::grant::pack_name(name.as_bytes());
        rt::send(INSTALLER_SLOT, lo, hi, name.len() as u64);
    }
    let (r0, r1, r2) = rt::receive(INSTALLER_REPLY_SLOT);
    let status = S::from_word(r0);
    let mut holder = String::new();
    // **Two more messages name the holder** (milestone 919 (a package carries several
    // programs)): calef ruled the owner removes the first package before installing the second.
    if status == S::NameTaken {
        let words = [
            rt::receive(INSTALLER_REPLY_SLOT),
            rt::receive(INSTALLER_REPLY_SLOT),
        ];
        let mut out = [0u8; spawnproto::HOLDER_MAX];
        holder = spawnproto::holder_from_words(words, &mut out).to_string();
    }
    Reply {
        status,
        live: r1,
        member: r2,
        holder,
    }
}

/// **Send `staged` to the installer and say what came of it**, naming a refused program from these
/// bytes, which are the ones the progenitor judged (milestone 919).
fn send_install(staged: &mut Staged, fetched: bool) -> ExitCode {
    let reply = ask(Activation::Install, Some(staged), None);
    let program = reply.member.checked_sub(1).and_then(|index| {
        let package = package_archive::Package::parse(staged.bytes()).ok()?;
        let index = usize::try_from(index).ok()?;
        package_archive::program_name(&package, index).map(String::from)
    });
    answer(Activation::Install, fetched, reply, program.as_deref())
}

/// Whether this process was granted an installer endpoint; says why not when it was not.
fn holds_installer() -> bool {
    let held = rt::is_granted(INSTALLER_SLOT) && rt::is_granted(INSTALLER_REPLY_SLOT);
    if !held {
        println!("  this jig holds no installer endpoint; only the owner's console grants one");
    }
    held
}

/// **`jig install <path>`**: the file's bytes, sent as they are. The progenitor's catalog decides.
fn install_file(path: &str) -> ExitCode {
    if !holds_installer() {
        return ExitCode::FAILURE;
    }
    // `./` is how a person says "a file, here" (DECISIONS §219's rule for a word with a `/`); the
    // shell designated the name after it, so that is the name opened.
    let mut name = path;
    while let Some(rest) = name.strip_prefix("./") {
        name = rest;
    }
    let mut file = match std::fs::File::open(name) {
        Ok(f) => f,
        Err(e) => return say_io(path, e),
    };
    let len = match file.metadata() {
        Ok(m) => m.len(),
        Err(e) => return say_io(path, e),
    };
    let mut staged = match Staged::new(len) {
        Ok(s) => s,
        Err(why) => return refuse(why),
    };
    if let Err(e) = file.read_exact(staged.bytes()) {
        return say_io(path, e);
    }
    send_install(&mut staged, false)
}

/// **`jig install <name>`**: the catalog names the stem, the package source serves
/// `/<stem>.nifepkg`, and what arrives is checked before anything is sent: it must be the package
/// asked for, and the catalog must vouch for its digest.
fn install_name(name: &str) -> ExitCode {
    if !holds_installer() {
        return ExitCode::FAILURE;
    }
    // **The index copy first, once `update` has written one** (milestone 801).
    if let Some(copy) = index_copy() {
        let Ok(index) = Index::parse(&copy) else {
            return refuse("the index copy cannot be read; run jig update");
        };
        match index.find(name, ARCHITECTURE) {
            Ok(entry) => return install_indexed(&entry),
            Err(Miss::SeveralVersions) => {
                return refuse(
                    "the index offers several versions of that package; \
                     name one with <package>@<version>",
                );
            }
            // Not in the index: the image's catalog may still vouch for it.
            Err(Miss::NoSuchPackage) => {}
        }
    }
    let Some(catalog) = catalog() else {
        return refuse("this jig holds no catalog to look that name up in");
    };
    let stem = match package_archive::cataloged_stem(catalog, name, ARCHITECTURE) {
        Ok(stem) => stem,
        Err(package_archive::CatalogMiss::NoSuchPackage) => {
            return refuse("this image's catalog names no such package, so nothing was fetched");
        }
        Err(package_archive::CatalogMiss::SeveralVersions) => {
            return refuse(
                "this image's catalog vouches for several versions of that package; \
                 name one with <package>@<version>",
            );
        }
    };
    let mut staged = match fetch(stem) {
        Ok(s) => s,
        Err(why) => {
            println!("  {why}; nothing was installed");
            return ExitCode::FAILURE;
        }
    };
    if package_archive::installable_as(catalog, stem, staged.bytes()).is_err() {
        return refuse(
            "this image's catalog does not vouch for those bytes; nothing was sent to the installer",
        );
    }
    send_install(&mut staged, true)
}

/// **`GET /<stem>.nifepkg` from the package source**, into a staging run sized from the response's
/// declared length. `http_response` reads the head and refuses what it cannot read exactly; the
/// body goes straight into the run.
fn fetch(stem: &str) -> Result<Staged, &'static str> {
    const FAILED: &str = "the package source did not send a whole package";
    let mut tcp = TcpStream::connect((Ipv4Addr::from(PACKAGE_PEER_IP), PACKAGE_PEER_PORT))
        .map_err(|_| "the package source did not answer")?;
    let path = format!("/{stem}.nifepkg");
    let mut request = [0u8; 160];
    let n = http_response::get_request(PACKAGE_PEER_HOST, &path, &mut request).ok_or(FAILED)?;
    tcp.write_all(&request[..n]).map_err(|_| FAILED)?;
    let mut response = http_response::Response::new();
    let mut staged: Option<Staged> = None;
    let mut filled = 0usize;
    let mut buf = [0u8; 2048];
    while !response.is_complete() {
        let n = match tcp.read(&mut buf) {
            Ok(0) | Err(_) => return Err(FAILED),
            Ok(n) => n,
        };
        let body = response.feed(&buf[..n]).map_err(|_| FAILED)?;
        if response.status().is_some_and(|s| s != 200) {
            return Err(FAILED);
        }
        if staged.is_none()
            && let Some(len) = response.content_length()
        {
            staged = Some(Staged::new(len)?);
        }
        if !body.is_empty() {
            let st = staged.as_mut().ok_or(FAILED)?;
            // `feed` hands back no more body than the head declared, and the run was sized from
            // that declaration, so this stays inside it.
            let dest = st
                .bytes()
                .get_mut(filled..filled + body.len())
                .ok_or(FAILED)?;
            dest.copy_from_slice(body);
            filled += body.len();
        }
    }
    staged.ok_or(FAILED)
}

/// **The image's index sources**, as this program was handed them (milestone 801): the text before
/// the page's first zero byte. `None` for a `jig` not granted the index store, which comes with it.
fn sources_text() -> Option<&'static str> {
    if !rt::is_granted(INDEX_SOURCES_SLOT) {
        return None;
    }
    // SAFETY: the progenitor maps the sources page read-only at `INDEX_SOURCES_PAGE` whenever it
    // places the page's capability at `INDEX_SOURCES_SLOT` (`system_initializer`'s `StdLayout`),
    // and the mapping lives as long as this process.
    let page = unsafe {
        std::slice::from_raw_parts(
            std_runtime_contract::INDEX_SOURCES_PAGE as *const u8,
            spawnproto::IMAGE_PAGE as usize,
        )
    };
    let end = page.iter().position(|&b| b == 0).unwrap_or(page.len());
    std::str::from_utf8(&page[..end]).ok()
}

/// The index copy `update` wrote, if this line holds the store and there is one.
fn index_copy() -> Option<String> {
    sources_text()?;
    std::fs::read_to_string(IMAGE_INDEX_COPY).ok()
}

/// The image's sources, read, or why not.
fn sources() -> Result<Sources<'static>, &'static str> {
    let text = sources_text()
        .ok_or("this jig holds no index store; only the owner's console grants one")?;
    Sources::parse(text).map_err(|_| "this image's index sources cannot be read")
}

/// Where to connect for one configured repository: the address the image gave, or the host
/// resolved once, through the resolver badge the progenitor placed (milestone 892 (a std program
/// at the prompt holds the network and a name resolver)).
fn addresses(c: &Configured<'_>) -> Result<Vec<SocketAddr>, String> {
    let port = c.repository.port;
    if let Some(ip) = c.address {
        return Ok(vec![SocketAddr::V4(SocketAddrV4::new(ip, port))]);
    }
    Ok((c.repository.host, port)
        .to_socket_addrs()
        .map_err(|e| format!("{:?}", e.kind()))?
        .collect())
}

/// **One `GET` over TLS**, connected to one of `to` and speaking to `server_name`, pinned to
/// `root`, the body handed to `body` as it arrives. `server_name` is for TLS alone; nothing here
/// resolves a name, so a listed location is dialed only at the address its check passed (milestone
/// 871 (a sixth outsider pass attacks the confinement claim)).
fn get(
    server_name: &str,
    to: &[SocketAddr],
    root: &[u8],
    path: &str,
    body: impl FnMut(&[u8]),
) -> Result<(), String> {
    let peer = PinnedPeer::new(server_name, root).map_err(|e| format!("{e:?}"))?;
    let tcp = TcpStream::connect(to).map_err(|e| format!("{:?}", e.kind()))?;
    let session = Session::handshake(&peer, tcp).map_err(|e| format!("{e:?}"))?;
    match session.get(path, body) {
        Ok(200) => Ok(()),
        Ok(status) => Err(format!("status {status}")),
        Err(e) => Err(format!("{e:?}")),
    }
}

/// **`jig update`**: the index, from the first of the image's addresses that answers (Q2), pinned
/// to the image's root; this architecture's lines kept, read back whole, and written as the copy.
fn update() -> ExitCode {
    let sources = match sources() {
        Ok(s) => s,
        Err(why) => return refuse(why),
    };
    let root = match root_der(&sources.root) {
        Ok(r) => r,
        Err(why) => return refuse(why),
    };
    for configured in sources.repositories() {
        let repo = configured.repository;
        let mut buf = [0u8; PATH_MAX];
        let Some(path) = repo.metadata_path(STAND_IN_INDEX_FILE, &mut buf) else {
            continue;
        };
        let mut text = Vec::new();
        let fetched = addresses(&configured).and_then(|to| {
            get(repo.host, &to, &root, path, |part| {
                text.extend_from_slice(part)
            })
        });
        if let Err(why) = fetched {
            println!("  index unreachable at {}: {why}", repo.host);
            continue;
        }
        let Ok(text) = String::from_utf8(text) else {
            return refuse("the index is not text; the copy was not changed");
        };
        if Index::parse(&text).is_err() {
            return refuse("the index has a line this jig cannot read; the copy was not changed");
        }
        // This architecture's lines, so the copy fits the page the progenitor reads it into.
        let ours: String = text
            .split('\n')
            .filter(|line| {
                line.split(' ')
                    .next()
                    .is_some_and(|stem| name_and_version(stem).is_some())
            })
            .map(|line| format!("{line}\n"))
            .collect();
        let Ok(index) = Index::parse(&ours) else {
            return refuse("the index has a line this jig cannot read; the copy was not changed");
        };
        if ours.len() > package_index::COPY_MAX {
            return refuse("this architecture's index is larger than a copy may be (4095 bytes)");
        }
        if let Err(e) = std::fs::write(IMAGE_INDEX_COPY, ours.as_bytes()) {
            return say_io(IMAGE_INDEX_COPY, e);
        }
        println!(
            "  index updated: {} packages for {ARCHITECTURE}, from {} over TLS",
            index.entries().count(),
            repo.host
        );
        return ExitCode::SUCCESS;
    }
    refuse("no index address answered; the copy was not changed")
}

/// **A package the index copy lists, fetched over HTTPS and judged by the entry's digest**
/// (milestone 801): the entry's listed locations, each only at a public address (Q1), then each of
/// the image's repositories' own `targets/`. The bytes go straight into the staging run.
fn install_indexed(entry: &Entry<'_>) -> ExitCode {
    let sources = match sources() {
        Ok(s) => s,
        Err(why) => return refuse(why),
    };
    let root = match root_der(&sources.root) {
        Ok(r) => r,
        Err(why) => return refuse(why),
    };
    let Some(first) = sources.repositories().next() else {
        return refuse("this image names no index address");
    };
    let mut staged = match Staged::new(spawnproto::IMAGE_MAX_PAGES * spawnproto::IMAGE_PAGE) {
        Ok(s) => s,
        Err(why) => return refuse(why),
    };
    let listed = Index::sources(entry, &first.repository, None).filter_map(|s| match s {
        Source::Listed(at) => Some(at),
        Source::Repository(_) => None,
    });
    let mut whence = None;
    for at in listed {
        // The root the signed index named for this location, among those the image carries
        // (calef's C2 on #2048, 2026-10-12 UTC); a redirect's target is held to the index's own.
        let fetched = sources
            .root_for(&at)
            .ok_or_else(|| "it names a root this image does not carry".to_string())
            .and_then(|named| root_der(&named).map_err(str::to_string))
            .and_then(|first_root| {
                let mut got = None;
                fill(&mut staged, |sink| {
                    got = Some(listed::fetch(
                        &at,
                        &first_root,
                        &root,
                        package_index::public_address,
                        sink,
                    )?);
                    Ok(())
                })?;
                got.ok_or_else(|| "nothing arrived".to_string())
            });
        match fetched {
            Ok(from) => {
                whence = Some(match from.redirected_from {
                    Some(via) => format!("{} via {via}", from.host),
                    None => from.host,
                });
                break;
            }
            Err(why) => println!(
                "  passed over https://{}:{}{}: {why}",
                at.host, at.port, at.path
            ),
        }
    }
    if whence.is_none() {
        for configured in sources.repositories() {
            let repo = configured.repository;
            let mut buf = [0u8; PATH_MAX];
            let Some(path) = repo.target_path(entry.stem, &mut buf) else {
                continue;
            };
            let fetched = addresses(&configured)
                .and_then(|to| fill(&mut staged, |sink| get(repo.host, &to, &root, path, sink)));
            match fetched {
                Ok(()) => {
                    whence = Some(repo.host.to_string());
                    break;
                }
                Err(why) => println!("  {}'s copy did not arrive: {why}", repo.host),
            }
        }
    }
    let Some(whence) = whence else {
        println!("  no source sent the package; nothing was installed");
        return ExitCode::FAILURE;
    };
    if package_index::accept(entry, staged.bytes()).is_err() {
        return refuse(
            "the index does not vouch for the bytes that arrived; nothing was sent to the installer",
        );
    }
    println!("  fetched {} from {whence} over TLS", entry.stem);
    send_install(&mut staged, true)
}

/// Run one fetch into the staging run from its start, and set the run's length to what arrived.
fn fill(
    staged: &mut Staged,
    fetch: impl FnOnce(&mut dyn FnMut(&[u8])) -> Result<(), String>,
) -> Result<(), String> {
    let capacity = staged.capacity;
    staged.len = capacity;
    let mut filled = 0usize;
    let mut over = false;
    {
        let bytes = staged.bytes();
        fetch(&mut |part: &[u8]| {
            match bytes.get_mut(filled..filled + part.len()) {
                Some(dest) => dest.copy_from_slice(part),
                None => over = true,
            }
            filled += part.len();
        })?;
    }
    if over {
        return Err("larger than a package may be (4 MiB)".into());
    }
    staged.len = filled as u64;
    Ok(())
}

/// **`jig remove <package>`**, or `<package>@<version>` for one version: every row the package
/// installed, whatever its programs are called (calef, 2026-10-14 (UTC), milestone 919 (a package
/// carries several programs): "jig remove should work on packages, not programs.").
fn remove(package: &str) -> ExitCode {
    if package.len() > filesystem_contract::grant::MAX_NAME {
        println!("{USAGE}");
        return ExitCode::from(2);
    }
    if !holds_installer() {
        return ExitCode::FAILURE;
    }
    answer(
        Activation::Remove,
        false,
        ask(Activation::Remove, None, Some(package)),
        None,
    )
}

/// **`jig rollback`**: one generation back.
fn rollback() -> ExitCode {
    if !holds_installer() {
        return ExitCode::FAILURE;
    }
    answer(
        Activation::Rollback,
        false,
        ask(Activation::Rollback, None, None),
        None,
    )
}

/// Say a refusal this program made itself, sending nothing.
fn refuse(why: &str) -> ExitCode {
    println!("  refused: {why}");
    ExitCode::FAILURE
}

fn say_io(path: &str, e: std::io::Error) -> ExitCode {
    let why = match e.kind() {
        ErrorKind::Unsupported => "no directory was granted to read it from".to_string(),
        k => format!("{k:?}"),
    };
    println!("  could not read {path}: {why}");
    ExitCode::FAILURE
}

/// **What the progenitor's answer means**, in the words the shell's `package` builtin used, so a
/// person who learned those reads the same sentences. Every answer names the generation live
/// afterwards, because after a refusal that is still the fact a person needs.
fn answer(verb: Activation, fetched: bool, reply: Reply, program: Option<&str>) -> ExitCode {
    let Reply {
        status,
        live,
        holder,
        ..
    } = reply;
    let program = program.map_or("one of its programs".to_string(), |p| {
        format!("the program {p}")
    });
    let named;
    let said = match (status, verb) {
        (S::Done, Activation::Install) if fetched => "fetched and installed",
        (S::Done, Activation::Install) => "installed",
        (S::Done, Activation::Remove) => "removed",
        (S::Done, _) => "rolled back",
        (S::NotCataloged, _) => {
            "refused: neither this image's catalog nor its index copy vouches for those bytes"
        }
        (S::NoProgram, _) => {
            "refused: that package carries no program jig can install: none named after it or \
             under bin/, or a bin/ name that is empty, nested or repeated"
        }
        (S::NotInstalled, _) => "refused: no package of that name is in the live generation",
        (S::NoEarlier, _) => "refused: there is no generation before the live one",
        (S::StoreFailed, _) => "could not write the installed programs",
        // calef, 2026-10-14 (UTC): "If two packages offer the same program name, refuse the
        // second package. Need to remove the first one before installing the second one."
        (S::NameTaken, _) => {
            named = format!(
                "refused: the package {holder} already provides {program}; \
                 jig remove {holder} first"
            );
            named.as_str()
        }
        (S::ImageName, _) => {
            named =
                format!("refused: the image carries {program}; a new base updates it, not install");
            named.as_str()
        }
        (S::Ambiguous, _) => {
            "refused: that version holds a program's default and several other versions of it are \
             live, so none can take its place"
        }
        (S::NotServedHere, _) => "refused: the installer endpoint does not take that request",
        (S::NotYours, _) => {
            "refused: another package manager installed that, and it is not jig's to undo"
        }
        (S::NotExecutable | S::Unknown, _) => "the progenitor could not take that request",
    };
    if live == 0 {
        println!("  {said}; nothing is installed");
    } else {
        println!("  {said}; generation {live} is live");
    }
    if status == S::Done {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
