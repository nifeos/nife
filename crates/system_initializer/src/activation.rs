//! **The installer's half of the spawn service**: an edit to the installed-programs table, asked on
//! the spawn endpoint by the owner's console or on an installer endpoint by a package manager
//! (DECISIONS §208 (installing a package is granting it, and the installed programs are
//! versioned), §270 (a package manager holds an installer endpoint, not the spawn endpoint)). The
//! spawn loop calls in. See notes/packages.md.

use super::*;

/// **Serve one request that arrived on the installer endpoint of the job labeled `label`**
/// (milestone 809 (the package client becomes a program)): install, remove or rollback through
/// [`activate`], recorded under the holder's name; anything else answered
/// `ActivationStatus::NotServedHere` with nothing read past its first message. The answer goes to
/// the holder's own reply endpoint, unless the holder died mid-request. Out of line, for
/// [`activate`]'s reason: the spawn loop's frame is under every request.
#[inline(never)]
pub(super) fn serve_installer(
    label: u64,
    (w0, w1, w2): (u64, u64, u64),
    spawn: &mut SpawnEndpoint,
    (own_ut, images_ut, fs, catalog): (u64, u64, Option<Fs>, &'static str),
    fs_mapped: &mut bool,
) {
    let Some((reply, manager)) = spawn.reply_for(label) else {
        // A holder this process has no record of: its reap came first. Nobody to answer.
        return;
    };
    let answer = match spawnproto::activation(w1, w2) {
        Some(Some(verb)) if spawnproto::installer_serves(verb) => activate(
            Some(verb),
            w0,
            spawn,
            &Activating {
                own_ut,
                images_ut,
                fs,
                catalog,
                manager,
            },
            fs_mapped,
        ),
        _ => Answer::from((spawnproto::ActivationStatus::NotServedHere, 0)),
    };
    if !spawn.abandoned {
        let (r0, r1, r2) = spawnproto::activation_reply(answer.status, answer.live, answer.member);
        send(reply, r0, r1, r2);
        // A name taken by another package is refused with that package's name, which `jig` reads
        // after the reply (milestone 919 (a package carries several programs)).
        if answer.status == spawnproto::ActivationStatus::NameTaken {
            for (a, b, c) in spawnproto::holder_words(answer.holder()) {
                send(reply, a, b, c);
            }
        }
    }
}

/// **What [`activate`] answers**: the status, the generation live afterwards, and for a refused
/// install the member whose program was refused (`spawnproto::activation_reply`'s word 2) and,
/// for `NameTaken`, the package that holds the name. Name: provisional, milestone 919's lane.
pub(super) struct Answer {
    pub(super) status: spawnproto::ActivationStatus,
    pub(super) live: u32,
    pub(super) member: u64,
    holder: [u8; spawnproto::HOLDER_MAX],
    holder_len: usize,
}

impl Answer {
    /// The holder's name, as [`Answer::refused`] copied it, or `""`.
    fn holder(&self) -> &str {
        core::str::from_utf8(&self.holder[..self.holder_len]).unwrap_or("")
    }

    /// An install `installed_programs::with_entries` refused, said with the program's member index
    /// in `got` and the holder's name.
    fn refused(
        refused: &installed_programs::Refused,
        got: &package_archive::Installable,
        live: u32,
    ) -> Self {
        use spawnproto::ActivationStatus as S;
        let status = match refused.error {
            installed_programs::Error::ImageName => S::ImageName,
            installed_programs::Error::Taken => S::NameTaken,
            _ => S::StoreFailed,
        };
        let member = got
            .programs()
            .find(|p| Some(p.name) == refused.program)
            .map_or(0, |p| p.member as u64 + 1);
        let mut answer = Answer {
            status,
            live,
            member,
            holder: [0; spawnproto::HOLDER_MAX],
            holder_len: 0,
        };
        if let Some(holder) = refused.holder {
            let len = holder.len().min(spawnproto::HOLDER_MAX);
            answer.holder[..len].copy_from_slice(&holder.as_bytes()[..len]);
            answer.holder_len = len;
        }
        answer
    }
}

impl From<(spawnproto::ActivationStatus, u32)> for Answer {
    fn from((status, live): (spawnproto::ActivationStatus, u32)) -> Self {
        Answer {
            status,
            live,
            member: 0,
            holder: [0; spawnproto::HOLDER_MAX],
            holder_len: 0,
        }
    }
}

/// **A package manager's two grants, made in the job's region `region`** (milestone 809): the
/// installer endpoint and its reply endpoint when `installer` (the request came from the owner's
/// console and the manifest asked), and a copy of the catalog's `lines` when `catalog`. The
/// endpoint is a copy of `spawn_ep` badged with the job's `label`; the reply endpoint is made from
/// the region, so the reap frees it. Either half of the installer failing yields neither half. Out
/// of line, for [`activate`]'s reason.
#[inline(never)]
pub(super) fn package_manager_parts(
    region: u64,
    (installer, catalog): (bool, bool),
    (spawn_ep, label): (u64, u64),
    own_ut: u64,
    lines: &str,
) -> (Option<(u64, u64)>, Option<u64>) {
    let endpoints = if installer {
        let reply = retype_obj(region, abi::objtype::RENDEZVOUS).ok();
        let inst = badged(spawn_ep, spawnproto::installer_badge(label)).ok();
        match (inst, reply) {
            (Some(i), Some(rp)) => Some((i, rp)),
            (i, rp) => {
                for c in [i, rp].into_iter().flatten() {
                    cap_delete(c);
                }
                None
            }
        }
    } else {
        None
    };
    let page = if catalog {
        copy_text(own_ut, region, lines)
    } else {
        None
    };
    (endpoints, page)
}

/// **The image's package catalog, copied into a fresh page of the child's own region**, for a
/// child that declares `grant_plan::Manifest::catalog` (milestone 809 (the package client becomes
/// a program)), and mapped read-only at `std_runtime_contract::CATALOG_PAGE`. The rest of the page
/// is zero, which is where the text ends. `None` if the catalog does not fit in a page or the page
/// could not be made; the spawn is then refused rather than started with half a catalog.
///
/// The image's index sources travel the same way since milestone 801 (packages over the
/// internet), to `std_runtime_contract::INDEX_SOURCES_PAGE`.
pub(super) fn copy_text(own_ut: u64, region: u64, catalog: &str) -> Option<u64> {
    if catalog.len() >= spawnproto::IMAGE_PAGE as usize {
        return None;
    }
    let page = retype_page_frame(region).ok()?;
    let Ok(ours) = supervision_protocol::map_scratch(page, true, own_ut) else {
        cap_delete(page);
        return None;
    };
    // SAFETY: `ours` is a fresh page mapped read/write just above, and the catalog is shorter than
    // a page. A retyped page is zeroed, so the bytes after the text are zero.
    unsafe {
        core::ptr::copy_nonoverlapping(catalog.as_ptr(), ours as *mut u8, catalog.len());
    }
    supervision_protocol::give_up_own_page(ours);
    Some(page)
}

/// What [`activate`] needs from the spawn service, named so the call site says it.
pub(super) struct Activating {
    pub(super) own_ut: u64,
    /// [`IMAGE_POOL_PAGES`]: a package is staged where an image is.
    pub(super) images_ut: u64,
    pub(super) fs: Option<Fs>,
    pub(super) catalog: &'static str,
    /// **Who is asking, as a row's manager column records it** (milestone 809 (the package client
    /// becomes a program)): `installed_programs::OWNER` on the spawn endpoint, the holder's program
    /// name on an installer endpoint.
    pub(super) manager: &'static str,
}

/// **Serve one activation request** (DECISIONS §208): install, remove, rollback or vouch, each
/// ending in one [`FsCalls::commit`], the rename of `current`. Returns the status and the
/// generation live afterwards. An install stages the package as an image is staged
/// ([`take_frames`], [`stage_frames`]) and places each program at
/// `packages/<name>/<version>/<program>`; a remove leaves the bytes for a rollback, and nothing
/// collects them; a vouch records this process's hash of the staged executable as
/// `installed_programs::OWNER`. Vouch is served only on the spawn endpoint, the owner's console
/// (§221 (the boot prompt is the owner's console)). Out of line, for [`build_grant`]'s reason: it
/// holds a page-sized buffer with [`edit`]'s beneath it, and the spawn loop calls it from two
/// places. See notes/packages.md.
#[inline(never)]
pub(super) fn activate(
    verb: Option<spawnproto::Activation>,
    w0: u64,
    spawn: &mut SpawnEndpoint,
    a: &Activating,
    fs_mapped: &mut bool,
) -> Answer {
    use spawnproto::{Activation, ActivationStatus as S};
    // The request's own trailing messages come off the endpoint first, whatever happens next, so a
    // refusal never leaves words behind that the next request would read as its own. The frames
    // are copied only once the name after them is read too ([`take_frames`]).
    let frames = match verb {
        Some(Activation::Install | Activation::Vouch) => Some(take_frames(spawn, w0, a.own_ut)),
        _ => None,
    };
    let mut named = [0u8; filesystem_contract::grant::MAX_NAME];
    // A vouch's name follows its frames, which `take_frames` has just taken.
    let named_len = if matches!(verb, Some(Activation::Remove | Activation::Vouch)) {
        let (lo, hi, len) = spawn.receive();
        filesystem_contract::grant::unpack_name(lo, hi, len as usize, &mut named)
    } else {
        0
    };
    let staging = frames
        .and_then(|f| stage_frames(spawn, &f, w0, a.own_ut, a.images_ut, true).map(|st| (st, w0)));
    let named = &named[..named_len];

    let outcome = (|| {
        let Some(verb) = verb else {
            return Answer::from((S::Unknown, 0));
        };
        // An installer holder that died mid-request sent nothing more, and is owed no answer.
        if spawn.abandoned {
            return Answer::from((S::Unknown, 0));
        }
        let Some(files) = FsCalls::map(a.fs, a.own_ut, fs_mapped) else {
            return Answer::from((S::StoreFailed, 0));
        };
        let act = files.directory(fs_operation::ROOT, installed_programs::DIRECTORY);
        if act < 0 {
            return Answer::from((S::StoreFailed, 0));
        }
        let act_h = act as u64;
        let mut old = [0u8; PAGE_BYTES];
        let answer = match files.live_generation(act_h, &mut old) {
            Err(()) => Answer::from((S::StoreFailed, 0)),
            // A live table that is not text is a table nothing can edit, the same answer as one
            // that cannot be read.
            Ok((live, n)) => match core::str::from_utf8(&old[..n]) {
                Ok(table) => edit(
                    &files, act_h, verb, live, table, staging, a.catalog, a.manager, named,
                ),
                Err(_) => Answer::from((S::StoreFailed, live)),
            },
        };
        files.close(act);
        answer
    })();

    // The staged package goes back to the image pool, whose most recent carve it is.
    if let Some((st, _)) = staging {
        supervision_protocol::memory_region_destroy(st);
        cap_delete(st);
    }
    outcome
}

/// The part of [`activate`] that decides and writes, with the live generation already read.
#[allow(clippy::too_many_arguments)]
fn edit(
    files: &FsCalls,
    act: u64,
    verb: spawnproto::Activation,
    live: u32,
    table: &str,
    staging: Option<(u64, u64)>,
    catalog: &str,
    manager: &str,
    named: &[u8],
) -> Answer {
    use spawnproto::Activation;
    let cx = EditContext {
        files,
        act,
        live,
        table,
        manager,
    };
    let mut new = [0u8; PAGE_BYTES];
    match verb {
        Activation::Rollback => rollback(&cx, &mut new).into(),
        Activation::Remove => remove(&cx, named, &mut new).into(),
        // **The owner's vouch** (DECISIONS §221 ruling 1; §195 clause 3). The digest is this
        // process's hash of its own copy, the one a spawn of the same file computes, so the entry
        // is found by the next run of those bytes and by nothing else.
        Activation::Vouch => vouch(&cx, staging, named, &mut new).into(),
        Activation::Install => install(&cx, staging, catalog, &mut new),
    }
}

/// What every verb of [`edit`] reads: the store, the activation directory's handle, the live
/// generation and its table, and who is asking.
#[derive(Clone, Copy)]
struct EditContext<'a> {
    files: &'a FsCalls,
    act: u64,
    live: u32,
    table: &'a str,
    manager: &'a str,
}

impl EditContext<'_> {
    /// The generation a writing verb commits: [`edit`]'s next free number.
    fn next(&self) -> u32 {
        // The next generation's number: the first one above the live generation with no file. After a
        // rollback the numbers above the live one are taken, and a generation is never rewritten.
        let mut m = self.live + 1;
        while self.files.generation_exists(self.act, m) && m < u32::MAX {
            m += 1;
        }
        m
    }
}

/// **Rollback**: make the generation one below the live one current, if this manager may undo
/// every row that differs.
fn rollback(cx: &EditContext, new: &mut [u8; PAGE_BYTES]) -> (spawnproto::ActivationStatus, u32) {
    use spawnproto::ActivationStatus as S;
    let EditContext {
        files,
        act,
        live,
        table,
        manager,
    } = *cx;
    if live <= 1 {
        return (S::NoEarlier, live);
    }
    let back = live - 1;
    // A manager may undo only its own rows and the owner's (`installed_programs::foreign_change`).
    // The generation below is read into `new`, which a rollback never writes.
    let Ok(n) = files.generation(act, back, new) else {
        return (S::StoreFailed, live);
    };
    let Ok(older) = core::str::from_utf8(&new[..n]) else {
        return (S::StoreFailed, live);
    };
    match installed_programs::foreign_change(older, table, manager) {
        Ok(None) => {}
        Ok(Some(_)) => return (S::NotYours, live),
        Err(_) => return (S::StoreFailed, live),
    }
    if !files.commit(act, back, None) {
        return (S::StoreFailed, live);
    }
    (S::Done, back)
}

/// **Remove**: write a new generation without the named package, or without one version of it.
fn remove(
    cx: &EditContext,
    named: &[u8],
    new: &mut [u8; PAGE_BYTES],
) -> (spawnproto::ActivationStatus, u32) {
    use spawnproto::ActivationStatus as S;
    let EditContext {
        files,
        act,
        live,
        table,
        manager,
    } = *cx;
    let Ok(operand) = core::str::from_utf8(named) else {
        return (S::NotInstalled, live);
    };
    // The operand is a package or `package@version` (milestone 919). `@` never appears in a
    // package name, so the two forms cannot be mistaken for each other.
    let (package, version) = match operand.split_once('@') {
        Some((p, v)) if !p.is_empty() && !v.is_empty() => (p, Some(v)),
        _ => (operand, None),
    };
    // Another manager's rows are not this one's to remove: checked over every row the edit would
    // take, before anything is written.
    let foreign = installed_programs::entries(table).flatten().any(|e| {
        e.package == package
            && e.package != installed_programs::OWNER
            && version.is_none_or(|v| e.version == v)
            && !installed_programs::can_edit(e.manager, manager)
    });
    if foreign {
        return (S::NotYours, live);
    }
    let n = match installed_programs::without_package(table, package, version, new) {
        Ok(n) => n,
        Err(installed_programs::Error::NotInstalled) => return (S::NotInstalled, live),
        Err(installed_programs::Error::SeveralVersions) => return (S::Ambiguous, live),
        Err(_) => return (S::StoreFailed, live),
    };
    let m = cx.next();
    if !files.commit(act, m, Some(&new[..n])) {
        return (S::StoreFailed, live);
    }
    (S::Done, m)
}

/// **Vouch**: write a new generation recording this process's hash of the staged executable under
/// the name that followed it, marked `installed_programs::OWNER`.
fn vouch(
    cx: &EditContext,
    staging: Option<(u64, u64)>,
    named: &[u8],
    new: &mut [u8; PAGE_BYTES],
) -> (spawnproto::ActivationStatus, u32) {
    use spawnproto::ActivationStatus as S;
    let EditContext {
        files,
        act,
        live,
        table,
        manager,
    } = *cx;
    let Some((_, len)) = staging else {
        return (S::Unknown, live);
    };
    let bytes = staged_image(len);
    let Ok(program) = core::str::from_utf8(named) else {
        return (S::NotExecutable, live);
    };
    if elf::Elf::parse(bytes).is_err() {
        return (S::NotExecutable, live);
    }
    let entry = installed_programs::Entry {
        program,
        // The owner vouches for bytes, not a version (§221 ruling 1), so no pointer names the row.
        version: installed_programs::NO_VERSION,
        package: installed_programs::OWNER,
        digest: measured_boot::sha256(bytes),
        manager,
    };
    // A vouch claims no name, so whether the image carries one does not matter to it.
    let n = match installed_programs::with_entry(table, &entry, false, new) {
        Ok(n) => n,
        Err(installed_programs::Error::BadName) => return (S::NotExecutable, live),
        Err(_) => return (S::StoreFailed, live),
    };
    let m = cx.next();
    if !files.commit(act, m, Some(&new[..n])) {
        return (S::StoreFailed, live);
    }
    (S::Done, m)
}

/// **Install**: check the staged package against the image's catalog, place every program it
/// carries under `packages/`, and write one new generation recording each program's digest
/// (milestone 919 (a package carries several programs)).
fn install(
    cx: &EditContext,
    staging: Option<(u64, u64)>,
    catalog: &str,
    new: &mut [u8; PAGE_BYTES],
) -> Answer {
    use package_archive::Refusal;
    use spawnproto::ActivationStatus as S;
    let EditContext {
        files,
        act,
        live,
        table,
        manager,
    } = *cx;
    let Some((_, len)) = staging else {
        return Answer::from((S::Unknown, live));
    };
    let bytes = staged_image(len);
    // Whoever fetched it checked it was the package asked for. What decides here is whether the
    // image vouches for these bytes, or else the copy of the index `jig update` wrote.
    let no_program = Answer::from((S::NoProgram, live));
    let got = match package_archive::installable(catalog, bytes) {
        Ok(got) => got,
        Err(Refusal::NoProgram | Refusal::ProgramName) => return no_program,
        Err(_) => match indexed(files, act, bytes, new) {
            Ok(got) => got,
            Err(Refusal::NoProgram | Refusal::ProgramName) => return no_program,
            Err(_) => return Answer::from((S::NotCataloged, live)),
        },
    };
    let package = got.package();
    // Every program's bytes are placed before any generation names them. A refusal below leaves
    // them under `packages/`, where they still run by path, and names none of them.
    if !got.programs().all(|p| place_program(files, &package, &p)) {
        return Answer::from((S::StoreFailed, live));
    }
    // The row is keyed on the digest; name and version are labels (ruling 2 of milestone 614 (two
    // installed versions of one program, each runnable)). No row carries the architecture.
    let rows = || {
        got.programs().map(|p| installed_programs::Entry {
            program: p.name,
            version: package.version(),
            package: package.name(),
            digest: p.digest,
            manager,
        })
    };
    // A bare name belongs to one package, and never to one the image carries (§229 (how a bare
    // name at the prompt reaches an installed program) B2; milestone 919). One program refused
    // refuses the package. A base program updates through the boot slot (§235 (the OS is built
    // and updated from packages)), so this blocks no update; the prompt still refuses a name that
    // is both, because a later base can add a name a package holds.
    let image = |program: &str| Program::from_name(program.as_bytes()).is_some();
    // The owner's grants, all or nothing: what each program's note asks of the network and of
    // resolver zones (milestone 892 (a std program at the prompt holds the network and a
    // resolver)). `jig install` lists these asks and sends nothing without the owner's yes.
    let granted = || got.programs().flat_map(asked_grants);
    let n = match installed_programs::with_entries_granting(table, rows, image, granted, new) {
        Ok(n) => n,
        Err(refused) => return Answer::refused(&refused, &got, live),
    };
    let m = cx.next();
    if !files.commit(act, m, Some(&new[..n])) {
        return Answer::from((S::StoreFailed, live));
    }
    Answer::from((S::Done, m))
}

/// **What one program's note asks the owner for**: the network, if it asks, then each resolver zone,
/// keyed on the program's digest. Nothing for a program whose bytes or note cannot be read, which
/// runs with none of them either (`endowed_image` refuses it).
fn asked_grants<'b>(
    p: package_archive::Program<'b>,
) -> impl Iterator<Item = installed_programs::Grant<'b>> + use<'b> {
    use installed_programs::{Grant, Granted};
    let digest = p.digest;
    let note = elf::Elf::parse(p.bytes).ok().and_then(|elf| {
        elf.note(
            capability_manifest_note::OWNER,
            capability_manifest_note::MANIFEST,
        )
        .ok()
        .flatten()
    });
    let decoded = note.and_then(|d| {
        Some((
            capability_manifest_note::decode(d).ok()?,
            capability_manifest_note::zones(d).ok()?,
        ))
    });
    let network = decoded.is_some_and(|(m, _)| m.network).then_some(Grant {
        digest,
        what: Granted::Network,
    });
    let zones = decoded.into_iter().flat_map(move |(_, zones)| {
        zones.iter().filter_map(move |z| {
            Some(Grant {
                digest,
                what: Granted::Zone(core::str::from_utf8(z).ok()?),
            })
        })
    });
    network.into_iter().chain(zones)
}

/// **Whether the copy of the image's index vouches for these bytes** (milestone 801 (packages
/// over the internet), ruling I2 of 2026-10-06: the progenitor trusts the copy in a directory only
/// the owner's console grants). The copy is read into `scratch`, the page the new generation is
/// written into afterwards; nothing borrowed from it outlives this call. Its entry for the
/// package's stem decides by `package_index::accept`, which is `package_archive::installable_as`
/// with the entry as a one-line catalog, so the decision is the image catalog's rule on another
/// line. A copy that does not read whole, or names the stem twice with two answers, vouches for
/// nothing.
fn indexed<'b>(
    files: &FsCalls,
    act: u64,
    bytes: &'b [u8],
    scratch: &mut [u8; PAGE_BYTES],
) -> Result<package_archive::Installable<'b>, package_archive::Refusal> {
    use package_archive::Refusal;
    let package = package_archive::Package::parse(bytes).map_err(|_| Refusal::Unreadable)?;
    let mut stem = [0u8; package_archive::STEM_LEN];
    let stem = package.stem(&mut stem);
    let n = files
        .index_copy(act, scratch)
        .map_err(|()| Refusal::NotCataloged)?;
    let text = core::str::from_utf8(&scratch[..n]).map_err(|_| Refusal::NotCataloged)?;
    let index = package_index::Index::parse(text).map_err(|_| Refusal::NotCataloged)?;
    let mut matching = index.entries().filter(|e| e.stem == stem);
    let entry = matching.next().ok_or(Refusal::NotCataloged)?;
    if matching.any(|e| e != entry) {
        return Err(Refusal::NotCataloged);
    }
    package_index::accept(&entry, bytes)
}

/// Write one of an installable package's programs to `packages/<name>/<version>/<program>`,
/// closing every handle opened on the way. `false` if any step failed.
fn place_program(
    files: &FsCalls,
    package: &package_archive::Package,
    got: &package_archive::Program,
) -> bool {
    // The program's bytes, where a person can run them (DECISIONS §219 option D hashes
    // whatever they run, so where they live is a convenience, not a trust decision).
    let packages = files.directory(fs_operation::ROOT, installed_programs::PACKAGES);
    if packages < 0 {
        return false;
    }
    // `packages/<name>/<version>/<program>`: one component per field, because a prompt
    // component is at most sixteen bytes and a stem is longer. The architecture is this
    // machine's; the generation's row does not carry it, because the digest is of
    // target-specific bytes and two ISAs never collide in one table.
    let name = files.directory(packages as u64, package.name());
    let version = if name >= 0 {
        files.directory(name as u64, package.version())
    } else {
        -1
    };
    let placed = version >= 0 && {
        let f = files.open_or_create(version as u64, got.name);
        let ok = f >= 0 && files.replace(f as u64, got.bytes);
        if f >= 0 {
            files.close(f);
        }
        ok
    };
    for h in [version, name, packages] {
        if h >= 0 {
            files.close(h);
        }
    }
    placed
}
