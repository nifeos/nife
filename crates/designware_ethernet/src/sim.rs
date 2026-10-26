//! **A simulated controller, PHY and memory system**, for the host tests.
//!
//! This is the crate's reading of the hardware, not the hardware: the descriptor and tail-pointer
//! behavior follows OpenBSD's driver and the register header it is built on, and where the real
//! controller does more (prefetching descriptors, the MTL FIFOs, timing) the simulation does the
//! least that still lets a wrong driver fail. Its one unusual part is [`Memory`], which can model
//! a CPU cache the device does not snoop, so the coherence probe can be shown to tell the
//! directions apart.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::vec;
use std::vec::Vec;

use crate::controller::{self, Identity};
use crate::management_data_input_output::{self, ManagementDataInputOutput};
use crate::motorcomm::{self, Link, Speed};
use crate::{
    Hardware, RDES3_ES, RDES3_FD, RDES3_LD, RDES3_OWN, Region, TDES2_BUF1_LEN, TDES2_IC, TDES3_OWN,
    Tails, layout, regs,
};

const LINE: u64 = 64;

/// How the CPU's view of memory relates to the device's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// One view: what the CPU wrote the device reads, and the reverse.
    Coherent,
    /// A write-back cache the device neither snoops nor is cleaned for: CPU stores stay in the
    /// cache, so the device reads what was in memory before.
    WriteBackUnsnooped,
    /// A write-through cache the device does not invalidate: CPU stores reach memory, but a line
    /// the CPU has touched keeps answering its reads after the device writes memory under it.
    WriteThroughUnsnooped,
}

/// The DMA region's memory, seen from the CPU and from the device.
pub(crate) struct Memory {
    base: u64,
    dram: RefCell<Vec<u8>>,
    cache: RefCell<BTreeMap<u64, [u8; LINE as usize]>>,
    mode: Mode,
}

impl Memory {
    fn new(base: u64, mode: Mode) -> Memory {
        Memory {
            base,
            dram: RefCell::new(vec![0; layout::BYTES as usize]),
            cache: RefCell::new(BTreeMap::new()),
            mode,
        }
    }
    /// The machine every published source says the JH7110 is.
    pub(crate) fn coherent(base: u64) -> Memory {
        Memory::new(base, Mode::Coherent)
    }
    /// A machine whose device cannot see the CPU's writes.
    pub(crate) fn write_back_unsnooped(base: u64) -> Memory {
        Memory::new(base, Mode::WriteBackUnsnooped)
    }
    /// A machine whose CPU cannot see the device's writes.
    pub(crate) fn write_through_unsnooped(base: u64) -> Memory {
        Memory::new(base, Mode::WriteThroughUnsnooped)
    }

    fn line(&self, off: u64) -> [u8; LINE as usize] {
        let start = (off / LINE * LINE) as usize;
        let mut l = [0; LINE as usize];
        l.copy_from_slice(&self.dram.borrow()[start..start + LINE as usize]);
        l
    }

    fn cpu_byte(&self, off: u64) -> u8 {
        if self.mode == Mode::Coherent {
            return self.dram.borrow()[off as usize];
        }
        let key = off / LINE;
        let mut cache = self.cache.borrow_mut();
        let l = cache.entry(key).or_insert_with(|| self.line(off));
        l[(off % LINE) as usize]
    }

    fn cpu_set(&self, off: u64, b: u8) {
        match self.mode {
            Mode::Coherent => self.dram.borrow_mut()[off as usize] = b,
            Mode::WriteBackUnsnooped | Mode::WriteThroughUnsnooped => {
                let key = off / LINE;
                let mut cache = self.cache.borrow_mut();
                let l = cache.entry(key).or_insert_with(|| self.line(off));
                l[(off % LINE) as usize] = b;
                if self.mode == Mode::WriteThroughUnsnooped {
                    self.dram.borrow_mut()[off as usize] = b;
                }
            }
        }
    }

    fn offset(&self, phys: u64, len: usize) -> Option<u64> {
        let off = phys.checked_sub(self.base)?;
        (off + len as u64 <= layout::BYTES).then_some(off)
    }

    /// The device reads `out.len()` bytes at `phys`, from memory, never from the CPU's cache.
    fn device_read(&self, phys: u64, out: &mut [u8]) {
        let off = self
            .offset(phys, out.len())
            .expect("device read outside the region") as usize;
        out.copy_from_slice(&self.dram.borrow()[off..off + out.len()]);
    }

    /// The device writes `src` at `phys`, to memory.
    fn device_write(&self, phys: u64, src: &[u8]) {
        let off = self
            .offset(phys, src.len())
            .expect("device write outside the region") as usize;
        self.dram.borrow_mut()[off..off + src.len()].copy_from_slice(src);
    }
}

/// The CPU's handle on the region.
pub(crate) struct Cpu<'a>(&'a Memory);

impl Region for Cpu<'_> {
    fn read_u64(&self, off: u64) -> u64 {
        let mut b = [0; 8];
        self.read_bytes(off, &mut b);
        u64::from_le_bytes(b)
    }
    fn write_u64(&mut self, off: u64, v: u64) {
        self.write_bytes(off, &v.to_le_bytes());
    }
    fn read_bytes(&self, off: u64, out: &mut [u8]) {
        assert!(
            off + out.len() as u64 <= layout::BYTES,
            "read outside the region"
        );
        for (i, b) in out.iter_mut().enumerate() {
            *b = self.0.cpu_byte(off + i as u64);
        }
    }
    fn write_bytes(&mut self, off: u64, src: &[u8]) {
        assert!(
            off + src.len() as u64 <= layout::BYTES,
            "write outside the region"
        );
        for (i, &b) in src.iter().enumerate() {
            self.0.cpu_set(off + i as u64, b);
        }
    }
}

/// The tail registers and a barrier counter.
pub(crate) struct TailRegs<'a> {
    regs: &'a mut BTreeMap<u32, u32>,
    barriers: &'a mut usize,
}

impl Tails for TailRegs<'_> {
    fn set_rx_tail(&mut self, addr: u32) {
        self.regs.insert(regs::CHAN_RX_TAIL, addr);
    }
    fn set_tx_tail(&mut self, addr: u32) {
        self.regs.insert(regs::CHAN_TX_TAIL, addr);
    }
    fn barrier(&mut self) {
        // A fence orders; it cleans and invalidates nothing. On a non-coherent machine it is not
        // enough, which is the point of modeling one.
        *self.barriers += 1;
    }
}

/// A YT8531 on the MDIO bus: clause 22 registers, the extended-register window, and a link.
pub(crate) struct Phy {
    regs: [u16; 32],
    extended: BTreeMap<u16, u16>,
    link: Option<Link>,
}

impl Phy {
    /// A YT8531 strapped to 1.8 V I/O, with every bit this driver clears set, so a test sees
    /// each one cleared.
    pub(crate) fn yt8531_at_1v8() -> Phy {
        let mut regs = [0; 32];
        regs[management_data_input_output::PHYSID1 as usize] = (motorcomm::YT8531_ID >> 16) as u16;
        regs[management_data_input_output::PHYSID2 as usize] = motorcomm::YT8531_ID as u16;
        let mut extended = BTreeMap::new();
        extended.insert(
            motorcomm::EXT_CHIP_CONFIG,
            0x2 << 4 | motorcomm::CHIP_CONFIG_RXC_DLY_EN,
        );
        extended.insert(motorcomm::EXT_SYNCE_CFG, motorcomm::SYNCE_CFG_EN);
        extended.insert(
            motorcomm::EXT_SLEEP_CONTROL1,
            motorcomm::EXT_SLEEP_CONTROL1_SLEEP_SW | 1 << 14,
        );
        extended.insert(
            motorcomm::EXT_CLOCK_GATING,
            motorcomm::EXT_CLOCK_GATING_RX_CLK_EN,
        );
        extended.insert(motorcomm::EXT_PAD_DRIVE, 0xffff);
        Phy {
            regs,
            extended,
            link: None,
        }
    }
    /// Set a clause 22 register directly.
    pub(crate) fn write_raw(&mut self, reg: u8, v: u16) {
        self.regs[reg as usize] = v;
    }
    /// Read a clause 22 register directly.
    pub(crate) fn raw(&self, reg: u8) -> u16 {
        self.regs[reg as usize]
    }
    /// An extended register's value.
    pub(crate) fn extended(&self, addr: u16) -> u16 {
        self.extended.get(&addr).copied().unwrap_or(0)
    }
    /// Plug a cable in, or pull it out.
    pub(crate) fn set_link(&mut self, link: Option<Link>) {
        self.link = link;
    }
}

impl ManagementDataInputOutput for Phy {
    fn read(&mut self, _phy: u8, reg: u8) -> Result<u16, management_data_input_output::Error> {
        let reg = reg & 0x1f;
        Ok(match reg {
            motorcomm::EXT_DATA => self.extended(self.regs[motorcomm::EXT_ADDR as usize]),
            management_data_input_output::BMSR => {
                if self.link.is_some() {
                    management_data_input_output::BMSR_LINK
                        | management_data_input_output::BMSR_ANEG_COMPLETE
                } else {
                    0
                }
            }
            motorcomm::SPECIFIC_STATUS => self.link.map_or(0, |l| {
                let speed = match l.speed {
                    Speed::Ten => 0,
                    Speed::Hundred => 1,
                    Speed::Thousand => 2,
                };
                motorcomm::STATUS_LINK
                    | motorcomm::STATUS_RESOLVED
                    | speed << motorcomm::STATUS_SPEED_SHIFT
                    | if l.full_duplex {
                        motorcomm::STATUS_DUPLEX
                    } else {
                        0
                    }
            }),
            r => self.regs[r as usize],
        })
    }
    fn write(
        &mut self,
        _phy: u8,
        reg: u8,
        v: u16,
    ) -> Result<(), management_data_input_output::Error> {
        let reg = reg & 0x1f;
        match reg {
            motorcomm::EXT_DATA => {
                self.extended
                    .insert(self.regs[motorcomm::EXT_ADDR as usize], v);
            }
            management_data_input_output::BMCR => {
                self.regs[0] = v & !(management_data_input_output::BMCR_ANRESTART
                    | management_data_input_output::BMCR_RESET);
            }
            r => self.regs[r as usize] = v,
        }
        Ok(())
    }
}

/// The controller.
pub(crate) struct NetworkInterfaceController {
    regs: BTreeMap<u32, u32>,
    mem: Memory,
    /// The PHY behind the MDIO pair.
    pub phy: Phy,
    reset_polls: u32,
    reset_left: Option<u32>,
    rx_clock: bool,
    flush_wedged: bool,
    loopback_broken: bool,
    receive_cursor: u16,
    transmit_cursor: u16,
    wire: Vec<Vec<u8>>,
    barriers: usize,
}

const VERSION: u32 = 0x1052;
/// 40-bit addressing, 2 KiB FIFOs each way.
const FEATURE1: u32 = 1 << 14 | 4 << 6 | 4;

impl NetworkInterfaceController {
    /// A controller fresh out of reset over `mem`. `_region` is where the tests will put the rings;
    /// the controller learns it from the base registers like the real one.
    pub(crate) fn new(_region: u64, mem: Memory) -> NetworkInterfaceController {
        let mut nic = NetworkInterfaceController {
            regs: BTreeMap::new(),
            mem,
            phy: Phy::yt8531_at_1v8(),
            reset_polls: 1,
            reset_left: None,
            rx_clock: true,
            flush_wedged: false,
            loopback_broken: false,
            receive_cursor: 0,
            transmit_cursor: 0,
            wire: Vec::new(),
            barriers: 0,
        };
        nic.reset_registers();
        nic
    }

    fn reset_registers(&mut self) {
        self.regs.clear();
        self.regs.insert(regs::MAC_VERSION, VERSION);
        self.regs.insert(regs::mac_hw_feature(1), FEATURE1);
        self.regs.insert(regs::MAC_ADDR0_HI, 0xffff);
        self.regs.insert(regs::MAC_ADDR0_LO, 0xffff_ffff);
        self.receive_cursor = 0;
        self.transmit_cursor = 0;
    }

    /// How many reads of the bus mode register the software reset takes to clear.
    pub(crate) fn set_reset_polls(&mut self, n: u32) {
        self.reset_polls = n;
    }
    /// The PHY stops driving the receive clock, and the software reset never completes.
    pub(crate) fn stop_receive_clock(&mut self) {
        self.rx_clock = false;
    }
    /// The transmit queue flush never completes.
    pub(crate) fn wedge_transmit_flush(&mut self) {
        self.flush_wedged = true;
    }
    /// The MAC's loopback does nothing.
    pub(crate) fn break_loopback(&mut self) {
        self.loopback_broken = true;
    }
    /// A register's current value.
    pub(crate) fn reg(&self, off: u32) -> u32 {
        self.regs.get(&off).copied().unwrap_or(0)
    }
    /// The receive tail register.
    pub(crate) fn rx_tail(&self) -> u32 {
        self.reg(regs::CHAN_RX_TAIL)
    }
    /// How many barriers the data plane has taken.
    pub(crate) fn barriers(&self) -> usize {
        self.barriers
    }
    /// The frames the controller put on the wire, CRC not included.
    pub(crate) fn wire_out(&self) -> Vec<Vec<u8>> {
        self.wire.clone()
    }

    /// Run `f` with the CPU's handle on the region and the tail registers.
    pub fn with<R>(&mut self, f: impl FnOnce(&mut Cpu<'_>, &mut TailRegs<'_>) -> R) -> R {
        let mut cpu = Cpu(&self.mem);
        let mut tails = TailRegs {
            regs: &mut self.regs,
            barriers: &mut self.barriers,
        };
        f(&mut cpu, &mut tails)
    }

    /// Configure the controller over rings at `region` and start it at 1000 Mbit/s, as the
    /// kernel does before handing the data plane over.
    pub(crate) fn program_rings_and_start(&mut self, region: u64) {
        let id = Identity::read(self);
        controller::configure(
            self,
            &id,
            &crate::jh7110::BUS,
            region,
            [0x6c, 0xcf, 0x39, 0, 0x47, 0x2c],
        )
        .unwrap();
        controller::start(
            self,
            Link {
                speed: Speed::Thousand,
                full_duplex: true,
            },
            false,
        );
    }

    fn ring(&self, base: u32, base_hi: u32, len: u32) -> (u64, u16) {
        let b = u64::from(self.reg(base_hi)) << 32 | u64::from(self.reg(base));
        (b, (self.reg(len) + 1) as u16)
    }

    fn index_of_tail(&self, base: u64, tail: u32, n: u16) -> u16 {
        ((u64::from(tail).wrapping_sub(base & 0xffff_ffff) / 16) % u64::from(n)) as u16
    }

    fn descriptor(&self, phys: u64) -> [u32; 4] {
        let mut b = [0u8; 16];
        self.mem.device_read(phys, &mut b);
        let w = |i: usize| u32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap());
        [w(0), w(1), w(2), w(3)]
    }

    fn set_descriptor(&self, phys: u64, d: [u32; 4]) {
        let mut b = [0u8; 16];
        for (i, w) in d.iter().enumerate() {
            b[4 * i..4 * i + 4].copy_from_slice(&w.to_le_bytes());
        }
        self.mem.device_write(phys, &b);
    }

    fn status(&mut self, bit: u32) {
        *self.regs.entry(regs::CHAN_STATUS).or_insert(0) |= bit;
    }

    /// How many receive descriptors the device could fill right now.
    pub(crate) fn rx_available(&self) -> usize {
        let (base, n) = self.ring(
            regs::CHAN_RX_BASE,
            regs::CHAN_RX_BASE_HI,
            regs::CHAN_RX_RING_LEN,
        );
        let tail = self.index_of_tail(base, self.rx_tail(), n);
        let mut i = self.receive_cursor;
        let mut count = 0;
        while i != tail && self.descriptor(base + u64::from(i) * 16)[3] & RDES3_OWN != 0 {
            count += 1;
            i = (i + 1) % n;
        }
        count
    }

    fn receiving(&self) -> bool {
        self.reg(regs::CHAN_RX_CONTROL) & regs::CHAN_RX_CONTROL_SR != 0
            && self.reg(regs::MAC_CONF) & regs::MAC_CONF_RE != 0
    }

    /// Deliver `wire` (frame and CRC) into the next receive descriptor, if the device owns one.
    fn deliver(&mut self, wire: &[u8], error: bool) -> bool {
        if !self.receiving() {
            return false;
        }
        let (base, n) = self.ring(
            regs::CHAN_RX_BASE,
            regs::CHAN_RX_BASE_HI,
            regs::CHAN_RX_RING_LEN,
        );
        let tail = self.index_of_tail(base, self.rx_tail(), n);
        let at = base + u64::from(self.receive_cursor) * 16;
        if self.receive_cursor == tail || self.descriptor(at)[3] & RDES3_OWN == 0 {
            self.status(regs::CHAN_STATUS_RBU);
            return false;
        }
        let d = self.descriptor(at);
        let buffer = u64::from(d[1]) << 32 | u64::from(d[0]);
        self.mem.device_write(buffer, wire);
        let es = if error { RDES3_ES } else { 0 };
        self.set_descriptor(at, [0, 0, 0, RDES3_FD | RDES3_LD | es | wire.len() as u32]);
        self.status(regs::CHAN_STATUS_RI);
        self.receive_cursor = (self.receive_cursor + 1) % n;
        true
    }

    /// A frame arrives off the wire (the simulation appends a CRC). True when it was delivered.
    pub(crate) fn wire_in(&mut self, frame: &[u8]) -> bool {
        let mut w = frame.to_vec();
        w.extend_from_slice(&[0xc0, 0xff, 0xee, 0x00]);
        self.deliver(&w, false)
    }

    /// A frame arrives with a receive error.
    pub(crate) fn wire_in_with_error(&mut self, frame: &[u8]) -> bool {
        let mut w = frame.to_vec();
        w.extend_from_slice(&[0; 4]);
        self.deliver(&w, true)
    }

    /// Run the transmit DMA until it suspends.
    pub(crate) fn step(&mut self) {
        if self.reg(regs::CHAN_TX_CONTROL) & regs::CHAN_TX_CONTROL_ST == 0 {
            return;
        }
        let (base, n) = self.ring(
            regs::CHAN_TX_BASE,
            regs::CHAN_TX_BASE_HI,
            regs::CHAN_TX_RING_LEN,
        );
        loop {
            let tail = self.index_of_tail(base, self.reg(regs::CHAN_TX_TAIL), n);
            if self.transmit_cursor == tail {
                return;
            }
            let at = base + u64::from(self.transmit_cursor) * 16;
            let d = self.descriptor(at);
            if d[3] & TDES3_OWN == 0 {
                self.status(regs::CHAN_STATUS_TBU);
                return;
            }
            let buffer = u64::from(d[1]) << 32 | u64::from(d[0]);
            let mut frame = vec![0u8; (d[2] & TDES2_BUF1_LEN) as usize];
            self.mem.device_read(buffer, &mut frame);
            self.set_descriptor(at, [0, 0, 0, d[3] & !TDES3_OWN]);
            if d[2] & TDES2_IC != 0 {
                self.status(regs::CHAN_STATUS_TI);
            }
            self.transmit_cursor = (self.transmit_cursor + 1) % n;
            let conf = self.reg(regs::MAC_CONF);
            if conf & regs::MAC_CONF_LM != 0 {
                if !self.loopback_broken {
                    let mut w = frame.clone();
                    w.extend_from_slice(&[0xc0, 0xff, 0xee, 0x00]);
                    self.deliver(&w, false);
                }
            } else if conf & regs::MAC_CONF_TE != 0 {
                self.wire.push(frame);
            }
        }
    }
}

impl Hardware for NetworkInterfaceController {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            regs::DMA_MODE => {
                if let Some(left) = self.reset_left {
                    if self.rx_clock && left <= 1 {
                        self.reset_left = None;
                        let v = self.reg(off) & !regs::DMA_MODE_SWR;
                        self.regs.insert(off, v);
                    } else if self.rx_clock {
                        self.reset_left = Some(left - 1);
                    }
                }
                self.reg(off)
            }
            regs::MTL_TXQ0_OP_MODE if !self.flush_wedged => {
                let v = self.reg(off) & !regs::MTL_TXQ_OP_MODE_FTQ;
                self.regs.insert(off, v);
                v
            }
            _ => self.reg(off),
        }
    }

    fn write(&mut self, off: u32, v: u32) {
        match off {
            regs::DMA_MODE if v & regs::DMA_MODE_SWR != 0 => {
                self.reset_registers();
                self.regs.insert(off, v);
                self.reset_left = Some(self.reset_polls);
            }
            regs::MAC_MDIO_ADDR if v & management_data_input_output::GB != 0 => {
                let phy = ((v >> management_data_input_output::PA_SHIFT) & 0x1f) as u8;
                let reg = ((v >> management_data_input_output::RDA_SHIFT) & 0x1f) as u8;
                if v & (0x3 << 2) == management_data_input_output::GOC_READ {
                    let r = self.phy.read(phy, reg).unwrap();
                    self.regs.insert(regs::MAC_MDIO_DATA, u32::from(r));
                } else {
                    let data = self.reg(regs::MAC_MDIO_DATA) as u16;
                    self.phy.write(phy, reg, data).unwrap();
                }
                self.regs.insert(off, v & !management_data_input_output::GB);
            }
            regs::CHAN_STATUS => {
                let s = self.reg(off) & !v;
                self.regs.insert(off, s);
            }
            _ => {
                self.regs.insert(off, v);
            }
        }
    }

    fn delay_us(&mut self, _us: u64) {
        self.step();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn management_data_input_output_through_the_controller_reaches_the_phy() {
        let mut nic = NetworkInterfaceController::new(0, Memory::coherent(0));
        let id = management_data_input_output::phy_id(
            &mut management_data_input_output::Bus(&mut nic),
            0,
        )
        .unwrap();
        assert_eq!(id, motorcomm::YT8531_ID);
    }

    #[test]
    fn an_unsnooped_write_back_cache_hides_the_cpus_stores_from_the_device() {
        let m = Memory::write_back_unsnooped(0x1000);
        Cpu(&m).write_u64(0, 0x1122_3344_5566_7788);
        let mut b = [0; 8];
        m.device_read(0x1000, &mut b);
        assert_eq!(b, [0; 8]);
        assert_eq!(Cpu(&m).read_u64(0), 0x1122_3344_5566_7788);
    }

    #[test]
    fn an_unsnooped_write_through_cache_hides_the_devices_stores_from_the_cpu() {
        let m = Memory::write_through_unsnooped(0x1000);
        Cpu(&m).write_u64(0, 1);
        m.device_write(0x1000, &[9; 8]);
        assert_eq!(Cpu(&m).read_u64(0), 1);
    }
}
