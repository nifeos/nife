//! **The controller's register map**, adapted from OpenBSD's `sys/dev/fdt/dwmmc.c` (the crate root
//! carries its notice) and cross-checked against `dwmmc-host` 0.4.2's `regs.rs` (Apache-2.0), which
//! keeps the databook's names. Offsets are bytes from the controller's base.
//!
//! Each constant copies OpenBSD's name without its `SDMMC_` prefix, or, where OpenBSD has none,
//! Linux's `dw_mmc.h` or the databook's as `dwmmc-host` keeps it. Each doc opens with the full
//! name and the name it copies, and each constant carries a plain-English alias.
//!
//! Only the first page matters to this driver: every register it touches, and the data FIFO, sit
//! below `0x1000`. The JH7110's window is `0x1_0000`, the rest of it unused by anything here.
//!
//! Name: provisional. Ruled 2026-10-26 (UTC): every `regs` module this tree names becomes
//! `registers`, and the constants inside keep the databook's names; was `regs`.

/// **Control Register** (OpenBSD's `SDMMC_CTRL`): the three self-clearing resets, the interrupt and
/// DMA enables.
#[doc(alias = "control register")]
pub const CTRL: u32 = 0x000;
/// **Controller reset** (OpenBSD's `SDMMC_CTRL_CONTROLLER_RESET`): reset the controller's state
/// machines. Self-clearing.
#[doc(alias = "controller reset")]
pub(crate) const CTRL_CONTROLLER_RESET: u32 = 1 << 0;
/// **FIFO reset** (OpenBSD's `SDMMC_CTRL_FIFO_RESET`): reset the data FIFO. Self-clearing.
#[doc(alias = "queue reset")]
pub(crate) const CTRL_FIFO_RESET: u32 = 1 << 1;
/// **DMA reset** (OpenBSD's `SDMMC_CTRL_DMA_RESET`): reset the DMA interface. Self-clearing.
#[doc(alias = "direct memory access reset")]
pub(crate) const CTRL_DMA_RESET: u32 = 1 << 2;
/// **All resets** (OpenBSD's `SDMMC_CTRL_ALL_RESET`): all three resets, which is how bring-up uses
/// them.
#[doc(alias = "all control resets")]
pub(crate) const CTRL_ALL_RESET: u32 = CTRL_CONTROLLER_RESET | CTRL_FIFO_RESET | CTRL_DMA_RESET;
/// **Interrupt enable** (OpenBSD's `SDMMC_CTRL_INT_ENABLE`): the global interrupt enable. This
/// driver polls, so it stays clear: an interrupt nobody routes is a line held asserted at the PLIC
/// for no reader.
#[doc(alias = "interrupt enable")]
pub(crate) const CTRL_INT_ENABLE: u32 = 1 << 4;
/// **DMA enable** (OpenBSD's `SDMMC_CTRL_DMA_ENABLE`): hand the FIFO to a DMA engine instead of the
/// CPU.
#[doc(alias = "direct memory access enable")]
pub(crate) const CTRL_DMA_ENABLE: u32 = 1 << 5;
/// **Use internal DMAC** (OpenBSD's `SDMMC_CTRL_USE_INTERNAL_DMAC`): use the controller's own
/// descriptor-walking DMA (IDMAC) rather than an external one.
#[doc(alias = "use internal direct memory access controller")]
pub(crate) const CTRL_USE_INTERNAL_DMAC: u32 = 1 << 25;

/// **Power Enable Register** (OpenBSD's `SDMMC_PWREN`): one bit per card slot. Slot 0 is the only
/// one on the JH7110.
#[doc(alias = "power enable register")]
pub(crate) const PWREN: u32 = 0x004;
/// **Clock Divider Register** (OpenBSD's `SDMMC_CLKDIV`): clock divider 0 in bits 7:0: card clock
/// is `base / (2 * divider)`, or `base` itself when 0.
#[doc(alias = "clock divider register")]
pub(crate) const CLKDIV: u32 = 0x008;
/// **Clock Source Register** (OpenBSD's `SDMMC_CLKSRC`): which divider each card uses. Always 0
/// here.
#[doc(alias = "clock source register")]
pub(crate) const CLKSRC: u32 = 0x00c;
/// **Clock Enable Register** (OpenBSD's `SDMMC_CLKENA`): card clock enable (bit 0) and low-power
/// gating (bit 16), per slot.
#[doc(alias = "clock enable register")]
pub(crate) const CLKENA: u32 = 0x010;
/// **Card clock enable** (OpenBSD's `SDMMC_CLKENA_CCLK_ENABLE`): card clock on, slot 0.
#[doc(alias = "card clock enable")]
pub(crate) const CLKENA_CCLK_ENABLE: u32 = 1 << 0;
/// **Card clock low-power mode** (OpenBSD's `SDMMC_CLKENA_CCLK_LOW_POWER`): stop the card clock
/// while the bus is idle. Memory cards only; an SDIO card needs the clock to signal its interrupt.
#[doc(alias = "card clock low power")]
pub(crate) const CLKENA_CCLK_LOW_POWER: u32 = 1 << 16;
/// **Timeout Register** (OpenBSD's `SDMMC_TMOUT`): response timeout in bits 7:0 (card clocks), data
/// read timeout in bits 31:8.
#[doc(alias = "timeout register")]
pub(crate) const TMOUT: u32 = 0x014;
/// **Card Type Register** (OpenBSD's `SDMMC_CTYPE`): bus width per slot: bit 0 for 4-bit, bit 16
/// for 8-bit, neither for 1-bit.
#[doc(alias = "card type register")]
pub(crate) const CTYPE: u32 = 0x018;
/// **4-bit card width** (OpenBSD's `SDMMC_CTYPE_4BIT`): 4-bit bus, slot 0.
#[doc(alias = "four-bit bus")]
pub(crate) const CTYPE_4BIT: u32 = 1 << 0;
/// **8-bit card width** (OpenBSD's `SDMMC_CTYPE_8BIT`): 8-bit bus, slot 0.
#[doc(alias = "eight-bit bus")]
pub(crate) const CTYPE_8BIT: u32 = 1 << 16;
/// **Block Size Register** (OpenBSD's `SDMMC_BLKSIZ`): block size in bytes for the next data phase.
#[doc(alias = "block size register")]
pub(crate) const BLKSIZ: u32 = 0x01c;
/// **Byte Count Register** (OpenBSD's `SDMMC_BYTCNT`): total bytes in the next data phase.
#[doc(alias = "byte count register")]
pub(crate) const BYTCNT: u32 = 0x020;
/// **Interrupt Mask Register** (OpenBSD's `SDMMC_INTMASK`): which raw interrupt bits reach the
/// interrupt line. Zero: this driver polls `RINTSTS`.
#[doc(alias = "interrupt mask register")]
pub(crate) const INTMASK: u32 = 0x024;
/// **Command Argument Register** (OpenBSD's `SDMMC_CMDARG`): the next command's argument.
#[doc(alias = "command argument register")]
pub(crate) const CMDARG: u32 = 0x028;
/// **Command Register** (OpenBSD's `SDMMC_CMD`): writing it with [`CMD_START_CMD`] hands the
/// command to the card interface.
#[doc(alias = "command register")]
pub(crate) const CMD: u32 = 0x02c;
/// **Start command** (OpenBSD's `SDMMC_CMD_START_CMD`): set by software to issue; the controller
/// clears it when it has taken the command.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): a register or bit name copied from a cited source
/// keeps that source's spelling strictly, and a name that only resembles it is corrected to it; was
/// `CMD_START` (OpenBSD's `dwmmc.c` writes `SDMMC_CMD_START_CMD`).
#[doc(alias = "start command")]
pub const CMD_START_CMD: u32 = 1 << 31;
/// **Use hold register** (OpenBSD's `SDMMC_CMD_USE_HOLD_REG`): drive the command and data lines
/// from the hold register (half a clock later). OpenBSD sets it on every command, on every `SoC` it
/// supports, the JH7110 among them.
#[doc(alias = "use hold register")]
pub(crate) const CMD_USE_HOLD_REG: u32 = 1 << 29;
/// **Update clock registers only** (OpenBSD's `SDMMC_CMD_UPDATE_CLOCK_REGISTERS_ONLY`): load
/// `CLKDIV`, `CLKSRC` and `CLKENA` into the card clock domain without sending anything.
#[doc(alias = "update clock registers only")]
pub(crate) const CMD_UPDATE_CLOCK_REGISTERS_ONLY: u32 = 1 << 21;
/// **Send initialization** (OpenBSD's `SDMMC_CMD_SEND_INITIALIZATION`): send 80 clocks of
/// initialization sequence before this command. CMD0 only.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): a register or bit name copied from a cited source
/// keeps that source's spelling strictly, and a name that only resembles it is corrected to it; was
/// `CMD_SEND_INIT` (OpenBSD's `dwmmc.c` writes `SDMMC_CMD_SEND_INITIALIZATION`).
#[doc(alias = "send initialization")]
pub const CMD_SEND_INITIALIZATION: u32 = 1 << 15;
/// **Stop or abort command** (OpenBSD's `SDMMC_CMD_STOP_ABORT_CMD`): this command stops a data
/// transfer in progress (CMD12).
#[doc(alias = "stop or abort command")]
pub(crate) const CMD_STOP_ABORT_CMD: u32 = 1 << 14;
/// **Wait for previous data to complete** (OpenBSD's `SDMMC_CMD_WAIT_PRVDATA_COMPLETE`): wait for a
/// previous data transfer to finish before sending.
#[doc(alias = "wait for previous data")]
pub(crate) const CMD_WAIT_PRVDATA_COMPLETE: u32 = 1 << 13;
/// **Send auto stop** (OpenBSD's `SDMMC_CMD_SEND_AUTO_STOP`): send CMD12 automatically when the
/// byte count is reached.
#[doc(alias = "send automatic stop")]
pub(crate) const CMD_SEND_AUTO_STOP: u32 = 1 << 12;
/// **Read or write** (OpenBSD's `SDMMC_CMD_WR`): the data phase is a write to the card.
#[doc(alias = "write to card")]
pub(crate) const CMD_WR: u32 = 1 << 10;
/// **Data expected** (OpenBSD's `SDMMC_CMD_DATA_EXPECTED`): the command has a data phase.
#[doc(alias = "data expected")]
pub(crate) const CMD_DATA_EXPECTED: u32 = 1 << 9;
/// **Check response CRC** (the databook's `check_response_crc`, which `dwmmc-host` keeps): check
/// the response's CRC7.
#[doc(alias = "check response CRC")]
pub(crate) const CMD_CHECK_RESPONSE_CRC: u32 = 1 << 8;
/// **Response length** (OpenBSD's `SDMMC_CMD_RESPONSE_LENGTH`): the response is 136 bits (R2).
#[doc(alias = "long response")]
pub(crate) const CMD_RESPONSE_LENGTH: u32 = 1 << 7;
/// **Response expect** (OpenBSD's `SDMMC_CMD_RESPONSE_EXPECT`): the command has a response.
#[doc(alias = "response expected")]
pub const CMD_RESPONSE_EXPECT: u32 = 1 << 6;

/// **Response Register 0** (OpenBSD's `SDMMC_RESP0`): response word 0, bits 31:0 of a short
/// response, or bits 31:0 of a long one.
#[doc(alias = "response register 0")]
pub(crate) const RESP0: u32 = 0x030;
/// **Response Register 1** (OpenBSD's `SDMMC_RESP1`): response word 1 (long responses).
#[doc(alias = "response register 1")]
pub(crate) const RESP1: u32 = 0x034;
/// **Response Register 2** (OpenBSD's `SDMMC_RESP2`): response word 2 (long responses).
#[doc(alias = "response register 2")]
pub(crate) const RESP2: u32 = 0x038;
/// **Response Register 3** (OpenBSD's `SDMMC_RESP3`): response word 3, bits 127:96 of a long
/// response.
#[doc(alias = "response register 3")]
pub(crate) const RESP3: u32 = 0x03c;
/// **Raw Interrupt Status Register** (OpenBSD's `SDMMC_RINTSTS`): write one to clear. The driver's
/// only completion signal.
#[doc(alias = "raw interrupt status register")]
pub(crate) const RINTSTS: u32 = 0x044;
/// **Response error** (OpenBSD's `SDMMC_RINTSTS_RE`): response error.
#[doc(alias = "response error")]
pub(crate) const RINTSTS_RE: u32 = 1 << 1;
/// **Command done** (OpenBSD's `SDMMC_RINTSTS_CD`): command done: the controller sent the command
/// and, if one was expected, took the response.
#[doc(alias = "command done")]
pub(crate) const RINTSTS_CD: u32 = 1 << 2;
/// **Data transfer over** (OpenBSD's `SDMMC_RINTSTS_DTO`): data transfer over.
#[doc(alias = "data transfer over")]
pub(crate) const RINTSTS_DTO: u32 = 1 << 3;
/// **Transmit FIFO data request** (OpenBSD's `SDMMC_RINTSTS_TXDR`): transmit FIFO data request:
/// room at or below the transmit watermark.
#[doc(alias = "transmit queue data request")]
pub(crate) const RINTSTS_TXDR: u32 = 1 << 4;
/// **Receive FIFO data request** (OpenBSD's `SDMMC_RINTSTS_RXDR`): receive FIFO data request: data
/// above the receive watermark.
#[doc(alias = "receive queue data request")]
pub(crate) const RINTSTS_RXDR: u32 = 1 << 5;
/// **Response CRC error** (OpenBSD's `SDMMC_RINTSTS_RCRC`): response CRC error.
#[doc(alias = "response CRC error")]
pub(crate) const RINTSTS_RCRC: u32 = 1 << 6;
/// **Data CRC error** (OpenBSD's `SDMMC_RINTSTS_DCRC`): data CRC error.
#[doc(alias = "data CRC error")]
pub(crate) const RINTSTS_DCRC: u32 = 1 << 7;
/// **Response timeout** (OpenBSD's `SDMMC_RINTSTS_RTO`): response timeout: no card answered.
#[doc(alias = "response timeout")]
pub(crate) const RINTSTS_RTO: u32 = 1 << 8;
/// **Data read timeout** (OpenBSD's `SDMMC_RINTSTS_DRTO`): data read timeout.
#[doc(alias = "data read timeout")]
pub(crate) const RINTSTS_DRTO: u32 = 1 << 9;
/// **Data starvation-by-host timeout** (OpenBSD's `SDMMC_RINTSTS_HTO`): data starvation by host
/// timeout: the FIFO was not serviced in time.
#[doc(alias = "host timeout")]
pub(crate) const RINTSTS_HTO: u32 = 1 << 10;
/// **FIFO underrun or overrun error** (OpenBSD's `SDMMC_RINTSTS_FRUN`): FIFO underrun or overrun.
#[doc(alias = "queue underrun or overrun")]
pub(crate) const RINTSTS_FRUN: u32 = 1 << 11;
/// **Hardware locked write error** (OpenBSD's `SDMMC_RINTSTS_HLE`): hardware locked write: a write
/// to `CMD` while a command was still loading.
#[doc(alias = "hardware locked write error")]
pub(crate) const RINTSTS_HLE: u32 = 1 << 12;
/// **Start-bit error** (OpenBSD's `SDMMC_RINTSTS_SBE`): start bit error.
#[doc(alias = "start bit error")]
pub(crate) const RINTSTS_SBE: u32 = 1 << 13;
/// **Auto command done** (OpenBSD's `SDMMC_RINTSTS_ACD`): auto command done: the automatic CMD12
/// finished.
#[doc(alias = "automatic command done")]
pub(crate) const RINTSTS_ACD: u32 = 1 << 14;
/// **End-bit error** (OpenBSD's `SDMMC_RINTSTS_EBE`): end bit error (read) or no CRC status
/// (write).
#[doc(alias = "end bit error")]
pub(crate) const RINTSTS_EBE: u32 = 1 << 15;
/// **Data errors** (OpenBSD's `SDMMC_RINTSTS_DATA_ERR`): every bit that means a data phase went
/// wrong, as OpenBSD groups them.
#[doc(alias = "data errors")]
pub(crate) const RINTSTS_DATA_ERR: u32 =
    RINTSTS_EBE | RINTSTS_SBE | RINTSTS_HLE | RINTSTS_FRUN | RINTSTS_DCRC;
/// **Data timeouts** (OpenBSD's `SDMMC_RINTSTS_DATA_TO`): every bit that means a data phase timed
/// out.
#[doc(alias = "data timeouts")]
pub(crate) const RINTSTS_DATA_TO: u32 = RINTSTS_HTO | RINTSTS_DRTO;
/// Every bit below the SDIO interrupts, which is what a clear writes.
pub(crate) const RINTSTS_ALL: u32 = 0xffff;

/// **Status Register** (OpenBSD's `SDMMC_STATUS`): FIFO count, busy, command state.
#[doc(alias = "status register")]
pub const STATUS: u32 = 0x048;
/// **FIFO full** (the databook's `fifo_full`, which `dwmmc-host` keeps): the FIFO is full.
#[doc(alias = "queue full")]
pub(crate) const STATUS_FIFO_FULL: u32 = 1 << 3;
/// **Data busy** (OpenBSD's `SDMMC_STATUS_DATA_BUSY`): the card is holding DAT0 low (busy after a
/// write or an R1b command).
#[doc(alias = "data busy")]
pub(crate) const STATUS_DATA_BUSY: u32 = 1 << 9;

/// **FIFO count** (OpenBSD's `SDMMC_STATUS_FIFO_COUNT`, the databook's `fifo_count`): how many FIFO
/// words are filled, bits 29:17 of `STATUS`.
#[doc(alias = "fifo")]
#[must_use]
pub(crate) const fn queue_count(status: u32) -> u32 {
    (status >> 17) & 0x1fff
}

/// **FIFO Threshold Watermark Register** (OpenBSD's `SDMMC_FIFOTH`): FIFO thresholds and the DMA
/// burst size.
#[doc(alias = "queue threshold register")]
pub(crate) const FIFOTH: u32 = 0x04c;

/// **The `FIFOTH` word for polled transfers** with a FIFO of `depth` words: a receive watermark one
/// below half and a transmit watermark at half, OpenBSD's `dwmmc_pio_mode`. The burst field is
/// OpenBSD's `2` and means nothing without DMA.
#[must_use]
pub(crate) const fn fifoth_for_pio(depth: u32) -> u32 {
    let half = depth / 2;
    let receive_watermark = if half == 0 { 0 } else { half - 1 };
    (2 << 28) | ((receive_watermark & 0xfff) << 16) | (half & 0xfff)
}

/// **The `FIFOTH` word for IDMAC transfers** with a FIFO of `depth` words. The transmit watermark
/// stays at half. The burst is the largest the databook's encoding has (1, 4, 8, ... 256 words)
/// that divides both a 512-byte block and the free half of the FIFO, so no burst is cut by the end
/// of a block or overruns the FIFO. The receive watermark is one burst less one word, so a burst
/// is ready whenever the IDMAC is asked for one. Depth 32 gives bursts of 16.
#[must_use]
pub(crate) fn fifoth_for_direct_memory_access(depth: u32) -> u32 {
    const BURSTS: [u32; 8] = [1, 4, 8, 16, 32, 64, 128, 256];
    let transmit_watermark = depth / 2;
    let free = depth - transmit_watermark;
    let code = (0..BURSTS.len())
        .rev()
        .find(|&c| 128 % BURSTS[c] == 0 && free.is_multiple_of(BURSTS[c]))
        .unwrap_or(0);
    let receive_watermark = BURSTS[code] - 1;
    ((code as u32) << 28) | ((receive_watermark & 0xfff) << 16) | (transmit_watermark & 0xfff)
}

/// The receive watermark field of `FIFOTH`, which OpenBSD reads back as `depth - 1` when the tree
/// gives no `fifo-depth` (the reset value is `depth - 1`).
#[must_use]
pub(crate) const fn fifoth_receive_watermark(fifoth: u32) -> u32 {
    (fifoth >> 16) & 0xfff
}

/// **Card Detect Register** (OpenBSD's `SDMMC_CDETECT`): active low: bit 0 clear means a card is in
/// slot 0.
#[doc(alias = "card detect register")]
pub(crate) const CDETECT: u32 = 0x050;
/// **Write Protect Register** (OpenBSD's `SDMMC_WRTPRT`): active high, slot 0.
#[doc(alias = "write protect register")]
pub const WRTPRT: u32 = 0x054;
/// **Transferred CIU Card Byte Count Register** (OpenBSD's `SDMMC_TCBCNT`): bytes the card
/// interface has moved in the current data phase.
#[doc(alias = "transferred card byte count")]
pub const TCBCNT: u32 = 0x05c;
/// **Transferred Host to BIU-FIFO Byte Count Register** (OpenBSD's `SDMMC_TBBCNT`): bytes moved
/// between the host and the FIFO in the current data phase.
#[doc(alias = "transferred host byte count")]
pub const TBBCNT: u32 = 0x060;
/// **User ID Register** (OpenBSD's `SDMMC_USRID`): the integrator's user identifier, a free-form
/// word.
#[doc(alias = "user identifier register")]
pub(crate) const USRID: u32 = 0x068;
/// **Version ID Register** (OpenBSD's `SDMMC_VERID`): `0x5342_xxxx`, where `xxxx` is the release,
/// `0x270a` for 2.70a.
#[doc(alias = "version identifier register")]
pub(crate) const VERID: u32 = 0x06c;
/// **Hardware Configuration Register** (OpenBSD's `SDMMC_HCON`): FIFO width, address width, how
/// many slots.
#[doc(alias = "hardware configuration register")]
pub(crate) const HCON: u32 = 0x070;
/// **The ultra high speed register** (`UHS_REG` in OpenBSD's `dwmmc.c` and Linux's `dw_mmc.h`),
/// for the SD bus's UHS-I modes: per-slot DDR mode (bit 16) and 1.8 V signaling (bit 0).
///
/// Name: provisional. Ruled 2026-10-26 (UTC): the SD and MMC vocabulary this tree authors spells
/// out, with the specification's abbreviation as a `doc(alias)`; was `UHS`.
#[doc(alias = "uhs")]
#[doc(alias = "UHS_REG")]
pub const ULTRA_HIGH_SPEED: u32 = 0x074;
/// **Hardware Reset Register** (Linux's `SDMMC_RST_N`; OpenBSD writes `SDMMC_RST_n`): per-slot
/// hardware reset line to an eMMC device, active low.
#[doc(alias = "hardware reset register")]
pub const RST_N: u32 = 0x078;
/// **Bus Mode Register** (OpenBSD's `SDMMC_BMOD`): the IDMAC's resets, burst mode and enable.
#[doc(alias = "bus mode register")]
pub const BMOD: u32 = 0x080;
/// **Software reset** (OpenBSD's `SDMMC_BMOD_SWR`): reset the IDMAC's registers and state.
/// Self-clearing.
#[doc(alias = "software reset")]
pub(crate) const BMOD_SWR: u32 = 1 << 0;
/// **Fixed burst** (OpenBSD's `SDMMC_BMOD_FB`): fixed-length bursts on the bus, of the size
/// `FIFOTH`'s burst field names.
#[doc(alias = "fixed burst")]
pub(crate) const BMOD_FB: u32 = 1 << 1;
/// **IDMAC enable** (OpenBSD's `SDMMC_BMOD_DE`): the IDMAC walks descriptors.
#[doc(alias = "internal direct memory access enable")]
pub(crate) const BMOD_DE: u32 = 1 << 7;
/// **Poll Demand Register** (OpenBSD's `SDMMC_PLDMND`): any write wakes an IDMAC suspended on a
/// descriptor it did not own.
#[doc(alias = "poll demand register")]
pub(crate) const PLDMND: u32 = 0x084;
/// **Descriptor List Base Address Register** (OpenBSD's `SDMMC_DBADDR`): the bus address of the
/// first descriptor (32-bit addressing; `HCON` bit 27 clear).
#[doc(alias = "descriptor list base address")]
pub(crate) const DBADDR: u32 = 0x088;
/// **Internal DMAC Status Register** (Linux's `SDMMC_IDSTS`, OpenBSD's `SDMMC_IDSTS32`): write one
/// to clear. Raw: set whatever `IDINTEN` holds. The driver reads only its error bits; the
/// completion bits are `sim`'s, and the tests'.
///
/// Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.
#[doc(alias = "internal direct memory access status register")]
pub const IDSTS: u32 = 0x08c;
/// **Transmit interrupt** (OpenBSD's `SDMMC_IDSTS_TI`): A descriptor without `DIC` finished a
/// transmit (memory to card).
#[cfg(test)]
#[doc(alias = "transmit interrupt")]
pub(crate) const IDSTS_TI: u32 = 1 << 0;
/// **Receive interrupt** (OpenBSD's `SDMMC_IDSTS_RI`): A descriptor without `DIC` finished a
/// receive (card to memory).
#[cfg(test)]
#[doc(alias = "receive interrupt")]
pub(crate) const IDSTS_RI: u32 = 1 << 1;
/// **Fatal bus error interrupt** (the databook's `FBE` in `IDSTS`, Linux's `SDMMC_IDMAC_INT_FBE`):
/// fatal bus error: the bus refused a descriptor or buffer access. The IDMAC stops.
#[doc(alias = "fatal bus error")]
pub(crate) const IDSTS_FBE: u32 = 1 << 2;
/// **Descriptor unavailable interrupt** (the databook's `DU` in `IDSTS`, Linux's
/// `SDMMC_IDMAC_INT_DU`): descriptor unavailable: the next descriptor's `OWN` bit was clear. The
/// IDMAC suspends.
#[doc(alias = "descriptor unavailable")]
pub(crate) const IDSTS_DU: u32 = 1 << 4;
/// **Card error summary** (the databook's `CES` in `IDSTS`, Linux's `SDMMC_IDMAC_INT_CES`): card
/// error summary: a data-phase error in `RINTSTS` closed a descriptor.
#[doc(alias = "card error summary")]
pub(crate) const IDSTS_CES: u32 = 1 << 5;
/// **Normal interrupt summary** (OpenBSD's `SDMMC_IDSTS_NIS`): normal interrupt summary (`TI` or
/// `RI`).
#[cfg(test)]
#[doc(alias = "normal interrupt summary")]
pub(crate) const IDSTS_NIS: u32 = 1 << 8;
/// **Abnormal interrupt summary** (the databook's `AIS` in `IDSTS`, Linux's `SDMMC_IDMAC_INT_AI`):
/// abnormal interrupt summary (`FBE`, `DU` or `CES`).
#[cfg(test)]
#[doc(alias = "abnormal interrupt summary")]
pub(crate) const IDSTS_AIS: u32 = 1 << 9;
/// Every `IDSTS` bit that means the IDMAC could not finish.
pub(crate) const IDSTS_ERRORS: u32 = IDSTS_FBE | IDSTS_DU | IDSTS_CES;
/// Every write-one-to-clear bit of `IDSTS`, bits 9:0.
pub(crate) const IDSTS_ALL: u32 = 0x3ff;
/// **Internal DMAC Interrupt Enable Register** (Linux's `SDMMC_IDINTEN`, OpenBSD's
/// `SDMMC_IDINTEN32`): zero: this driver polls, as it does `RINTSTS`.
#[doc(alias = "internal direct memory access interrupt enable register")]
pub(crate) const IDINTEN: u32 = 0x090;
/// **Current Host Descriptor Address Register** (OpenBSD's `SDMMC_DSCADDR`): the descriptor the
/// IDMAC is on, for a transcript after a failure.
///
/// Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.
#[doc(alias = "current descriptor address")]
pub const DSCADDR: u32 = 0x094;
/// **Current Buffer Descriptor Address Register** (OpenBSD's `SDMMC_BUFADDR`): the buffer address
/// the IDMAC is on.
///
/// Name: provisional, minted 2026-10-22 (UTC) by milestone/53-radon-idmac.
#[doc(alias = "current buffer address")]
pub const BUFADDR: u32 = 0x098;
/// **Card Threshold Control Register** (OpenBSD's `SDMMC_CARDTHRCTL`): hold a read until a block
/// fits in the FIFO.
#[doc(alias = "card threshold control register")]
pub(crate) const CARDTHRCTL: u32 = 0x100;
/// **Card read threshold enable** (OpenBSD's `SDMMC_CARDTHRCTL_RDTHREN`): enable the card read
/// threshold, which the driver never does; the sim and tests name it.
#[cfg(test)]
#[doc(alias = "card read threshold enable")]
pub(crate) const CARDTHRCTL_RDTHREN: u32 = 1 << 0;

/// **Release 2.40a** (Linux's `DW_MMC_240A`): the release `VERID` is compared at. Controllers
/// before it put the data FIFO at `0x100`, from it on at `0x200`.
#[doc(alias = "release 2.40a")]
pub(crate) const DW_MMC_240A: u32 = 0x240a;

/// **Where the data FIFO is**, given `VERID`: `0x200` from release 2.40a, `0x100` before. Linux's
/// `dw_mci_probe` makes the same test; OpenBSD assumes `0x200` and so supports only the later
/// releases. Reading the version rather than assuming it is what lets the bench transcript say
/// which applied.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `fifo` becomes `queue` in every name this tree
/// authors, and copied register names keep `FIFO`; was `fifo_offset`.
#[doc(alias = "fifo")]
#[must_use]
pub const fn queue_offset(verid: u32) -> u32 {
    if verid & 0xffff < DW_MMC_240A {
        0x100
    } else {
        0x200
    }
}

/// **The FIFO's width in bytes**, from `HCON` bits 9:7: 0 is 16-bit, 1 is 32-bit, 2 is 64-bit.
/// `None` for anything else. This driver moves the FIFO in 32-bit words and so refuses the others;
/// OpenBSD does the same for 16-bit and handles 64-bit as two words, which is a step this driver
/// has not needed yet.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `fifo` becomes `queue` in every name this tree
/// authors; was `fifo_width`.
#[doc(alias = "fifo")]
#[must_use]
pub const fn queue_width(hcon: u32) -> Option<u32> {
    match (hcon >> 7) & 0x7 {
        0 => Some(2),
        1 => Some(4),
        2 => Some(8),
        _ => None,
    }
}

/// **Does the IDMAC use 64-bit addresses?** `HCON` bit 27. Changes the layout of every register
/// from `0x88` up and doubles the descriptor, so the IDMAC path refuses it; radon's is clear.
///
/// Name: provisional. Ruled 2026-10-26 (UTC): `dma` is spelled out wherever this tree authors it;
/// was `dma_64bit`.
#[must_use]
pub const fn uses_64_bit_direct_memory_access(hcon: u32) -> bool {
    hcon & (1 << 27) != 0
}

/// **The controller's external DMA interface**, `HCON` bits 17:16: 0 is none, 1 Synopsys' own
/// `DW_DMA`, 2 a generic DMA handshake, 3 a non-Synopsys one. An IDMAC is built only into a
/// controller that reads 0 here, which is how Linux's `dw_mci_probe` decides to use it. radon
/// reads 0, and its firmware reads the card through the IDMAC (`CTRL` bit 25 as it hands over).
#[must_use]
pub(crate) const fn direct_memory_access_interface(hcon: u32) -> u32 {
    (hcon >> 16) & 0x3
}

/// The highest offset this driver reads or writes, plus one word. The kernel maps at least this.
pub const WINDOW_USED: u32 = 0x204;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hcon_names_every_fifo_width_and_refuses_the_reserved_codes() {
        assert_eq!(queue_width(0), Some(2));
        assert_eq!(queue_width(1 << 7), Some(4));
        assert_eq!(queue_width(2 << 7), Some(8));
        for code in 3..8 {
            assert_eq!(queue_width(code << 7), None, "code {code}");
        }
        // Bits outside 9:7 do not leak into the width.
        assert_eq!(queue_width(!(7 << 7) | (1 << 7)), Some(4));
        assert!(uses_64_bit_direct_memory_access(1 << 27));
        assert!(!uses_64_bit_direct_memory_access(!(1 << 27)));
        // radon's word, read on both slots: no external DMA interface, so an IDMAC, and 32-bit
        // descriptor addresses.
        assert_eq!(direct_memory_access_interface(0x00c4_3cc1), 0);
        assert!(!uses_64_bit_direct_memory_access(0x00c4_3cc1));
        assert_eq!(direct_memory_access_interface(3 << 16), 3);
    }

    /// The burst field is an encoding, not a count, and the IDMAC moves a burst at a time: a burst
    /// that does not divide the FIFO's free half would overrun it. Depth 32 is radon's tree; 48
    /// leaves a free half of 24, which 16 does not divide.
    #[test]
    fn the_dma_burst_divides_a_block_and_the_free_half_of_the_fifo() {
        assert_eq!(
            fifoth_for_direct_memory_access(32),
            (3 << 28) | (15 << 16) | 16
        );
        assert_eq!(
            fifoth_for_direct_memory_access(48),
            (2 << 28) | (7 << 16) | 24
        );
        assert_eq!(
            fifoth_for_direct_memory_access(128),
            (5 << 28) | (63 << 16) | 64
        );
        assert_eq!(fifoth_for_direct_memory_access(1), 0);
        // An odd depth leaves a free half one word larger than the transmit watermark: 4 of 7.
        assert_eq!(
            fifoth_for_direct_memory_access(7),
            (1 << 28) | (3 << 16) | 3
        );
    }

    #[test]
    fn the_fifo_moves_at_release_2_40a_and_the_pio_thresholds_straddle_half() {
        assert_eq!(queue_offset(0x5342_2309), 0x100);
        assert_eq!(queue_offset(0x5342_240a), 0x200);
        assert_eq!(queue_offset(0x5342_290a), 0x200);
        // depth 32: receive watermark 15, transmit 16, OpenBSD's burst code 2.
        assert_eq!(fifoth_for_pio(32), (2 << 28) | (15 << 16) | 16);
        assert_eq!(fifoth_receive_watermark(fifoth_for_pio(32)), 15);
        // A one-word FIFO cannot go below zero.
        assert_eq!(fifoth_receive_watermark(fifoth_for_pio(1)), 0);
        assert_eq!(queue_count(0x1fff << 17), 0x1fff);
        assert_eq!(queue_count(!(0x1fff << 17)), 0);
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
            (CLKENA_CCLK_ENABLE, 0x1),
            (CLKENA_CCLK_LOW_POWER, 0x10000),
            (CTYPE_4BIT, 0x1),
            (CTYPE_8BIT, 0x10000),
            (CMD_START_CMD, 0x80000000),
            (CMD_USE_HOLD_REG, 0x20000000),
            (CMD_UPDATE_CLOCK_REGISTERS_ONLY, 0x200000),
            (CMD_SEND_INITIALIZATION, 0x8000),
            (CMD_STOP_ABORT_CMD, 0x4000),
            (CMD_WAIT_PRVDATA_COMPLETE, 0x2000),
            (CMD_SEND_AUTO_STOP, 0x1000),
            (CMD_WR, 0x400),
            (CMD_DATA_EXPECTED, 0x200),
            (CMD_CHECK_RESPONSE_CRC, 0x100),
            (CMD_RESPONSE_LENGTH, 0x80),
            (CMD_RESPONSE_EXPECT, 0x40),
            (RINTSTS_RE, 0x2),
            (RINTSTS_CD, 0x4),
            (RINTSTS_DTO, 0x8),
            (RINTSTS_TXDR, 0x10),
            (RINTSTS_RXDR, 0x20),
            (RINTSTS_RCRC, 0x40),
            (RINTSTS_DCRC, 0x80),
            (RINTSTS_RTO, 0x100),
            (RINTSTS_DRTO, 0x200),
            (RINTSTS_HTO, 0x400),
            (RINTSTS_FRUN, 0x800),
            (RINTSTS_HLE, 0x1000),
            (RINTSTS_SBE, 0x2000),
            (RINTSTS_ACD, 0x4000),
            (RINTSTS_EBE, 0x8000),
            (STATUS_DATA_BUSY, 0x200),
            (CARDTHRCTL_RDTHREN, 0x1),
            (BMOD_SWR, 0x1),
            (BMOD_FB, 0x2),
            (BMOD_DE, 0x80),
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
        assert_eq!(RINTSTS_DATA_ERR, 0xb880);
        assert_eq!(RINTSTS_DATA_TO, 0x600);
    }
}
