//! **One writer's chunks into lines.** A byte-sink writer sends at most sixteen bytes a message,
//! and two writers' messages arrive interleaved in whatever order the scheduler ran them. Keeping a
//! partial line per badge is what turns that into whole lines per writer, which is the property the
//! console needs from this service. Milestone 342 (the kernel and the `console` server drive one
//! UART from two address spaces) rests on it: the splice cannot happen with the drainer alive.

use system_log_contract::record::TEXT_MAX;

/// How many writers can be mid-line at once. See the crate's BUGS for the ninth.
pub const SLOTS: usize = 8;

/// A finished line, not yet stamped.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    /// The writer's badge.
    pub badge: u64,
    /// The text, without its newline.
    pub text: [u8; TEXT_MAX],
    /// How much of `text` is the line.
    pub len: usize,
    /// The most severe level any of the line's chunks carried, if any did.
    pub severity: Option<u8>,
    /// The line did not end in a newline: it filled [`TEXT_MAX`], or its writer ended or was
    /// crowded out first.
    pub cut: bool,
}

#[derive(Clone, Copy)]
struct Partial {
    /// 0 means free: badge 0 is the spawner, which never writes bytes.
    badge: u64,
    line: Line,
    used: u64,
}

const FREE: Partial = Partial {
    badge: 0,
    line: Line {
        badge: 0,
        text: [0; TEXT_MAX],
        len: 0,
        severity: None,
        cut: false,
    },
    used: 0,
};

/// The partial lines.
pub struct Assembler {
    slots: [Partial; SLOTS],
    tick: u64,
}

impl Default for Assembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Assembler {
    /// No writer mid-line.
    pub const fn new() -> Self {
        Assembler {
            slots: [FREE; SLOTS],
            tick: 0,
        }
    }

    /// Add `bytes` from `badge`, calling `emit` with each line they finish. A chunk can finish
    /// several (it may hold several newlines), and the first chunk from a ninth writer finishes
    /// the least recently used writer's partial line as a cut one.
    ///
    /// Lines are lent, never moved: a kernel-spawned program has one 4 KiB stack page, and in an
    /// unoptimized build every by-value `Line` is another 250-byte temporary on it.
    pub fn push(
        &mut self,
        badge: u64,
        bytes: &[u8],
        level: Option<u8>,
        mut emit: impl FnMut(&Line),
    ) {
        self.tick += 1;
        let i = match self.slots.iter().position(|s| s.badge == badge) {
            Some(i) => i,
            None => {
                let i = match self.slots.iter().position(|s| s.badge == 0) {
                    Some(i) => i,
                    None => {
                        let lru = (0..SLOTS).min_by_key(|&i| self.slots[i].used).unwrap_or(0);
                        let crowded = &mut self.slots[lru].line;
                        if crowded.len > 0 {
                            crowded.cut = true;
                            emit(crowded);
                        }
                        lru
                    }
                };
                let slot = &mut self.slots[i];
                slot.badge = badge;
                slot.line.badge = badge;
                reset(&mut slot.line);
                i
            }
        };
        let slot = &mut self.slots[i];
        slot.used = self.tick;
        for &b in bytes {
            if let Some(l) = level {
                slot.line.severity = Some(slot.line.severity.map_or(l, |s| s.min(l)));
            }
            if b == b'\n' {
                emit(&slot.line);
                reset(&mut slot.line);
                continue;
            }
            slot.line.text[slot.line.len] = b;
            slot.line.len += 1;
            if slot.line.len == TEXT_MAX {
                slot.line.cut = true;
                emit(&slot.line);
                reset(&mut slot.line);
            }
        }
    }

    /// `badge` ended its stream: its partial line, if it had one, flagged as cut. Frees the slot;
    /// the line stays readable until that slot is next taken.
    pub fn finish(&mut self, badge: u64) -> Option<&Line> {
        let slot = self.slots.iter_mut().find(|s| s.badge == badge)?;
        slot.badge = 0;
        if slot.line.len == 0 {
            return None;
        }
        slot.line.cut = true;
        Some(&slot.line)
    }
}

fn reset(line: &mut Line) {
    line.len = 0;
    line.severity = None;
    line.cut = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ninth writer crowds out the least recently used one, whose line comes out cut rather
    /// than lost, and the eight still writing keep their partial lines.
    #[test]
    fn a_ninth_writer_cuts_the_quietest_line_without_losing_it() {
        let mut a = Assembler::new();
        let mut out = 0;
        for badge in 1..=SLOTS as u64 {
            a.push(badge, b"partial", None, |_| out += 1);
        }
        assert_eq!(out, 0);
        let mut cut = None;
        a.push(99, b"new", None, |l| cut = Some(*l));
        let cut = cut.expect("the quietest writer's line was dropped");
        assert_eq!(
            (cut.badge, &cut.text[..cut.len], cut.cut),
            (1, &b"partial"[..], true)
        );
        let mut whole = None;
        a.push(2, b" done\n", None, |l| whole = Some(*l));
        let whole = whole.unwrap();
        assert_eq!(&whole.text[..whole.len], b"partial done");
        assert!(!whole.cut);
    }

    /// One chunk holding several newlines finishes several lines, in order.
    #[test]
    fn one_chunk_can_finish_several_lines() {
        let mut a = Assembler::new();
        let mut lines = [[0u8; 2]; 3];
        let mut k = 0;
        a.push(1, b"ab\ncd\nef\n", Some(5), |l| {
            lines[k].copy_from_slice(&l.text[..2]);
            assert_eq!(l.severity, Some(5));
            k += 1;
        });
        assert_eq!(k, 3);
        assert_eq!(lines, [*b"ab", *b"cd", *b"ef"]);
    }

    /// The writer evicted is the least recently heard from, not the first slot: a writer that spoke
    /// again after the others is safe, and the quietest one after it is cut.
    #[test]
    fn the_writer_cut_is_the_least_recently_heard_from() {
        let mut a = Assembler::new();
        for badge in 1..=SLOTS as u64 {
            a.push(badge, b"partial", None, |_| {});
        }
        a.push(1, b"+", None, |_| {});
        let mut cut = None;
        a.push(99, b"new", None, |l| cut = Some(l.badge));
        assert_eq!(cut, Some(2));
    }

    /// A slot whose writer is between lines has nothing to cut, so a ninth writer takes it without
    /// emitting an empty cut line.
    #[test]
    fn an_idle_slot_is_taken_without_a_cut_line() {
        let mut a = Assembler::new();
        for badge in 1..=SLOTS as u64 {
            a.push(badge, b"done\n", None, |_| {});
        }
        let mut emitted = 0;
        a.push(99, b"new", None, |_| emitted += 1);
        assert_eq!(emitted, 0);
    }
}
