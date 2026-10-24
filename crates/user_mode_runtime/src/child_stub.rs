//! **The smallest whole program: `SEND` one word on slot 0, then exit**, hand-assembled per
//! architecture, for a parent that builds a child from EL0 through the granular verbs and needs
//! the child to run something.
//!
//! A parent copies [`SEND_THEN_EXIT`] into a frame it maps writable in its own space, then aliases
//! that frame as `MAP_CODE` into each child; the kernel makes the I-cache coherent at that map. One
//! copy, because two hand-assembled copies of three instruction sets would drift (AGENTS.md rule 7).
//!
//! Name: provisional (milestone 168's lane, 2026-09-19); the module and the constant are both
//! unratified. `child_stub` is what `os_primitives_benchmarker` already called it in prose.

/// The child's whole program, hand-assembled: `SEND(slot 0, rendezvous::SEND, 1)` then `SYS_EXIT`.
/// Its slot 0 is the `CHILD_DONE` endpoint the spawner inserts. Nine instructions: `SEND` the done
/// word (1) on the slot-0 endpoint, then `EXIT`. Every child runs this identical code, which is why
/// the spawner writes it into one shared frame and aliases that frame into each child. Hand-assembled
/// machine code, arch-gated: aarch64 `movz`/`svc`, RISC-V `li`/`ecall`.
///
/// aarch64: x0=slot 0, x1=SEND, x2=1, `x8=SYS_INVOKE`, svc; then `x8=SYS_EXIT`, svc.
#[cfg(target_arch = "aarch64")]
pub const SEND_THEN_EXIT: [u32; 9] = [
    0xD280_0000,
    0xD280_0001,
    0xD280_0000 | (1u32 << 5) | 2, // movz x2, #1  (the done word)
    0xD280_0003,
    0xD280_0004,
    0xD280_0000 | ((abi::SYS_INVOKE as u32) << 5) | 8,
    0xD400_0001,
    0xD280_0008,
    0xD400_0001,
];

/// RISC-V: a0=slot 0, a1=SEND, a2=1, a3=a4=0, a7=SYS_INVOKE, ecall; then a7=SYS_EXIT, ecall.
/// Each `li aN, imm` is `addi aN, x0, imm` = `(imm << 20) | (reg << 7) | 0x13`; `ecall` is 0x73.
#[cfg(target_arch = "riscv64")]
pub const SEND_THEN_EXIT: [u32; 9] = [
    0x0000_0513,                                          // li a0, 0            (slot 0)
    0x0000_0593 | ((abi::rendezvous::SEND as u32) << 20), // li a1, SEND         (method)
    0x0010_0613,                                          // li a2, 1            (the done word)
    0x0000_0693,                                          // li a3, 0
    0x0000_0713,                                          // li a4, 0
    0x0000_0893 | ((abi::SYS_INVOKE as u32) << 20),       // li a7, SYS_INVOKE
    0x0000_0073,                                          // ecall               (SEND)
    0x0000_0893 | ((abi::SYS_EXIT as u32) << 20),         // li a7, SYS_EXIT
    0x0000_0073,                                          // ecall               (EXIT)
];

/// `x86_64`: rdi=slot 0, rsi=SEND, rdx=1, r10=0, r8=0, rax=`SYS_INVOKE`, syscall; then rax=`SYS_EXIT`,
/// rdi=0, syscall (DECISIONS §124).
///
/// Bytes rather than words, because x86 instructions are not a fixed width. Each immediate uses
/// `mov r32, imm32`, which zeroes the upper half and keeps a small constant to five bytes; `r10`
/// and `r8` need a REX prefix (`0x41`), so those rows are six. See notes/user-mode-runtime.md.
#[cfg(target_arch = "x86_64")]
pub const SEND_THEN_EXIT: [u8; 46] = {
    const SEND_METHOD: u32 = abi::rendezvous::SEND as u32;
    const INVOKE_NR: u32 = abi::SYS_INVOKE as u32;
    const EXIT_NR: u32 = abi::SYS_EXIT as u32;
    /// The little-endian bytes of a 32-bit immediate, so each row below reads as one instruction
    /// rather than as four hand-computed constants.
    const fn imm(v: u32) -> [u8; 4] {
        v.to_le_bytes()
    }
    let send = imm(SEND_METHOD);
    let invoke = imm(INVOKE_NR);
    let exit = imm(EXIT_NR);
    [
        0xBF, 0x00, 0x00, 0x00, 0x00, // mov edi, 0          (slot 0)
        0xBE, send[0], send[1], send[2], send[3], // mov esi, SEND       (method)
        0xBA, 0x01, 0x00, 0x00, 0x00, // mov edx, 1          (the done word)
        0x41, 0xBA, 0x00, 0x00, 0x00, 0x00, // mov r10d, 0
        0x41, 0xB8, 0x00, 0x00, 0x00, 0x00, // mov r8d, 0
        0xB8, invoke[0], invoke[1], invoke[2], invoke[3], // mov eax, SYS_INVOKE
        0x0F, 0x05, // syscall             (SEND)
        0xB8, exit[0], exit[1], exit[2], exit[3], // mov eax, SYS_EXIT
        0xBF, 0x00, 0x00, 0x00, 0x00, // mov edi, 0          (the exit status)
        0x0F, 0x05, // syscall             (EXIT)
    ]
};
