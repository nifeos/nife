//! **Is RedoxFS actually crash consistent?** (milestone 37, DECISIONS §34 condition 1)
//!
//! §34 made RedoxFS the primary filesystem on three conditions, and the first was that crash
//! consistency stop being a claim taken from the upstream design description. See
//! notes/fs-server/crash-consistency.md.

//! # The property, stated so it can fail
//!
//! A workload's operations each return only after the engine commits. With `S(p)` the state after
//! the first `p`: for every point at which the device could stop, a fresh mount recovers exactly
//! `S(p)` for some `p`, never a blend, a half-applied operation, an unwritten length or a failed
//! mount. That implies "every acknowledged write is wholly present or absent" and also forbids a
//! later operation surviving an earlier one. `p` must be non-decreasing as the cut advances, and the
//! whole workload at the last cut, so a filesystem recovering `S(0)` every time fails.

//! # Determinism, because a crash harness that leaks state manufactures facts
//!
//! Every fault point starts from a byte-identical clone of one in-memory image, built in this
//! process by `FileSystem::create` and never written to, so nothing carries between iterations or
//! runs (DECISIONS §27 records the day a fixture one run mutated and another asserted on cost).

//! # The controls
//!
//! `the_ring_is_what_saves_it` and `a_torn_commit_is_a_header_the_engine_rejects` are the negative
//! controls, and not optional: without them a passing suite is equally consistent with an injector
//! that does nothing at all.

#![cfg(feature = "hosttest")]

use redoxfs::{FileSystem, HEADER_RING, Node, TreePtr};
use redoxfs_server::crash::{
    Damage, MemIo, Recording, Write, blank_image, damaged_image, header_at, only_this_generation,
};
use redoxfs_server::{BLOCK, BlockDisk, NameCopy, Server};
/// A fixed name as the [`NameCopy`] every `Server` verb takes; the server copies a client's names
/// out of its window the same way.
fn named(name: &str) -> NameCopy {
    NameCopy::copy_in(name.as_bytes()).expect("a fixed name fits one block and is UTF-8")
}

/// The image size the fixture is built at. Big enough for the record-spanning write below to
/// copy-on-write itself several times over, small enough that cloning it once per fault point is
/// cheap: the sweeps below do that a few hundred times.
const IMAGE_MIB: usize = 8;

/// The names the workload touches. A snapshot reads all of them, so a file that should not exist
/// yet is part of the state rather than something the test forgets to look at.
const NAMES: [&str; 4] = ["motd", "scratch", "made", "renamed"];

/// One name's whole observable state: its bytes, and its extended attributes in store order.
///
/// The attributes are part of the state rather than a second test, and that is the point (milestone
/// 57). They live in a **separate file** from the data they describe (`.nife-attrs/<node id>`),
/// so "the file and its metadata survive a cut together" is a claim about two writes reaching the
/// platter in one commit, which is exactly what a prefix-consistency sweep can decide and nothing
/// else here can. Before this the property was an argument from the transaction boundary.
type Attrs = Vec<(Vec<u8>, u32, Vec<u8>)>;
type Entry = (Vec<u8>, Attrs);

/// The state of the filesystem: every name, and its contents, or `None` if it does not exist.
type State = Vec<(&'static str, Option<Entry>)>;

/// Read the whole state out of a mounted server. Reads only, so calling it never perturbs the write
/// log a [`Recording`] is building.
///
/// Fallible on purpose: a failed read is a detection, and the lying-device test must tell "the
/// filesystem refused these bytes" from "it handed back the wrong ones". `ENOENT` is the only error
/// that means absent: a dropped write to a directory's tree block makes a lookup answer `EIO`, and
/// reading that as absence once reported nine states that never existed
/// (notes/fs-server/crash-consistency.md).
fn snapshot<D: redoxfs::Disk>(srv: &mut Server<D>) -> Result<State, syscall::error::Error> {
    let mut state = State::new();
    for &name in &NAMES {
        let content = match srv.open_file(&named(name)) {
            Err(e) if e.errno == syscall::error::ENOENT => None, // no such name in this state
            Err(e) => return Err(e), // anything else is the filesystem refusing, not an absence
            Ok(h) => {
                let size = srv.fstat(h)? as usize;
                let mut out = vec![0u8; size];
                let mut off = 0usize;
                // Chunked because the server reads through one page at a time on device and this
                // test drives the same core; a short read is normal, a zero-length one is EOF.
                while off < size {
                    let n = srv.read(h, off as u64, &mut out[off..])?;
                    if n == 0 {
                        break;
                    }
                    off += n;
                }
                out.truncate(off);
                let attrs = attrs_of(srv, h)?;
                srv.close(h)?;
                Some((out, attrs))
            }
        };
        state.push((name, content));
    }
    Ok(state)
}

/// Every attribute on one handle, name, kind and value, in the order the store holds them.
///
/// Fallible for the same reason [`snapshot`] is: a store file whose checksum does not match makes
/// this fail, and that is a **detection**, not an empty attribute set. Reading a refusal as absence
/// is the exact mistake that invented nine filesystem bugs the first time this harness was written,
/// and it would be easier to make here, because "this file has no attributes" is the common case.
fn attrs_of<D: redoxfs::Disk>(
    srv: &mut Server<D>,
    handle: u32,
) -> Result<Attrs, syscall::error::Error> {
    let mut page = [0u8; 4096];
    let n = srv.list_xattr(handle, &mut page)?;
    let names: Vec<Vec<u8>> = filesystem_contract::xattr::list::iter(&page[..n])
        .map(|s| s.to_vec())
        .collect();
    let mut out = Vec::new();
    for name in names {
        let mut value = [0u8; 4096];
        let (kind, len) = srv.get_xattr(handle, &name, &mut value)?;
        out.push((name, kind, value[..len].to_vec()));
    }
    Ok(out)
}

/// What a fresh mount of a damaged platter produced. Three outcomes, and the fourth (a state that
/// never existed) is not in this enum because it is not an outcome the tests tolerate: it is the
/// bucket [`Outcome::Recovered`] is checked against.
enum Outcome {
    /// The mount refused the image. The earliest possible detection.
    NoMount,
    /// It mounted, and then refused to hand back a file's bytes. Also a detection: RedoxFS checks a
    /// seahash on every block it reads through a pointer, so a lost or torn block is an error.
    NoRead,
    /// It mounted and read clean.
    Recovered(State),
}

/// Mount a reconstructed platter with **exactly the path the FS server always mounts with**
/// ([`Server::open`], which is `FileSystem::open(.., cleanup: true)`), and read the whole state out.
fn recover(image: Vec<u8>) -> Outcome {
    match Server::open(BlockDisk(MemIo(image))) {
        Err(_) => Outcome::NoMount,
        Ok(mut srv) => match snapshot(&mut srv) {
            Err(_) => Outcome::NoRead,
            Ok(state) => Outcome::Recovered(state),
        },
    }
}

/// A tagged, position-dependent payload: a stale read, a shifted read, or a block from a different
/// operation cannot match one of these by accident.
///
/// **The filler is a xorshift rather than a pattern, and that is not decoration.** RedoxFS lz4s
/// every record, so a compressible 160 KiB payload becomes a couple of blocks and the record path
/// this test exists to attack is never taken. The first version used `(i as u8) * 31`, which repeats
/// every 256 bytes: the whole workload issued 55 block writes. With incompressible bytes it issues
/// well over a hundred, most of them the multi-block data writes that make "wholly present or wholly
/// absent" a claim worth checking.
fn tagged(tag: u8, len: usize) -> Vec<u8> {
    let mut v = vec![0u8; len];
    let mut x = 0x243f_6a88_85a3_08d3u64 ^ (tag as u64);
    for b in v.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = x as u8;
    }
    v[..8].copy_from_slice(b"CRK37___");
    v[7] = tag;
    v
}

/// One recorded run of the workload: the disk as it stood before the first write, every write the
/// engine issued in order, and the state after each acknowledged operation.
struct Recorded {
    pristine: Vec<u8>,
    log: Vec<Write>,
    /// `states[p]` is `S(p)`: the state after the first `p` operations. `states[0]` is the image as
    /// the host tool made it, before the mount had written anything.
    states: Vec<State>,
}

/// Build the fixture and run the workload once, recording every block write. Each step attacks a
/// shape a crash is likely to catch out, in a real client's order:
///
/// 1. an overwrite of an existing file;
/// 2. a shorter overwrite, leaving the old tail (DECISIONS §27), so a torn recovery is visible;
/// 3. the `TRUNCATE` that removes it, a change to the file's shape rather than its bytes;
/// 4. a `CREATE`, which extends a directory;
/// 5. a 160 KiB write, over RedoxFS's 128 KiB record, so one operation is forty block writes;
/// 6. a 60 KiB shrink of it, which frees records; 7. an overwrite, so the biggest is not last;
/// 8. a `RENAME`, changing two entries at once (§42): "neither" and "both" fail the sweep.
fn record_workload() -> Recorded {
    // The fixture, built the way the running system's is: the host tool creates the filesystem and
    // puts the files in, and the server only ever opens it.
    let mut fs = FileSystem::create(BlockDisk(MemIo(blank_image(IMAGE_MIB))), None, 0, 0)
        .expect("mkfs the fixture");
    fs.tx(|tx| {
        for (name, body) in [
            ("motd", &b"redoxfs served this file to an EL0 client\n"[..]),
            (
                "scratch",
                &b"(placeholder overwritten by the crash test)"[..],
            ),
        ] {
            let ptr = tx
                .create_node(TreePtr::root(), name, Node::MODE_FILE | 0o644, 0, 0)?
                .ptr();
            tx.write_node(ptr, 0, body, 0, 0)?;
        }
        Ok(())
    })
    .expect("populate the fixture");
    let pristine = fs.disk.0.0;

    // From here every block write is recorded, including the ones the mount itself does: RedoxFS
    // opens with `cleanup: true`, which releases unused nodes and commits, so a fault point can land
    // inside the recovery mount's own transaction. That case is worth having and is easy to miss.
    let mut srv =
        Server::open(BlockDisk(Recording::new(pristine.clone()))).expect("mount the fixture");

    let mut states = vec![snapshot(&mut srv).expect("snapshot the mounted fixture")];
    let mark = |srv: &mut Server<_>, states: &mut Vec<State>, operation: u32| {
        states.push(
            snapshot(srv).unwrap_or_else(|e| panic!("snapshot after operation {operation}: {e:?}")),
        );
    };
    let big = tagged(b'5', 160 * 1024);

    let h = srv.open_file(&named("scratch")).expect("open scratch");
    srv.write(h, 0, &tagged(b'1', 64)).expect("operation 1");
    mark(&mut srv, &mut states, 1);
    srv.write(h, 0, &tagged(b'2', 61)).expect("operation 2");
    mark(&mut srv, &mut states, 2);
    srv.truncate(h, 61).expect("operation 3");
    mark(&mut srv, &mut states, 3);

    let g = srv.create_file(&named("made")).expect("operation 4");
    mark(&mut srv, &mut states, 4);
    srv.write(g, 0, &big).expect("operation 5");
    mark(&mut srv, &mut states, 5);
    srv.truncate(g, 100 * 1024).expect("operation 6");
    mark(&mut srv, &mut states, 6);

    srv.write(h, 0, &tagged(b'7', 200)).expect("operation 7");
    mark(&mut srv, &mut states, 7);

    // The rename: `made` becomes `renamed`, both of which `snapshot` reads, so a recovery that
    // found the file under both names or under neither is a state that never existed.
    let root = filesystem_contract::fs::ROOT as u32;
    srv.rename(root, &named("made"), root, &named("renamed"))
        .expect("operation 8");
    mark(&mut srv, &mut states, 8);

    // Operations 9 to 12 are the attribute ones (milestone 57), interleaved with a write to the
    // same file: an attribute and its file live in different files on the platter, and a recovery
    // holding one change without the other never existed and fails the sweep. 9 creates the store
    // and the node's first blob (two creations in one commit), 11 grows the blob, and 12 shrinks
    // it, the path that must truncate afterwards (DECISIONS §27, notes/xattr.md).
    srv.set_xattr(
        h,
        b"user.DOSATTRIB",
        filesystem_contract::xattr::RAW,
        b"0x20",
    )
    .expect("operation 9");
    mark(&mut srv, &mut states, 9);
    srv.write(h, 0, &tagged(b'a', 300)).expect("operation 10");
    mark(&mut srv, &mut states, 10);
    srv.set_xattr(h, b"user.com.apple.FinderInfo", 0x4353_5452, &[7u8; 32])
        .expect("operation 11");
    mark(&mut srv, &mut states, 11);
    srv.remove_xattr(h, b"user.DOSATTRIB")
        .expect("operation 12");
    mark(&mut srv, &mut states, 12);

    // Sanity on the workload itself, so a test that silently stopped exercising anything fails here
    // rather than passing everywhere below.
    assert_eq!(states.len(), 13, "twelve operations plus the initial state");
    for w in states.windows(2) {
        assert_ne!(
            w[0], w[1],
            "an operation that changed nothing proves nothing"
        );
    }

    Recorded {
        pristine,
        log: srv.into_disk().0.into_log(),
        states,
    }
}

/// Which `S(p)` a recovered state is, or `None` if it is a state that never existed.
fn prefix_of(states: &[State], observed: &State) -> Option<usize> {
    states.iter().position(|s| s == observed)
}

/// The ring slot the next commit after write `landed` was aimed at, which is the generation a mount
/// with no history to fall back on would have to trust. Used by the negative control.
fn next_commit_slot(log: &[Write], landed: usize) -> u64 {
    log[landed..]
        .iter()
        .chain(log[..landed].iter().rev())
        .find(|w| w.is_commit())
        .expect("the workload committed at least once")
        .block
}

// ===========================================================================================
// The three injections
// ===========================================================================================

/// **A power cut at every single point the device could stop.**
///
/// This is the kill-mid-transaction case as well as the power-cut case: the reconstructed platter
/// is all there is, and the mount that reads it carries nothing over from the process that died.
#[test]
fn a_power_cut_at_every_write_recovers_a_state_that_really_existed() {
    let Recorded {
        pristine,
        log,
        states,
    } = record_workload();
    let commits = log.iter().filter(|w| w.is_commit()).count();
    assert!(
        commits >= states.len() - 1,
        "each operation commits at least once: {commits} commits for {} operations",
        states.len() - 1
    );

    let mut last = 0usize;
    for landed in 1..=log.len() {
        let image = damaged_image(&pristine, &log, Damage::Cut { landed });
        let observed = match recover(image) {
            Outcome::NoMount => panic!(
                "a cut after write {landed} of {} would not mount at all, through the FS server's \
                 own `cleanup: true` open",
                log.len()
            ),
            Outcome::NoRead => panic!(
                "a cut after write {landed} of {} mounted and then refused to read a file back",
                log.len()
            ),
            Outcome::Recovered(state) => state,
        };
        let p = prefix_of(&states, &observed).unwrap_or_else(|| {
            panic!(
                "a cut after write {landed} of {} recovered a filesystem that never existed: \
                 no prefix of the workload produces it",
                log.len()
            )
        });
        assert!(
            p >= last,
            "recovery went BACKWARDS: a cut after write {landed} recovered S({p}) where a cut one \
             write earlier recovered S({last}). A later crash must never lose more.",
        );
        last = p;
    }
    assert_eq!(
        last,
        states.len() - 1,
        "with every write landed, nothing may be lost: an acknowledged write must be present, not \
         merely absent-or-present",
    );
    eprintln!(
        "power cut: {} fault points, {commits} of them commits, all prefix-consistent",
        log.len()
    );
}

/// **A power cut that caught a write in flight**, at every point and at four tear offsets.
///
/// A torn block is the physically honest shape of a power cut: the drive was mid-sector when the
/// rail collapsed, so the block holds new bytes in front and old bytes behind. The offsets straddle
/// a sector boundary (512), the middle of the block, and a point past the seahash-covered region, so
/// a tear that a checksum would happen to miss is included rather than assumed away.
#[test]
fn a_torn_final_write_at_every_point_recovers_a_state_that_really_existed() {
    let Recorded {
        pristine,
        log,
        states,
    } = record_workload();

    for &bytes in &[8usize, 512, BLOCK / 2, BLOCK - 8] {
        let mut last = 0usize;
        for landed in 1..=log.len() {
            let image = damaged_image(&pristine, &log, Damage::TornCut { landed, bytes });
            let observed = match recover(image) {
                Outcome::NoMount => {
                    panic!("a cut tearing write {landed} at {bytes} bytes would not mount")
                }
                Outcome::NoRead => panic!(
                    "a cut tearing write {landed} at {bytes} bytes mounted and then refused a read"
                ),
                Outcome::Recovered(state) => state,
            };
            let p = prefix_of(&states, &observed).unwrap_or_else(|| {
                panic!(
                    "a cut tearing write {landed} at {bytes} bytes recovered a filesystem that \
                     never existed"
                )
            });
            assert!(
                p >= last,
                "recovery went backwards at torn write {landed} ({bytes} bytes): S({p}) after S({last})",
            );
            last = p;
        }
    }
    eprintln!(
        "torn cut: {} fault points x 4 tear offsets, all prefix-consistent",
        log.len()
    );
}

/// A device that lies: it acknowledged a write and never persisted it (or half-persisted it), then
/// carried on persisting everything after.
///
/// No filesystem can promise to survive this: RedoxFS's `Disk` has no flush or barrier, so a valid
/// header can point at a block that never landed. What RedoxFS guarantees is that it is never
/// silent, since every `BlockPtr` carries a seahash checked on every read. So the test asserts that
/// no outcome is silently wrong: each fault point lands in one of three buckets, and the fourth,
/// bytes nobody ever wrote, must be empty.
#[test]
fn a_lying_device_is_detected_and_never_silently_wrong() {
    let Recorded {
        pristine,
        log,
        states,
    } = record_workload();

    let (mut recovered, mut refused_mount, mut refused_read) = (0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();

    for index in 1..=log.len() {
        for damage in [
            Damage::Dropped { index },
            Damage::Torn { index, bytes: 2048 },
        ] {
            let image = damaged_image(&pristine, &log, damage);
            match recover(image) {
                Outcome::NoMount => refused_mount += 1,
                Outcome::NoRead => refused_read += 1,
                // Losing a block that nothing live points at (a freed block, or the header of a
                // generation a later one superseded) costs nothing: this is a real recovery.
                Outcome::Recovered(observed) => match prefix_of(&states, &observed) {
                    Some(_) => recovered += 1,
                    None => wrong.push(format!("{damage:?}")),
                },
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "a lying device produced {} SILENTLY WRONG filesystems, which is the one outcome RedoxFS's \
         per-pointer checksums are supposed to make impossible: {wrong:?}",
        wrong.len(),
    );
    assert!(
        refused_mount + refused_read > 0,
        "not one dropped or torn write was refused, at the mount or at a read, so this suite is \
         measuring an injector that does nothing",
    );
    eprintln!(
        "lying device: {} fault points x 2 damages -> {recovered} recovered, {refused_mount} \
         refused at mount, {refused_read} refused at read, 0 silently wrong",
        log.len(),
    );
}

// ===========================================================================================
// The negative controls, which are what make the three above mean anything
// ===========================================================================================

/// **The control: the ring's history is the only reason any of that worked.**
///
/// The recovery mechanism is the scan in `FileSystem::open`: read all 256 header slots, keep the
/// newest whose seahash still checks out, ignore the rest. Take the older generations away and the
/// mount has exactly one header to trust, so a fault point that damaged the newest commit now has
/// nowhere to step back to.
///
/// If the injector were inert, blanking the ring would change nothing and every point would still
/// mount. It is not: a large fraction of the fault points become unmountable, and the ones that do
/// not are exactly the points where the newest commit had already landed whole.
#[test]
fn the_ring_is_what_saves_it() {
    let Recorded { pristine, log, .. } = record_workload();

    let mut unrecoverable = 0usize;
    for landed in 1..=log.len() {
        let mut image = damaged_image(&pristine, &log, Damage::Cut { landed });
        only_this_generation(&mut image, next_commit_slot(&log, landed));
        if !matches!(recover(image), Outcome::Recovered(_)) {
            unrecoverable += 1;
        }
    }

    assert!(
        unrecoverable > 0,
        "with the ring's history removed, every one of the {} fault points still mounted. The \
         injector is not damaging anything and the crash suite proves nothing.",
        log.len(),
    );
    // Every point strictly inside a transaction has an unlanded commit, and there are far more of
    // those than there are commits, so a majority is the honest floor. Stated as a fraction rather
    // than a bare `> 0` because `> 0` would pass on a single point.
    assert!(
        unrecoverable * 2 > log.len(),
        "only {unrecoverable} of {} fault points were unrecoverable without the ring; a cut lands \
         inside a transaction far more often than on a commit boundary, so this is too few to \
         believe",
        log.len(),
    );
    eprintln!(
        "control: without the ring's older generations, {unrecoverable} of {} fault points do not \
         mount at all. With it, none of them fail.",
        log.len(),
    );
}

/// **The control, stated at the byte level: a torn commit really is a header the engine rejects.**
///
/// The test above shows the ring matters. This shows the injector's damage is the specific thing
/// the ring is defending against, without going through a mount at all: tear a commit write in half
/// and the header in that slot fails `Header::valid()`, while the previous generation's slot is
/// still valid and still older. That is the whole recovery argument in three assertions.
#[test]
fn a_torn_commit_is_a_header_the_engine_rejects() {
    let Recorded { pristine, log, .. } = record_workload();

    // The last commit of the workload: the newest generation, and the one a torn write destroys.
    let (i, last_commit) = log
        .iter()
        .enumerate()
        .rfind(|(_, w)| w.is_commit())
        .expect("the workload committed");
    let slot = last_commit.block;
    assert!(slot < HEADER_RING, "a commit writes into the header ring");

    let whole = damaged_image(&pristine, &log, Damage::Cut { landed: i + 1 });
    let generation = header_at(&whole, slot).generation();
    assert!(
        header_at(&whole, slot).valid(),
        "the undamaged commit must be valid, or the control is measuring the wrong block",
    );

    let torn = damaged_image(
        &pristine,
        &log,
        Damage::TornCut {
            landed: i + 1,
            bytes: 2048,
        },
    );
    assert!(
        !header_at(&torn, slot).valid(),
        "a commit torn at 2048 bytes was still accepted as a valid header, so the injector is not \
         reaching the thing the ring protects",
    );

    // And the generation a recovering mount falls back to is genuinely an older one, which is what
    // makes "step back a generation" a real step rather than a relabelling of the same state.
    let previous = log[..i]
        .iter()
        .rfind(|w| w.is_commit())
        .expect("the workload committed more than once");
    assert!(
        header_at(&torn, previous.block).valid(),
        "the previous generation must survive a tear of the newest one",
    );
    assert!(
        header_at(&torn, previous.block).generation() < generation,
        "the fallback must be an older generation",
    );
}
