//! **The progenitor's one way to speak before its console exists**: polled writes to the UART the
//! kernel granted it, for a refusal that stops the boot before the `console` server is built. The
//! `console` server is measured against the very table a refusal here doubts, so it cannot carry
//! the line. The kernel configured the UART at boot; this only transmits, the way
//! `components/src/console.rs`'s `uart_put` does on each architecture.
//!
//! Best effort, and silent when it cannot write: a slot 28 or slot 1 the kernel did not grant, or a
//! mapping it refuses, leaves the trap that follows as the only report.
//!
//! Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability, for
//! the module and the file.

use super::*;

/// How many polls of the UART's transmit status go by before a byte is given up. A 16-byte FIFO at
/// 115200 baud drains in under 1.5 ms; a UART that never empties is a wrong register shape, and
/// the trap that follows a refusal must not wait on it.
const POLLS_PER_BYTE: u32 = 1 << 20;

/// Where the progenitor maps the UART's registers in its own address space to write a refusal,
/// on aarch64 and riscv64. In the scratch family of [`RNG_DMA_PEEK_VA`]; never given up, because
/// the process traps straight after.
///
/// Name: provisional, minted 2026-11-02 (UTC) by milestone/695-part-1-base-digest-capability.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
const REFUSAL_UART_VIRTUAL_ADDRESS: u64 = address_space_map::pair_page(0x0f70_0000);

/// **Write `parts`, in order, to the UART.** `uart_shape` is the register shape the kernel passed
/// at start (`abi::uart`), which riscv64's NS16550 needs. Returns once every byte is written or
/// given up; never fails.
pub(super) fn say(g: &BootEndowment, uart_shape: u64, parts: &[&str]) {
    let Some(uart) = Uart::open(g, uart_shape) else {
        return;
    };
    for part in parts {
        for &byte in part.as_bytes() {
            uart.put(byte);
        }
    }
}

/// The granted UART, ready to transmit.
struct Uart {
    #[cfg(target_arch = "riscv64")]
    shape: abi::uart::RegisterShape,
}

#[cfg(target_arch = "aarch64")]
impl Uart {
    /// PL011 registers: data at 0x00, flags at 0x18, and the flag that says the FIFO is full.
    const DR: u64 = 0x00;
    const FR: u64 = 0x18;
    const FR_TXFF: u32 = 1 << 5;

    /// Map the PL011's register page into this process; `None` if the kernel refuses.
    fn open(g: &BootEndowment, _uart_shape: u64) -> Option<Self> {
        (address_space_map_into(
            g.own_space,
            REFUSAL_UART_VIRTUAL_ADDRESS,
            g.uart_dev,
            abi::address_space::MAP_RW,
        ) == 0)
            .then_some(Uart {})
    }

    fn put(&self, byte: u8) {
        for _ in 0..POLLS_PER_BYTE {
            // SAFETY: REFUSAL_UART_VIRTUAL_ADDRESS is this process's device mapping of the PL011, made in
            // `open`, and FR is inside its one page and 4-byte aligned.
            let flags = unsafe {
                core::ptr::read_volatile((REFUSAL_UART_VIRTUAL_ADDRESS + Self::FR) as *const u32)
            };
            if flags & Self::FR_TXFF == 0 {
                // SAFETY: as above, for DR.
                unsafe {
                    core::ptr::write_volatile(
                        (REFUSAL_UART_VIRTUAL_ADDRESS + Self::DR) as *mut u32,
                        byte as u32,
                    );
                }
                return;
            }
            core::hint::spin_loop();
        }
    }
}

#[cfg(target_arch = "riscv64")]
impl Uart {
    /// NS16550 register indexes, before the shape's stride: transmit holding and line status, and
    /// the status bit that says the holding register is empty.
    const THR: u64 = 0;
    const LSR: u64 = 5;
    const LSR_THRE: u8 = 1 << 5;

    /// Map the NS16550's register page into this process; `None` if the kernel refuses.
    fn open(g: &BootEndowment, uart_shape: u64) -> Option<Self> {
        (address_space_map_into(
            g.own_space,
            REFUSAL_UART_VIRTUAL_ADDRESS,
            g.uart_dev,
            abi::address_space::MAP_RW,
        ) == 0)
            .then_some(Uart {
                shape: abi::uart::RegisterShape::from_word(uart_shape),
            })
    }

    fn put(&self, byte: u8) {
        for _ in 0..POLLS_PER_BYTE {
            if self.read(Self::LSR) & Self::LSR_THRE != 0 {
                self.write(Self::THR, byte);
                return;
            }
            core::hint::spin_loop();
        }
    }

    fn read(&self, index: u64) -> u8 {
        let addr = REFUSAL_UART_VIRTUAL_ADDRESS + self.shape.offset(index);
        // SAFETY: REFUSAL_UART_VIRTUAL_ADDRESS is this process's device mapping of the NS16550, made in `open`,
        // and the largest offset `RegisterShape` allows stays inside that page. The 32-bit arm is
        // taken only when the stride keeps every register 4-byte aligned.
        unsafe {
            if self.shape.is_word_access() {
                core::ptr::read_volatile(addr as *const u32) as u8
            } else {
                core::ptr::read_volatile(addr as *const u8)
            }
        }
    }

    fn write(&self, index: u64, value: u8) {
        let addr = REFUSAL_UART_VIRTUAL_ADDRESS + self.shape.offset(index);
        // SAFETY: as `read`.
        unsafe {
            if self.shape.is_word_access() {
                core::ptr::write_volatile(addr as *mut u32, value as u32);
            } else {
                core::ptr::write_volatile(addr as *mut u8, value);
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
impl Uart {
    /// COM1's 16550: transmit holding register at the base port, line status at base + 5, and the
    /// status bit that says the holding register is empty. The kernel granted this process the
    /// `PortRange` over these eight ports, so the TSS I/O bitmap lets it reach them.
    const THR: u16 = 0x3F8;
    const LSR: u16 = 0x3FD;
    const LSR_THRE: u8 = 1 << 5;

    /// Nothing to map: the ports are reachable while the `PortRange` in slot 1 is held.
    fn open(g: &BootEndowment, _uart_shape: u64) -> Option<Self> {
        is_granted(g.uart_dev).then_some(Uart {})
    }

    fn put(&self, byte: u8) {
        for _ in 0..POLLS_PER_BYTE {
            if user_mode_runtime::inb(Self::LSR) & Self::LSR_THRE != 0 {
                user_mode_runtime::outb(Self::THR, byte);
                return;
            }
            core::hint::spin_loop();
        }
    }
}
