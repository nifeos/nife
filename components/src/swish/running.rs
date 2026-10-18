//! **Running a program**: the argv and name-set pages, the words grant, `run_image` and the
//! manifest note, `dir_grant`, `spawn`, draining what a child prints and its second stream, and
//! `outcome`.
//!
//! Moved whole out of `components/src/swish.rs` by milestone 844 (the shell program is split
//! along its seams), as §273 (the shell program becomes a directory of modules) lays out.
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
/// the frame in it. The frame travels to the progenitor narrowed to `READ` (`spawnproto::ARGS_BIT`)
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
/// The caretaker descends into the shell's current directory with the rights `grant` asks for
/// (`WordGrant::rights`; the progenitor clamps them to the manifest it endows, so an unvouched
/// program gets read-only whatever this says). Names: `fs_nameset_caretaker`, with the set on a
/// page. The whole directory (`.`): `fs_subtree_caretaker`, no page. The confined program's own
/// `START` words are zero, because a `std` program reads none.
///
/// `Err(Some(sentence))` is a grant this shell cannot deliver, `Err(None)` its budget.
///
/// # BUGS
///
/// **The set's filter can be raced on a real boot.** The caretaker forwards a checked name through
/// the file service's one shared frame, which this shell writes too while it drains a redirected
/// job (`std_grep needle docs > out.txt`). A write landing between the caretaker's re-staging and
/// the server's read opens a name the set never approved. `notes/a-set-grant-at-the-prompt.md`
/// has the case, `notes/shared-page-audit.md` finding 1 the mechanism; milestone 599 (a frame per
/// filesystem client channel)'s production pool closes it.
pub(super) fn words_grant(
    g: &GrantDir,
    grant: grant_plan::WordGrant,
    stage: usize,
) -> Result<WordsGrant, Option<&'static [u8]>> {
    if g.which != nav::Which::A {
        return Err(Some(
            b"  that directory is in a mounted tree, and the progenitor builds a caretaker\n  from the root tree; a grant there is not built\n",
        ));
    }
    let dir = match g.dir.depth() {
        1 => g.dir.component(0),
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
    if !filesystem_contract::grant::fits(dir) {
        return Err(Some(
            b"  that name does not fit in a grant's two argument words\n",
        ));
    }
    let (dir_lo, dir_hi) = filesystem_contract::grant::pack_name(dir);
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
        Some(_) => Ok(WordsGrant {
            words,
            set: Some(page),
        }),
        None => {
            release_argv(page, true);
            Err(Some(
                b"  that is more names than one grant carries (at most 8)\n",
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
/// and never given back. It exists because of how a region returns memory: a staging region's
/// pages go back to [`BUDGET`] on `DESTROY` only if it is the budget's most recent carve, and a
/// page table allocated from the budget *while* staging would sit above it and strand every
/// staging page for the life of the shell. Every mapping in the window takes its tables from this
/// region instead, so none comes from the budget after the first image. Tables, once, are the price
/// of having no unmap (DECISIONS §162 (whether a holder can give up a mapping)).
///
/// One last-level table per 2 MiB span the window touches, and one more for the level above in
/// case the window is the first thing this shell maps in its gigabyte. Until milestone 595 (the
/// shell runs a `std` program) raised the image cap past 2 MiB, one primer page mapped at the
/// window's base paid for its one table.
const IMAGE_TABLE_PAGES: u64 = (IMAGE_WINDOW_END - 1) / 0x20_0000 - IMAGE_WINDOW / 0x20_0000 + 2;

/// The region [`IMAGE_TABLE_PAGES`] describes, once split; [`u64::MAX`] until then.
static IMAGE_TABLES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(u64::MAX);

/// **Run a file by its bytes** (DECISIONS §219 option D, ruled 2026-09-26; milestone 198 rung 3a).
///
/// The shell reads the file through the file service it already holds, into frames split from its
/// own budget, and sends them on the spawn endpoint under `IMAGE_BIT`. The progenitor hashes its own
/// copy and looks the digest up in the installed-programs table: a hit runs with the manifest the bytes
/// carry. A miss runs only if this session holds the run-unvouched capability
/// ([`HOLDS_RUN_UNVOUCHED`], §219's gate D2), which this request then claims and presents as its
/// last message; the child gets what the line granted and the clock and configuration pages.
/// Otherwise the miss is refused with its own word ([`swish::UNVOUCHED_SENTENCE`]).
///
/// **The line is bound against the manifest the file carries** (milestone 597 (a program carries
/// its manifest in an ELF note), provisional), read by [`read_manifest_note`] before anything is
/// sent, or against `grant_plan::NO_NOTE_MANIFEST` when it carries none. So an argument the note
/// does not declare is refused here, exactly as `uptime 3` is, and a note that cannot be read or
/// asks for what an image cannot be given is refused with nothing sent. Whether the note is
/// honored is the progenitor's call, on its own copy (`grant_plan::image_manifest`).
///
/// One frame is held at a time: retyped, filled, delegated narrowed to `READ`, deleted. The staging
/// region is destroyed once the answer is in, which revokes the frames from both address spaces.
/// See `spawnproto`'s BUGS for what this first cut does not do.
pub(super) fn run_image(nav: &mut Nav, spec: RunSpec) {
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
        e.arg,
        if mem_slot.is_some() { e.mem_pages } else { 0 },
        spawnproto::Wiring {
            image: true,
            run_unvouched: holds_run_unvouched,
            args: argv.is_some(),
            dir: set_grant.is_some(),
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
    // The argv's frame follows the image's, before every other capability (`spawnproto::ARGS_BIT`).
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
    // The output's shape is the manifest's, never `e.prog`'s: an image's row is a stand-in
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
fn plan_image(nav: &mut Nav, spec: RunSpec) -> Option<PlannedImage> {
    if nav.dir.is_none() {
        say(Say::NoDirectory);
        return None;
    }
    let (t, handle, size) = open_for_bytes(nav, spec.prog)?;
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
    // **Expanded, or designated, once the note says which** (milestone 205): bytes that hear words
    // have their words designated rather than their patterns expanded, and only the note says
    // whether these do.
    let expanded = match m.arg {
        grant_plan::ArgSpec::Words(grant) => {
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
    spec: RunSpec,
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
    let mem_slot = if e.mem_pages > 0 {
        memory_region_split(e.mem_pages)
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
    let set_grant = match (e.dir, m.arg) {
        (Some(g), grant_plan::ArgSpec::Words(grant)) => match words_grant(&g, grant, 0) {
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
        },
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

/// **The manifest the open file `handle` carries in its own bytes** (milestone 597, provisional),
/// read the way the progenitor reads its copy: the `PT_NOTE` headers from the file's first page
/// (`elf::NoteSegments::from_head`), each note segment read whole and searched with the same
/// `elf::NoteSearch`, the descriptor decoded by `manifest_note::decode`. `Ok(None)` is a file with
/// no manifest note, and a note that is there and cannot be read is a refusal, as it is at the
/// progenitor.
///
/// **A file whose first page is not an ELF header with its program headers inside it reads as
/// carrying no note**, because that is not this reader's question to answer: the progenitor's
/// `Elf::parse` refuses what is not a program, and says so. The one real program that could land
/// here is one whose header table starts past its first page, which no linker script in this tree
/// produces; the line is then bound against the no-note default, and if the progenitor finds a
/// note the request does not fit, it refuses (`spawnproto::SPAWN_REFUSED_BY_MANIFEST`). A note
/// segment larger than one page is refused for the same fail-closed reason.
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
    let mut search = elf::NoteSearch::new(manifest_note::OWNER, manifest_note::MANIFEST);
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
            found = Some(manifest_note::decode(d).map_err(|_| Unreadable)?);
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
    let (p, _) = match nav.plan_path(path) {
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
            // A staging region sized to `pages` cannot run out before the last page, and this
            // loop holds one frame at a time, so this is a full capability table or a kernel
            // contradicting the budget. There is then no frame to send, and the progenitor will
            // wait for it: the prompt hangs. Recorded rather than papered over; see
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
/// Install, remove and rollback were this function's too, as the `package` builtin, until
/// milestone 809 (the package client becomes a program) moved them into `jig`, which asks on an
/// installer endpoint that refuses a vouch. So a vouch stays here, on the endpoint only the owner's
/// console holds. The shell never reads or writes `activation/` itself here; §221 ruled that this
/// prompt, the owner's console, may.
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
    let (lo, hi) = filesystem_contract::grant::pack_name(name);
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
pub(super) fn refuse(spec: RunSpec, refusal: Refusal) {
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

/// **Turn a planned directory grant into the two triples the progenitor needs**, or into the sentence that
/// says why this one cannot be delivered (milestone 31 phase 3).
///
/// The `Ok` half is the whole of the delivery: `(the caretaker's START words, the program's)`. The
/// caretaker is told the directory and the `filesystem_contract::dir` rights to ask for; the program is told
/// the operand and the options that were typed. Neither triple is decoded by anything between here
/// and the process it starts, which is why `spawnproto` can carry them without linking the
/// filesystem contract.
///
/// **The rights are computed from what was typed, and that is the point of `-r` being an option
/// rather than a mode.** Without it the capability is `REMOVE` alone: the program may take a name
/// out of this one directory and cannot enumerate it, read it, or walk into it. With it the
/// capability is `REMOVE_TREE`, which adds exactly the walk. Typing one letter visibly widens the
/// grant, and `caps rm -r logs` prints the difference before anything happens.
///
/// # The two shapes this cannot deliver, and neither is a permission
///
/// Both are facts about what a `fs_subtree_caretaker` is, so both are refused here with nothing
/// spawned rather than half-granted.
///
/// **A grant on the root of this shell's namespace.** A caretaker's whole attenuation is one
/// `OPENDIR` *into* the granted directory, and the root has no name to descend into: `filesystem_contract`'s
/// contract resolves a single component under a handle, and there is no verb for "the directory I
/// already hold, with fewer rights". So `rm gate.txt` typed at the top prompt cannot be narrowed at
/// all, and handing the program the unnarrowed service would be granting it the whole disk. The
/// answer is a design fork rather than a line of code (a `NARROW` verb on the contract, or a boot
/// whose shell is rooted one level down), and it is recorded in the roadmap rather than guessed at
/// here.
///
/// **A set of more than one name.** That grant is a `fs_nameset_caretaker`, which is a different
/// program taking its set in a frame; the progenitor builds the subtree one today. `rm *.txt` is planned,
/// previewed by `caps`, and refused at the point of delivery.
pub(super) fn dir_grant(g: &GrantDir, flags: u64) -> Result<DirWords, &'static [u8]> {
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
    let dir = match g.dir.depth() {
        0 => {
            return Err(
                b"  that names the root of this shell's namespace, and a caretaker is built by \n  descending into a directory; there is no name here to descend into\n",
            );
        }
        1 => g.dir.component(0),
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
    if !filesystem_contract::grant::fits(dir) || !filesystem_contract::grant::fits(name) {
        return Err(b"  that name does not fit in a grant's two argument words\n");
    }
    // What `-r` buys, stated as the difference between two capabilities rather than as a branch in
    // the program: REMOVE alone cannot even look at what is under the directory.
    let rights = if g.subtree {
        filesystem_contract::dir::REMOVE_TREE
    } else {
        filesystem_contract::dir::REMOVE
    };
    let (dir_lo, dir_hi) = filesystem_contract::grant::pack_name(dir);
    let (name_lo, name_hi) = filesystem_contract::grant::pack_name(name);
    Ok(DirWords {
        // `fs_subtree_caretaker::_start(name_lo, name_hi, spec)`.
        caretaker: (
            dir_lo,
            dir_hi,
            filesystem_contract::grant::spec(dir.len(), rights),
        ),
        // `rm::_start(spec, name_lo, name_hi)`, whose spec carries the options where a caretaker's
        // carries rights: both are "what this process was started with".
        child: (
            filesystem_contract::grant::spec(name.len(), flags),
            name_lo,
            name_hi,
        ),
    })
}

/// Grant and spawn. The one moment authority moves: split any memory grant off our own budget,
/// direct the progenitor to load the program, delegate the grant, and read the one answer that comes back.
pub(super) fn spawn(e: Endowment, argv: Option<Argv>) {
    // A grant this path cannot deliver must stop here, loudly. `plan` already refuses a `file:` when
    // `holdings().dir` is false, so today this is unreachable; it exists because the day that flips,
    // the thing that must NOT happen is a child spawned without the file the command named while the
    // prompt says nothing. Authority the user thought they granted must never quietly evaporate,
    // which is the same rule that makes an unexpected token a refusal instead of a shrug.
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
    let mem_slot = if e.mem_pages > 0 {
        match memory_region_split(e.mem_pages) {
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
    // (`spawnproto::ARGS_BIT`), `READ` only: the progenitor copies it and can neither write it nor
    // pass it on. The slot goes now; the region waits for the child's answer.
    if let Some(a) = argv {
        user_mode_runtime::send_capability(SPAWN, a.frame, abi::rights::READ, 0);
        cap_delete(a.frame);
    }
    // The name set's frame follows the argv's (`spawnproto::NAMESET_BIT`).
    if let Some(w) = &set_grant {
        send_set(w);
    }

    // If a budget rode along, delegate it now with every right so the progenitor can re-insert it into
    // the child (the progenitor narrows it there to `SPEND_ONLY`: the child spends it, it does not
    // lend it). Every right and not WRITE|GRANT since milestone 824: a budget's rights are what its
    // retypes carry.
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
    if e.prog.manifest().output.is_byte_stream() {
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
    // **The directory grant, which the progenitor delivers** (milestone 31 phase 3). This shell's file-service
    // rendezvous carries no GRANT, so it holds nothing it could hand a caretaker; what it can do is
    // say what the grant *is*, and the progenitor, which holds the service, builds a `fs_subtree_caretaker`
    // for it. [`dir_grant`] is where the shape of the grant meets the shape of what can be
    // delivered, and it returns the words rather than sending them so a refusal happens here, with
    // nothing spawned.
    // **A program that hears words is granted what they designated** (milestone 205), as a name
    // set or the whole directory, through `words_grant` rather than `rm`'s `dir_grant`.
    let mut set_grant = None;
    let dir_words = match (e.dir, e.prog.manifest().arg) {
        (None, _) => None,
        (Some(g), grant_plan::ArgSpec::Words(grant)) => match words_grant(&g, grant, 0) {
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
        },
        (Some(g), _) => match dir_grant(&g, e.short_options) {
            Ok(words) => Some(words),
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
    // The request: program id, argument, page count, and the operators' answer (grant_plan::spawnproto).
    //
    // **`diagnostics` is false here and always will be** (DECISIONS §67). This path runs a line with
    // no operators on it, so there is no `2>` and nothing for this shell to back; a program that
    // declares a second stream gets the terminal's own sink, which the progenitor endows from the manifest the
    // way it endows the clock. `2>` makes a line non-plain and goes down [`pipeline`].
    spawnproto::request(
        e.prog.id(),
        e.arg,
        e.mem_pages,
        spawnproto::Wiring {
            interruptible: false,
            sink: false,
            source: false,
            diagnostics: false,
            dir,
            // **Always false from this shell**: no manifest declares two directory operands, so
            // nothing here constructs a two-directory grant for a spawned program; see
            // `spawnproto::DIR2_BIT`'s own doc and notes/two-trees.md's BUGS.
            dir2: false,
            // **Also always false here** (DECISIONS §106), and for the same shape of reason as
            // `diagnostics`: a program only reaches this path (no file operand, no pipe, no
            // redirection at all) when `plan` put nothing in its source slot, and every program
            // that declares `writes_while_reading` also declares `InputSpec::Required`, which
            // sends it down [`run`]'s other arm (`Source::File`) instead. So this bit would never
            // have anything to narrow.
            screen: false,
            image: false,
            run_unvouched: false,
            args,
            nameset,
            machine: machine_wiring(e),
            // **"This is a graphical terminal launch", and nothing follows it** (milestone 715
            // (provisional)): the spawn service holds the display devices and this shell holds
            // none, so the bit is the whole request, and a boot with no display answers
            // `spawnproto::SPAWN_NO_DISPLAY`. Set only on a plain line, which is the only line
            // `graphical_terminal`'s manifest can reach this path on (`Words` output refuses `>`
            // and a pipe's left, `InputSpec::Forbidden` its right). `keyboard` is never set: the
            // spawn service decides the arm from what the boot granted.
            graphics: e.prog == grant_plan::Prog::GraphicalTerminal,
            keyboard: false,
        },
    )
}

/// **Drain a byte stream off the result rendezvous and print it**, which is what the shell does when
/// a program's output slot is the shell's own (`Sink::Report`, the default).
///
/// This used to stop at the first newline, because the framing was a convention rather than a
/// contract and there was nothing else to stop at. Milestone 50 gave it an end: it drains until
/// `OPERATION_EOF`, so a program that prints two lines prints two lines, and the rendezvous is left clean for
/// the next command instead of holding a message the next `receive` would mistake for an answer.
///
/// That change is not optional. Before it, a `date` that announced end of stream would leave that
/// message queued, and the next command's single read would take it.
pub(super) fn drain_text() {
    print(b"  ");
    for _ in 0..MAX_OUTPUT_CHUNKS {
        let (w0, w1, w2) = receive(RESULT);
        // Checked before decoding: the progenitor's sentinel is `u64::MAX`, whose top byte is an opcode this
        // contract does not define, so it would otherwise read as a malformed message rather than as
        // the one thing it is.
        if w0 == spawnproto::SPAWN_FAILED {
            print(b"could not spawn (the progenitor is out of memory)\n");
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
/// The kernel's own exit-delivery (DECISIONS §26 (the fault endpoint)) is the completion signal: `receive_fault` blocks
/// until the child is dead-until-reaped, which is stronger than "it painted its last line" (a dead
/// thread cannot enqueue any further `SEND`, so the child itself cannot still be mid-write here).
/// What can still be in flight is `terminal_sink_caretaker`'s own trailing `CALL` to `line_editor`,
/// which is the caretaker-hop display race DECISIONS §106 carries as a known, accepted interim
/// (tracked at milestone 151): the next `$ ` this shell prints can interleave with that call's
/// output. A display glitch, not a correctness or confinement failure.
///
/// The retry loop mirrors `job_undertaker::collect`'s reason: a corpse can be refused once if its
/// region still holds something schedulable, and yielding between attempts is what lets that settle
/// without spinning the only core there is.
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
/// It runs **before** the output is drained, and the order is forced rather than chosen. In
/// `date | wc` the shell is not the reader of `date`'s output (`wc` is), so a shell that drained the
/// output first would be waiting on `wc`, which is waiting on `date`, which is blocked in a
/// rendezvous `SEND` on this rendezvous. Diagnostics first is the only order in which nobody waits on
/// somebody who is waiting on them.
///
/// What that costs is a rule the declaring program has to keep, and it is in notes/pipes.md's BUGS:
/// **say everything you have to say before you produce anything.** `date` does, because its
/// complaints are all reasons it has no answer.
///
/// `writers` is how many stages were handed the rendezvous, and the drain ends when that many have
/// announced end of stream. Each declaring stage sends exactly one `OPERATION_EOF`, so the count is the
/// termination condition and no stage's silence can be mistaken for the line being over.
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
/// The ceiling is not a policy about how much a program may say. It is there so a program that
/// never announces end of stream costs a truncated answer instead of a prompt that never returns,
/// which is a bound on a **bug**, and a bug is not more tolerable when the bytes are going into a
/// file. There were two numbers here until milestone 40's viewer met the smaller one: printing was
/// capped at 32 chunks (512 bytes) while a `>` allowed 1024, so the same program's output was
/// silently cut at the terminal and whole in a file, and nothing said which had happened. The
/// reason on record for the split ("a file is where output goes when there is too much of it to
/// read") is a policy about length, which the same comment disclaimed in its next sentence.
///
/// 64 KiB rather than 16, because the number's whole job is to be larger than anything a working
/// program says and smaller than forever. A rendered manual page is tens of kilobytes; a few
/// seconds of serial output is the cost of hitting it, against a prompt that never comes back.
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
pub(super) fn run(nav: &mut Nav, cmd: &[u8], spec: RunSpec) {
    // **A path runs bytes, not a name** (DECISIONS §219 (how the shell names an installed program to the spawner) option D, milestone 198 (a package manager) rung 3a). A program
    // the image names is a word with no `/` in it; anything with one is a file this shell reads and
    // hands the progenitor as frames. `installed/uptime` and `./a.out` both land here.
    if spec.prog.contains(&b'/') {
        return run_image(nav, spec);
    }
    // **A bare word is the image's, an installed program's, or refused** (DECISIONS §229 (how a
    // bare name at the prompt reaches an installed program), B2). An installed program runs down
    // the same road its path does, so what it is granted and who vouched for it do not depend on
    // how it was named.
    match bare(nav, spec.prog) {
        swish::bare::Bare::Installed { path, notice } => {
            // **The spawn line says both when a version set asked for a version that is not live**
            // (milestone 614 (two installed versions of one program, each runnable, and a caller
            // granted the one it needs), ruling 4): what runs, and what the set specified. A
            // preview ([`caps`]) says nothing ran, so it prints no notice.
            if let Some(notice) = notice {
                swish::bare::write_divergence(spec.prog, &notice, &mut print);
            }
            let mut by_path = spec;
            by_path.prog = path.as_bytes();
            return run_image(nav, by_path);
        }
        swish::bare::Bare::Both(path) => {
            refused();
            return swish::bare::write_both(spec.prog, &path, &mut print);
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
        Ok(endow) if endow.interruptible => spawn_interruptible(endow),
        // **`wc report.txt`**, which is `wc < report.txt` with the operator left out: the planner
        // put the designated name in the *source* (`grant_plan`'s input operand), so this line is a
        // one-stage pipeline the shell feeds, and it runs down exactly the path a `<` runs down.
        // There is no second mechanism here, which is the reason to do it this way: the bytes reach
        // the child the same way, and the child still cannot tell a file from a pipe.
        Ok(endow) => match endow.source {
            Source::File(g) => {
                // The line reparses into the one stage it is. It cannot fail (this command came out
                // of it), and a `Line` is what `run_pipeline` reads its stage count off.
                let Ok(l) = line::split(cmd) else { return };
                // **One element, not `MAX_STAGES`**, and that is a stack decision rather than a
                // tidiness one. An `Endowment` carries a whole `NameSet` by value, so a full-width
                // array here overflowed the shell's stack at the *first* line this arm ran, which
                // presented as a data abort one word below the lowest mapped stack page. This line
                // is a single stage by construction, and `run_pipeline` takes a slice.
                run_pipeline(nav, l, &[Some(endow)], false, None, Some(g), None);
            }
            // **A program that hears words gets the line as its argv** (milestone 205,
            // DECISIONS §170 (how a foreign program is told what to do)). Assembled here, where a refusal can still name the program, and
            // before anything is sent, so a line that does not fit spawns nothing.
            _ if endow.prog.manifest().arg.hears_words() => match assemble_argv(spec.line(), 0) {
                Ok(a) => spawn(endow, Some(a)),
                Err(Some(r)) => refuse(spec, r),
                Err(None) => out_of_budget(),
            },
            _ => spawn(endow, None),
        },
    }
}
