//! **Hand-assembled `x86_64` user programs**, because no compiled one exists yet.
//!
//! Milestone 161, roadmap item 4. Every other architecture in this tree runs real ELF binaries out
//! of an initial ramdisk; `x86_64-unknown-none` has none, because `crates/user_mode_runtime` has no
//! arms for this ISA and `fixtures/build.rs` cannot compile its C components for it
//! (notes/x86-port.md). So the programs the kernel needs in order to *have* a userspace at all are
//! written here as machine code, exactly as the other two ports' test fixtures still are
//! (`user/supervision_tests.rs`, `user/force_kill_tests.rs`), and for the same reason: a
//! four-instruction program that faults on purpose is not worth a compiler.
//!
//! **This module is not test-only, and that is the difference from those two files.** The boot tour
//! needs a real ring-3 process to show, and a `#[cfg(test)]` fixture cannot be it. One copy lives
//! here and the test modules alias it, so the ABI these programs speak is written down once.
//!
//! # Why the programs are `&[u32]` when x86 instructions are not four bytes
//!
//! `sched`'s child builders take a stub as `&[u32]`, one word per instruction, which is exactly
//! right on two fixed-width ISAs and means nothing here. Changing that type would have made the
//! aarch64 and RISC-V programs byte soup, losing the one-instruction-per-line-with-a-comment shape
//! that makes them readable; so instead each program below is written as a **byte listing** (what an
//! assembler actually produced, verified against `clang -target x86_64-unknown-none` rather than
//! recalled) and packed into little-endian words at compile time. The bytes reach memory in the same
//! order either way, which is the only thing the CPU cares about.
//!
//! # The ABI these speak
//!
//! `rax` carries the syscall number and `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9` the arguments
//! (`arch::x86_64::exceptions::TrapFrame::syscall_nr`), ratified as **DECISIONS §124**. These
//! programs are the first things in the tree to speak it from ring 3 through a scheduler.
//!
//! # BUGS
//!
//! - **Every immediate is a 32-bit one**, so a report word or a syscall number above `u32::MAX`
//!   would be silently truncated. Nothing here is near that, and the `debug_assert`s below say so;
//!   the sixty-four-bit forms are five bytes longer and would buy nothing today.
//! - **Nothing here checks a syscall's return value.** These are fixtures, not programs: what each
//!   one proves is that it reached the kernel and came back, and the kernel's own side of the
//!   transaction is what the caller asserts on.

/// Pack a byte listing into the little-endian words the child builders take. See this module's
/// header for why the type is `u32` at all.
macro_rules! packer {
    ($name:ident, $bytes:literal, $words:literal) => {
        const fn $name(b: [u8; $bytes]) -> [u32; $words] {
            let mut out = [0u32; $words];
            let mut i = 0;
            while i < $words {
                out[i] = u32::from_le_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]);
                i += 1;
            }
            out
        }
    };
}

packer!(pack_1, 4, 1);
packer!(pack_2, 8, 2);
packer!(pack_4, 16, 4);
packer!(pack_8, 32, 8);
packer!(pack_7, 28, 7);
packer!(pack_9, 36, 9);
packer!(pack_10, 40, 10);
packer!(pack_16, 64, 16);

/// `0x90`, the one-byte `nop`. Programs are padded up to a word boundary with it rather than with
/// zero, because `00 00` is `add [rax], al` and would fault rather than fall off the end quietly.
const NOP: u8 = 0x90;

/// **A one-instruction runaway**: `jmp .`, forever.
///
/// It never yields, never syscalls and never touches a rendezvous, so nothing cooperative can end
/// it and the forcible tier (DECISIONS §16) is the only thing that can. The twin of aarch64's
/// `b .` and RISC-V's `j .`.
///
/// ```text
///   eb fe        jmp $-2          (to itself)
/// ```
pub const SPIN: &[u32] = &pack_1([0xEB, 0xFE, NOP, NOP]);

/// **A child that faults on its very first memory access**: load from `addr`, which nothing maps.
///
/// The faulting instruction is the second, so the reported pc is the entry plus five (the length of
/// the `mov` that precedes it). That offset is x86's own and differs from the other two ports'
/// four, which is why a test asserting on the faulting pc has to ask this rather than add a
/// constant.
///
/// ```text
///   b8 xx xx xx xx    mov eax, addr    (32-bit form: writing eax zeroes the top half of rax)
///   48 8b 18          mov rbx, [rax]   (page fault: nothing maps addr)
/// ```
pub const fn fault(addr: u32) -> [u32; 2] {
    let a = addr.to_le_bytes();
    pack_2([0xB8, a[0], a[1], a[2], a[3], 0x48, 0x8B, 0x18])
}

/// How far past the entry the faulting instruction in [`fault`] begins. See its doc comment.
pub const FAULT_PC_OFFSET: u64 = 5;

/// **A child that SENDs one word on the endpoint capability in slot 0, then exits cleanly.**
///
/// The x86 twin of the nine-instruction `REPORT_STUB` both other ports carry, and it is nine
/// instructions here too. "It ran" is the SEND arriving; "it finished" is the exit.
///
/// ```text
///   31 ff             xor edi, edi      (arg0: slot 0)
///   31 f6             xor esi, esi      (arg1: rendezvous::SEND, which is 0)
///   ba xx xx xx xx    mov edx, word     (arg2: the word to send)
///   45 31 d2          xor r10d, r10d    (arg3: the second message word)
///   45 31 c0          xor r8d, r8d      (arg4: the third)
///   b8 xx xx xx xx    mov eax, SYS_INVOKE
///   0f 05             syscall           (SEND)
///   b8 xx xx xx xx    mov eax, SYS_EXIT
///   0f 05             syscall           (exit)
/// ```
///
/// # Panics
/// At compile time, through the `assert!`s below, if `rendezvous::SEND` is ever given a nonzero
/// value: the `xor esi, esi` above encodes it as a zero and would then be sending under the wrong
/// method with nothing to say so.
pub const fn report(word: u32) -> [u32; 8] {
    const {
        assert!(
            abi::rendezvous::SEND == 0,
            "the `xor esi, esi` in this program encodes SEND as zero"
        );
    }
    let w = word.to_le_bytes();
    let inv = (abi::SYS_INVOKE as u32).to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    pack_8([
        0x31, 0xFF, // xor edi, edi
        0x31, 0xF6, // xor esi, esi
        0xBA, w[0], w[1], w[2], w[3], // mov edx, word
        0x45, 0x31, 0xD2, // xor r10d, r10d
        0x45, 0x31, 0xC0, // xor r8d, r8d
        0xB8, inv[0], inv[1], inv[2], inv[3], // mov eax, SYS_INVOKE
        0x0F, 0x05, // syscall
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall
        NOP, NOP, NOP,
    ])
}

/// **A child that writes one byte to an I/O port, then SENDs a word and exits** (milestone 299).
/// The port-capability twin of [`report`]: it prefixes the report with `out dx, al`, so whether the
/// word arrives is the test of whether ring 3 was allowed the port. A holder of the `PortRange`
/// capability naming `port` reports `word`; a non-holder takes a general protection fault on the
/// `out` and its supervisor sees `EVENT_FAULT` instead of the word.
///
/// The `out` is the third instruction (`mov dx` then `mov al` set up its operands), at
/// [`PORT_OUT_PC_OFFSET`] past the entry. `mov dx, port` is the 16-bit-operand form (`66` prefix); the
/// `report` tail that follows is byte-for-byte [`report`], including its own `mov edx, word`, which
/// harmlessly overwrites `dx` after the `out` has used it.
///
/// ```text
///   66 ba xx xx       mov dx, port     (16-bit immediate)
///   b0 xx             mov al, val
///   ee                out dx, al       (#GP here if this thread holds no PortRange for `port`)
///   ...               <report(word)>   (SEND the word on slot 0, then SYS_EXIT)
/// ```
pub(crate) const fn port_out(port: u16, val: u8, word: u32) -> [u32; 9] {
    const {
        assert!(
            abi::rendezvous::SEND == 0,
            "the `xor esi, esi` in this program encodes SEND as zero"
        );
    }
    let p = port.to_le_bytes();
    let w = word.to_le_bytes();
    let inv = (abi::SYS_INVOKE as u32).to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    pack_9([
        0x66, 0xBA, p[0], p[1], // mov dx, port
        0xB0, val,  // mov al, val
        0xEE, // out dx, al
        0x31, 0xFF, // xor edi, edi      (slot 0)
        0x31, 0xF6, // xor esi, esi      (SEND)
        0xBA, w[0], w[1], w[2], w[3], // mov edx, word
        0x45, 0x31, 0xD2, // xor r10d, r10d
        0x45, 0x31, 0xC0, // xor r8d, r8d
        0xB8, inv[0], inv[1], inv[2], inv[3], // mov eax, SYS_INVOKE
        0x0F, 0x05, // syscall (SEND)
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall (exit)
    ])
}

/// The faulting pc offset for [`port_out`]: `mov dx, port` (`66 BA` + imm16, 4 bytes) and
/// `mov al, val` (`B0` + imm8, 2 bytes) precede the `out`, so a fault on it is reported six bytes
/// past the entry.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const PORT_OUT_PC_OFFSET: u64 = 6;

/// **A child that executes one `out` to `port` and, if the CPU permitted it, exits cleanly.**
///
/// Unlike [`port_out`] it SENDs nothing, so no receiver is needed and neither branch of the
/// outcome can park the child on a rendezvous: a permitted `out` falls straight into `SYS_EXIT`
/// (the supervisor reads `EVENT_EXIT`), and a refused `out` faults at [`PORT_OUT_PC_OFFSET`] (the
/// supervisor reads `EVENT_FAULT`). That is the shape a port claim needs when the escape under
/// test is "the `out` was wrongly permitted": [`port_out`] would hang the run in that case because
/// the child would reach a blocking SEND, which is the row-26 hazard
/// `notes/confinement-claims.md` names.
///
/// ```text
///   66 ba xx xx       mov dx, port
///   b0 xx             mov al, val
///   ee                out dx, al        (offset 6; faults here if the port is denied)
///   b8 xx xx xx xx    mov eax, SYS_EXIT
///   0f 05             syscall           (exit; reached only if the `out` was permitted)
/// ```
///
/// Name: provisional (lane/633-outsider-2, 2026-10-05 UTC). The verb-first spelling mirrors
/// [`port_out_on_cpu_then_exit`]; an architect ratifies the port builders as a set.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const fn port_out_then_exit(port: u16, val: u8) -> [u32; 4] {
    let p = port.to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    pack_4([
        0x66, 0xBA, p[0], p[1], // mov dx, port
        0xB0, val,  // mov al, val
        0xEE, // out dx, al (offset 6)
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall (exit)
        NOP, NOP, // pad 14 -> 16 (4 words)
    ])
}

/// The address of the current-CPU word a ring-3 thread reads its own core from:
/// `current_cpu_contract::CPU_WORD_OFFSET` into that crate's page. It is under 2 GiB
/// (`address_space_map::CODE_MODEL_CEILING`), so the 32-bit absolute form of `mov` below
/// addresses it with no sign-extension surprise.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
const CURRENT_CPU_WORD: u32 =
    (current_cpu_contract::PAGE_VA + current_cpu_contract::CPU_WORD_OFFSET) as u32;

#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
packer!(pack_12, 48, 12);

/// **A port holder that writes one byte, then SENDs `word` and the core it wrote it on**, then
/// exits. [`port_out`] for the hand-off test, which has to know which core's TSS bitmap the
/// holder's grant was installed in: the per-core bitmap is the thing the hand-off is about, and on
/// a machine with more than one core a test that does not know the core is not testing it (the
/// 2026-10-03 finding this program exists for: at `NIFE_SMP=2` the non-holder ran on the other
/// core and faulted for a reason that had nothing to do with the hand-off).
///
/// It reads its core from the current-CPU page (`current_cpu_contract`, written by the context
/// switch on the core doing the switch) **once before the `out` and once after**, and sends both:
/// equal, they bracket the `out` on one core. The message is `[word, core after, core before]`.
/// Not proof against a thread that migrated away and back inside three instructions, which would
/// take two preemptions and two steals; the caller says so rather than this.
///
/// ```text
///   4c 8b 04 25 xx xx xx xx   mov r8, [CURRENT_CPU_WORD]    (core before)
///   66 ba xx xx               mov dx, port
///   b0 xx                     mov al, val
///   ee                        out dx, al
///   4c 8b 14 25 xx xx xx xx   mov r10, [CURRENT_CPU_WORD]   (core after)
///   31 ff                     xor edi, edi                  (slot 0)
///   31 f6                     xor esi, esi                  (SEND)
///   ba xx xx xx xx            mov edx, word
///   b8 xx xx xx xx            mov eax, SYS_INVOKE
///   0f 05                     syscall                       (SEND [word, r10, r8])
///   b8 xx xx xx xx            mov eax, SYS_EXIT
///   0f 05                     syscall                       (exit)
/// ```
/// Encodings checked with `llvm-mc -triple x86_64-unknown-none -show-encoding`, 2026-10-03.
///
/// Name: ratified 2026-10-03 (calef, reviewing the x86 port-out program builders: "The two port builders are ratified.").
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const fn port_out_reporting_cpu(port: u16, val: u8, word: u32) -> [u32; 12] {
    const {
        assert!(
            abi::rendezvous::SEND == 0,
            "the `xor esi, esi` in this program encodes SEND as zero"
        );
    }
    let a = CURRENT_CPU_WORD.to_le_bytes();
    let p = port.to_le_bytes();
    let w = word.to_le_bytes();
    let inv = (abi::SYS_INVOKE as u32).to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    pack_12([
        0x4C, 0x8B, 0x04, 0x25, a[0], a[1], a[2], a[3], // mov r8, [cpu]
        0x66, 0xBA, p[0], p[1], // mov dx, port
        0xB0, val,  // mov al, val
        0xEE, // out dx, al
        0x4C, 0x8B, 0x14, 0x25, a[0], a[1], a[2], a[3], // mov r10, [cpu]
        0x31, 0xFF, // xor edi, edi
        0x31, 0xF6, // xor esi, esi
        0xBA, w[0], w[1], w[2], w[3], // mov edx, word
        0xB8, inv[0], inv[1], inv[2], inv[3], // mov eax, SYS_INVOKE
        0x0F, 0x05, // syscall (SEND)
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall (exit)
        NOP, NOP,
    ])
}

/// **A non-holder that writes one byte to a port only if it is running on core `cpu`**, and
/// otherwise faults somewhere else on purpose (milestone 313 (the security audit that was due since
/// August)'s shape, made core-aware on
/// 2026-10-03). The hand-off test's second child.
///
/// On core `cpu` it is the old `port_out_then_exit`: `out`, then exit, so a wrongly permitted `out`
/// arrives as `EVENT_EXIT` in the message the test wants `EVENT_FAULT` in (a reporting child would
/// park on a `SEND` nobody receives and hang the run instead). On any other core it never reaches
/// the `out`: it loads from address 0, which `address_space_map::NULL_GUARD` keeps unmapped, and
/// the page fault arrives at [`PORT_OUT_ON_CPU_WRONG_CPU_PC_OFFSET`]. The two faults are told apart
/// by pc, so "landed on the wrong core, try again" can never be read as "the `out` faulted".
///
/// ```text
///   48 8b 04 25 xx xx xx xx   mov rax, [CURRENT_CPU_WORD]
///   83 f8 xx                  cmp eax, cpu
///   75 0e                     jne wrong
///   66 ba xx xx               mov dx, port
///   b0 xx                     mov al, val
///   ee                        out dx, al          (#GP here: offset 19)
///   b8 xx xx xx xx            mov eax, SYS_EXIT   (reached only if the `out` was permitted)
///   0f 05                     syscall             (exit)
/// wrong:
///   31 c0                     xor eax, eax
///   48 8b 18                  mov rbx, [rax]      (page fault at 0: offset 29)
/// ```
/// Encodings checked with `llvm-mc -triple x86_64-unknown-none -show-encoding`, 2026-10-03.
///
/// Name: ratified 2026-10-03 (calef, reviewing the x86 port-out program builders: "The two port builders are ratified.").
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const fn port_out_on_cpu_then_exit(port: u16, val: u8, cpu: u8) -> [u32; 8] {
    let a = CURRENT_CPU_WORD.to_le_bytes();
    let p = port.to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    pack_8([
        0x48, 0x8B, 0x04, 0x25, a[0], a[1], a[2], a[3], // mov rax, [cpu]
        0x83, 0xF8, cpu, // cmp eax, cpu
        0x75, 0x0E, // jne wrong (+14, to offset 27)
        0x66, 0xBA, p[0], p[1], // mov dx, port
        0xB0, val,  // mov al, val
        0xEE, // out dx, al
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall (exit)
        0x31, 0xC0, // wrong: xor eax, eax
        0x48, 0x8B, 0x18, // mov rbx, [rax]
    ])
}

/// Where [`port_out_on_cpu_then_exit`]'s `out` is: eight, three and two bytes of core check, then
/// [`port_out`]'s own six bytes before its `out`.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const PORT_OUT_ON_CPU_PC_OFFSET: u64 = 13 + PORT_OUT_PC_OFFSET;

/// Where [`port_out_on_cpu_then_exit`] faults when it is on the wrong core: the load from address 0.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const PORT_OUT_ON_CPU_WRONG_CPU_PC_OFFSET: u64 = 29;

/// **A child that deletes its own capability in `slot`, then writes a byte to a port, then exits**
/// (milestone 313's audit). The drop-it-yourself fixture: a holder of the `PortRange` capability
/// naming `port` executes `SYS_CAP_DELETE` on the slot that capability sits in, and the `out` that
/// follows must take a general protection fault, because the authority is gone by the thread's own
/// hand. Before milestone 313 it did not fault: the port grant is enforced by a cached field the
/// context switch reads, and deleting the capability left the cache alone.
///
/// **It exits rather than reporting, and that is the falsification's shape, not a shortcut.** The
/// first draft ended in [`port_out`]'s `SEND`-then-exit tail, and on the unfixed kernel the run hung
/// rather than going red: the permitted `out` was followed by a `SEND` on a rendezvous the test was
/// not receiving (it was waiting on the supervision endpoint for a death report), so the child
/// parked forever and so did the test. That is `notes/confinement-claims.md`'s row 26 one object
/// over: a real escape that blocks instead of reporting is not evidence. Exiting instead turns the
/// escape into `EVENT_EXIT` on the supervision endpoint, in the same message the test wants
/// `EVENT_FAULT` in, so the assertion that states the claim is the one that fires.
///
/// `SYS_CAP_DELETE` is a plain syscall number, not an `INVOKE` method, so the slot rides in `edi`
/// (arg0) and the number in `eax`; nothing else is read. `dx` is set after the syscall rather than
/// before it, so no register the syscall clobbers matters. The `out` is at
/// [`CAP_DELETE_THEN_PORT_OUT_PC_OFFSET`] past the entry.
///
/// ```text
///   bf xx xx xx xx    mov edi, slot
///   b8 xx xx xx xx    mov eax, SYS_CAP_DELETE
///   0f 05             syscall           (the capability is gone)
///   66 ba xx xx       mov dx, port
///   b0 xx             mov al, val
///   ee                out dx, al        (#GP here: the grant went with the capability)
///   b8 xx xx xx xx    mov eax, SYS_EXIT (reached only if the `out` was permitted)
///   0f 05             syscall           (exit: the supervisor sees EVENT_EXIT, not EVENT_FAULT)
/// ```
pub const fn cap_delete_then_port_out(slot: u32, port: u16, val: u8) -> [u32; 7] {
    let s = slot.to_le_bytes();
    let del = (abi::SYS_CAP_DELETE as u32).to_le_bytes();
    let p = port.to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    pack_7([
        0xBF, s[0], s[1], s[2], s[3], // mov edi, slot
        0xB8, del[0], del[1], del[2], del[3], // mov eax, SYS_CAP_DELETE
        0x0F, 0x05, // syscall (CAP_DELETE)
        0x66, 0xBA, p[0], p[1], // mov dx, port
        0xB0, val,  // mov al, val
        0xEE, // out dx, al
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall (exit)
        NOP, NOP,
    ])
}

/// The faulting pc offset for [`cap_delete_then_port_out`]: two five-byte `mov r32, imm32`s and the
/// two-byte `syscall` (12 bytes), then [`port_out`]'s own six bytes before its `out`.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const CAP_DELETE_THEN_PORT_OUT_PC_OFFSET: u64 = 12 + PORT_OUT_PC_OFFSET;

/// **A child that reads one byte from an I/O port and exits.** [`port_out_then_exit`]'s `in` twin:
/// the read side of the port range, so a READ-only capability's positive control and a non-holder's
/// fault can both be driven through a port the claim names.
///
/// ```text
///   66 ba xx xx       mov dx, port
///   ec                in al, dx         (offset 4; #GP here if the port is denied)
///   b8 xx xx xx xx    mov eax, SYS_EXIT
///   0f 05             syscall           (reached only if the `in` was permitted)
/// ```
///
/// Name: provisional (the eighth outsider pass's lane, 2026-10-13 UTC).
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const fn port_in_then_exit(port: u16) -> [u32; 4] {
    let p = port.to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    pack_4([
        0x66, 0xBA, p[0], p[1], // mov dx, port
        0xEC, // in al, dx (offset 4)
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall (exit)
        NOP, NOP, NOP, NOP, // pad 12 -> 16 (4 words)
    ])
}

/// The faulting pc offset for [`port_in_then_exit`]'s `in`: four bytes of `mov dx, port` precede it.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const PORT_IN_ONLY_PC_OFFSET: u64 = 4;

/// **A child that reads one byte from a port and then writes one to it, and exits only if both were
/// permitted.** The READ-only capability's probe: with the read granted and the write refused, the
/// fault lands on the `out` (offset [`PORT_IN_THEN_OUT_OUT_PC_OFFSET`]), which is itself the proof
/// the `in` at offset 4 succeeded; a capability that was dead or wrongly denied for reads would
/// fault four bytes earlier and say so.
///
/// ```text
///   66 ba xx xx       mov dx, port
///   ec                in al, dx         (offset 4)
///   b0 xx             mov al, val
///   ee                out dx, al        (offset 7; #GP here if the write is denied)
///   b8 xx xx xx xx    mov eax, SYS_EXIT
///   0f 05             syscall           (reached only if both were permitted)
/// ```
///
/// Name: provisional (the eighth outsider pass's lane, 2026-10-13 UTC).
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const fn port_in_then_out(port: u16, val: u8) -> [u32; 4] {
    let p = port.to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    pack_4([
        0x66, 0xBA, p[0], p[1], // mov dx, port
        0xEC, // in al, dx (offset 4)
        0xB0, val,  // mov al, val
        0xEE, // out dx, al (offset 7)
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall (exit)
        NOP,  // pad 15 -> 16 (4 words)
    ])
}

/// The faulting pc offset for [`port_in_then_out`]'s `out`: the four-byte `mov dx, port`, the
/// one-byte `in`, and the two-byte `mov al, val` precede it.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub(crate) const PORT_IN_THEN_OUT_OUT_PC_OFFSET: u64 = 7;

/// **A child that deletes two capability slots, then writes one byte to a port, then exits.**
/// [`cap_delete_then_port_out`]'s two-slot twin, for a child holding the same port range in two
/// slots: deleting one must leave the `out` permitted (the other slot's grant stands), and deleting
/// both must revoke it. The `out` is at [`TWO_CAPABILITY_DELETES_THEN_PORT_OUT_PC_OFFSET`].
///
/// ```text
///   bf xx xx xx xx    mov edi, slot1
///   b8 xx xx xx xx    mov eax, SYS_CAP_DELETE
///   0f 05             syscall
///   bf xx xx xx xx    mov edi, slot2
///   b8 xx xx xx xx    mov eax, SYS_CAP_DELETE
///   0f 05             syscall
///   66 ba xx xx       mov dx, port
///   b0 xx             mov al, val
///   ee                out dx, al        (offset 30; #GP here if every holder of the range is gone)
///   b8 xx xx xx xx    mov eax, SYS_EXIT (reached only if the `out` was permitted)
///   0f 05             syscall
/// ```
///
/// Name: provisional (the eighth outsider pass's lane, 2026-10-13 UTC).
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const fn two_capability_deletes_then_port_out(
    slot1: u32,
    slot2: u32,
    port: u16,
    val: u8,
) -> [u32; 10] {
    let a = slot1.to_le_bytes();
    let b = slot2.to_le_bytes();
    let del = (abi::SYS_CAP_DELETE as u32).to_le_bytes();
    let p = port.to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    pack_10([
        0xBF, a[0], a[1], a[2], a[3], // mov edi, slot1
        0xB8, del[0], del[1], del[2], del[3], // mov eax, SYS_CAP_DELETE
        0x0F, 0x05, // syscall (CAP_DELETE)
        0xBF, b[0], b[1], b[2], b[3], // mov edi, slot2
        0xB8, del[0], del[1], del[2], del[3], // mov eax, SYS_CAP_DELETE
        0x0F, 0x05, // syscall (CAP_DELETE)
        0x66, 0xBA, p[0], p[1], // mov dx, port
        0xB0, val,  // mov al, val
        0xEE, // out dx, al (offset 30)
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall (exit)
        NOP, NOP, // pad 38 -> 40 (10 words)
    ])
}

/// The faulting pc offset for [`two_capability_deletes_then_port_out`]'s `out`: two delete sequences (12
/// bytes each) then [`port_out`]'s six bytes.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const TWO_CAPABILITY_DELETES_THEN_PORT_OUT_PC_OFFSET: u64 = 24 + PORT_OUT_PC_OFFSET;

/// The faulting pc offset for [`receive_then_port_out`]'s `out`: the RECEIVE invocation (25 bytes:
/// two `mov r32, imm32`s, three operand setups and the `syscall`) then [`port_out`]'s six bytes.
#[cfg(all(any(test, feature = "system_tests"), target_arch = "x86_64"))]
pub const RECEIVE_THEN_PORT_OUT_PC_OFFSET: u64 = 25 + PORT_OUT_PC_OFFSET;

/// **A child that blocks in RECEIVE on slot 1, then writes a byte to a port, then exits**
/// (milestone 299). The revocation fixture: a holder of the `PortRange` capability naming `port`
/// parks in RECEIVE, and while it is parked the test revokes the range and wakes it; the `out` it then
/// executes takes a general protection fault, so its supervisor sees `EVENT_FAULT` and the `word`
/// never arrives. The RECEIVE is what lets the test choose the instant between "holds the port" and
/// "does the `out`" to revoke, so the fault is the revocation's doing and not a race.
///
/// `RECEIVE` reuses [`invoke_then_exit`]'s exact encoding through slot 1; the `out` and `report` tail
/// are [`port_out`]'s. The `out` is the instruction after the RECEIVE `syscall`, at
/// [`RECEIVE_THEN_PORT_OUT_PC_OFFSET`] past the entry.
///
/// ```text
///   bf 01 00 00 00    mov edi, 1        (slot 1)
///   be xx xx xx xx    mov esi, RECEIVE
///   31 d2             xor edx, edx
///   45 31 d2          xor r10d, r10d
///   45 31 c0          xor r8d, r8d
///   b8 xx xx xx xx    mov eax, SYS_INVOKE
///   0f 05             syscall           (RECEIVE: blocks until the test wakes it)
///   66 ba xx xx       mov dx, port
///   b0 xx             mov al, val
///   ee                out dx, al        (#GP here once the port has been revoked)
///   b8 xx xx xx xx    mov eax, SYS_EXIT (reached only if the revoke did not take)
///   0f 05             syscall           (exit)
/// ```
///
/// **It exits after the `out` rather than reporting** (milestone 313's audit; it carried
/// [`port_out`]'s `SEND`-then-exit tail until then). The child's `out` is *expected* to fault, so
/// the interesting case is the one where it does not, and in that case a `SEND` on a rendezvous the
/// test is not receiving parks the child forever and hangs the run: a broken revoke could not turn
/// the test red, only stall it. Exiting makes the broken case arrive as `EVENT_EXIT` where the test
/// wants `EVENT_FAULT`, which is what lets the test carry a falsification record at all.
pub const fn receive_then_port_out(port: u16, val: u8) -> [u32; 10] {
    let p = port.to_le_bytes();
    let rcv = (abi::rendezvous::RECEIVE as u32).to_le_bytes();
    let inv = (abi::SYS_INVOKE as u32).to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    pack_10([
        0xBF, 0x01, 0x00, 0x00, 0x00, // mov edi, 1        (slot 1)
        0xBE, rcv[0], rcv[1], rcv[2], rcv[3], // mov esi, RECEIVE
        0x31, 0xD2, // xor edx, edx
        0x45, 0x31, 0xD2, // xor r10d, r10d
        0x45, 0x31, 0xC0, // xor r8d, r8d
        0xB8, inv[0], inv[1], inv[2], inv[3], // mov eax, SYS_INVOKE
        0x0F, 0x05, // syscall (RECEIVE)
        0x66, 0xBA, p[0], p[1], // mov dx, port
        0xB0, val,  // mov al, val
        0xEE, // out dx, al
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall (exit)
        NOP,  // pad 39 -> 40 (10 words)
    ])
}

/// **A child that blocks in RECEIVE on the endpoint capability in slot 0, forever.**
///
/// A server is a thing that blocks, and DECISIONS §16's kill is armed by a refusal and spent by
/// `schedule()`, which a thread parked in RECEIVE never reaches. That is the whole point of this
/// program: it is the shape of resident whose region used to be unreclaimable.
///
/// ```text
///   31 ff             xor edi, edi      (arg0: slot 0)
///   be xx xx xx xx    mov esi, RECEIVE     (arg1: the method)
///   31 d2             xor edx, edx      (arg2)
///   45 31 d2          xor r10d, r10d    (arg3)
///   45 31 c0          xor r8d, r8d      (arg4)
///   b8 xx xx xx xx    mov eax, SYS_INVOKE
///   0f 05             syscall           (RECEIVE: blocks)
///   b8 xx xx xx xx    mov eax, SYS_EXIT
///   0f 05             syscall           (exit, if it is ever woken)
/// ```
pub const fn receive() -> [u32; 8] {
    invoke_then_exit(abi::rendezvous::RECEIVE as u32)
}

/// **A child that blocks in `CALL` on the endpoint capability in slot 0, and is never replied to.**
///
/// Milestone 133's second shape, and the harder one. A `CALL` caller whose request a server has
/// collected sits on **no queue at all**: it left the sender queue at the rendezvous and the one
/// thing that can wake it is the one-shot `Reply` capability the server now holds. So it is the
/// resident that no endpoint sweep can reach, and the one whose teardown has to sweep that `Reply`
/// out of the server's capability table or leave a forgeable answer behind.
///
/// Byte for byte [`receive`] with a different method word; see it for the encoding.
pub const fn call() -> [u32; 8] {
    invoke_then_exit(abi::rendezvous::CALL as u32)
}

/// The body [`receive`] and [`call`] share: `invoke(slot 0, method, 0, 0, 0)`, then `SYS_EXIT` if the
/// invocation ever returns. Written once because the two differ in exactly one immediate, and two
/// copies of an eight-instruction hand-assembled program are two chances to typo a syscall number.
///
/// ```text
///   31 ff             xor edi, edi      (arg0: slot 0)
///   be xx xx xx xx    mov esi, method   (arg1)
///   31 d2             xor edx, edx      (arg2)
///   45 31 d2          xor r10d, r10d    (arg3)
///   45 31 c0          xor r8d, r8d      (arg4)
///   b8 xx xx xx xx    mov eax, SYS_INVOKE
///   0f 05             syscall           (blocks)
///   b8 xx xx xx xx    mov eax, SYS_EXIT
///   0f 05             syscall           (exit, if it is ever woken)
/// ```
const fn invoke_then_exit(method: u32) -> [u32; 8] {
    let m = method.to_le_bytes();
    let inv = (abi::SYS_INVOKE as u32).to_le_bytes();
    let ext = (abi::SYS_EXIT as u32).to_le_bytes();
    pack_8([
        0x31, 0xFF, // xor edi, edi
        0xBE, m[0], m[1], m[2], m[3], // mov esi, method
        0x31, 0xD2, // xor edx, edx
        0x45, 0x31, 0xD2, // xor r10d, r10d
        0x45, 0x31, 0xC0, // xor r8d, r8d
        0xB8, inv[0], inv[1], inv[2], inv[3], // mov eax, SYS_INVOKE
        0x0F, 0x05, // syscall
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall
        NOP, NOP, NOP,
    ])
}
