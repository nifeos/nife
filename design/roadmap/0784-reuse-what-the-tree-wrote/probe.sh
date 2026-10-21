#!/bin/sh
# Probes behind measured-for-the-rulings.md, beside this script. Name provisional.
#
#   probe.sh heap      # (d): the benchmark below on the host, then each allocator's code size on aarch64
#   probe.sh object    # (g): the crates `object` brings with the features a converter would need
#   probe.sh uutils    # (h): build uu_printenv, uu_wc, uu_rm and uu_date for the three nife targets
#
# Every probe is generated OUTSIDE this repository, under $WORK, so this tree's Cargo.lock and
# rust-toolchain.toml stay untouched and nothing here is a dependency of nife. Versions are pinned
# with `=` so a re-run measures the same code. `uutils` builds against this worktree's patched std
# farm, the filesystem-crate probes' recipe (notes/filesystem-crates-2026-10-07/probe.sh); run
# `cargo xtask std-src` first.
#
# BUGS: needs the network. `uutils` builds and links only; nothing here runs a program under nife.
# The heap figures are host figures (see the appendix for what that leaves out).
set -eu
HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH='' cd -- "$HERE/../../.." && pwd)
WORK="${WORK:-${TMPDIR:-/tmp}/nife-784-probes}"
MODE=${1:?usage: probe.sh heap|object|uutils}
mkdir -p "$WORK"
# Outside the repository rust-toolchain.toml does not apply, so the host probes name its pin.
PIN=$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$ROOT/rust-toolchain.toml")

crate() { # crate <dir> <kind> <dependency lines>
    mkdir -p "$WORK/$1/src"
    printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n' "$1" > "$WORK/$1/Cargo.toml"
    [ "$2" = lib ] && printf '[lib]\ncrate-type = ["staticlib"]\n' >> "$WORK/$1/Cargo.toml"
    printf '[dependencies]\n%s\n[profile.release]\noverflow-checks = true\npanic = "abort"\nlto = true\ncodegen-units = 1\n' "$3" >> "$WORK/$1/Cargo.toml"
}

case "$MODE" in
heap)
    crate heap-bench bin "user_mode_heap = { path = \"$ROOT/crates/user_mode_heap\" }
talc = { version = \"=5.1.1\", default-features = false }"
    cat > "$WORK/heap-bench/src/main.rs" <<'RUST'
// The (d) measurement: `user_mode_heap::Heap` against `talc` 5.1.1 on the host, each given the same
// 256 MiB arena and the same operation stream. `probe.sh heap` writes this out and runs it.
//
// Peak extent is the highest byte any allocation reached, measured from the arena's base. Over
// peak live bytes (the sum of the sizes asked for), it is the fragmentation figure, per-chunk
// overhead included. The std PAL's realloc is alloc, copy, free, so `grow` does that for ours;
// talc's `GlobalAlloc` tries growing in place first, so `grow` does that for talc.
use std::alloc::Layout;
use std::ptr::NonNull;
use std::time::Instant;

const ARENA: usize = 256 << 20;
const REPS: usize = 11;

trait Heap {
    fn name(&self) -> &'static str;
    fn alloc(&mut self, l: Layout) -> Option<NonNull<u8>>;
    unsafe fn free(&mut self, p: NonNull<u8>, l: Layout);
    unsafe fn grow(&mut self, p: NonNull<u8>, old: Layout, new: usize) -> Option<NonNull<u8>>;
}

struct Ours(user_mode_heap::Heap);
impl Heap for Ours {
    fn name(&self) -> &'static str { "user_mode_heap" }
    fn alloc(&mut self, l: Layout) -> Option<NonNull<u8>> { self.0.alloc(l) }
    unsafe fn free(&mut self, p: NonNull<u8>, l: Layout) { unsafe { self.0.dealloc(p, l) } }
    unsafe fn grow(&mut self, p: NonNull<u8>, old: Layout, new: usize) -> Option<NonNull<u8>> {
        let q = self.0.alloc(Layout::from_size_align(new, old.align()).ok()?)?;
        unsafe {
            std::ptr::copy_nonoverlapping(p.as_ptr(), q.as_ptr(), old.size());
            self.0.dealloc(p, old);
        }
        Some(q)
    }
}

type TalcCore = talc::base::Talc<talc::source::Manual, talc::base::binning::DefaultBinning>;
struct Talc(TalcCore);
impl Heap for Talc {
    fn name(&self) -> &'static str { "talc 5.1.1" }
    fn alloc(&mut self, l: Layout) -> Option<NonNull<u8>> { unsafe { self.0.allocate(l) } }
    unsafe fn free(&mut self, p: NonNull<u8>, l: Layout) { unsafe { self.0.deallocate(p.as_ptr(), l) } }
    unsafe fn grow(&mut self, p: NonNull<u8>, old: Layout, new: usize) -> Option<NonNull<u8>> {
        unsafe {
            if self.0.try_grow_in_place(p.as_ptr(), old, new) {
                return Some(p);
            }
            let q = self.0.allocate(Layout::from_size_align(new, old.align()).ok()?)?;
            std::ptr::copy_nonoverlapping(p.as_ptr(), q.as_ptr(), old.size());
            self.0.deallocate(p.as_ptr(), old);
            Some(q)
        }
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

struct Run { ns: u128, ops: u64, peak_live: usize, peak_extent: usize }

fn extent(base: usize, p: NonNull<u8>, l: Layout) -> usize { p.as_ptr() as usize + l.size() - base }

// The shape of std_heap_contention on one thread: a 48-byte box allocated and freed 200,000 times.
fn box48(h: &mut dyn Heap, _base: usize) -> Run {
    let l = Layout::from_size_align(48, 8).unwrap();
    let t = Instant::now();
    for _ in 0..200_000 {
        let p = h.alloc(l).unwrap();
        unsafe {
            std::ptr::write_volatile(p.as_ptr(), 1);
            h.free(p, l);
        }
    }
    Run { ns: t.elapsed().as_nanos(), ops: 200_000, peak_live: 0, peak_extent: 0 }
}

// 1,000 vectors, each doubled from 16 bytes to 64 KiB, with a 32-byte allocation kept after every
// doubling so a neighbor sits above the vector, as it does in a real program.
fn vecgrow(h: &mut dyn Heap, base: usize) -> Run {
    let small = Layout::from_size_align(32, 8).unwrap();
    let (mut ops, mut live, mut peak_live, mut peak_extent) = (0u64, 0usize, 0usize, 0usize);
    let mut kept = Vec::new();
    let t = Instant::now();
    for _ in 0..1_000 {
        let mut l = Layout::from_size_align(16, 8).unwrap();
        let mut p = h.alloc(l).unwrap();
        ops += 1;
        live += 16;
        while l.size() < 65536 {
            let n = l.size() * 2;
            p = unsafe { h.grow(p, l, n).unwrap() };
            live += n - l.size();
            l = Layout::from_size_align(n, 8).unwrap();
            peak_extent = peak_extent.max(extent(base, p, l));
            let s = h.alloc(small).unwrap();
            live += 32;
            peak_extent = peak_extent.max(extent(base, s, small));
            kept.push(s);
            ops += 2;
            peak_live = peak_live.max(live);
        }
        unsafe { h.free(p, l) };
        ops += 1;
        live -= l.size();
    }
    for s in kept {
        unsafe { h.free(s, small) };
        ops += 1;
    }
    Run { ns: t.elapsed().as_nanos(), ops, peak_live, peak_extent }
}

// A live set filled to about half the ceiling and then churned by a fair coin, bounded by it. Sizes: 70%
// 8 to 127 bytes, 25% up to 1 KiB, 4% up to 16 KiB, 1% up to 128 KiB; one request in twenty is
// 64-aligned. Frees pick a random live object.
fn churn(h: &mut dyn Heap, base: usize, ceiling: usize, nops: u64, seed: u64) -> Run {
    let mut r = Rng(seed);
    let mut live: Vec<(NonNull<u8>, Layout)> = Vec::with_capacity(ceiling);
    let (mut bytes, mut peak_live, mut peak_extent) = (0usize, 0usize, 0usize);
    let t = Instant::now();
    for _ in 0..nops {
        let filling = live.len() < ceiling / 4 || (live.len() < ceiling / 2 && r.next() % 2 == 0);
        let coin = live.len() < ceiling && r.next() % 2 == 0;
        if live.is_empty() || filling || coin {
            let k = r.next() % 100;
            let size = match k {
                0..70 => 8 + (r.next() % 120) as usize,
                70..95 => 128 + (r.next() % 896) as usize,
                95..99 => 1024 + (r.next() % 15360) as usize,
                _ => 16384 + (r.next() % 114688) as usize,
            };
            let align = if r.next() % 20 == 0 { 64 } else { 8 };
            let l = Layout::from_size_align(size, align).unwrap();
            let p = h.alloc(l).expect("arena exhausted");
            bytes += size;
            peak_live = peak_live.max(bytes);
            peak_extent = peak_extent.max(extent(base, p, l));
            live.push((p, l));
        } else {
            let (p, l) = live.swap_remove((r.next() as usize) % live.len());
            unsafe { h.free(p, l) };
            bytes -= l.size();
        }
    }
    let ns = t.elapsed().as_nanos();
    for (p, l) in live {
        unsafe { h.free(p, l) }
    }
    Run { ns, ops: nops, peak_live, peak_extent }
}

fn fresh(talc: bool, mem: *mut u8) -> Box<dyn Heap> {
    if talc {
        let mut t = TalcCore::new(talc::source::Manual);
        unsafe { t.claim(mem, ARENA) }.expect("claim");
        Box::new(Talc(t))
    } else {
        let mut h = user_mode_heap::Heap::new();
        unsafe { h.add_region(mem, ARENA) };
        Box::new(Ours(h))
    }
}

type Case = (&'static str, fn(&mut dyn Heap, usize) -> Run);

fn main() {
    let cases: [Case; 5] = [
        ("box48, 200k", box48),
        ("vecgrow, 1k vectors", vecgrow),
        ("churn, ~500 live, 2M ops", |h, b| churn(h, b, 1_000, 2_000_000, 0x9e3779b97f4a7c15)),
        ("churn, ~5k live, 2M ops", |h, b| churn(h, b, 10_000, 2_000_000, 0x2545f4914f6cdd1d)),
        ("churn, ~25k live, 1M ops", |h, b| churn(h, b, 50_000, 1_000_000, 0x5851f42d4c957f2d)),
    ];
    println!("| case | allocator | median ns/op | min to max | peak live KiB | peak extent / live |");
    println!("|---|---|---|---|---|---|");
    let arena = Layout::from_size_align(ARENA, 4096).unwrap();
    for (name, f) in cases {
        for talc in [false, true] {
            let mut per_op = Vec::new();
            let mut last = None;
            for _ in 0..REPS {
                let mem = unsafe { std::alloc::alloc(arena) };
                let mut h = fresh(talc, mem);
                let run = f(h.as_mut(), mem as usize);
                per_op.push(run.ns as f64 / run.ops as f64);
                last = Some((h.name(), run));
                drop(h);
                unsafe { std::alloc::dealloc(mem, arena) };
            }
            per_op.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let (heap, run) = last.unwrap();
            let ratio = if run.peak_live == 0 { "n/a".to_string() } else {
                format!("{:.2}", run.peak_extent as f64 / run.peak_live as f64)
            };
            println!("| {name} | {heap} | {:.1} | {:.1} to {:.1} | {} | {ratio} |",
                per_op[REPS / 2], per_op[0], per_op[REPS - 1], run.peak_live / 1024);
        }
    }
    println!();
    println!("size_of: Heap {} bytes, Talc {} bytes; talc's first heap reserves {} bytes of bins",
        std::mem::size_of::<user_mode_heap::Heap>(), std::mem::size_of::<TalcCore>(),
        talc::min_first_heap_size::<talc::base::binning::DefaultBinning>());
}
RUST
    (cd "$WORK/heap-bench" && cargo +"$PIN" build -q --release && ./target/release/heap-bench)
    # Code size: alloc, free and add-a-region behind extern "C", built no_std for aarch64 with LTO,
    # so only what those three reach is linked. Both builds carry the same null global allocator,
    # because talc's allocator-api2 dependency links `alloc`, which needs one.
    common='#![no_std]
use core::alloc::Layout;
#[panic_handler] fn ph(_: &core::panic::PanicInfo) -> ! { loop {} }
struct Null;
unsafe impl core::alloc::GlobalAlloc for Null { unsafe fn alloc(&self, _: Layout) -> *mut u8 { core::ptr::null_mut() } unsafe fn dealloc(&self, _: *mut u8, _: Layout) {} }
#[global_allocator] static G: Null = Null;'
    crate size-ours lib "user_mode_heap = { path = \"$ROOT/crates/user_mode_heap\" }"
    crate size-talc lib 'talc = { version = "=5.1.1", default-features = false }'
    printf '%s\n%s\n' "$common" 'static mut H: user_mode_heap::Heap = user_mode_heap::Heap::new();
#[unsafe(no_mangle)] pub unsafe extern "C" fn x_add(p: *mut u8, n: usize) { unsafe { (*(&raw mut H)).add_region(p, n) } }
#[unsafe(no_mangle)] pub unsafe extern "C" fn x_alloc(s: usize, a: usize) -> *mut u8 { unsafe { (*(&raw mut H)).alloc(Layout::from_size_align_unchecked(s, a)).map_or(core::ptr::null_mut(), |p| p.as_ptr()) } }
#[unsafe(no_mangle)] pub unsafe extern "C" fn x_free(p: *mut u8, s: usize, a: usize) { unsafe { (*(&raw mut H)).dealloc(core::ptr::NonNull::new_unchecked(p), Layout::from_size_align_unchecked(s, a)) } }' > "$WORK/size-ours/src/lib.rs"
    printf '%s\n%s\n' "$common" 'static mut H: talc::base::Talc<talc::source::Manual, talc::base::binning::DefaultBinning> = talc::base::Talc::new(talc::source::Manual);
#[unsafe(no_mangle)] pub unsafe extern "C" fn x_add(p: *mut u8, n: usize) { unsafe { let _ = (*(&raw mut H)).claim(p, n); } }
#[unsafe(no_mangle)] pub unsafe extern "C" fn x_alloc(s: usize, a: usize) -> *mut u8 { unsafe { (*(&raw mut H)).allocate(Layout::from_size_align_unchecked(s, a)).map_or(core::ptr::null_mut(), |p| p.as_ptr()) } }
#[unsafe(no_mangle)] pub unsafe extern "C" fn x_free(p: *mut u8, s: usize, a: usize) { unsafe { (*(&raw mut H)).deallocate(p, Layout::from_size_align_unchecked(s, a)) } }
#[unsafe(no_mangle)] pub unsafe extern "C" fn x_grow(p: *mut u8, s: usize, a: usize, n: usize) -> bool { unsafe { (*(&raw mut H)).try_grow_in_place(p, Layout::from_size_align_unchecked(s, a), n) } }' > "$WORK/size-talc/src/lib.rs"
    nm="$(rustc +"$PIN" --print sysroot)/lib/rustlib/$(rustc +"$PIN" -vV | sed -n 's/^host: //p')/bin/llvm-nm"
    for w in ours talc; do
        (cd "$WORK/size-$w" && cargo +"$PIN" build -q --release --target aarch64-unknown-none-softfloat)
        echo "== $w: text symbols from the allocator and the extern \"C\" wrappers"
        "$nm" --print-size --radix=d -C "$WORK/size-$w/target/aarch64-unknown-none-softfloat/release/libsize_$w.a" |
            grep -E ' [tT] ' | grep -E '(x_|user_mode_heap::|talc::)' | grep -v compiler_builtins | awk '{ s += $2; print $2, substr($0, index($0, $4)) } END { print "sum", s }'
    done
    ;;
object)
    crate object-probe lib 'object = { version = "=0.40.0", default-features = false, features = ["read_core", "elf", "write_core", "pe"] }'
    echo 'pub fn f() -> usize { let mut v = Vec::new(); object::write::pe::Writer::new(true, 0x1000, 0x200, &mut v).reserve_dos_header_and_stub(); v.len() }' > "$WORK/object-probe/src/lib.rs"
    (cd "$WORK/object-probe" && cargo +"$PIN" build -q && cargo +"$PIN" tree -e normal --prefix none | sed 's/ (\*)//' | sort -u)
    echo "== each, against this tree's Cargo.lock"
    for c in $(cd "$WORK/object-probe" && cargo +"$PIN" tree -e normal --prefix none | sed 's/ (\*)//' | awk '{print $1}' | sort -u); do
        printf '%s: ' "$c"; grep -c "^name = \"$c\"$" "$ROOT/Cargo.lock" || true
    done
    ;;
uutils)
    for u in printenv wc rm date; do
        crate "uu-$u" bin "uu_$u = \"=0.12.0\""
        printf 'fn main() { std::process::exit(uu_%s::uumain(std::env::args_os())); }\n' "$u" > "$WORK/uu-$u/src/main.rs"
        sed -i.bak 's/^lto = true$/lto = true\nopt-level = "z"\nstrip = true/' "$WORK/uu-$u/Cargo.toml"
        for a in aarch64 riscv64 x86_64; do
            spec="$ROOT/targets/$a-unknown-nife.json"
            if (cd "$WORK/uu-$u" && RUSTUP_TOOLCHAIN="$ROOT/target/nife-farm" cargo build -q --keep-going --release \
                -Zjson-target-spec -Zbuild-std=core,alloc,std,panic_abort \
                -Zbuild-std-features=compiler-builtins-mem --target "$spec" > "build-$a.log" 2>&1); then
                echo "uu_$u $a: builds, $(wc -c < "$WORK/uu-$u/target/$a-unknown-nife/release/uu-$u") bytes"
            else
                echo "uu_$u $a: fails in $(grep '^error: could not compile' "$WORK/uu-$u/build-$a.log" | sed 's/.*compile `\([^`]*\)`.*/\1/' | sort -u | tr '\n' ' ')"
            fi
        done
    done
    ;;
*) echo "probe.sh: unknown mode $MODE" >&2; exit 2 ;;
esac
