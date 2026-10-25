//! **Is device DMA coherent with the CPU's caches on this machine? Measured, not assumed.**
//!
//! Once per bring-up, before any frame is trusted, with the controller in MAC loopback and only
//! ordinary cached stores and the data plane's barrier: post the receive ring, fill receive buffer
//! 0 with [`POISON`] and **read it back** (so a write-back cache the device does not snoop holds
//! the poison when the device writes), queue [`probe_frame`], poll receive descriptor 0 for up to
//! [`WAIT_US`], then read the channel status register. [`classify`] compares the two accounts; the
//! status register is read over MMIO, which no cache stands in front of, so a non-coherent machine
//! fails in a way that names the direction.

use crate::{
    CRC_BYTES, DataPlane, RDES3_OWN, RX_ENTRIES, Region, RxVerdict, TDES3_OWN, Tails, TxError,
    layout, regs, rx_verdict,
};

/// The probe frame's `EtherType`: IEEE 802's first local experimental type, which no real
/// protocol uses and no switch acts on (it never leaves the MAC anyway).
pub(crate) const PROBE_ETHERTYPE: u16 = 0x88b5;
/// The probe frame's length, CRC excluded: comfortably more than one cache line, so a stale line
/// anywhere in it is seen.
pub(crate) const PROBE_LEN: usize = 128;
/// What receive buffer 0 holds before the device writes it. Never a byte of [`probe_frame`]'s
/// payload, so a buffer still holding it cannot be mistaken for a delivery.
pub(crate) const POISON: u8 = 0xa5;
/// How long the caller waits for receive descriptor 0 before reading the device's account. A
/// 128-byte frame at any speed through a loopback is microseconds; a hundred milliseconds is
/// slack for a slow first descriptor fetch, not a guess at the wire.
pub const WAIT_US: u64 = 100_000;

/// **The probe frame**: addressed from and to `mac` (so the packet filter passes it), the
/// experimental `EtherType`, and a payload whose every byte differs from [`POISON`] and from zero.
pub(crate) fn probe_frame(mac: [u8; 6]) -> [u8; PROBE_LEN] {
    let mut f = [0u8; PROBE_LEN];
    f[..6].copy_from_slice(&mac);
    f[6..12].copy_from_slice(&mac);
    f[12..14].copy_from_slice(&PROBE_ETHERTYPE.to_be_bytes());
    for (i, b) in f[14..].iter_mut().enumerate() {
        // 0x40 to 0x7f, repeating: never 0xa5 and never zero.
        *b = 0x40 | (i as u8 & 0x3f);
    }
    f
}

/// **Steps 1 to 3**: post the receive ring, poison and read back receive buffer 0, queue the
/// probe frame. The controller must already be configured and started in loopback; the receive
/// tail write in step 1 and the transmit tail write in step 3 are what set it moving.
pub fn prepare(
    dp: &mut DataPlane,
    region: &mut impl Region,
    tails: &mut impl Tails,
    mac: [u8; 6],
) -> Result<(), TxError> {
    dp.start(region, tails);
    let poison = [POISON; PROBE_LEN + CRC_BYTES];
    region.write_bytes(layout::rx_buffer(0), &poison);
    // A copy of the poison, so the read-back covers exactly the bytes poisoned: a second length
    // written out here could drift from the first, and `sim::Memory`'s write-allocate cache would
    // hide a short one.
    let mut back = poison;
    region.read_bytes(layout::rx_buffer(0), &mut back);
    dp.transmit(region, tails, &probe_frame(mac))
}

/// **Step 4's poll**: has the CPU seen receive descriptor 0 handed back?
pub fn delivered(region: &impl Region) -> bool {
    (region.read_u64(layout::rx_descriptor(0) + 8) >> 32) as u32 & RDES3_OWN == 0
}

/// What receive buffer 0 held when it was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payload {
    /// The probe frame, byte for byte.
    Matches,
    /// The poison, untouched: the device's write is not what the CPU read.
    Poison,
    /// Something else: the device sent bytes the CPU did not write.
    Other,
}

/// Everything the probe saw: the CPU's view of the ring and buffer, and the device's view of
/// itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observation {
    /// Receive descriptor 0's word 3, read by the CPU.
    pub rx_des3: u32,
    /// Transmit descriptor 0's word 3, read by the CPU.
    pub tx_des3: u32,
    /// [`regs::CHAN_STATUS`], read through MMIO.
    pub status: u32,
    /// Receive buffer 0, judged.
    pub payload: Payload,
}

/// **Read the CPU's half of the observation** and pair it with the device's status word.
pub fn observe(region: &impl Region, mac: [u8; 6], status: u32) -> Observation {
    let rx_des3 = (region.read_u64(layout::rx_descriptor(0) + 8) >> 32) as u32;
    let tx_des3 = (region.read_u64(layout::tx_descriptor(0) + 8) >> 32) as u32;
    let mut got = [0u8; PROBE_LEN];
    region.read_bytes(layout::rx_buffer(0), &mut got);
    let payload = if got == probe_frame(mac) {
        Payload::Matches
    } else if got.iter().all(|&b| b == POISON) {
        Payload::Poison
    } else {
        Payload::Other
    };
    Observation {
        rx_des3,
        tx_des3,
        status,
        payload,
    }
}

/// The probe's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// **Coherent in both directions.** The device read descriptors and a payload the CPU had
    /// only written into its cache, and the CPU read a descriptor and a payload over lines it had
    /// cached before the device wrote them, all byte for byte.
    Coherent,
    /// The device reported descriptors unavailable without completing them: it fetched what was
    /// in memory, not what the CPU had written. **CPU-to-device is not coherent**: a clean is
    /// needed before every tail write.
    DeviceCannotSeeCpuWrites,
    /// The device reports a frame received and the CPU still sees the descriptor as the
    /// device's. **Device-to-CPU is not coherent**: an invalidate is needed before every read.
    CpuCannotSeeDeviceWrites,
    /// The CPU sees the write-back, and the buffer still holds the poison: descriptors and data
    /// disagree, so device-to-CPU is not coherent for at least the buffer's lines.
    ReceivedPayloadStale,
    /// A frame arrived whole and it is not the frame the CPU wrote: the device transmitted what
    /// was in memory rather than what was in the cache.
    TransmittedPayloadStale,
    /// The DMA reported a fatal bus error: an address the interconnect refused. Not a coherence
    /// answer at all; the region or the bus setup is wrong.
    BusError,
    /// Nothing completed and nothing was refused: the loopback never ran. No answer, and the
    /// driver says so rather than assuming either.
    Inconclusive,
}

/// **Compare the two accounts.**
pub fn classify(o: &Observation) -> Verdict {
    let s = o.status;
    if s & regs::CHAN_STATUS_FBE != 0 {
        return Verdict::BusError;
    }
    if o.rx_des3 & RDES3_OWN == 0 {
        return match (o.payload, rx_verdict(o.rx_des3, PROBE_LEN)) {
            (Payload::Matches, RxVerdict::Frame(PROBE_LEN)) => Verdict::Coherent,
            (Payload::Poison, _) => Verdict::ReceivedPayloadStale,
            _ => Verdict::TransmittedPayloadStale,
        };
    }
    if s & regs::CHAN_STATUS_RI != 0 {
        return Verdict::CpuCannotSeeDeviceWrites;
    }
    let tx_refused = s & regs::CHAN_STATUS_TBU != 0 && s & regs::CHAN_STATUS_TI == 0;
    let rx_refused = s & regs::CHAN_STATUS_RBU != 0;
    if tx_refused || rx_refused {
        return Verdict::DeviceCannotSeeCpuWrites;
    }
    Verdict::Inconclusive
}

/// Whether the transmit descriptor was handed back, for a report line.
pub const fn is_transmit_complete(o: &Observation) -> bool {
    o.tx_des3 & TDES3_OWN == 0
}

/// How many receive descriptors the probe posts, for a report line: every one but the held-back
/// slot, as [`DataPlane::start`] does.
pub const POSTED: u16 = RX_ENTRIES - 1;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::{self, Identity};
    use crate::jh7110::BUS;
    use crate::motorcomm::{Link, Speed};
    use crate::sim::{Memory, Nic};
    use crate::{Hw, RDES3_FD, RDES3_LD};

    const PHYS: u64 = 0x4800_0000;
    const MAC: [u8; 6] = [0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c];
    const GIG: Link = Link {
        speed: Speed::Thousand,
        full_duplex: true,
    };

    /// The whole probe, as the kernel runs it, against a controller whose memory behaves as
    /// `memory` says and whose loopback works when `loops` is true.
    fn run(memory: Memory, loops: bool) -> Verdict {
        let mut nic = Nic::new(PHYS, memory);
        if !loops {
            nic.break_loopback();
        }
        let id = Identity::read(&mut nic);
        controller::configure(&mut nic, &id, &BUS, PHYS, MAC).unwrap();
        controller::start(&mut nic, GIG, true);
        let mut dp = DataPlane::new(PHYS);
        nic.with(|r, t| prepare(&mut dp, r, t, MAC)).unwrap();
        let mut waited = 0;
        while !nic.with(|r, _| delivered(r)) && waited < WAIT_US {
            nic.delay_us(1000);
            waited += 1000;
        }
        let status = nic.read(regs::CHAN_STATUS);
        let o = nic.with(|r, _| observe(r, MAC, status));
        classify(&o)
    }

    #[test]
    fn a_coherent_machine_reads_coherent() {
        assert_eq!(run(Memory::coherent(PHYS), true), Verdict::Coherent);
    }

    #[test]
    fn a_write_back_cache_the_device_does_not_snoop_is_caught_on_the_way_out() {
        assert_eq!(
            run(Memory::write_back_unsnooped(PHYS), true),
            Verdict::DeviceCannotSeeCpuWrites
        );
    }

    #[test]
    fn a_cache_the_device_writes_around_is_caught_on_the_way_in() {
        assert_eq!(
            run(Memory::write_through_unsnooped(PHYS), true),
            Verdict::CpuCannotSeeDeviceWrites
        );
    }

    #[test]
    fn a_loopback_that_never_runs_is_inconclusive_not_coherent() {
        assert_eq!(run(Memory::coherent(PHYS), false), Verdict::Inconclusive);
    }

    #[test]
    fn the_probe_frame_is_never_mistaken_for_poison_or_silence() {
        let f = probe_frame(MAC);
        assert!(f[14..].iter().all(|&b| b != POISON && b != 0));
        assert_eq!(&f[..6], &MAC);
        assert_eq!(u16::from_be_bytes([f[12], f[13]]), PROBE_ETHERTYPE);
    }

    #[test]
    fn a_whole_frame_that_is_not_the_one_written_is_a_stale_transmit() {
        let o = Observation {
            rx_des3: RDES3_FD | RDES3_LD | (PROBE_LEN + CRC_BYTES) as u32,
            tx_des3: 0,
            status: regs::CHAN_STATUS_RI | regs::CHAN_STATUS_TI,
            payload: Payload::Other,
        };
        assert_eq!(classify(&o), Verdict::TransmittedPayloadStale);
        let o = Observation {
            payload: Payload::Poison,
            ..o
        };
        assert_eq!(classify(&o), Verdict::ReceivedPayloadStale);
        let o = Observation {
            status: o.status | regs::CHAN_STATUS_FBE,
            ..o
        };
        assert_eq!(classify(&o), Verdict::BusError);
    }

    #[test]
    fn an_ordinary_transmit_suspend_after_a_completion_is_not_a_refusal() {
        // A transmit DMA that sent its one frame then found the next slot empty sets TI and TBU
        // together; that is success, and only the receive side's silence makes it inconclusive.
        let o = Observation {
            rx_des3: RDES3_OWN,
            tx_des3: 0,
            status: regs::CHAN_STATUS_TI | regs::CHAN_STATUS_TBU,
            payload: Payload::Poison,
        };
        assert_eq!(classify(&o), Verdict::Inconclusive);
        assert!(is_transmit_complete(&o));
    }

    /// The payload bytes run `0x40` to `0x7f` and repeat: the bytes `the_probe_frame_is_never_
    /// mistaken_for_poison_or_silence` relies on, pinned at both ends of a lap.
    #[test]
    fn the_payload_counts_from_0x40_and_wraps_at_0x7f() {
        let f = probe_frame(MAC);
        assert_eq!(
            [f[14], f[15], f[14 + 63], f[14 + 64]],
            [0x40, 0x41, 0x7f, 0x40]
        );
    }

    /// **Step 2 poisons the frame and its CRC, and nothing past them**, so whatever the device
    /// writes into those bytes replaces poison, and the read-back covers every line it touches.
    /// Step 3 has queued the frame and the device has not run yet: descriptor 0 is still its.
    #[test]
    fn prepare_poisons_exactly_the_frame_and_its_crc_and_waits_on_descriptor_0() {
        let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
        let id = Identity::read(&mut nic);
        controller::configure(&mut nic, &id, &BUS, PHYS, MAC).unwrap();
        controller::start(&mut nic, GIG, true);
        let mut dp = DataPlane::new(PHYS);
        nic.with(|r, t| prepare(&mut dp, r, t, MAC)).unwrap();
        nic.with(|r, _| {
            let mut b = [0u8; PROBE_LEN + CRC_BYTES + 1];
            r.read_bytes(layout::rx_buffer(0), &mut b);
            assert!(b[..PROBE_LEN + CRC_BYTES].iter().all(|&x| x == POISON));
            assert_eq!(b[PROBE_LEN + CRC_BYTES], 0);
            assert!(!delivered(r));
        });
        assert_eq!(nic.rx_available(), usize::from(POSTED));
        assert_eq!(POSTED, 15);
    }

    /// After a coherent loopback, the CPU sees receive descriptor 0 handed back and transmit
    /// descriptor 0's write-back: first and last, its length, no ownership.
    #[test]
    fn a_completed_loopback_is_seen_on_both_rings() {
        let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
        let id = Identity::read(&mut nic);
        controller::configure(&mut nic, &id, &BUS, PHYS, MAC).unwrap();
        controller::start(&mut nic, GIG, true);
        let mut dp = DataPlane::new(PHYS);
        nic.with(|r, t| prepare(&mut dp, r, t, MAC)).unwrap();
        nic.step();
        let o = nic.with(|r, _| {
            assert!(delivered(r));
            observe(r, MAC, 0)
        });
        assert_eq!(
            o.tx_des3,
            crate::TDES3_FS | crate::TDES3_LS | PROBE_LEN as u32
        );
        assert!(is_transmit_complete(&o));
        assert_eq!(o.payload, Payload::Matches);
        assert!(!is_transmit_complete(&Observation {
            tx_des3: crate::TDES3_OWN,
            ..o
        }));
    }

    /// A buffer of nothing but poison reads as poison, and one of zeros as neither poison nor
    /// the frame.
    #[test]
    fn a_buffer_is_judged_poison_only_when_every_byte_is() {
        let mut nic = Nic::new(PHYS, Memory::coherent(PHYS));
        let judged = |nic: &mut Nic, fill: u8| {
            nic.with(|r, _| {
                r.write_bytes(layout::rx_buffer(0), &[fill; PROBE_LEN]);
                observe(r, MAC, 0).payload
            })
        };
        assert_eq!(judged(&mut nic, POISON), Payload::Poison);
        assert_eq!(judged(&mut nic, 0), Payload::Other);
    }

    /// Silence from the device, with no completion, is no answer: a transmit side that neither
    /// completed nor reported a descriptor unavailable has refused nothing.
    #[test]
    fn a_status_of_nothing_is_inconclusive() {
        let o = Observation {
            rx_des3: RDES3_OWN,
            tx_des3: TDES3_OWN,
            status: 0,
            payload: Payload::Poison,
        };
        assert_eq!(classify(&o), Verdict::Inconclusive);
    }
}
