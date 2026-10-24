//! **Running a program**: the argv and name-set pages, the words grant, `run_image` and the
//! manifest note, `dir_grant`, `spawn`, draining what a child prints and its second stream, and
//! `outcome`.
//!
//! One of the modules §273 (the shell program becomes a directory of modules) lays out. The design
//! is in `notes/swish-language/running.md`.
//!
//! Name: provisional 2026-10-11 (UTC), milestone 844's lane. calef deferred every module name
//! in §273 to a name-ratification pass.

use super::*;

/// **The window a line's argvs are written through** (milestone 205 (how a foreign program is told
/// what to do), DECISIONS §170): one page per pipeline stage at the bottom of [`IMAGE_WINDOW`], below
/// the name sets and the image, so their page tables come from [`IMAGE_TABLE_PAGES`] as the image's
/// do.
const ARGV_VA: u64 = IMAGE_WINDOW;
/// **And the window a stage's name set is written through** (milestone 205's designation half,
/// `spawnproto::NAMESET_BIT`): one page per stage, above the argv pages and below [`IMAGE_VA`], so
/// [`IMAGE_TABLE_PAGES`] pays for these tables too.
const SET_VA: u64 = ARGV_VA + line::MAX_STAGES as u64 * spawnproto::IMAGE_PAGE;

/// **One line's argv, written and waiting to be sent**: the one-page region it was carved from and
/// the frame in it. The frame travels to the progenitor narrowed to `READ` (`spawnproto::PROGRAM_ARGUMENTS_BIT`)
/// and the progenitor copies it before the child runs, so the region goes back as soon as the
/// child has answered.
#[derive(Clone, Copy)]
pub(super) struct Argv {
    staging: u64,
    pub(super) frame: u64,
}

/// **Write `cmd`'s words onto a fresh frame**, through window `stage`, with `grant_plan::argv`, or
/// say why not: `Err(Some)` is a refusal about the line, `Err(None)` is this shell's budget.
/// Nothing is sent either way. The planner already ran `check_words` on the same bytes, so a
/// refusal here is not expected; it is handled rather than unwrapped because a shell that panics
/// takes the prompt with it.
pub(super) fn assemble_argv(cmd: &[u8], stage: usize) -> Result<Argv, Option<Refusal>> {
    let va = ARGV_VA + stage as u64 * spawnproto::IMAGE_PAGE;
    let a = fresh_page(va).ok_or(None)?;
    // SAFETY: `va` is one page this shell just mapped read/write, fresh, and nothing else reaches
    // it until the frame is sent; the borrow ends before this function returns.
    let page = unsafe { &mut *(va as *mut [u8; spawnproto::IMAGE_PAGE as usize]) };
    match grant_plan::argv(cmd, page) {
        Ok(_) => Ok(a),
        Err(r) => {
            release_argv(a, true);
            Err(Some(r))
        }
    }
}

/// **A fresh one-page region and its frame, mapped read/write at `va`**, or `None` if this shell's
/// budget cannot pay for it. What an argv and a name set are each written on.
fn fresh_page(va: u64) -> Option<Argv> {
    if !prime_image_window() {
        return None;
    }
    let staging = memory_region_split(1)?;
    match u64::try_from(user_mode_runtime::retype_into_page_frame(staging)) {
        Ok(frame) if map_window_frame(frame, va) => Some(Argv { staging, frame }),
        Ok(frame) => {
            cap_delete(frame);
            release_region(staging);
            None
        }
        Err(_) => {
            release_region(staging);
            None
        }
    }
}

/// **A directory grant for a program that hears words, ready to send** (milestone 205, §170 clauses
/// 2 to 5): the two `START` word triples every directory grant travels as, and, when the grant is a
/// set of names rather than the whole directory, the page the set is written on.
#[derive(Clone, Copy)]
pub(super) struct WordsGrant {
    pub(super) words: DirWords,
    pub(super) set: Option<Argv>,
}

/// **Turn a word designation into what the progenitor builds it from**, through set window `stage`.
///
/// The caretaker descends into the current directory with `grant`'s rights, clamped by the
/// progenitor: a `fs_nameset_caretaker` with the set on a page, or for `.` a
/// `fs_subtree_caretaker`. `Err(Some(sentence))` is undeliverable and `Err(None)` the budget.
///
/// # BUGS
///
/// - A redirected job's drain can race the set's filter (`notes/a-set-grant-at-the-prompt.md`)
///   until milestone 599 (a frame per filesystem client channel).
pub(super) fn words_grant(
    g: &GrantDir,
    grant: grant_plan::WordGrant,
    stage: usize,
) -> Result<WordsGrant, Option<&'static [u8]>> {
    let dir = match g.directory.depth() {
        1 => g.directory.component(0),
        0 => {
            return Err(Some(
                b"  its words name things in the root of this shell's namespace, and a caretaker is\n  built by descending into a directory: `cd` into one first\n",
            ));
        }
        _ => {
            return Err(Some(
                b"  this shell is standing more than one level down, and the progenitor builds one\n  caretaker per grant; a deeper grant is a chain of them, which is not built\n",
            ));
        }
    };
    // **A directory in the mounted tree is named through the mount** (milestone 834 (ioping on nife and Linux)): the
    // progenitor builds the caretaker against the tree `spawnproto::in_mounted_tree` says.
    let mut routed = [0u8; filesystem_contract::grant::WORDS_NAME + 1];
    let dir = match g.which {
        nav::Which::A => dir,
        nav::Which::B => {
            let mount = spawnproto::MOUNTED_TREE_NAME;
            let n = mount.len() + 1 + dir.len();
            if n > routed.len() {
                return Err(Some(
                    b"  that name does not fit in a grant's two argument words\n",
                ));
            }
            routed[..mount.len()].copy_from_slice(mount);
            routed[mount.len()] = b'/';
            routed[mount.len() + 1..n].copy_from_slice(dir);
            &routed[..n]
        }
    };
    let Some((dir_lo, dir_hi)) = filesystem_contract::grant::pack_name(dir) else {
        return Err(Some(
            b"  that name does not fit in a grant's two argument words\n",
        ));
    };
    let words = DirWords {
        caretaker: (
            dir_lo,
            dir_hi,
            filesystem_contract::grant::spec(dir.len(), grant.rights()),
        ),
        child: (0, 0, 0),
    };
    if g.names.is_empty() {
        return Ok(WordsGrant { words, set: None });
    }
    Ok(WordsGrant {
        words,
        set: Some(set_page(g, stage)?),
    })
}

/// **Write a grant's names onto a fresh page**, through set window `stage`, in
/// `filesystem_contract::nameset`'s encoding: what the progenitor maps into `fs_nameset_caretaker`.
/// `Err(Some(sentence))` is a set the encoding refuses, which the planner's own bounds make
/// unexpected, and `Err(None)` the budget.
fn set_page(g: &GrantDir, stage: usize) -> Result<Argv, Option<&'static [u8]>> {
    let va = SET_VA + stage as u64 * spawnproto::IMAGE_PAGE;
    let page = fresh_page(va).ok_or(None)?;
    let mut list = [(&b""[..], false); filesystem_contract::nameset::MAX_NAMES];
    let mut n = 0;
    for (name, is_dir) in g.names.iter() {
        list[n] = (name, is_dir);
        n += 1;
    }
    // SAFETY: `va` is one page this shell just mapped read/write, fresh, and nothing else reaches
    // it until the frame is sent; the borrow ends before this function returns.
    let out = unsafe { &mut *(va as *mut [u8; spawnproto::IMAGE_PAGE as usize]) };
    match filesystem_contract::nameset::encode(&list[..n], out) {
        Some(_) => Ok(page),
        None => {
            release_argv(page, true);
            Err(Some(
                b"  that is more names, or more bytes of name, than one grant carries\n",
            ))
        }
    }
}

/// Send a words grant's set page, `READ` only (`spawnproto::NAMESET_BIT`), after the argv's.
pub(super) fn send_set(w: &WordsGrant) {
    if let Some(p) = w.set {
        user_mode_runtime::send_capability(SPAWN, p.frame, abi::rights::READ, 0);
        cap_delete(p.frame);
    }
}

/// Give back a words grant's page, if it has one.
pub(super) fn release_words_grant(w: &WordsGrant, unsent: bool) {
    if let Some(p) = w.set {
        release_argv(p, unsent);
    }
}

/// Give back an argv's region, and its frame's slot if it was never sent.
pub(super) fn release_argv(a: Argv, unsent: bool) {
    if unsent {
        cap_delete(a.frame);
    }
    release_region(a.staging);
}

fn release_region(region: u64) {
    user_mode_runtime::destroy_memory_region(region);
    cap_delete(region);
}

/// **Where this shell writes what it sends the progenitor in frames**: the argv pages
/// ([`ARGV_VA`]), the name-set pages ([`SET_VA`]), then an image's frames ([`IMAGE_VA`], DECISIONS §219 option D).
pub(super) const IMAGE_WINDOW: u64 = address_space_map::pair_page(0x0000_0000_0400_0000);

/// **The window this shell writes an image's frames through**, above the name-set pages. At most
/// [`spawnproto::IMAGE_MAX_PAGES`] pages, which is 4 MiB and so crosses 2 MiB page-table spans.
const IMAGE_VA: u64 = SET_VA + line::MAX_STAGES as u64 * spawnproto::IMAGE_PAGE;

/// The first address past [`IMAGE_WINDOW`].
pub(super) const IMAGE_WINDOW_END: u64 =
    IMAGE_VA + spawnproto::IMAGE_MAX_PAGES * spawnproto::IMAGE_PAGE;

/// **The pages that pay for [`IMAGE_WINDOW`]'s page tables**, split off [`BUDGET`] once per shell
/// and never given back. A staging region's pages go back to [`BUDGET`] on `DESTROY` only if it is
/// the budget's most recent carve, so a page table allocated from the budget while staging would
/// strand every staging page. Tables, once, are the price of having no unmap (DECISIONS §162
/// (whether a holder can give up a mapping)).
///
/// One last-level table per 2 MiB span the window touches, and one more for the level above in
/// case the window is the first thing this shell maps in its gigabyte.
const IMAGE_TABLE_PAGES: u64 = (IMAGE_WINDOW_END - 1) / 0x20_0000 - IMAGE_WINDOW / 0x20_0000 + 2;

/// The region [`IMAGE_TABLE_PAGES`] describes, once split; [`u64::MAX`] until then.
static IMAGE_TABLES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(u64::MAX);

/// **Run a file by its bytes** (DECISIONS §219 (how the shell names an installed program to the
/// spawner) option D; milestone 198 (a package manager) rung 3a).
///
/// Sends the file's frames on the spawn endpoint under `IMAGE_BIT`; a digest the progenitor does
/// not find runs only under [`HOLDS_RUN_UNVOUCHED`], presented last. The line is bound against the
/// manifest note [`read_manifest_note`] finds (milestone 597 (a program carries its manifest in an
/// ELF note)), or `grant_plan::NO_NOTE_MANIFEST`, and every refusal sends nothing. The staging
/// region is destroyed once the answer is in, revoking the frames from both address spaces. See
/// `notes/swish-language/running.md` and `spawnproto`'s BUGS.
pub(super) fn run_image(nav: &mut Nav, spec: ParsedInvocation) {
    let Some(PlannedImage {
        t,
        handle,
        size,
        m,
        e,
    }) = plan_image(nav, spec)
    else {
        return;
    };
    let pages = spawnproto::image_pages(size);
    let holds_run_unvouched = HOLDS_RUN_UNVOUCHED.load(core::sync::atomic::Ordering::Relaxed);

    let Some(ImageGrants {
        mem_slot,
        argv,
        set_grant,
        staging,
    }) = carve_image_grants(nav, spec, t, handle, &e, &m, pages)
    else {
        return;
    };

    let (w0, w1, w2) = spawnproto::request(
        size,
        e.program_argument,
        if mem_slot.is_some() {
            e.memory_grant_pages
        } else {
            0
        },
        spawnproto::Wiring {
            image: true,
            run_unvouched: holds_run_unvouched,
            program_arguments: argv.is_some(),
            directory_grant: set_grant.is_some(),
            nameset: set_grant.is_some_and(|w| w.set.is_some()),
            ..spawnproto::Wiring::default()
        },
    );
    send(SPAWN, w0, w1, w2);
    // The grant's data before the image's frames, the order the progenitor reads (`GRANT_WORDS`).
    if let Some(DirWords { caretaker, child }) = set_grant.map(|w| w.words) {
        send(SPAWN, caretaker.0, caretaker.1, caretaker.2);
        send(SPAWN, child.0, child.1, child.2);
    }
    let read_ok = send_frames(t.slot, handle, pages, staging);
    nav.close_in(t, handle);
    // The argv's frame follows the image's, before every other capability (`spawnproto::PROGRAM_ARGUMENTS_BIT`).
    if let Some(a) = argv {
        user_mode_runtime::send_capability(SPAWN, a.frame, abi::rights::READ, 0);
        cap_delete(a.frame);
    }
    if let Some(w) = &set_grant {
        send_set(w);
    }
    if let Some(slot) = mem_slot {
        // Every right: the progenitor builds the child from it, and since milestone 824 (a retype
        // mints no right its budget lacks) what it builds carries the budget's rights.
        delegate(slot, abi::rights::ALL);
        cap_delete(slot);
    }
    // **The presentation, last** (`spawnproto::RUN_UNVOUCHED_BIT`): one word on the endpoint only a
    // holder can reach. Sent on every image request this session makes, vouched or not, because
    // the shell cannot know the verdict and the progenitor is waiting for it either way.
    if holds_run_unvouched {
        send(spawnproto::RUN_UNVOUCHED_SLOT, 0, 0, 0);
    }

    report_image_answer(e, &m, read_ok);
    // The frames are spent: the child was built from the progenitor's copy. Destroying the region
    // revokes every mapping of them, ours and the progenitor's, and returns the pages.
    user_mode_runtime::destroy_memory_region(staging);
    cap_delete(staging);
    if let Some(w) = &set_grant {
        release_words_grant(w, false);
    }
    if let Some(a) = argv {
        release_argv(a, false);
    }
}

/// [`run_image`]'s answer: the image's output or its one word, then whether the bytes sent were
/// the whole file.
fn report_image_answer(e: Endowment, m: &grant_plan::Manifest, read_ok: bool) {
    // The output's shape is the manifest's, never `e.program`'s: an image's row is a stand-in
    // (`grant_plan::IMAGE_ROW`).
    if m.output.is_byte_stream() {
        drain_text();
    } else {
        let answer = receive(RESULT).0;
        // The four progenitor words an image can be answered with sit at the top of `u64`
        // (`SPAWN_REFUSED_BY_MANIFEST` is the lowest; `SPAWN_NO_DISPLAY`, below it, answers only a
        // `graphical_terminal` launch); anything below is the program's own answer, which no row
        // renders for an image.
        if answer >= spawnproto::SPAWN_REFUSED_BY_MANIFEST {
            outcome(e, answer);
        } else {
            print(b"  it answered ");
            print_num(answer);
            print(b"\n");
        }
    }
    if !read_ok {
        print(b"  (this shell could not read the whole file, so those were not its bytes)\n");
    }
}

/// What [`plan_image`] read and decided: the open file, its size, the manifest the line was bound
/// against, and the endowment.
struct PlannedImage {
    t: Tree,
    handle: u64,
    size: u64,
    m: grant_plan::Manifest,
    e: Endowment,
}

/// [`run_image`]'s first half: open the file, read its note, and bind the line against the manifest
/// it carries. Each refusal closes the file and says why, and the answer is `None`.
fn plan_image(nav: &mut Nav, spec: ParsedInvocation) -> Option<PlannedImage> {
    if nav.dir.is_none() {
        say(Say::NoDirectory);
        return None;
    }
    let (t, handle, size) = open_for_bytes(nav, spec.program)?;
    let declared = match read_manifest_note(t.slot, handle, size) {
        Ok(d) => d,
        Err(r) => {
            nav.close(handle);
            refused();
            print(swish::image_refusal_sentence(r));
            return None;
        }
    };
    let m = declared.unwrap_or(grant_plan::NO_NOTE_MANIFEST);
    if !grant_plan::image_can_carry(&m) {
        nav.close(handle);
        refused();
        print(swish::image_refusal_sentence(
            grant_plan::ImageRefusal::NotCarried,
        ));
        return None;
    }
    // **A note that asks for the network is judged on the vouch before anything is sent** (milestone
    // 892): vouched bytes with no `grant` line for it are refused it, which the progenitor would
    // answer only as `SPAWN_REFUSED_BY_MANIFEST`, so the shell hashes the file as `caps` does and
    // says which grant is missing. Only such notes pay for the hash; the progenitor still judges its own copy.
    if m.network {
        let vouch = super::capabilities::file_digest(t, handle, size)
            .and_then(|d| super::capabilities::live_generation_listing(nav, &d))
            .map_or(grant_plan::Vouch::Unvouched, |v| {
                grant_plan::Vouch::Vouched {
                    network: v.network_granted,
                }
            });
        if let Err(r) = grant_plan::image_manifest(declared, vouch) {
            nav.close(handle);
            refused();
            print(swish::image_refusal_sentence(r));
            return None;
        }
    }
    // **Expanded, or designated, once the note says which** (milestone 205): bytes that hear words
    // have their words designated rather than their patterns expanded, and only the note says
    // whether these do.
    let expanded = match m.program_argument {
        grant_plan::ProgramArgumentDeclaration::Words(grant) => {
            swish::designation(spec.line(), grant, &mut |token| nav.expand(token))
        }
        _ => expansion(nav, &spec),
    };
    let expanded = match expanded {
        Ok(e) => e,
        Err(said) => {
            nav.close(handle);
            match said {
                Say::Cannot(r) => refuse(spec, r),
                said => say(said),
            }
            return None;
        }
    };
    let e = match grant_plan::plan_against(&spec, grant_plan::IMAGE_ROW, m, holdings(nav), expanded)
    {
        Ok(e) => e,
        Err(r) => {
            nav.close(handle);
            refuse(spec, r);
            return None;
        }
    };
    Some(PlannedImage {
        t,
        handle,
        size,
        m,
        e,
    })
}

/// What [`carve_image_grants`] split off this shell's budget for one image.
struct ImageGrants {
    mem_slot: Option<u64>,
    argv: Option<Argv>,
    set_grant: Option<WordsGrant>,
    staging: u64,
}

/// [`run_image`]'s carving, in budget order: the window's tables, any `--mem` region, the argv,
/// the words grant and the staging region. Each failure closes the file, gives back what was
/// carved, says why, and answers `None`.
fn carve_image_grants(
    nav: &mut Nav,
    spec: ParsedInvocation,
    t: Tree,
    handle: u64,
    e: &Endowment,
    m: &grant_plan::Manifest,
    pages: u64,
) -> Option<ImageGrants> {
    // The window's tables first (once), then any `--mem` region, then the staging region, so
    // staging is the top of the budget when it is destroyed. See [`IMAGE_TABLE_PAGES`].
    if !prime_image_window() {
        nav.close_in(t, handle);
        out_of_budget();
        return None;
    }
    let mem_slot = if e.memory_grant_pages > 0 {
        memory_region_split(e.memory_grant_pages)
    } else {
        None
    };
    // **Bytes whose note says they hear words get the line as their argv** (milestone 205 (how a
    // foreign program is told what to do)), which is how an installed `rg` hears its pattern. The
    // line's first word is the path it was run by, so that is `argv[0]`. Its region is carved
    // below staging, so staging is still the top when it goes; a program that hears words takes no
    // `--mem` (`grant_plan::image_can_carry`), so nothing sits between the two.
    let argv = if grant_plan::image_hears_words(m) {
        match assemble_argv(spec.line(), 0) {
            Ok(a) => Some(a),
            Err(why) => {
                nav.close_in(t, handle);
                match why {
                    Some(r) => refuse(spec, r),
                    None => out_of_budget(),
                }
                return None;
            }
        }
    } else {
        None
    };
    // **And what its words designated** (milestone 205's designation half), on a page carved
    // after the argv's and still below staging.
    let set_grant = match (e.directory, m.program_argument) {
        (Some(g), grant_plan::ProgramArgumentDeclaration::Words(grant)) => {
            match words_grant(&g, grant, 0) {
                Ok(w) => Some(w),
                Err(why) => {
                    nav.close_in(t, handle);
                    if let Some(a) = argv {
                        release_argv(a, true);
                    }
                    match why {
                        Some(sentence) => {
                            refused();
                            print(sentence);
                        }
                        None => out_of_budget(),
                    }
                    return None;
                }
            }
        }
        _ => None,
    };
    let Some(staging) = memory_region_split(pages) else {
        nav.close_in(t, handle);
        if let Some(m) = mem_slot {
            cap_delete(m);
        }
        if let Some(w) = &set_grant {
            release_words_grant(w, true);
        }
        if let Some(a) = argv {
            release_argv(a, true);
        }
        out_of_budget();
        return None;
    };
    Some(ImageGrants {
        mem_slot,
        argv,
        set_grant,
        staging,
    })
}

/// The first page of a file whose manifest note is being read, and the note segment it points at.
/// Statics rather than locals for [`GENERATION_TABLE`]'s reason: two pages on the stack would take
/// the frame past the guard page `script/stack-frame-check` holds every frame to.
static mut NOTE_HEAD: [u8; filesystem_contract::PAGE] = [0; filesystem_contract::PAGE];
static mut NOTE_SEGMENT: [u8; filesystem_contract::PAGE] = [0; filesystem_contract::PAGE];

/// **The manifest the open file `handle` carries in its own bytes** (milestone 597 (a program
/// carries its manifest in an ELF note)), read the way the progenitor reads its copy: `PT_NOTE`
/// headers from the first page, each note segment searched whole, decoded by
/// `capability_manifest_note::decode`. `Ok(None)` is no manifest note, and also a first page that
/// is not an ELF header holding its program headers. A note that cannot be read, or a note segment
/// over one page, is `Err`. See `notes/swish-language/running.md`, "The manifest note".
pub(super) fn read_manifest_note(
    dir: u64,
    handle: u64,
    size: u64,
) -> Result<Option<grant_plan::Manifest>, grant_plan::ImageRefusal> {
    use grant_plan::ImageRefusal::Unreadable;
    // SAFETY: this shell is one thread, and these two statics are used here and nowhere else, so
    // no other reference to either is live.
    let head = unsafe { &mut *core::ptr::addr_of_mut!(NOTE_HEAD) };
    // SAFETY: as above; a distinct static, so the two borrows do not alias.
    let seg_buf = unsafe { &mut *core::ptr::addr_of_mut!(NOTE_SEGMENT) };
    let head_len = (size as usize).min(head.len());
    if read_at(dir, handle, 0, &mut head[..head_len]).is_none() {
        return Err(Unreadable);
    }
    let Ok(segs) = elf::NoteSegments::from_head(&head[..head_len]) else {
        return Ok(None);
    };
    let mut search = elf::NoteSearch::new(
        capability_manifest_note::OWNER,
        capability_manifest_note::MANIFEST,
    );
    let mut found = None;
    for seg in segs {
        let r = seg.range(size as usize).map_err(|_| Unreadable)?;
        let len = r.len();
        if len > seg_buf.len()
            || read_at(dir, handle, r.start as u64, &mut seg_buf[..len]).is_none()
        {
            return Err(Unreadable);
        }
        if let Some(d) = search
            .segment(&seg_buf[..len], seg.p_align)
            .map_err(|_| Unreadable)?
        {
            found = Some(capability_manifest_note::decode(d).map_err(|_| Unreadable)?);
        }
    }
    Ok(found)
}

/// Fill `out` from the open file `handle` starting at byte `offset`, through the shared file page.
/// `None` if the file ends or a read fails before `out` is full.
fn read_at(dir: u64, handle: u64, offset: u64, out: &mut [u8]) -> Option<()> {
    let mut done = 0usize;
    while done < out.len() {
        let want = (out.len() - done).min(PAGE as usize) as u64;
        let n = call(dir, fs::req(fs::READ, handle, want), offset + done as u64).0 as i64;
        if n <= 0 {
            return None;
        }
        let n = (n as usize).min(out.len() - done);
        get_page_at(0, &mut out[done..done + n]);
        done += n;
    }
    Some(())
}

pub(super) fn out_of_budget() {
    failed();
    print(b"  this shell's memory budget is exhausted; nothing left to grant\n");
}

/// **Open the file `path` names, for its bytes**, the way `<` would: walk the lead, open the last
/// component. Returns the tree it is in (either, in a two-grant shell), the handle and the size, or
/// prints why not and returns `None`. A file that
/// is empty or larger than [`spawnproto::IMAGE_MAX_PAGES`] is refused here, before anything is
/// sent, because both a §219 image and a package travel as that many frames at most.
pub(super) fn open_for_bytes(nav: &mut Nav, path: &[u8]) -> Option<(Tree, u64, u64)> {
    let p = match nav.plan_name_path(path) {
        Ok(planned) => planned,
        Err(said) => {
            say(said);
            return None;
        }
    };
    let Some((lead, name)) = p.split_last_component() else {
        refused();
        print(b"  that path ends in a directory, not a file\n");
        return None;
    };
    let w = match nav.walk_steps(p.is_from_root(), lead) {
        Ok(w) => w,
        Err(said) => {
            say(said);
            return None;
        }
    };
    let t = w.tree;
    let opened = nav.name_call_in(t, fs::OPEN, w.handle, name, 0);
    nav.unwind(&w);
    if opened < 0 {
        say(Say::Failed(-opened as i32));
        return None;
    }
    let handle = opened as u64;
    let size = call(t.slot, fs::req(fs::FSTAT, handle, 0), 0).0 as i64;
    if size <= 0 || spawnproto::image_pages(size as u64) > spawnproto::IMAGE_MAX_PAGES {
        nav.close_in(t, handle);
        refused();
        print(b"  that file is empty, or larger than an image may be (4 MiB)\n");
        return None;
    }
    Some((t, handle, size as u64))
}

/// Split [`IMAGE_TABLE_PAGES`] off the budget once per shell, before any staging region, so the
/// window's page tables never come from the budget above one. `false` if the budget could not pay.
fn prime_image_window() -> bool {
    use core::sync::atomic::Ordering::Relaxed;
    if IMAGE_TABLES.load(Relaxed) == u64::MAX {
        let Some(tables) = memory_region_split(IMAGE_TABLE_PAGES) else {
            return false;
        };
        IMAGE_TABLES.store(tables, Relaxed);
    }
    true
}

/// Map a frame read/write into [`IMAGE_WINDOW`], its page tables paid from [`IMAGE_TABLES`]. Only
/// after [`prime_image_window`] has answered `true`.
fn map_window_frame(frame: u64, va: u64) -> bool {
    debug_assert!((IMAGE_WINDOW..IMAGE_WINDOW_END).contains(&va));
    let tables = IMAGE_TABLES.load(core::sync::atomic::Ordering::Relaxed);
    tables != u64::MAX && user_mode_runtime::map_page_frame(frame, va, true, tables)
}

/// **Send a file's bytes on the spawn endpoint as `pages` frames**, after a request that announced
/// them (DECISIONS §219 option D's image, or a `vouch`). One frame is held at a time:
/// retyped from `staging`, filled from the file, delegated narrowed to `READ`, deleted. Returns
/// whether every page was read; `false` still sends every announced frame, because the progenitor
/// is waiting for exactly that many and the messages behind them.
fn send_frames(dir: u64, handle: u64, pages: u64, staging: u64) -> bool {
    let mut read_ok = true;
    for i in 0..pages {
        let va = IMAGE_VA + i * PAGE;
        let Ok(frame) = u64::try_from(user_mode_runtime::retype_into_page_frame(staging)) else {
            // Only a full capability table or a kernel contradicting the budget lands here. The
            // progenitor then waits for a frame that never comes, and the prompt hangs; see
            // `spawnproto`'s BUGS.
            return false;
        };
        if map_window_frame(frame, va) {
            // SAFETY: `va` was just mapped read/write, one page.
            let window = unsafe { MappedWindow::new(va, PAGE) };
            let n = call(dir, fs::req(fs::READ, handle, PAGE), i * PAGE).0 as i64;
            if n < 0 {
                read_ok = false;
            } else {
                for at in 0..n as u64 {
                    window.w8(at, FS_WINDOW.r8(at));
                }
            }
        } else {
            read_ok = false;
        }
        user_mode_runtime::send_capability(SPAWN, frame, abi::rights::READ, i);
        cap_delete(frame);
    }
    read_ok
}

/// **`vouch <file>`: the owner vouches for a file's bytes** (DECISIONS §221 (the boot prompt is the
/// owner's console), ruling 1): one activation request on the spawn endpoint, carrying the file's
/// bytes exactly as [`run_image`] sends an executable's, so the progenitor hashes its own copy, and
/// then the name to record them under. One reply names the generation live afterwards.
///
/// It stays here, on the endpoint only the owner's console holds, because `jig`'s installer
/// endpoint refuses a vouch (milestone 809 (the package client becomes a program)). The shell never
/// reads or writes `activation/` itself here; §221 ruled that this prompt, the owner's console, may.
pub(super) fn vouch(nav: &mut Nav, path: Option<&[u8]>) {
    use spawnproto::{Activation, ActivationStatus as S};
    let Some(path) = path else {
        refused();
        print(swish::VOUCH_USAGE);
        return;
    };
    if nav.dir.is_none() {
        return say(Say::NoDirectory);
    }
    let Some(name) = grant_plan::vouched_name(path) else {
        refused();
        print(swish::VOUCH_USAGE);
        return;
    };
    let Some((t, handle, size)) = open_for_bytes(nav, path) else {
        return;
    };
    let pages = spawnproto::image_pages(size);
    let staging = if prime_image_window() {
        memory_region_split(pages)
    } else {
        None
    };
    let Some(staging) = staging else {
        nav.close_in(t, handle);
        return out_of_budget();
    };
    let (w0, w1, w2) = spawnproto::activation_request(Activation::Vouch, size);
    send(SPAWN, w0, w1, w2);
    let read_ok = send_frames(t.slot, handle, pages, staging);
    nav.close_in(t, handle);
    // `vouched_name` held the name to `grant::WORDS_NAME`, so it packs; the frames are already
    // sent, so the name message goes whatever happens, and empty words are a name nothing holds.
    let (lo, hi) = filesystem_contract::grant::pack_name(name).unwrap_or((0, 0));
    send(SPAWN, lo, hi, name.len() as u64);
    let (r0, r1, _) = receive(RESULT);
    user_mode_runtime::destroy_memory_region(staging);
    cap_delete(staging);
    if !read_ok {
        print(b"  (this shell could not read the whole file, so those were not its bytes)\n");
    }
    let status = S::from_word(r0);
    match status {
        S::Done => {}
        S::StoreFailed | S::Unknown => failed(),
        _ => refused(),
    }
    swish::write_vouch(status, r1, &mut print);
}

/// Print a refusal in the capability model's voice, which is [`swish::write_refusal`]'s job: the
/// fixed half is `grant_plan`'s and the program name is supplied where one helps.
pub(super) fn refuse(spec: ParsedInvocation, refusal: Refusal) {
    refused();
    swish::write_refusal(&spec, refusal, &mut print);
}

/// One process's three `START` argument words, packed by this shell and forwarded by the progenitor without
/// being read. `filesystem_contract::grant`'s layout: two words of name, and a spec carrying the length plus
/// either a rights mask (a caretaker's) or an option mask (a program's).
type StartWords = (u64, u64, u64);

/// **What a directory grant travels as**: one process's `START` words each, for the two processes a
/// grant is made of.
///
/// Named rather than a bare pair of tuples because the order is the wire's, and getting it backwards
/// would start `rm` with a directory's name and a caretaker with a file's, which comes up serving a
/// namespace nobody meant. `spawnproto::GRANT_WORDS` is the count on the other side.
#[derive(Clone, Copy)]
pub(super) struct DirWords {
    /// The `fs_subtree_caretaker`'s: the granted directory, and the rights to ask for.
    pub(super) caretaker: StartWords,
    /// The confined program's: the operand inside that directory, and the options typed with it.
    pub(super) child: StartWords,
}

/// **Turn a planned directory grant into the two triples the progenitor needs**, or into the
/// sentence that says why it cannot be delivered (milestone 31 (a capability shell: designation is
/// authorization) phase 3).
///
/// `Ok` is `(the caretaker's START words, the program's)`: the directory and the
/// `filesystem_contract::dir` rights (`REMOVE`, or `REMOVE_TREE` with `-r`), then the operand and
/// `flags`, plus whether the operand rides a name set rather than the words. A name longer than
/// two words hold (`grant::WORDS_NAME`, milestone 946) does: the program is told
/// `grant::WHOLE_NAMESPACE` and the grant becomes a set of that one name, which
/// `fs_nameset_caretaker` serves and [`spawn_dir_grant`] writes on a page. Nothing between here and
/// the process decodes the words. Refused with nothing spawned: a second tree, the namespace root,
/// a directory deeper than one level, a set of more than one name, and a directory whose name does
/// not fit in a grant's two argument words. See `notes/swish-language/running.md`, "The directory
/// grant".
pub(super) fn dir_grant(g: &GrantDir, flags: u64) -> Result<(DirWords, bool), &'static [u8]> {
    // **A grant in a two-grant shell's second tree** (milestone 154). The progenitor builds the
    // caretaker by descending from the root of the one filesystem it holds, which is the first
    // tree's; the words below say nothing about which tree, so sending them for the second would
    // hand the program a directory of the same name in the *first*. Refused here, with nothing
    // spawned, until the spawn protocol carries the tree (the roadmap block's BUGS entry).
    if g.which != nav::Which::A {
        return Err(
            b"  that directory is in a mounted tree, and the progenitor builds a caretaker\n  from the root tree; a grant there is not built\n",
        );
    }
    // The directory the caretaker descends into. One component, because that is one `OPENDIR`; a
    // deeper path is a *chain* of caretakers (DECISIONS §92 names it as the case supervision was
    // chosen for) and the progenitor builds one.
    let dir = match g.directory.depth() {
        0 => {
            return Err(
                b"  that names the root of this shell's namespace, and a caretaker is built by \n  descending into a directory; there is no name here to descend into\n",
            );
        }
        1 => g.directory.component(0),
        _ => {
            return Err(
                b"  that directory is more than one level down, and the progenitor builds one caretaker \n  per grant; a deeper grant is a chain of them, which is not built\n",
            );
        }
    };
    let Some(name) = g.names.only() else {
        return Err(
            b"  a set of names is delivered by a nameset caretaker, and the progenitor builds the subtree \n  one; name a single file\n",
        );
    };
    let Some((dir_lo, dir_hi)) = filesystem_contract::grant::pack_name(dir) else {
        return Err(b"  that name does not fit in a grant's two argument words\n");
    };
    // What `-r` buys, stated as the difference between two capabilities rather than as a branch in
    // the program: REMOVE alone cannot even look at what is under the directory.
    let rights = if g.subtree {
        filesystem_contract::dir::REMOVE_TREE
    } else {
        filesystem_contract::dir::REMOVE
    };
    // `fs_subtree_caretaker::_start(name_lo, name_hi, spec)`, or `fs_nameset_caretaker`'s, which
    // takes the same words.
    let caretaker = (
        dir_lo,
        dir_hi,
        filesystem_contract::grant::spec(dir.len(), rights),
    );
    // `rm::_start(spec, name_lo, name_hi)`, whose spec carries the options where a caretaker's
    // carries rights: both are "what this process was started with".
    let (child, set) = match filesystem_contract::grant::pack_name(name) {
        Some((name_lo, name_hi)) => (
            (
                filesystem_contract::grant::spec(name.len(), flags),
                name_lo,
                name_hi,
            ),
            false,
        ),
        None => (
            (
                filesystem_contract::grant::spec(filesystem_contract::grant::WHOLE_NAMESPACE, flags),
                0,
                0,
            ),
            true,
        ),
    };
    Ok((DirWords { caretaker, child }, set))
}

/// Grant and spawn. The one moment authority moves: split any memory grant off our own budget,
/// direct the progenitor to load the program, delegate the grant, and read the one answer that comes back.
pub(super) fn spawn(e: Endowment, argv: Option<Argv>) {
    // Unreachable while `plan` refuses a `file:` without `holdings().dir`. If that flips, a file
    // grant must be refused loudly here, never dropped while the child runs without it.
    if e.file.is_some() {
        refused();
        print(
            b"  a file grant needs the progenitor to build the caretaker; this shell cannot deliver one yet\n",
        );
        release_unsent_argv(argv);
        return;
    }
    let Some((dir_words, set_grant)) = spawn_dir_grant(&e, argv) else {
        return;
    };
    // A memory grant is carved from the shell's own untyped. If our budget is spent, say so plainly
    // rather than sending the progenitor a promise we cannot keep.
    let mem_slot = if e.memory_grant_pages > 0 {
        match memory_region_split(e.memory_grant_pages) {
            Some(slot) => Some(slot),
            None => {
                failed();
                print(b"  this shell's memory budget is exhausted; nothing left to grant\n");
                if let Some(w) = &set_grant {
                    release_words_grant(w, true);
                }
                release_unsent_argv(argv);
                return;
            }
        }
    } else {
        None
    };

    let (w0, w1, w2) = plain_request(
        &e,
        argv.is_some(),
        dir_words.is_some(),
        set_grant.is_some_and(|w| w.set.is_some()),
    );
    send(SPAWN, w0, w1, w2);

    // **The grant's two data messages, before any delegation** (`spawnproto::GRANT_WORDS`): the
    // caretaker's three `START` words, then the confined program's. They go first because they are
    // data and the delegation is capabilities, and both sides read the order off the one wiring word.
    if let Some(DirWords { caretaker, child }) = dir_words {
        send(SPAWN, caretaker.0, caretaker.1, caretaker.2);
        send(SPAWN, child.0, child.1, child.2);
    }

    // **The argv's one frame, after the data and before every other capability**
    // (`spawnproto::PROGRAM_ARGUMENTS_BIT`), `READ` only: the progenitor copies it and can neither write it nor
    // pass it on. The slot goes now; the region waits for the child's answer.
    if let Some(a) = argv {
        user_mode_runtime::send_capability(SPAWN, a.frame, abi::rights::READ, 0);
        cap_delete(a.frame);
    }
    // The name set's frame follows the argv's (`spawnproto::NAMESET_BIT`).
    if let Some(w) = &set_grant {
        send_set(w);
    }

    // The progenitor narrows the budget to `SPEND_ONLY` in the child.
    if let Some(slot) = mem_slot {
        // Every right: the progenitor builds the child from it, and since milestone 824 (a retype
        // mints no right its budget lacks) what it builds carries the budget's rights.
        delegate(slot, abi::rights::ALL);
        cap_delete(slot); // our copy is delegated; free the slot
    }
    delegate_machine_page(machine_wiring(&e));

    // One reader, one word: a real program's answer, or the progenitor's spawn-failed sentinel. A program
    // whose manifest says it writes **bytes** is the exception: its answer is a stream, so it is
    // drained by a reader that knows the sink contract's framing.
    if e.program.manifest().output.is_byte_stream() {
        drain_text();
    } else {
        let answer = receive(RESULT).0;
        outcome(e, answer);
    }
    // The set's page was carved after the argv's, so it goes back first and each is the top.
    if let Some(w) = &set_grant {
        release_words_grant(w, false);
    }
    if let Some(a) = argv {
        release_argv(a, false);
    }
}

/// Give back an argv that was never sent, when [`spawn`] stops before the request.
fn release_unsent_argv(argv: Option<Argv>) {
    if let Some(a) = argv {
        release_argv(a, true);
    }
}

/// [`spawn`]'s directory grant: the words the progenitor builds a caretaker from, and the words
/// grant behind them for a program that hears words. A refusal says why, gives back `argv`, and
/// answers `None`.
fn spawn_dir_grant(
    e: &Endowment,
    argv: Option<Argv>,
) -> Option<(Option<DirWords>, Option<WordsGrant>)> {
    // The progenitor builds the caretaker, because this shell holds no GRANT on the file service.
    // Both helpers return words rather than sending them, so a refusal spawns nothing.
    let mut set_grant = None;
    let dir_words = match (e.directory, e.program.manifest().program_argument) {
        (None, _) => None,
        (Some(g), grant_plan::ProgramArgumentDeclaration::Words(grant)) => {
            match words_grant(&g, grant, 0) {
                Ok(w) => {
                    set_grant = Some(w);
                    Some(w.words)
                }
                Err(why) => {
                    match why {
                        Some(sentence) => {
                            refused();
                            print(sentence);
                        }
                        None => out_of_budget(),
                    }
                    release_unsent_argv(argv);
                    return None;
                }
            }
        }
        (Some(g), _) => match dir_grant(&g, e.short_options) {
            Ok((words, false)) => Some(words),
            // **A name longer than two words hold rides a set of one** (milestone 946): the same
            // page a words grant's names are written on.
            Ok((words, true)) => match set_page(&g, 0) {
                Ok(page) => {
                    set_grant = Some(WordsGrant {
                        words,
                        set: Some(page),
                    });
                    Some(words)
                }
                Err(why) => {
                    match why {
                        Some(sentence) => {
                            refused();
                            print(sentence);
                        }
                        None => out_of_budget(),
                    }
                    release_unsent_argv(argv);
                    return None;
                }
            },
            Err(sentence) => {
                refused();
                print(sentence);
                release_unsent_argv(argv);
                return None;
            }
        },
    };
    Some((dir_words, set_grant))
}

/// [`spawn`]'s request words for a plain line: `args`, `dir` and `nameset` say what follows it.
fn plain_request(e: &Endowment, args: bool, dir: bool, nameset: bool) -> (u64, u64, u64) {
    // `diagnostics` is false (DECISIONS §67 (a program's second stream is a declaration)): a plain
    // line has no `2>`, and a declared second stream gets the terminal's own sink. `2>` goes down
    // [`pipeline`].
    spawnproto::request(
        e.program.id(),
        e.program_argument,
        e.memory_grant_pages,
        spawnproto::Wiring {
            interruptible: false,
            sink: false,
            source: false,
            diagnostics: false,
            directory_grant: dir,
            // **Always false from this shell**: no manifest declares two directory operands, so
            // nothing here constructs a two-directory grant for a spawned program; see
            // `spawnproto::SECOND_DIRECTORY_GRANT_BIT`'s own doc and notes/two-trees.md's BUGS.
            second_directory_grant: false,
            // Always false (DECISIONS §106 (an unredirected tail stage's output goes to the
            // screen)): every `writes_while_reading` program also declares
            // `InputDeclaration::Required`, which takes [`run`]'s `Source::File` arm instead.
            screen: false,
            image: false,
            run_unvouched: false,
            program_arguments: args,
            nameset,
            machine: machine_wiring(e),
            // The whole request, nothing follows (milestone 715 (the spawn service holds the
            // display grants, and the shell holds none)); no display answers `SPAWN_NO_DISPLAY`.
            // A plain line is the only one `graphical_terminal` reaches. `keyboard` is the spawn
            // service's call, from what the boot granted.
            graphics: e.program == grant_plan::Program::GraphicalTerminal,
            keyboard: false,
        },
    )
}

/// **Drain a byte stream off the result rendezvous and print it**, which is what the shell does when
/// a program's output slot is the shell's own (`Sink::Report`, the default).
///
/// It drains until `OPERATION_EOF` (milestone 50 (pipes and redirection)). Stopping earlier would
/// leave a message queued that the next command's single `receive` takes as its answer.
pub(super) fn drain_text() {
    print(b"  ");
    for _ in 0..MAX_OUTPUT_CHUNKS {
        let (w0, w1, w2) = receive(RESULT);
        // Checked before decoding: the progenitor's sentinel is `u64::MAX`, whose top byte is an opcode this
        // contract does not define, so it would otherwise read as a malformed message rather than as
        // the one thing it is.
        if w0 == spawnproto::SPAWN_FAILED || w0 == spawnproto::SPAWN_NOT_IN_IMAGE {
            // Past the sentence's own indent: this function printed one before the first read.
            print(&swish::spawn_failure_sentence(w0)[2..]);
            return;
        }
        // **Refused as unvouched** (DECISIONS §219), which nothing but an image request earns.
        // Printed the way the faulted sentence below is, indent and all.
        if w0 == spawnproto::SPAWN_UNVOUCHED {
            refused();
            print(swish::UNVOUCHED_SENTENCE);
            return;
        }
        // **Refused by the bytes' own manifest** (milestone 597, provisional), which nothing but an
        // image request earns either.
        if w0 == spawnproto::SPAWN_REFUSED_BY_MANIFEST {
            refused();
            print(swish::REFUSED_BY_MANIFEST_SENTENCE);
            return;
        }
        // **The stream stops here because its writer is dead** (milestone 235), and this is the
        // one place that difference is visible: without the word, this loop would run to
        // `MAX_OUTPUT_CHUNKS` on a rendezvous nobody will ever send on again.
        if w0 == spawnproto::JOB_FAULTED {
            failed();
            print(swish::FAULTED_SENTENCE);
            return;
        }
        let mut buf = [0u8; byte_sink_contract::INLINE_MAX];
        match byte_sink_contract::unpack(w0, w1, w2, &mut buf) {
            byte_sink_contract::Msg::Bytes(n) => print(&buf[..n]),
            byte_sink_contract::Msg::Eof => return,
            byte_sink_contract::Msg::Malformed => {
                print(b"\n  (that program sent something this shell cannot read as bytes)\n");
                return;
            }
        }
    }
    print(b"\n  (output truncated: that program never said it was finished)\n");
}

// ---- the declared second stream, and where its bytes go (`2>`, DECISIONS §67) ----

/// **This shell's diagnostic rendezvous**, minted once and kept for the session. `u64::MAX` until
/// something needs it, because most sessions never spawn a program that declares a second stream.
///
/// It is retyped straight out of [`BUDGET`] rather than out of a per-line region, and that is a
/// decision rather than a shortcut: a pipeline's region is destroyed to turn a dead reader into
/// `Gone`, and a stream this shell always drains to `OPERATION_EOF` has no dead reader to signal. One
/// rendezvous for the session costs one page and saves a SPLIT and a DESTROY per `date`.
pub(super) static DIAG_EP: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(u64::MAX);

/// The rendezvous a declaring child's second stream arrives on, minting it if this session has not
/// needed one yet. `None` when the budget cannot back it, and the caller then spawns without one:
/// a program whose diagnostic slot is empty says what it has to say in-band, which is what every
/// program did before §67.
pub(super) fn diag_rendezvous() -> Option<u64> {
    let held = DIAG_EP.load(core::sync::atomic::Ordering::Relaxed);
    if held != u64::MAX {
        return Some(held);
    }
    let ep = retype_rendezvous(BUDGET)?;
    DIAG_EP.store(ep, core::sync::atomic::Ordering::Relaxed);
    Some(ep)
}

// ---- the screen-narrowed tail's completion signal (DECISIONS §106) ----

/// **This shell's fault rendezvous for a screen-narrowed tail stage**, [`DIAG_EP`]'s shape reused for
/// a second purpose: minted once, kept for the session. At most one line is ever narrowed at a time
/// (a plan has one tail), so one rendezvous suffices and there is no `SPLIT`/`DESTROY` pair to pay per
/// invocation.
static SCREEN_EP: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(u64::MAX);

/// The rendezvous a screen-narrowed tail's exit is delivered to, minting it if this session has not
/// needed one yet. `None` when the budget cannot back it, and the caller then falls back to the
/// pre-§106 path: an unredirected `writes_while_reading` stage with nowhere for its completion
/// signal to arrive is refused by `check_chain` exactly as it always was, rather than spawned with
/// no way to know when it is done.
pub(super) fn screen_rendezvous() -> Option<u64> {
    let held = SCREEN_EP.load(core::sync::atomic::Ordering::Relaxed);
    if held != u64::MAX {
        return Some(held);
    }
    let ep = retype_rendezvous(BUDGET)?;
    SCREEN_EP.store(ep, core::sync::atomic::Ordering::Relaxed);
    Some(ep)
}

/// **Wait for a screen-narrowed tail to finish, then free its region.**
///
/// The completion signal is the child's death on `ep` (DECISIONS §26 (the fault endpoint)), and the
/// reap retries with a yield, as `job_undertaker::collect` does. See
/// `notes/swish-language/running.md`, "The screen-narrowed tail".
///
/// # BUGS
///
/// - The next `$ ` can interleave with `terminal_sink_caretaker`'s trailing `CALL`, the display race
///   DECISIONS §106 (an unredirected tail stage's output goes to the screen) accepts.
pub(super) fn await_screen(ep: u64) {
    let (_event, tid, ..) = receive_fault(ep);
    for _ in 0..SCREEN_REAP_ATTEMPTS {
        if reap(ep, tid) == 0 {
            return;
        }
        yield_now();
    }
    // Not fatal to this shell the way a stuck reap is to `job_undertaker`: worst case this leaks
    // one job's worth of the progenitor's pool, and the next `could not spawn (the progenitor is out of memory)` is the
    // visible symptom, exactly as it would be for job_undertaker's own exhaustion.
    failed();
    print(b"  that page's process would not collect; the progenitor's job budget may be short one job\n");
}

/// `job_undertaker::MAX_ATTEMPTS`'s reasoning, held here rather than shared: this shell reaps at
/// most one screen-narrowed corpse at a time and only ever the one it just watched for, where
/// `job_undertaker` reaps every ordinary job on the machine, so the two have no data to share and a
/// shared constant would imply a coupling that is not there.
const SCREEN_REAP_ATTEMPTS: usize = 1024;

/// **Drain every declaring stage's second stream into `dest`, then hand it back.**
///
/// Runs before the output is drained, the only order that cannot deadlock in `date | wc`, so a
/// declaring program must say everything before it produces anything (notes/pipes.md's BUGS).
/// `writers` is how many stages hold the rendezvous; each sends exactly one `OPERATION_EOF`, and
/// the drain ends at that count. See `notes/swish-language/running.md`, "The second stream".
pub(super) fn drain_diagnostics(dest: &mut dyn ByteOut, writers: usize) {
    let Some(ep) = diag_rendezvous() else { return };
    let mut done = 0usize;
    for _ in 0..MAX_OUTPUT_CHUNKS {
        if done == writers {
            break;
        }
        let (w0, w1, w2) = receive(ep);
        let mut buf = [0u8; byte_sink_contract::INLINE_MAX];
        match byte_sink_contract::unpack(w0, w1, w2, &mut buf) {
            byte_sink_contract::Msg::Bytes(n) => dest.push(&buf[..n]),
            byte_sink_contract::Msg::Eof => done += 1,
            // The progenitor's failure sentinel arrives here too, as an `OPERATION_EOF` it sends on this rendezvous so
            // this drain can end; anything else is a program that cannot spell the contract.
            byte_sink_contract::Msg::Malformed => done += 1,
        }
    }
    dest.finish();
}

/// **The most 16-byte chunks one program's output may take before this shell stops reading**: 64
/// KiB, and one number for every destination.
///
/// A bound on a bug, a program that never announces end of stream, and not a policy about length:
/// larger than anything a working program says (a rendered manual page is tens of kilobytes) and
/// smaller than forever. See `notes/swish-language/running.md`, "The output ceiling".
pub(super) const MAX_OUTPUT_CHUNKS: usize = 4096;

/// Report what the spawned program did, in terms of the grant it was given.
fn outcome(e: Endowment, answer: u64) {
    // **The only failure a spawned child reports back today**, which is why the sweep's stop rule
    // is honest about its reach: there is no exit status on this path, so a child that ran and did
    // the wrong thing looks to a sweep exactly like one that succeeded. See notes/glob-grant.md.
    if answer == spawnproto::SPAWN_FAILED {
        failed();
    }
    // **A job the kernel killed** (milestone 235). `Failed` rather than `Refused`: this shell was
    // willing and something did run, which is exactly the line [`failed`] draws.
    if answer == spawnproto::JOB_FAULTED {
        failed();
    }
    // **Bytes nobody vouched for** (DECISIONS §219). `Refused`, not `Failed`: nothing was
    // attempted and nothing broke; the progenitor declined, the way this shell declines a grant.
    if answer == spawnproto::SPAWN_UNVOUCHED || answer == spawnproto::SPAWN_REFUSED_BY_MANIFEST {
        refused();
    }
    swish::write_outcome(&e, answer, &mut print);
}

/// Resolve an invocation, then either refuse it at the prompt (a mismatch the manifest caught) or
/// spawn it, granting exactly what the command named and nothing else.
pub(super) fn run(nav: &mut Nav, cmd: &[u8], spec: ParsedInvocation) {
    // **A path runs bytes, not a name** (DECISIONS §219 (how the shell names an installed program to the spawner) option D, milestone 198 (a package manager) rung 3a). A program
    // the image names is a word with no `/` in it; anything with one is a file this shell reads and
    // hands the progenitor as frames. `installed/uptime` and `./a.out` both land here.
    if spec.program.contains(&b'/') {
        return run_image(nav, spec);
    }
    // **A bare word is the image's, an installed program's, or refused** (DECISIONS §229 (how a
    // bare name at the prompt reaches an installed program), B2). An installed program runs down
    // the same road its path does, so what it is granted and who vouched for it do not depend on
    // how it was named.
    match bare(nav, spec.program) {
        swish::bare::Bare::Installed { path, notice } => {
            // **The spawn line says both when a version set asked for a version that is not live**
            // (milestone 614 (two installed versions of one program, each runnable, and a caller
            // granted the one it needs), ruling 4): what runs, and what the set specified. A
            // preview ([`caps`]) says nothing ran, so it prints no notice.
            if let Some(notice) = notice {
                swish::bare::write_divergence(spec.program, &notice, &mut print);
            }
            let mut by_path = spec;
            by_path.program = path.as_bytes();
            return run_image(nav, by_path);
        }
        swish::bare::Bare::Both(path) => {
            refused();
            return swish::bare::write_both(spec.program, &path, &mut print);
        }
        swish::bare::Bare::Image | swish::bare::Bare::Unknown => {}
    }
    // **Expand first.** A pattern designates the names it matched, so the planner has to see the
    // set; and a pattern that matched nothing, or too much, is refused here with nothing spawned.
    let expanded = match expansion(nav, &spec) {
        Ok(e) => e,
        // A grant refusal is attributed to the program when there is one, because "rm: no name here
        // matches that pattern" reads better than the bare sentence and is just as true.
        Err(Say::Cannot(r)) => return refuse(spec, r),
        Err(said) => return say(said),
    };
    match grant_plan::plan(&spec, holdings(nav), expanded) {
        Err(refusal) => refuse(spec, refusal),
        // A supervised job runs under the two-tier ^C path (DECISIONS §24); a fast job is simply
        // spawned and waited on.
        Ok(endow) if endow.interruptible => spawn_interruptible(&mut nav.jobs, endow),
        // `wc report.txt` is `wc < report.txt`: the planner put the name in the source, so this is
        // a one-stage pipeline down the same path a `<` takes, and the child cannot tell the two apart.
        Ok(endow) => match endow.source {
            Source::File(g) => {
                // Cannot fail: this command came out of `cmd`.
                let Ok(l) = line::split(cmd) else { return };
                // One element, not `MAX_STAGES`: an `Endowment` carries a whole `NameSet` by value,
                // and a full-width array overflows the shell's stack (a data abort one word below
                // the lowest mapped stack page).
                run_pipeline(nav, l, &[Some(endow)], false, None, Some(g), None);
            }
            // **A program that hears words gets the line as its argv** (milestone 205,
            // DECISIONS §170 (how a foreign program is told what to do)). Assembled here, where a refusal can still name the program, and
            // before anything is sent, so a line that does not fit spawns nothing.
            _ if endow.program.manifest().program_argument.hears_words() => {
                // **A job that reads the console needs the line discipline** (milestone 892 (a std
                // program at the prompt holds the network and a resolver)): the terminal answers its
                // stdin read only in cooked mode, so the shell leaves raw mode first, as it does for a
                // supervised job, and the next prompt's editor takes it back.
                let mut verb = [0u8; grant_plan::MAX_VERB];
                let verb = grant_plan::verb_of(spec.line(), &mut verb);
                let program = endow.program;
                if grant_plan::narrow_by_verb(&program.manifest(), program.verbs(), verb)
                    .console_input
                {
                    super::prompt::leave_raw();
                }
                match assemble_argv(spec.line(), 0) {
                    Ok(a) => spawn(endow, Some(a)),
                    Err(Some(r)) => refuse(spec, r),
                    Err(None) => out_of_budget(),
                }
            }
            _ => spawn(endow, None),
        },
    }
}
