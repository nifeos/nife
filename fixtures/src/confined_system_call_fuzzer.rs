//! A confined process that issues random system calls, one at a time, under a conductor (milestone
//! 779 (fuzz the surface a confined process can reach), provisional).
//!
//! `system_tests/src/user/confined_fuzzer_tests.rs` spawns three of these with the endowment
//! `confined_system_call_fuzzer_contract` lays out, and between every two calls checks that none holds
//! a capability, or maps a page, it was never granted. This is the generator: it waits on [`GO`],
//! makes one call drawn from its seed, writes what it did to the report page, and waits again. It
//! never interprets an answer, so a wrong answer cannot steer it; the oracle is the conductor's.
//! `arg0` is the seed, which alone decides every call, so a run replays from it; `arg1` is the role
//! (`confined_system_call_fuzzer_contract::ROLE_*`).

//! # What the generator reaches
//!
//! `SYSTEM_CALL_INVOKE` on any slot but [`GO`] with any method and any words, biased toward the endowment
//! and the methods objects have; `SYSTEM_CALL_CAPABILITY_DELETE`; `SYSTEM_CALL_YIELD`; and unknown system call numbers. Words
//! are small integers, slot numbers, rights masks, page-aligned addresses in the playground, report
//! page, image and stack, the ABI's tag values, and raw random words. It is spawned with
//! `user::run` and enters through `svc`/`ecall`/`system_call`, so it reaches what milestone 752 (a
//! seeded system call driver with a shadow model)'s kernel threads skip by entering at
//! `system_call::invoke`: the trap entry, the register marshaling, and every check that depends on the
//! caller having a user address space.

//! # BUGS
//!
//! - [`GO`] is never fuzzed: the conductor needs it. It holds `READ` alone, so `RECEIVE` and
//!   `RECEIVE_CAPABILITY` on it go unexercised.
//! - No thread or timer is ever made: a `MemoryRegion::RETYPE_OBJECT` for either is rewritten to a
//!   rendezvous, since a thread or a timer would make a seed's run depend on timing.
//! - `SYSTEM_CALL_EXIT` and `SYSTEM_CALL_EXIT_THREAD` are never drawn; either would end the run.
//! - It never touches the memory it maps; the page oracle reads the page tables instead.
//!
//! Name: ratified 2026-10-17 (calef, convention-rulings Rule 2).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates)'s ratchet tracks
// (DECISIONS §107 (`missing_docs` moves to `workspace.lints.rust`)): each `[[bin]]` is its own crate root with one `_start`.
#![allow(missing_docs)]
#![no_main]

use abi::{SYSTEM_CALL_EXIT, SYSTEM_CALL_INVOKE, SYSTEM_CALL_YIELD};
use confined_system_call_fuzzer_contract::{
    ENDOWED, GO, GO_EXIT, NO_CAPABILITY, PLAYGROUND_PAGES, PLAYGROUND_VA, REPORT_VA,
    SYSTEM_CALL_CAPABILITY_DELETE, report,
};
use user_mode_runtime::{exit, raw_system_call, receive};

const SLOTS: u64 = abi::CAPABILITY_TABLE_SLOTS;
const PAGE: u64 = address_space_layout::PAGE_BYTES;

fn splitmix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        splitmix(self.0)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    /// A slot: mostly the endowment and the slots just above it, where delegations land; sometimes
    /// anywhere in the table; sometimes past it. Never [`GO`].
    fn slot(&mut self) -> u64 {
        let s = match self.below(10) {
            0..=4 => self.below(ENDOWED),
            5..=7 => self.below(ENDOWED + 8),
            8 => self.below(SLOTS),
            _ => match self.below(3) {
                0 => SLOTS,
                1 => u64::MAX,
                _ => self.next(),
            },
        };
        if s == GO { GO + 1 } else { s }
    }

    /// A virtual address worth trying: mostly a playground page, sometimes one the process already
    /// uses (the report page, its image, its stack), sometimes one no user mapping may have.
    fn address(&mut self) -> u64 {
        match self.below(10) {
            0..=5 => PLAYGROUND_VA + self.below(PLAYGROUND_PAGES) * PAGE,
            6 => REPORT_VA,
            7 => address_space_layout::IMAGE_BASE_VIRTUAL_ADDRESS + self.below(4) * PAGE,
            8 => address_space_layout::STACK_TOP_PAGE_VIRTUAL_ADDRESS,
            _ => match self.below(3) {
                0 => PLAYGROUND_VA + self.below(PAGE), // misaligned
                1 => 0xFFFF_0000_0000_0000 | (self.next() & 0xFFFF_F000), // the kernel half
                _ => self.next(),
            },
        }
    }

    /// A word for an argument register.
    fn word(&mut self) -> u64 {
        match self.below(12) {
            0 | 1 => self.below(8),
            2 | 3 => self.slot(),
            4 => self.below(16), // a rights mask, ENUMERATE included
            5 | 6 => self.address(),
            7 => 1 + self.below(4), // a page count
            8 => NO_CAPABILITY,
            9 => match self.below(3) {
                0 => abi::rendezvous::REPLY_DELIVERED,
                1 => abi::notification::BOUND,
                _ => (abi::Error::NoSuchSlot as i64) as u64,
            },
            10 => self.below(1 << 16),
            _ => self.next(),
        }
    }

    /// A method number: mostly one some object answers, sometimes not.
    fn method(&mut self) -> u64 {
        if self.chance(88) {
            self.below(8)
        } else if self.chance(50) {
            self.below(32)
        } else {
            self.next()
        }
    }
}

/// One call: a system call number and its six words.
fn pick(rng: &mut Rng) -> (u64, [u64; 6]) {
    let (nr, mut words) = draw(rng);
    // `MemoryRegion::RETYPE_OBJECT`'s first word is the object type. A TCB or a timer would run on its
    // own clock and break the seed's determinism (see the module's `BUGS`), so either becomes a
    // rendezvous. Applied whatever the slot holds, which nudges a `SEND_CAPABILITY`'s capability argument
    // off 3 and 5 too; that is the price of not knowing the slot's type.
    if nr == SYSTEM_CALL_INVOKE
        && words[1] == abi::memory_region::RETYPE_OBJECT
        && (words[2] == abi::object_type::THREAD_CONTROL_BLOCK
            || words[2] == abi::object_type::TIMER)
    {
        words[2] = abi::object_type::RENDEZVOUS;
    }
    (nr, words)
}

fn draw(rng: &mut Rng) -> (u64, [u64; 6]) {
    let roll = rng.below(100);
    if roll < 88 {
        let slot = rng.slot();
        let method = rng.method();
        (
            SYSTEM_CALL_INVOKE,
            [slot, method, rng.word(), rng.word(), rng.word(), 0],
        )
    } else if roll < 94 {
        (SYSTEM_CALL_CAPABILITY_DELETE, [rng.slot(), 0, 0, 0, 0, 0])
    } else if roll < 96 {
        (SYSTEM_CALL_YIELD, [0; 6])
    } else {
        // A number no system call answers, or one a little past the last real one (`SYSTEM_CALL_EXIT_THREAD`,
        // 4, since milestone 812 (`std::thread::spawn` runs real threads in one address space)).
        let nr = match rng.below(3) {
            0 => abi::SYSTEM_CALL_EXIT_THREAD + 1 + rng.below(12),
            1 => rng.below(1 << 16),
            _ => rng.next(),
        };
        // Neither exit is drawn: each would end the fuzzer, which teaches the oracle nothing.
        let nr = if nr == SYSTEM_CALL_EXIT || nr == abi::SYSTEM_CALL_EXIT_THREAD {
            SYSTEM_CALL_YIELD
        } else {
            nr
        };
        let mut words = [0; 6];
        for w in &mut words {
            *w = rng.word();
        }
        if words[0] == GO {
            words[0] = GO + 1;
        }
        (nr, words)
    }
}

fn report_word(i: usize, v: u64) {
    // SAFETY: the conductor maps one read-write page at REPORT_VA before this process runs, and
    // `report::WORDS` fits in it. Volatile so the conductor, reading the frame from the kernel,
    // sees every store in order when this thread is parked on GO.
    unsafe { core::ptr::write_volatile((REPORT_VA as *mut u64).add(i), v) }
}

user_mode_runtime::program_entry!(start);

fn start(seed: u64, role: u64, _arg2: u64) -> ! {
    let mut rng = Rng(splitmix(
        seed ^ role.wrapping_add(1).wrapping_mul(0xA076_1D64_78BD_642F),
    ));
    let mut steps = 0u64;
    loop {
        report_word(report::STEPS, steps);
        let (word, _, _) = receive(GO);
        if word == GO_EXIT {
            exit()
        }
        let (nr, words) = pick(&mut rng);
        report_word(report::NUMBER, nr);
        for (i, &w) in words.iter().enumerate() {
            report_word(report::ARGS + i, w);
        }
        // SAFETY: the point of the program. The kernel validates every word before acting on it;
        // `pick` never draws SYSTEM_CALL_EXIT and never names GO.
        let back = unsafe { raw_system_call(nr, words) };
        for (i, &w) in back.iter().enumerate() {
            report_word(report::RETURNS + i, w);
        }
        steps += 1;
    }
}

user_mode_runtime::panic_handler!();
