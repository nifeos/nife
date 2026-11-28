//! **Where a received Transmission Control Protocol (TCP) stream's time goes**, as distributions the network bench boot prints:
//! when each data segment reaches the ring, how many frames each poll of the stack takes, what the
//! waits between looks cost, and the receive window the stack advertises against the room the peer
//! has left when its next segment lands. The program supplies the clock in microseconds and calls
//! these on its receive and transmit paths; only a bench build does (`net_stack`'s
//! `receive_trace`).
//!
//! Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.

use core::fmt::{self, Write};

/// How many buckets a [`Spread`] keeps: one for zero, then one per power of two up to 2^30.
const BUCKETS: usize = 32;

/// **A distribution of one quantity**: its count, sum and largest value, and a count per power
/// of two. Bucket 0 holds zero and bucket `k` holds `2^(k-1) ..= 2^k - 1`, so a percentile is
/// known to within a factor of two, which is what telling a 20 us wait from a 3 ms one needs.
///
/// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spread {
    buckets: [u32; BUCKETS],
    count: u64,
    sum: u64,
    maximum: u64,
}

impl Default for Spread {
    fn default() -> Self {
        Self::new()
    }
}

impl Spread {
    /// An empty distribution.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub const fn new() -> Self {
        Self {
            buckets: [0; BUCKETS],
            count: 0,
            sum: 0,
            maximum: 0,
        }
    }

    /// Count one value.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn record(&mut self, value: u64) {
        let bucket = ((u64::BITS - value.leading_zeros()) as usize).min(BUCKETS - 1);
        self.buckets[bucket] = self.buckets[bucket].saturating_add(1);
        self.count += 1;
        self.sum = self.sum.saturating_add(value);
        self.maximum = self.maximum.max(value);
    }

    /// Count every value `other` counted.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn merge(&mut self, other: &Spread) {
        for (mine, theirs) in self.buckets.iter_mut().zip(other.buckets) {
            *mine = mine.saturating_add(theirs);
        }
        self.count += other.count;
        self.sum = self.sum.saturating_add(other.sum);
        self.maximum = self.maximum.max(other.maximum);
    }

    /// How many values were counted.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub const fn count(&self) -> u64 {
        self.count
    }

    /// The bound below which `percent` of the values lie, as the top of their bucket: `None` while
    /// nothing is counted.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn below(&self, percent: u64) -> Option<u64> {
        let wanted = (self.count * percent).div_ceil(100).max(1);
        let mut seen = 0u64;
        for (k, &n) in self.buckets.iter().enumerate() {
            seen += u64::from(n);
            if seen >= wanted {
                return Some(if k == 0 { 0 } else { (1u64 << k) - 1 });
            }
        }
        None
    }

    /// One line: `label: n N, mean M, p50 <= A, p90 <= B, p99 <= C, max X unit | from:count ...`,
    /// where each `from:count` is a non-empty bucket and its lowest value.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn write_line(&self, label: &str, unit: &str, out: &mut impl Write) -> fmt::Result {
        let Some(p50) = self.below(50) else {
            return writeln!(out, "{label}: none");
        };
        write!(
            out,
            "{label}: n {}, mean {}, p50 <= {p50}, p90 <= {}, p99 <= {}, max {} {unit} |",
            self.count,
            self.sum / self.count,
            self.below(90).unwrap_or(0),
            self.below(99).unwrap_or(0),
            self.maximum,
        )?;
        for (k, &n) in self.buckets.iter().enumerate() {
            if n != 0 {
                let from = if k == 0 { 0 } else { 1u64 << (k - 1) };
                write!(out, " {from}:{n}")?;
            }
        }
        writeln!(out)
    }
}

/// **Text written into a fixed buffer**, for a program with no heap. Writing past the end keeps
/// what fits and fails the write.
///
/// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
#[derive(Debug, Clone)]
pub struct Text<const N: usize> {
    bytes: [u8; N],
    used: usize,
}

impl<const N: usize> Default for Text<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Text<N> {
    /// An empty buffer.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            used: 0,
        }
    }

    /// What has been written.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.used]
    }
}

impl<const N: usize> Write for Text<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let room = N - self.used;
        let take = s.len().min(room);
        self.bytes[self.used..self.used + take].copy_from_slice(&s.as_bytes()[..take]);
        self.used += take;
        if take == s.len() {
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}

/// The fields of a TCP segment over Internet Protocol version 4 (IPv4) in an Ethernet frame that the trace reads.
///
/// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    /// The sequence number of the first payload byte.
    pub sequence: u32,
    /// The acknowledgment number, meaningful when [`Segment::acknowledges`] holds.
    pub acknowledgment: u32,
    /// The acknowledgment (ACK) flag.
    pub acknowledges: bool,
    /// The window field, unscaled.
    pub window: u16,
    /// How many payload bytes the segment carries.
    pub payload_bytes: u32,
}

/// Read `frame` as an untagged Ethernet II frame carrying IPv4 carrying TCP, or `None` if it is
/// anything else or too short to say.
///
/// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
pub fn tcp_segment(frame: &[u8]) -> Option<Segment> {
    const ETHERNET_HEADER_BYTES: usize = 14;
    const TCP: u8 = 6;
    let be16 = |at: usize| Some(u16::from_be_bytes([*frame.get(at)?, *frame.get(at + 1)?]));
    let be32 = |at: usize| {
        Some(u32::from_be_bytes([
            *frame.get(at)?,
            *frame.get(at + 1)?,
            *frame.get(at + 2)?,
            *frame.get(at + 3)?,
        ]))
    };
    if be16(12)? != 0x0800 {
        return None;
    }
    let packet = ETHERNET_HEADER_BYTES;
    let version_and_length = *frame.get(packet)?;
    if version_and_length >> 4 != 4 || *frame.get(packet + 9)? != TCP {
        return None;
    }
    let packet_header = usize::from(version_and_length & 0xf) * 4;
    let packet_total = usize::from(be16(packet + 2)?);
    let tcp = packet + packet_header;
    let tcp_header = usize::from(*frame.get(tcp + 12)? >> 4) * 4;
    let payload = packet_total.checked_sub(packet_header + tcp_header)?;
    Some(Segment {
        sequence: be32(tcp + 4)?,
        acknowledgment: be32(tcp + 8)?,
        acknowledges: *frame.get(tcp + 13)? & 0x10 != 0,
        window: be16(tcp + 14)?,
        payload_bytes: payload as u32,
    })
}

/// **The trace of one receiving stack**: every distribution the bench prints for the server side,
/// fed by the program's receive, transmit, poll and wait paths with times in microseconds.
///
/// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
#[derive(Debug, Clone, Default)]
pub struct Receiver {
    /// Microseconds between data segments taken from the ring.
    pub gap: Spread,
    /// Frames taken by each poll of the stack that took any.
    pub frames_per_poll: Spread,
    /// Microseconds each such poll spent per frame it took: the stack's own work per frame.
    pub poll_per_frame: Spread,
    /// Microseconds a yield between looks at the ring took, counting only the yields before a
    /// data segment, so the wait after the stream's last one is left out.
    pub yields: Spread,
    /// Microseconds a timed sleep between looks took, counted the same way.
    pub sleeps: Spread,
    /// Microseconds from a receive request to the first data segment taken while serving it.
    pub request_to_frame: Spread,
    /// Microseconds from a receive request to its reply being ready.
    pub request_to_reply: Spread,
    /// The window each acknowledgment advertised, in bytes.
    pub advertised: Spread,
    /// Bytes the peer could still have sent, past the end of each data segment, under the window
    /// last advertised: near zero means the peer was out of window.
    pub room: Spread,
    /// Microseconds from an acknowledgment to the next data segment, for the first data segment
    /// after each acknowledgment: the peer's turnaround plus the wire, if the peer waits on it.
    pub acknowledgment_to_data: Spread,
    /// Frames taken from the ring that were not TCP data.
    pub other_frames: u64,
    last_data_microseconds: Option<u64>,
    last_acknowledgment: Option<(u64, u32, u16)>,
    acknowledged_since_data: bool,
    frames_this_poll: u64,
    request_microseconds: Option<u64>,
    yields_since_data: Spread,
    sleeps_since_data: Spread,
}

impl Receiver {
    /// An empty trace.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub const fn new() -> Self {
        Self {
            gap: Spread::new(),
            frames_per_poll: Spread::new(),
            poll_per_frame: Spread::new(),
            yields: Spread::new(),
            sleeps: Spread::new(),
            request_to_frame: Spread::new(),
            request_to_reply: Spread::new(),
            advertised: Spread::new(),
            room: Spread::new(),
            acknowledgment_to_data: Spread::new(),
            other_frames: 0,
            last_data_microseconds: None,
            last_acknowledgment: None,
            acknowledged_since_data: false,
            frames_this_poll: 0,
            request_microseconds: None,
            yields_since_data: Spread::new(),
            sleeps_since_data: Spread::new(),
        }
    }

    /// A frame was taken from the ring at `now`.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn received(&mut self, now: u64, frame: &[u8]) {
        self.frames_this_poll += 1;
        let Some(segment) = tcp_segment(frame).filter(|s| s.payload_bytes != 0) else {
            self.other_frames += 1;
            return;
        };
        if let Some(asked) = self.request_microseconds.take() {
            self.request_to_frame.record(now.saturating_sub(asked));
        }
        self.yields
            .merge(&core::mem::take(&mut self.yields_since_data));
        self.sleeps
            .merge(&core::mem::take(&mut self.sleeps_since_data));
        if let Some(previous) = self.last_data_microseconds {
            self.gap.record(now.saturating_sub(previous));
        }
        self.last_data_microseconds = Some(now);
        if let Some((sent, acknowledged, window)) = self.last_acknowledgment {
            let edge = acknowledged.wrapping_add(u32::from(window));
            let end = segment.sequence.wrapping_add(segment.payload_bytes);
            self.room
                .record((edge.wrapping_sub(end) as i32).max(0) as u64);
            if self.acknowledged_since_data {
                self.acknowledgment_to_data.record(now.saturating_sub(sent));
            }
        }
        self.acknowledged_since_data = false;
    }

    /// A frame was handed to the NIC at `now`.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn transmitted(&mut self, now: u64, frame: &[u8]) {
        let Some(segment) = tcp_segment(frame).filter(|s| s.acknowledges) else {
            return;
        };
        self.advertised.record(u64::from(segment.window));
        self.last_acknowledgment = Some((now, segment.acknowledgment, segment.window));
        self.acknowledged_since_data = true;
    }

    /// A poll of the stack ran from `started` to `now`.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn polled(&mut self, started: u64, now: u64) {
        let frames = core::mem::take(&mut self.frames_this_poll);
        if frames != 0 {
            self.frames_per_poll.record(frames);
            self.poll_per_frame
                .record(now.saturating_sub(started) / frames);
        }
    }

    /// A wait between looks at the ring took `microseconds`: a yield, or else a timed sleep.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn waited(&mut self, yielded: bool, microseconds: u64) {
        if yielded {
            self.yields_since_data.record(microseconds);
        } else {
            self.sleeps_since_data.record(microseconds);
        }
    }

    /// A receive request arrived at `now`.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn requested(&mut self, now: u64) {
        self.request_microseconds = Some(now);
    }

    /// The request that arrived at `asked` has its reply ready at `now`. A request served from
    /// bytes already buffered took no data segment, and its `request_to_frame` is not counted.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn replied(&mut self, asked: u64, now: u64) {
        self.request_microseconds = None;
        self.request_to_reply.record(now.saturating_sub(asked));
    }

    /// Every distribution, one line each, each line starting `label`.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn write_summary(&self, out: &mut impl Write) -> fmt::Result {
        writeln!(
            out,
            "server    : {} data segments, {} other frames, {} acknowledgments sent",
            self.gap.count() + u64::from(self.last_data_microseconds.is_some()),
            self.other_frames,
            self.advertised.count(),
        )?;
        let lines: [(&str, &Spread, &str); 10] = [
            ("gap       ", &self.gap, "us"),
            ("per poll  ", &self.frames_per_poll, "frames"),
            ("poll cost ", &self.poll_per_frame, "us/frame"),
            ("yield     ", &self.yields, "us"),
            ("sleep     ", &self.sleeps, "us"),
            ("to frame  ", &self.request_to_frame, "us"),
            ("to reply  ", &self.request_to_reply, "us"),
            ("advertised", &self.advertised, "bytes"),
            ("room      ", &self.room, "bytes"),
            ("ack->data ", &self.acknowledgment_to_data, "us"),
        ];
        for (label, spread, unit) in lines {
            spread.write_line(label, unit, out)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An Ethernet II, IPv4 (no options) and TCP (no options) frame, as a peer's data segment or
    /// our acknowledgment would be.
    fn frame(
        sequence: u32,
        acknowledgment: u32,
        flags: u8,
        window: u16,
        payload: usize,
    ) -> Vec<u8> {
        let mut f = vec![0u8; 14 + 20 + 20 + payload];
        f[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
        f[14] = 0x45;
        f[16..18].copy_from_slice(&((20 + 20 + payload) as u16).to_be_bytes());
        f[23] = 6;
        let t = 34;
        f[t + 4..t + 8].copy_from_slice(&sequence.to_be_bytes());
        f[t + 8..t + 12].copy_from_slice(&acknowledgment.to_be_bytes());
        f[t + 12] = 5 << 4;
        f[t + 13] = flags;
        f[t + 14..t + 16].copy_from_slice(&window.to_be_bytes());
        f
    }

    /// The offsets: a wrong one would print plausible windows that mean nothing.
    #[test]
    fn a_segment_reads_back_its_own_fields() {
        let s = tcp_segment(&frame(7, 0x1234_5678, 0x10, 16384, 1460)).unwrap();
        assert_eq!(
            s,
            Segment {
                sequence: 7,
                acknowledgment: 0x1234_5678,
                acknowledges: true,
                window: 16384,
                payload_bytes: 1460,
            }
        );
        let mut not_internet_protocol = frame(0, 0, 0, 0, 0);
        not_internet_protocol[12..14].copy_from_slice(&0x0806u16.to_be_bytes());
        assert_eq!(tcp_segment(&not_internet_protocol), None);
        assert_eq!(tcp_segment(&not_internet_protocol[..20]), None);
    }

    /// A peer that sends to the edge of the window shows zero room; the first segment after an
    /// acknowledgment, and only that one, times the turnaround.
    #[test]
    fn room_and_turnaround_follow_the_last_acknowledgment() {
        let mut r = Receiver::new();
        r.transmitted(100, &frame(1, 1000, 0x10, 2920, 0));
        r.received(150, &frame(1000, 1, 0x10, 0, 1460));
        r.received(160, &frame(2460, 1, 0x10, 0, 1460));
        assert_eq!(r.room.count(), 2);
        assert_eq!(r.room.below(100), Some(2047)); // 1460, then 0
        assert_eq!(r.room.below(50), Some(0));
        assert_eq!(r.acknowledgment_to_data.count(), 1);
        assert_eq!(r.acknowledgment_to_data.below(100), Some(63)); // 50 us
        assert_eq!(r.gap.count(), 1);
    }

    #[test]
    fn a_percentile_is_the_top_of_its_bucket() {
        let mut s = Spread::new();
        for v in [0, 1, 300, 300, 300, 300, 300, 300, 300, 5000] {
            s.record(v);
        }
        assert_eq!(s.below(10), Some(0));
        assert_eq!(s.below(50), Some(511));
        assert_eq!(s.below(100), Some(8191));
        let mut line = String::new();
        s.write_line("x", "us", &mut line).unwrap();
        assert_eq!(
            line,
            "x: n 10, mean 710, p50 <= 511, p90 <= 511, p99 <= 8191, max 5000 us | 0:1 1:1 256:7 4096:1\n"
        );
    }

    /// The stream's tail, a peer that sent its last byte and has not closed, is waits with no
    /// data after them; they must not land in the yield and sleep lines, or a run that ends in a
    /// 15 s wait reads as a stack that slept through the stream.
    #[test]
    fn waits_after_the_last_segment_are_not_counted() {
        let mut r = Receiver::new();
        r.waited(false, 10_000);
        r.waited(true, 3);
        r.received(100, &frame(1, 0, 0x10, 0, 1460));
        r.waited(false, 10_000);
        r.waited(false, 10_000);
        assert_eq!((r.sleeps.count(), r.yields.count()), (1, 1));
    }

    /// The top bucket takes everything past it, so a huge value neither panics nor loses count.
    #[test]
    fn a_huge_value_lands_in_the_last_bucket() {
        let mut s = Spread::new();
        s.record(u64::MAX);
        assert_eq!(s.count(), 1);
        assert_eq!(s.below(100), Some((1u64 << (BUCKETS - 1)) - 1));
    }

    /// Fixed-buffer text keeps what fits, accumulates across writes, and refuses the rest.
    #[test]
    fn text_keeps_what_fits_and_refuses_the_rest() {
        let mut t = Text::<8>::new();
        assert!(t.write_str("abc").is_ok());
        assert!(t.write_str("de").is_ok());
        assert_eq!(t.as_bytes(), b"abcde");
        assert!(t.write_str("fghi").is_err());
        assert_eq!(t.as_bytes(), b"abcdefgh");
        let mut exact = Text::<3>::new();
        assert!(exact.write_str("xyz").is_ok());
        assert_eq!(exact.as_bytes(), b"xyz");
    }

    /// A frame that is IPv4 but not TCP, or TCP without the acknowledgment flag, is read as such.
    #[test]
    fn only_tcp_and_only_flags_that_are_set_count() {
        let mut udp = frame(0, 0, 0x10, 0, 10);
        udp[23] = 17;
        assert_eq!(tcp_segment(&udp), None);
        let mut version_six = frame(0, 0, 0x10, 0, 10);
        version_six[14] = 0x65;
        assert_eq!(tcp_segment(&version_six), None);
        assert!(!tcp_segment(&frame(0, 0, 0x02, 0, 0)).unwrap().acknowledges);
        assert!(tcp_segment(&frame(0, 0, 0x12, 0, 0)).unwrap().acknowledges);
    }

    /// A poll counts the frames since the last one and divides its time among them; a poll that
    /// took none records nothing.
    #[test]
    fn a_poll_records_its_frames_and_its_time_per_frame() {
        let mut r = Receiver::new();
        r.polled(0, 500);
        assert_eq!(r.frames_per_poll.count(), 0);
        for at in [10, 20, 30, 40] {
            r.received(at, &frame(1, 0, 0x10, 0, 0));
        }
        r.polled(0, 100);
        assert_eq!(r.other_frames, 4);
        assert_eq!(r.frames_per_poll.count(), 1);
        assert_eq!(r.frames_per_poll.below(100), Some(7)); // 4 frames
        assert_eq!(r.poll_per_frame.below(100), Some(31)); // 25 us each
        r.polled(0, 100);
        assert_eq!(r.frames_per_poll.count(), 1);
    }

    /// A request served by a data segment times both, and a reply alone times only itself.
    #[test]
    fn requests_time_to_the_first_frame_and_to_the_reply() {
        let mut r = Receiver::new();
        r.requested(100);
        r.received(150, &frame(1, 0, 0x10, 0, 1460));
        r.replied(100, 200);
        assert_eq!(r.request_to_frame.count(), 1);
        assert_eq!(r.request_to_frame.below(100), Some(63)); // 50 us
        assert_eq!(r.request_to_reply.count(), 1);
        assert_eq!(r.request_to_reply.below(100), Some(127)); // 100 us
        r.requested(300);
        r.replied(300, 310);
        r.received(400, &frame(1461, 0, 0x10, 0, 1460));
        assert_eq!(r.request_to_frame.count(), 1);
    }

    /// The summary names every line and the counts of segments, other frames and acknowledgments.
    #[test]
    fn the_summary_names_each_distribution() {
        let mut r = Receiver::new();
        r.transmitted(5, &frame(1, 1000, 0x10, 2920, 0));
        r.received(10, &frame(1000, 1, 0x10, 0, 1460));
        r.received(20, &frame(2460, 1, 0x10, 0, 1460));
        r.received(30, &frame(1, 0, 0x10, 0, 0));
        let mut text = String::new();
        r.write_summary(&mut text).unwrap();
        assert!(
            text.starts_with(
                "server    : 2 data segments, 1 other frames, 1 acknowledgments sent\n"
            )
        );
        for label in [
            "gap       :",
            "per poll  : none",
            "poll cost : none",
            "yield     : none",
            "sleep     : none",
            "to frame  : none",
            "to reply  : none",
            "advertised:",
            "room      :",
            "ack->data :",
        ] {
            assert!(text.contains(label), "{label} missing from {text}");
        }
    }
}
