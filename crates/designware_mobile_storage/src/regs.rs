//! **The controller's register map**, adapted from OpenBSD's `sys/dev/fdt/dwmmc.c` (the crate root
//! carries its notice) and cross-checked against `dwmmc-host` 0.4.2's `regs.rs` (Apache-2.0), which
//! keeps the databook's names. Offsets are bytes from the controller's base.
//!
//! Only the first page matters to this driver: every register it touches, and the data FIFO, sit
//! below `0x1000`. The JH7110's window is `0x1_0000`, the rest of it unused by anything here.

/// Control: the three self-clearing resets, the interrupt and DMA enables.
pub const CTRL: u32 = 0x000;
/// Reset the controller's state machines. Self-clearing.
pub(crate) const CTRL_CONTROLLER_RESET: u32 = 1 << 0;
/// Reset the data FIFO. Self-clearing.
pub(crate) const CTRL_FIFO_RESET: u32 = 1 << 1;
/// Reset the DMA interface. Self-clearing.
pub(crate) const CTRL_DMA_RESET: u32 = 1 << 2;
/// All three resets, which is how bring-up uses them.
pub(crate) const CTRL_ALL_RESET: u32 = CTRL_CONTROLLER_RESET | CTRL_FIFO_RESET | CTRL_DMA_RESET;
/// The global interrupt enable. This driver polls, so it stays clear: an interrupt nobody routes
/// is a line held asserted at the PLIC for no reader.
pub(crate) const CTRL_INT_ENABLE: u32 = 1 << 4;
/// Hand the FIFO to a DMA engine instead of the CPU.
pub(crate) const CTRL_DMA_ENABLE: u32 = 1 << 5;
/// Use the controller's own descriptor-walking DMA (IDMAC) rather than an external one.
pub(crate) const CTRL_USE_INTERNAL_DMAC: u32 = 1 << 25;

/// Power enable, one bit per card slot. Slot 0 is the only one on the JH7110.
pub(crate) const PWREN: u32 = 0x004;
/// Clock divider 0 in bits 7:0: card clock is `base / (2 * div)`, or `base` itself when 0.
pub(crate) const CLKDIV: u32 = 0x008;
/// Which divider each card uses. Always 0 here.
pub(crate) const CLKSRC: u32 = 0x00c;
/// Card clock enable (bit 0) and low-power gating (bit 16), per slot.
pub(crate) const CLKENA: u32 = 0x010;
/// Card clock on, slot 0.
pub(crate) const CLKENA_ENABLE: u32 = 1 << 0;
/// Stop the card clock while the bus is idle. Memory cards only; an SDIO card needs the clock to
/// signal its interrupt.
pub(crate) const CLKENA_LOW_POWER: u32 = 1 << 16;
/// Response timeout in bits 7:0 (card clocks), data read timeout in bits 31:8.
pub(crate) const TMOUT: u32 = 0x014;
/// Bus width per slot: bit 0 for 4-bit, bit 16 for 8-bit, neither for 1-bit.
pub(crate) const CTYPE: u32 = 0x018;
/// 4-bit bus, slot 0.
pub(crate) const CTYPE_4BIT: u32 = 1 << 0;
/// 8-bit bus, slot 0.
pub(crate) const CTYPE_8BIT: u32 = 1 << 16;
/// Block size in bytes for the next data phase.
pub(crate) const BLKSIZ: u32 = 0x01c;
/// Total bytes in the next data phase.
pub(crate) const BYTCNT: u32 = 0x020;
/// Which raw interrupt bits reach the interrupt line. Zero: this driver polls `RINTSTS`.
pub(crate) const INTMASK: u32 = 0x024;
/// The next command's argument.
pub(crate) const CMDARG: u32 = 0x028;
/// The command register; writing it with [`CMD_START`] hands the command to the card interface.
pub(crate) const CMD: u32 = 0x02c;
/// Set by software to issue; the controller clears it when it has taken the command.
pub const CMD_START: u32 = 1 << 31;
/// Drive the command and data lines from the hold register (half a clock later). OpenBSD sets it
/// on every command, on every `SoC` it supports, the JH7110 among them.
pub(crate) const CMD_USE_HOLD_REG: u32 = 1 << 29;
/// Load `CLKDIV`, `CLKSRC` and `CLKENA` into the card clock domain without sending anything.
pub(crate) const CMD_UPDATE_CLOCK_ONLY: u32 = 1 << 21;
/// Send 80 clocks of initialization sequence before this command. CMD0 only.
pub const CMD_SEND_INIT: u32 = 1 << 15;
/// This command stops a data transfer in progress (CMD12).
pub(crate) const CMD_STOP_ABORT: u32 = 1 << 14;
/// Wait for a previous data transfer to finish before sending.
pub(crate) const CMD_WAIT_PREVIOUS_DATA: u32 = 1 << 13;
/// Send CMD12 automatically when the byte count is reached.
pub(crate) const CMD_SEND_AUTO_STOP: u32 = 1 << 12;
/// The data phase is a write to the card.
pub(crate) const CMD_WRITE: u32 = 1 << 10;
/// The command has a data phase.
pub(crate) const CMD_DATA_EXPECTED: u32 = 1 << 9;
/// Check the response's CRC7.
pub(crate) const CMD_CHECK_RESPONSE_CRC: u32 = 1 << 8;
/// The response is 136 bits (R2).
pub(crate) const CMD_LONG_RESPONSE: u32 = 1 << 7;
/// The command has a response.
pub const CMD_RESPONSE_EXPECT: u32 = 1 << 6;

/// Response word 0, bits 31:0 of a short response, or bits 31:0 of a long one.
pub(crate) const RESP0: u32 = 0x030;
/// Response word 1 (long responses).
pub(crate) const RESP1: u32 = 0x034;
/// Response word 2 (long responses).
pub(crate) const RESP2: u32 = 0x038;
/// Response word 3, bits 127:96 of a long response.
pub(crate) const RESP3: u32 = 0x03c;
/// Raw interrupt status, write one to clear. The driver's only completion signal.
pub(crate) const RINTSTS: u32 = 0x044;
/// Response error.
pub(crate) const INT_RE: u32 = 1 << 1;
/// Command done: the controller sent the command and, if one was expected, took the response.
pub(crate) const INT_CD: u32 = 1 << 2;
/// Data transfer over.
pub(crate) const INT_DTO: u32 = 1 << 3;
/// Transmit FIFO data request: room at or below the transmit watermark.
pub(crate) const INT_TXDR: u32 = 1 << 4;
/// Receive FIFO data request: data above the receive watermark.
pub(crate) const INT_RXDR: u32 = 1 << 5;
/// Response CRC error.
pub(crate) const INT_RCRC: u32 = 1 << 6;
/// Data CRC error.
pub(crate) const INT_DCRC: u32 = 1 << 7;
/// Response timeout: no card answered.
pub(crate) const INT_RTO: u32 = 1 << 8;
/// Data read timeout.
pub(crate) const INT_DRTO: u32 = 1 << 9;
/// Data starvation by host timeout: the FIFO was not serviced in time.
pub(crate) const INT_HTO: u32 = 1 << 10;
/// FIFO underrun or overrun.
pub(crate) const INT_FRUN: u32 = 1 << 11;
/// Hardware locked write: a write to `CMD` while a command was still loading.
pub(crate) const INT_HLE: u32 = 1 << 12;
/// Start bit error.
pub(crate) const INT_SBE: u32 = 1 << 13;
/// Auto command done: the automatic CMD12 finished.
pub(crate) const INT_ACD: u32 = 1 << 14;
/// End bit error (read) or no CRC status (write).
pub(crate) const INT_EBE: u32 = 1 << 15;
/// Every bit that means a data phase went wrong, as OpenBSD groups them.
pub(crate) const INT_DATA_ERRORS: u32 = INT_EBE | INT_SBE | INT_HLE | INT_FRUN | INT_DCRC;
/// Every bit that means a data phase timed out.
pub(crate) const INT_DATA_TIMEOUTS: u32 = INT_HTO | INT_DRTO;
/// Every bit below the SDIO interrupts, which is what a clear writes.
pub(crate) const INT_ALL: u32 = 0xffff;

/// Status: FIFO count, busy, command state.
pub const STATUS: u32 = 0x048;
/// The FIFO is full.
pub(crate) const STATUS_FIFO_FULL: u32 = 1 << 3;
/// The card is holding DAT0 low (busy after a write or an R1b command).
pub(crate) const STATUS_DATA_BUSY: u32 = 1 << 9;

/// How many FIFO words are filled, bits 29:17 of `STATUS`.
#[must_use]
pub(crate) const fn fifo_count(status: u32) -> u32 {
    (status >> 17) & 0x1fff
}

/// FIFO thresholds and the DMA burst size.
pub(crate) const FIFOTH: u32 = 0x04c;

/// **The `FIFOTH` word for polled transfers** with a FIFO of `depth` words: a receive watermark one
/// below half and a transmit watermark at half, OpenBSD's `dwmmc_pio_mode`. The burst field is
/// OpenBSD's `2` and means nothing without DMA.
#[must_use]
pub(crate) const fn fifoth_for_pio(depth: u32) -> u32 {
    let half = depth / 2;
    let rx = if half == 0 { 0 } else { half - 1 };
    (2 << 28) | ((rx & 0xfff) << 16) | (half & 0xfff)
}

/// **The `FIFOTH` word for IDMAC transfers** with a FIFO of `depth` words. The transmit watermark
/// stays at half. The burst is the largest the databook's encoding has (1, 4, 8, ... 256 words)
/// that divides both a 512-byte block and the free half of the FIFO, so no burst is cut by the end
/// of a block or overruns the FIFO. The receive watermark is one burst less one word, so a burst
/// is ready whenever the IDMAC is asked for one. Depth 32 gives bursts of 16.
#[must_use]
pub(crate) fn fifoth_for_dma(depth: u32) -> u32 {
    const BURSTS: [u32; 8] = [1, 4, 8, 16, 32, 64, 128, 256];
    let tx = depth / 2;
    let free = depth - tx;
    let code = (0..BURSTS.len())
        .rev()
        .find(|&c| 128 % BURSTS[c] == 0 && free.is_multiple_of(BURSTS[c]))
        .unwrap_or(0);
    let rx = BURSTS[code] - 1;
    ((code as u32) << 28) | ((rx & 0xfff) << 16) | (tx & 0xfff)
}

/// The receive watermark field of `FIFOTH`, which OpenBSD reads back as `depth - 1` when the tree
/// gives no `fifo-depth` (the reset value is `depth - 1`).
#[must_use]
pub(crate) const fn fifoth_rx_watermark(fifoth: u32) -> u32 {
    (fifoth >> 16) & 0xfff
}

/// Card detect, active low: bit 0 clear means a card is in slot 0.
pub(crate) const CDETECT: u32 = 0x050;
/// Write protect, active high, slot 0.
pub const WRTPRT: u32 = 0x054;
/// Bytes the card interface has moved in the current data phase.
pub const TCBCNT: u32 = 0x05c;
/// Bytes moved between the host and the FIFO in the current data phase.
pub const TBBCNT: u32 = 0x060;
/// The integrator's user identifier, a free-form word.
pub(crate) const USRID: u32 = 0x068;
/// Synopsys version identifier: `0x5342_xxxx`, where `xxxx` is the release, `0x270a` for 2.70a.
pub(crate) const VERID: u32 = 0x06c;
/// Hardware configuration: FIFO width, address width, how many slots.
pub(crate) const HCON: u32 = 0x070;
/// UHS-I register: per-slot DDR mode (bit 16) and 1.8 V signaling (bit 0).
pub const UHS: u32 = 0x074;
/// Per-slot hardware reset line to an eMMC device, active low.
pub const RST_N: u32 = 0x078;
/// Internal DMA bus mode.
pub const BMOD: u32 = 0x080;
/// Reset the IDMAC's registers and state. Self-clearing.
pub(crate) const BMOD_SOFTWARE_RESET: u32 = 1 << 0;
/// Fixed-length bursts on the bus, of the size `FIFOTH`'s burst field names.
pub(crate) const BMOD_FIXED_BURST: u32 = 1 << 1;
/// The IDMAC walks descriptors.
pub(crate) const BMOD_DMA_ENABLE: u32 = 1 << 7;
/// Poll demand: any write wakes an IDMAC suspended on a descriptor it did not own.
pub(crate) const PLDMND: u32 = 0x084;
/// The bus address of the first descriptor (32-bit addressing; `HCON` bit 27 clear).
pub(crate) const DBADDR: u32 = 0x088;
/// IDMAC status, write one to clear. Raw: set whatever `IDINTEN` holds. The driver reads only
/// its error bits; the completion bits are `sim`'s, and the tests'.
pub const IDSTS: u32 = 0x08c;
/// A descriptor without `DIC` finished a transmit (memory to card).
#[cfg(test)]
pub(crate) const IDSTS_TI: u32 = 1 << 0;
/// A descriptor without `DIC` finished a receive (card to memory).
#[cfg(test)]
pub(crate) const IDSTS_RI: u32 = 1 << 1;
/// Fatal bus error: the bus refused a descriptor or buffer access. The IDMAC stops.
pub(crate) const IDSTS_FBE: u32 = 1 << 2;
/// Descriptor unavailable: the next descriptor's `OWN` bit was clear. The IDMAC suspends.
pub(crate) const IDSTS_DU: u32 = 1 << 4;
/// Card error summary: a data-phase error in `RINTSTS` closed a descriptor.
pub(crate) const IDSTS_CES: u32 = 1 << 5;
/// Normal interrupt summary (`TI` or `RI`).
#[cfg(test)]
pub(crate) const IDSTS_NIS: u32 = 1 << 8;
/// Abnormal interrupt summary (`FBE`, `DU` or `CES`).
#[cfg(test)]
pub(crate) const IDSTS_AIS: u32 = 1 << 9;
/// Every `IDSTS` bit that means the IDMAC could not finish.
pub(crate) const IDSTS_ERRORS: u32 = IDSTS_FBE | IDSTS_DU | IDSTS_CES;
/// Every write-one-to-clear bit of `IDSTS`, bits 9:0.
pub(crate) const IDSTS_ALL: u32 = 0x3ff;
/// IDMAC interrupt enables. Zero: this driver polls, as it does `RINTSTS`.
pub(crate) const IDINTEN: u32 = 0x090;
/// The descriptor the IDMAC is on, for a transcript after a failure.
pub const DSCADDR: u32 = 0x094;
/// The buffer address the IDMAC is on.
pub const BUFADDR: u32 = 0x098;
/// Card read threshold control: hold a read until a block fits in the FIFO.
pub(crate) const CARDTHRCTL: u32 = 0x100;
/// Enable the card read threshold, which the driver never does; the sim and tests name it.
#[cfg(test)]
pub(crate) const CARDTHRCTL_READ_ENABLE: u32 = 1 << 0;

/// The release `VERID` is compared at, Linux's `DW_MMC_240A`. Controllers before it put the data
/// FIFO at `0x100`, from it on at `0x200`.
pub(crate) const VERID_240A: u32 = 0x240a;

/// **Where the data FIFO is**, given `VERID`: `0x200` from release 2.40a, `0x100` before. Linux's
/// `dw_mci_probe` makes the same test; OpenBSD assumes `0x200` and so supports only the later
/// releases. Reading the version rather than assuming it is what lets the bench transcript say
/// which applied.
#[must_use]
pub const fn fifo_offset(verid: u32) -> u32 {
    if verid & 0xffff < VERID_240A {
        0x100
    } else {
        0x200
    }
}

/// **The FIFO's width in bytes**, from `HCON` bits 9:7: 0 is 16-bit, 1 is 32-bit, 2 is 64-bit.
/// `None` for anything else. This driver moves the FIFO in 32-bit words and so refuses the others;
/// OpenBSD does the same for 16-bit and handles 64-bit as two words, which is a step this driver
/// has not needed yet.
#[must_use]
pub const fn fifo_width(hcon: u32) -> Option<u32> {
    match (hcon >> 7) & 0x7 {
        0 => Some(2),
        1 => Some(4),
        2 => Some(8),
        _ => None,
    }
}

/// **Does the IDMAC use 64-bit addresses?** `HCON` bit 27. Changes the layout of every register
/// from `0x88` up and doubles the descriptor, so the IDMAC path refuses it; radon's is clear.
#[must_use]
pub const fn dma_64bit(hcon: u32) -> bool {
    hcon & (1 << 27) != 0
}

/// **The controller's external DMA interface**, `HCON` bits 17:16: 0 is none, 1 Synopsys' own
/// `DW_DMA`, 2 a generic DMA handshake, 3 a non-Synopsys one. An IDMAC is built only into a
/// controller that reads 0 here, which is how Linux's `dw_mci_probe` decides to use it. radon
/// reads 0, and its firmware reads the card through the IDMAC (`CTRL` bit 25 as it hands over).
#[must_use]
pub(crate) const fn dma_interface(hcon: u32) -> u32 {
    (hcon >> 16) & 0x3
}

/// The highest offset this driver reads or writes, plus one word. The kernel maps at least this.
pub const WINDOW_USED: u32 = 0x204;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hcon_names_every_fifo_width_and_refuses_the_reserved_codes() {
        assert_eq!(fifo_width(0), Some(2));
        assert_eq!(fifo_width(1 << 7), Some(4));
        assert_eq!(fifo_width(2 << 7), Some(8));
        for code in 3..8 {
            assert_eq!(fifo_width(code << 7), None, "code {code}");
        }
        // Bits outside 9:7 do not leak into the width.
        assert_eq!(fifo_width(!(7 << 7) | (1 << 7)), Some(4));
        assert!(dma_64bit(1 << 27));
        assert!(!dma_64bit(!(1 << 27)));
        // radon's word, read on both slots: no external DMA interface, so an IDMAC, and 32-bit
        // descriptor addresses.
        assert_eq!(dma_interface(0x00c4_3cc1), 0);
        assert!(!dma_64bit(0x00c4_3cc1));
        assert_eq!(dma_interface(3 << 16), 3);
    }

    /// The burst field is an encoding, not a count, and the IDMAC moves a burst at a time: a burst
    /// that does not divide the FIFO's free half would overrun it. Depth 32 is radon's tree; 48
    /// leaves a free half of 24, which 16 does not divide.
    #[test]
    fn the_dma_burst_divides_a_block_and_the_free_half_of_the_fifo() {
        assert_eq!(fifoth_for_dma(32), (3 << 28) | (15 << 16) | 16);
        assert_eq!(fifoth_for_dma(48), (2 << 28) | (7 << 16) | 24);
        assert_eq!(fifoth_for_dma(128), (5 << 28) | (63 << 16) | 64);
        assert_eq!(fifoth_for_dma(1), 0);
        // An odd depth leaves a free half one word larger than the transmit watermark: 4 of 7.
        assert_eq!(fifoth_for_dma(7), (1 << 28) | (3 << 16) | 3);
    }

    #[test]
    fn the_fifo_moves_at_release_2_40a_and_the_pio_thresholds_straddle_half() {
        assert_eq!(fifo_offset(0x5342_2309), 0x100);
        assert_eq!(fifo_offset(0x5342_240a), 0x200);
        assert_eq!(fifo_offset(0x5342_290a), 0x200);
        // depth 32: receive watermark 15, transmit 16, OpenBSD's burst code 2.
        assert_eq!(fifoth_for_pio(32), (2 << 28) | (15 << 16) | 16);
        assert_eq!(fifoth_rx_watermark(fifoth_for_pio(32)), 15);
        // A one-word FIFO cannot go below zero.
        assert_eq!(fifoth_rx_watermark(fifoth_for_pio(1)), 0);
        assert_eq!(fifo_count(0x1fff << 17), 0x1fff);
        assert_eq!(fifo_count(!(0x1fff << 17)), 0);
    }

    /// **Every bit, as a number.** The driver and `sim` both name these constants, so a wrong one
    /// would agree with itself in every sequence test; only the databook's values can catch it.
    /// Read from OpenBSD's `dwmmc.c` (the bit numbers in its `#define`s) and the `dwmmc-host`
    /// cross-check, written out as hexadecimal.
    #[test]
    fn the_register_bits_are_the_databooks() {
        for (bit, value) in [
            (CTRL_CONTROLLER_RESET, 0x1),
            (CTRL_FIFO_RESET, 0x2),
            (CTRL_DMA_RESET, 0x4),
            (CTRL_INT_ENABLE, 0x10),
            (CTRL_DMA_ENABLE, 0x20),
            (CTRL_USE_INTERNAL_DMAC, 0x2000000),
            (CLKENA_ENABLE, 0x1),
            (CLKENA_LOW_POWER, 0x10000),
            (CTYPE_4BIT, 0x1),
            (CTYPE_8BIT, 0x10000),
            (CMD_START, 0x80000000),
            (CMD_USE_HOLD_REG, 0x20000000),
            (CMD_UPDATE_CLOCK_ONLY, 0x200000),
            (CMD_SEND_INIT, 0x8000),
            (CMD_STOP_ABORT, 0x4000),
            (CMD_WAIT_PREVIOUS_DATA, 0x2000),
            (CMD_SEND_AUTO_STOP, 0x1000),
            (CMD_WRITE, 0x400),
            (CMD_DATA_EXPECTED, 0x200),
            (CMD_CHECK_RESPONSE_CRC, 0x100),
            (CMD_LONG_RESPONSE, 0x80),
            (CMD_RESPONSE_EXPECT, 0x40),
            (INT_RE, 0x2),
            (INT_CD, 0x4),
            (INT_DTO, 0x8),
            (INT_TXDR, 0x10),
            (INT_RXDR, 0x20),
            (INT_RCRC, 0x40),
            (INT_DCRC, 0x80),
            (INT_RTO, 0x100),
            (INT_DRTO, 0x200),
            (INT_HTO, 0x400),
            (INT_FRUN, 0x800),
            (INT_HLE, 0x1000),
            (INT_SBE, 0x2000),
            (INT_ACD, 0x4000),
            (INT_EBE, 0x8000),
            (STATUS_DATA_BUSY, 0x200),
            (CARDTHRCTL_READ_ENABLE, 0x1),
            (BMOD_SOFTWARE_RESET, 0x1),
            (BMOD_FIXED_BURST, 0x2),
            (BMOD_DMA_ENABLE, 0x80),
            (IDSTS_TI, 0x1),
            (IDSTS_RI, 0x2),
            (IDSTS_FBE, 0x4),
            (IDSTS_DU, 0x10),
            (IDSTS_CES, 0x20),
            (IDSTS_NIS, 0x100),
            (IDSTS_AIS, 0x200),
            (PLDMND, 0x84),
            (DBADDR, 0x88),
            (IDSTS, 0x8c),
            (IDINTEN, 0x90),
            (DSCADDR, 0x94),
            (BUFADDR, 0x98),
        ] {
            assert_eq!(bit, value);
        }
        assert_eq!(CTRL_ALL_RESET, 0x7);
        assert_eq!(INT_DATA_ERRORS, 0xb880);
        assert_eq!(INT_DATA_TIMEOUTS, 0x600);
    }
}
