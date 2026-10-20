//! `std_layout`: where a `std` child's endowment goes in its table and its space. Moved out of
//! `lib.rs` unchanged but for visibility and one link by milestone 812 (`std::thread::spawn` runs
//! real threads in one address space), whose three new slots grew it past §266 (a Rust source file
//! stays under 2,000 lines)'s baseline.
//!
//! Name: provisional (milestone 812's lane, 2026-10-10 UTC).

/// **A `std` child's endowment, placed where nife's `std` reads it** (milestone 595 (provisional)).
///
/// The native spawn fills a child's table from slot 0, output first; nife's `std` fixes a slot per
/// authority (`crates/std_runtime_contract`), and a native child's output slot is a `std` child's
/// heap budget, so a `std` program built natively dies on its first `println!`. This is the other
/// shape, from the decisions [`spawn_service`](super::spawn_service) already made.
///
/// - slot 0: the job's own region, `SPEND_ONLY`, as the heap's budget. See
///   [`grant_plan::STD_REGION_PAGES`] for why it is the same region the child is built from.
/// - slot 1: the output, exactly the capability a native child would get at slot 0.
/// - slot 4 and a page at `FS_PAGE`: the caretaker's narrowed endpoint, if the line granted a
///   directory. The same frame the caretaker and the file server map, at the `std` address.
/// - slot 5 and a read-only page at `CLOCK_PAGE`, slot 7 and one at `CONFIG_PAGE`: the manifest's
///   clock and configuration pages.
/// - slot 6: the entropy service, `WRITE`, if the manifest declared it and this boot built one.
/// - slot 8 and a read-only page at `ARGS_PAGE`: the line's argv (milestone 205, DECISIONS §170),
///   if the shell sent one. The page is the child's own, copied out of the shell's frame.
/// - slots 2 and 3: the network stack, `WRITE`, and the job's region again, `ALL`; the net PAL
///   retypes each socket's page from the region and lends it (milestone 809 (the package client
///   becomes a program)), and since milestone 824 (a retype mints no right its budget lacks) the
///   lending `GRANT` has to be on the region. Slots 15, 16 and 17: the installer endpoint, its
///   reply endpoint and the catalog page, mapped at `CATALOG_PAGE` (milestone 809;
///   `grant_plan::INSTALLER_SLOT` has the rights). Slot 18 and a read-only page at
///   `INDEX_SOURCES_PAGE`: the image's index sources, beside the index store at slot 4 (milestone
///   801 (packages over the internet)).
/// - slots 9 and 22 to 28: one resolver badge per granted zone, for a job holding the network; slot
///   29: the boot terminal as stdin, for `console_input` (milestone 892 (a std program at the
///   prompt holds the network and a resolver)); slot 30: the line's input as a byte stream, `READ`,
///   when it redirected or piped one (milestone 784 (reuse what the tree wrote), item (f2)).
///
/// Name: provisional.
///
/// # BUGS
///
/// - **The child holds `WRITE` on the region it is built in**, because that region is its heap
///   (`grant_plan::STD_REGION_PAGES` says why one region). `WRITE` on a region is also `SPLIT`,
///   and a region that has been split cannot be destroyed until its children are, so a program
///   that splits its own heap pins its job region: `job_undertaker` never reclaims it, and the
///   pool is one region smaller until reboot. nife's `std` never splits (its allocator only
///   `MAP`s), so this takes a program written to do it. Closing it needs a right that allows `MAP`
///   and not `SPLIT`, which is the syscall surface and an architect's call.
pub(super) struct StdLayout {
    pub(super) caps: [(u64, u64); 2],
    placed: [(u64, u64, u64); 24],
    placed_n: usize,
    maps: [(u64, u64, u64); 6],
    maps_n: usize,
}

/// **What a `std` child is granted beyond its heap and its output**, each `None` when the manifest
/// did not ask or the boot has none: the pieces [`StdLayout::new`] places, named so the call site
/// says which is which.
pub(super) struct StdGrants {
    /// The directory grant's endpoint and the page it shares.
    pub(super) dir: Option<(u64, u64)>,
    pub(super) clock: Option<u64>,
    pub(super) config: Option<u64>,
    pub(super) entropy: Option<u64>,
    /// The child's own copy of the argv page.
    pub(super) args: Option<u64>,
    /// The network stack's endpoint (milestone 809).
    pub(super) network: Option<u64>,
    /// The installer endpoint and its reply endpoint (milestone 809, §270).
    pub(super) installer: Option<(u64, u64)>,
    /// The child's own copy of the image's catalog (milestone 809).
    pub(super) catalog: Option<u64>,
    /// The name resolver's endpoint, one badged copy per zone this job was granted, filled from
    /// the front (milestone 892).
    pub(super) resolvers: Resolvers,
    /// The child's own copy of the image's index sources (milestone 801).
    pub(super) index_sources: Option<u64>,
    /// The boot terminal, badged as standard input (milestone 892).
    pub(super) stdin: Option<u64>,
    /// The line's input, a file the shell streams or a pipe's read end (milestone 784, item (f2)).
    pub(super) source: Option<u64>,
}

// `caps` lands in order from slot 0, so the two in-order slots must be 0 and 1. Checked here rather
// than assumed, because the contract crate could renumber them and this would build a child whose
// heap and output were swapped.
const _: () = assert!(
    std_runtime_contract::MEMORY_REGION_SLOT == 0 && std_runtime_contract::STDOUT_SLOT == 1
);

/// **One badged resolver endpoint per granted zone**, in the order std asks them
/// (`std_runtime_contract::RESOLVER_SLOTS`). Name: provisional, minted 2026-10-19 (UTC) by
/// milestone/892-installed-programs-hold-the-network-and-zones.
pub(super) type Resolvers = [Option<u64>; std_runtime_contract::RESOLVER_SLOTS.len()];

/// No resolver badge at all.
pub(super) const NO_RESOLVERS: Resolvers = [None; std_runtime_contract::RESOLVER_SLOTS.len()];

// `grant_plan` places `jig`'s installer, reply, catalog and index sources in the std layout too, and
// the shell's configuration sits at 21; none may land on a resolver badge, and all sit below the
// first slot std makes its own capabilities in.
const _: () = {
    let theirs = [
        grant_plan::INSTALLER_SLOT,
        grant_plan::INSTALLER_REPLY_SLOT,
        grant_plan::CATALOG_SLOT,
        grant_plan::INDEX_SOURCES_SLOT,
        grant_plan::SHELL_CONFIGURATION_SLOT,
    ];
    let mut i = 0;
    while i < theirs.len() {
        assert!(theirs[i] < std_runtime_contract::SLOTS);
        let mut r = 0;
        while r < std_runtime_contract::RESOLVER_SLOTS.len() {
            assert!(theirs[i] != std_runtime_contract::RESOLVER_SLOTS[r]);
            r += 1;
        }
        i += 1;
    }
};

impl StdLayout {
    pub(super) fn new(region: u64, out: (u64, u64), g: StdGrants) -> Self {
        use std_runtime_contract as rt;
        let StdGrants {
            dir,
            clock,
            config,
            entropy,
            args,
            network,
            installer,
            catalog,
            resolvers,
            index_sources,
            stdin,
            source,
        } = g;
        // The heap's budget is spend-only, unless the child holds the installer: `jig` stages a
        // package in pages retyped from its heap and lends them to the progenitor, and since
        // milestone 824 (a retype mints no right its budget lacks) a page carries its budget's
        // rights, so the lending `GRANT` has to be on the budget.
        let heap = if installer.is_some() {
            abi::rights::ALL
        } else {
            abi::rights::SPEND_ONLY
        };
        let mut l = StdLayout {
            caps: [(region, heap), out],
            placed: [(0, 0, 0); 24],
            placed_n: 0,
            maps: [(0, 0, 0); 6],
            maps_n: 0,
        };
        use abi::address_space::MAP_RW;
        use abi::rights::{BIND, READ, WRITE};
        // Each grant at its slot, and a page grant mapped where `std` reads it too.
        if let Some((ep, page)) = dir {
            l.place(rt::FS_DIR_SLOT, ep, WRITE);
            l.map(rt::FS_PAGE, page, MAP_RW);
        }
        l.page(clock, rt::CLOCK_SLOT, rt::CLOCK_PAGE);
        if let Some(ep) = entropy {
            l.place(rt::ENTROPY_SLOT, ep, WRITE);
        }
        l.page(config, rt::CONFIG_SLOT, rt::CONFIG_PAGE);
        l.page(args, rt::ARGS_SLOT, rt::ARGS_PAGE);
        if let Some(stack) = network {
            l.place(rt::STACK_SLOT, stack, WRITE);
            // Lent pages carry this budget's rights, milestone 824 (a retype mints no right its
            // budget lacks), so `GRANT` is delegated here rather than minted with a page.
            l.place(rt::NET_MEMORY_REGION_SLOT, region, abi::rights::ALL);
        }
        if let Some((endpoint, reply)) = installer {
            l.place(grant_plan::INSTALLER_SLOT, endpoint, WRITE);
            l.place(grant_plan::INSTALLER_REPLY_SLOT, reply, READ);
        }
        if let Some(endpoint) = stdin {
            l.place(rt::STDIN_SLOT, endpoint, WRITE);
        }
        if let Some(endpoint) = source {
            l.place(rt::STDIN_STREAM_SLOT, endpoint, READ);
        }
        for (slot, endpoint) in rt::RESOLVER_SLOTS.into_iter().zip(resolvers) {
            if let Some(endpoint) = endpoint {
                l.place(slot, endpoint, WRITE);
            }
        }
        l.page(catalog, grant_plan::CATALOG_SLOT, rt::CATALOG_PAGE);
        l.page(
            index_sources,
            grant_plan::INDEX_SOURCES_SLOT,
            rt::INDEX_SOURCES_PAGE,
        );
        // **Threads** (milestone 812 (`std::thread::spawn` runs real threads in one address
        // space)): every `std` program is built as a process of its own and may add threads to it,
        // within its own region's budget. Its own process with `BIND` and nothing more, its own
        // first thread to set its thread pointer through, and its own space with `READ` for the
        // futex waits its locks make. None of the three reaches anything outside the program.
        l.place(
            rt::PROCESS_SLOT,
            supervision_protocol::CHILDS_OWN_PROCESS,
            BIND,
        );
        l.place(
            rt::THREAD_SLOT,
            supervision_protocol::CHILDS_OWN_THREAD,
            WRITE,
        );
        l.place(rt::SPACE_SLOT, supervision_protocol::CHILDS_OWN_SPACE, READ);
        l
    }

    fn place(&mut self, slot: u64, cap: u64, rights: u64) {
        self.placed[self.placed_n] = (slot, cap, rights);
        self.placed_n += 1;
    }

    fn map(&mut self, va: u64, cap: u64, mode: u64) {
        self.maps[self.maps_n] = (va, cap, mode);
        self.maps_n += 1;
    }

    /// A read-only page, if the child is given it: its capability at `slot`, mapped at `va`.
    fn page(&mut self, page: Option<u64>, slot: u64, va: u64) {
        if let Some(page) = page {
            self.place(slot, page, abi::rights::READ);
            self.map(va, page, abi::address_space::MAP_RO);
        }
    }

    pub(super) fn placed(&self) -> &[(u64, u64, u64)] {
        &self.placed[..self.placed_n]
    }

    pub(super) fn maps(&self) -> &[(u64, u64, u64)] {
        &self.maps[..self.maps_n]
    }
}
