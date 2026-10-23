//! **Hand-assembled `x86_64` user programs**: the tests' fixtures and the boot tour's ring-3 process.
//!
//! `x86_64` has no compiled user program yet, so each is a byte listing packed into the `&[u32]`
//! words `sched`'s child builders take. They speak §124 (ratify the x86_64 syscall ABI): `rax`
//! the number, `rdi` to `r9` the arguments. See notes/x86-user-programs.md.
//!
//! # BUGS
//!
//! - Every immediate is 32 bits wide, so a word or syscall number above `u32::MAX` would truncate.
//! - No program checks a syscall's return value; the caller asserts on the kernel's side.

/// Pack a byte listing into the little-endian words the child builders take
/// (notes/x86-user-programs.md has why the type is `u32`).
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

/// **A child that faults on its very first memory access**: a load from `addr`, which nothing maps.
///
/// The fault is reported at the entry plus [`FAULT_PC_OFFSET`], which is x86's own instruction
/// length and not the other ports' four, so a test asks for it rather than adding a constant.
pub const fn fault(addr: u32) -> [u32; 2] {
    let a = addr.to_le_bytes();
    pack_2([0xB8, a[0], a[1], a[2], a[3], 0x48, 0x8B, 0x18]) // mov eax, addr; mov rbx, [rax]
}

/// How far past the entry the faulting instruction in [`fault`] begins. See its doc comment.
pub const FAULT_PC_OFFSET: u64 = 5;

/// **A child that SENDs `word` on the endpoint capability in slot 0, then exits cleanly.**
///
/// The x86 twin of the other ports' nine-instruction `REPORT_STUB`: the SEND arriving says it ran,
/// and the exit says it finished.
///
/// # Panics
/// At compile time if `rendezvous::SEND` is ever nonzero, because the `xor esi, esi` encodes it as
/// zero.
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
        0x31, 0xFF, // xor edi, edi      (arg0: slot 0)
        0x31, 0xF6, // xor esi, esi      (arg1: SEND, which is 0)
        0xBA, w[0], w[1], w[2], w[3], // mov edx, word
        0x45, 0x31, 0xD2, // xor r10d, r10d    (the second message word)
        0x45, 0x31, 0xC0, // xor r8d, r8d      (the third)
        0xB8, inv[0], inv[1], inv[2], inv[3], // mov eax, SYS_INVOKE
        0x0F, 0x05, // syscall
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall
        NOP, NOP, NOP,
    ])
}

/// **A child that writes one byte to an I/O port, then SENDs a word and exits** (milestone 299).
///
/// [`report`] with `out dx, al` in front. A holder of the `PortRange` capability naming `port`
/// reports `word`; a non-holder takes a general protection fault on the `out`, at
/// [`PORT_OUT_PC_OFFSET`], and its supervisor sees `EVENT_FAULT`. The tail's own `mov edx, word`
/// overwrites `dx` after the `out` has used it.
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
        0x66, 0xBA, p[0], p[1], // mov dx, port      (16-bit immediate)
        0xB0, val,  // mov al, val
        0xEE, // out dx, al        (#GP here without a PortRange for `port`)
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
/// It SENDs nothing, so neither outcome can park it on a rendezvous: a permitted `out` exits
/// (`EVENT_EXIT`), and a refused one faults at [`PORT_OUT_PC_OFFSET`] (`EVENT_FAULT`). A port claim
/// whose escape is "the `out` was wrongly permitted" needs this shape; see notes/x86-user-programs.md.
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
/// exits.
///
/// It reads its core from the current-CPU page once before the `out` and once after, and sends
/// `[word, core after, core before]`; equal, they bracket the `out` on one core. The hand-off test
/// needs the core because the port grant lives in a per-core TSS bitmap. A thread that migrates
/// away and back between the two reads is not caught; see notes/x86-user-programs.md.
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
/// otherwise faults somewhere else on purpose: the hand-off test's second child.
///
/// On core `cpu` it is [`port_out_then_exit`], so a wrongly permitted `out` arrives as `EVENT_EXIT`.
/// On any other core it loads from address 0, which `address_space_map::NULL_GUARD` keeps unmapped,
/// and faults at [`PORT_OUT_ON_CPU_WRONG_CPU_PC_OFFSET`]. The pc tells "landed on the wrong core"
/// apart from "the `out` faulted".
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
        0xEE, // out dx, al          (#GP here: offset 19)
        0xB8, ext[0], ext[1], ext[2], ext[3], // mov eax, SYS_EXIT
        0x0F, 0x05, // syscall (exit)
        0x31, 0xC0, // wrong: xor eax, eax
        0x48, 0x8B, 0x18, // mov rbx, [rax]      (page fault at 0: offset 29)
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
/// (milestone 313 (the security audit that was due since August)'s audit).
///
/// A holder of the `PortRange` capability naming `port` runs `SYS_CAP_DELETE` on its slot, a plain
/// syscall with the slot in `edi`, and the `out` that follows must take a general protection fault
/// at [`CAP_DELETE_THEN_PORT_OUT_PC_OFFSET`]. It exits rather than reporting, so a grant that
/// survived the delete arrives as `EVENT_EXIT` instead of hanging on a SEND nobody receives.
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

/// **A child that reads one byte from an I/O port and exits**: [`port_out_then_exit`]'s `in` twin.
///
/// A READ-only capability's positive control, and a non-holder's fault at
/// [`PORT_IN_ONLY_PC_OFFSET`].
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
/// permitted.**
///
/// The READ-only capability's probe. With the read granted and the write refused, the fault lands on
/// the `out` at [`PORT_IN_THEN_OUT_OUT_PC_OFFSET`], which proves the `in` at offset 4 succeeded; a
/// capability wrongly denied for reads faults at offset 4 instead.
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
///
/// [`cap_delete_then_port_out`] for a child holding one port range in two slots: deleting one must
/// leave the `out` permitted, and deleting both must revoke it. The `out` is at
/// [`TWO_CAPABILITY_DELETES_THEN_PORT_OUT_PC_OFFSET`].
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
/// (milestone 299 (the x86 port-range capability)).
///
/// The revocation fixture. The test revokes the port range while the child is parked and then wakes
/// it, so the `out` takes a general protection fault at [`RECEIVE_THEN_PORT_OUT_PC_OFFSET`] and the
/// revocation, not a race, is what faulted. It exits rather than reporting, so a revoke that did not
/// take arrives as `EVENT_EXIT` instead of hanging; see notes/x86-user-programs.md.
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
/// A thread parked in RECEIVE never reaches `schedule()`, where the armed kill of DECISIONS §16
/// (object revocation) is spent, so this is the resident only a region's teardown can end.
pub const fn receive() -> [u32; 8] {
    invoke_then_exit(abi::rendezvous::RECEIVE as u32)
}

/// **A child that blocks in `CALL` on the endpoint capability in slot 0, and is never replied to.**
///
/// The second shape of milestone 133 (ending a permanently blocked thread). A caller whose request
/// a server has collected is on no queue, and only the one-shot `Reply` the server holds can wake
/// it, so its teardown has to sweep that `Reply` out of the server's table. [`receive`] with a
/// different method word.
pub const fn call() -> [u32; 8] {
    invoke_then_exit(abi::rendezvous::CALL as u32)
}

/// The body [`receive`] and [`call`] share: `invoke(slot 0, method, 0, 0, 0)`, then `SYS_EXIT` if
/// the invocation ever returns. One copy, so a syscall number has one place to be typed wrong.
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
