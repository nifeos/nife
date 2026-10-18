//! **`caps`**: the shell's whole endowment, the preview of what a line would grant,
//! `caps_image`, the live generation table, and what a bare word names (`bare`).
//!
//! Moved whole out of `components/src/swish.rs` by milestone 844 (the shell program is split
//! along its seams), as §273 (the shell program becomes a directory of modules) lays out.
//!
//! Name: provisional 2026-10-11 (UTC), milestone 844's lane. calef deferred every module name
//! in §273 to a name-ratification pass.

use super::*;

/// Print the shell's whole endowment, or, with a tail, preview what that command would grant. This
/// is the introspection that makes "reading one literal tells you a process's authority" real.
///
/// The preview is [`swish::write_capabilities`], and it moved there because **nothing about it needs a
/// capability**: it renders what a line *would* grant, so it is the one command in this shell whose
/// entire meaning is checkable without a machine to run it on. What is left here is the directory
/// read a pattern needs and the terminal to print to.
pub(super) fn caps(nav: &mut Nav, tail: &[u8]) {
    if let Some(spec) = plain_line(tail) {
        if spec.prog.contains(&b'/') {
            return caps_image(nav, spec);
        }
        // A bare word previews what running it would do (§229 (how a bare name at the prompt
        // reaches an installed program), B2), through the same resolution [`run`] uses.
        match bare(nav, spec.prog) {
            swish::bare::Bare::Installed { path, .. } => {
                let mut by_path = spec;
                by_path.prog = path.as_bytes();
                return caps_image(nav, by_path);
            }
            swish::bare::Bare::Both(path) => {
                return swish::bare::write_both(spec.prog, &path, &mut print);
            }
            swish::bare::Bare::Image | swish::bare::Bare::Unknown => {}
        }
    }
    let holdings = holdings(nav);
    // The clock row is a *slot number*, and it comes from what this shell was told rather than from
    // a constant, for [`CLOCK_SLOT`]'s reason: printing a number this shell did not get would make
    // the one table that claims to be a complete reading into a story.
    let clock = match CLOCK_SLOT.load(core::sync::atomic::Ordering::Relaxed) {
        NO_CLOCK => None,
        slot => Some(slot),
    };
    // SAFETY: the progenitor maps the page at `SHELL_CONFIG_VA` read-only in the same build that
    // places its capability, and never unmaps it; [`HOLDS_CONFIG`] is that capability's presence.
    let config = HOLDS_CONFIG
        .load(core::sync::atomic::Ordering::Relaxed)
        .then(|| unsafe { environment_contract::ConfigPage::new(grant_plan::SHELL_CONFIG_VA) });
    swish::write_capabilities(
        tail,
        SH_BUDGET_PAGES,
        holdings,
        clock,
        config,
        &mut |token| nav.expand(token),
        &mut print,
    );
    // **And what is actually in the table** (milestone 715 (provisional)), under the shell's own
    // endowment only: the rows above are what this shell understands it was given, and the census
    // is the kernel's answer, so a slot no row names cannot hide. It is what `script/swish-check`
    // reads to prove this shell holds no display device.
    if grant_plan::trim(tail).is_empty() {
        swish::write_census(
            abi::fault::FAULT_EP_SLOT,
            &user_mode_runtime::is_granted,
            &mut print,
        );
    }
}

/// **The line is a plain run**: one stage, no operator. The shape [`run`] hands to [`run_image`]
/// when its program token has a `/` in it or names an installed program, so `caps` previews
/// exactly what would run.
fn plain_line(tail: &[u8]) -> Option<RunSpec<'_>> {
    let l = line::split(grant_plan::trim(tail)).ok()?;
    let [stage] = l.stages() else {
        return None;
    };
    if l.output.is_some() || l.input.is_some() || l.diagnostics.is_some() {
        return None;
    }
    match grant_plan::parse(stage) {
        Command::Run(spec) => Some(spec),
        _ => None,
    }
}

/// **`caps <path>`** (DECISIONS §219 D and D2; the manifest note, milestone 597, provisional):
/// read the file's manifest note and bind the line against it as [`run_image`] would, hash the
/// file's bytes as the progenitor will, look the digest up in the live generation, and say what
/// would be granted, what the note asked for, and on whose word. Nothing is sent to the progenitor.
fn caps_image(nav: &mut Nav, spec: RunSpec) {
    if nav.dir.is_none() {
        return say(Say::NoDirectory);
    }
    let Some((t, handle, size)) = open_for_bytes(nav, spec.prog) else {
        return;
    };
    let declared = match read_manifest_note(t.slot, handle, size) {
        Ok(d) => d,
        Err(r) => {
            nav.close(handle);
            return print(swish::image_refusal_sentence(r));
        }
    };
    let m = declared.unwrap_or(grant_plan::NO_NOTE_MANIFEST);
    if !grant_plan::image_can_carry(&m) {
        nav.close(handle);
        return print(swish::image_refusal_sentence(
            grant_plan::ImageRefusal::NotCarried,
        ));
    }
    // **Expanded, or designated, once the note says which** (milestone 205): bytes that hear words
    // have their words designated rather than their patterns expanded, and only the note says
    // whether these do.
    let expanded = match m.arg {
        grant_plan::ProgramArgumentDeclaration::Words(grant) => {
            swish::designation(spec.line(), grant, &mut |token| nav.expand(token))
        }
        _ => expansion(nav, &spec),
    };
    let expanded = match expanded {
        Ok(e) => e,
        Err(said) => {
            nav.close(handle);
            return match said {
                Say::Cannot(r) => swish::write_refusal(&spec, r, &mut print),
                said => swish::write_say(said, &mut print),
            };
        }
    };
    let e = match grant_plan::plan_against(&spec, grant_plan::IMAGE_ROW, m, holdings(nav), expanded)
    {
        Ok(e) => e,
        Err(r) => {
            nav.close(handle);
            return swish::write_refusal(&spec, r, &mut print);
        }
    };
    let mut hash = measured_boot::Sha256::new();
    let mut chunk = [0u8; 256];
    let mut at = 0u64;
    let mut read_ok = true;
    while at < size {
        let n = call(t.slot, fs::req(fs::READ, handle, PAGE), at).0 as i64;
        if n <= 0 {
            read_ok = false;
            break;
        }
        let mut done = 0usize;
        while done < n as usize {
            let take = (n as usize - done).min(chunk.len());
            get_page_at(done, &mut chunk[..take]);
            hash.update(&chunk[..take]);
            done += take;
        }
        at += n as u64;
    }
    nav.close_in(t, handle);
    if !read_ok {
        failed();
        return print(b"  this shell could not read the whole file, so it cannot say what it is\n");
    }
    let digest = hash.finalize();
    let text = measured_boot::digest_text(&digest);
    let vouched_by = live_generation_listing(nav, &digest);
    swish::write_image_capabilities(
        spec.prog,
        &text,
        vouched_by,
        HOLDS_RUN_UNVOUCHED.load(core::sync::atomic::Ordering::Relaxed),
        declared,
        &e,
        &nav.holds,
        &mut print,
    );
}

/// One page of a generation table, read by [`live_generation_listing`]. A static rather than a
/// local because a page on the stack would take `caps`'s frame over the 4 KiB guard page
/// `script/stack-frame-check` holds every frame to; [`RANKED`] is here for the same reason.
pub(super) static mut GENERATION_TABLE: [u8; filesystem_contract::PAGE] =
    [0; filesystem_contract::PAGE];

/// One page for the nearest version set, a static for [`GENERATION_TABLE`]'s reason. Provisional;
/// the file it holds (`.jig-versions`) was ratified 2026-10-07, and this static's name was not.
static mut VERSION_SET: [u8; filesystem_contract::PAGE] = [0; filesystem_contract::PAGE];

/// **The nearest version set at or above where this shell stands** (milestone 614 (two installed
/// versions of one program, each runnable, and a caller granted the one it needs), ruling 4):
/// `swish::jig_versions::FILE` here, then in each parent up to the root, read with the directory
/// handles this shell already holds. asdf walks the same way, on calef's usability ruling; the walk
/// is over directories for one file name and not over program names, so §229 stands. `None` when
/// there is none, or one that is not text, or one that fills a page.
fn nearest_version_set(nav: &Nav) -> Option<&'static str> {
    nav.dir?;
    let (which, pos) = nav.holds.locate(&nav.holds.cwd);
    let t = nav.tree(which);
    // SAFETY: this shell is one thread and `VERSION_SET` is used here and nowhere else, so no
    // other reference to it can exist while this one does; the returned borrow is dropped before
    // the next read of the static, because [`bare`] resolves inside this one's scope.
    let set = unsafe { &mut *core::ptr::addr_of_mut!(VERSION_SET) };
    for level in (0..=pos.depth()).rev() {
        if let Some(n) = read_named(
            nav,
            t,
            nav.at(level),
            swish::jig_versions::FILE.as_bytes(),
            set,
        ) && n < set.len()
        {
            return core::str::from_utf8(&set[..n]).ok();
        }
    }
    None
}

/// **The live generation, if it lists `digest`, and whether the owner vouched for it**, looked up
/// with the progenitor's own `installed_programs::lookup_digest` in [`with_live_table`]'s read.
fn live_generation_listing(nav: &Nav, digest: &measured_boot::Digest) -> Option<swish::Vouched> {
    with_live_table(
        nav,
        |text, number| match installed_programs::lookup_digest(text, digest) {
            Ok(Some(entry)) => Some(swish::Vouched {
                generation: number,
                by_owner: entry.package == installed_programs::OWNER,
            }),
            _ => None,
        },
    )
}

/// **Read the live generation and hand it to `f`**, with its number: `activation/current`, then the
/// generation it names, read as the progenitor reads them (`crates/system_initializer`'s
/// `FsCalls::live_generation`). `None` on every way of failing to read it, which is the
/// progenitor's rule too: a table that cannot be read vouches for nothing and names nothing.
pub(super) fn with_live_table<R>(nav: &Nav, f: impl FnOnce(&str, u32) -> Option<R>) -> Option<R> {
    nav.dir?;
    // The first tree's, whichever tree this shell stands in: the activation table is the system's.
    let t = nav.first();
    let act = nav.name_call_in(
        t,
        fs::OPENDIR,
        fs::ROOT,
        installed_programs::DIRECTORY.as_bytes(),
        filesystem_contract::dir::READ,
    );
    if act < 0 {
        return None;
    }
    let act = act as u64;
    let found = (|| {
        let mut line = [0u8; 16];
        let n = read_named(
            nav,
            t,
            act,
            installed_programs::CURRENT.as_bytes(),
            &mut line,
        )?;
        let number = core::str::from_utf8(&line[..n])
            .ok()
            .and_then(installed_programs::parse_current)?;
        let mut name = [0u8; 10];
        let generation = installed_programs::generation_name(number, &mut name);
        // SAFETY: this shell is one thread and `GENERATION_TABLE` is used here and nowhere else,
        // so no other reference to it can exist while this one does.
        let table = unsafe { &mut *core::ptr::addr_of_mut!(GENERATION_TABLE) };
        let n = read_named(nav, t, act, generation.as_bytes(), table)?;
        // A page and not a byte more, the progenitor's rule: a full page may have been cut short.
        if n >= table.len() {
            return None;
        }
        f(core::str::from_utf8(&table[..n]).ok()?, number)
    })();
    nav.close_in(t, act);
    found
}

/// **What a bare word names** (DECISIONS §229 (how a bare name at the prompt reaches an installed
/// program), B2), from the image's programs and the live table, refined by the nearest version set
/// when it names the program (milestone 614, ruling 4). See `swish::bare`.
pub(super) fn bare(nav: &Nav, word: &[u8]) -> swish::bare::Bare {
    let image = grant_plan::Prog::from_name(word).is_some();
    with_live_table(nav, |text, _| {
        // The set is consulted only for a name the table answers at all: it selects among
        // installed versions, so for anything else it can only change nothing, and the reads are
        // saved.
        let set = core::str::from_utf8(word)
            .ok()
            .and_then(|name| installed_programs::lookup(text, name).ok().flatten())
            .and_then(|_| nearest_version_set(nav));
        Some(swish::bare::resolve(word, image, Some(text), set))
    })
    .unwrap_or_else(|| swish::bare::resolve(word, image, None, None))
}

/// Open `name` under the directory handle `at` in tree `t`, read up to `out.len()` bytes of it (one `READ`,
/// so at most a page), close it. The byte count, or `None`.
pub(super) fn read_named(
    nav: &Nav,
    t: Tree,
    at: u64,
    name: &[u8],
    out: &mut [u8],
) -> Option<usize> {
    let h = nav.name_call_in(t, fs::OPEN, at, name, 0);
    if h < 0 {
        return None;
    }
    let want = (out.len() as u64).min(PAGE);
    let n = call(t.slot, fs::req(fs::READ, h as u64, want), 0).0 as i64;
    nav.close_in(t, h as u64);
    let n = usize::try_from(n).ok()?.min(out.len());
    get_page_at(0, &mut out[..n]);
    Some(n)
}

/// Copy `out.len()` bytes out of the file page, starting `from` bytes in.
pub(super) fn get_page_at(from: usize, out: &mut [u8]) {
    for (i, b) in out.iter_mut().enumerate() {
        *b = FS_WINDOW.r8((from + i) as u64);
    }
}
