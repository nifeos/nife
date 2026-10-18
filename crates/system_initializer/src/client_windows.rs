//! The file service's client windows: one frame per channel, the bound and job channels built on
//! them, and `FsCalls`. Moved out of `lib.rs` unchanged but for visibility by milestone 843 (the
//! system-initializer crate root is split along its seams), under §279 (the system-initializer
//! crate root splits into private modules).
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

use super::*;

// -------------------------------------------------------------------------------------------
// The file service's client windows (milestone 599 (a frame per filesystem client channel),
// calef's ruling of 2026-09-27, option 4 of notes/a-frame-per-filesystem-client-channel.md).
// -------------------------------------------------------------------------------------------

/// **One page of window `w` of the file service's pool**, as a capability in a fresh slot, or
/// `None` if the kernel refused. The progenitor's `fs_page` names every window as one run (the
/// kernel grants it that way at slot 6), so it can never be mapped into a client as it is: that
/// would hand the client every other client's channel, the bug the pool exists to fix. What a
/// client maps is a slice, made here, mapped with `MAP_INTO`, and deleted by the caller as soon as
/// the build is done. One page, not the window's sixteen, because a client of this contract maps
/// one page (`filesystem_contract::fs::TRANSFER_PAGES` says a client maps what it uses).
pub(super) fn window_page(pool: u64, w: u64) -> Option<u64> {
    let first = w * fs_operation::TRANSFER_PAGES as u64;
    // SAFETY: the syscall; the kernel validates the capability, the range and the right.
    let slot = unsafe { invoke(pool, abi::page_frame::SLICE, first, 1, 0) };
    (slot >= 0).then_some(slot as u64)
}

/// **A copy of endpoint `ep` badged `badge`**, in a fresh slot the caller deletes (§230 (badged
/// endpoint capabilities)). `Err` if the kernel refused: `ep` lacks `GRANT`, is already badged, or
/// the table is full.
pub(super) fn badged(ep: u64, badge: u64) -> Result<u64, ()> {
    // SAFETY: the syscall; the kernel validates the capability, the right and the badge.
    let slot = unsafe { invoke(ep, abi::rendezvous::BADGE, badge, 0, 0) };
    if slot >= 0 { Ok(slot as u64) } else { Err(()) }
}

/// **The file service's endpoint badged with `w`**, which is how the server knows which window
/// a request's bytes are in (§230 (badged endpoint capabilities)). `None` if the kernel refused.
fn window_endpoint(ep: u64, w: u64) -> Option<u64> {
    // SAFETY: as [`window_page`].
    let slot = unsafe { invoke(ep, abi::rendezvous::BADGE, w, 0, 0) };
    (slot >= 0).then_some(slot as u64)
}

/// **Window 0, the default channel**: the unbadged endpoint and one page of the first window. For
/// the long-lived clients the boot builds once (the shell, `login`, the identity provisioner),
/// which have always shared it with each other and with this process's own activation calls. The
/// page is a fresh slice the caller deletes; the endpoint is the progenitor's own, not a copy.
pub(super) fn window_zero(fs: Fs) -> Option<Fs> {
    Some(Fs {
        ep: fs.ep,
        page: window_page(fs.page, 0)?,
    })
}

/// **The pool of windows directory-granted jobs get** (milestone 599, the rule since milestone 685
/// (a job is finished when its memory is back)): windows `1..login_contract::DURABLE_WINDOW`, each
/// held by the job it was handed to until `job_undertaker` says that job is reaped. Window 0 is the
/// boot's long-lived clients' (see [`window_zero`]), and the last is `login`'s durable sessions'
/// (milestone 152).
///
/// The bookkeeping and its host test live in `grant_plan::job_windows`; this process owns the
/// capabilities and the binding.
///
/// # BUGS
///
/// - **A bound window is taken back when it is reused, not when its job is reaped** (milestone 606
///   (a directory walk costs what it does on Linux), ruling D's default "UNBIND at reap"): `UNBIND`
///   runs in [`FsCalls::unbind`], just before the window is handed out again. Since milestone 685
///   that is always after the reap, so no live job holds the old badge when the window is bound to
///   a new root. Until then a window came round again after `DURABLE_WINDOW - 1` more granted jobs
///   whether or not its holder lived, and milestone 633 (an outside agent attacks the confinement
///   claim)'s second pass read the consequence on 2026-10-05 (UTC): the old job's badge resolved
///   `ROOT` to the new job's directory, and its still-mapped window showed the new job's requests
///   and file bytes. Confinement claim 24's row in `notes/confinement-claims.md` records the fix.
///   What remains is that a reaped job's binding outlives it until the window's next take, which
///   costs the file server a binding and grants nothing to anybody.
/// - **A window whose job is never reaped by `job_undertaker` is never free again**: see
///   `grant_plan::job_windows`'s `BUGS` (a screen-narrowed job behind a directory grant, which no
///   manifest asks for today).
pub(super) const fn job_windows() -> Windows {
    Windows::new(1, login_contract::DURABLE_WINDOW)
}

/// **Whether a file server's image says it enforces subtree grants itself** (milestone 606, ruling
/// T1): it carries a `capability_manifest_note::SUBTREE_GRANTS` note naming the one scope crate there is. No
/// image, no note, or a note that does not decode all answer `false`.
pub(super) fn subtree_grants(server: Option<&elf::Elf<'_>>) -> bool {
    let Some(server) = server else {
        return false;
    };
    matches!(
        server.note(capability_manifest_note::OWNER, capability_manifest_note::SUBTREE_GRANTS),
        Ok(Some(d)) if capability_manifest_note::decode_subtree_grants(d)
            == Ok(capability_manifest_note::Scope::SubtreeScope)
    )
}

/// **A bound channel for one job behind a directory grant** (milestone 606, rulings D and T1): the
/// next window, its badge bound by the file server to the directory the grant names, a page of the
/// window, and the endpoint badged to match. What a caretaker would have done at its start, done
/// once here instead: open the named directory asking for exactly the granted rights. The order is
/// the safety property. A badge is the endpoint's whole authority until it is bound, so the job is
/// handed nothing until `BIND` has answered. `None` for any failure, which the caller answers with
/// a caretaker.
pub(super) fn bound_channel(
    fs: Fs,
    (windows, w): (&mut Windows, u64),
    own_ut: u64,
    mapped: &mut bool,
    (lo, hi, spec): (u64, u64, u64),
) -> Option<Fs> {
    let files = FsCalls::map(Some(fs), own_ut, mapped)?;
    let mut buf = [0u8; filesystem_contract::grant::MAX_NAME];
    let n = filesystem_contract::grant::unpack_name(
        lo,
        hi,
        filesystem_contract::grant::spec_len(spec),
        &mut buf,
    );
    let name = core::str::from_utf8(&buf[..n]).ok()?;
    files.unbind(windows, w);
    let handle = files.named(
        fs_operation::OPENDIR,
        fs_operation::ROOT,
        name,
        filesystem_contract::grant::spec_rights(spec),
    );
    bind_window(fs, &files, (windows, w), handle)
}

/// **Bind window `w`'s badge to the directory `handle` names, and hand back the window's page and
/// endpoint**: the half [`bound_channel`] and [`bound_index_store`] share once each has opened its
/// directory. A negative `handle` is a failure already; a refused `BIND` closes it.
fn bind_window(
    fs: Fs,
    files: &FsCalls,
    (windows, w): (&mut Windows, u64),
    handle: i64,
) -> Option<Fs> {
    if handle < 0 {
        return None;
    }
    if call(
        fs.ep,
        fs_operation::req(fs_operation::BIND, handle as u64, 0),
        w,
    )
    .0 != 0
    {
        files.close(handle);
        return None;
    }
    windows.set_bound(w, true);
    // SAFETY: as [`job_channel`]'s zeroing: window `w`'s first page, inside the mapped pool.
    unsafe {
        core::ptr::write_bytes(
            (ACTIVATION_FS_VA + w * fs_operation::TRANSFER_MAX as u64) as *mut u8,
            0,
            PAGE_BYTES,
        );
    }
    let page = window_page(fs.page, w)?;
    let Some(ep) = window_endpoint(fs.ep, w) else {
        cap_delete(page);
        return None;
    };
    Some(Fs { ep, page })
}

/// **A bound channel to the machine's index store** (milestone 801 (packages over the internet),
/// ruling I2): [`bound_channel`]'s shape for the one directory the progenitor names rather than
/// the line, `activation/indexes`, made if it is not there yet and then opened with `rights`. It
/// lives under `activation/` because what it holds decides what installs, the same trust as the
/// generations beside it, and a path the progenitor names cannot collide with a package's name the
/// way `packages/indexes` would.
///
/// Name: provisional 2026-10-12 (UTC), lane `milestone/801-jig-installs-from-the-index`, as is
/// the directory's name.
pub(super) fn bound_index_store(
    fs: Fs,
    (windows, w): (&mut Windows, u64),
    own_ut: u64,
    mapped: &mut bool,
    rights: u64,
) -> Option<Fs> {
    let files = FsCalls::map(Some(fs), own_ut, mapped)?;
    files.unbind(windows, w);
    let act = files.directory(fs_operation::ROOT, installed_programs::DIRECTORY);
    if act < 0 {
        return None;
    }
    // Made with every right if it is not there, then opened again with only the verb's.
    let made = files.directory(act as u64, package_index::STORE_DIRECTORY);
    if made >= 0 {
        files.close(made);
    }
    let handle = files.named(
        fs_operation::OPENDIR,
        act as u64,
        package_index::STORE_DIRECTORY,
        rights,
    );
    files.close(act);
    bind_window(fs, &files, (windows, w), handle)
}

/// **A sync-only endpoint for one job** (milestone 805 (`reboot` at the prompt),
/// `fs::BIND_SYNC`): window `w`'s badge, unbound from whatever it last held, bound sync-only, and
/// the endpoint badged with it in a fresh slot the caller deletes once the job holds its copy. The
/// window is marked bound, so the next job to take it unbinds it first, as for a directory grant.
///
/// Name: ratified 2026-10-06 (calef, #1783: "Approve sync for the file-server
/// request and its capability, and flush for the device cache only.").
pub(super) fn bind_sync_only(
    fs: Fs,
    windows: &mut Windows,
    w: u64,
    own_ut: u64,
    mapped: &mut bool,
) -> Option<u64> {
    let files = FsCalls::map(Some(fs), own_ut, mapped)?;
    files.unbind(windows, w);
    if call(fs.ep, fs_operation::req(fs_operation::BIND_SYNC, 0, 0), w).0 != 0 {
        return None;
    }
    windows.set_bound(w, true);
    window_endpoint(fs.ep, w)
}

/// **A channel of its own for one job behind a directory grant**: window `w`'s page and the
/// endpoint badged with it, both fresh slots the caller deletes once the job and its caretaker are
/// built. The window's first page is zeroed first, through this process's own mapping of the pool
/// (made once, [`FsCalls::map`]), so nothing the window's last job left there is visible to the
/// next: the same "no stale RAM across a share" rule the kernel's own pool follows.
pub(super) fn job_channel(
    fs: Fs,
    (windows, w): (&mut Windows, u64),
    own_ut: u64,
    mapped: &mut bool,
) -> Option<Fs> {
    let files = FsCalls::map(Some(fs), own_ut, mapped)?;
    // A window last used by a bound grant goes back to being open before a caretaker uses it.
    files.unbind(windows, w);
    // SAFETY: the pool is mapped read/write at ACTIVATION_FS_VA (`FsCalls::map`, whole run), and
    // window `w`'s first page lies inside it; no client is using window `w` between two jobs.
    unsafe {
        core::ptr::write_bytes(
            (ACTIVATION_FS_VA + w * fs_operation::TRANSFER_MAX as u64) as *mut u8,
            0,
            PAGE_BYTES,
        );
    }
    let page = window_page(fs.page, w)?;
    let Some(ep) = window_endpoint(fs.ep, w) else {
        cap_delete(page);
        return None;
    };
    Some(Fs { ep, page })
}

/// **The progenitor's calls on the file service**, through the page it maps at
/// [`ACTIVATION_FS_VA`]. Shared by [`vouched`], which reads the installed-programs table, and [`activate`](activation::activate),
/// which writes it (milestone 198 rung 3a).
///
/// What is mapped there is the whole client-window pool (milestone 599), and these calls use its
/// first page, window 0, the one the shell shares. That is sound for the reason `build_caretaker`
/// gives: the shell is parked in its `RECEIVE` on the result endpoint for the whole of a spawn or an
/// activation, so nothing else is mid-request on it. A job's caretaker has its own window now.
pub(super) struct FsCalls {
    ep: u64,
}

impl FsCalls {
    /// Map the file page here, once, on first use. `None` on a boot with no disk, or if the map
    /// was refused.
    pub(super) fn map(fs: Option<Fs>, own_ut: u64, mapped: &mut bool) -> Option<Self> {
        let fs = fs?;
        if !*mapped {
            // SAFETY: the syscall; read/write because names and data are written into this page.
            *mapped =
                unsafe { invoke(fs.page, abi::page_frame::MAP, ACTIVATION_FS_VA, 1, own_ut) } == 0;
        }
        mapped.then_some(Self { ep: fs.ep })
    }

    fn page() -> *mut u8 {
        ACTIVATION_FS_VA as *mut u8
    }

    /// A name-taking verb: the name goes at the page's start, its length in the request.
    pub(super) fn named(&self, verb: u64, handle: u64, name: &str, w1: u64) -> i64 {
        let len = name.len().min(PAGE_BYTES);
        // SAFETY: the page is mapped read/write (`map`) and `len` is at most a page.
        unsafe { core::ptr::copy_nonoverlapping(name.as_ptr(), Self::page(), len) };
        call(self.ep, fs_operation::req(verb, handle, len as u64), w1).0 as i64
    }

    /// **Take window `w`'s grant back, if it has one** (milestone 606, ruling D): `UNBIND` closes
    /// every handle the badge minted and leaves it revoked until it is bound again. A no-op for a
    /// window that was never bound.
    fn unbind(&self, windows: &mut Windows, w: u64) {
        if windows.is_bound(w) {
            call(self.ep, fs_operation::req(fs_operation::UNBIND, 0, 0), w);
            windows.set_bound(w, false);
        }
    }

    pub(super) fn close(&self, handle: i64) {
        call(
            self.ep,
            fs_operation::req(fs_operation::CLOSE, handle as u64, 0),
            0,
        );
    }

    /// Read a file from offset 0 into `out`, up to `out.len()` bytes (at most a page). `None` on an
    /// error.
    fn read(&self, handle: u64, out: &mut [u8]) -> Option<usize> {
        let want = out.len().min(PAGE_BYTES);
        let n = call(
            self.ep,
            fs_operation::req(fs_operation::READ, handle, want as u64),
            0,
        )
        .0 as i64;
        if n < 0 {
            return None;
        }
        let n = (n as usize).min(want);
        // SAFETY: the server wrote `n` bytes (at most `want`, at most a page) at the page's start.
        unsafe { core::ptr::copy_nonoverlapping(Self::page(), out.as_mut_ptr(), n) };
        Some(n)
    }

    /// Replace a file's contents with `bytes`, a page at a time. `false` at the first short or
    /// refused write.
    pub(super) fn replace(&self, handle: u64, bytes: &[u8]) -> bool {
        if call(
            self.ep,
            fs_operation::req(fs_operation::TRUNCATE, handle, 0),
            0,
        )
        .0 as i64
            != 0
        {
            return false;
        }
        for (i, chunk) in bytes.chunks(PAGE_BYTES).enumerate() {
            // SAFETY: the page is mapped read/write and a chunk is at most a page.
            unsafe { core::ptr::copy_nonoverlapping(chunk.as_ptr(), Self::page(), chunk.len()) };
            let at = (i * PAGE_BYTES) as u64;
            let n = call(
                self.ep,
                fs_operation::req(fs_operation::WRITE, handle, chunk.len() as u64),
                at,
            )
            .0 as i64;
            if n != chunk.len() as i64 {
                return false;
            }
        }
        true
    }

    /// Open `name` in `parent` for writing, creating it if it is not there.
    pub(super) fn open_or_create(&self, parent: u64, name: &str) -> i64 {
        let h = self.named(fs_operation::OPEN, parent, name, 0);
        if h >= 0 {
            h
        } else {
            self.named(fs_operation::CREATE, parent, name, 0)
        }
    }

    /// Descend into `name` in `parent` with every right, making it if it is not there.
    pub(super) fn directory(&self, parent: u64, name: &str) -> i64 {
        let h = self.named(fs_operation::OPENDIR, parent, name, dir::ALL);
        if h >= 0 {
            h
        } else {
            self.named(fs_operation::MKDIR, parent, name, dir::ALL)
        }
    }

    /// Whether generation `number` has a file in the activation directory.
    pub(super) fn generation_exists(&self, act: u64, number: u32) -> bool {
        let mut name = [0u8; 10];
        let h = self.named(
            fs_operation::OPEN,
            act,
            installed_programs::generation_name(number, &mut name),
            0,
        );
        if h >= 0 {
            self.close(h);
        }
        h >= 0
    }

    /// **The live generation**: its number and its table, read into `out`. `(0, 0)` when there is
    /// no `current` at all, which is a disk nothing was ever installed on and vouches for nothing.
    /// `Err` when there is a `current` and it, or the generation it names, cannot be read or is
    /// larger than a page: a table that cannot be read vouches for nothing, and must not be
    /// written over either.
    pub(super) fn live_generation(
        &self,
        act: u64,
        out: &mut [u8; PAGE_BYTES],
    ) -> Result<(u32, usize), ()> {
        let c = self.named(fs_operation::OPEN, act, installed_programs::CURRENT, 0);
        if c < 0 {
            return Ok((0, 0));
        }
        let mut line = [0u8; 16];
        let got = self.read(c as u64, &mut line);
        self.close(c);
        let number = got
            .and_then(|n| core::str::from_utf8(&line[..n]).ok())
            .and_then(installed_programs::parse_current)
            .ok_or(())?;
        Ok((number, self.generation(act, number, out)?))
    }

    /// **Generation `number`'s table**, read whole into `out`; its length. `Err` if it cannot be
    /// opened or read, or fills the page.
    /// **The copy of the image's index** in the index store under `act` (milestone 801 (packages
    /// over the internet)), read into `out`. `Err` when there is none, or when it fills the page,
    /// for [`FsCalls::generation`]'s reason: a copy cut short vouches for nothing.
    pub(super) fn index_copy(&self, act: u64, out: &mut [u8; PAGE_BYTES]) -> Result<usize, ()> {
        let store = self.named(
            fs_operation::OPENDIR,
            act,
            package_index::STORE_DIRECTORY,
            dir::READ,
        );
        if store < 0 {
            return Err(());
        }
        let t = self.named(
            fs_operation::OPEN,
            store as u64,
            package_index::IMAGE_INDEX_COPY,
            0,
        );
        self.close(store);
        if t < 0 {
            return Err(());
        }
        let n = self.read(t as u64, &mut out[..]);
        self.close(t);
        match n {
            Some(n) if n <= package_index::COPY_MAX => Ok(n),
            _ => Err(()),
        }
    }

    pub(super) fn generation(
        &self,
        act: u64,
        number: u32,
        out: &mut [u8; PAGE_BYTES],
    ) -> Result<usize, ()> {
        let mut name = [0u8; 10];
        let t = self.named(
            fs_operation::OPEN,
            act,
            installed_programs::generation_name(number, &mut name),
            0,
        );
        if t < 0 {
            return Err(());
        }
        let n = self.read(t as u64, &mut out[..]);
        self.close(t);
        // A page and not a byte more: a generation that fills one may have been cut short, and a
        // table that ends mid-line vouches for nothing. `installed_programs`'s BUGS: forty entries fit.
        match n {
            Some(n) if n < PAGE_BYTES => Ok(n),
            _ => Err(()),
        }
    }

    /// **Write generation `number` whole, then make it live, then make that durable.** The order
    /// is the whole of the crash story: a generation file is never rewritten, `current` names only
    /// a generation that was written completely, and the flip itself is a `RENAME` of a staged
    /// `current` onto the real one, so a reader meets the old line or the new one and never half
    /// of each. `table` is `None` for a rollback, whose generation already exists.
    pub(super) fn commit(&self, act: u64, number: u32, table: Option<&[u8]>) -> bool {
        if let Some(table) = table {
            let mut name = [0u8; 10];
            let g = self.named(
                fs_operation::CREATE,
                act,
                installed_programs::generation_name(number, &mut name),
                0,
            );
            if g < 0 {
                return false;
            }
            let written = self.replace(g as u64, table);
            self.close(g);
            if !written {
                return false;
            }
        }
        let mut line = [0u8; 16];
        let Ok(n) = installed_programs::format_current(number, &mut line) else {
            return false;
        };
        let staged = self.open_or_create(act, installed_programs::CURRENT_STAGED);
        if staged < 0 {
            return false;
        }
        let written = self.replace(staged as u64, &line[..n]);
        self.close(staged);
        if !written {
            return false;
        }
        // RENAME: both names in the page, source first; the destination's handle and length ride
        // in the second word.
        let src = installed_programs::CURRENT_STAGED.as_bytes();
        let dst = installed_programs::CURRENT.as_bytes();
        // SAFETY: the page is mapped read/write and both names together are a few bytes.
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), Self::page(), src.len());
            core::ptr::copy_nonoverlapping(dst.as_ptr(), Self::page().add(src.len()), dst.len());
        }
        let renamed = call(
            self.ep,
            fs_operation::req(fs_operation::RENAME, act, src.len() as u64),
            fs_operation::rename_dst(act, dst.len() as u64),
        )
        .0 as i64;
        if renamed != 0 {
            return false;
        }
        // Durable before the answer, because the answer is what a person acts on: "installed"
        // followed by a power cut that forgets it would be the prompt saying something untrue.
        // A device that cannot flush answers `EOPNOTSUPP`, and then the generation is written and
        // live and only as durable as the device makes it; that is recorded, not refused.
        call(
            self.ep,
            fs_operation::req(fs_operation::SYNC, fs_operation::ROOT, 0),
            0,
        );
        true
    }
}

/// One page, the unit the file service trades bytes in, and the most a generation may hold.
pub(super) const PAGE_BYTES: usize = spawnproto::IMAGE_PAGE as usize;
