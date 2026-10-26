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
    Tails, layout, registers,
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

    fn line(&self, offset: u64) -> [u8; LINE as usize] {
        let start = (offset / LINE * LINE) as usize;
        let mut l = [0; LINE as usize];
        l.copy_from_slice(&self.dram.borrow()[start..start + LINE as usize]);
        l
    }

    fn cpu_byte(&self, offset: u64) -> u8 {
        if self.mode == Mode::Coherent {
            return self.dram.borrow()[offset as usize];
        }
        let key = offset / LINE;
        let mut cache = self.cache.borrow_mut();
        let l = cache.entry(key).or_insert_with(|| self.line(offset));
        l[(offset % LINE) as usize]
    }

    fn cpu_set(&self, offset: u64, b: u8) {
        match self.mode {
            Mode::Coherent => self.dram.borrow_mut()[offset as usize] = b,
            Mode::WriteBackUnsnooped | Mode::WriteThroughUnsnooped => {
                let key = offset / LINE;
                let mut cache = self.cache.borrow_mut();
                let l = cache.entry(key).or_insert_with(|| self.line(offset));
                l[(offset % LINE) as usize] = b;
                if self.mode == Mode::WriteThroughUnsnooped {
                    self.dram.borrow_mut()[offset as usize] = b;
                }
            }
        }
    }

    fn offset(&self, physical_address: u64, length: usize) -> Option<u64> {
        let offset = physical_address.checked_sub(self.base)?;
        (offset + length as u64 <= layout::BYTES).then_some(offset)
    }

    /// The device reads `out.len()` bytes at `physical_address`, from memory, never from the CPU's
    /// cache.
    fn device_read(&self, physical_address: u64, out: &mut [u8]) {
        let offset = self
            .offset(physical_address, out.len())
            .expect("device read outside the region") as usize;
        out.copy_from_slice(&self.dram.borrow()[offset..offset + out.len()]);
    }

    /// The device writes `source` at `physical_address`, to memory.
    fn device_write(&self, physical_address: u64, source: &[u8]) {
        let offset = self
            .offset(physical_address, source.len())
            .expect("device write outside the region") as usize;
        self.dram.borrow_mut()[offset..offset + source.len()].copy_from_slice(source);
    }
}

/// The CPU's handle on the region.
pub(crate) struct Cpu<'a>(&'a Memory);

impl Region for Cpu<'_> {
    fn read_u64(&self, offset: u64) -> u64 {
        let mut b = [0; 8];
        self.read_bytes(offset, &mut b);
        u64::from_le_bytes(b)
    }
    fn write_u64(&mut self, offset: u64, v: u64) {
        self.write_bytes(offset, &v.to_le_bytes());
    }
    fn read_bytes(&self, offset: u64, out: &mut [u8]) {
        assert!(
            offset + out.len() as u64 <= layout::BYTES,
            "read outside the region"
        );
        for (i, b) in out.iter_mut().enumerate() {
            *b = self.0.cpu_byte(offset + i as u64);
        }
    }
    fn write_bytes(&mut self, offset: u64, source: &[u8]) {
        assert!(
            offset + source.len() as u64 <= layout::BYTES,
            "write outside the region"
        );
        for (i, &b) in source.iter().enumerate() {
            self.0.cpu_set(offset + i as u64, b);
        }
    }
}

/// The tail registers and a barrier counter.
pub(crate) struct TailRegs<'a> {
    registers: &'a mut BTreeMap<u32, u32>,
    barriers: &'a mut usize,
}

impl Tails for TailRegs<'_> {
    fn set_receive_tail(&mut self, address: u32) {
        self.registers.insert(registers::CHAN_RX_END_ADDR, address);
    }
    fn set_transmit_tail(&mut self, address: u32) {
        self.registers.insert(registers::CHAN_TX_END_ADDR, address);
    }
    fn barrier(&mut self) {
        // A fence orders; it cleans and invalidates nothing. On a non-coherent machine it is not
        // enough, which is the point of modeling one.
        *self.barriers += 1;
    }
}

/// A YT8531 on the MDIO bus: clause 22 registers, the extended-register window, and a link.
pub(crate) struct PhysicalLayerDevice {
    registers: [u16; 32],
    extended: BTreeMap<u16, u16>,
    link: Option<Link>,
}

impl PhysicalLayerDevice {
    /// A YT8531 strapped to 1.8 V I/O, with every bit this driver clears set, so a test sees
    /// each one cleared.
    pub(crate) fn yt8531_at_1v8() -> PhysicalLayerDevice {
        let mut registers = [0; 32];
        registers[management_data_input_output::PHYSID1 as usize] =
            (motorcomm::PHY_ID_YT8531 >> 16) as u16;
        registers[management_data_input_output::PHYSID2 as usize] = motorcomm::PHY_ID_YT8531 as u16;
        let mut extended = BTreeMap::new();
        extended.insert(motorcomm::EXT_CHIP_CONFIG, 0x2 << 4 | motorcomm::RXC_DLY_EN);
        extended.insert(motorcomm::EXT_SYNCE_CFG, motorcomm::EN_SYNC_E);
        extended.insert(
            motorcomm::EXTREG_SLEEP_CONTROL1_REG,
            motorcomm::ESC1R_SLEEP_SW | 1 << 14,
        );
        extended.insert(motorcomm::CLOCK_GATING_REG, motorcomm::CGR_RX_CLK_EN);
        extended.insert(motorcomm::EXT_PAD_DRIVE_STRENGTH, 0xffff);
        PhysicalLayerDevice {
            registers,
            extended,
            link: None,
        }
    }
    /// Set a clause 22 register directly.
    pub(crate) fn write_raw(&mut self, register: u8, v: u16) {
        self.registers[register as usize] = v;
    }
    /// Read a clause 22 register directly.
    pub(crate) fn raw(&self, register: u8) -> u16 {
        self.registers[register as usize]
    }
    /// An extended register's value.
    pub(crate) fn extended(&self, address: u16) -> u16 {
        self.extended.get(&address).copied().unwrap_or(0)
    }
    /// Plug a cable in, or pull it out.
    pub(crate) fn set_link(&mut self, link: Option<Link>) {
        self.link = link;
    }
}

impl ManagementDataInputOutput for PhysicalLayerDevice {
    fn read(
        &mut self,
        _physical_layer_device: u8,
        register: u8,
    ) -> Result<u16, management_data_input_output::Error> {
        let register = register & 0x1f;
        Ok(match register {
            motorcomm::REG_DATA => self.extended(self.registers[motorcomm::REG_ADDR as usize]),
            management_data_input_output::BMSR => {
                if self.link.is_some() {
                    management_data_input_output::BMSR_LSTATUS
                        | management_data_input_output::BMSR_ANEGCOMPLETE
                } else {
                    0
                }
            }
            motorcomm::SPECIFIC_STATUS_REG => self.link.map_or(0, |l| {
                let speed = match l.speed {
                    Speed::Ten => 0,
                    Speed::Hundred => 1,
                    Speed::Thousand => 2,
                };
                motorcomm::SSR_LINK
                    | motorcomm::SSR_SPEED_DUPLEX_RESOLVED
                    | speed << motorcomm::SSR_SPEED_SHIFT
                    | if l.full_duplex {
                        motorcomm::SSR_DUPLEX
                    } else {
                        0
                    }
            }),
            r => self.registers[r as usize],
        })
    }
    fn write(
        &mut self,
        _physical_layer_device: u8,
        register: u8,
        v: u16,
    ) -> Result<(), management_data_input_output::Error> {
        let register = register & 0x1f;
        match register {
            motorcomm::REG_DATA => {
                self.extended
                    .insert(self.registers[motorcomm::REG_ADDR as usize], v);
            }
            management_data_input_output::BMCR => {
                self.registers[0] = v & !(management_data_input_output::BMCR_ANRESTART
                    | management_data_input_output::BMCR_RESET);
            }
            r => self.registers[r as usize] = v,
        }
        Ok(())
    }
}

/// The controller.
pub(crate) struct NetworkInterfaceController {
    registers: BTreeMap<u32, u32>,
    memory: Memory,
    /// The PHY behind the MDIO pair.
    pub physical_layer_device: PhysicalLayerDevice,
    reset_polls: u32,
    reset_left: Option<u32>,
    receive_clock: bool,
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
    /// A controller fresh out of reset over `memory`. `_region` is where the tests will put the
    /// rings; the controller learns it from the base registers like the real one.
    pub(crate) fn new(_region: u64, memory: Memory) -> NetworkInterfaceController {
        let mut nic = NetworkInterfaceController {
            registers: BTreeMap::new(),
            memory,
            physical_layer_device: PhysicalLayerDevice::yt8531_at_1v8(),
            reset_polls: 1,
            reset_left: None,
            receive_clock: true,
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
        self.registers.clear();
        self.registers.insert(registers::VERSION, VERSION);
        self.registers
            .insert(registers::mac_hw_feature(1), FEATURE1);
        self.registers.insert(registers::MAC_ADDR0_HI, 0xffff);
        self.registers.insert(registers::MAC_ADDR0_LO, 0xffff_ffff);
        self.receive_cursor = 0;
        self.transmit_cursor = 0;
    }

    /// How many reads of the bus mode register the software reset takes to clear.
    pub(crate) fn set_reset_polls(&mut self, n: u32) {
        self.reset_polls = n;
    }
    /// The PHY stops driving the receive clock, and the software reset never completes.
    pub(crate) fn stop_receive_clock(&mut self) {
        self.receive_clock = false;
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
    pub(crate) fn register(&self, offset: u32) -> u32 {
        self.registers.get(&offset).copied().unwrap_or(0)
    }
    /// The receive tail register.
    pub(crate) fn receive_tail(&self) -> u32 {
        self.register(registers::CHAN_RX_END_ADDR)
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
        let mut cpu = Cpu(&self.memory);
        let mut tails = TailRegs {
            registers: &mut self.registers,
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

    fn ring(&self, base: u32, base_high: u32, length: u32) -> (u64, u16) {
        let b = u64::from(self.register(base_high)) << 32 | u64::from(self.register(base));
        (b, (self.register(length) + 1) as u16)
    }

    fn index_of_tail(&self, base: u64, tail: u32, n: u16) -> u16 {
        ((u64::from(tail).wrapping_sub(base & 0xffff_ffff) / 16) % u64::from(n)) as u16
    }

    fn descriptor(&self, physical_address: u64) -> [u32; 4] {
        let mut b = [0u8; 16];
        self.memory.device_read(physical_address, &mut b);
        let w = |i: usize| u32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap());
        [w(0), w(1), w(2), w(3)]
    }

    fn set_descriptor(&self, physical_address: u64, d: [u32; 4]) {
        let mut b = [0u8; 16];
        for (i, w) in d.iter().enumerate() {
            b[4 * i..4 * i + 4].copy_from_slice(&w.to_le_bytes());
        }
        self.memory.device_write(physical_address, &b);
    }

    fn status(&mut self, bit: u32) {
        *self.registers.entry(registers::CHAN_STATUS).or_insert(0) |= bit;
    }

    /// How many receive descriptors the device could fill right now.
    pub(crate) fn receive_available(&self) -> usize {
        let (base, n) = self.ring(
            registers::CHAN_RX_BASE_ADDR,
            registers::CHAN_RX_BASE_ADDR_HI,
            registers::CHAN_RX_RING_LEN,
        );
        let tail = self.index_of_tail(base, self.receive_tail(), n);
        let mut i = self.receive_cursor;
        let mut count = 0;
        while i != tail && self.descriptor(base + u64::from(i) * 16)[3] & RDES3_OWN != 0 {
            count += 1;
            i = (i + 1) % n;
        }
        count
    }

    fn receiving(&self) -> bool {
        self.register(registers::CHAN_RX_CONTROL) & registers::CHAN_RX_CONTROL_SR != 0
            && self.register(registers::MAC_CONF) & registers::MAC_CONF_RE != 0
    }

    /// Deliver `wire` (frame and CRC) into the next receive descriptor, if the device owns one.
    fn deliver(&mut self, wire: &[u8], error: bool) -> bool {
        if !self.receiving() {
            return false;
        }
        let (base, n) = self.ring(
            registers::CHAN_RX_BASE_ADDR,
            registers::CHAN_RX_BASE_ADDR_HI,
            registers::CHAN_RX_RING_LEN,
        );
        let tail = self.index_of_tail(base, self.receive_tail(), n);
        let at = base + u64::from(self.receive_cursor) * 16;
        if self.receive_cursor == tail || self.descriptor(at)[3] & RDES3_OWN == 0 {
            self.status(registers::CHAN_STATUS_RBU);
            return false;
        }
        let d = self.descriptor(at);
        let buffer = u64::from(d[1]) << 32 | u64::from(d[0]);
        self.memory.device_write(buffer, wire);
        let es = if error { RDES3_ES } else { 0 };
        self.set_descriptor(at, [0, 0, 0, RDES3_FD | RDES3_LD | es | wire.len() as u32]);
        self.status(registers::CHAN_STATUS_RI);
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
        if self.register(registers::CHAN_TX_CONTROL) & registers::CHAN_TX_CONTROL_ST == 0 {
            return;
        }
        let (base, n) = self.ring(
            registers::CHAN_TX_BASE_ADDR,
            registers::CHAN_TX_BASE_ADDR_HI,
            registers::CHAN_TX_RING_LEN,
        );
        loop {
            let tail = self.index_of_tail(base, self.register(registers::CHAN_TX_END_ADDR), n);
            if self.transmit_cursor == tail {
                return;
            }
            let at = base + u64::from(self.transmit_cursor) * 16;
            let d = self.descriptor(at);
            if d[3] & TDES3_OWN == 0 {
                self.status(registers::CHAN_STATUS_TBU);
                return;
            }
            let buffer = u64::from(d[1]) << 32 | u64::from(d[0]);
            let mut frame = vec![0u8; (d[2] & TDES2_BUF1_LEN) as usize];
            self.memory.device_read(buffer, &mut frame);
            self.set_descriptor(at, [0, 0, 0, d[3] & !TDES3_OWN]);
            if d[2] & TDES2_IC != 0 {
                self.status(registers::CHAN_STATUS_TI);
            }
            self.transmit_cursor = (self.transmit_cursor + 1) % n;
            let conf = self.register(registers::MAC_CONF);
            if conf & registers::CONFIG_LM != 0 {
                if !self.loopback_broken {
                    let mut w = frame.clone();
                    w.extend_from_slice(&[0xc0, 0xff, 0xee, 0x00]);
                    self.deliver(&w, false);
                }
            } else if conf & registers::MAC_CONF_TE != 0 {
                self.wire.push(frame);
            }
        }
    }
}

impl Hardware for NetworkInterfaceController {
    fn read(&mut self, offset: u32) -> u32 {
        match offset {
            registers::BUS_MODE => {
                if let Some(left) = self.reset_left {
                    if self.receive_clock && left <= 1 {
                        self.reset_left = None;
                        let v = self.register(offset) & !registers::BUS_MODE_SWR;
                        self.registers.insert(offset, v);
                    } else if self.receive_clock {
                        self.reset_left = Some(left - 1);
                    }
                }
                self.register(offset)
            }
            registers::MTL_CHAN_TX_OP_MODE if !self.flush_wedged => {
                let v = self.register(offset) & !registers::MTL_CHAN_TX_OP_MODE_FTQ;
                self.registers.insert(offset, v);
                v
            }
            _ => self.register(offset),
        }
    }

    fn write(&mut self, offset: u32, v: u32) {
        match offset {
            registers::BUS_MODE if v & registers::BUS_MODE_SWR != 0 => {
                self.reset_registers();
                self.registers.insert(offset, v);
                self.reset_left = Some(self.reset_polls);
            }
            registers::MAC_MDIO_ADDR if v & management_data_input_output::GB != 0 => {
                let physical_layer_device =
                    ((v >> management_data_input_output::PA_SHIFT) & 0x1f) as u8;
                let register = ((v >> management_data_input_output::RDA_SHIFT) & 0x1f) as u8;
                if v & (0x3 << 2) == management_data_input_output::GOC_READ {
                    let r = self
                        .physical_layer_device
                        .read(physical_layer_device, register)
                        .unwrap();
                    self.registers
                        .insert(registers::MAC_MDIO_DATA, u32::from(r));
                } else {
                    let data = self.register(registers::MAC_MDIO_DATA) as u16;
                    self.physical_layer_device
                        .write(physical_layer_device, register, data)
                        .unwrap();
                }
                self.registers
                    .insert(offset, v & !management_data_input_output::GB);
            }
            registers::CHAN_STATUS => {
                let s = self.register(offset) & !v;
                self.registers.insert(offset, s);
            }
            _ => {
                self.registers.insert(offset, v);
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
    fn management_data_input_output_through_the_controller_reaches_the_physical_layer_device() {
        let mut nic = NetworkInterfaceController::new(0, Memory::coherent(0));
        let id = management_data_input_output::physical_layer_device_id(
            &mut management_data_input_output::Bus(&mut nic),
            0,
        )
        .unwrap();
        assert_eq!(id, motorcomm::PHY_ID_YT8531);
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
