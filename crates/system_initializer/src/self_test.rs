//! **The boot self-test's userspace half**: build `boot_self_test`, hand it the kernel's half and
//! what its checks need, wait for its verdict, and print its transcript before the prompt
//! (milestone 1063 (the boot self-test's spawning checks run from userspace)). The checks
//! themselves are the program's; this is only the wiring, in the terms
//! `boot_ladder::self_test_program` fixes. See notes/self-test-checks.md, "The two halves".
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane
//! `milestone/1063-the-boot-self-test-spawns-from-userspace`.

//! # BUGS
//!
//! - A program that never sends `DONE` and never dies holds the boot here, before the shell.
//! - The progenitor can print only once the shell's output page is mapped, so the transcript
//!   reaches the console after the steps between the program and the shell have run.
//! - The test and benchmark kernels run no self-test and grant no page, and their boots print no
//!   verdict at all.

use boot_ladder::self_test_program::{
    self as contract, entropy_source, header, image, image_virtual_address, images_fit, message,
    slot,
};

use super::*;
use crate::boot_steps::Entropy;

/// Where the progenitor maps the transcript frame, read-only, to print it.
const TRANSCRIPT_VIEW_VIRTUAL_ADDRESS: u64 = address_space_map::pair_page(0x0f90_0000);

/// The program's own stack, in pages: it parses ELF images and builds children, a deeper chain than
/// an ordinary child's.
const SELF_TEST_STACK_PAGES: u64 = 16;

/// Pages for the program's own image, its stack and its tables, over its image and blobs.
const SELF_TEST_BUILD_SLACK_PAGES: u64 = 96;

/// Pages for the children the program builds, over twice the largest image: one child at a time,
/// plus the depleter's own budget and every child's stack and tables.
const SELF_TEST_BUDGET_SLACK_PAGES: u64 = 192;

/// **Which of the kernel's conditional self-test slots hold something**, probed at the top of
/// [`boot`](crate::boot) before anything retypes into an empty one.
#[derive(Clone, Copy)]
pub(super) struct SelfTestGrants {
    page: bool,
    random_number_generator: bool,
    disk: bool,
}

impl SelfTestGrants {
    /// Probe the slots.
    pub(super) fn probe(g: &BootEndowment) -> Self {
        Self {
            page: is_granted(g.self_test_page),
            random_number_generator: is_granted(g.self_test_random_number_generator),
            disk: g.self_test_disk.iter().all(|&s| is_granted(s)),
        }
    }
}

/// What [`run_self_test`] leaves for [`print_self_test`]: the transcript frame and how far the
/// program wrote, and whether it said `DONE`.
pub(super) struct SelfTestTranscript {
    frame: Option<u64>,
    end: usize,
    done: bool,
    missing: bool,
}

/// **Build and run `boot_self_test`, and wait for it to finish.** Every capability the kernel
/// granted for it is deleted on every path, and its regions are reclaimed once it is done.
#[inline(never)]
pub(super) fn run_self_test(
    g: &BootEndowment,
    memory_region: u64,
    (fs, table): (&nifefs::Fs<'static>, &str),
    grants: SelfTestGrants,
    entropy: &Entropy,
) -> SelfTestTranscript {
    // A kernel without a self-test grants no page, and there is nothing to run.
    if !grants.page {
        return SelfTestTranscript {
            frame: None,
            end: 0,
            done: false,
            missing: false,
        };
    }
    let transcript = run_measured(g, memory_region, (fs, table), grants, entropy);
    capability_delete(g.self_test_page);
    if grants.random_number_generator {
        capability_delete(g.self_test_random_number_generator);
    }
    if grants.disk {
        for s in g.self_test_disk {
            capability_delete(s);
        }
    }
    transcript
}

/// [`run_self_test`] without the cleanup of the kernel's grants.
fn run_measured(
    g: &BootEndowment,
    memory_region: u64,
    (fs, table): (&nifefs::Fs<'static>, &str),
    grants: SelfTestGrants,
    entropy: &Entropy,
) -> SelfTestTranscript {
    let none = SelfTestTranscript {
        frame: None,
        end: 0,
        done: false,
        missing: true,
    };
    let Some(elf) = measured(fs, table, "boot_self_test").elf else {
        return none;
    };
    let Some(ImageTable {
        lengths_bytes: lengths,
        images,
        head,
    }) = image_table((fs, table), entropy)
    else {
        return none;
    };

    let image_pages: u64 = lengths
        .iter()
        .map(|l| l.div_ceil(process_builder::PAGE))
        .sum();
    let largest = lengths
        .iter()
        .max()
        .copied()
        .unwrap_or(0)
        .div_ceil(process_builder::PAGE);
    let own_pages = elf
        .segments()
        .map(|s| {
            let (start, end) = s.page_range(process_builder::PAGE);
            (end - start) / process_builder::PAGE
        })
        .sum::<u64>();
    let build_pages =
        own_pages + image_pages + 1 + SELF_TEST_STACK_PAGES + SELF_TEST_BUILD_SLACK_PAGES;
    let budget_pages = 2 * largest + SELF_TEST_BUDGET_SLACK_PAGES;

    let built = must(memory_region_split(memory_region, build_pages));
    let budget = must(memory_region_split(memory_region, budget_pages));
    let report = must(retype_obj(memory_region, abi::objtype::RENDEZVOUS));
    let frame = must(retype_page_frame(memory_region));
    must_ok(map_page_frame(
        frame,
        TRANSCRIPT_VIEW_VIRTUAL_ADDRESS,
        false,
        memory_region,
    ));

    let (placed, n) = placements(g, grants, entropy);
    let blobs = [
        (contract::IMAGES_VIRTUAL_ADDRESS, &head[..]),
        (image_virtual_address(&lengths, 0), images[0]),
        (image_virtual_address(&lengths, 1), images[1]),
        (image_virtual_address(&lengths, 2), images[2]),
    ];
    let child = must(build_child(
        memory_region,
        built,
        &elf,
        &ChildEndowment {
            capabilities: &[(budget, abi::rights::ALL), (report, abi::rights::WRITE)],
            placed: &placed[..n],
            maps: &[
                (
                    address_space_map::pair_page(contract::KERNEL_HALF_VIRTUAL_ADDRESS),
                    g.self_test_page,
                    abi::address_space::MAP_RO,
                ),
                (
                    machine_statistics_contract::PAGE_VA,
                    g.machine_page,
                    abi::address_space::MAP_RO,
                ),
                (
                    address_space_map::pair_page(contract::TRANSCRIPT_VIRTUAL_ADDRESS),
                    frame,
                    abi::address_space::MAP_RW,
                ),
            ],
            blobs: &blobs,
            fault: Some(report),
            stack_pages: SELF_TEST_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    must_ok(start_child(child, 0, 0, 0));

    let (end, done) = wait_for_verdict(report);
    // The budget first: the program's own region is the budget's sibling, not its parent.
    reclaim(budget);
    reclaim(built);
    capability_delete(report);
    SelfTestTranscript {
        frame: Some(frame),
        end,
        done,
        missing: false,
    }
}

/// The header page and the images, as [`image_virtual_address`] lays them out.
struct ImageTable {
    lengths_bytes: [u64; image::COUNT],
    images: [&'static [u8]; image::COUNT],
    head: [u8; header::COUNT * 8],
}

/// **Read the images the table vouches for and write the header**, or `None` when they do not fit
/// the program's address space.
fn image_table((fs, table): (&nifefs::Fs<'static>, &str), entropy: &Entropy) -> Option<ImageTable> {
    let source = match (entropy.client, entropy.from_instruction) {
        (None, _) => entropy_source::NONE,
        (Some(_), true) => entropy_source::INSTRUCTION,
        (Some(_), false) => entropy_source::DEVICE,
    };
    // Each image only when the table vouches for it; an empty one fails its check by name.
    let mut lengths = [0u64; image::COUNT];
    let mut images: [&[u8]; image::COUNT] = [&[]; image::COUNT];
    for (i, name) in image::NAMES.iter().enumerate() {
        if measured(fs, table, name).elf.is_some()
            && let Some(bytes) = fs.read(name)
        {
            lengths[i] = bytes.len() as u64;
            images[i] = bytes;
        }
    }
    if !images_fit(&lengths) {
        return None;
    }
    let mut words = [0u64; header::COUNT];
    words[header::ENTROPY_SOURCE] = source;
    words[header::IMAGE_BYTE_COUNTS..].copy_from_slice(&lengths);
    let mut head = [0u8; header::COUNT * 8];
    for (chunk, w) in head.as_chunks_mut::<8>().0.iter_mut().zip(words) {
        chunk.copy_from_slice(&w.to_le_bytes());
    }

    Some(ImageTable {
        lengths_bytes: lengths,
        images,
        head,
    })
}

/// **The capabilities the program finds in its fixed slots**: its own thread always, then each
/// service and device this machine has. Returns the placements and how many are in use.
fn placements(
    g: &BootEndowment,
    grants: SelfTestGrants,
    entropy: &Entropy,
) -> ([(u64, u64, u64); 6], usize) {
    let mut placed = [(0u64, 0u64, 0u64); 6];
    let mut n = 0;
    let mut place = |child_slot, ours, rights| {
        placed[n] = (child_slot, ours, rights);
        n += 1;
    };
    place(
        slot::OWN_THREAD,
        supervision_contract::CHILDS_OWN_THREAD,
        abi::rights::WRITE,
    );
    if let Some(client) = entropy.client {
        place(slot::ENTROPY, client, abi::rights::WRITE);
    }
    if grants.random_number_generator {
        place(
            slot::RANDOM_NUMBER_GENERATOR,
            g.self_test_random_number_generator,
            abi::rights::WRITE,
        );
    }
    if grants.disk {
        // `GRANT` on the transport and the interrupt, because the program passes both on to
        // `block_driver`; the DMA page it only maps into the driver, which needs none.
        let [transport, interrupt, direct_memory_access] = g.self_test_disk;
        place(
            slot::DISK_TRANSPORT,
            transport,
            abi::rights::WRITE | abi::rights::GRANT,
        );
        place(
            slot::DISK_INTERRUPT,
            interrupt,
            abi::rights::READ | abi::rights::GRANT,
        );
        place(
            slot::DISK_DIRECT_MEMORY_ACCESS,
            direct_memory_access,
            abi::rights::READ | abi::rights::WRITE,
        );
    }
    (placed, n)
}

/// **Follow the program's transcript until it says `DONE` or dies**: every message is the
/// program's, `LINE` or `DONE`, until its death arrives on the same endpoint as its supervisor's
/// message. Returns the transcript's end and whether the verdict came.
fn wait_for_verdict(report: u64) -> (usize, bool) {
    let mut end = 0usize;
    let done = loop {
        let (code, at, ..) = user_mode_runtime::receive_fault(report);
        match code {
            message::LINE => end = (at as usize).min(contract::TRANSCRIPT_BYTES),
            message::DONE => {
                end = (at as usize).min(contract::TRANSCRIPT_BYTES);
                break true;
            }
            _ => break false,
        }
    };
    (end, done)
}

/// `DESTROY` a region, retrying while a resident it holds is still being killed, a bounded number
/// of times; a region that will not go stays spent, and the boot carries on.
fn reclaim(region: u64) {
    for _ in 0..1000 {
        if user_mode_runtime::destroy_memory_region(region) == 0 {
            break;
        }
        user_mode_runtime::yield_now();
    }
    capability_delete(region);
}

/// **Print the transcript, line by line, then let the frame go.** A program that died before its
/// verdict, or a boot with no program, says so: with no `nife self-test:` line, `boot-check` fails
/// the boot.
#[inline(never)]
pub(super) fn print_self_test(terminal_endpoint: u64, t: &SelfTestTranscript) {
    if t.missing {
        announce(
            terminal_endpoint,
            b"progenitor: boot_self_test could not be measured or built, so the self-test has no verdict\n",
        );
        return;
    }
    let Some(frame) = t.frame else { return };
    // SAFETY: the transcript frame is mapped read-only here at `TRANSCRIPT_VIEW_VIRTUAL_ADDRESS`, one page, and
    // the program that wrote it is gone, so nothing changes under the read.
    let bytes =
        unsafe { core::slice::from_raw_parts(TRANSCRIPT_VIEW_VIRTUAL_ADDRESS as *const u8, t.end) };
    for line in bytes.split_inclusive(|&b| b == b'\n') {
        announce(terminal_endpoint, line);
    }
    if !t.done {
        announce(
            terminal_endpoint,
            b"progenitor: boot_self_test died before its verdict\n",
        );
    }
    process_builder::give_up_own_page(TRANSCRIPT_VIEW_VIRTUAL_ADDRESS);
    capability_delete(frame);
}
