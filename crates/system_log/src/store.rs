//! **The ring and what feeds it**: the sequence counter, and the buffer a line is rendered into.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The ring and what feeds it: the sequence counter, and the buffer a line is rendered into.
pub(crate) struct Store {
    pub(crate) ring: Ring,
    pub(crate) next_seq: u64,
    /// Where a line is rendered before it is copied into the ring. Here rather than on the stack
    /// because a program the kernel spawns has one 4 KiB stack page, and half of it is too much to
    /// spend on one buffer.
    pub(crate) scratch: [u8; json::LINE_MAX],
}

impl Store {
    /// Stamp a finished line from its writer's badge, render it, store it.
    pub(crate) fn line(&mut self, registry: &Registry, line: &Line, now: u64) {
        let entry = registry.get(line.badge);
        let header = Header {
            seq: 0,
            time: now,
            source: line.badge,
            severity: line
                .severity
                .unwrap_or(entry.map_or(severity::INFO, |e| e.default_severity)),
            flags: if line.cut { record::flags::CUT } else { 0 },
            len: line.len as u16,
            kind: byte_sink_contract::KIND_TEXT,
        };
        let program = entry.map(|e| e.program);
        let user = entry.map(|e| e.user);
        self.record(&header, &line.text[..line.len], program, user);
    }

    /// F3 to JSONL to the ring, numbering the record.
    pub(crate) fn record(
        &mut self,
        header: &Header,
        text: &[u8],
        program: Option<Name>,
        user: Option<Name>,
    ) {
        let mut h = *header;
        h.seq = self.next_seq;
        self.next_seq += 1;
        let program = program
            .as_ref()
            .map(Name::as_bytes)
            .filter(|p| !p.is_empty());
        let user = user.as_ref().map(Name::as_bytes).filter(|u| !u.is_empty());
        let n = json::render(&h, program, user, text, &mut self.scratch);
        self.ring
            .append(h.seq, user.unwrap_or(&[]), &self.scratch[..n]);
    }
}
