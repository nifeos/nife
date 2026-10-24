//! **The filesystem test fixture**: the names and contents of the files the test image ships, the
//! witnesses' verdict words, and the shapes of the `walk` and `throughput` benchmarks, in one place
//! so the three programs that touch them agree: the host build (`cargo xtask`, which writes these
//! into the RedoxFS image with the host tool), the client (which reads and writes them through the
//! FS server), and the kernel test (which asserts the client's report). Not part of either wire
//! protocol: test data and test hooks only, and nothing that boots depends on it. Its one
//! dependency is `filesystem_contract`. notes/filesystem-fixture.md has the reasons behind each
//! shape.
//!
//! Name: provisional (proposed by block 886, its crate-root name lifted; considered and refused
//! `filesystem_test_image`, which names the shipped files but not the verdict words or the
//! benchmark shapes). Calef names crates.

#![no_std]

/// A file the image ships with; the client reads it back through a granted directory capability.
pub const MOTD_NAME: &str = "motd";
/// Its exact contents. Longer than eight bytes so the report's head word is a real prefix.
pub const MOTD: &[u8] = b"redoxfs served this file to an EL0 client through a capability handle\n";

/// A file the image ships with (with placeholder contents) so the client can open it and write.
pub const SCRATCH_NAME: &str = "scratch";
/// Placeholder contents the host tool writes; overwritten by the client's write test.
pub const SCRATCH_INIT: &[u8] = b"(placeholder overwritten by the redoxfs_server write test)";
/// What the client writes to `scratch` and reads back; the host tool re-reads it after the run
/// to prove the write reached the on-disk image and the filesystem is still consistent.
pub const WRITE_PATTERN: &[u8] = b"CRKFS_WRITE_OK: this round-tripped through the RedoxFS image\n";

/// The client's success sentinel, sent as the report's second word; the head of [`MOTD`] is the
/// first. Any other value (or silence) fails the test.
pub const SUCCESS: u64 = 0xF11E_600D;

/// **The shared-frame witness's two files** (milestone 599 (a frame per filesystem client channel),
/// provisional): same-length names and distinct contents, so a substitution shows as the wrong
/// body. The lengths must be equal because the length a client sends travels in a register while
/// the name travels in the shared page, which any holder of the frame can rewrite. A server that
/// read the victim's own bytes returns [`SHARED_VICTIM_BODY`]; one that read the attacker's returns
/// [`SHARED_USURPER_BODY`], the escape. See `system_tests/src/user/fs_shared_page_tests.rs` and
/// notes/filesystem-fixture.md.
///
/// Names provisional (this lane's coinage); an architect names files.
pub const SHARED_VICTIM_NAME: &str = "fs-witness-victim";
/// See [`SHARED_VICTIM_NAME`]. Exactly as long as [`SHARED_USURPER_NAME`].
pub const SHARED_USURPER_NAME: &str = "fs-witness-usurpr";
/// What [`SHARED_VICTIM_NAME`] holds: what the victim expects to read back.
pub const SHARED_VICTIM_BODY: &[u8] = b"CRK599-VICTIM: the file this client actually named\n";
/// What [`SHARED_USURPER_NAME`] holds: what a client reads when another client substituted the
/// staged name mid-request. Distinct from [`SHARED_VICTIM_BODY`] in its first eight bytes, so a
/// single head word tells them apart.
pub const SHARED_USURPER_BODY: &[u8] =
    b"CRK599-USURPER: a file named by a different client entirely\n";

/// Equal-length names, checked at build time: the witness rests on it (see [`SHARED_VICTIM_NAME`]).
const _: () = assert!(SHARED_VICTIM_NAME.len() == SHARED_USURPER_NAME.len());

/// The shared-frame witness's report words. The victim sends one of these as its report's first
/// word. [`SHARED_SUBSTITUTED`] is the live defect (the server resolved the attacker's name);
/// [`SHARED_ISOLATED`] is what milestone 599's fix would make true (the victim's own name
/// resolved despite the attacker's write). A negative or [`SHARED_UNEXPECTED`] means the open
/// failed or returned bytes matching neither file, which is a wiring fault in the witness.
pub const SHARED_SUBSTITUTED: u64 = 0x5995_500B;
/// See [`SHARED_SUBSTITUTED`]: the isolated outcome milestone 599 would produce.
pub const SHARED_ISOLATED: u64 = 0x5991_150A;
/// See [`SHARED_SUBSTITUTED`]: the open returned neither file's body, so the witness is wired wrong.
pub const SHARED_UNEXPECTED: u64 = 0x599B_ADDD;

/// **What the durability witness found** (milestone 55 (Time Machine: SMB3 with Apple's
/// extensions)), sent as the report's third word by the in-guest role that runs it. It asks
/// [`filesystem_contract::fs::SYNC`] twice: the count must be non-zero at the first ask (another
/// process answered a flush, the end-to-end leg) and must strictly increase between the two (a
/// device round trip, not a constant). The classes are far apart so a device that cannot flush is
/// never confused with a flush that never left the server (notes/filesystem-fixture.md).
///
/// # BUGS
///
/// - Nothing produces these verdicts: the role that ran the witness went with SMB (notes/smb.md).
pub mod durability {
    /// Both checks passed: something above this process had already flushed the device, and
    /// this process's own sync moved the count again. The only passing verdict.
    pub const DURABLE: u64 = 1;
    /// The count was zero when the witness first asked, so **no flush ever reached the
    /// device** before it ran: nothing above this process reached `filesystem_contract::fs::SYNC`.
    pub const NEVER_FLUSHED: u64 = 2;
    /// [`filesystem_contract::fs::SYNC`] answered `EOPNOTSUPP`: this device offers no
    /// `VIRTIO_BLK_F_FLUSH`, so the claim cannot be backed here at all. **Not a bug in this
    /// tree**, and it must not be reported as one; it is the honest answer for a device that
    /// does not do the thing, and a run that met it should say so and stop rather than pass.
    pub const NO_DEVICE_FLUSH: u64 = 3;
    /// The first sync was refused for some other reason; the errno rides in the report's
    /// remaining room where a caller has one, and a bare occurrence of this verdict means the
    /// verb is wired wrong rather than unsupported.
    pub const REFUSED: u64 = 4;
    /// Two syncs, the same count: the second one did not reach the device. A server answering
    /// a cached or constant yes lands exactly here, which is the failure this witness is
    /// shaped around.
    pub const NOT_ADVANCING: u64 = 5;
}

/// **The file milestone 38's throughput benchmark writes and reads back.** Not shipped in the
/// image: the throughput role creates it (or truncates it, if a previous run left it there),
/// which is deliberate rather than incidental, because the sequential-write phase *is* the
/// creation and a file the image already carried would have been laid out by the host tool
/// rather than by the server under test.
///
/// Name: provisional (2026-08-18, milestone 38 (filesystem throughput)): calef names files a
/// reader meets, and this one is met by anyone who lists the bench image after a run.
pub const THROUGHPUT_NAME: &str = "throughput";

/// **The shape of milestone 38's throughput measurement**, shared by the client that performs it
/// (`fixtures/src/fs_test_client/throughput.rs`, the throughput role) and the bench boot that names
/// its results (`kernel/src/bench.rs`). Two programs agree on these, so they are a crate and not a
/// constant written twice (CLAUDE.md rule 7).
pub mod throughput {
    /// **The transfer unit: the whole file channel, the most one `filesystem_contract` request can
    /// carry.** It is [`filesystem_contract::fs::TRANSFER_MAX`], so this phase measures the
    /// protocol's own ceiling and the tuning lives at that one constant. A figure here is how fast
    /// this architecture moves one request's worth, so a comparison against a system whose client
    /// passes a differently sized buffer compares two different things; see notes/benchmarks.md.
    pub const UNIT: usize = filesystem_contract::fs::TRANSFER_MAX;

    /// **Bytes moved per phase, held constant across transfer sizes on purpose.** 1 MiB, bounded by
    /// the 16 MiB fixture image rather than chosen for statistics: the write phases fit beside
    /// everything else the image carries, and RedoxFS is copy-on-write, so a rewritten block takes
    /// fresh space until the old one is freed. The bytes are pinned rather than the transfer count,
    /// so two runs move the same file however the requests are shaped, and a before and an after
    /// compare.
    pub(crate) const TOTAL: u64 = 1024 * 1024;

    /// Transfers per phase: [`TOTAL`] divided by [`UNIT`]. 256 at one page, 16 at sixteen.
    pub const BLOCKS: u64 = TOTAL / UNIT as u64;

    /// **The phase moves exactly [`TOTAL`] bytes at every transfer size**, which is what makes a
    /// before and an after on `bench/transfer-size-sweep.sh` a comparison rather than two
    /// numbers, and what the sequential phase's `FSTAT` check against the file size depends on.
    /// A build is where this has to hold, because the phase runs at EL0 where no test does.
    const _: () = {
        assert!(BLOCKS >= 1);
        assert!(BLOCKS * UNIT as u64 == TOTAL);
        assert!(TOTAL.is_multiple_of(RECORD));
    };

    /// Untimed transfers before each phase: the `OPEN`, the first cold block, and whatever the
    /// engine touches on its first walk of the file.
    pub const WARMUP: u64 = 8;

    /// Phase tags, sent as the report's third word so the bench boot names a result rather than
    /// counting messages. The order they are performed in is the order they are listed, and it
    /// is forced: the write phase is what creates the file the read phases read.
    pub const SEQ_WRITE: u64 = 1;
    /// Sequential read of the file the write phase left behind.
    pub const SEQ_READ: u64 = 2;
    /// Read at page-aligned offsets drawn from a fixed-seed PRNG, so a run is reproducible.
    pub const RAND_READ: u64 = 3;
    /// Write at those same pseudo-random offsets, in place, over a file that already exists.
    pub const RAND_WRITE: u64 = 4;
    /// **A read of the first page of a record**, kept because it refuted a model of where the
    /// milliseconds go. A read at a record's start was predicted to be cheaper than one in its
    /// middle; it measures the same as the other read phases, to within 3%, because `read_record`
    /// fetches the block the pointer stores, so a read of an ordinary file fetches the whole record
    /// whatever offset it asked for. notes/filesystem-fixture.md, "The record read", has the
    /// mechanism.
    pub const RECORD_READ: u64 = 6;

    /// **A record boundary**, the one property [`RECORD_READ`] needs: the phase reads at multiples
    /// of this, which is meaningful exactly when each is also a multiple of the store's record
    /// size. 128 KiB is, at every RedoxFS record level from 0 to 5, so it stays put when the level
    /// changes and every figure this phase produced stays comparable. **This is the store's number,
    /// not the protocol's**: `redoxfs_server/src/block.rs` asserts at compile time that it is a
    /// whole number of records.
    pub const RECORD: u64 = 128 * 1024;

    /// **The cost of producing one page of payload**, with no filesystem in it at all.
    ///
    /// It is measured and reported because the two write phases pay it inside their timed
    /// window and cannot avoid it. RedoxFS compresses a record with lz4 and skips a write whose
    /// record is byte-identical to what is already there, so a benchmark that sends the same
    /// page twice measures the short circuit and one that sends 32 identical pages into one
    /// 128 KiB record measures lz4. The payload therefore has to be fresh and incompressible
    /// per write, which costs 512 stores, and this row is how much. Subtract it to get the
    /// filesystem's own share.
    pub const PAYLOAD_FILL: u64 = 5;
    /// How many phase reports the bench boot waits for.
    pub const PHASES: usize = 6;

    /// **The client role the bench boot spawns**, which is the one thing here that two
    /// *binaries* have to agree on rather than two modules: the kernel passes it to
    /// `fs_service::start` and `fs_test_client` matches on it. The other eight roles are
    /// literals with comments on both sides, which is what this one would have been.
    pub const ROLE: u64 = 9;

    /// The name of a phase tag, for the line the bench boot prints. `None` for anything else,
    /// which is a client that reported a tag this table does not know.
    pub const fn name(tag: u64) -> Option<&'static str> {
        match tag {
            SEQ_WRITE => Some("fs_seq_write"),
            SEQ_READ => Some("fs_seq_read"),
            RAND_READ => Some("fs_rand_read"),
            RAND_WRITE => Some("fs_rand_write"),
            RECORD_READ => Some("fs_record_read"),
            PAYLOAD_FILL => Some("fs_payload_fill"),
            _ => None,
        }
    }
}

/// **The attacker's report: a bitmap of what got through**, not a pass/fail. Each bit says one
/// specific thing happened, so the test asserts an *expected set* rather than "zero", and a
/// failure names itself. That shape is what lets one attacker serve as its own negative control:
/// run against a read-only grant every bit must be clear, and run against a read/write grant of
/// the same shape the two write bits must be **set**, which is what proves the read-only
/// refusals were a narrowed capability rather than a caretaker that refuses everything.
pub mod escape {
    /// It opened a file the grant does not designate. Never allowed.
    pub const SECOND_FILE: u64 = 1 << 0;
    /// Its write to the granted file was accepted. Expected only with `grant::WRITE`.
    pub const WROTE: u64 = 1 << 1;
    /// Its truncate of the granted file was accepted. Expected only with `grant::WRITE`; a
    /// separate bit because a truncate carries no bytes, so a guard that only covered `WRITE`
    /// would leave a way to destroy a file just as thoroughly.
    pub const TRUNCATED: u64 = 1 << 2;
    /// It created a file through a file capability. Never allowed: a file is not a directory.
    pub const CREATED: u64 = 1 << 3;
    /// It reached a file with a handle it was never given. Never allowed.
    pub const FORGED_HANDLE: u64 = 1 << 4;
    /// The thing it *should* be able to do failed, so nothing above was actually proven. A
    /// capability that reaches nothing is trivially unescapable, and a test that only checked
    /// the refusals would pass against one.
    pub const GRANTED_READ_FAILED: u64 = 1 << 5;
    /// **Its extended-attribute write was accepted.** Expected only with `grant::WRITE`, and it
    /// is `WROTE`'s twin for the same reason `TRUNCATED` is: an attribute is a way to change a
    /// file that carries no file bytes, so a direction check that only covered `WRITE` and
    /// `TRUNCATE` would leave one open. Added by milestone 61 with the forwarding it guards.
    pub const WROTE_ATTR: u64 = 1 << 6;
    /// **Reading its own file's attributes failed**, so the capability does not carry what
    /// milestone 61 says it carries. A control bit in [`GRANTED_READ_FAILED`]'s shape rather
    /// than an escape, and the one that catches a regression to the old behaviour: before
    /// milestone 61 the caretaker answered `EOPNOTSUPP` to all four attribute verbs, so this
    /// bit would be set in **both** runs.
    pub const GRANTED_ATTRS_FAILED: u64 = 1 << 7;
}

/// The attacker's report leads with this so a silent client (a trapped one) cannot be mistaken
/// for a clean verdict of zero.
pub const VERDICT: u64 = 0xE5_CA9E00;

/// **The subtree milestone 47 (navigation and naming)'s directory capability is measured against.**
/// `sub` is granted; `other` is its real sibling, so "it cannot reach a sibling" is about a real
/// directory; `deeper` gives a second descent somewhere to go (notes/filesystem-fixture.md).
///
/// ```text
///   /            motd  scratch  sub/  other/
///   /sub         inner  deeper/          <- the granted capability is here
///   /sub/deeper  leaf
///   /other       secret                  <- never reachable from the grant
/// ```
pub mod tree {
    /// The directory a dir grant designates.
    pub const SUB: &str = "sub";
    /// A file inside it. Pinned by the post-run host check, so nothing may damage it.
    pub const INNER: &str = "inner";
    /// What [`INNER`] holds.
    pub const INNER_BODY: &[u8] = b"CRK47-INNER: a file inside the granted subtree\n";
    /// A directory inside the grant: what a second descent descends into.
    pub const DEEPER: &str = "deeper";
    /// A file inside that, reachable only with two descents.
    pub const LEAF: &str = "leaf";
    /// What [`LEAF`] holds.
    pub const LEAF_BODY: &[u8] = b"CRK47-LEAF: two descents below the granted directory\n";
    /// The granted directory's **sibling**. A capability to [`SUB`] must not reach it.
    pub const OTHER: &str = "other";
    /// A file in the sibling, pinned by the post-run host check.
    pub const SECRET: &str = "secret";
    /// [`INNER`] and [`SECRET`] **as absolute paths**, which the navigating witness types to
    /// ask the same two questions with a leading slash (milestone 47's namespace half).
    ///
    /// Constants rather than bytes built at the call site because the witness runs on two
    /// stack pages and has been one page short before now: a static string costs it nothing.
    pub const ABS_INNER: &str = "/inner";
    /// [`SECRET`] as an absolute path. See [`ABS_INNER`].
    pub const ABS_SECRET: &str = "/secret";
    /// What [`SECRET`] holds.
    pub const SECRET_BODY: &[u8] = b"CRK47-SECRET: in a sibling of the granted directory\n";

    /// The name the writable attacker creates inside its grant, with a run index appended so
    /// three runs sharing one image do not collide. It stays on the image afterwards, which is
    /// deliberate: the post-run host check asserts a name with this **prefix** is in [`SUB`] and
    /// **not** in the root, which is the escape it is looking for.
    pub const MADE: &str = "made-by-atk";
    /// What the attacker writes into [`MADE`]; read straight back, because "the server accepted
    /// my write" and "my write landed" are different claims.
    pub const MADE_BODY: &[u8] = b"CRK47: written through a directory capability\n";
    /// The directory the writable attacker makes inside its grant, to prove `MKDIR` mints a
    /// capability and that the capability it mints is not wider than the one that made it.
    pub const MADE_DIR: &str = "dir-by-atk";
    /// What the attacker renames [`MADE`] to, if its capability carries
    /// [`filesystem_contract::dir::REMOVE`]. The post-run host check looks for a name with this prefix
    /// in [`SUB`] **and** a [`MADE`] one that was not renamed, which is `REMOVE` being enforced
    /// witnessed from outside the guest: one capability moved a name and another could not.
    pub const MOVED: &str = "moved-by-atk";

    /// The file a navigating shell creates and **leaves** in its root, so that [`NAV_GONE`]'s
    /// absence afterwards is a fact about `unlink` rather than about a shell that created
    /// nothing. Run-indexed like the names above, for the same reason.
    pub const NAV_KEPT: &str = "nav-kept";
    /// The file a navigating shell creates, opens, writes, and then **unlinks while its own
    /// handle is still open**. Two claims ride on it: in-guest, the read through that handle
    /// still returns [`NAV_BODY`] (unlink removes a name, not an object); on the host, this name
    /// is not in the directory afterwards (the removal reached the platter).
    pub const NAV_GONE: &str = "nav-gone";
    /// What goes into [`NAV_GONE`] before it is unlinked, and what must still be readable
    /// through the open handle after.
    pub const NAV_BODY: &[u8] = b"CRK47-NAV: a name can go while the bytes stay\n";
    /// The directory a navigating shell makes with `mkdir` and leaves behind. It stays because
    /// the shell never empties it: [`filesystem_contract::fs::UNLINK`] refuses a directory and
    /// [`filesystem_contract::fs::RMDIR`] refuses a non-empty one, so a directory with a name in it
    /// takes two deliberate steps to remove and neither of them is one word.
    pub const NAV_DIR: &str = "nav-dir";
    /// The directory a navigating shell makes, puts [`NAV_INSIDE`] in, and then removes with
    /// `RMDIR` **after** taking that name back out. Two claims ride on it: in-guest, the first
    /// `RMDIR` is refused with `ENOTEMPTY` and the second succeeds; on the host, this name is
    /// not in the directory afterwards, so the removal reached the platter.
    pub const NAV_EMPTY: &str = "nav-empty";
    /// The one name inside [`NAV_EMPTY`], which is what makes the first `RMDIR` a refusal. It is
    /// unlinked before the second, so nothing of it survives either.
    pub const NAV_INSIDE: &str = "in";
    /// The file a navigating shell `touch`es twice: once absent (proving the create half),
    /// once present with a body already in it (proving the second call is a no-op rather than
    /// a truncate). Left behind, run-indexed like the names above, so the host check can find
    /// it and confirm the second `touch` did not disturb what the first write put there.
    pub const NAV_TOUCH: &str = "nav-touch";
    /// What a navigating shell writes into [`NAV_TOUCH`] before touching it a second time, and
    /// what must still be there after: the only claim `touch` on an existing name makes is
    /// that it changes nothing.
    pub const NAV_TOUCH_BODY: &[u8] = b"CRK47-TOUCH: a second touch must not disturb this\n";

    /// **The instant `touch -t` asserts in the mtime probes** (milestone 47's mtime lane,
    /// DECISIONS §112): 2030-01-01T00:00:00Z, far enough past this repository's own history
    /// that neither this server's advancing logical clock nor a real wall-clock reading could
    /// coincidentally produce it. The RFC 3339 text a real command line carries.
    pub const NAV_TOUCH_AT_RFC3339: &str = "2030-01-01T00:00:00Z";
    /// [`NAV_TOUCH_AT_RFC3339`] as the Unix-seconds value the wire and a read-back both use, so
    /// a round-trip check compares against the same number the text names rather than a second
    /// copy that could drift from it. Verified independently rather than derived at either end:
    /// `date -u -d 2030-01-01T00:00:00Z +%s` and `crates/calendar`'s own `DateTime::to_unix`
    /// both agree it is this.
    pub const NAV_TOUCH_AT_UNIX: u64 = 1_893_456_000;
    /// The fresh directory the navigating witness mints with `MKDIR`, attenuated to `WRITE` and
    /// not `SETTIME`, to prove `touch -t` needs the right through the real wire and not only in
    /// `redoxfs_server`'s own host tests. Ten bytes plus a run digit, `run_name`'s bound.
    pub const NAV_TOUCH_NO_SETTIME: &str = "nav-notime";

    /// **`bind`'s witness** (milestone 47, "`bind` is not blocked on a mount table. It is
    /// blocked on a second grant"; milestone 154 supplied it). A directory nested inside
    /// [`NAV_DIR`], holding [`NAV_BIND_MARKER`], and bound to a fresh top-level name
    /// ([`NAV_BIND_ALIAS`]) so `ls`, `cd` and `..` can all be checked through the bound name
    /// against the same real files a direct walk would reach.
    pub const NAV_BIND_NESTED: &str = "nav-bind-nest";
    /// The marker file [`NAV_BIND_NESTED`] holds. Its name alone is the claim: reachable
    /// through the bind means the same `OPENDIR`/`OPEN` the direct path would have sent.
    pub const NAV_BIND_MARKER: &str = "marker";
    /// The top-level name [`NAV_BIND_NESTED`] is bound to. A fresh word, never a real
    /// top-level entry, so reaching it through `ls`/`cd` is unambiguously the bind and not a
    /// coincidence of the fixture tree's own shape.
    pub const NAV_BIND_ALIAS: &str = "nav-bound";

    /// **The directory milestone 50 (pipes and redirection)'s redirection witness works in**, a
    /// sibling of [`SUB`]: the root is shared with every test in the boot, so a shell that writes
    /// files needs somewhere its writes cannot be mistaken for another test's. Two files, so `ls >
    /// out.txt` writes a listing with something in it and the `wc` that counts it counts more than
    /// its own output's name.
    ///
    /// ```text
    ///   /redir      alpha  beta        <- the redirection witness is granted this
    /// ```
    pub const REDIR: &str = "redir";
    /// The first of the two files inside [`REDIR`].
    pub const REDIR_ONE: &str = "alpha";
    /// The second of the two files inside [`REDIR`].
    pub const REDIR_TWO: &str = "beta";
    /// What both [`REDIR_ONE`] and [`REDIR_TWO`] hold.
    pub const REDIR_BODY: &[u8] = b"CRK50: a file the redirection witness lists\n";

    /// **The subtree the `rm` program is granted** (milestone 47's `rm -r`), a sibling of [`SUB`]
    /// so a capability to one is provably not a capability to the other.
    ///
    /// ```text
    ///   /rmtree                 rm-keep  rm-doomed/     <- the grant is here
    ///   /rmtree/rm-doomed       rm-one  rm-two  rm-nested/
    ///   /rmtree/rm-doomed/rm-nested   rm-leaf
    /// ```
    ///
    /// [`RM_KEEP`] is the control: the grant reaches it, and it stays because nothing named it.
    pub const RMTREE: &str = "rmtree";
    /// A file beside the doomed tree, inside the grant. Nothing names it, so nothing removes it.
    pub const RM_KEEP: &str = "rm-keep";
    /// What [`RM_KEEP`] holds.
    pub const RM_KEEP_BODY: &[u8] = b"CRK47-RM: inside the grant, and never named\n";
    /// The directory `rm -r` is pointed at.
    pub const RM_DOOMED: &str = "rm-doomed";
    /// Two files directly inside it, so the walk has something to unlink at the top level.
    pub const RM_ONE: &str = "rm-one";
    /// The second of the two top-level files inside [`RM_DOOMED`].
    pub const RM_TWO: &str = "rm-two";
    /// A directory inside it, so the walk has to recurse and `RMDIR` has to run bottom-up.
    pub const RM_NESTED: &str = "rm-nested";
    /// The file at the bottom: the last thing unlinked and the deepest the walk goes.
    pub const RM_LEAF: &str = "rm-leaf";
    /// What every file under [`RM_DOOMED`] holds.
    pub const RM_BODY: &[u8] = b"CRK47-RM: a file `rm -r` is expected to take away\n";
    /// A plain file beside the doomed tree, removed by a plain `rm` with no options. It is what
    /// makes **silence on success** falsifiable: that run must report a zero status and print
    /// nothing at all, and `rm(1)`'s `-v` exists precisely because the default prints nothing.
    pub const RM_SOLO: &str = "rm-solo";
    /// A name that is **not** on the image. `rm` of it is a diagnostic and a non-zero exit;
    /// `rm -f` of it is silence and a zero one, which is the whole of what `-f` means.
    pub const RM_MISSING: &str = "rm-nothing";

    /// **The directory the globbing lane's set grant is planned in**, a sibling of [`RMTREE`].
    ///
    /// ```text
    ///   /globset            gl-one.txt  gl-two.txt  gl-three.log  gl-dir/
    ///   /globset/gl-dir     gl-inner
    /// ```
    ///
    /// [`GLOB_PATTERN`] matches exactly [`GLOB_ONE`] and [`GLOB_TWO`]. [`GLOB_MISS`] is a file a
    /// set-attenuated capability must not name; [`GLOB_DIR`] is a directory, so the type bit
    /// [`filesystem_contract::nameset::IS_DIR`] carries has something to be wrong about.
    pub const GLOBSET: &str = "globset";
    /// The pattern the shell expands, and the whole of what `rm *.txt` designates here. Pinned
    /// against the matcher by a host test, so the set the kernel test states literally is
    /// provably the expansion rather than a second opinion about it.
    pub const GLOB_PATTERN: &[u8] = b"gl-*.txt";
    /// The first of the two names [`GLOB_PATTERN`] matches.
    pub const GLOB_ONE: &str = "gl-one.txt";
    /// The second of the two names [`GLOB_PATTERN`] matches.
    pub const GLOB_TWO: &str = "gl-two.txt";
    /// A file in the same directory that the pattern does **not** match. It is the escape the
    /// nameset caretaker is attacked with: it exists, and the caretaker one hop up could remove
    /// it.
    pub const GLOB_MISS: &str = "gl-three.log";
    /// A directory in the same directory, not matched either. It is why a set record carries a
    /// type bit at all.
    pub const GLOB_DIR: &str = "gl-dir";
    /// A file inside [`GLOB_DIR`], so the unmatched directory is not empty and a run that
    /// removed it would have had to walk it.
    pub const GLOB_INNER: &str = "gl-inner";
    /// What every matched file holds.
    pub const GLOB_BODY: &[u8] = b"CRK47-GLOB: a name a pattern either matched or did not\n";
    /// A pattern that matches **nothing** here, so the empty-expansion refusal has something to
    /// be about. Same prefix as the others deliberately: it fails on the extension alone, which
    /// is the case a matcher that gave up early would get wrong.
    pub const GLOB_NOMATCH: &[u8] = b"gl-*.rs";
    /// A line with no pattern in it, echoed to check that globbing did not quietly turn `echo`
    /// into a word-splitter. Its two consecutive spaces are the whole point.
    pub const GLOB_PLAIN: &[u8] = b"two  spaces";

    /// **Milestone 109's batching tree**: a directory holding more matching names than one
    /// grant can carry, so `xargs` has something at the prompt to be about.
    ///
    /// A sibling of [`GLOBSET`] rather than an extension of it, deliberately: the globbing
    /// lane's tests assert that `gl-*.txt` matches exactly two names, and adding files there to
    /// make this one's point would make that one's point by accident.
    pub const GLOBMANY: &str = "globmany";
    /// The pattern, matching every one of [`MANY_NAMES`] and not [`MANY_MISS`]. Pinned against
    /// the matcher by a host test, the way [`GLOB_PATTERN`] is.
    pub const MANY_PATTERN: &[u8] = b"m-*.txt";
    /// **Eleven names: one full batch and a short one.** Eleven rather than sixteen because the
    /// second batch has to be *short*, or a sweep that always ran full batches would pass. They
    /// are zero-padded so their byte order is their numeric order, which makes the batch
    /// boundary something a reader can check by eye.
    pub const MANY_NAMES: [&str; 11] = [
        "m-00.txt", "m-01.txt", "m-02.txt", "m-03.txt", "m-04.txt", "m-05.txt", "m-06.txt",
        "m-07.txt", "m-08.txt", "m-09.txt", "m-10.txt",
    ];
    /// **The first batch, as the prompt prints it**: the eight smallest matches, space
    /// separated. Spelled out rather than derived, because a gate that computes its own expected
    /// answer from the same rule it is testing checks nothing.
    pub const MANY_FIRST_BATCH: &str =
        "m-00.txt m-01.txt m-02.txt m-03.txt m-04.txt m-05.txt m-06.txt m-07.txt";
    /// **The second batch**, which is the short one, and which the first batch's watermark
    /// decides. A sweep resuming from a directory *cursor* rather than from a name would print
    /// something else here the moment anything in the directory moved.
    pub const MANY_SECOND_BATCH: &str = "m-08.txt m-09.txt m-10.txt";
    /// A file in the same directory the pattern does not match, so "the batches are the match"
    /// is a claim about *which* names rather than about how many.
    pub const MANY_MISS: &str = "m-keep.log";
    /// What every one of [`MANY_NAMES`] holds.
    pub const MANY_BODY: &[u8] = b"CRK109-XARGS: a name in a match too large to hand over\n";

    /// **The names the image root must still carry after a run**, checked by the post-run host
    /// tool: this is the half of the assertion made from *outside* the confined program, and no
    /// in-guest verdict could have reported it. A capability granted on [`SUB`] can remove
    /// nothing above itself, so a name missing here escaped.
    ///
    /// **Containment, not equality**, and the reason is worth stating where the constant is:
    /// the root is shared with every other test in the boot (the `std::fs` test creates
    /// `made-by-std` in it), so an exact comparison would couple this milestone's gate to what
    /// unrelated tests happen to write. The upward-escape half is checked against [`MADE`] and
    /// [`MADE_DIR`] instead, which are names only the attacker writes.
    pub const ROOT_ENTRIES: [&str; 9] = [
        GLOBMANY,
        GLOBSET,
        crate::MOTD_NAME,
        OTHER,
        REDIR,
        RMTREE,
        crate::SCRATCH_NAME,
        SUB,
        crate::walk::ROOT,
    ];
}

/// **The tree milestone 121 (`ripgrep` on nife: enumeration as a capability) prices a walk over**:
/// a sibling of [`tree::SUB`], granted alone and written by nothing, so every figure the pricing
/// prints is a function of this shape. One region per cost, each a slope over one variable: `chain`
/// (a component per level), `wide` (an entry per name), `narrow` (the one-entry control) and
/// `sizes` (a byte per byte). `walk_pricing` (provisional) stages it on a host and walks it on both
/// sides; notes/filesystem-fixture.md draws it.
///
/// Names: provisional (2026-09-26, milestone 121's lane). They are met by anyone who lists
/// the test image, which is calef's to name.
pub mod walk {
    /// The directory the pricing is granted, one component under the image root.
    pub const ROOT: &str = "walk";
    /// The depth region's top directory.
    pub const CHAIN: &str = "chain";
    /// The name of every directory below [`CHAIN`]: one letter, so a deeper path costs
    /// components rather than name bytes.
    pub const LEVEL: &str = "c";
    /// Directories below [`CHAIN`]. Eight, so a slope has nine points and the deepest open is
    /// ten components from the grant, deeper than any path the rest of the fixture uses.
    pub const DEPTH: usize = 8;
    /// The file at every level of the chain, [`CHAIN`] included.
    pub const LEVEL_FILE: &str = "f";
    /// The width region: one directory holding [`WIDE_COUNT`] files.
    pub const WIDE: &str = "wide";
    /// Files in [`WIDE`]. 128 four-byte names are 768 bytes of `READDIR` records
    /// ([`filesystem_contract::dirent::record_len`]), so the whole listing is one page and the slope is per
    /// entry rather than per page. Pricing a page boundary would want more than 680 names.
    pub const WIDE_COUNT: usize = 128;
    /// The control: [`WIDE`]'s shape with one file, so the difference is entries alone.
    pub const NARROW: &str = "narrow";
    /// The byte region.
    pub const SIZES: &str = "sizes";
    /// Its files and their sizes. 4 KiB is one page; 256 KiB is sixteen of the file channel's
    /// largest requests ([`filesystem_contract::fs::TRANSFER_MAX`] at the time of writing).
    pub const SIZE_FILES: [(&str, usize); 3] =
        [("s4k", 4096), ("s64k", 65_536), ("s256k", 262_144)];
    /// What every file in [`CHAIN`], [`WIDE`] and [`NARROW`] holds. Sixteen bytes, so a small
    /// file's read is one request and the byte region is the only place bytes vary.
    pub const SMALL_BODY: &[u8] = b"nife walk entry\n";

    /// The byte at `i` of a sized file: printable, a newline every 64, so a search tool run
    /// over the tree later has lines to match and the host and guest agree without a file.
    pub const fn sized_byte(i: usize) -> u8 {
        if i % 64 == 63 {
            b'\n'
        } else {
            b'a' + (i % 26) as u8
        }
    }

    /// Name of the `i`th file in [`WIDE`] (and the one in [`NARROW`]): `n` and three digits.
    pub const fn wide_name(i: usize) -> [u8; 4] {
        [
            b'n',
            b'0' + (i / 100 % 10) as u8,
            b'0' + (i / 10 % 10) as u8,
            b'0' + (i % 10) as u8,
        ]
    }

    /// Directories a full walk from [`ROOT`] lists: the four regions, and the chain's levels.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by maintainer/names-comply-question-shapes.
    /// Refused `WALK_DIRS` (`dir` is spelled out, per §154 (the acronym test is whether the phrase
    /// is spoken)).
    pub const WALK_DIRECTORIES: usize = 4 + DEPTH;
    /// Files a full walk opens.
    pub const WALK_FILES: usize = (DEPTH + 1) + WIDE_COUNT + 1 + SIZE_FILES.len();
    /// Entries a full walk is handed by `read_dir`, which is every directory and every file
    /// once. Printed and asserted by the priced program, so a walk that silently skipped a
    /// directory it could not look into fails on a count rather than on a slower figure.
    pub const WALK_ENTRIES: usize = WALK_DIRECTORIES + WALK_FILES;
    /// Bytes a full walk reads.
    pub const WALK_BYTES: usize = {
        let mut sized = 0;
        let mut i = 0;
        while i < SIZE_FILES.len() {
            sized += SIZE_FILES[i].1;
            i += 1;
        }
        (WALK_FILES - SIZE_FILES.len()) * SMALL_BODY.len() + sized
    };
    /// Path components a full walk resolves: each entry's depth below [`ROOT`], summed, since
    /// that is the length of the path a walker hands back to list or open it. On nife it is
    /// the number of hops a component-at-a-time resolver pays, and what a multi-component
    /// resolve would save. Computed from the shape rather than by walking, so a walker that
    /// skipped a directory cannot agree with it by construction.
    pub const WALK_COMPONENTS: usize = {
        // The four regions, at depth 1.
        let mut sum = 4;
        // The chain: level `i` (the chain itself is level 0, at depth 1) holds its entries at
        // depth `i + 2`, two of them at every level but the last, which holds only the file.
        let mut i = 0;
        while i <= DEPTH {
            let here = if i < DEPTH { 2 } else { 1 };
            sum += here * (i + 2);
            i += 1;
        }
        // The wide, narrow and sized files, at depth 2.
        sum + 2 * (WIDE_COUNT + 1 + SIZE_FILES.len())
    };

    /// **What unmodified `ripgrep` is told to do over this tree** (milestone 121), as a line
    /// typed at the prompt, with the tree as its working directory. The kernel harness turns
    /// it into an argv with `grant_plan::argv`, the shell's own function, and
    /// `walk_pricing`'s `rg_host` example runs the same words on a host, so nife and Linux run
    /// one command. `--threads 1` and `--no-mmap` are the block's two honesty pins: nife
    /// answered one thread when this was written and has no `mmap`, so the host must be held to
    /// the same. The threaded lines are below. `--stats`
    /// is what makes the search measurable from inside both: ripgrep times its own walk and
    /// search. Name: provisional.
    pub const RG_SEARCH: &str = "rg --threads 1 --no-mmap --stats 'walk entry'";
    /// Lines [`RG_SEARCH`] matches: one in every file holding [`SMALL_BODY`], none in the
    /// sized files, whose bytes are the alphabet.
    pub const RG_MATCHES: usize = WALK_FILES - SIZE_FILES.len();
    /// **The control for the refusal**: the same pattern naming one file, which needs `READ`
    /// and `DESCEND` and no `ENUMERATE`. Under a grant lacking `ENUMERATE` this still finds
    /// its one line, so a refused walk is about enumeration alone. Typed as a person would,
    /// with neither pin: a single named file is where `ripgrep` tries a memory map, so this
    /// also shows nife needs no `--no-mmap`. Name: provisional.
    pub const RG_NAMED: &str = "rg 'walk entry' narrow/n000";

    /// **[`RG_SEARCH`] without `--threads 1`**: `ripgrep` picks its own thread count from
    /// `available_parallelism`, the online core count, and takes `ignore`'s parallel walker when
    /// that is more than one. A line a person would type, apart from the measurement pins.
    /// Name: provisional (lane milestone/121-rg-threads, 2026-10-12 UTC).
    pub const RG_SEARCH_THREADED: &str = "rg --no-mmap --stats 'walk entry'";
    /// **[`RG_SEARCH`] told four threads**, whatever the core count.
    ///
    /// Name: provisional, lane milestone/121-rg-threads (#2016), 2026-10-12 (UTC).
    /// The pull request records no other name considered.
    pub const RG_SEARCH_FOUR_THREADS: &str = "rg --threads 4 --no-mmap --stats 'walk entry'";
    /// The thread count [`RG_SEARCH_FOUR_THREADS`] names.
    pub const RG_FOUR: usize = 4;

    /// **The path of the `i`th file [`RG_SEARCH`] matches**, `i` below [`RG_MATCHES`], written
    /// into `buf` as `ripgrep` prints it relative to the tree: the chain's files from the top
    /// down, then [`WIDE`]'s in name order, then [`NARROW`]'s. Returns its length. Each matching
    /// line is this, `:` and [`SMALL_BODY`]. The kernel harness and the host's `rg` are both held
    /// to this one list, so a parallel walk that prints in another order, or prints a line twice,
    /// is caught on either side.
    ///
    /// Name: provisional, lane milestone/121-rg-threads (#2016), 2026-10-12 (UTC).
    /// The pull request records no other name considered.
    pub fn rg_match_path(i: usize, buf: &mut [u8; 32]) -> usize {
        let mut len = 0;
        let mut put = |bytes: &[u8], len: &mut usize| {
            buf[*len..*len + bytes.len()].copy_from_slice(bytes);
            *len += bytes.len();
        };
        if i <= DEPTH {
            put(CHAIN.as_bytes(), &mut len);
            put(b"/", &mut len);
            for _ in 0..i {
                put(LEVEL.as_bytes(), &mut len);
                put(b"/", &mut len);
            }
            put(LEVEL_FILE.as_bytes(), &mut len);
        } else {
            let (dir, n) = if i - (DEPTH + 1) < WIDE_COUNT {
                (WIDE, i - (DEPTH + 1))
            } else {
                (NARROW, 0)
            };
            put(dir.as_bytes(), &mut len);
            put(b"/", &mut len);
            put(&wide_name(n), &mut len);
        }
        len
    }
}

/// **The directory attacker's report** (milestone 47), a bitmap for the same reason the per-file
/// attacker's is one: the test asserts an *expected set*, so the read-only run and the wide run
/// are each other's control and a caretaker that refused everything fails one of them.
pub mod dirscape {
    /// It opened a file that exists only in the granted directory's **parent**. Never allowed:
    /// this is "cannot reach its parent".
    pub const REACHED_PARENT: u64 = 1 << 0;
    /// It descended into, or opened anything in, the granted directory's **sibling**. Never
    /// allowed: this is "cannot reach a sibling".
    pub const REACHED_SIBLING: u64 = 1 << 1;
    /// `..` resolved to something. Never allowed, at any rights.
    pub const WALKED_UP: u64 = 1 << 2;
    /// **It asked for a right its capability did not carry, and got it.** Never allowed, and
    /// this is the bit that answers "can a child's rights exceed its parent's".
    pub const WIDENED: u64 = 1 << 3;
    /// It enumerated the granted directory. Expected only with [`filesystem_contract::dir::ENUMERATE`].
    pub const ENUMERATED: u64 = 1 << 4;
    /// It descended one level inside the grant. Expected only with
    /// [`filesystem_contract::dir::DESCEND`].
    pub const DESCENDED: u64 = 1 << 5;
    /// It created a name inside the grant. Expected only with [`filesystem_contract::dir::CREATE`].
    pub const CREATED: u64 = 1 << 6;
    /// It made a **directory** inside the grant and got a capability to it. Expected only with
    /// [`filesystem_contract::dir::CREATE`] and [`filesystem_contract::dir::DESCEND`] together.
    pub const MADE_A_DIR: u64 = 1 << 12;
    /// Its write to a file inside the grant was accepted **and read back**. Expected only with
    /// [`filesystem_contract::dir::WRITE`].
    pub const WROTE: u64 = 1 << 7;
    /// **It renamed a name inside its grant.** Expected only with
    /// [`filesystem_contract::dir::REMOVE`] and [`filesystem_contract::dir::CREATE`] together, which is what
    /// makes this the bit that gives the `REMOVE` rung something to fail.
    pub const RENAMED: u64 = 1 << 8;
    /// An enumeration it was allowed to make returned a name that is not in the granted
    /// directory. Never allowed: a listing is a rendering of authority, so a name from outside
    /// the grant appearing in it is an escape even though nothing was opened.
    pub const ENUMERATED_A_STRANGER: u64 = 1 << 9;
    /// It reached something with a handle it was never given. Never allowed.
    pub const FORGED_HANDLE: u64 = 1 << 10;
    /// **It opened the file inside its own grant**, which it is supposed to be able to do. The
    /// control bit: without it every refusal above is equally consistent with a caretaker that
    /// answers no to everything, or a grant that reaches nothing at all.
    pub const OPENED_ITS_OWN: u64 = 1 << 13;
    /// **The thing it should be able to do failed**, so nothing above was proven. A capability
    /// that reaches nothing is trivially unescapable.
    pub const GRANTED_ACCESS_FAILED: u64 = 1 << 11;
    /// **It listed the extended attributes of the file inside its grant** (milestone 61).
    /// Expected wherever [`filesystem_contract::dir::READ`] is held, which is every configuration that
    /// can open the file at all: an attribute is part of what a file *is*, so a capability that
    /// may read the file may read what is attached to it. Before milestone 61 the caretaker
    /// answered `EOPNOTSUPP` and this bit was never set.
    pub const READ_ATTRS: u64 = 1 << 14;
    /// **It set an extended attribute on that file and read it back with its type code.**
    /// Expected only with [`filesystem_contract::dir::WRITE`], which is [`WROTE`]'s rule applied to the
    /// third way of changing a file.
    ///
    /// The interesting part is *who* enforces it: `fs_subtree_caretaker` performs no check, so
    /// this bit is a statement about the handle the FS server minted from the caretaker's one
    /// `OPENDIR`. A file handle inherits `READ`/`WRITE` from the directory it was opened
    /// through, so a read-only subtree grant is refused by the server rather than by a branch in
    /// the caretaker, which is exactly that program's design property holding under a verb it
    /// was never taught.
    pub const SET_AN_ATTR: u64 = 1 << 15;
    /// **It reached a name in the granted directory that the grant's *set* does not carry**
    /// (milestone 61's name-set witness). Never allowed.
    ///
    /// A set capability's whole property is that `rm *.txt` cannot reach `notes.md`. `rm` asks
    /// that question with [`filesystem_contract::dir::REMOVE`] and nothing else; this asks it with
    /// `READ`, because the milestone taught `fs_nameset_caretaker` four verbs that read and it
    /// would be a poor trade to close an attribute gap by opening a naming one.
    pub const REACHED_AN_UNMATCHED_NAME: u64 = 1 << 16;
}

/// **What the two-directory witness reports** (milestone 154,
/// design/roadmap/0154-multi-directory-namespace.md): one process, two `fs_subtree_caretaker`s,
/// two capability table slots, and the negative control that only a union of two grants can state.
///
/// A bitmap for [`dirscape`]'s reason: the test asserts an *exact* set, so a witness that
/// reached nothing and one that reached everything both fail. The witness is told nothing
/// about its two grants beyond which capability table slot each landed in (slot 0 is always the first,
/// slot 1 the second, per `kernel/src/user/fs_service/two_dirs.rs`'s `start_granted_two_dirs` doc); it
/// tries to name a file in each subtree, tries to cross from one to the other both by request
/// and by `grant_plan::nav::TwoRoots`'s own client-side resolution, and reports what it found.
pub mod twodir {
    /// It opened the file inside the **first** grant, through the first grant's own endpoint.
    /// The control for that half: without it, every refusal below is equally consistent with a
    /// caretaker that reaches nothing.
    pub const OPENED_A: u64 = 1 << 0;
    /// It opened the file inside the **second** grant, through the second grant's own endpoint.
    /// The control for the other half, for the same reason.
    pub const OPENED_B: u64 = 1 << 1;
    /// **It opened the second grant's file through the first grant's endpoint.** Never allowed:
    /// this is the structural finding notes/dir-capability.md records (the endpoint is the
    /// boundary), witnessed here with two live caretakers rather than inferred from one.
    pub const REACHED_B_VIA_A: u64 = 1 << 2;
    /// **It opened the first grant's file through the second grant's endpoint.** Never allowed,
    /// the same claim from the other side.
    pub const REACHED_A_VIA_B: u64 = 1 << 3;
    /// **`grant_plan::nav::TwoRoots::resolve` did not refuse a path that ascends out of one
    /// grant and into the other** (the roadmap block's own example, `/a/../b`). Never allowed:
    /// this is the negative control that only a union of two grants can state, proven as pure
    /// client-side logic before any request reaches a caretaker.
    pub const DOT_DOT_CROSSED_MOUNTS: u64 = 1 << 4;
    /// **The thing the witness should be able to do failed**, so nothing above was proven. A
    /// grant that reaches nothing is trivially unescapable, the same caveat every other
    /// witness in this module carries.
    pub const GRANTED_ACCESS_FAILED: u64 = 1 << 5;
}

/// **What a shell holding two trees reports**, the live half of milestone 154 (a process that
/// holds two directory capabilities): the real `swish` builtins, `cd`, `pwd`, `ls`, `bind` and a
/// planned `<`, run over two live caretakers, the first over [`tree::SUB`] and the second over
/// [`tree::OTHER`], mounted at [`twotrees::MOUNT`] in the first.
///
/// calef ruled the presentation on 2026-09-26: one tree, with other trees mounted at names in
/// it. [`twodir`] proved one process can hold two trees; this proves the shell presents them
/// as one, with the same exact-set check, so the reaching bits are the controls for the
/// refusal bits. Provisional names, all of them.
pub mod twotrees {
    /// Where the witness mounts the second tree, as the shell's own path.
    pub const MOUNT: &str = "/second";
    /// `pwd` printed `/` at the start: one root, whatever the shell holds.
    pub const PWD_STARTS_AT_ROOT: u64 = 1 << 0;
    /// `ls /` named [`crate::tree::INNER`] and the mount point, and not
    /// [`crate::tree::SECRET`]: a mount point is listed in its parent.
    pub const LISTED_ROOT: u64 = 1 << 1;
    /// `cd` to the mount point moved the shell into the second tree, and `pwd` printed the
    /// mount path.
    pub const MOVED_INTO_MOUNT: u64 = 1 << 2;
    /// `ls` inside the mount named [`crate::tree::SECRET`] and not [`crate::tree::INNER`].
    pub const LISTED_MOUNT: u64 = 1 << 3;
    /// A relative name opened inside the mount, over the second tree's own endpoint.
    pub const OPENED_RELATIVE_IN_MOUNT: u64 = 1 << 4;
    /// `/inner` opened from inside the mount: an absolute path reaches the first tree
    /// without moving the shell.
    pub const OPENED_ROOT_FROM_MOUNT: u64 = 1 << 5;
    /// `cd ..` at the mount point moved to its parent in the first tree, as a Unix mount does,
    /// and a relative `cd` back in returned to the mount.
    pub const UP_FROM_MOUNT_IS_PARENT: u64 = 1 << 6;
    /// `..` past the one root was refused and nothing moved, from the root and from inside the
    /// mount.
    pub const ROOT_CLAMPED: u64 = 1 << 7;
    /// `<` planned against a file under the mount opened in the second tree and read
    /// [`crate::tree::SECRET_BODY`]: the per-command grant carried its tree through to the open.
    pub const REDIRECTED_FROM_MOUNT: u64 = 1 << 9;
    /// A bare `cd` came back to `/` from inside the mount.
    pub const HOME_IS_ROOT: u64 = 1 << 10;
    /// A name bound to the mount point listed the second tree's files, and binding the mount
    /// point's own name was refused.
    pub const BOUND_INTO_MOUNT: u64 = 1 << 11;
    /// `rm` under the mount was planned and then refused at delivery rather than sent: the
    /// progenitor would build its caretaker in the first tree.
    pub const RM_IN_MOUNT_REFUSED: u64 = 1 << 12;

    /// **A refused move moved the shell.** Never allowed.
    pub const MOVED_ON_REFUSAL: u64 = 1 << 16;
    /// **`..` past the root was not refused.** Never allowed.
    pub const CLIMBED_OUT: u64 = 1 << 17;
    /// **A name that exists only in one tree opened through the other**: the second tree's
    /// file at the first tree's path, or the first's under the mount. Never allowed: the mount
    /// selects the endpoint, and the endpoint is the boundary.
    pub const REACHED_ACROSS: u64 = 1 << 18;
    /// Nothing reachable was reached, so nothing above was proven.
    pub const TWO_TREES_FAILED: u64 = 1 << 19;
}

/// **What a navigating shell reports** (milestone 47's commands: `cd`, `pwd`, `ls`, `mkdir`,
/// `rm`).
///
/// A bitmap for the same reason the two attackers' are: the kernel test asserts an *exact* set,
/// so a shell that could do nothing and a shell that could do everything both fail, and the two
/// shells the headline test runs are each other's control. The shell running this script is told
/// **nothing about which subtree it was rooted in** beyond a run index; it tries to name one file
/// from each of two different roots and reports which it reached, so "two shells cannot name each
/// other's files" is read off the *pair* of reports rather than claimed by either.
pub mod navscape {
    /// `pwd` rendered `/` before anything moved. The root of your namespace is the root of the
    /// only namespace you have, which is the whole of "pwd is relative to your root".
    pub const PWD_IS_ROOT: u64 = 1 << 0;
    /// `cd ..` at the root was refused **and nothing moved**. This is `..` clamping: the shell
    /// descends from what it holds and has nothing above it to pop.
    pub const CLAMPED_AT_ROOT: u64 = 1 << 1;
    /// `cd ..` at the root moved it somewhere. Never allowed, at any rights.
    pub const WALKED_UP: u64 = 1 << 2;
    /// **`/` named this shell's own root**: from one level down, `cd /` came back to the root
    /// and `pwd` rendered `/` again. Plan 9's answer rather than DOS's, and the round trip
    /// `pwd` had always implied: the path it prints is one you can type back.
    ///
    /// It replaced an `ABSOLUTE_REFUSED` bit on 2026-08-18. That bit was true of a system with
    /// no namespace to root a path in, and rooting one in the holder's own namespace grants
    /// nothing, which [`ABSOLUTE_REACHED_INNER`] and [`ABSOLUTE_REACHED_SECRET`] are the
    /// measurement of rather than the claim.
    pub const ABSOLUTE_IS_MY_ROOT: u64 = 1 << 3;
    /// It opened [`crate::tree::INNER`], which exists only in [`crate::tree::SUB`].
    pub const REACHED_INNER: u64 = 1 << 4;
    /// It opened [`crate::tree::SECRET`], which exists only in [`crate::tree::OTHER`].
    pub const REACHED_SECRET: u64 = 1 << 5;
    /// `ls` of its own root returned a listing.
    pub const LISTED: u64 = 1 << 6;
    /// That listing named [`crate::tree::INNER`].
    pub const SAW_INNER: u64 = 1 << 7;
    /// That listing named [`crate::tree::SECRET`]. A listing is a rendering of authority, so the
    /// pair of these two bits is the property stated in what the two shells can *see* as well as
    /// in what they can open.
    pub const SAW_SECRET: u64 = 1 << 8;
    /// `cd` into a child directory worked **and `pwd` then rendered the child's path**, which is
    /// the half that proves the shell's own position tracked the capability it moved to.
    pub const DESCENDED: u64 = 1 << 9;
    /// `cd ..` from there came back, and `pwd` rendered `/` again.
    pub const RETURNED: u64 = 1 << 10;
    /// It created both of its files with the shell's `create` path.
    pub const CREATED: u64 = 1 << 11;
    /// `mkdir` made a directory in its root.
    pub const MADE_DIR: u64 = 1 << 12;
    /// `rm` removed the name it had made.
    pub const UNLINKED: u64 = 1 << 13;
    /// **After that `rm`, a read through the handle it still held returned the bytes.** The
    /// unlink/revoke split, made a measurement: the name went and the object did not.
    pub const HOLDER_KEPT_READING: u64 = 1 << 14;
    /// And the name it removed no longer opens. Without this, the bit above is equally
    /// consistent with an `rm` that did nothing at all.
    pub const NAME_GONE_AFTER_UNLINK: u64 = 1 << 15;
    /// `UNLINK` of a **directory** was refused. It unlinks the name of a file; a verb that
    /// removed whatever it found is how one word takes a subtree away.
    pub const UNLINK_REFUSED_A_DIRECTORY: u64 = 1 << 16;
    /// **Something that should have worked did not**, so the refusals above prove nothing. The
    /// control bit, in the shape both attackers already use.
    pub const NAVIGATION_FAILED: u64 = 1 << 17;
    /// `RMDIR` of a directory with a name still in it was refused (`ENOTEMPTY`). The other half
    /// of "no single call on this contract takes a subtree away": `UNLINK` will not remove a
    /// directory at all, and `RMDIR` will not remove one that holds anything.
    pub const RMDIR_REFUSED_NON_EMPTY: u64 = 1 << 18;
    /// And once the name inside it was taken out, `RMDIR` removed it. Without this, the bit
    /// above is equally true of a `RMDIR` that refuses everything.
    pub const RMDIR_REMOVED_EMPTY: u64 = 1 << 19;
    /// It opened [`crate::tree::INNER`] **by absolute path**, `/inner`, which exists only in
    /// [`crate::tree::SUB`].
    ///
    /// The pair with [`ABSOLUTE_REACHED_SECRET`] is why absolute paths cost nothing here: the
    /// same two probes as [`REACHED_INNER`] and [`REACHED_SECRET`], asked with a leading slash,
    /// and each shell reaches exactly the one its own root contains. A `/` rooted in a global
    /// namespace would make both shells reach both.
    pub const ABSOLUTE_REACHED_INNER: u64 = 1 << 20;
    /// It opened [`crate::tree::SECRET`] by absolute path, `/secret`, which exists only in
    /// [`crate::tree::OTHER`].
    pub const ABSOLUTE_REACHED_SECRET: u64 = 1 << 21;
    /// **`cd /..` was refused and nothing moved.** The negative control on the syntax: an
    /// absolute path meets the same wall `..` does, because your root is the only root there
    /// is and there is no level above it to name.
    pub const ABSOLUTE_CLAMPED_AT_ROOT: u64 = 1 << 22;
    /// `touch` on a name that was not there created it: the name now opens and its size is
    /// zero. The create half of milestone 47's `touch`, made a measurement rather than a claim.
    pub const TOUCH_CREATED: u64 = 1 << 23;
    /// `touch` on a name that was already there, holding [`crate::tree::NAV_TOUCH_BODY`],
    /// left it holding exactly that afterward. Without this bit, [`TOUCH_CREATED`] is equally
    /// true of a `touch` that quietly truncates whatever it finds.
    pub const TOUCH_PRESERVED: u64 = 1 << 24;
    /// **A bare second `touch`, real command line and all, observed a strictly later mtime**
    /// (milestone 47's mtime lane; DECISIONS §112). [`TOUCH_CREATED`] and [`TOUCH_PRESERVED`]
    /// prove the *bytes*; this is the first bit that reads a timestamp back through the wire
    /// (`GETMTIME`) at all, so it is also the first proof that the getter and the create-time
    /// stamp agree on what "before" means.
    pub const TOUCH_MTIME_ADVANCED: u64 = 1 << 25;
    /// **`touch -t 2030-01-01T00:00:00Z`, typed as a real command line, produced a mtime that
    /// reads back as exactly [`crate::tree::NAV_TOUCH_AT_UNIX`] and not the value
    /// [`TOUCH_MTIME_ADVANCED`] just observed.** This is DECISIONS §112 made a measurement: a
    /// `touch -t` that silently fell back to "now" would still be a later timestamp than the
    /// bare touch before it, so the *exact* comparison, not merely "later", is what makes this
    /// bit mean the caller's assertion moved rather than the server's own clock.
    pub const TOUCH_AT_ROUND_TRIPPED: u64 = 1 << 26;
    /// **A directory handle carrying `WRITE` and not `SETTIME` still bumped a name's mtime to
    /// now**, through the real wire and the real `fs_subtree_caretaker`, not just the
    /// `redoxfs_server` host tests' in-process `Server`. DECISIONS §112's "plain `WRITE` covers
    /// 'now'" made a measurement one layer further out: a bug in `verb::TABLE`'s row for
    /// `SETMTIME` (the wrong `needs_all`, or an off-by-one index after `SYNC`'s row was
    /// inserted ahead of it) would show up here and would not show up in a test that calls
    /// `Server::set_mtime_now` directly.
    pub const TOUCH_NOW_NEEDS_ONLY_WRITE: u64 = 1 << 27;
    /// **The same handle's `touch -t` attempt was refused.** The pair with
    /// [`TOUCH_NOW_NEEDS_ONLY_WRITE`] is what makes DECISIONS §112's "no, they are not the same
    /// right" non-vacuous through the real wire: both probes run against **one** handle, so a
    /// caretaker or a table row that let `SETMTIME_AT` through on `WRITE` alone would set this
    /// bit's sibling and clear this one, and a caretaker that refused everything would clear
    /// both.
    pub const TOUCH_AT_REFUSED_WITHOUT_SETTIME: u64 = 1 << 28;
    /// **`ls` of a bound name listed the real directory it points at**: [`crate::tree::NAV_BIND_MARKER`]
    /// appeared in a listing of [`crate::tree::NAV_BIND_ALIAS`], which this shell never `mkdir`ed
    /// or `cd`ed into directly. `bind`'s own claim (milestone 47/154), made a measurement: a
    /// bound name reaches what the position it was given reaches, over the real wire.
    pub const BIND_REACHED_TARGET: u64 = 1 << 29;
    /// **`..` from inside a bound path landed at the bind's own real parent**, not at some
    /// boundary invented at the alias: `cd /<alias>` followed by one `cd ..` rendered
    /// [`crate::tree::NAV_DIR`]'s own run-indexed path (`pwd` shows the real tree, not the
    /// name used to reach it, because a position has no memory of how it was reached). Proves
    /// `bind` composes the underlying position's own resolution rather than re-clamping at
    /// the alias.
    pub const BIND_ASCEND_REACHES_REAL_PARENT: u64 = 1 << 30;
    /// **A third `cd ..` from there refused at this shell's own root**, exactly where a direct
    /// walk to the same depth would have: the negative control that a bind cannot manufacture
    /// either more reach or a new boundary, only a second name for a position already held.
    pub const BIND_STOPS_AT_TRUE_ROOT: u64 = 1 << 31;
}

/// **What the globbing witness reports** (milestone 47's globbing lane): a bitmap, for the same
/// reason every other witness here reports one, and with the same vacuity control at the end.
///
/// The shell it runs in holds a real directory capability served by a real
/// `fs_subtree_caretaker`, so what is under test is the prompt's own expansion path over a real
/// `READDIR`, not a reimplementation of it.
pub mod globscape {
    /// `echo` expanded the pattern into a non-empty set. The precondition for everything else:
    /// a shell that expanded nothing agrees with a grant of nothing, perfectly and uselessly.
    pub const EXPANDED: u64 = 1 << 0;
    /// **The names `echo` printed and the names the grant carries are the same bytes.** The
    /// headline: the expansion you see is the authority that moves.
    pub const AGREED: u64 = 1 << 1;
    /// The expansion contains neither [`crate::tree::GLOB_MISS`] nor [`crate::tree::GLOB_DIR`],
    /// which exist in the same directory and are one entry away. Without this, [`AGREED`] is
    /// equally true of a shell that expanded a pattern to everything it could see.
    pub const EXCLUDED_A_STRANGER: u64 = 1 << 2;
    /// A pattern that matched nothing was **refused**, rather than printed as itself. zsh's
    /// answer, and the model's: an empty expansion is an empty grant.
    pub const NO_MATCH_REFUSED: u64 = 1 << 3;
    /// A pattern in a leading path component was refused. Selecting directories to walk is an
    /// authority question, so it is not one a matcher answers (notes/glob.md).
    pub const PATTERN_IN_PATH_REFUSED: u64 = 1 << 4;
    /// A line with no pattern in it came out of `echo` **byte for byte**, spacing included.
    /// Globbing is not allowed to quietly turn `echo` into a word-splitter.
    pub const TEXT_UNTOUCHED: u64 = 1 << 5;
    /// Something that should have worked did not, so nothing above proves anything. Always a
    /// failure.
    pub const GLOB_FAILED: u64 = 1 << 6;
}

/// **What the extended-attribute witness reports** (milestone 57), a bitmap for the reason every
/// other witness here reports one: the kernel test asserts an *exact* set, so a client that could
/// do nothing and a client that could do everything both fail, and a failure names itself.
///
/// It rides the third word of the FS client's proof report, beside the `motd` head and the
/// success sentinel, because the attribute layer is served by the same FS server over the same
/// endpoint and a second boot of the whole three-process stack would prove nothing new.
pub mod attrs {
    /// An attribute was set and read back with **its type code and its bytes**. The type code
    /// half is what makes this more than a byte-string round trip: a layer that dropped the kind
    /// would pass every other bit here.
    pub const SET_AND_READ_BACK: u64 = 1 << 0;
    /// The listing named it, and named nothing else. "Nothing else" is the half that catches a
    /// store keyed on the wrong node, which would show up as somebody else's attributes.
    pub const LISTED: u64 = 1 << 1;
    /// **The attribute followed its file across a rename.** The headline property, and the one
    /// an AppleDouble sidecar gets wrong. It works because the store keys on the node and a
    /// rename changes a directory entry, so nothing in the rename path knows attributes exist.
    pub const SURVIVED_RENAME: u64 = 1 << 2;
    /// After a remove, reading it answers `ENODATA`. Without this, [`SET_AND_READ_BACK`] is
    /// equally consistent with a store that never forgets anything.
    pub const GONE_AFTER_REMOVE: u64 = 1 << 3;
    /// **A file recreated at a name whose previous occupant had attributes has none.** The
    /// on-device half of "deletion is in the same transaction as the file's": a leaked blob is
    /// not wasted space, it is somebody else's metadata on a file that never asked for it.
    pub const GONE_AFTER_UNLINK: u64 = 1 << 4;
    /// A value past [`filesystem_contract::xattr::MAX_VALUE`] was refused with
    /// [`filesystem_contract::xattr::E2BIG`], loudly, rather than stored short (DECISIONS §42 (a filesystem declares what it offers and must be truthful)).
    pub const OVERSIZE_REFUSED: u64 = 1 << 5;
    /// The store's directory could not be opened, created, or descended into. A client that
    /// could name it could read any file's attributes as ordinary bytes.
    pub const STORE_UNNAMEABLE: u64 = 1 << 6;
    /// And a full enumeration of the root did not name it, which is the other half: what cannot
    /// be named is also not listed. Reported only if the enumeration ran to the end and returned
    /// something, so an empty listing cannot pass as a clean one.
    pub const STORE_UNLISTED: u64 = 1 << 7;
    /// **Something that should have worked did not**, so nothing above proves anything. The
    /// control bit, in the shape every other witness here uses.
    pub const ATTRS_FAILED: u64 = 1 << 8;

    /// What a correct run reports, stated once so the two ISA legs' assertions cannot drift.
    pub const EXPECTED: u64 = SET_AND_READ_BACK
        | LISTED
        | SURVIVED_RENAME
        | GONE_AFTER_REMOVE
        | GONE_AFTER_UNLINK
        | OVERSIZE_REFUSED
        | STORE_UNNAMEABLE
        | STORE_UNLISTED;

    /// The file the witness makes, sets an attribute on, and renames. Its own name, so nothing
    /// it leaves behind can be confused with a fixture the post-run host check pins.
    pub const PROBE: &str = "attr-probe";
    /// What it renames [`PROBE`] to. The attribute has to be readable through this name.
    pub const PROBE_MOVED: &str = "attr-moved";
    /// The attribute the witness writes, spelled the way Samba spells one.
    pub const NAME: &[u8] = b"user.com.apple.metadata";
    /// Its value: opaque bytes, which is exactly what `streams_xattr` stores.
    pub const VALUE: &[u8] = b"CRK57: an opaque byte string\n";
    /// Its type code, `'CSTR'` in BFS's spelling. Non-zero on purpose: a layer that silently
    /// wrote [`filesystem_contract::xattr::RAW`] would pass a test whose fixture used the default.
    pub const KIND: u32 = 0x4353_5452;
}

/// **The on-device crash test's vocabulary** (milestone 37 (prove RedoxFS's crash consistency),
/// DECISIONS §34 (RedoxFS is the primary filesystem, on three conditions) condition 1): one crash
/// driven through the real stack on its own disk, so the recovery is a real FS-server process
/// mounting an image a real virtio write left half finished. The host sweep in
/// `redoxfs_server/tests/crash_consistency.rs` proves the property exhaustively. **The two payloads
/// are the same length** because the contract's `WRITE` does not truncate (DECISIONS §27 (the
/// filesystem service: a capability-shaped contract)): a shorter second payload would leave the
/// first one's tail behind, and "the file is exactly A or exactly B" would have no answer.
pub mod crash {
    /// The file the crash driver writes. Its own name on its own disk, so nothing this test does
    /// can be confused with the shared fixture disk's `scratch`.
    pub const NAME: &str = "cut";
    /// What the image ships with. The driver's first write is **acknowledged** before the crash,
    /// so recovering this would mean an acknowledged write vanished, and the test says so.
    pub const INITIAL: &[u8] = b"CRK37-INITIAL: the value the host tool wrote before this boot ran";
    /// Payload A: written and acknowledged, and then the server is killed. It must survive.
    pub const A: &[u8] = b"CRK37-PAYLOAD-A: acknowledged before the kill, must be whole after";
    /// Payload B: the write the FS server dies in the middle of. Whole or absent, never partial.
    pub const B: &[u8] = b"CRK37-PAYLOAD-B: the write the server was killed in the middle of!";

    /// The FS server's "I am about to die" word, sent on its readiness endpoint immediately
    /// before it traps. It is what lets the kernel test start the recovery mount at a defined
    /// moment rather than guessing, and it is also the evidence that the *injector* killed the
    /// server rather than something else having gone wrong.
    pub const CUT: u64 = 0x0C07_DEAD;

    /// What the verifier found in [`NAME`] after the recovery mount, sent as its report's second
    /// word. Anything else, including silence, fails.
    pub const SAW_A: u64 = 0x0037_00A0;
    /// What the verifier found if [`NAME`] held [`B`] after recovery: the crashed write landed
    /// whole rather than being lost.
    pub const SAW_B: u64 = 0x0037_00B0;
    /// The file held something that was never one of the two payloads: a partial write, the
    /// pre-boot contents, or a length nobody asked for. This is the failure the test exists for.
    pub const SAW_NEITHER: u64 = 0x0037_00FF;
}

/// **The blank disk milestone 57's write half partitions and formats**, and the layout the two
/// programs and the post-run host check agree about. It is its own disk, regenerated every run: a
/// test that writes a partition table cannot share a fixture other tests read (DECISIONS §27). It
/// arrives as 64 MiB of zeros with no table, so a program denied entropy that wrote nothing leaves
/// a disk that still parses as nothing. notes/filesystem-fixture.md, "The blank disk", draws it.
pub mod blank {
    /// The disk's size in 512-byte logical blocks. 64 MiB, matching the `sgdisk` fixture disk so
    /// the two are comparable at a glance.
    pub const DISK_BLOCKS: u64 = 131_072;
    /// The logical block size the table counts in. Every disk this project has met uses it, and
    /// nothing on the blk wire reports it (`disk_surveyor`'s BUGS).
    pub const LBA: u64 = 512;

    /// The EFI system partition: first and last logical block, inclusive. 2048 is the 1 MiB
    /// alignment every real tool uses. Nothing formats it or [`LINUX`]; they give the table more
    /// than one entry and more than one unique GUID.
    pub const ESP: (u64, u64) = (2048, 10239);
    /// The Linux filesystem partition.
    pub const LINUX: (u64, u64) = (10240, 30719);
    /// **The nife data partition**, the one that gets a filesystem. Its start and its
    /// length are both whole multiples of RedoxFS's 4096-byte block, which `mkfs` requires
    /// and refuses without: a filesystem whose blocks straddle the partition's first byte would
    /// read fine through the offset disk that made it and be unreadable by anything else.
    pub const DATA: (u64, u64) = (30720, 129_023);

    /// How many partitions the table carries. The post-run host check asserts on it.
    pub const PARTITIONS: usize = 3;

    /// The name written into every entry's 36 UTF-16 units, in order.
    pub const NAMES: [&str; PARTITIONS] = ["nife boot", "nife root", "nife data"];

    /// The file `mkfs` creates on the filesystem it just made, so the host tool has
    /// something to read back. An empty filesystem that opens proves the header; a file the
    /// guest wrote proves the tree, the allocator and the root node too.
    pub const MADE_NAME: &str = "made-on-target";
    /// Its contents, checked byte for byte on the host after the run.
    pub const MADE_BODY: &[u8] = b"CRK57: this filesystem was created by nife on the target\n";
}

#[cfg(test)]
mod tests;
