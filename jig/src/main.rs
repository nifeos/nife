//! **`jig`, the package manager** (milestone 809 (the package client becomes a program)).
//!
//! The client of the progenitor's installer endpoint (DECISIONS §270 (a package manager holds an
//! installer endpoint, not the spawn endpoint)); the progenitor stays the installer (DECISIONS §208
//! (installing a package is granting it, and the installed programs are versioned)). `jig` fetches
//! a package itself, so the progenitor parses no network input, and sends it only if it is the one
//! asked for and the catalog vouches for its digest (`package_archive::installable_as`). `update`
//! fetches the image's index over HTTPS, pinned to the root the image names (milestone 801 (packages
//! over the internet)), and keeps a copy later lookups consult first. notes/packages/jig.md has what
//! each verb holds, how an install and an `update` travel, and the BUGS.

//! # SYNOPSIS
//! ```text
//! jig update                   fetch the image's index over HTTPS and keep a copy of it
//! jig list                     the packages the index copy offers, or else the image's catalog
//! jig list -u                  the installed packages the index copy offers a newer version of
//! jig install <path>           install a package file already on this system
//! jig install <name>[@<ver>]   fetch a package by name and install it
//! jig remove <package>         a new generation without the package's programs
//! jig rollback                 the generation below the live one becomes live
//! jig approve <file>           store the approval file's compiled records, on the owner's yes
//! jig revoke <name>            drop the named approval
//! ```

//! The flags an install, removal or rollback takes, after the operand: `--approved <file>`
//! replaces the owner's prompt with the check that the package's asks are within the file's
//! entry, and fails closed naming every extra grant; `--within <name>` sends the request on the
//! delegated endpoint the spawn placed, whose check is the progenitor's; `--json` prints one
//! machine-readable object. `jig approve` prints every grant the file contains and asks once;
//! `jig revoke <name>` drops the stored approval.

//! # EXIT STATUS
//! Documented and stable: `0` the verb succeeded; `1` it was refused or failed, and said why;
//! `2` the line was not a spelling of the verbs above.

//! Name: ratified 2026-10-06 (calef). See `grant_plan::Program::Jig`.

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream, ToSocketAddrs};
use std::process::ExitCode;

use entropy_backend as _;
use grant_plan::spawnproto::{self, Activation, ActivationStatus as S};
use grant_plan::{CATALOG_SLOT, INDEX_SOURCES_SLOT, INSTALLER_REPLY_SLOT, INSTALLER_SLOT};
use package_index::{
    Configured, Entry, IMAGE_INDEX_COPY, Index, Miss, PATH_MAXIMUM, Root, STAND_IN_INDEX_FILE,
    Source, Sources,
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
// (`capability_manifest_note::encode` refuses both at compile time), and a `std` image cannot carry the network.
capability_manifest_note::carry!(grant_plan::Manifest {
    network: false,
    installer: false,
    catalog: false,
    indexes: grant_plan::IndexStoreDeclaration::Forbidden,
    activation_directory: false,
    console_input: false,
    ..grant_plan::Program::Jig.manifest()
});

/// **Where the staging run is mapped**: one package, read here and handed on a page at a time. A
/// window of this program's own, so a pair page (`address_space_map`'s rule 7), clear of everything
/// a loader maps into a `std` program. Four MiB, `spawnproto::IMAGE_MAXIMUM_PAGES` pages.
const STAGING_VA: u64 = address_space_map::pair_page(0x0400_0000);

/// This machine's architecture, as a package stem spells it.
const ARCHITECTURE: &str = std::env::consts::ARCH;

const USAGE: &str = "  jig update | jig list [-u] | jig install <file or name> [--approved <file>] \
                     | jig remove <package> | jig rollback | jig approve <file> | jig revoke <name>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["update"] => update(),
        ["list"] => list(),
        ["list", "-u"] => list_outdated(),
        // calef, 2026-10-17 (UTC): "`-u` only, with a refusal that names `-u` when someone types
        // `--upgradable` or `--upgradeable`." apt accepts both; this names the one jig takes.
        ["list", "--upgradable" | "--upgradeable"] => {
            refuse("jig spells that list -u; it takes no long form")
        }
        ["install", rest @ ..] => install(rest),
        ["remove", rest @ ..] => remove(rest),
        ["rollback", rest @ ..] => rollback(rest),
        ["approve", rest @ ..] => approve(rest),
        ["revoke", rest @ ..] => revoke(rest),
        _ => {
            println!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// **The flags an install, removal, rollback, approve or revoke takes** (milestone 911 (scripted
/// installs grant only what the owner's approval file allows)): `--approved <file>` and
/// `--within <name>` before or after the operand, `--json` anywhere. A word this reader does not
/// know is a usage exit, not a silence.
struct Flags<'a> {
    /// The approval file the answer is held to, as the line named it.
    approved: Option<&'a str>,
    /// The approval the spawn delegated this job to, whose check is the progenitor's.
    within: Option<&'a str>,
    /// The machine-readable form.
    json: bool,
    /// The verb's one operand: a package, a package file, an approval file or a name.
    operand: Option<&'a str>,
}

/// [`Flags`] from the words after the verb, or `None` for a line that is not one spelling.
fn parse_flags<'a>(words: &'a [&'a str]) -> Option<Flags<'a>> {
    let mut flags = Flags {
        approved: None,
        within: None,
        json: false,
        operand: None,
    };
    let mut words = words.iter();
    while let Some(word) = words.next() {
        match *word {
            "--approved" => flags.approved = Some(words.next()?),
            "--within" => flags.within = Some(words.next()?),
            "--json" => flags.json = true,
            w if w.starts_with('-') => return None,
            w => {
                if flags.operand.is_some() {
                    return None;
                }
                flags.operand = Some(w);
            }
        }
    }
    Some(flags)
}

/// The console's one trailer message, the answered-by every install and removal sends
/// (milestone 911): the kind word, and the progenitor reads no more of it.
const CONSOLE_TRAILER: [(u64, u64, u64); 1] = [(spawnproto::ANSWERED_CONSOLE, 0, 0)];

/// **One JSON string**, escaped: the quotes, the backslash and the controls a name could carry.
fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// **The machine-readable refusal**, one object: what was refused and, for the subset test's
/// failure, every extra grant as data (milestone 911, item 7).
fn json_refused(extra: &[String], why: &str) {
    let each: Vec<String> = extra.iter().map(|e| json_string(e)).collect();
    println!(
        "  {{\"result\":\"refused\",\"why\":{},\"extra\":[{}]}}",
        json_string(why),
        each.join(",")
    );
}

/// **The machine-readable result**, one object: the verb and the generation live afterwards.
fn json_result(what: &str, generation: u64) {
    println!(
        "  {{\"result\":{},\"generation\":{generation}}}",
        json_string(what)
    );
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

/// **`jig list -u`**: each installed package the index copy offers a newer version of, as
/// `<name> <installed> -> <newer>`. Installed means a row of the live generation that a package
/// put there; an owner's vouch claims no version, so it has nothing to be outdated against.
fn list_outdated() -> ExitCode {
    let installed = match live_generation() {
        Ok(table) => table,
        Err(why) => return refuse(&why),
    };
    let copy_path = format!("{}/{IMAGE_INDEX_COPY}", package_index::STORE_DIRECTORY);
    let Ok(copy) = std::fs::read_to_string(&copy_path) else {
        return refuse("there is no index copy to compare against; run jig update");
    };
    let Ok(index) = Index::parse(&copy) else {
        return refuse("the index copy cannot be read; run jig update");
    };
    let offered: Vec<(&str, &str)> = index
        .entries()
        .filter_map(|e| name_and_version(e.stem))
        .collect();
    let outdated = match outdated(&installed, &offered) {
        Ok(rows) => rows,
        Err(why) => return refuse(&why),
    };
    if outdated.is_empty() {
        println!("  no installed package has a newer version in the index copy");
    } else {
        println!("  installed packages the index offers newer (the copy jig update wrote):");
        for (name, have, newer) in outdated {
            println!("  {name} {have} -> {newer}");
        }
    }
    ExitCode::SUCCESS
}

/// **The live generation's table**, read through the activation directory: `current` names it.
fn live_generation() -> Result<String, String> {
    const NO_VIEW: &str =
        "this jig holds no view of what is installed; only the owner's console grants one";
    // The sources page comes with the index store alone, so a line holding it was granted
    // `activation/indexes` at the directory slot, not `activation/`: there `current` is absent,
    // and reading on would report nothing installed rather than nothing granted.
    if sources_text().is_some() {
        return Err(NO_VIEW.into());
    }
    let current = match std::fs::read_to_string(installed_programs::CURRENT) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(String::new()),
        Err(e) if e.kind() == ErrorKind::Unsupported => return Err(NO_VIEW.into()),
        Err(e) => return Err(format!("could not read what is installed: {:?}", e.kind())),
    };
    let number = installed_programs::parse_current(&current)
        .ok_or("the live generation's name cannot be read")?;
    let mut name = [0u8; 10];
    let name = installed_programs::generation_name(number, &mut name);
    std::fs::read_to_string(name)
        .map_err(|e| format!("could not read generation {number}: {:?}", e.kind()))
}

/// **Which installed packages `offered` has a newer version of**, in the table's order: each as
/// its name, its newest installed version and the newest version offered. A version Debian's order
/// cannot read refuses the whole answer, naming it, since skipping it would report a package as
/// current that might not be.
fn outdated<'a>(
    table: &'a str,
    offered: &[(&'a str, &'a str)],
) -> Result<Vec<(&'a str, &'a str, &'a str)>, String> {
    let mut rows: Vec<(&str, &str, &str)> = Vec::new();
    for entry in installed_programs::entries(table) {
        let entry = entry.map_err(|_| "the live generation cannot be read".to_string())?;
        let package = entry.package;
        if package == installed_programs::OWNER || rows.iter().any(|r| r.0 == package) {
            continue;
        }
        let rows_of = installed_programs::entries(table).flatten();
        let have = rows_of
            .filter(|e| e.package == package && e.version != installed_programs::NO_VERSION)
            .map(|e| e.version);
        let Some(have) = newest(have)? else {
            continue;
        };
        let theirs = offered.iter().filter(|o| o.0 == package).map(|o| o.1);
        // The installed version first, so an equal one offered (`1.00` for `1.0`) is not newer.
        if let Some(best) = newest([have].into_iter().chain(theirs))?
            && best != have
        {
            rows.push((package, have, best));
        }
    }
    Ok(rows)
}

/// [`package_index::version::newest`], with the version it could not order named.
fn newest<'a>(versions: impl Iterator<Item = &'a str>) -> Result<Option<&'a str>, String> {
    package_index::version::newest(versions)
        .map_err(|(v, why)| format!("jig cannot order the version {v} ({why:?})"))
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
        if pages == 0 || pages > spawnproto::IMAGE_MAXIMUM_PAGES {
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
            rt::capability_delete(run);
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
/// index, 0 otherwise), the package already holding that program's name for `NameTaken`, the
/// stored records an `Approve` replaced ([`Reply::stored`], which the re-approval diffs its
/// file against), and the ask an `--approved` install's check refused ([`Reply::ask`]).
struct Reply {
    status: S,
    live: u64,
    member: u64,
    holder: String,
    stored: Option<String>,
    ask: Option<[u64; 3]>,
}

/// **Send one activation request on the installer endpoint and read its answer**: the frames
/// of `staged` for an install, the packed name for a removal, nothing for a rollback, and then
/// `trailer`, the answered-by messages an install and a removal always send (milestone 911:
/// how the answer was given, which the progenitor writes beside the record). An `Approve`'s
/// records are their own payload; it sends none of these. After the reply, the followers the
/// status carries: the package a `NameTaken` or a `NotApproved` refusal is about, the ask the
/// owner's `--approved` check refused, and the stored records an `Approve` replaced, whose
/// length the reply's word 2 carries.
fn ask(
    verb: Activation,
    staged: Option<&Staged>,
    name: Option<&str>,
    trailer: &[(u64, u64, u64)],
) -> Reply {
    let len = staged.map_or(0, |s| s.len);
    let (w0, w1, w2) = spawnproto::activation_request(verb, len);
    rt::send(INSTALLER_SLOT, w0, w1, w2);
    if let Some(st) = staged {
        for i in 0..spawnproto::image_pages(st.len) {
            match u64::try_from(rt::slice_page_frame(st.run, i, 1)) {
                Ok(page) => {
                    rt::send_capability(INSTALLER_SLOT, page, abi::rights::READ, i);
                    rt::capability_delete(page);
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
        // Every caller has held `name` to `grant::WORDS_NAME`; a longer one sends empty words,
        // which the progenitor answers as a name it does not know.
        let (lo, hi) = filesystem_contract::grant::pack_name(name.as_bytes()).unwrap_or((0, 0));
        rt::send(INSTALLER_SLOT, lo, hi, name.len() as u64);
    }
    for (a, b, c) in trailer {
        rt::send(INSTALLER_SLOT, *a, *b, *c);
    }
    let (r0, r1, r2) = rt::receive(INSTALLER_REPLY_SLOT);
    let status = S::from_word(r0);
    let mut holder = String::new();
    let mut ask = None;
    // **The messages that name what a refusal is about** (milestone 919 (a package carries
    // several programs): calef ruled the owner removes the first package before installing the
    // second; milestone 911: the delegated check's refusal names the package it refused, the
    // owner's `--approved` check the ask too, and the reply's word 2 counts them).
    if status == S::NameTaken || status == S::NotApproved {
        let count = if status == S::NotApproved {
            r2.clamp(2, 3) as usize
        } else {
            2
        };
        let words: Vec<_> = (0..count)
            .map(|_| rt::receive(INSTALLER_REPLY_SLOT))
            .collect();
        let mut out = [0u8; spawnproto::HOLDER_MAXIMUM];
        holder = spawnproto::holder_from_words([words[0], words[1]], &mut out).to_string();
        if count == 3 {
            let (a, b, c) = words[2];
            ask = Some([a, b, c]);
        }
    }
    // **The stored records an `Approve` replaced** (milestone 911), as the reply's word 2
    // length and the messages that carry them: the diff the re-approval shows is against
    // these, which the progenitor read from its own store.
    let mut stored = None;
    if status == S::Done && verb == Activation::Approve && r2 > 0 {
        let len = r2.min(spawnproto::IMAGE_PAGE) as usize;
        let mut bytes = vec![0u8; len];
        for i in 0..spawnproto::stored_messages(len as u64) {
            let (a, b, c) = rt::receive(INSTALLER_REPLY_SLOT);
            let mut write = |at: usize, word: u64| {
                let take = word.to_le_bytes();
                let out = &mut bytes[at..(at + 8).min(len)];
                out.copy_from_slice(&take[..out.len()]);
            };
            let at = (i * 24) as usize;
            write(at, a);
            write(at + 8, b);
            write(at + 16, c);
        }
        stored = String::from_utf8(bytes).ok();
    }
    Reply {
        status,
        live: r1,
        member: r2,
        holder,
        stored,
        ask,
    }
}

/// **What one program of a package asks the owner for** (milestone 892 (a std program at the
/// prompt holds the network and a resolver), "The network at install, ruled"; the service ask is
/// milestone 911 (scripted installs grant only what the owner's approval file allows)'s, note
/// version 3): the network, each resolver zone its manifest note names, and the service. A
/// program whose bytes or note cannot be read asks nothing, and the progenitor refuses those
/// bytes its own way.
#[derive(Debug)]
struct Asks {
    /// The program's name, as the package names the member.
    name: String,
    network: bool,
    service: bool,
    /// The zones, as the note spells them; the root zone is empty.
    zones: Vec<String>,
}

/// **Every program's asks in `bytes`**, in the package's order.
fn asks(bytes: &[u8]) -> Vec<Asks> {
    let Ok(package) = package_archive::Package::parse(bytes) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for index in 0..package.len() {
        let (Some(member), Some(name)) = (
            package.member(index),
            package_archive::program_name(&package, index),
        ) else {
            continue;
        };
        let Ok(elf) = elf::Elf::parse(member) else {
            continue;
        };
        let Ok(Some(d)) = elf.note(
            capability_manifest_note::OWNER,
            capability_manifest_note::MANIFEST,
        ) else {
            continue;
        };
        let (Ok(m), Ok(zones)) = (
            capability_manifest_note::decode(d),
            capability_manifest_note::zones(d),
        ) else {
            continue;
        };
        let Ok(service) = capability_manifest_note::service(d) else {
            continue;
        };
        if !m.network && !service && zones.is_empty() {
            continue;
        }
        out.push(Asks {
            name: name.to_string(),
            network: m.network,
            service,
            zones: zones
                .iter()
                .map(|z| String::from_utf8_lossy(z).into_owned())
                .collect(),
        });
    }
    out
}

/// **One grant, in the words the confirmation prints it in**, which is what the approval file's
/// display and its refusals share (milestone 911: "by the same names jig install's confirmation
/// prints").
fn say_grant(what: &installed_programs::Granted<'_>) -> String {
    match what {
        installed_programs::Granted::Network => "reaches the network (connects out)".to_string(),
        installed_programs::Granted::Zone(zone) if zone.is_empty() => {
            "LOOKS UP ANY NAME AT ALL (the root zone)".to_string()
        }
        installed_programs::Granted::Zone(zone) => format!("looks up names in {zone}"),
        installed_programs::Granted::Service => {
            "runs as a service, now and at every boot".to_string()
        }
        installed_programs::Granted::Directory(name, rights) => {
            format!("holds {name} ({})", rights.spell())
        }
        installed_programs::Granted::Installer => {
            "holds the installer, within this approval".to_string()
        }
    }
}

/// **One `Granted` as data**, the `--json` refusal's spelling of it.
fn json_grant(what: &installed_programs::Granted<'_>) -> String {
    match what {
        installed_programs::Granted::Network => "network".to_string(),
        installed_programs::Granted::Zone(zone) if zone.is_empty() => "zone .".to_string(),
        installed_programs::Granted::Zone(zone) => format!("zone {zone}"),
        installed_programs::Granted::Service => "service".to_string(),
        installed_programs::Granted::Directory(name, rights) => {
            format!("dir {name} {}", rights.spell())
        }
        installed_programs::Granted::Installer => "installer".to_string(),
    }
}

/// **The asks listed before the package is sent**, the one confirmation (milestone 892; calef's
/// rulings on #2405). Whether anything was listed, which is whether the owner must be asked
/// ([`owner_says_yes`]).
fn say_asks(listed: &[Asks]) -> bool {
    let mut any = false;
    for asks in listed {
        any = true;
        println!("  {} asks for:", asks.name);
        if asks.network {
            println!("    reaches the network (connects out)");
        }
        for zone in &asks.zones {
            if zone.is_empty() {
                println!("    LOOKS UP ANY NAME AT ALL (the root zone)");
            } else {
                println!("    looks up names in {zone}");
            }
        }
        if asks.service {
            println!("    runs as a service, now and at every boot");
        }
    }
    any
}

/// **The owner's one yes for every grant listed** (milestone 892; calef's rulings on #2405: one
/// confirmation, all or nothing, read from the console as standard input). `y` or `yes`, in any
/// case, grants them; anything else, end of input included, grants nothing and installs nothing.
/// A `jig` started with no console input (a script, a line that is not `install`) reads end of
/// input at once, so it can never install a package that asks for something.
fn owner_says_yes(question: &str) -> bool {
    use std::io::{BufRead, Write};
    print!("  {question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    if !answer.ends_with('\n') {
        // End of input reached before a line: say so on its own line.
        println!();
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// **Send `staged` to the installer and say what came of it**, naming a refused program from these
/// bytes, which are the ones the progenitor judged (milestone 919). The flags decide who answers
/// (milestone 911 (scripted installs grant only what the owner's approval file allows)): the
/// owner's yes, the `--approved` file's check, or neither under `--within`, whose check is the
/// progenitor's. The trailer says which, beside the record.
fn send_install(staged: &mut Staged, fetched: bool, flags: &Flags<'_>) -> ExitCode {
    let listed = asks(staged.bytes());
    // The package's name and version, for the ask refusal's naming: parsed and taken before the
    // send, whose frames borrow the staged pages.
    let (package_name, version) = {
        let package = package_archive::Package::parse(staged.bytes()).ok();
        (
            package.as_ref().map(|p| p.name().to_string()),
            package.as_ref().map(|p| p.version().to_string()),
        )
    };
    let answered;
    let approved;
    let mut said = Said {
        package: package_name.as_deref(),
        version: version.as_deref(),
        ..Said::default()
    };
    if let Some(within) = flags.within {
        // The delegated endpoint's check is the progenitor's; the owner is not asked, and jig
        // runs no subset test of its own (it holds no file). The trailer says the console sent
        // it, which the badge's approval overrides.
        println!("  installing within approval {within}; the progenitor decides");
        said.within = Some(within);
        answered = CONSOLE_TRAILER.to_vec();
    } else if let Some(path) = flags.approved {
        // The owner's own file replaces the prompt: the package must be one its entry bounds,
        // which is checked here, before anything is sent. The subset test is the progenitor's,
        // against the records the trailer's digest binds.
        let file = match approval_answer(staged.bytes(), path, flags.json) {
            Ok(file) => file,
            Err(exit) => return exit,
        };
        approved = Some(file.name);
        answered = file.trailer;
        said.approved = approved.as_deref();
    } else if say_asks(&listed) && !owner_says_yes("grant these and install?") {
        println!("  not installed; nothing was granted");
        return ExitCode::FAILURE;
    } else {
        answered = CONSOLE_TRAILER.to_vec();
    }
    let reply = ask(Activation::Install, Some(staged), None, &answered);
    let program = reply.member.checked_sub(1).and_then(|index| {
        let package = package_archive::Package::parse(staged.bytes()).ok()?;
        let index = usize::try_from(index).ok()?;
        package_archive::program_name(&package, index).map(String::from)
    });
    answer(
        Activation::Install,
        fetched,
        reply,
        program.as_deref(),
        &said,
        flags.json,
    )
}

/// **The approval's pre-flight, and the trailer that says it answered** (milestone 911): the
/// file parsed whole, and the package refused here when no entry of the file bounds it, before
/// anything is sent or fetched past the manifest. The subset test itself is the progenitor's,
/// against the store's records, which the trailer's digest binds; its refusal names the ask and
/// the generation. The trailer binds the answer to the file's name and digest.
struct ApprovalAnswer {
    /// The three trailer messages.
    trailer: Vec<(u64, u64, u64)>,
    /// The file's `approval` key, for the refusal's naming.
    name: String,
}

fn approval_answer(bytes: &[u8], path: &str, json: bool) -> Result<ApprovalAnswer, ExitCode> {
    let failure = |why: &str, extra: &[String]| {
        if json {
            json_refused(extra, why);
        } else {
            for line in extra {
                println!("  refused: {line}");
            }
        }
        ExitCode::FAILURE
    };
    let text = read_approval_text(path, json)?;
    let file = parse_approval(&text, path, json)?;
    let package = package_archive::Package::parse(bytes).ok();
    let package_name = package.as_ref().map(|p| p.name()).unwrap_or("");
    let version = package.as_ref().map(|p| p.version()).unwrap_or("");
    if file.entry(package_name).is_none() {
        let why = format!(
            "{package_name} {version} is not in {path}, which {} bounds",
            file.name()
        );
        return Err(failure(&why, &[]));
    }
    // The grants given are exactly the asks; the answer binds to the file's digest, which the
    // progenitor could not otherwise know.
    Ok(ApprovalAnswer {
        trailer: approval_trailer(&text, &file).to_vec(),
        name: file.name().to_string(),
    })
}

/// Whether this process was granted an installer endpoint; says why not when it was not.
fn holds_installer() -> bool {
    let held = rt::is_granted(INSTALLER_SLOT) && rt::is_granted(INSTALLER_REPLY_SLOT);
    if !held {
        println!("  this jig holds no installer endpoint; only the owner's console grants one");
    }
    held
}

/// **`jig install <path> ...`**: the file's bytes, sent as they are. The progenitor's catalog
/// decides. The flags decide who answers: the owner's yes, the `--approved` file's check, or the
/// delegated endpoint's.
fn install(words: &[&str]) -> ExitCode {
    let Some(flags) = parse_flags(words) else {
        println!("{USAGE}");
        return ExitCode::from(2);
    };
    let Some(operand) = flags.operand else {
        println!("{USAGE}");
        return ExitCode::from(2);
    };
    if operand.contains('/') {
        install_file(operand, &flags)
    } else {
        install_name(operand, &flags)
    }
}

/// **`jig install <path>`**: the file's bytes, sent as they are. The progenitor's catalog decides.
fn install_file(path: &str, flags: &Flags<'_>) -> ExitCode {
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
    send_install(&mut staged, false, flags)
}

/// **`jig install <name>`**: the catalog names the stem, the package source serves
/// `/<stem>.nifepkg`, and what arrives is checked before anything is sent: it must be the package
/// asked for, and the catalog must vouch for its digest.
fn install_name(name: &str, flags: &Flags<'_>) -> ExitCode {
    if !holds_installer() {
        return ExitCode::FAILURE;
    }
    // **The index copy first, once `update` has written one** (milestone 801).
    if let Some(copy) = index_copy() {
        let Ok(index) = Index::parse(&copy) else {
            return refuse("the index copy cannot be read; run jig update");
        };
        match index.find(name, ARCHITECTURE) {
            Ok(entry) => return install_indexed(&entry, flags),
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
    send_install(&mut staged, true, flags)
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
        .map_err(|e| format!("not resolved: {}", listed::reason(&e)))?
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
    let tcp = TcpStream::connect(to).map_err(|e| listed::reason(&e))?;
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
        let mut buf = [0u8; PATH_MAXIMUM];
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
        if ours.len() > package_index::COPY_MAXIMUM {
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
fn install_indexed(entry: &Entry<'_>, flags: &Flags<'_>) -> ExitCode {
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
    let mut staged = match Staged::new(spawnproto::IMAGE_MAXIMUM_PAGES * spawnproto::IMAGE_PAGE) {
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
            let mut buf = [0u8; PATH_MAXIMUM];
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
    send_install(&mut staged, true, flags)
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
/// carries several programs): "jig remove should work on packages, not programs."). Under
/// `--approved <file>` the entry's `remove` answer is checked first; under `--within <name>` the
/// delegated endpoint's check is the progenitor's.
fn remove(words: &[&str]) -> ExitCode {
    let Some(flags) = parse_flags(words) else {
        println!("{USAGE}");
        return ExitCode::from(2);
    };
    let Some(package) = flags.operand else {
        println!("{USAGE}");
        return ExitCode::from(2);
    };
    if !filesystem_contract::grant::fits_words(package.as_bytes()) {
        println!("{USAGE}");
        return ExitCode::from(2);
    }
    if !holds_installer() {
        return ExitCode::FAILURE;
    }
    let answered = if flags.within.is_some() {
        CONSOLE_TRAILER.to_vec()
    } else if let Some(path) = flags.approved {
        match removal_answer(package, path, flags.json) {
            Ok(trailer) => trailer,
            Err(exit) => return exit,
        }
    } else {
        CONSOLE_TRAILER.to_vec()
    };
    answer(
        Activation::Remove,
        false,
        ask(Activation::Remove, None, Some(package), &answered),
        None,
        &Said {
            within: flags.within,
            ..Said::default()
        },
        flags.json,
    )
}

/// **The removal the approval file allows** (milestone 911): a removal passes when the entry
/// says `remove = true`, and fails closed before anything is sent. The trailer binds the answer
/// to the file's name and digest, the same binding an approved install's carries.
fn removal_answer(package: &str, path: &str, json: bool) -> Result<Vec<(u64, u64, u64)>, ExitCode> {
    let text = read_approval_text(path, json)?;
    let file = parse_approval(&text, path, json)?;
    let entry = file
        .entry(package)
        .ok_or_else(|| not_bounded(package, path, &file, json))?;
    if !entry.remove {
        let why = format!(
            "{package} may not be removed by {}, whose entry says remove = false",
            file.name()
        );
        if json {
            json_refused(&[], &why);
        } else {
            println!("  refused: {why}");
        }
        return Err(ExitCode::FAILURE);
    }
    Ok(approval_trailer(&text, &file).to_vec())
}

/// **The approval file read whole**, or the refusal naming the line: the bytes the answer's
/// digest is taken over. The caller parses it beside the text, since the parse borrows it.
fn read_approval_text(path: &str, json: bool) -> Result<String, ExitCode> {
    let failure = |why: String| {
        if json {
            json_refused(&[], &why);
        } else {
            println!("  refused: {why}");
        }
        ExitCode::FAILURE
    };
    std::fs::read_to_string(path.strip_prefix("./").unwrap_or(path)).map_err(|e| {
        failure(format!(
            "could not read the approval file {path}: {:?}",
            e.kind()
        ))
    })
}

/// **The approval file parsed whole**, or the refusal naming the line.
fn parse_approval<'a>(
    text: &'a str,
    path: &str,
    json: bool,
) -> Result<approval_file::File<'a>, ExitCode> {
    approval_file::File::parse(text).map_err(|e| {
        let why = format!(
            "the approval file {path} cannot be read: line {}: {:?}",
            e.at, e.why
        );
        if json {
            json_refused(&[], &why);
        } else {
            println!("  refused: {why}");
        }
        ExitCode::FAILURE
    })
}

/// **The not-bounded refusal**: the package no entry of the file's bounds, so it may receive
/// nothing (the subset test fails closed).
fn not_bounded(package: &str, path: &str, file: &approval_file::File<'_>, json: bool) -> ExitCode {
    let why = format!("{package} is not in {path}, which {} bounds", file.name());
    if json {
        json_refused(&[], &why);
    } else {
        println!("  refused: {why}");
    }
    ExitCode::FAILURE
}

/// **The answered trailer for an approval**: the file's name and the digest of the bytes the
/// answer was given on, which the progenitor could not otherwise know (milestone 911).
fn approval_trailer(text: &str, file: &approval_file::File<'_>) -> [(u64, u64, u64); 3] {
    spawnproto::answered_approval(file.name(), &package_archive::sha256(text.as_bytes()))
}

/// **`jig rollback`**: one generation back. Under `--within <name>` the delegated endpoint's
/// check is the progenitor's, against the approval the badge carries.
fn rollback(words: &[&str]) -> ExitCode {
    let Some(flags) = parse_flags(words) else {
        println!("{USAGE}");
        return ExitCode::from(2);
    };
    if flags.operand.is_some() || flags.approved.is_some() {
        println!("{USAGE}");
        return ExitCode::from(2);
    }
    if !holds_installer() {
        return ExitCode::FAILURE;
    }
    // A rollback writes no generation, so it carries no answered line and sends no trailer.
    answer(
        Activation::Rollback,
        false,
        ask(Activation::Rollback, None, None, &[]),
        None,
        &Said {
            within: flags.within,
            ..Said::default()
        },
        flags.json,
    )
}

/// **`jig approve <file>`**: the file compiled on the owner's console, every grant it contains
/// printed (the root zone flagged apart, the way the confirmation flags it), the changed grants
/// only when the name is stored already, one yes covering all of it, and the records sent as an
/// `Approve` request (milestone 911). `--within <name>` sends without the ask: the delegated
/// endpoint refuses it, which is the row the gate proves.
fn approve(words: &[&str]) -> ExitCode {
    let Some(flags) = parse_flags(words) else {
        println!("{USAGE}");
        return ExitCode::from(2);
    };
    let Some(path) = flags.operand else {
        println!("{USAGE}");
        return ExitCode::from(2);
    };
    if !holds_installer() {
        return ExitCode::FAILURE;
    }
    let text = match read_approval_text(path, flags.json) {
        Ok(t) => t,
        Err(exit) => return exit,
    };
    let file = match parse_approval(&text, path, flags.json) {
        Ok(file) => file,
        Err(exit) => return exit,
    };
    // Every grant the file makes, in its own words: what the owner's yes is about to put in
    // force, whatever was stored before. What changed against the store is said after the
    // `approved` line, from the records the reply carries back.
    let name = file.name();
    println!("  {name} approves:");
    for section in file.sections() {
        println!("    {}:", section.name);
        for allow in &section.allows {
            println!("      {}", say_grant(allow));
        }
    }
    if flags.within.is_none() && !owner_says_yes("approve these delegations?") {
        println!("  not approved; nothing was stored");
        return ExitCode::FAILURE;
    }
    // The records travel as frames, the digest in their header binding them to these bytes.
    let digest = package_archive::sha256(text.as_bytes());
    let digest_text = measured_boot::digest_text(&digest);
    let hex: String = digest_text.iter().map(|b| char::from(*b)).collect();
    let mut out = vec![0u8; 4096];
    let n = match approval_file::compile(&file, digest, &mut out) {
        Ok(n) => n,
        Err(e) => {
            return refuse(&format!("the approval file cannot be compiled: {e:?}"));
        }
    };
    let mut staged = match Staged::new(n as u64) {
        Ok(s) => s,
        Err(why) => return refuse(why),
    };
    staged.bytes().copy_from_slice(&out[..n]);
    let reply = ask(Activation::Approve, Some(&staged), None, &[]);
    let stored = reply.stored.clone();
    let exit = answer(
        Activation::Approve,
        false,
        reply,
        None,
        &Said {
            named: Some(name),
            digest: Some(&hex),
            ..Said::default()
        },
        flags.json,
    );
    if exit == ExitCode::SUCCESS && !flags.json {
        if let Some(stored) = stored {
            say_changed(&file, &stored);
        }
    }
    exit
}

/// **What a re-approval changed against the stored records** (milestone 911): the store's
/// document parsed, and each grant this file adds or drops beside it, said in the ask's own
/// words. A store that does not parse is not one this file can be compared against, and the
/// line says so.
fn say_changed(file: &approval_file::File<'_>, stored: &str) {
    let Ok(stored) = installed_programs::approval::Document::parse(stored) else {
        println!(
            "  the stored records cannot be read; jig revoke drops them, and jig approve stores \
             them again"
        );
        return;
    };
    let name = file.name();
    println!("  {name}, changed since the stored approval:");
    let mut any = false;
    for section in file.sections() {
        let old = stored.entry(section.name).ok().flatten();
        let mut lines: Vec<String> = Vec::new();
        for allow in &section.allows {
            let held = old
                .as_ref()
                .map(|stored| stored.allows().any(|g| g.as_ref() == Ok(allow)))
                .unwrap_or(false);
            if !held {
                lines.push(say_grant(allow));
            }
        }
        // The grants the stored records had that this file drops, if any: the same words,
        // named as gone.
        for allow in old.iter().flat_map(|stored| stored.allows()).flatten() {
            if !section.allows.iter().any(|a| a == &allow) {
                lines.push(format!("no longer: {}", say_grant(&allow)));
            }
        }
        if lines.is_empty() {
            continue;
        }
        any = true;
        println!("    {}:", section.name);
        for line in lines {
            println!("      {line}");
        }
    }
    if !any {
        println!("    nothing; the records are the stored ones");
    }
}

/// **`jig revoke <name>`**: the progenitor drops the named approval, and every later request
/// under a badge that carries it is answered `Revoked` (milestone 911).
fn revoke(words: &[&str]) -> ExitCode {
    let Some(flags) = parse_flags(words) else {
        println!("{USAGE}");
        return ExitCode::from(2);
    };
    let Some(name) = flags.operand else {
        println!("{USAGE}");
        return ExitCode::from(2);
    };
    if !filesystem_contract::grant::fits_words(name.as_bytes()) {
        println!("{USAGE}");
        return ExitCode::from(2);
    }
    if !holds_installer() {
        return ExitCode::FAILURE;
    }
    answer(
        Activation::Revoke,
        false,
        ask(Activation::Revoke, None, Some(name), &[]),
        None,
        &Said {
            named: Some(name),
            ..Said::default()
        },
        flags.json,
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
/// afterwards, because after a refusal that is still the fact a person needs. Under `--json` the
/// same answer is one object, the verb and the generation (milestone 911, item 7), a
/// `NotApproved` refusal names the package and the ask as data ([`json_refused`]'s `extra`), and
/// the `approved` and `revoked` lines name what the verb acted by, which the progenitor's answer
/// does not carry back.
///
/// **What the request acted by and on**, for those lines (milestone 911). The fields a verb does
/// not use are left at their defaults.
#[derive(Default)]
struct Said<'a> {
    /// The approval a delegated line acts within, from `--within`, which the delegated
    /// refusals name.
    within: Option<&'a str>,
    /// The approval file a scripted line acted by, its `approval` key, which the
    /// progenitor-check refusals name.
    approved: Option<&'a str>,
    /// The package the install carried, whose name the ask refusal says.
    package: Option<&'a str>,
    /// The name an `Approve` stored or a `Revoke` dropped, for the Done line.
    named: Option<&'a str>,
    /// The digest's hex an `Approve` bound, for the `approved` line's `sha256:`.
    digest: Option<&'a str>,
    /// The version the install's package carried, for the ask refusal's naming.
    version: Option<&'a str>,
}

fn answer(
    verb: Activation,
    fetched: bool,
    reply: Reply,
    program: Option<&str>,
    said: &Said<'_>,
    json: bool,
) -> ExitCode {
    let Reply {
        status,
        live,
        holder,
        ask,
        ..
    } = reply;
    let program = program.map_or("one of its programs".to_string(), |p| {
        format!("the program {p}")
    });
    let (said_text, extra) = answer_words(verb, fetched, status, &holder, ask, said, &program);
    if json {
        if status == S::Done {
            json_result(json_verb(verb), live);
        } else {
            json_refused(&extra, &said_text);
        }
    } else if live == 0 {
        println!("  {said_text}; nothing is installed");
    } else {
        println!("  {said_text}; generation {live} is live");
    }
    if status == S::Done {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// **The words one answer earns**, and the grants a refusal names as data: the line for `status`,
/// and for `--json` one `extra` entry per grant an ask names (milestone 911, item 7).
///
/// The ask an `--approved` install's check refused is the reply's third message. `ask` holds its
/// words, which this decodes into the kind, the directory rights and the zone's name, spelled the
/// way the ask names them.
fn answer_words(
    verb: Activation,
    fetched: bool,
    status: S,
    holder: &str,
    ask: Option<[u64; 3]>,
    said: &Said<'_>,
    program: &str,
) -> (String, Vec<String>) {
    let mut extra = Vec::new();
    let mut zone = [0u8; 16];
    let decoded = ask
        .as_ref()
        .and_then(|&[word, lo, hi]| spawnproto::refused_ask_from_words((word, lo, hi), &mut zone));
    let said_text = match (status, verb) {
        (S::Done, Activation::Install) if fetched => "fetched and installed".to_string(),
        (S::Done, Activation::Install) => "installed".to_string(),
        (S::Done, Activation::Remove) => "removed".to_string(),
        (S::Done, Activation::Approve) => format!(
            "approved {} (sha256:{})",
            said.named.unwrap_or(""),
            said.digest.unwrap_or("")
        ),
        (S::Done, Activation::Revoke) => format!("revoked {}", said.named.unwrap_or("")),
        (S::Done, _) => "rolled back".to_string(),
        (S::NotCataloged, _) => {
            "refused: neither this image's catalog nor its index copy vouches for those bytes"
                .to_string()
        }
        (S::NoProgram, _) => {
            "refused: that package carries no program jig can install: none named after it or \
             under bin/, or a bin/ name that is empty, nested or repeated"
                .to_string()
        }
        (S::NotInstalled, _) => {
            "refused: no package of that name is in the live generation".to_string()
        }
        (S::NoEarlier, _) => "refused: there is no generation before the live one".to_string(),
        (S::StoreFailed, _) => "could not write the installed programs".to_string(),
        // calef, 2026-10-14 (UTC): "If two packages offer the same program name, refuse the
        // second package. Need to remove the first one before installing the second one."
        (S::NameTaken, _) => format!(
            "refused: the package {holder} already provides {program}; \
             jig remove {holder} first"
        ),
        (S::ImageName, _) => {
            format!("refused: the image carries {program}; a new base updates it, not install")
        }
        (S::Ambiguous, _) => {
            "refused: that version holds a program's default and several other versions of it are \
             live, so none can take its place"
                .to_string()
        }
        (S::NotServedHere, _) => {
            "refused: the installer endpoint does not take that request; a delegated badge may \
             neither approve nor revoke"
                .to_string()
        }
        (S::NotYours, _) => {
            "refused: another package manager installed that, and it is not jig's to undo"
                .to_string()
        }
        // The delegated check's refusals (milestone 911): the approval does not allow this
        // package, named in the reply's follower messages, or the approval is not stored, which
        // is a revoke's doing. The owner's own `--approved` install is refused with the ask
        // named as the third message, which the owner may read whole.
        (S::NotApproved, _) => match (decoded, said.approved, said.package) {
            (Some((kind, rights, name)), Some(approval), Some(package)) => {
                let what = ask_of(kind, rights, name);
                extra.push(json_grant(&what));
                format!(
                    "refused: {package} {} asks for {}, which {approval} does not allow",
                    said.version.unwrap_or(""),
                    say_grant(&what)
                )
            }
            _ => match said.within.or(said.approved) {
                Some(name) => format!("refused: approval {name} does not allow {holder}"),
                None => {
                    format!("refused: the approval on this endpoint does not allow {holder}")
                }
            },
        },
        (S::Revoked, _) => match (said.within, said.approved) {
            (Some(name), _) => {
                format!("refused: approval {name} is not stored; jig approve stores it")
            }
            (None, Some(name)) => format!(
                "refused: approval {name} is not stored as this file's records; jig approve \
                 stores it"
            ),
            (None, None) => "refused: the approval on this endpoint is not stored".to_string(),
        },
        (S::NotExecutable | S::Unknown, _) => {
            "the progenitor could not take that request".to_string()
        }
    };
    (said_text, extra)
}

/// **The ask the reply's third message named**, as a [`Granted`] for the wording and the data:
/// the kinds and the rights bit spelling [`spawnproto::refused_ask_words`] keeps.
fn ask_of(kind: u64, rights: u64, name: &str) -> installed_programs::Granted<'_> {
    use installed_programs::{DirectoryRights, Granted};
    match kind {
        spawnproto::ASK_SERVICE => Granted::Service,
        spawnproto::ASK_INSTALLER => Granted::Installer,
        spawnproto::ASK_ZONE => Granted::Zone(name),
        spawnproto::ASK_DIRECTORY => Granted::Directory(
            name,
            if rights >= 3 {
                DirectoryRights::RW
            } else {
                DirectoryRights::R
            },
        ),
        _ => Granted::Network,
    }
}

/// The verb, as the `--json` result names it.
fn json_verb(verb: Activation) -> &'static str {
    match verb {
        Activation::Install => "installed",
        Activation::Remove => "removed",
        Activation::Rollback => "rolled back",
        Activation::Approve => "approved",
        Activation::Revoke => "revoked",
        Activation::Vouch => "vouched",
    }
}
