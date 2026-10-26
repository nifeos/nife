#!/bin/sh
# Probes behind dtoolkit-against-device-tree-blob.md, beside this script. Name provisional.
#
#   dtoolkit-probe.sh write          # write the scratch crates under $WORK (every mode does this first)
#   dtoolkit-probe.sh agree          # every .dtb in the tree, the hostile tests' own blobs, then probes
#   dtoolkit-probe.sh deep           # chain and flat trees by size: walk time, and Display's limit
#   dtoolkit-probe.sh bench          # the kernel's boot queries, per parser, on five fixtures
#   dtoolkit-probe.sh fit            # both parsers, no allocator, on the three nife targets; code size
#   dtoolkit-probe.sh fuzz [SECONDS] # three cargo-fuzz targets at once, fork mode
#   dtoolkit-probe.sh kani           # our four properties asked of dtoolkit, plus whole-parse ones
#   dtoolkit-probe.sh churn          # the adapter built against 0.2.0, 0.2.1, 0.3.0, 0.4.0 and main
#
# Every probe is generated OUTSIDE this repository, under $WORK, so this tree's Cargo.lock and
# rust-toolchain.toml stay untouched and dtoolkit never enters nife's graph. Versions are pinned
# with `=`. `ours` is a copy of crates/device_tree_blob with a `record` feature that writes every
# blob `from_bytes` is handed to $RECORD_DIR, so the hostile tests' own blobs become a corpus.
#
# BUGS: needs the network, Python 3 and, for `fuzz`, cargo-fuzz. Host figures only. `fuzz` and
# `kani` are heavy: run them one at a time on a shared machine. `fuzz` fetches dtoolkit's own fuzz
# target from GitHub at a pinned commit rather than carrying a copy.
set -eu
HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH='' cd -- "$HERE/../../.." && pwd)
WORK="${WORK:-${TMPDIR:-/tmp}/nife-784-dtoolkit-probe}"
MODE=${1:?usage: dtoolkit-probe.sh write|agree|deep|bench|fit|fuzz|kani|churn}
shift
PIN=$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$ROOT/rust-toolchain.toml")
DTK_PARSE_REV=edc53ad0f
DTK_MAIN_REV=4a7faf2e7
mkdir -p "$WORK"

write() {
    mkdir -p "$WORK/."
    cat > "$WORK/record.py" <<'SOURCE'
"""Give a copy of device_tree_blob a `record` feature: every blob from_bytes sees goes to $RECORD_DIR,
and index.txt names the test thread that handed it over."""
import sys

p = sys.argv[1]
s = open(p).read()
hook = """#![cfg_attr(not(any(test, feature = "record")), no_std)]

#[cfg(feature = "record")]
fn record(bytes: &[u8]) {
    use std::io::Write;
    if let Ok(dir) = std::env::var("RECORD_DIR") {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let _ = std::fs::write(format!("{dir}/{h:016x}.dtb"), bytes);
        let name = std::thread::current().name().unwrap_or("?").to_string();
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(format!("{dir}/index.txt"))
        {
            let _ = writeln!(f, "{h:016x} {name}");
        }
    }
}
"""
s = s.replace("#![cfg_attr(not(test), no_std)]", hook, 1)
start = "    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, Error> {\n"
assert start in s
s = s.replace(start, start + '        #[cfg(feature = "record")]\n        record(bytes);\n', 1)
open(p, "w").write(s)
SOURCE
    mkdir -p "$WORK/."
    cat > "$WORK/Cargo.toml" <<'SOURCE'
[workspace]
members = ["adapter", "agree", "ours"]
exclude = ["fuzz", "fit", "kani", "churn"]
resolver = "3"

[profile.release]
debug-assertions = true
overflow-checks = true
SOURCE
    mkdir -p "$WORK/adapter"
    cat > "$WORK/adapter/Cargo.toml" <<'SOURCE'
[package]
name = "dtk_adapter"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
dtoolkit = { version = "=0.4.1", default-features = false }
SOURCE
    mkdir -p "$WORK/adapter/src"
    cat > "$WORK/adapter/src/lib.rs" <<'SOURCE'
//! nife's device-tree queries, answered by dtoolkit 0.4.1's read-only API.
//!
//! Each public function here has the same contract as the `device_tree_blob` method of the same
//! name, so the two can be compared answer for answer. dtoolkit does the parsing (validation, the
//! node and property walk, `reg` decoding with the parent's cells); the policy stays nife's and is
//! written once here: which node counts as a match, a zero-size `reg` pair skipped, a wrapping
//! pair refused, the 16-deep refusal of the per-depth walkers, and a fixed output slice.
//!
//! `no_std`, no `alloc`: the tree walk keeps an explicit stack of child iterators, 32 deep, because
//! dtoolkit offers children and properties of one node but no whole-tree iterator.

#![no_std]

use dtoolkit::fdt::{Fdt, FdtNode};
use dtoolkit::standard::NodeStandard;
use dtoolkit::{Node, Property};

/// The same shape as `device_tree_blob::Region`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub start: u64,
    pub size: u64,
}

/// Why a query failed. Coarser than either crate's error: the comparison asks only whether both
/// refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Parse,
    Property,
    TooManyRegions,
    RegionOverflow,
    TooDeep,
}

/// nife's per-depth walkers stop matching at 16 (`device_tree_blob`'s `MAX_DEPTH`).
const MAX_DEPTH: usize = 16;
/// How deep this adapter's own walk can go before it gives up with `Error::TooDeep`.
const STACK: usize = 32;

type Children<'a> = <FdtNode<'a> as Node>::Children<'a>;

pub struct Dt<'a> {
    pub fdt: Fdt<'a>,
}

/// One step of a depth-first walk. Depth 1 is the root, as in `device_tree_blob`.
pub enum Event<'a> {
    Enter(FdtNode<'a>, usize),
    Exit(FdtNode<'a>, usize),
}

impl<'a> Dt<'a> {
    /// Parse `bytes`, trimmed to the header's `totalsize` as `device_tree_blob` trims it, since
    /// `Fdt::new` refuses any slack after the blob.
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, Error> {
        let total = bytes
            .get(4..8)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
            .ok_or(Error::Parse)?;
        let bytes = bytes.get(..total).ok_or(Error::Parse)?;
        Ok(Dt {
            fdt: Fdt::new(bytes).map_err(|_| Error::Parse)?,
        })
    }

    pub fn total_size(&self) -> usize {
        self.fdt.data().len()
    }

    /// Walk the whole tree depth first, calling `f` on entry to and exit from every node. Stops
    /// early, returning `Ok(Some(r))`, when `f` returns `Some(r)`.
    pub fn walk<R>(
        &self,
        mut f: impl FnMut(&Event<'a>) -> Option<Result<R, Error>>,
    ) -> Result<Option<R>, Error> {
        let root = self.fdt.root();
        let mut nodes: [Option<FdtNode<'a>>; STACK] = [None; STACK];
        let mut iters: [Option<Children<'a>>; STACK] = core::array::from_fn(|_| None);
        let mut depth = 1usize;
        nodes[1] = Some(root);
        iters[1] = Some(root.children());
        if let Some(r) = f(&Event::Enter(root, 1)) {
            return r.map(Some);
        }
        while depth >= 1 {
            let next = iters[depth].as_mut().and_then(Iterator::next);
            match next {
                Some(child) => {
                    if depth + 1 >= STACK {
                        return Err(Error::TooDeep);
                    }
                    depth += 1;
                    nodes[depth] = Some(child);
                    iters[depth] = Some(child.children());
                    if let Some(r) = f(&Event::Enter(child, depth)) {
                        return r.map(Some);
                    }
                }
                None => {
                    let node = nodes[depth].take().expect("open node");
                    iters[depth] = None;
                    if let Some(r) = f(&Event::Exit(node, depth)) {
                        return r.map(Some);
                    }
                    depth -= 1;
                }
            }
        }
        Ok(None)
    }

    /// `memory_regions`: every root child named `memory` or `memory@...`, its `reg` decoded.
    pub fn memory_regions(&self, out: &mut [Region]) -> Result<usize, Error> {
        let mut n = 0;
        for child in self.fdt.root().children() {
            let name = child.name();
            if name == "memory" || name.starts_with("memory@") {
                n += decode_reg(child, &mut out[n..])?;
            }
        }
        Ok(n)
    }

    /// `reserved_regions`: the memory reservation block.
    pub fn reserved_regions(&self, out: &mut [Region]) -> Result<usize, Error> {
        let mut n = 0;
        for r in self.fdt.memory_reservations() {
            *out.get_mut(n).ok_or(Error::TooManyRegions)? = Region {
                start: r.address(),
                size: r.size(),
            };
            n += 1;
        }
        Ok(n)
    }

    /// `reserved_memory_regions`: the `reg` of each child of `/reserved-memory`.
    pub fn reserved_memory_regions(&self, out: &mut [Region]) -> Result<usize, Error> {
        let mut n = 0;
        if let Some(children) = self.fdt.reserved_memory() {
            for child in children {
                let dest = out.get_mut(n..).ok_or(Error::TooManyRegions)?;
                n += decode_reg(*child, dest)?;
            }
        }
        Ok(n)
    }

    /// `initial_ramdisk`: `/chosen`'s `linux,initrd-start` and `linux,initrd-end`.
    pub fn initial_ramdisk(&self) -> Result<Option<Region>, Error> {
        let Some(chosen) = self.fdt.chosen() else {
            return Ok(None);
        };
        let int = |name| -> Result<Option<u64>, Error> {
            match chosen.property(name) {
                None => Ok(None),
                Some(p) => match p.value().len() {
                    4 => Ok(Some(
                        p.value_as::<u32>().map_err(|_| Error::Property)? as u64
                    )),
                    8 => Ok(Some(p.value_as::<u64>().map_err(|_| Error::Property)?)),
                    _ => Err(Error::Property),
                },
            }
        };
        let start = int("linux,initrd-start")?;
        let end = int("linux,initrd-end")?;
        Ok(match (start, end) {
            (Some(s), Some(e)) if e > s => Some(Region {
                start: s,
                size: e - s,
            }),
            _ => None,
        })
    }

    /// `node_reg`: the first node (depth 2 to 15, tree order) whose name starts with `prefix` and
    /// that has a `reg`. A match without `reg` is passed over, and nodes inside it are not matched.
    pub fn node_reg(&self, prefix: &[u8], out: &mut [Region]) -> Result<usize, Error> {
        let mut inside: Option<usize> = None;
        let r = self.walk(|ev| match *ev {
            Event::Enter(node, d) => {
                if inside.is_none()
                    && (2..MAX_DEPTH).contains(&d)
                    && node.name().as_bytes().starts_with(prefix)
                {
                    if node.property("reg").is_some() {
                        return Some(decode_reg(node, out));
                    }
                    inside = Some(d);
                }
                None
            }
            Event::Exit(_, d) => {
                if inside == Some(d) {
                    inside = None;
                }
                None
            }
        })?;
        Ok(r.unwrap_or(0))
    }

    /// The first node, in the order nodes close, at depth 2 to 15, for which `matches` holds.
    /// `device_tree_blob` decides at `END_NODE`, so a matching child wins over a matching parent.
    fn first_closing(
        &self,
        mut matches: impl FnMut(FdtNode<'a>) -> bool,
    ) -> Result<Option<(FdtNode<'a>, Option<FdtNode<'a>>)>, Error> {
        let mut parents: [Option<FdtNode<'a>>; STACK] = [None; STACK];
        self.walk(|ev| match *ev {
            Event::Enter(node, d) => {
                parents[d] = Some(node);
                None
            }
            Event::Exit(node, d) => {
                if (2..MAX_DEPTH).contains(&d) && matches(node) {
                    Some(Ok((node, parents[d - 1])))
                } else {
                    None
                }
            }
        })
    }

    /// `node_reg_compatible`.
    pub fn node_reg_compatible(&self, compat: &[u8], out: &mut [Region]) -> Result<usize, Error> {
        match self.first_closing(|n| is_compatible(n, compat))? {
            Some((node, _)) => decode_reg(node, out),
            None => Ok(0),
        }
    }

    /// `node_prop_compatible`.
    pub fn node_prop_compatible(
        &self,
        compat: &[u8],
        name: &[u8],
    ) -> Result<Option<&'a [u8]>, Error> {
        Ok(self
            .first_closing(|n| is_compatible(n, compat))?
            .and_then(|(node, _)| prop(node, name)))
    }

    /// `parent_prop_compatible`.
    pub fn parent_prop_compatible(
        &self,
        compat: &[u8],
        name: &[u8],
    ) -> Result<Option<&'a [u8]>, Error> {
        Ok(self
            .first_closing(|n| is_compatible(n, compat))?
            .and_then(|(_, parent)| parent.and_then(|p| prop(p, name))))
    }

    /// `phandle_prop`: `phandle` or `linux,phandle`, four bytes, equal to `phandle`.
    pub fn phandle_prop(&self, phandle: u32, name: &[u8]) -> Result<Option<&'a [u8]>, Error> {
        let is = |n: FdtNode<'a>| {
            n.properties().any(|p| {
                (p.name() == "phandle" || p.name() == "linux,phandle")
                    && p.value_as::<u32>() == Ok(phandle)
            })
        };
        Ok(self
            .first_closing(is)?
            .and_then(|(node, _)| prop(node, name)))
    }

    /// `node_prop`: property `name` of the first node (any depth from 2, tree order) whose name
    /// starts with `prefix`.
    pub fn node_prop(&self, prefix: &[u8], name: &[u8]) -> Result<Option<&'a [u8]>, Error> {
        let r = self.walk(|ev| match *ev {
            Event::Enter(node, d) if d >= 2 && node.name().as_bytes().starts_with(prefix) => {
                Some(Ok(prop(node, name)))
            }
            _ => None,
        })?;
        Ok(r.flatten())
    }

    /// `node_prop_inherited`: as `node_prop` (depth 2 to 15), falling back to the nearest ancestor.
    pub fn node_prop_inherited(
        &self,
        prefix: &[u8],
        name: &[u8],
    ) -> Result<Option<&'a [u8]>, Error> {
        let mut open: [Option<FdtNode<'a>>; STACK] = [None; STACK];
        let r = self.walk(|ev| match *ev {
            Event::Enter(node, d) => {
                open[d] = Some(node);
                if (2..MAX_DEPTH).contains(&d) && node.name().as_bytes().starts_with(prefix) {
                    let found = (1..=d)
                        .rev()
                        .find_map(|i| open[i].and_then(|n| prop(n, name)));
                    return Some(Ok(found));
                }
                None
            }
            Event::Exit(..) => None,
        })?;
        Ok(r.flatten())
    }

    /// `node_props`: property `name` on every node whose name starts with `prefix`, not counting a
    /// match nested inside another. Returns the number of matches.
    pub fn node_props(
        &self,
        prefix: &[u8],
        name: &[u8],
        out: &mut [Option<&'a [u8]>],
    ) -> Result<usize, Error> {
        for slot in out.iter_mut() {
            *slot = None;
        }
        let mut inside: Option<usize> = None;
        let mut matched = 0usize;
        self.walk(|ev| {
            match *ev {
                Event::Enter(node, d) => {
                    if inside.is_none() && d >= 2 && node.name().as_bytes().starts_with(prefix) {
                        inside = Some(d);
                        if let Some(slot) = out.get_mut(matched) {
                            *slot = prop(node, name);
                        }
                        matched += 1;
                    }
                }
                Event::Exit(_, d) => {
                    if inside == Some(d) {
                        inside = None;
                    }
                }
            }
            None::<Result<(), Error>>
        })?;
        Ok(matched)
    }
}

fn prop<'a>(node: FdtNode<'a>, name: &[u8]) -> Option<&'a [u8]> {
    node.properties()
        .find(|p| p.name().as_bytes() == name)
        .map(|p| p.value_as::<&'a [u8]>().unwrap_or_default())
}

fn is_compatible(node: FdtNode<'_>, compat: &[u8]) -> bool {
    match core::str::from_utf8(compat) {
        Ok(c) => node.is_compatible(c),
        Err(_) => false,
    }
}

/// A node's `reg`, decoded by dtoolkit with the parent's cells, under nife's policy.
fn decode_reg(node: FdtNode<'_>, out: &mut [Region]) -> Result<usize, Error> {
    let Some(regs) = node.reg().map_err(|_| Error::Property)? else {
        return Ok(0);
    };
    let mut n = 0;
    for reg in regs {
        let start: u64 = reg.address().map_err(|_| Error::Property)?;
        let size: u64 = reg.size().map_err(|_| Error::Property)?;
        if size == 0 {
            continue;
        }
        if start.checked_add(size).is_none() {
            return Err(Error::RegionOverflow);
        }
        *out.get_mut(n).ok_or(Error::TooManyRegions)? = Region { start, size };
        n += 1;
    }
    Ok(n)
}
SOURCE
    mkdir -p "$WORK/agree"
    cat > "$WORK/agree/Cargo.toml" <<'SOURCE'
[package]
name = "agree"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
dtk_adapter = { path = "../adapter" }
device_tree_blob = { path = "../ours" }
dtoolkit = { version = "=0.4.1", default-features = false }
SOURCE
    mkdir -p "$WORK/agree/src"
    cat > "$WORK/agree/src/main.rs" <<'SOURCE'
//! Every blob named on the command line through both parsers, for every query nife makes, over
//! every needle the tree itself offers. Prints each disagreement and a summary per blob.
//!
//!     agree [--quiet] FILE...

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use device_tree_blob::{DeviceTreeBlob, Region as OurRegion};
use dtk_adapter::{Dt, Event, Region};
use dtoolkit::{Node, Property};

/// Needles nife's callers pass literally (grepped from kernel/, crates/ and uefi_loader/), added to
/// the ones each tree offers.
const PREFIXES: &[&str] = &[
    "",
    "intc",
    "intc@",
    "plic",
    "plic@",
    "memory",
    "virtio_mmio",
    "cpu@",
    "cpus",
    "psci",
    "chosen",
    "interrupt-controller",
    "smmuv3@",
    "aon_syscon@",
    "serial",
    "uart",
    "pl011",
    "pl011@9000000",
    "ethernet-phy@0",
    "other",
];
const COMPATS: &[&str] = &[
    "",
    "arm,pl031",
    "google,goldfish-rtc",
    "riscv,plic0",
    "virtio,mmio",
    "pci-host-ecam-generic",
    "qemu,fw-cfg-mmio",
    "arm,armv8-timer",
    "arm,gic-v3",
    "arm,cortex-a15-gic",
    "arm,gic-400",
    "sifive,plic-1.0.0",
    "thead,c900-plic",
    "snps,dw-apb-uart",
    "snps,dw-mshc",
    "starfive,jh7110-trng",
    "starfive,jh7110-wdt",
    "starfive,jh7110-dwmac",
    "starfive,jh7110-aon-syscon",
    "x-powers,axp15060",
    "starfive,jh7110-syscrg",
];
const NAMES: &[&str] = &[
    "reg",
    "compatible",
    "interrupts",
    "interrupt-parent",
    "#interrupt-cells",
    "status",
    "ranges",
    "clocks",
    "resets",
    "reg-names",
    "phandle",
    "riscv,isa",
    "riscv,isa-extensions",
    "mmu-type",
    "enable-method",
    "timebase-frequency",
    "#address-cells",
    "clock-frequency",
    "method",
    "cpu_on",
    "boot-hartid",
    "stdout-path",
    "not-a-property",
];

#[derive(Default)]
struct Tally {
    queries: usize,
    disagreements: BTreeMap<String, Vec<String>>,
    panics: Vec<String>,
}

type Ans = Result<String, String>;

fn regions_ours(r: Result<usize, device_tree_blob::Error>, out: &[OurRegion]) -> Ans {
    match r {
        Ok(n) => Ok(format!(
            "{:x?}",
            out[..n]
                .iter()
                .map(|r| (r.start, r.size))
                .collect::<Vec<_>>()
        )),
        Err(e) => Err(format!("{e:?}")),
    }
}
fn regions_theirs(r: Result<usize, dtk_adapter::Error>, out: &[Region]) -> Ans {
    match r {
        Ok(n) => Ok(format!(
            "{:x?}",
            out[..n]
                .iter()
                .map(|r| (r.start, r.size))
                .collect::<Vec<_>>()
        )),
        Err(e) => Err(format!("{e:?}")),
    }
}
fn bytes<E: std::fmt::Debug>(r: Result<Option<&[u8]>, E>) -> Ans {
    r.map(|o| format!("{o:x?}")).map_err(|e| format!("{e:?}"))
}

fn guard<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|p| {
        p.downcast_ref::<String>()
            .cloned()
            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default()
    })
}

impl Tally {
    fn cmp(
        &mut self,
        family: &str,
        query: String,
        ours: impl FnOnce() -> Ans,
        theirs: impl FnOnce() -> Ans,
    ) {
        self.queries += 1;
        let o = guard(ours);
        let t = guard(theirs);
        match (&o, &t) {
            (Err(p), _) => self
                .panics
                .push(format!("ours panicked: {family} {query}: {p}")),
            (_, Err(p)) => self
                .panics
                .push(format!("dtoolkit panicked: {family} {query}: {p}")),
            (Ok(a), Ok(b)) => {
                // Both refusing counts as agreement, whatever each called the refusal.
                let same = match (a, b) {
                    (Ok(x), Ok(y)) => x == y,
                    (Err(_), Err(_)) => true,
                    _ => false,
                };
                if !same {
                    self.disagreements
                        .entry(family.to_string())
                        .or_default()
                        .push(format!("{query}: ours {a:?}, dtoolkit {b:?}"));
                }
            }
        }
    }
}

fn run(path: &str, data: &[u8], tally: &mut Tally) -> String {
    let ours = guard(|| DeviceTreeBlob::from_bytes(data));
    let theirs = guard(|| Dt::from_bytes(data));
    let (ours, theirs) = match (ours, theirs) {
        (Err(p), _) => {
            tally
                .panics
                .push(format!("ours panicked in from_bytes: {p}"));
            return "ours panicked".into();
        }
        (_, Err(p)) => {
            tally
                .panics
                .push(format!("dtoolkit panicked in Fdt::new: {p}"));
            return "dtoolkit panicked".into();
        }
        (Ok(o), Ok(t)) => (o, t),
    };
    let (ours, theirs) = match (ours, theirs) {
        (Ok(o), Ok(t)) => (o, t),
        (Err(e), Err(f)) => return format!("both refuse: ours {e:?}, dtoolkit {f:?}"),
        (Ok(_), Err(f)) => {
            let detail = dtoolkit::fdt::Fdt::new(
                &data[..data
                    .len()
                    .min(u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize)],
            )
            .err()
            .map(|e| format!("{e:?}"))
            .unwrap_or_else(|| format!("{f:?}"));
            tally
                .disagreements
                .entry("from_bytes".into())
                .or_default()
                .push(format!("{path}: only ours accepts; dtoolkit says {detail}"));
            return "ONLY OURS ACCEPTS".into();
        }
        (Err(e), Ok(_)) => {
            tally
                .disagreements
                .entry("from_bytes".into())
                .or_default()
                .push(format!("{path}: only dtoolkit accepts; ours says {e:?}"));
            return "ONLY DTOOLKIT ACCEPTS".into();
        }
    };

    // The needles: every node name, name stem, compatible string, property name and phandle the
    // tree holds, through dtoolkit's walk, plus the callers' literals.
    let mut prefixes: BTreeSet<String> = PREFIXES.iter().map(|s| s.to_string()).collect();
    let mut compats: BTreeSet<String> = COMPATS.iter().map(|s| s.to_string()).collect();
    let mut names: BTreeSet<String> = NAMES.iter().map(|s| s.to_string()).collect();
    let mut phandles: BTreeSet<u32> = [0, 1, 0xffff_ffff].into_iter().collect();
    let _ = guard(|| {
        theirs.walk(|ev| {
            if let Event::Enter(n, _) = ev {
                let name = n.name();
                prefixes.insert(name.to_string());
                let stem = name.split('@').next().unwrap_or("");
                prefixes.insert(stem.to_string());
                prefixes.insert(format!("{stem}@"));
                for p in n.properties() {
                    names.insert(p.name().to_string());
                    if p.name() == "compatible" {
                        for c in p.value().split(|&b| b == 0) {
                            if let Ok(s) = std::str::from_utf8(c) {
                                compats.insert(s.to_string());
                            }
                        }
                    }
                    if p.name() == "phandle" || p.name() == "linux,phandle" {
                        if let Ok(v) = p.value_as::<u32>() {
                            phandles.insert(v);
                        }
                    }
                }
            }
            None::<Result<(), dtk_adapter::Error>>
        })
    });

    let before = tally.queries;
    let z = OurRegion { start: 0, size: 0 };
    let zt = Region { start: 0, size: 0 };
    for cap in [0usize, 1, 2, 8, 64] {
        let tag = format!("out[{cap}]");
        tally.cmp(
            "memory_regions",
            tag.clone(),
            || {
                let mut o = vec![z; cap];
                let r = ours.memory_regions(&mut o);
                regions_ours(r, &o)
            },
            || {
                let mut o = vec![zt; cap];
                let r = theirs.memory_regions(&mut o);
                regions_theirs(r, &o)
            },
        );
        tally.cmp(
            "reserved_regions",
            tag.clone(),
            || {
                let mut o = vec![z; cap];
                let r = ours.reserved_regions(&mut o);
                regions_ours(r, &o)
            },
            || {
                let mut o = vec![zt; cap];
                let r = theirs.reserved_regions(&mut o);
                regions_theirs(r, &o)
            },
        );
        tally.cmp(
            "reserved_memory_regions",
            tag.clone(),
            || {
                let mut o = vec![z; cap];
                let r = ours.reserved_memory_regions(&mut o);
                regions_ours(r, &o)
            },
            || {
                let mut o = vec![zt; cap];
                let r = theirs.reserved_memory_regions(&mut o);
                regions_theirs(r, &o)
            },
        );
    }
    tally.cmp(
        "total_size",
        String::new(),
        || Ok(ours.total_size().to_string()),
        || Ok(theirs.total_size().to_string()),
    );
    tally.cmp(
        "initial_ramdisk",
        String::new(),
        || {
            ours.initial_ramdisk()
                .map(|r| format!("{:x?}", r.map(|r| (r.start, r.size))))
                .map_err(|e| format!("{e:?}"))
        },
        || {
            theirs
                .initial_ramdisk()
                .map(|r| format!("{:x?}", r.map(|r| (r.start, r.size))))
                .map_err(|e| format!("{e:?}"))
        },
    );

    for p in &prefixes {
        let pb = p.as_bytes();
        tally.cmp(
            "node_reg",
            format!("{p:?}"),
            || {
                let mut o = [z; 16];
                let r = ours.node_reg(pb, &mut o);
                regions_ours(r, &o)
            },
            || {
                let mut o = [zt; 16];
                let r = theirs.node_reg(pb, &mut o);
                regions_theirs(r, &o)
            },
        );
        for n in &names {
            let nb = n.as_bytes();
            tally.cmp(
                "node_prop",
                format!("{p:?} {n:?}"),
                || bytes(ours.node_prop(pb, nb)),
                || bytes(theirs.node_prop(pb, nb)),
            );
            tally.cmp(
                "node_prop_inherited",
                format!("{p:?} {n:?}"),
                || bytes(ours.node_prop_inherited(pb, nb)),
                || bytes(theirs.node_prop_inherited(pb, nb)),
            );
            tally.cmp(
                "node_props",
                format!("{p:?} {n:?}"),
                || {
                    let mut o = [None; 8];
                    ours.node_props(pb, nb, &mut o)
                        .map(|k| format!("{k} {o:x?}"))
                        .map_err(|e| format!("{e:?}"))
                },
                || {
                    let mut o = [None; 8];
                    theirs
                        .node_props(pb, nb, &mut o)
                        .map(|k| format!("{k} {o:x?}"))
                        .map_err(|e| format!("{e:?}"))
                },
            );
        }
    }
    for c in &compats {
        let cb = c.as_bytes();
        tally.cmp(
            "node_reg_compatible",
            format!("{c:?}"),
            || {
                let mut o = [z; 16];
                let r = ours.node_reg_compatible(cb, &mut o);
                regions_ours(r, &o)
            },
            || {
                let mut o = [zt; 16];
                let r = theirs.node_reg_compatible(cb, &mut o);
                regions_theirs(r, &o)
            },
        );
        for n in &names {
            let nb = n.as_bytes();
            tally.cmp(
                "node_prop_compatible",
                format!("{c:?} {n:?}"),
                || bytes(ours.node_prop_compatible(cb, nb)),
                || bytes(theirs.node_prop_compatible(cb, nb)),
            );
            tally.cmp(
                "parent_prop_compatible",
                format!("{c:?} {n:?}"),
                || bytes(ours.parent_prop_compatible(cb, nb)),
                || bytes(theirs.parent_prop_compatible(cb, nb)),
            );
        }
    }
    for ph in &phandles {
        for n in &names {
            let nb = n.as_bytes();
            tally.cmp(
                "phandle_prop",
                format!("{ph:#x} {n:?}"),
                || bytes(ours.phandle_prop(*ph, nb)),
                || bytes(theirs.phandle_prop(*ph, nb)),
            );
        }
    }
    format!("{} queries", tally.queries - before)
}

fn main() {
    let mut quiet = false;
    let mut files = Vec::new();
    for a in std::env::args().skip(1) {
        if a == "--quiet" {
            quiet = true
        } else {
            files.push(a)
        }
    }
    std::panic::set_hook(Box::new(|_| {}));
    let mut grand = Tally::default();
    let mut total_q = 0;
    for f in &files {
        let data = std::fs::read(f).expect("read");
        let mut t = Tally::default();
        let start = Instant::now();
        let summary = run(f, &data, &mut t);
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let nd: usize = t.disagreements.values().map(Vec::len).sum();
        println!(
            "{f}: {summary}, {nd} disagreements, {} panics, {ms:.0} ms",
            t.panics.len()
        );
        total_q += t.queries;
        for (fam, v) in t.disagreements {
            let e = grand.disagreements.entry(fam.clone()).or_default();
            for d in v {
                if !quiet {
                    println!("    {fam}: {d}");
                }
                e.push(format!("{f}: {d}"));
            }
        }
        for p in t.panics {
            println!("    PANIC {p}");
            grand.panics.push(format!("{f}: {p}"));
        }
    }
    println!("== {} files, {total_q} queries", files.len());
    for (fam, v) in &grand.disagreements {
        println!("== {fam}: {} disagreements", v.len());
    }
    println!("== panics: {}", grand.panics.len());
}
SOURCE
    mkdir -p "$WORK/agree/src/bin"
    cat > "$WORK/agree/src/bin/deep.rs" <<'SOURCE'
//! Hostile shapes by size: a chain `depth` nodes deep, and a flat tree `width` nodes wide, each
//! timed through Fdt::new, a whole-tree walk on dtoolkit's API (heap stack, so the walk itself has
//! no depth limit), ours (`node_prop` for a name nothing has, which walks every token), and
//! dtoolkit's `Display`, run on a thread with an 8 MiB stack.
//!
//!     deep chain|flat N [display]

use dtoolkit::Node;
use dtoolkit::fdt::Fdt;
use std::time::Instant;

fn blob(chain: bool, n: usize) -> Vec<u8> {
    let mut s = Vec::new();
    let tok = |s: &mut Vec<u8>, t: u32| s.extend_from_slice(&t.to_be_bytes());
    tok(&mut s, 1);
    s.extend_from_slice(&[0, 0, 0, 0]);
    for _ in 0..n {
        tok(&mut s, 1);
        s.extend_from_slice(b"a\0\0\0");
        if !chain {
            tok(&mut s, 2);
        }
    }
    if chain {
        for _ in 0..n {
            tok(&mut s, 2);
        }
    }
    tok(&mut s, 2);
    tok(&mut s, 9);
    let off_struct = 40 + 16;
    let off_strings = off_struct + s.len();
    let total = off_strings + 4;
    let mut b = Vec::new();
    for v in [
        0xd00d_feedu32,
        total as u32,
        off_struct as u32,
        off_strings as u32,
        40,
        17,
        16,
        0,
        4,
        s.len() as u32,
    ] {
        b.extend_from_slice(&v.to_be_bytes());
    }
    b.extend_from_slice(&[0; 16]);
    b.extend_from_slice(&s);
    b.extend_from_slice(b"reg\0");
    b
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let chain = args[1] == "chain";
    let n: usize = args[2].parse().unwrap();
    let display = args.get(3).is_some_and(|a| a == "display");
    let b = blob(chain, n);
    let t = Instant::now();
    let fdt = Fdt::new(&b).expect("valid");
    let parse = t.elapsed();
    let t = Instant::now();
    let mut stack = vec![fdt.root().children()];
    let mut nodes = 0usize;
    while let Some(top) = stack.last_mut() {
        match top.next() {
            Some(c) => {
                nodes += 1;
                stack.push(c.children());
            }
            None => {
                stack.pop();
            }
        }
    }
    let walk = t.elapsed();
    let t = Instant::now();
    let ours = device_tree_blob::DeviceTreeBlob::from_bytes(&b).unwrap();
    let r = ours.node_prop(b"zzz", b"reg");
    let ours_t = t.elapsed();
    println!(
        "{} {n}: {} bytes, Fdt::new {:?}, dtoolkit walk {:?} ({nodes} nodes), ours full walk {:?} ({r:?})",
        if chain { "chain" } else { "flat" },
        b.len(),
        parse,
        walk,
        ours_t
    );
    if display {
        let b2 = b.clone();
        let h = std::thread::Builder::new()
            .stack_size(8 << 20)
            .spawn(move || {
                let fdt = Fdt::new(&b2).unwrap();
                format!("{fdt}").len()
            })
            .unwrap();
        println!("display: {:?}", h.join().map_err(|_| "panicked"));
    }
}
SOURCE
    mkdir -p "$WORK/agree/src/bin"
    cat > "$WORK/agree/src/bin/bench.rs" <<'SOURCE'
//! The kernel's boot queries (the set `fit` links) on each blob named, per parser: best of 7 rounds
//! of 200, in microseconds per boot's worth.
use std::time::Instant;

macro_rules! boot {
    ($D:ty, $R:expr, $b:expr) => {{
        let dt = <$D>::from_bytes($b).ok().unwrap();
        let mut out = [$R; 8];
        let mut sum = dt.total_size() as u64;
        if let Ok(k) = dt.memory_regions(&mut out) {
            sum += k as u64
        }
        if let Ok(k) = dt.reserved_regions(&mut out) {
            sum += k as u64
        }
        if let Ok(k) = dt.reserved_memory_regions(&mut out) {
            sum += k as u64
        }
        if let Ok(Some(_)) = dt.initial_ramdisk() {
            sum += 1
        }
        if let Ok(k) = dt.node_reg(b"plic@", &mut out) {
            sum += k as u64
        }
        if let Ok(k) = dt.node_reg_compatible(b"pci-host-ecam-generic", &mut out) {
            sum += k as u64
        }
        if let Ok(Some(v)) = dt.node_prop_compatible(b"pci-host-ecam-generic", b"ranges") {
            sum += v.len() as u64
        }
        if let Ok(Some(v)) = dt.node_prop(b"pl011", b"compatible") {
            sum += v.len() as u64
        }
        if let Ok(Some(v)) = dt.node_prop_inherited(b"pl011", b"interrupt-parent") {
            sum += v.len() as u64
        }
        if let Ok(Some(v)) = dt.phandle_prop(1, b"#interrupt-cells") {
            sum += v.len() as u64
        }
        let mut isa = [None; 8];
        if let Ok(k) = dt.node_props(b"cpu@", b"riscv,isa", &mut isa) {
            sum += k as u64
        }
        sum
    }};
}

fn best(f: &dyn Fn() -> u64) -> f64 {
    let mut b = f64::MAX;
    for _ in 0..7 {
        let t = Instant::now();
        for _ in 0..200 {
            std::hint::black_box(f());
        }
        b = b.min(t.elapsed().as_secs_f64() / 200.0 * 1e6);
    }
    b
}

fn main() {
    for f in std::env::args().skip(1) {
        let b = std::fs::read(&f).unwrap();
        let ours = best(&|| {
            boot!(
                device_tree_blob::DeviceTreeBlob,
                device_tree_blob::Region { start: 0, size: 0 },
                std::hint::black_box(&b[..])
            )
        });
        let dtk = best(&|| {
            boot!(
                dtk_adapter::Dt,
                dtk_adapter::Region { start: 0, size: 0 },
                std::hint::black_box(&b[..])
            )
        });
        println!(
            "{}: {} bytes, ours {ours:.1} us, dtoolkit {dtk:.1} us",
            f.rsplit('/').next().unwrap(),
            b.len()
        );
    }
}
SOURCE
    mkdir -p "$WORK/probes"
    cat > "$WORK/probes/make_probes.py" <<'SOURCE'
"""Variants of a real device tree that dtoolkit's validation and ours might treat differently.

    python3 -I make_probes.py SOURCE.dtb OUTDIR

Each variant changes one thing: one property's name is repointed at a new string appended to the
strings block, or bytes are appended after the blob.
"""
import struct, sys

src, out = sys.argv[1], sys.argv[2]
b = bytearray(open(src, 'rb').read())
hdr = list(struct.unpack('>10I', b[:40]))
magic, total, off_struct, off_strings, off_rsv, ver, lcv, cpu, sz_strings, sz_struct = hdr
assert off_strings + sz_strings == total, "strings block must end the blob"

def strings_index(name):
    blk = bytes(b[off_strings:off_strings + sz_strings])
    return blk.index(name.encode() + b'\0')

def first_prop_with(nameoff):
    at = off_struct
    while True:
        tok = struct.unpack('>I', b[at:at + 4])[0]
        at += 4
        if tok == 1:
            end = b.index(0, at)
            at = (end + 1 + 3) & ~3
        elif tok == 3:
            ln, no = struct.unpack('>II', b[at:at + 8])
            if no == nameoff:
                return at + 4
            at = (at + 8 + ln + 3) & ~3
        elif tok in (2, 4):
            pass
        else:
            raise SystemExit('no such property')

def renamed(old, new):
    c = bytearray(b)
    where = first_prop_with(strings_index(old))
    newoff = sz_strings
    c += new + b'\0'
    struct.pack_into('>I', c, where, newoff)
    struct.pack_into('>I', c, 4, len(c))
    struct.pack_into('>I', c, 32, sz_strings + len(new) + 1)
    return c

variants = {
    'prop-name-31': renamed('status', b'snps,dis-u2-freeclk-exist-quirk'),
    'prop-name-32': renamed('status', b'snps,dis-u2-freeclk-exists-quirk'),
    'prop-name-33': renamed('status', b'snps,dis-tx-ipgap-linecheck-quirk'),
    'prop-name-non-utf8': renamed('status', b'stat\xffus'),
    'prop-name-space': renamed('status', b'st atus'),
    'slack-16': bytes(b) + bytes(16),
}
for k, v in variants.items():
    open(f'{out}/{k}.dtb', 'wb').write(v)
    print(k, len(v))
SOURCE
    mkdir -p "$WORK/fuzz"
    cat > "$WORK/fuzz/Cargo.toml" <<'SOURCE'
[package]
name = "dtk-measure-fuzz"
version = "0.0.0"
publish = false
edition = "2024"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
device_tree_blob = { path = "../ours" }
dtk_adapter = { path = "../adapter" }
dtoolkit = { version = "=0.4.1", default-features = false }

[workspace]

[profile.release]
debug = 1
debug-assertions = true
overflow-checks = true

[[bin]]
name = "ours"
path = "fuzz_targets/ours.rs"
test = false
doc = false
bench = false

[[bin]]
name = "dtoolkit_nife"
path = "fuzz_targets/dtoolkit_nife.rs"
test = false
doc = false
bench = false

[[bin]]
name = "dtoolkit_parse"
path = "fuzz_targets/dtoolkit_parse.rs"
test = false
doc = false
bench = false
SOURCE
    mkdir -p "$WORK/fuzz/fuzz_targets"
    cat > "$WORK/fuzz/fuzz_targets/dtoolkit_nife.rs" <<'SOURCE'
//! nife's fuzz target `device_tree_blob_walk`, call for call, with dtoolkit 0.4.1 under the adapter
//! (`dtk_adapter`) in place of `device_tree_blob`. Three calls the original lacks are added at the end
//! so the adapter's every walk is reached.

#![no_main]

use dtk_adapter::{Dt as DeviceTreeBlob, Region};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(dtb) = DeviceTreeBlob::from_bytes(data) else {
        return;
    };

    // Small on purpose. A caller in the kernel passes a fixed array off its own stack, so the
    // interesting paths are the ones where the output runs out and the parser has to say
    // `TooManyRegions` instead of writing past the end.
    let mut out = [Region { start: 0, size: 0 }; 8];

    let _ = dtb.total_size();
    let _ = dtb.reserved_regions(&mut out);
    let _ = dtb.memory_regions(&mut out);
    let _ = dtb.reserved_memory_regions(&mut out);
    let _ = dtb.initial_ramdisk();

    // The three lookups that take a caller-supplied needle, with the arch layer's real needles
    // (`intc`/`plic`, `pl031`/`goldfish-rtc`, `virtio_mmio`). The empty prefix reaches the decode
    // path cheapest, since it matches the first node at depth >= 2, and for that reason it cannot
    // reach the deep-nesting bug: matching stops at the first candidate (notes/fuzzing.md's BUGS).
    for prefix in [&b""[..], b"intc", b"plic", b"memory", b"virtio_mmio"] {
        let _ = dtb.node_reg(prefix, &mut out);
        let _ = dtb.node_prop(prefix, b"reg");
        let _ = dtb.node_prop(prefix, b"compatible");
        // The inherited variant keeps a per-depth value stack the blob's nesting indexes, which is
        // the same shape as the `node_reg` cell stacks this target already caught a real
        // out-of-bounds in. `interrupts` beside `interrupt-parent` because that pair is the
        // kernel's real read (the console UART's line).
        let _ = dtb.node_prop_inherited(prefix, b"interrupt-parent");
        let _ = dtb.node_prop(prefix, b"interrupts");
    }
    // The phandle walk: whatever `interrupt-parent` bytes the blob offers, followed, plus a
    // constant needle so the walk runs even when the property is absent.
    if let Ok(Some(parent)) = dtb.node_prop_inherited(b"", b"interrupt-parent")
        && parent.len() >= 4
    {
        let ph = u32::from_be_bytes([parent[0], parent[1], parent[2], parent[3]]);
        let _ = dtb.phandle_prop(ph, b"#interrupt-cells");
    }
    let _ = dtb.phandle_prop(1, b"#interrupt-cells");
    for compat in [
        &b""[..],
        b"arm,pl031",
        b"google,goldfish-rtc",
        b"riscv,plic0",
    ] {
        let _ = dtb.node_reg_compatible(compat, &mut out);
    }

    // `Region::end()` is a `pub fn` on a `pub struct` whose fields came out of the blob, so it has
    // to hold on its own. Called on whatever the walkers actually produced rather than on invented
    // numbers, so a region the parser is willing to return is a region this is willing to add up.
    if let Ok(n) = dtb.memory_regions(&mut out) {
        for r in &out[..n] {
            let _ = r.start.saturating_add(r.size);
        }
    }
    let _ = dtb.node_props(b"cpu@", b"riscv,isa", &mut [None; 8]);
    let _ = dtb.parent_prop_compatible(b"riscv,plic0", b"reg");
    let _ = dtb.node_prop_compatible(b"riscv,plic0", b"interrupts-extended");
});
SOURCE
    mkdir -p "$WORK/fit"
    cat > "$WORK/fit/Cargo.toml" <<'SOURCE'
[package]
name = "fit"
version = "0.0.0"
edition = "2024"
publish = false

[features]
ours = ["dep:device_tree_blob"]
dtk = ["dep:dtk_adapter"]

[dependencies]
device_tree_blob = { path = "../ours", optional = true }
dtk_adapter = { path = "../adapter", optional = true }

[workspace]

[profile.release]
panic = "abort"
opt-level = 3
SOURCE
    mkdir -p "$WORK/fit/src"
    cat > "$WORK/fit/src/main.rs" <<'SOURCE'
//! The kernel's device-tree calls in a freestanding binary with no allocator, so the link answers
//! "does this need one?" and `llvm-size` answers "what does it cost?".
#![no_std]
#![no_main]

#[cfg(feature = "ours")]
use device_tree_blob::{DeviceTreeBlob as D, Region};
#[cfg(feature = "dtk")]
use dtk_adapter::{Dt as D, Region};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(p: *const u8, n: usize) -> u64 {
    // SAFETY: a probe, never run; the volatile read keeps the optimizer from folding the input.
    let b = unsafe { core::slice::from_raw_parts(core::ptr::read_volatile(&p), n) };
    let Ok(dt) = D::from_bytes(b) else { return 0 };
    let mut out = [Region { start: 0, size: 0 }; 8];
    let mut sum = dt.total_size() as u64;
    let mut add = |r: &[Region]| {
        for x in r {
            sum ^= x.start ^ x.size
        }
    };
    if let Ok(k) = dt.memory_regions(&mut out) {
        add(&out[..k])
    }
    if let Ok(k) = dt.reserved_regions(&mut out) {
        add(&out[..k])
    }
    if let Ok(k) = dt.reserved_memory_regions(&mut out) {
        add(&out[..k])
    }
    if let Ok(Some(r)) = dt.initial_ramdisk() {
        add(&[r])
    }
    if let Ok(k) = dt.node_reg(b"plic@", &mut out) {
        add(&out[..k])
    }
    if let Ok(k) = dt.node_reg_compatible(b"pci-host-ecam-generic", &mut out) {
        add(&out[..k])
    }
    let mut len = |r: Option<&[u8]>| sum = sum.wrapping_add(r.map_or(0, |v| v.len() as u64));
    if let Ok(v) = dt.node_prop_compatible(b"pci-host-ecam-generic", b"ranges") {
        len(v)
    }
    if let Ok(v) = dt.parent_prop_compatible(b"x-powers,axp15060", b"clocks") {
        len(v)
    }
    if let Ok(v) = dt.node_prop(b"serial", b"reg-shift") {
        len(v)
    }
    if let Ok(v) = dt.node_prop_inherited(b"serial", b"interrupt-parent") {
        len(v)
    }
    if let Ok(v) = dt.phandle_prop(1, b"#interrupt-cells") {
        len(v)
    }
    let mut isa = [None; 8];
    if let Ok(k) = dt.node_props(b"cpu@", b"riscv,isa", &mut isa) {
        sum ^= k as u64
    }
    sum
}
SOURCE
    mkdir -p "$WORK/kani"
    cat > "$WORK/kani/Cargo.toml" <<'SOURCE'
[package]
name = "kani_dtk"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
dtoolkit = { version = "=0.4.1", default-features = false }
device_tree_blob = { path = "../ours" }

[workspace]
SOURCE
    mkdir -p "$WORK/kani/src"
    cat > "$WORK/kani/src/lib.rs" <<'SOURCE'
//! Our four device_tree_blob properties, asked of dtoolkit 0.4.1's nearest public functions, plus
//! two whole-parse properties our crate's walkers are too big to have proved.
#![cfg_attr(not(kani), allow(dead_code))]

/// A valid tree: root with #address-cells = #size-cells = 2, one child `d@0` whose 16-byte `reg`
/// value starts at REG.
const TEMPLATE: [u8; 175] = [
    0xd0, 0x0d, 0xfe, 0xed, 0x00, 0x00, 0x00, 0xaf, 0x00, 0x00, 0x00, 0x38, 0x00, 0x00, 0x00, 0x90,
    0x00, 0x00, 0x00, 0x28, 0x00, 0x00, 0x00, 0x11, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x58, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
    0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x02,
    0x00, 0x00, 0x00, 0x01, 0x64, 0x40, 0x30, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x10,
    0x00, 0x00, 0x00, 0x1b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x09,
    0x23, 0x61, 0x64, 0x64, 0x72, 0x65, 0x73, 0x73, 0x2d, 0x63, 0x65, 0x6c, 0x6c, 0x73, 0x00, 0x23,
    0x73, 0x69, 0x7a, 0x65, 0x2d, 0x63, 0x65, 0x6c, 0x6c, 0x73, 0x00, 0x72, 0x65, 0x67, 0x00,
];
const REG: usize = 116;

#[cfg(kani)]
mod proofs {
    use super::*;
    use dtoolkit::fdt::Fdt;
    use dtoolkit::{FromPropertyValue, Node, Property};

    fn be(b: &[u8]) -> u64 {
        b.iter().fold(0u64, |v, &x| (v << 8) | x as u64)
    }

    /// P1/P2 analogue: the public 4- and 8-byte readers are total on any slice.
    #[kani::proof]
    fn be_readers_are_total() {
        let b: [u8; 12] = kani::any();
        let n: usize = kani::any();
        kani::assume(n <= 12);
        let _ = u32::from_property_value(&b[..n]);
        let _ = u64::from_property_value(&b[..n]);
    }

    /// P3 analogue: an Ok u32 read is the four bytes, most significant first.
    #[kani::proof]
    fn u32_reads_big_endian() {
        let b: [u8; 4] = kani::any();
        assert_eq!(u32::from_property_value(&b).unwrap() as u64, be(&b));
    }

    /// P4 analogue: dtoolkit pads with core's `usize::next_multiple_of(4)`.
    #[kani::proof]
    fn next_multiple_of_four_rounds_up() {
        let n: usize = kani::any();
        kani::assume(n <= 1 << 60);
        let a = n.next_multiple_of(4);
        assert!(a % 4 == 0 && a >= n && a < n + 4);
    }

    /// Beyond our four: a `reg` with any 16 bytes decodes to exactly those bytes, big-endian.
    #[kani::proof]
    #[kani::unwind(48)]
    fn dtoolkit_reg_decodes_any_value() {
        let mut t = TEMPLATE;
        let v: [u8; 16] = kani::any();
        t[REG..REG + 16].copy_from_slice(&v);
        let fdt = Fdt::new(&t).unwrap();
        let d = fdt.root().children().next().unwrap();
        let r = d.reg().unwrap().unwrap().next().unwrap();
        assert_eq!(r.address::<u64>().unwrap(), be(&v[..8]));
        assert_eq!(r.size::<u64>().unwrap(), be(&v[8..]));
    }

    /// The same question of ours, through `node_reg` and its policy.
    #[kani::proof]
    #[kani::unwind(48)]
    fn ours_reg_decodes_any_value() {
        let mut t = TEMPLATE;
        let v: [u8; 16] = kani::any();
        t[REG..REG + 16].copy_from_slice(&v);
        let dt = device_tree_blob::DeviceTreeBlob::from_bytes(&t).unwrap();
        let mut out = [device_tree_blob::Region { start: 0, size: 0 }; 2];
        let (s, z) = (be(&v[..8]), be(&v[8..]));
        match dt.node_reg(b"d", &mut out) {
            Ok(1) => assert!(out[0].start == s && out[0].size == z),
            Ok(0) => assert!(z == 0),
            Err(device_tree_blob::Error::RegionOverflow) => assert!(s.checked_add(z).is_none()),
            _ => panic!(),
        }
    }

    /// Whole parse: `Fdt::new` never panics on any N bytes.
    #[kani::proof]
    #[kani::unwind(20)]
    fn fdt_new_is_total_64() {
        let b: [u8; 64] = kani::any();
        let _ = Fdt::new(&b);
    }

    /// Whole parse, then the accessors that `expect` validity: none of them panics on a tree
    /// `Fdt::new` accepted.
    #[kani::proof]
    #[kani::unwind(20)]
    fn accepted_tree_never_panics_64() {
        let b: [u8; 64] = kani::any();
        if let Ok(fdt) = Fdt::new(&b) {
            let root = fdt.root();
            let _ = root.name();
            for p in root.properties() {
                let _ = p.name();
            }
            for c in root.children() {
                let _ = c.name();
                for p in c.properties() {
                    let _ = p.name();
                }
            }
            for r in fdt.memory_reservations() {
                let _ = r.size();
            }
        }
    }

    /// Ours, the closest it can come: `from_bytes` on any N bytes.
    #[kani::proof]
    #[kani::unwind(20)]
    fn ours_from_bytes_is_total_64() {
        let b: [u8; 64] = kani::any();
        let _ = device_tree_blob::DeviceTreeBlob::from_bytes(&b);
    }

    /// Ours: one whole walker over any 64 bytes.
    #[kani::proof]
    #[kani::unwind(20)]
    fn ours_memory_regions_never_panics_64() {
        let b: [u8; 64] = kani::any();
        if let Ok(dt) = device_tree_blob::DeviceTreeBlob::from_bytes(&b) {
            let mut out = [device_tree_blob::Region { start: 0, size: 0 }; 2];
            let _ = dt.memory_regions(&mut out);
        }
    }
}
SOURCE
    rm -rf "$WORK/ours/src" "$WORK/ours/tests"
    mkdir -p "$WORK/ours"
    cp -R "$ROOT/crates/device_tree_blob/src" "$ROOT/crates/device_tree_blob/tests" "$WORK/ours/"
    printf '[package]\nname = "device_tree_blob"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n\n[features]\nrecord = []\n' \
        > "$WORK/ours/Cargo.toml"
    python3 -I "$WORK/record.py" "$WORK/ours/src/lib.rs"
}

# The hostile tests' blobs, one file per distinct blob, with the fixtures they share taken out.
record() {
    rm -rf "$WORK/recorded"
    mkdir -p "$WORK/recorded"
    (cd "$WORK/ours" && RECORD_DIR="$WORK/recorded" cargo "+$PIN" test -q --features record -- --test-threads=1 >/dev/null)
    for f in "$ROOT"/crates/device_tree_blob/tests/fixtures/*.dtb; do
        for g in "$WORK"/recorded/*.dtb; do
            if cmp -s "$f" "$g"; then rm -f "$g"; fi
        done
    done
}

write
cd "$WORK"
case "$MODE" in
write) ;;
agree)
    cargo "+$PIN" build -q --release -p agree
    record
    mkdir -p "$WORK/probes-out"
    python3 -I "$WORK/probes/make_probes.py" "$ROOT/crates/device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb" "$WORK/probes-out" >/dev/null
    (cd "$ROOT" && "$WORK/target/release/agree" $(git ls-files '*.dtb'))
    "$WORK/target/release/agree" "$WORK"/recorded/*.dtb
    sort -u "$WORK/recorded/index.txt"
    "$WORK/target/release/agree" "$WORK"/probes-out/*.dtb
    ;;
deep)
    cargo "+$PIN" build -q --release -p agree
    for n in 1000 10000 50000 174000; do "$WORK/target/release/deep" chain "$n"; done
    "$WORK/target/release/deep" flat 174000
    for n in 16383 16384; do "$WORK/target/release/deep" chain "$n" display 2>&1 | grep -v '^note:'; done
    ;;
bench)
    cargo "+$PIN" build -q --release -p agree
    F="$ROOT/crates/device_tree_blob/tests/fixtures"
    M="$ROOT/crates/machine_discovery/tests/fixtures"
    "$WORK/target/release/bench" "$F/qemu-aarch64-virt.dtb" "$F/qemu-riscv64-virt.dtb" \
        "$M/qemu-aarch64-virt-gicv3.dtb" "$M/visionfive2-uboot-control.dtb" "$M/jh7110.dtb"
    ;;
fit)
    BIN=$(rustc "+$PIN" --print sysroot)/lib/rustlib/$(rustc "+$PIN" -vV | sed -n 's/^host: //p')/bin
    for t in aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf x86_64-unknown-none; do
        for f in ours dtk; do
            if (cd "$WORK/fit" && CARGO_TARGET_DIR=../target-fit cargo "+$PIN" build -q --release \
                --target "$t" --features "$f" 2>/dev/null); then
                echo "$t [$f] links; text, rodata: $("$BIN/llvm-size" -A "$WORK/target-fit/$t/release/fit" \
                    | awk '$1==".text"{t=$2} $1==".rodata"{r=$2} END{print t+0, r+0}')"
            else
                echo "$t [$f] does not link"
            fi
        done
    done
    ;;
fuzz)
    SECS=${1:-1200}
    record
    mkdir -p "$WORK/seeds"
    (cd "$ROOT" && for f in $(git ls-files '*.dtb'); do cp "$f" "$WORK/seeds/$(echo "$f" | tr / _)"; done)
    cp "$WORK"/recorded/*.dtb "$WORK/seeds/"
    cp "$ROOT/fuzz/fuzz_targets/device_tree_blob_walk.rs" "$WORK/fuzz/fuzz_targets/ours.rs"
    curl -sfL "https://raw.githubusercontent.com/google/dtoolkit/$DTK_PARSE_REV/fuzz/fuzz_targets/parse.rs" \
        -o "$WORK/fuzz/fuzz_targets/dtoolkit_parse.rs"
    cd "$WORK/fuzz"
    cargo "+$PIN" fuzz build -O
    for t in ours dtoolkit_nife dtoolkit_parse; do
        mkdir -p "corpus/$t" "artifacts/$t"
        # fork mode with crashes, timeouts and OOMs ignored, so a run counts failures instead of
        # stopping at the first; each one lands in artifacts/<target>/.
        cargo "+$PIN" fuzz run -O "$t" "corpus/$t" "$WORK/seeds" -- \
            -dict="$ROOT/fuzz/dictionaries/device_tree_blob_walk.dict" -artifact_prefix="artifacts/$t/" \
            -max_total_time="$SECS" -timeout=10 -malloc_limit_mb=512 -rss_limit_mb=2048 -fork=8 \
            -ignore_crashes=1 -ignore_timeouts=1 -ignore_ooms=1 -print_final_stats=1 > "$t.log" 2>&1 &
    done
    wait
    for t in ours dtoolkit_nife dtoolkit_parse; do
        echo "$t: $(find "artifacts/$t" -type f | wc -l | tr -d ' ') artifacts; $(tr '\r' '\n' < "$t.log" | grep -o '#[0-9]*: cov: [0-9]*' | tail -1)"
    done
    ;;
kani)
    (cd "$WORK/ours" && cargo kani)
    for h in be_readers_are_total u32_reads_big_endian next_multiple_of_four_rounds_up \
        dtoolkit_reg_decodes_any_value ours_reg_decodes_any_value ours_from_bytes_is_total_64 \
        fdt_new_is_total_64 accepted_tree_never_panics_64 ours_memory_regions_never_panics_64; do
        echo "$h: $(cd "$WORK/kani" && timeout 900 cargo kani --harness "proofs::$h" --exact 2>&1 \
            | grep -E 'VERIFICATION:-|Verification Time' | tr '\n' ' ')"
    done
    ;;
churn)
    for v in 0.2.0 0.2.1 0.3.0 0.4.0 main; do
        d="$WORK/churn/$v"
        mkdir -p "$d/src"
        cp "$WORK/adapter/src/lib.rs" "$d/src/"
        if [ "$v" = main ]; then
            dep="dtoolkit = { git = \"https://github.com/google/dtoolkit\", rev = \"$DTK_MAIN_REV\", default-features = false }"
        else
            dep="dtoolkit = { version = \"=$v\", default-features = false }"
        fi
        printf '[package]\nname = "churn"\nversion = "0.0.0"\nedition = "2024"\n\n[dependencies]\n%s\n\n[workspace]\n' "$dep" > "$d/Cargo.toml"
        echo "$v: $( (cd "$d" && cargo "+$PIN" build -q 2>&1) | grep -c '^error\[' ) errors"
    done
    ;;
*) echo "dtoolkit-probe.sh: unknown mode '$MODE'" >&2; exit 2 ;;
esac
