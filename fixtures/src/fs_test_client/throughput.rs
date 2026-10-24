//! **Sequential and random throughput** through the FS server (`ROLE_THROUGHPUT`, milestone 38
//! (filesystem throughput, and the comparison)).
//!
//! Cut out of `fs_test_client.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/files-over-1500, for §282 (when a file is split, the parent becomes an index). What
//! another role uses is `pub(crate)` here.
//!
//! Name: provisional (lane maintainer/files-over-1500, 2026-10-12 UTC), for the module and the
//! file.

use filesystem_contract::fs;
use filesystem_fixture as fixture;
use user_mode_runtime::{call, exit, now, require, send};

use crate::channel::{FILE, REPORT, STAGE_WRITE, WINDOW, fail, open, put_page, read};

/// The filesystem throughput benchmark (milestone 38, DECISIONS §34 condition 2): six self-timed
/// phases over one file, each reported as `[ticks, transfers, phase]`, which the bench boot prints
/// as `fs_seq_write`, `fs_seq_read`, `fs_rand_read`, `fs_record_read`, `fs_rand_write` and
/// `fs_payload_fill`.
///
/// One request moves at most `fs::TRANSFER_MAX` bytes (`UNIT`), and each phase moves `tp::TOTAL`
/// whatever `UNIT` is. The write phase runs first because it creates the file. Random offsets come
/// from a fixed seed, page-aligned and inside the file. A read loop issues only the `CALL`; a write
/// loop repaints the page first. See notes/benchmarks/filesystem-throughput.md.
pub(crate) fn throughput() -> ! {
    use fixture::throughput as tp;

    let handle = throughput_file();

    // **Every write gets a fresh, incompressible page, and that is not fussiness.** RedoxFS stores
    // a file in records (8 KiB since milestone 138 step 1, 128 KiB before it), compresses each with
    // lz4 when it is larger than one block, and skips a write outright when the record it would
    // produce is byte-identical to the one already there. So a benchmark that sends one constant
    // page measures the short circuit on every repeat, and one that sends several copies of an
    // incompressible page into a single record measures lz4 finding the copies. The first draft
    // of this bench did both, and reported random writes as fast as random reads because none of
    // them wrote anything. See notes/benchmarks.md.
    let mut payload = 0x9E37_79B9_7F4A_7C15_u64;

    // What that costs, measured on its own so a reader can take it back out of the write phases,
    // which pay it inside their window because a client that writes data has to produce the data.
    let start = now();
    for _ in 0..tp::BLOCKS {
        paint_page(&mut payload);
    }
    let ticks = now().wrapping_sub(start);
    send(REPORT, ticks, tp::BLOCKS, tp::PAYLOAD_FILL);

    // --- Phase 1: sequential write. This is also the file's creation. ---
    // **No warmup here, on purpose.** Every other phase warms up to keep a cold first touch out of
    // the window; this phase's whole subject is the cold path, because writing a file that did not
    // exist is what allocates its blocks. Warming it would mean overwriting the first few blocks
    // before timing them, which measures a different operation than the rest of the loop.
    let start = now();
    for k in 0..tp::BLOCKS {
        paint_page(&mut payload);
        write_unit(handle, k * tp::UNIT as u64);
    }
    let ticks = now().wrapping_sub(start);
    // The file is exactly as long as the phase claims to have written it. Without this a server
    // that answered every write with a short count would still produce a throughput number.
    let (size, _) = call(FILE, fs::req(fs::FSTAT, handle, 0), 0);
    require(size == tp::BLOCKS * tp::UNIT as u64);
    send(REPORT, ticks, tp::BLOCKS, tp::SEQ_WRITE);

    // --- Phase 2: sequential read of what phase 1 left behind. ---
    for _ in 0..tp::WARMUP {
        require(read(handle, 0, tp::UNIT) == tp::UNIT);
    }
    let start = now();
    let mut got = 0u64;
    for k in 0..tp::BLOCKS {
        got += read(handle, k * tp::UNIT as u64, tp::UNIT) as u64;
    }
    let ticks = now().wrapping_sub(start);
    require(got == tp::BLOCKS * tp::UNIT as u64);
    send(REPORT, ticks, tp::BLOCKS, tp::SEQ_READ);

    // --- Phase 3: random read, page-aligned, inside the file. ---
    let mut rng = 0x38F5_7042_1DEA_u64;
    for _ in 0..tp::WARMUP {
        require(read(handle, next_offset(&mut rng), tp::UNIT) == tp::UNIT);
    }
    let start = now();
    let mut got = 0u64;
    for _ in 0..tp::BLOCKS {
        got += read(handle, next_offset(&mut rng), tp::UNIT) as u64;
    }
    let ticks = now().wrapping_sub(start);
    require(got == tp::BLOCKS * tp::UNIT as u64);
    send(REPORT, ticks, tp::BLOCKS, tp::RAND_READ);

    // --- Phase 5, out of order on purpose: the cheapest 4 KiB read there is. ---
    // Record-aligned, so `read_record` fetches one block instead of averaging over a record's
    // worth. It runs before the second write phase so that it reads a file no write has touched
    // since the read phases above, and its gap to the sequential read is the record geometry with
    // everything else held identical. See `fixture::throughput::RECORD_READ`.
    let records = (tp::BLOCKS * tp::UNIT as u64) / tp::RECORD;
    for k in 0..tp::WARMUP {
        require(read(handle, (k % records) * tp::RECORD, tp::UNIT) == tp::UNIT);
    }
    let start = now();
    let mut got = 0u64;
    for k in 0..tp::BLOCKS {
        got += read(handle, (k % records) * tp::RECORD, tp::UNIT) as u64;
    }
    let ticks = now().wrapping_sub(start);
    require(got == tp::BLOCKS * tp::UNIT as u64);
    send(REPORT, ticks, tp::BLOCKS, tp::RECORD_READ);

    // --- Phase 4: random write, in place, over blocks that already exist. The offset stream runs
    // on from phase 3 rather than restarting, so no timed write lands on a block the warmup just
    // touched. ---
    for _ in 0..tp::WARMUP {
        paint_page(&mut payload);
        write_unit(handle, next_offset(&mut rng));
    }
    let start = now();
    for _ in 0..tp::BLOCKS {
        paint_page(&mut payload);
        write_unit(handle, next_offset(&mut rng));
    }
    let ticks = now().wrapping_sub(start);
    // The file did not grow: a random write that appended would be measuring allocation.
    let (size, _) = call(FILE, fs::req(fs::FSTAT, handle, 0), 0);
    require(size == tp::BLOCKS * tp::UNIT as u64);
    send(REPORT, ticks, tp::BLOCKS, tp::RAND_WRITE);

    exit();
}

/// Open the throughput file, creating it if this is the first run against this image and emptying
/// it if it is not. `CREATE` answers `EEXIST` rather than opening (`filesystem_contract`'s note says why), so
/// the fallback is an explicit open-and-truncate.
fn throughput_file() -> u64 {
    let name = fixture::THROUGHPUT_NAME;
    put_page(name.as_bytes());
    let (r0, _) = call(FILE, fs::req(fs::CREATE, 0, name.len() as u64), 0);
    if (r0 as i64) >= 0 {
        return r0;
    }
    let h = open(name);
    let (t, _) = call(FILE, fs::req(fs::TRUNCATE, h, 0), 0);
    require((t as i64) >= 0);
    h
}

/// One timed write: a page of the shared page's current contents at `offset`. No `put_page`, so the
/// measured window holds the request and not a 4 KiB byte-at-a-time fill.
fn write_unit(handle: u64, offset: u64) {
    let unit = fixture::throughput::UNIT;
    let (r0, _) = call(FILE, fs::req(fs::WRITE, handle, unit as u64), offset);
    if (r0 as i64) < 0 {
        fail(STAGE_WRITE, r0);
    }
    require(r0 as usize == unit);
}

/// **Fill the whole shared page with fresh, incompressible bytes**, advancing `state`. Eight bytes
/// at a time, so it is 512 stores rather than 4096; the cost is reported as its own row
/// (`fs_payload_fill`) because the write phases pay it inside their timed window.
fn paint_page(state: &mut u64) {
    let mut i = 0;
    while i < fixture::throughput::UNIT {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        WINDOW.write::<u64>(i as u64, x);
        i += 8;
    }
}

/// The next page-aligned offset inside the throughput file, from a xorshift64 the seed makes
/// reproducible. A benchmark whose access pattern changes between runs cannot be compared with
/// itself, let alone with another system.
fn next_offset(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    (x % fixture::throughput::BLOCKS) * fixture::throughput::UNIT as u64
}
