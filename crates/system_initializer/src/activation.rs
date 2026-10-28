//! **The installer's half of the spawn service**: an edit to the installed-programs table, asked on
//! the spawn endpoint by the owner's console or on an installer endpoint by a package manager
//! (DECISIONS §208 (installing a package is granting it, and the installed programs are
//! versioned), §270 (a package manager holds an installer endpoint, not the spawn endpoint)). The
//! spawn loop calls in. See notes/packages.md.

use spawnproto::Activation;

use super::*;

/// **Serve one request that arrived on the installer endpoint of the job labeled `label`**
/// (milestone 809 (the package client becomes a program)): install, remove or rollback through
/// [`activate`], recorded under the holder's name; the owner's `Approve` and `Revoke` when the
/// badge is undelegated; anything else answered `ActivationStatus::NotServedHere` with nothing
/// acted on. A **delegated** badge (milestone 911 (scripted installs grant only what the owner's
/// approval file allows)) carries an approval id, and its every request is checked against that
/// approval's stored records before anything is written, or answered `Revoked` when it is not
/// stored. The answer goes to the holder's own reply endpoint, unless the holder died
/// mid-request. Out of line, for [`activate`]'s reason: the spawn loop's frame is under every
/// request.
#[inline(never)]
pub(super) fn serve_installer(
    label: u64,
    approval: Option<u64>,
    (w0, w1, w2): (u64, u64, u64),
    spawn: &mut SpawnEndpoint,
    (own_ut, images_ut, fs, catalog): (u64, u64, Option<Fs>, &'static str),
    fs_mapped: &mut bool,
) {
    let Some((reply, manager)) = spawn.reply_for(label) else {
        // A holder this process has no record of: its reap came first. Nobody to answer.
        return;
    };
    // A delegated badge may neither approve nor revoke (milestone 911: it can neither widen its
    // own approval nor mint another), and a request whose approval is not stored is answered
    // `Revoked` with nothing acted on. The request's own trailing messages come off the endpoint
    // first whatever the answer, so none of them is left for the next one to read as its own.
    let verb = spawnproto::activation(w1, w2);
    // The page the delegated check reads its approval into, and the one an `Approve`'s reply
    // carries the stored records back in: the two never meet, because a delegated badge can
    // send neither an `Approve` nor the check that would read over it.
    let mut text = [0u8; PAGE_BYTES];
    let answer = match (approval, verb) {
        (Some(_), Some(Some(Activation::Approve | Activation::Revoke))) => {
            drain(spawn, verb, w0);
            Answer::from((spawnproto::ActivationStatus::NotServedHere, 0))
        }
        (Some(id), _) => 'delegated: {
            // The file service's calls, mapped once for the read and the activation both
            // (`FsCalls::map` is idempotent in `fs_mapped`).
            let Some(files) = FsCalls::map(fs, own_ut, fs_mapped) else {
                // No file service this boot: nothing is stored and nothing is checked against.
                drain(spawn, verb, w0);
                break 'delegated Answer::from((spawnproto::ActivationStatus::StoreFailed, 0));
            };
            // The approval is read before the request is decided on, into the one page buffer
            // the check keeps (`with_approval`'s doc): the read is done by the time `activate`
            // runs, and the document it returns borrows that page.
            // The buffer lives in this frame, as `activate`'s own page buffers do; the
            // document borrows it and is used before the frame returns.
            let allowed = with_approval(id, spawn, &files, &mut text);
            match (allowed, verb) {
                (None, _) => {
                    drain(spawn, verb, w0);
                    break 'delegated Answer::from((spawnproto::ActivationStatus::Revoked, 0));
                }
                (Some(allowed), _) => match verb {
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
                            allowed: Some(&allowed),
                        },
                        fs_mapped,
                        // The delegated check reads the badge's own approval; the store's
                        // records are not read twice.
                        None,
                    ),
                    _ => {
                        drain(spawn, verb, w0);
                        break 'delegated Answer::from((
                            spawnproto::ActivationStatus::NotServedHere,
                            0,
                        ));
                    }
                },
            }
        }
        _ => match verb {
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
                    allowed: None,
                },
                fs_mapped,
                // The owner's own requests read the store here: an `Approve`'s reply carries
                // the records it replaced, and an install whose trailer binds an approval is
                // checked against them (milestone 911).
                Some(&mut text),
            ),
            _ => {
                drain(spawn, verb, w0);
                Answer::from((spawnproto::ActivationStatus::NotServedHere, 0))
            }
        },
    };
    // **An `Approve` that stored mint its name's id** (milestone 911), so a delegation the
    // console places later, and the standing form 910 will, badge with the same key the store's
    // file is read by.
    if verb == Some(Some(Activation::Approve))
        && answer.status == spawnproto::ActivationStatus::Done
    {
        let _ = spawn.approval_id_for(answer.holder().as_bytes());
    }
    if !spawn.abandoned {
        let (r0, r1, r2) = spawnproto::activation_reply(answer.status, answer.live, answer.member);
        send(reply, r0, r1, r2);
        // A name taken by another package, and a delegated request the approval does not allow,
        // are refused with the package's name, which `jig` reads after the reply (milestone 919
        // (a package carries several programs); milestone 911). The owner's own `--approved`
        // install is refused with the ask that exceeded the entry as the third message
        // (`refused_ask_words`), which the reply's word 2 counts.
        if answer.status == spawnproto::ActivationStatus::NameTaken
            || answer.status == spawnproto::ActivationStatus::NotApproved
        {
            for (a, b, c) in spawnproto::holder_words(answer.holder()) {
                send(reply, a, b, c);
            }
            if answer.member == 3 {
                let [a, b, c] = answer.ask;
                send(reply, a, b, c);
            }
        }
        // **An `Approve`'s reply carries the records it replaced** (milestone 911), so the
        // console's re-approval shows what changed against the store it now holds. The length
        // is the reply's word 2; the bytes are three words a message, tail padded.
        if verb == Some(Some(Activation::Approve))
            && answer.status == spawnproto::ActivationStatus::Done
            && answer.member > 0
        {
            let records = &text[..answer.member as usize];
            for m in 0..spawnproto::stored_messages(answer.member) as usize {
                let (a, b, c) = spawnproto::stored_words(records, m * 24);
                send(reply, a, b, c);
            }
        }
    }
}

/// **The request's own trailing messages, taken and dropped**: what a refusal owes the endpoint
/// before it answers, so no frame, name or answered-by trailer is left for the next request on it
/// to read as its own. The frames are taken, never staged; nothing is copied and nothing is
/// written. An install's or a removal's trailer is one message for the console's answer, three
/// for an approval's.
fn drain(spawn: &mut SpawnEndpoint, verb: Option<Option<Activation>>, w0: u64) {
    match verb {
        Some(Some(Activation::Install | Activation::Vouch)) => {
            take_frames(spawn, w0, 0);
            drain_trailer(spawn);
        }
        Some(Some(Activation::Remove)) => {
            let _ = spawn.receive();
            drain_trailer(spawn);
        }
        Some(Some(Activation::Approve)) => {
            take_frames(spawn, w0, 0);
        }
        Some(Some(Activation::Revoke)) => {
            let _ = spawn.receive();
        }
        _ => {}
    }
}

/// The answered-by trailer, read and dropped: one message, or three when the kind word says an
/// approval sent it. An `Approve`'s records are their own payload; it sends none.
fn drain_trailer(spawn: &mut SpawnEndpoint) {
    let (kind, _, _) = spawn.receive();
    if kind == spawnproto::ANSWERED_APPROVAL {
        let _ = spawn.receive();
        let _ = spawn.receive();
    }
}

/// **The approval an id names, as the store's document**, read per request: the file is the
/// authority, so a revoke's deletion and an approve's replacement are both seen here. `None`
/// when the id names nothing stored, or the file is gone. The document borrows this call's
/// page buffer, so the caller uses it before the next read remaps the page.
fn with_approval<'a>(
    id: u64,
    spawn: &SpawnEndpoint,
    files: &'a FsCalls,
    text: &'a mut [u8; PAGE_BYTES],
) -> Option<installed_programs::approval::Document<'a>> {
    let name = spawn.approval_name(id)?;
    let act = files.directory(fs_operation::ROOT, installed_programs::DIRECTORY);
    if act < 0 {
        return None;
    }
    let document = approval_document(files, act as u64, name, text);
    files.close(act);
    document
}

/// **The store's document for `name`**, read from the activation directory `act` holds: the
/// store file per request, so a revoke's deletion and an approve's replacement are both seen.
/// `None` when the store cannot be opened or the file is gone or does not parse. The document
/// borrows this call's page buffer, so the caller uses it before the next read remaps the page.
fn approval_document<'a>(
    files: &FsCalls,
    act: u64,
    name: &[u8],
    text: &'a mut [u8; PAGE_BYTES],
) -> Option<installed_programs::approval::Document<'a>> {
    let store = files.named(
        fs_operation::OPENDIR,
        act,
        installed_programs::approval::STORE,
        dir::ENUMERATE | dir::READ | dir::DESCEND,
    );
    let mut name_buf = [0u8; 16];
    let answer = if store >= 0 {
        let file = files.named(
            fs_operation::OPEN,
            store as u64,
            name_of(name, &mut name_buf),
            0,
        );
        let n = if file >= 0 {
            files.read(file as u64, text.as_mut_slice())
        } else {
            None
        };
        if file >= 0 {
            files.close(file);
        }
        n.and_then(|n| core::str::from_utf8(&text[..n]).ok())
            .and_then(|text| installed_programs::approval::Document::parse(text).ok())
    } else {
        None
    };
    files.close(store);
    answer
}

/// The store file's name for an approval, one component.
fn name_of<'o>(name: &[u8], out: &'o mut [u8; 16]) -> &'o str {
    let len = name.len().min(out.len());
    out[..len].copy_from_slice(&name[..len]);
    core::str::from_utf8(&out[..len]).unwrap_or("")
}

/// **What [`activate`] answers**: the status, the generation live afterwards, and for a refused
/// install the member whose program was refused (`spawnproto::activation_reply`'s word 2) and,
/// for `NameTaken`, the package that holds the name. Name: provisional, milestone 919's lane.
pub(super) struct Answer {
    pub(super) status: spawnproto::ActivationStatus,
    pub(super) live: u32,
    pub(super) member: u64,
    holder: [u8; spawnproto::HOLDER_MAXIMUM],
    holder_len: usize,
    /// **The ask an undelegated `--approved` install's check refused** (milestone 911), as
    /// [`spawnproto::refused_ask_words`] spelled it, all zero when the refusal names no ask.
    ask: [u64; 3],
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
            holder: [0; spawnproto::HOLDER_MAXIMUM],
            holder_len: 0,
            ask: [0; 3],
        };
        if let Some(holder) = refused.holder {
            let len = holder.len().min(spawnproto::HOLDER_MAXIMUM);
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
            holder: [0; spawnproto::HOLDER_MAXIMUM],
            holder_len: 0,
            ask: [0; 3],
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
    delegated: Option<u64>,
) -> (Option<(u64, u64)>, Option<u64>) {
    let endpoints = if installer {
        let reply = retype_obj(region, abi::objtype::RENDEZVOUS).ok();
        // **The delegated endpoint is the same object, badged for one job and one approval**
        // (milestone 911 (scripted installs grant only what the owner's approval file allows)),
        // never a second one; §270's amendment records the semantics.
        let badge = delegated.map_or(spawnproto::installer_badge(label), |id| {
            spawnproto::delegated_installer_badge(label, id)
        });
        let inst = badged(spawn_ep, badge).ok();
        match (inst, reply) {
            (Some(i), Some(rp)) => Some((i, rp)),
            (i, rp) => {
                for c in [i, rp].into_iter().flatten() {
                    capability_delete(c);
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
    let Ok(ours) = supervision_contract::map_scratch(page, true, own_ut) else {
        capability_delete(page);
        return None;
    };
    // SAFETY: `ours` is a fresh page mapped read/write just above, and the catalog is shorter than
    // a page. A retyped page is zeroed, so the bytes after the text are zero.
    unsafe {
        core::ptr::copy_nonoverlapping(catalog.as_ptr(), ours as *mut u8, catalog.len());
    }
    supervision_contract::give_up_own_page(ours);
    Some(page)
}

/// What [`activate`] needs from the spawn service, named so the call site says it.
pub(super) struct Activating<'a> {
    pub(super) own_ut: u64,
    /// [`IMAGE_POOL_PAGES`]: a package is staged where an image is.
    pub(super) images_ut: u64,
    pub(super) fs: Option<Fs>,
    pub(super) catalog: &'static str,
    /// **Who is asking, as a row's manager column records it** (milestone 809 (the package client
    /// becomes a program)): `installed_programs::OWNER` on the spawn endpoint, the holder's program
    /// name on an installer endpoint.
    pub(super) manager: &'static str,
    /// **The approval a delegated badge carries, as the store's document** (milestone 911 (scripted
    /// installs grant only what the owner's approval file allows)), read per request; the check
    /// against it is in the installer, not in the holder. `None` for the owner's own requests,
    /// which need no check.
    pub(super) allowed: Option<&'a installed_programs::approval::Document<'a>>,
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
    // The page the owner's own requests read the store's records into: an `Approve`'s reply
    // carries the records it replaced, and an install whose trailer binds an approval is checked
    // against them (milestone 911). `None` from a delegated badge, whose own approval is already
    // read, and from the spawn endpoint, whose verbs are the boot shell's.
    store: Option<&mut [u8; PAGE_BYTES]>,
) -> Answer {
    use spawnproto::{Activation, ActivationStatus as S};
    // The request's own trailing messages come off the endpoint first, whatever happens next, so a
    // refusal never leaves words behind that the next request would read as its own. The frames
    // are copied only once the name after them is read too ([`take_frames`]).
    let frames = match verb {
        Some(Activation::Install | Activation::Vouch | Activation::Approve) => {
            Some(take_frames(spawn, w0, a.own_ut))
        }
        _ => None,
    };
    let mut named = [0u8; filesystem_contract::grant::WORDS_NAME];
    // A vouch's name follows its frames, which `take_frames` has just taken; a revoke's names
    // the approval to drop (milestone 911 (scripted installs grant only what the owner's
    // approval file allows)).
    let named_len = if matches!(
        verb,
        Some(Activation::Remove | Activation::Vouch | Activation::Revoke)
    ) {
        let (lo, hi, len) = spawn.receive();
        filesystem_contract::grant::unpack_name(lo, hi, len as usize, &mut named)
    } else {
        0
    };
    // The answered-by trailer, which an install and a removal always send after their data
    // (milestone 911): how the answer was given, the record's own line beside it. A delegated
    // request's badge overrides it with the approval the check ran against, in
    // [`EditContext::answered_of`].
    let mut answered_words = [0u8; 16];
    let answered =
        if matches!(verb, Some(Activation::Install | Activation::Remove)) && !spawn.abandoned {
            let (kind, lo, hi) = spawn.receive();
            let more = if kind == spawnproto::ANSWERED_APPROVAL {
                [spawn.receive(), spawn.receive()]
            } else {
                [(0, 0, 0); 2]
            };
            spawnproto::answered_from_trailer((kind, lo, hi), &more, &mut answered_words)
                .map(|a| match a {
                    spawnproto::Answered::Console => installed_programs::Answered::Console,
                    spawnproto::Answered::Approval { name, len, digest } => {
                        // The name is copied into `answered_words`, which outlives the Answered:
                        // the line's text is the record's `answered` line's own.
                        answered_words[..len].copy_from_slice(&name[..len]);
                        let name = core::str::from_utf8(&answered_words[..len]).unwrap_or("");
                        installed_programs::Answered::Approval(name, digest)
                    }
                })
                .unwrap_or(installed_programs::Answered::Console)
        } else {
            installed_programs::Answered::Console
        };
    let staging = frames
        .and_then(|f| stage_frames(spawn, &f, w0, a.own_ut, a.images_ut, true).map(|st| (st, w0)));
    let named = &named[..named_len];
    // Where the delegated check names the package it refused is `edit`'s cell, read into the
    // Answer when the verb decides (milestone 911).

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
                    a.allowed, &answered, store,
                ),
                Err(_) => Answer::from((S::StoreFailed, live)),
            },
        };
        files.close(act);
        answer
    })();

    // The staged package goes back to the image pool, whose most recent carve it is.
    if let Some((st, _)) = staging {
        supervision_contract::memory_region_destroy(st);
        capability_delete(st);
    }
    outcome
}

/// The part of [`activate`] that decides and writes, with the live generation already read.
/// `allowed` is the delegated badge's approval (milestone 911 (scripted installs grant only what
/// the owner's approval file allows)); its check runs here, in the installer, before anything is
/// written. `answered` is how the answer was given, the trailer's word or the badge's approval.
/// `store` is [`activate`]'s page for the store's records.
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
    allowed: Option<&installed_programs::approval::Document<'_>>,
    answered: &installed_programs::Answered<'_>,
    store: Option<&mut [u8; PAGE_BYTES]>,
) -> Answer {
    use spawnproto::Activation;
    let refused = core::cell::Cell::new(([0u8; spawnproto::HOLDER_MAXIMUM], 0usize));
    let ask = core::cell::Cell::new([0u64; 3]);
    let cx = EditContext {
        files,
        act,
        live,
        table,
        manager,
        allowed,
        answered,
        refused: &refused,
        ask: &ask,
    };
    let mut new = [0u8; PAGE_BYTES];
    let mut answer = match verb {
        Activation::Rollback => rollback(&cx, &mut new).into(),
        Activation::Remove => remove(&cx, named, &mut new).into(),
        // **The owner's vouch** (DECISIONS §221 ruling 1; §195 clause 3). The digest is this
        // process's hash of its own copy, the one a spawn of the same file computes, so the entry
        // is found by the next run of those bytes and by nothing else.
        Activation::Vouch => vouch(&cx, staging, named, &mut new).into(),
        Activation::Install => install(&cx, staging, catalog, store, &mut new),
        // **The owner's approval** (milestone 911): the records are stored beside the
        // generations, never as rows in them, and no generation is written. The spawn endpoint
        // does not take either: the console compiles through `jig`, which sends on its installer
        // endpoint.
        Activation::Approve => store_approval(&cx, staging, named, store),
        Activation::Revoke => drop_approval(&cx, named),
    };
    if answer.status == spawnproto::ActivationStatus::NotApproved {
        // The delegated check's refusal names the package it refused, which the reply's follower
        // carries (milestone 911), as a NameTaken's does; the owner's own `--approved` install
        // carries the ask too, and the reply's word 2 counts the messages.
        let (bytes, len) = cx.refused.get();
        answer.holder[..len].copy_from_slice(&bytes[..len]);
        answer.holder_len = len;
        answer.ask = cx.ask.get();
        answer.member = if answer.ask == [0u64; 3] { 2 } else { 3 };
    }
    answer
}

/// What every verb of [`edit`] reads: the store, the activation directory's handle, the live
/// generation and its table, who is asking, and the delegation (milestone 911).
struct EditContext<'a> {
    files: &'a FsCalls,
    act: u64,
    live: u32,
    table: &'a str,
    manager: &'a str,
    /// The delegated badge's approval, whose check runs here.
    allowed: Option<&'a installed_programs::approval::Document<'a>>,
    /// How the answer was given, the trailer's word or the badge's approval.
    answered: &'a installed_programs::Answered<'a>,
    /// **Where the delegated check's refused package is named** (milestone 911), for the reply's
    /// follower: the caller's cell, read after the verb returns.
    refused: &'a core::cell::Cell<([u8; spawnproto::HOLDER_MAXIMUM], usize)>,
    /// **Where the owner's `--approved` check's refused ask is named** (milestone 911), for the
    /// reply's third follower, as [`spawnproto::refused_ask_words`] spells it; all zero when the
    /// refusal names no ask.
    ask: &'a core::cell::Cell<[u64; 3]>,
}

impl EditContext<'_> {
    /// **The answered line this edit writes**: the badge's approval when the request arrived
    /// under one, else the trailer's word (milestone 911).
    fn answered_of(&self) -> installed_programs::Answered<'_> {
        match self.allowed {
            Some(allowed) => {
                installed_programs::Answered::Approval(allowed.name(), allowed.digest())
            }
            None => *self.answered,
        }
    }
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
    // The verbs read the context by field; the refused package's name is theirs to write.
    let files = cx.files;
    let act = cx.act;
    let live = cx.live;
    let table = cx.table;
    let manager = cx.manager;
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
    // **The delegated check** (milestone 911 (scripted installs grant only what the owner's
    // approval file allows), the ruling of 2026-10-12 on #2031): a rollback under an approval
    // passes when every row the previous generation restores lies within its records, and is
    // refused, naming the row, when one does not. The rows are the target generation's, grants
    // and all; a row whose package the approval does not bound lies outside it. The owner's
    // vouch rows are not the holder's to undo and no approval bounds them, so the check skips
    // them; they are still restored.
    if let Some(allowed) = cx.allowed {
        let within = |row: &installed_programs::Entry<'_>| {
            if row.package == installed_programs::OWNER {
                return true;
            }
            let Ok(Some(entry)) = allowed.entry(row.package) else {
                return false;
            };
            // Every grant the restored generation records for this row's digest, within the
            // entry: asked one at a time, so no list is kept.
            installed_programs::grants(older)
                .flatten()
                .filter(|g| g.digest == row.digest)
                .all(|g| entry.covers(g.what) == Ok(true))
        };
        let outside = installed_programs::entries(older)
            .flatten()
            .find(|row| !within(row));
        if let Some(row) = outside {
            return refused_not_approved(cx, row.package);
        }
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
    // The verbs borrow the context, since the refused package's name is theirs to write.
    let files = cx.files;
    let act = cx.act;
    let live = cx.live;
    let table = cx.table;
    let manager = cx.manager;
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
    // **The delegated check** (milestone 911 (scripted installs grant only what the owner's
    // approval file allows)): a removal passes when the entry says `remove = true`, and a
    // package the approval does not bound is refused before anything is written.
    if let Some(allowed) = cx.allowed {
        let entry = match allowed.entry(package) {
            Ok(Some(entry)) => entry,
            Ok(None) => return refused_not_approved(cx, package),
            Err(_) => return (S::StoreFailed, live),
        };
        if !entry.remove {
            return refused_not_approved(cx, package);
        }
    }
    let n = match installed_programs::without_package(
        table,
        package,
        version,
        &cx.answered_of(),
        new,
    ) {
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
    // The verbs borrow the context, since the refused package's name is theirs to write.
    let files = cx.files;
    let act = cx.act;
    let live = cx.live;
    let table = cx.table;
    let manager = cx.manager;
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
    store: Option<&mut [u8; PAGE_BYTES]>,
    new: &mut [u8; PAGE_BYTES],
) -> Answer {
    use package_archive::Refusal;
    use spawnproto::ActivationStatus as S;
    // The verbs borrow the context, since the refused package's name is theirs to write.
    let files = cx.files;
    let act = cx.act;
    let live = cx.live;
    let table = cx.table;
    let manager = cx.manager;
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
    // The owner's grants, all or nothing: what each program's note asks of the network, the
    // resolver zones and the service (milestone 892 (a std program at the prompt holds the
    // network and a resolver); the service ask is milestone 911's, note version 3). `jig
    // install` lists these asks and sends nothing without the owner's yes.
    let granted = || got.programs().flat_map(asked_grants);
    // **The delegated check** (milestone 911 (scripted installs grant only what the owner's
    // approval file allows)), before anything is placed or written: every program's asks within
    // the package's entry, and the entry's `update` answer when the package is installed
    // already. A package no entry bounds receives nothing.
    if let Some(allowed) = cx.allowed {
        let installed = installed_programs::entries(table)
            .flatten()
            .any(|e| e.package == package.name());
        let entry = match allowed.entry(package.name()) {
            Ok(Some(entry)) => entry,
            Ok(None) => return refused_not_approved(cx, package.name()).into(),
            Err(_) => return Answer::from((S::StoreFailed, live)),
        };
        if installed && !entry.update {
            return refused_not_approved(cx, package.name()).into();
        }
        for p in got.programs() {
            let outside = asked_grants_of(&p).any(|ask| entry.covers(ask) != Ok(true));
            if outside {
                return refused_not_approved(cx, package.name()).into();
            }
        }
    } else if let (Some(text), installed_programs::Answered::Approval(name, digest)) =
        (store, cx.answered)
    {
        // **The owner's own `--approved` install is checked in the installer too** (milestone
        // 911): the trailer names the approval and the digest the records bind, and the store's
        // records are what is in force. A name not stored, or stored at another digest, is
        // answered `Revoked`; an ask the stored entry does not cover is refused with the ask
        // named, which the owner may read. The `update` answer is not enforced here: the owner
        // is acting, and their console's own `jig install` reinstalls without it.
        let Some(stored) = approval_document(files, act, name.as_bytes(), text) else {
            return Answer::from((S::Revoked, live));
        };
        if stored.digest() != *digest {
            return Answer::from((S::Revoked, live));
        }
        let entry = match stored.entry(package.name()) {
            Ok(Some(entry)) => entry,
            Ok(None) => {
                // No entry bounds the package, so its first ask is the one that fails.
                if let Some(p) = got.programs().next()
                    && let Some(ask) = asked_grants_of(&p).next()
                {
                    return refused_with_ask(cx, package.name(), ask).into();
                }
                return refused_not_approved(cx, package.name()).into();
            }
            Err(_) => return Answer::from((S::StoreFailed, live)),
        };
        for p in got.programs() {
            let outside = asked_grants_of(&p).find(|ask| entry.covers(*ask) != Ok(true));
            if let Some(ask) = outside {
                return refused_with_ask(cx, package.name(), ask).into();
            }
        }
    }
    let answered = cx.answered_of();
    let n = match installed_programs::with_entries_granting(
        table,
        rows,
        image,
        granted,
        Some(&answered),
        new,
    ) {
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
    let digest = p.digest;
    asked_grants_of(&p).map(move |what| installed_programs::Grant { digest, what })
}

/// **What one program's note asks for**, as the grants the owner's yes covers: the network,
/// each resolver zone, and the service ask of note version 3 (milestone 911 (scripted installs
/// grant only what the owner's approval file allows)). Nothing for a program whose bytes or note
/// cannot be read, which runs with none of them either (`endowed_image` refuses it).
fn asked_grants_of<'b>(
    p: &package_archive::Program<'b>,
) -> impl Iterator<Item = installed_programs::Granted<'b>> + use<'b> {
    use installed_programs::Granted;
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
            capability_manifest_note::service(d).ok()?,
        ))
    });
    let network = decoded
        .is_some_and(|(m, _, _)| m.network)
        .then_some(Granted::Network);
    let service = decoded
        .is_some_and(|(_, _, s)| s)
        .then_some(Granted::Service);
    let zones = decoded.into_iter().flat_map(move |(_, zones, _)| {
        zones
            .iter()
            .filter_map(|z| Some(Granted::Zone(core::str::from_utf8(z).ok()?)))
    });
    network.into_iter().chain(service).chain(zones)
}

/// **The owner's `Approve`** (milestone 911 (scripted installs grant only what the owner's
/// approval file allows)): the staged records, parsed whole, are stored beside the generations
/// (`approvals/<name>`, made if it is not there), and no generation is written. The name and the
/// digest are the header's, which is what the answered lines of the requests under it carry.
/// The answer carries the approval's name, which is how the caller mints the badge's id for it,
/// and its word 2 is the length of the records that were stored before this one, whose bytes
/// `store` holds for the reply to carry back ([`spawnproto::stored_words`]), 0 when the name was
/// not stored. A store write that fails is [`ActivationStatus::StoreFailed`]; nothing else is
/// changed.
fn store_approval(
    cx: &EditContext,
    staging: Option<(u64, u64)>,
    _named: &[u8],
    store: Option<&mut [u8; PAGE_BYTES]>,
) -> Answer {
    use spawnproto::ActivationStatus as S;
    let EditContext {
        files, act, live, ..
    } = *cx;
    let Some((_, len)) = staging else {
        return Answer::from((S::Unknown, live));
    };
    let bytes = staged_image(len);
    let Ok(text) = core::str::from_utf8(bytes) else {
        return Answer::from((S::NotExecutable, live));
    };
    let Ok(document) = installed_programs::approval::Document::parse(text) else {
        return Answer::from((S::NotExecutable, live));
    };
    let act2 = files.directory(act, installed_programs::approval::STORE);
    if act2 < 0 {
        return Answer::from((S::StoreFailed, live));
    }
    // The records the store held before this one, read for the reply, before the replace takes
    // them: the re-approval's diff is against what was in force, and only the progenitor reads
    // the store (a std holder's namespace root is the directory it was granted).
    let mut replaced = 0usize;
    if let Some(buf) = store {
        let mut name_buf = [0u8; 16];
        let file = files.named(
            fs_operation::OPEN,
            act2 as u64,
            name_of(document.name().as_bytes(), &mut name_buf),
            0,
        );
        if file >= 0 {
            if let Some(n) = files.read(file as u64, buf.as_mut_slice()) {
                replaced = n;
            }
            files.close(file);
        }
    }
    let mut name_buf = [0u8; 16];
    let file = files.open_or_create(
        act2 as u64,
        name_of(document.name().as_bytes(), &mut name_buf),
    );
    let written = if file >= 0 {
        files.replace(file as u64, bytes)
    } else {
        false
    };
    if file >= 0 {
        files.close(file);
    }
    files.close(act2);
    if !written {
        return Answer::from((S::StoreFailed, live));
    }
    let mut answer = Answer::from((S::Done, live));
    let len = document.name().len().min(spawnproto::HOLDER_MAXIMUM);
    answer.holder[..len].copy_from_slice(&document.name().as_bytes()[..len]);
    answer.holder_len = len;
    answer.member = replaced as u64;
    answer
}

/// **The owner's `Revoke`** (milestone 911): the store's file for the named approval is deleted,
/// and no generation is written. Every later request under a badge that carries the name is
/// answered `Revoked`, from the read that no longer finds it. A name nothing stored is still
/// answered `Done`: the state it names holds. The unlink is verified: a store that still opens
/// the name it was asked to drop answers [`ActivationStatus::StoreFailed`], since a `Done` that
/// left the records in force would be a lie.
fn drop_approval(cx: &EditContext, named: &[u8]) -> Answer {
    use spawnproto::ActivationStatus as S;
    let EditContext {
        files, act, live, ..
    } = *cx;
    let Ok(name) = core::str::from_utf8(named) else {
        return Answer::from((S::NotInstalled, live));
    };
    // Removing the store's file: `UNLINK` is `dir::REMOVE`'s answer on the directory that
    // holds it, so the store is opened carrying that right. A store this boot cannot open is
    // the state the request asks for, and the answer is `Done`.
    let act2 = files.named(
        fs_operation::OPENDIR,
        act,
        installed_programs::approval::STORE,
        dir::ENUMERATE | dir::READ | dir::DESCEND | dir::REMOVE,
    );
    if act2 < 0 {
        return Answer::from((S::Done, live));
    }
    let mut name_buf = [0u8; 16];
    let gone = files.named(
        fs_operation::UNLINK,
        act2 as u64,
        name_of(name.as_bytes(), &mut name_buf),
        0,
    );
    files.close(act2);
    if gone < 0 {
        // The unlink was refused: the file may still be there. A store the name cannot be
        // opened from is a store the file is gone from, and `Done` is the answer; one that
        // still opens it is the refusal.
        let act3 = files.named(
            fs_operation::OPENDIR,
            act,
            installed_programs::approval::STORE,
            dir::ENUMERATE | dir::READ | dir::DESCEND,
        );
        if act3 >= 0 {
            let mut name_buf = [0u8; 16];
            let file = files.named(
                fs_operation::OPEN,
                act3 as u64,
                name_of(name.as_bytes(), &mut name_buf),
                0,
            );
            if file >= 0 {
                files.close(file);
            }
            files.close(act3);
            if file >= 0 {
                return Answer::from((S::StoreFailed, live));
            }
        }
    }
    Answer::from((S::Done, live))
}

/// **The delegated check's refusal**: the approval does not allow this package, named for the
/// reply's follower ([`holder_words`]), which `jig` reads and says (milestone 911).
fn refused_not_approved(
    cx: &EditContext<'_>,
    package: &str,
) -> (spawnproto::ActivationStatus, u32) {
    // The name rides the caller's buffer, which the Answer reads after the verb returns, as a
    // NameTaken's holder does (milestone 911).
    let mut bytes = [0u8; spawnproto::HOLDER_MAXIMUM];
    let len = package.len().min(spawnproto::HOLDER_MAXIMUM);
    bytes[..len].copy_from_slice(&package.as_bytes()[..len]);
    cx.refused.set((bytes, len));
    (spawnproto::ActivationStatus::NotApproved, cx.live)
}

/// **The owner's `--approved` check's refusal** (milestone 911): the ask that exceeded the entry
/// rides the reply's third follower, which the owner's console may read, beside the package the
/// first two name.
fn refused_with_ask(
    cx: &EditContext<'_>,
    package: &str,
    what: installed_programs::Granted<'_>,
) -> (spawnproto::ActivationStatus, u32) {
    let refused = refused_not_approved(cx, package);
    // The directory rights ride the kind word as 1 for `r` and 3 for `rw`, the bit spelling
    // `spawnproto::refused_ask_words` keeps, so the owner's console spells the ask whole.
    let (kind, rights, name) = match what {
        installed_programs::Granted::Network => (spawnproto::ASK_NETWORK, 0, ""),
        installed_programs::Granted::Service => (spawnproto::ASK_SERVICE, 0, ""),
        installed_programs::Granted::Installer => (spawnproto::ASK_INSTALLER, 0, ""),
        installed_programs::Granted::Zone(zone) => (spawnproto::ASK_ZONE, 0, zone),
        installed_programs::Granted::Directory(name, rights) => (
            spawnproto::ASK_DIRECTORY,
            match rights {
                installed_programs::DirectoryRights::R => 1,
                installed_programs::DirectoryRights::RW => 3,
            },
            name,
        ),
    };
    let (a, b, c) = spawnproto::refused_ask_words(kind, rights, name);
    cx.ask.set([a, b, c]);
    refused
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
