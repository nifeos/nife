//! **A simulated I219, for the host tests of [`super::sequence`].**
//!
//! It is this crate's reading of FreeBSD's code turned inside out, not the device: the semaphore
//! sticks unless firmware holds it, `MDIC` answers at once, a PHY in `SMBus` mode answers only a MAC
//! forced into `SMBus` too, `LANPHYPC` power-cycles the PHY back to PCIe, and the flash registers
//! read words out of an NVM image. What it can catch is a sequence that disagrees with itself or
//! with FreeBSD's order: a semaphore left held, a page select to the wrong address, a PHY reset
//! issued when firmware forbids it. What it cannot catch is FreeBSD being wrong about silicon, or
//! this model being wrong the same way the port is. xenon is the test of that.

use std::collections::HashMap;
use std::vec::Vec;

use super::phy::{self, mdic};
use super::sequence::Note;
use super::{Hardware, non_volatile_memory, regs};
use crate::regs as mac;

/// How the PHY behaves when the bring-up starts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Start {
    /// Answers over PCIe.
    Healthy,
    /// Left in `SMBus` mode by firmware: answers only while the MAC is forced to `SMBus` too.
    InSystemManagementBus,
    /// Answers nothing until a `LANPHYPC` power cycle.
    Dead,
}

pub struct Sim {
    pub mac: HashMap<u64, u32>,
    pub phy: HashMap<u32, u16>,
    /// Every PHY write that reached a register, as (paged offset, value), in order.
    pub phy_writes: Vec<(u32, u16)>,
    /// Every MDIC write as issued, for checking page selects and addresses.
    pub mdic_log: Vec<u32>,
    pub notes: Vec<Note>,
    pub non_volatile_memory: Vec<u16>,
    pub firmware_holds_software_flag: bool,
    pub phy_in_system_management_bus: bool,
    pub phy_dead: bool,
    pub page: u32,
    pub lanphypc_toggles: u32,
    pub phy_resets: u32,
    pub global_resets: u32,
    pub strap_writes: Vec<u32>,
    pub elapsed_us: u64,
    flash_status: u16,
    flash_ctl: u16,
    /// Reads of the flash status that still show a cycle in progress; `u32::MAX` never clears.
    pub flash_busy_reads: u32,
    /// Cycles that end in `FLCERR` before one succeeds.
    pub flash_failing_cycles: u32,
    /// Status reads a started cycle stays in progress for before it ends; `u32::MAX` never ends.
    /// Zero, the default, ends each cycle at `GO`. The data and `FDONE` arrive when it ends, as
    /// on the device, and a `FDONE` left set by an earlier cycle stays set until it is cleared.
    pub flash_slow_reads: u32,
    flash_pending: u32,
    /// Every `GO` issued, as the control half it was issued with.
    pub flash_cycles: Vec<u16>,
    /// `GO` issued while a cycle was still in progress, which the device ignores.
    pub flash_go_while_busy: u32,
    /// Every BAR0 read by offset, in order, except of the flash status word, which a cycle that
    /// never ends polls ten million times.
    pub reads: Vec<u64>,
    /// Every BAR0 write, in order.
    pub writes: Vec<(u64, u32)>,
    lanphypc_low: bool,
}

pub(crate) const PHY_ID: u32 = 0x0154_00a0;
/// `SMBus` address 2, frequency field 3, NVM size field 0 (one 4 KiB NVM: two 1024-word banks).
const STRAP_VALUE: u32 = 0x0004_3000;

impl Sim {
    pub fn new(start: Start, me: bool) -> Self {
        let mut s = Sim {
            mac: HashMap::new(),
            phy: HashMap::new(),
            phy_writes: Vec::new(),
            mdic_log: Vec::new(),
            notes: Vec::new(),
            non_volatile_memory: std::vec![0xffff; 2048],
            firmware_holds_software_flag: false,
            phy_in_system_management_bus: start == Start::InSystemManagementBus,
            phy_dead: start == Start::Dead,
            page: 0,
            lanphypc_toggles: 0,
            phy_resets: 0,
            global_resets: 0,
            strap_writes: Vec::new(),
            elapsed_us: 0,
            flash_status: non_volatile_memory::status::DESCRIPTOR_VALID,
            flash_ctl: 0,
            flash_busy_reads: 0,
            flash_failing_cycles: 0,
            flash_slow_reads: 0,
            flash_pending: 0,
            flash_cycles: Vec::new(),
            flash_go_while_busy: 0,
            reads: Vec::new(),
            writes: Vec::new(),
            lanphypc_low: false,
        };
        // FW_VALID when there is an ME; RSPCIPHY set, so firmware does not block PHY resets.
        s.mac.insert(
            regs::FWSM,
            phy::FWSM_RSPCIPHY | if me { super::ulp::FWSM_FW_VALID } else { 0 },
        );
        s.mac.insert(regs::STRAP, STRAP_VALUE);
        s.phy.insert(phy::r::ID1, (PHY_ID >> 16) as u16);
        s.phy.insert(phy::r::ID2, PHY_ID as u16 | 0x3);
        // Bank 0 carries the valid signature (word 0x13, high byte 0b10xx_xxxx).
        s.non_volatile_memory[non_volatile_memory::SIGNATURE_WORD as usize] = 0x8000;
        s
    }

    fn get(&self, off: u64) -> u32 {
        *self.mac.get(&off).unwrap_or(&0)
    }

    fn phy_answers(&self) -> bool {
        let forced = self.get(mac::CTRL_EXT) & 0x800 != 0;
        !self.phy_dead && self.phy_in_system_management_bus == forced
    }

    fn mdic(&mut self, v: u32) {
        self.mdic_log.push(v);
        let reg = (v & mdic::REG_MASK) >> mdic::REG_SHIFT;
        let address = v >> mdic::PHY_SHIFT & 0x1f;
        if !self.phy_answers() {
            self.mac.insert(mac_mdic(), v | mdic::READY | mdic::ERROR);
            return;
        }
        let offset = if address == 1 && reg == phy::PAGE_SELECT {
            None
        } else if address == 1 {
            let page = if self.page == 0 {
                phy::HV_PAGE_START
            } else {
                self.page
            };
            Some(phy::reg(page, reg))
        } else {
            Some(reg)
        };
        if v & mdic::OP_WRITE != 0 {
            let data = v as u16;
            match offset {
                None => self.page = u32::from(data) >> phy::PAGE_SHIFT,
                Some(o) => {
                    self.phy.insert(o, data);
                    self.phy_writes.push((o, data));
                    if o == phy::r::SMB_CTRL && data & 1 == 0 {
                        self.phy_in_system_management_bus = false;
                    }
                }
            }
            self.mac.insert(mac_mdic(), v | mdic::READY);
        } else {
            let data = offset.map_or(0, |o| *self.phy.get(&o).unwrap_or(&0));
            self.mac
                .insert(mac_mdic(), (v & !0xffff) | u32::from(data) | mdic::READY);
        }
    }

    fn ctrl(&mut self, v: u32) {
        const OVERRIDE: u32 = 0x0001_0000;
        const VALUE: u32 = 0x0002_0000;
        if v & OVERRIDE != 0 && v & VALUE == 0 {
            self.lanphypc_low = true;
        } else if self.lanphypc_low && v & OVERRIDE == 0 {
            self.lanphypc_low = false;
            self.lanphypc_toggles += 1;
            self.phy_dead = false;
            self.phy_in_system_management_bus = false;
            let e = self.get(mac::CTRL_EXT);
            self.mac.insert(mac::CTRL_EXT, e | 0x4); // LPCD
        }
        let mut stored = v;
        if v & 0x8000_0000 != 0 {
            self.phy_resets += 1;
            let s = self.get(mac::STATUS);
            self.mac.insert(mac::STATUS, s | 0x600); // PHYRA, LAN_INIT_DONE
        }
        if v & crate::CTRL_RST != 0 {
            self.global_resets += 1;
            stored &= !crate::CTRL_RST; // self-clears in the model at once
            // The reset drops the semaphore; the configuration pointer is reloaded from the NVM.
            let e = self.get(regs::EXTCNF_CTRL);
            self.mac
                .insert(regs::EXTCNF_CTRL, e & !phy::EXTCNF_CTRL_SWFLAG);
            let s = self.get(mac::STATUS);
            self.mac.insert(mac::STATUS, s | 0x200); // LAN_INIT_DONE
        }
        self.mac.insert(mac::CTRL, stored);
    }

    fn flash_write(&mut self, v: u32) {
        let low = v as u16;
        // Write one to clear: done, error, direct access error.
        self.flash_status &= !(low
            & (non_volatile_memory::status::DONE
                | non_volatile_memory::status::ERROR
                | non_volatile_memory::status::DIRECT_ACCESS_ERROR));
        self.flash_ctl = (v >> 16) as u16;
        if self.flash_ctl & non_volatile_memory::control::GO != 0 {
            if self.flash_busy_reads > 0 || self.flash_pending > 0 {
                self.flash_go_while_busy += 1;
            }
            self.flash_cycles.push(self.flash_ctl);
            self.flash_ctl &= !non_volatile_memory::control::GO;
            self.flash_pending = self.flash_slow_reads;
            if self.flash_pending == 0 {
                self.flash_end_cycle();
            }
        }
    }

    fn flash_end_cycle(&mut self) {
        let byte = self.get(non_volatile_memory::FLASH_BASE + non_volatile_memory::FADDR) as usize;
        let w = byte / 2;
        let lo = *self.non_volatile_memory.get(w).unwrap_or(&0xffff);
        let hi = *self.non_volatile_memory.get(w + 1).unwrap_or(&0xffff);
        self.mac.insert(
            non_volatile_memory::FLASH_BASE + non_volatile_memory::FDATA0,
            u32::from(hi) << 16 | u32::from(lo),
        );
        if self.flash_failing_cycles > 0 {
            self.flash_failing_cycles -= 1;
            self.flash_status |=
                non_volatile_memory::status::DONE | non_volatile_memory::status::ERROR;
        } else {
            self.flash_status |= non_volatile_memory::status::DONE;
        }
    }
}

fn mac_mdic() -> u64 {
    regs::MDIC
}

impl Hardware for Sim {
    fn read(&mut self, off: u64) -> u32 {
        if off == non_volatile_memory::FLASH_BASE + non_volatile_memory::HSFSTS {
            if self.flash_pending > 0 {
                if self.flash_pending != u32::MAX {
                    self.flash_pending -= 1;
                }
                if self.flash_pending == 0 {
                    self.flash_end_cycle();
                } else {
                    let st = self.flash_status | non_volatile_memory::status::IN_PROGRESS;
                    return u32::from(self.flash_ctl) << 16 | u32::from(st);
                }
            }
            let mut st = self.flash_status;
            if self.flash_busy_reads > 0 {
                if self.flash_busy_reads != u32::MAX {
                    self.flash_busy_reads -= 1;
                }
                st |= non_volatile_memory::status::IN_PROGRESS;
            }
            return u32::from(self.flash_ctl) << 16 | u32::from(st);
        }
        self.reads.push(off);
        self.get(off)
    }
    fn write(&mut self, off: u64, v: u32) {
        self.writes.push((off, v));
        match off {
            o if o == regs::MDIC => self.mdic(v),
            o if o == mac::CTRL => self.ctrl(v),
            o if o == regs::EXTCNF_CTRL => {
                let keep = if self.firmware_holds_software_flag {
                    v & !phy::EXTCNF_CTRL_SWFLAG
                } else {
                    v
                };
                self.mac.insert(off, keep);
            }
            o if o == non_volatile_memory::FLASH_BASE + non_volatile_memory::HSFSTS => {
                self.flash_write(v);
            }
            o if o == regs::STRAP => self.strap_writes.push(v),
            _ => {
                self.mac.insert(off, v);
            }
        }
    }
    fn read16(&mut self, off: u64) -> u16 {
        self.read(off) as u16
    }
    fn delay_us(&mut self, us: u64) {
        self.elapsed_us += us;
    }
    fn pci_vendor_id(&mut self) -> u16 {
        0x8086
    }
    fn note(&mut self, n: Note) {
        self.notes.push(n);
    }
}

#[cfg(test)]
mod tests {
    use super::super::phy::{Error as PhyError, Swflag, plan, r, reg};
    use super::super::sequence::{self, Error, Reached};
    use super::*;

    fn swflag_free(s: &Sim) -> bool {
        s.get(regs::EXTCNF_CTRL) & phy::EXTCNF_CTRL_SWFLAG == 0
    }

    #[test]
    fn a_paged_register_is_reached_through_a_page_select_at_address_one() {
        assert_eq!(
            plan(r::SMB_CTRL),
            Ok(phy::Plan {
                select: Some((769 << 5) as u16),
                phy: 1,
                register: 23
            })
        );
        // Page 768 is selected as page 0; page 0 lives at address 2 and needs no select.
        assert_eq!(plan(r::SMB_ADDR).unwrap().select, Some(0));
        assert_eq!(
            plan(r::ID1),
            Ok(phy::Plan {
                select: None,
                phy: 2,
                register: 2
            })
        );
        assert_eq!(plan(reg(800, 1)), Err(PhyError::PageNotPorted));
        assert_eq!(plan(reg(5, 17)), Err(PhyError::PageNotPorted));

        let mut s = Sim::new(Start::Healthy, true);
        let f = Swflag::acquire(&mut s).unwrap();
        phy::write(&mut s, &f, r::PM_CTRL, 0x4000).unwrap();
        assert_eq!(phy::read(&mut s, &f, r::PM_CTRL), Ok(0x4000));
        f.release(&mut s);
        assert!(swflag_free(&s));
        // The select went to address 1, register 31, with the page shifted left by five.
        let sel = s.mdic_log[0];
        assert_eq!(sel >> mdic::PHY_SHIFT & 0x1f, 1);
        assert_eq!((sel & mdic::REG_MASK) >> mdic::REG_SHIFT, 31);
        assert_eq!(sel as u16, (770 << 5) as u16);
    }

    #[test]
    fn mdic_failures_are_told_apart() {
        let mut s = Sim::new(Start::Dead, true);
        assert_eq!(
            phy::read_mdic(&mut s, 2, 2),
            Err(PhyError::ManagementDataInterface)
        );
        assert_eq!(phy::read_mdic(&mut s, 2, 32), Err(PhyError::OutOfRange));
        // A device that never raises READY: the poll gives up after FreeBSD's 96 ms.
        struct Mute(u64);
        impl Hardware for Mute {
            fn read(&mut self, _: u64) -> u32 {
                0
            }
            fn write(&mut self, _: u64, _: u32) {}
            fn read16(&mut self, _: u64) -> u16 {
                0
            }
            fn delay_us(&mut self, us: u64) {
                self.0 += us;
            }
            fn pci_vendor_id(&mut self) -> u16 {
                0
            }
            fn note(&mut self, _: Note) {}
        }
        let mut m = Mute(0);
        assert_eq!(phy::read_mdic(&mut m, 1, 2), Err(PhyError::NotReady));
        assert_eq!(m.0, u64::from(mdic::POLLS) * mdic::POLL_US);
        assert_eq!(m.0, 96_000, "FreeBSD's 96 ms: 1920 polls of 50 us");
    }

    /// The edges of MDIO's register field and of the pages this port reaches: register 31 is the
    /// last one there is, page `MAX_PAGE` is the last whose select fits, and register 15 is the last
    /// one reached without a page select.
    #[test]
    fn the_last_register_and_the_last_page_are_reachable_and_one_past_is_not() {
        let mut s = Sim::new(Start::Healthy, true);
        assert!(phy::read_mdic(&mut s, 2, phy::MAX_REG).is_ok());
        assert!(plan(reg(phy::MAX_PAGE, 1)).is_ok());
        assert_eq!(
            plan(reg(phy::MAX_PAGE + 1, 1)),
            Err(PhyError::PageNotPorted)
        );
        assert_eq!(plan(reg(769, phy::MAX_UNPAGED_REG)).unwrap().select, None);
        assert!(
            plan(reg(769, phy::MAX_UNPAGED_REG + 1))
                .unwrap()
                .select
                .is_some()
        );
    }

    /// **The PHY identifier is `ID1:ID2` with the revision nibble cleared**, the path QEMU's
    /// 82574L takes and the one value the gates see from this module.
    #[test]
    fn the_phy_id_is_both_words_without_the_revision() {
        let mut s = Sim::new(Start::Healthy, true);
        assert_eq!(phy::id_at(&mut s, 2), Ok(PHY_ID));
        let mut dead = Sim::new(Start::Dead, true);
        assert_eq!(
            phy::id_at(&mut dead, 2),
            Err(PhyError::ManagementDataInterface)
        );
    }

    /// A firmware block on PHY resets is polled 31 times, 10 ms apart, with no wait after the
    /// last: 300 ms in all, as FreeBSD's `e1000_check_reset_block_ich8lan`. An unblocked one
    /// answers at once.
    #[test]
    fn a_reset_block_is_polled_for_three_hundred_milliseconds() {
        let mut s = Sim::new(Start::Healthy, true);
        assert!(!phy::reset_blocked(&mut s));
        assert_eq!(s.elapsed_us, 0);
        s.mac.insert(regs::FWSM, 0);
        assert!(phy::reset_blocked(&mut s));
        assert_eq!(s.elapsed_us, 300_000);
    }

    /// A refused semaphore is cleared without disturbing the rest of `EXTCNF_CTRL`, and releasing
    /// a flag that is already gone writes nothing.
    #[test]
    fn the_semaphore_touches_only_its_own_bit() {
        let mut s = Sim::new(Start::Healthy, true);
        s.firmware_holds_software_flag = true;
        s.mac.insert(regs::EXTCNF_CTRL, 0x80);
        assert!(Swflag::acquire(&mut s).is_err());
        assert_eq!(s.get(regs::EXTCNF_CTRL), 0x80);

        let mut s = Sim::new(Start::Healthy, true);
        let f = Swflag::acquire(&mut s).unwrap();
        s.mac.insert(regs::EXTCNF_CTRL, 0x80);
        s.writes.clear();
        f.release(&mut s);
        assert!(s.writes.is_empty(), "{:x?}", s.writes);
    }

    #[test]
    fn the_semaphore_refused_by_firmware_is_left_clear_and_a_stuck_one_is_reported() {
        let mut s = Sim::new(Start::Healthy, true);
        s.firmware_holds_software_flag = true;
        assert_eq!(
            Swflag::acquire(&mut s).unwrap_err(),
            PhyError::SwflagRefused
        );
        assert!(swflag_free(&s));
        s.firmware_holds_software_flag = false;
        s.mac.insert(regs::EXTCNF_CTRL, phy::EXTCNF_CTRL_SWFLAG);
        assert_eq!(Swflag::acquire(&mut s).unwrap_err(), PhyError::SwflagHeld);
    }

    #[test]
    fn with_an_me_and_a_healthy_phy_the_phy_is_reached_at_once_and_reset() {
        let mut s = Sim::new(Start::Healthy, true);
        assert_eq!(sequence::init_phy_workarounds(&mut s), Ok(PHY_ID));
        assert!(s.notes.contains(&Note::UlpByMe { cleared: true }));
        assert!(s.notes.contains(&Note::PhyReached {
            at: Reached::Directly,
            id: PHY_ID
        }));
        assert_eq!(s.phy_resets, 1);
        assert_eq!(s.lanphypc_toggles, 0);
        assert_ne!(
            s.get(regs::EXTCNF_CTRL) & 0x80,
            0,
            "hardware PHY config gated"
        );
        assert!(swflag_free(&s));
    }

    #[test]
    fn a_phy_left_in_system_management_bus_mode_is_reached_by_forcing_the_mac() {
        let mut s = Sim::new(Start::InSystemManagementBus, true);
        assert_eq!(sequence::init_phy_workarounds(&mut s), Ok(PHY_ID));
        assert!(s.notes.contains(&Note::PhyReached {
            at: Reached::InSystemManagementBus,
            id: PHY_ID
        }));
        assert_eq!(s.lanphypc_toggles, 0, "no power cycle was needed");
        // With an ME, FreeBSD leaves the unforce to the firmware ("Only unforce SMBus if ME is
        // not active"), and so does this.
        assert_ne!(s.get(mac::CTRL_EXT) & 0x800, 0);
    }

    #[test]
    fn a_dead_phy_is_power_cycled_unless_firmware_forbids_it() {
        let mut s = Sim::new(Start::Dead, true);
        assert_eq!(sequence::init_phy_workarounds(&mut s), Ok(PHY_ID));
        assert_eq!(s.lanphypc_toggles, 1);
        assert!(s.notes.contains(&Note::PhyReached {
            at: Reached::AfterUnforce,
            id: PHY_ID
        }));

        let mut s = Sim::new(Start::Dead, true);
        s.mac.insert(regs::FWSM, super::super::ulp::FWSM_FW_VALID); // RSPCIPHY clear
        assert_eq!(
            sequence::init_phy_workarounds(&mut s),
            Err(Error::LanphypcBlocked)
        );
        assert_eq!(s.lanphypc_toggles, 0);
        assert!(swflag_free(&s), "the error path released the semaphore");
    }

    #[test]
    fn without_an_me_ulp_is_left_by_phy_registers() {
        let mut s = Sim::new(Start::InSystemManagementBus, false);
        s.phy.insert(r::ULP_CONFIG1, 0x1ff4);
        s.phy.insert(r::SMB_CTRL, 0x0001);
        s.mac.insert(regs::FEXTNVM7, 0x20);
        assert_eq!(sequence::init_phy_workarounds(&mut s), Ok(PHY_ID));
        assert!(s.notes.contains(&Note::UlpBySoftware(Ok(()))));
        assert!(s.lanphypc_toggles >= 1);
        assert_eq!(s.phy[&r::SMB_CTRL] & 1, 0);
        assert_ne!(s.phy[&r::PM_CTRL] & 0x4000, 0, "K1 re-enabled");
        let ulp: Vec<u16> = s
            .phy_writes
            .iter()
            .filter(|w| w.0 == r::ULP_CONFIG1)
            .map(|w| w.1)
            .collect();
        let cleared = 0x1ff4 & !0x1d74;
        assert_eq!(ulp, [cleared, cleared | 1], "cleared, then started");
        assert_eq!(s.get(regs::FEXTNVM7) & 0x20, 0);
        assert!(swflag_free(&s));
    }

    #[test]
    fn the_global_reset_takes_the_phy_with_it_only_when_firmware_allows() {
        let mut s = Sim::new(Start::Healthy, true);
        sequence::global_reset(&mut s, 1000).unwrap();
        assert_eq!((s.global_resets, s.phy_resets), (1, 1));
        assert_eq!(s.strap_writes, [0x8086, 0x8086]);
        assert_ne!(s.get(regs::KABGTXD) & 0x0005_0000, 0);
        assert_eq!(
            s.get(mac::STATUS) & 0x600,
            0,
            "LAN_INIT_DONE and PHYRA cleared"
        );
        assert!(swflag_free(&s));

        let mut s = Sim::new(Start::Healthy, true);
        s.mac.insert(regs::FWSM, super::super::ulp::FWSM_FW_VALID);
        sequence::global_reset(&mut s, 1000).unwrap();
        assert_eq!((s.global_resets, s.phy_resets), (1, 0));
    }

    #[test]
    fn a_missing_nvm_fails_the_phy_reset_as_freebsd_does() {
        let mut s = Sim::new(Start::Healthy, true);
        s.non_volatile_memory[non_volatile_memory::SIGNATURE_WORD as usize] = 0xffff;
        assert_eq!(
            sequence::init_phy_workarounds(&mut s),
            Err(Error::NonVolatileMemory(
                non_volatile_memory::Error::NoValidBank
            ))
        );
    }

    #[test]
    fn the_nvm_configuration_region_is_replayed_into_the_phy_from_the_valid_bank() {
        let mut s = Sim::new(Start::Healthy, true);
        let bank = non_volatile_memory::Geometry::read(&mut s).bank_words as usize;
        assert_eq!(bank, 1024);
        // Bank 0 invalid, bank 1 valid.
        s.non_volatile_memory[non_volatile_memory::SIGNATURE_WORD as usize] = 0;
        s.non_volatile_memory[bank + non_volatile_memory::SIGNATURE_WORD as usize] = 0x8000;
        // An extended configuration region at `u32` 0x40 (word 0x80): select page 769, then write
        // 0x1234 to register 17, then 0x0042 to register 25.
        let words = [(769u16 << 5, 0x1f), (0x1234, 17), (0x0042, 25)];
        for (i, (v, at)) in words.iter().enumerate() {
            s.non_volatile_memory[bank + 0x80 + 2 * i] = *v;
            s.non_volatile_memory[bank + 0x80 + 2 * i + 1] = *at;
        }
        s.mac.insert(regs::FEXTNVM, 1 << 27);
        s.mac.insert(regs::EXTCNF_CTRL, 0x40 << 16);
        s.mac.insert(regs::EXTCNF_SIZE, 3 << 16);
        s.mac.insert(regs::LEDCTL, 0x0007_8484);
        sequence::global_reset(&mut s, 1000).unwrap();
        assert!(
            s.notes
                .contains(&Note::LanConnectedDeviceConfigured { pairs: 3 })
        );
        assert_eq!(s.phy[&reg(769, 17)], 0x1234);
        assert_eq!(s.phy[&reg(769, 25)], 0x0042);
        assert_eq!(s.phy[&r::LED_CONFIG], 0x8484);
        // SMBus address 2 from STRAP, PEC and valid set, frequency 3 - 1 = 2 into bit 12.
        assert_eq!(s.phy[&r::SMB_ADDR], 0x0002 | 0x0200 | 0x0080 | 0x1000);
        assert!(swflag_free(&s));
    }

    #[test]
    fn the_copper_link_advertises_every_speed_without_pause_and_restarts_autonegotiation() {
        let mut s = Sim::new(Start::Healthy, true);
        s.phy.insert(r::AUTONEG_ADV, 0x0c01);
        s.phy.insert(r::CTRL_1000T, 0x0100);
        sequence::init_hardware(&mut s).unwrap();
        assert_eq!(s.phy[&r::AUTONEG_ADV], 0x0001 | 0x01e0);
        assert_eq!(s.phy[&r::CTRL_1000T], 0x0200);
        assert_eq!(s.phy[&r::CONTROL], 0x1200);
        assert_ne!(s.get(mac::CTRL) & crate::CTRL_SLU, 0);
        assert_eq!(s.get(regs::TXDCTL0) & 0x013f_003f, 0x0101_001f);
        assert!(swflag_free(&s));
    }

    #[test]
    fn a_word_is_read_from_either_half_of_its_u32() {
        let mut s = Sim::new(Start::Healthy, true);
        s.non_volatile_memory[0x10] = 0xaaaa;
        s.non_volatile_memory[0x11] = 0xbbbb;
        let g = non_volatile_memory::Geometry::read(&mut s);
        assert_eq!(non_volatile_memory::read_word(&mut s, g, 0x10), Ok(0xaaaa));
        assert_eq!(non_volatile_memory::read_word(&mut s, g, 0x11), Ok(0xbbbb));
        assert_eq!(
            non_volatile_memory::read_word(&mut s, g, 2048),
            Err(non_volatile_memory::Error::OutOfRange)
        );
        s.flash_status = 0;
        assert_eq!(
            non_volatile_memory::read_word(&mut s, g, 0x10),
            Err(non_volatile_memory::Error::NoDescriptor)
        );
    }

    #[test]
    fn a_flash_cycle_already_running_is_waited_out_and_one_that_never_ends_is_busy() {
        let mut s = Sim::new(Start::Healthy, true);
        s.non_volatile_memory[0x20] = 0x1234;
        let g = non_volatile_memory::Geometry::read(&mut s);
        s.flash_busy_reads = 5;
        assert_eq!(non_volatile_memory::read_word(&mut s, g, 0x20), Ok(0x1234));
        // `read_word` falls back to bank 0 when the bank probe fails, so ask `read_u32` itself.
        s.flash_busy_reads = 5;
        assert_eq!(
            non_volatile_memory::read_u32(&mut s, 0x20).map(|d| d as u16),
            Ok(0x1234)
        );
        assert_eq!(s.flash_go_while_busy, 0);
        s.flash_busy_reads = u32::MAX;
        assert_eq!(
            non_volatile_memory::read_u32(&mut s, 0x20),
            Err(non_volatile_memory::Error::Busy)
        );
    }

    #[test]
    fn a_failed_flash_cycle_is_retried_and_gives_up_after_freebsds_count() {
        let mut s = Sim::new(Start::Healthy, true);
        s.non_volatile_memory[0x30] = 0xbeef;
        s.flash_failing_cycles = 3;
        assert_eq!(
            non_volatile_memory::read_u32(&mut s, 0x30).map(|d| d as u16),
            Ok(0xbeef)
        );
        s.flash_failing_cycles = non_volatile_memory::CYCLE_REPEATS + 1;
        assert_eq!(
            non_volatile_memory::read_u32(&mut s, 0x30),
            Err(non_volatile_memory::Error::Cycle)
        );
        assert_eq!(
            non_volatile_memory::read_u32_at_byte(
                &mut s,
                non_volatile_memory::LINEAR_ADDRESS_MASK + 1
            ),
            Err(non_volatile_memory::Error::OutOfRange)
        );
    }

    /// The flash status and control bits where FreeBSD's `ich8_hws_flash_status` and
    /// `ich8_hws_flash_ctrl` unions put them.
    #[test]
    fn the_flash_register_bits_are_the_ich8_unions() {
        assert_eq!(non_volatile_memory::status::DONE, 0x0001);
        assert_eq!(non_volatile_memory::status::ERROR, 0x0002);
        assert_eq!(non_volatile_memory::status::DIRECT_ACCESS_ERROR, 0x0004);
        assert_eq!(non_volatile_memory::status::IN_PROGRESS, 0x0020);
        assert_eq!(non_volatile_memory::status::DESCRIPTOR_VALID, 0x4000);
        assert_eq!(non_volatile_memory::control::GO, 0x0001);
        assert_eq!(non_volatile_memory::control::CYCLE_MASK, 0x0006);
        assert_eq!(non_volatile_memory::control::BYTE_COUNT_SHIFT, 8);
        assert_eq!(non_volatile_memory::control::BYTE_COUNT_MASK, 0x0300);
    }

    /// **A flash read touches only the flash registers** and starts one cycle, a read (cycle type
    /// zero) of four bytes (byte count three), which ends without a wait in this model.
    #[test]
    fn a_flash_read_is_one_four_byte_cycle_on_the_flash_registers_alone() {
        let mut s = Sim::new(Start::Healthy, true);
        s.non_volatile_memory[0x20] = 0x1234;
        s.non_volatile_memory[0x21] = 0x5678;
        assert_eq!(non_volatile_memory::read_u32(&mut s, 0x20), Ok(0x5678_1234));
        let flash = non_volatile_memory::FLASH_BASE..non_volatile_memory::FLASH_BASE + 0x100;
        assert!(s.reads.iter().all(|o| flash.contains(o)), "{:x?}", s.reads);
        assert!(
            s.writes.iter().all(|(o, _)| flash.contains(o)),
            "{:x?}",
            s.writes
        );
        assert_eq!(s.flash_cycles.len(), 1);
        let ctl = s.flash_cycles[0];
        assert_eq!(ctl & 0x0300, 0x0300, "four bytes");
        assert_eq!(ctl & 0x0006, 0, "a read");
        assert_eq!(
            s.elapsed_us, 1,
            "one microsecond before the cycle, none in it"
        );
        // The last byte the linear address reaches is still in range.
        assert!(
            non_volatile_memory::read_u32_at_byte(&mut s, non_volatile_memory::LINEAR_ADDRESS_MASK)
                .is_ok()
        );
    }

    /// A cycle that takes a few status reads to finish is waited for, one microsecond a poll.
    #[test]
    fn a_slow_flash_cycle_is_waited_for() {
        let mut s = Sim::new(Start::Healthy, true);
        s.non_volatile_memory[0x20] = 0x1234;
        s.flash_slow_reads = 3;
        assert_eq!(
            non_volatile_memory::read_u32(&mut s, 0x20).map(|d| d as u16),
            Ok(0x1234)
        );
        assert_eq!(s.flash_cycles.len(), 1);
        assert_eq!(s.elapsed_us, 1 + 2);
    }

    /// **Errors left set by an earlier cycle are cleared before the next one starts** (they are
    /// write-one-to-clear), so the next read needs one cycle, not a failed one and a retry.
    #[test]
    fn stale_flash_errors_are_cleared_before_the_cycle() {
        let mut s = Sim::new(Start::Healthy, true);
        s.non_volatile_memory[0x20] = 0x1234;
        // FLCERR (bit 1) and the direct-access error bit (bit 2), as numbers so a wrong constant cannot agree.
        s.flash_status |= 0x0002 | 0x0004;
        assert_eq!(
            non_volatile_memory::read_u32(&mut s, 0x20).map(|d| d as u16),
            Ok(0x1234)
        );
        assert_eq!(s.flash_cycles.len(), 1);
        assert_eq!(s.flash_status & (0x0002 | 0x0004), 0);
    }

    /// **A cycle someone else started is waited out, and its `FDONE` and `FLCERR` cleared,
    /// before ours starts.** Starting ours early would be ignored by the device; leaving its
    /// `FDONE` set would end ours at the first poll and read the other cycle's data.
    #[test]
    fn a_running_cycle_is_waited_out_and_its_status_cleared() {
        let mut s = Sim::new(Start::Healthy, true);
        s.non_volatile_memory[0x30] = 0x2222;
        s.non_volatile_memory[0x31] = 0x3333;
        // A foreign cycle at FADDR 0 (erased, 0xffff) ends, with an error, two reads from now.
        s.flash_pending = 2;
        s.flash_failing_cycles = 1;
        s.flash_slow_reads = 3;
        assert_eq!(non_volatile_memory::read_u32(&mut s, 0x30), Ok(0x3333_2222));
        assert_eq!(s.flash_go_while_busy, 0);
        assert_eq!(s.flash_cycles.len(), 1, "ours ran once and did not fail");
    }

    /// **A cycle that never finishes is a cycle error after FreeBSD's bound, and is not
    /// retried**: only an error is worth a second cycle, and a hung flash would only hang again.
    #[test]
    fn a_flash_cycle_that_never_ends_fails_once_and_is_not_retried() {
        let mut s = Sim::new(Start::Healthy, true);
        s.flash_slow_reads = u32::MAX;
        assert_eq!(
            non_volatile_memory::read_u32(&mut s, 0x20),
            Err(non_volatile_memory::Error::Cycle)
        );
        assert_eq!(s.flash_cycles.len(), 1);
    }

    /// The NVM's size is `STRAP` bits 5:1 plus one, in 4 KiB units, over two banks.
    #[test]
    fn the_nvm_size_comes_from_strap_bits_five_to_one() {
        assert_eq!(
            non_volatile_memory::Geometry::from_strap(STRAP_VALUE).bank_words,
            1024
        );
        assert_eq!(
            non_volatile_memory::Geometry::from_strap(1 << 1).bank_words,
            2048
        );
        assert_eq!(
            non_volatile_memory::Geometry::from_strap(0x1f << 1).bank_words,
            32 * 1024
        );
        // Bit 0 and bit 6 are not the size.
        assert_eq!(
            non_volatile_memory::Geometry::from_strap(1 | 1 << 6).bank_words,
            1024
        );
    }

    #[test]
    fn neither_bank_valid_is_reported_and_a_word_read_falls_back_to_bank_zero() {
        let mut s = Sim::new(Start::Healthy, true);
        s.non_volatile_memory[non_volatile_memory::SIGNATURE_WORD as usize] = 0;
        s.non_volatile_memory[0x40] = 0x5555;
        let g = non_volatile_memory::Geometry::read(&mut s);
        assert_eq!(
            non_volatile_memory::valid_bank(&mut s, g),
            Err(non_volatile_memory::Error::NoValidBank)
        );
        assert_eq!(non_volatile_memory::read_word(&mut s, g, 0x40), Ok(0x5555));
    }
}
