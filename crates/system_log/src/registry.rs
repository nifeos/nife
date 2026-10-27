//! **What a badge means**: the program and user a spawner registered for it, the severity its
//! stream infers, and, for a reader, which window and which scope.
//!
//! The spawner registers a badge before handing the badged capability to the process it spawns,
//! so a writer's first line is already attributed. A badge nobody registered is still stamped (its
//! `source` is the badge itself) with no program and no user.

use system_log_contract::{control, severity};

/// How many badges the service knows at once. A spawner forgets a badge when its process is
/// reaped, so this bounds live writers and readers, not every process the boot ever ran.
pub const ENTRIES: usize = 32;

/// A registered name: a program's or a user's, at most [`control::NAME_MAX`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Name {
    bytes: [u8; control::NAME_MAX],
    len: u8,
}

impl Name {
    /// The empty name.
    pub const EMPTY: Name = Name {
        bytes: [0; control::NAME_MAX],
        len: 0,
    };

    /// A name from bytes, cut at [`control::NAME_MAX`]. For the service's own records (`kernel`,
    /// from milestone 342 (the kernel and the `console` server drive one UART from two address
    /// spaces)) and for tests.
    pub fn new(name: &[u8]) -> Name {
        let mut n = Name::EMPTY;
        let len = name.len().min(control::NAME_MAX);
        n.bytes[..len].copy_from_slice(&name[..len]);
        n.len = len as u8;
        n
    }

    /// The name's bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

/// A reader's grant: which window the service fills for it, and whether it sees every record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reader {
    /// The window index.
    pub window: u8,
    /// The system read (every record) rather than the per-user read.
    pub system: bool,
}

/// One registered badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// The badge.
    pub badge: u64,
    /// The program the spawner said holds it.
    pub program: Name,
    /// The user the spawner said it runs for; empty for a system service.
    pub user: Name,
    /// The level a line gets when its writer set none.
    pub default_severity: u8,
    /// Set when the badge is a reader's.
    pub reader: Option<Reader>,
}

/// Every badge the service knows.
pub struct Registry {
    entries: [Option<Entry>; ENTRIES],
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    /// Nobody registered.
    pub const fn new() -> Self {
        Registry {
            entries: [const { None }; ENTRIES],
        }
    }

    /// The entry for `badge`, if registered.
    pub fn get(&self, badge: u64) -> Option<&Entry> {
        self.entries.iter().flatten().find(|e| e.badge == badge)
    }

    /// The entry for `badge`, created on first mention; `None` when the table is full.
    pub fn entry(&mut self, badge: u64) -> Option<&mut Entry> {
        let i = match self
            .entries
            .iter()
            .position(|e| matches!(e, Some(e) if e.badge == badge))
        {
            Some(i) => i,
            None => {
                let i = self.entries.iter().position(Option::is_none)?;
                self.entries[i] = Some(Entry {
                    badge,
                    program: Name::EMPTY,
                    user: Name::EMPTY,
                    default_severity: severity::INFO,
                    reader: None,
                });
                i
            }
        };
        self.entries[i].as_mut()
    }

    /// Apply one [`control::OPERATION_NAME`] chunk. Chunk 0 replaces the field; chunk 1 extends it, and
    /// only a field chunk 0 filled to sixteen bytes can be extended. `false` when refused.
    pub fn name(&mut self, badge: u64, field: u64, chunk: usize, bytes: &[u8; 16]) -> bool {
        let Some(e) = self.entry(badge) else {
            return false;
        };
        let name = if field == control::PROGRAM {
            &mut e.program
        } else {
            &mut e.user
        };
        let n = bytes.iter().position(|&b| b == 0).unwrap_or(16);
        match chunk {
            0 => {
                *name = Name::new(&bytes[..n]);
                true
            }
            1 if name.len == 16 => {
                name.bytes[16..16 + n].copy_from_slice(&bytes[..n]);
                name.len = 16 + n as u8;
                true
            }
            _ => false,
        }
    }

    /// Drop `badge`'s entry.
    pub fn forget(&mut self, badge: u64) {
        for e in &mut self.entries {
            if matches!(e, Some(x) if x.badge == badge) {
                *e = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(s: &[u8]) -> [u8; 16] {
        let mut b = [0u8; 16];
        b[..s.len()].copy_from_slice(s);
        b
    }

    /// A name fills both chunks only in order: a second chunk after a first that did not fill
    /// sixteen bytes is refused, because it would splice two names into one.
    #[test]
    fn a_second_chunk_extends_only_a_full_first() {
        let mut r = Registry::default();
        assert!(r.name(5, control::PROGRAM, 0, b"sink_transcript_"));
        assert!(r.name(5, control::PROGRAM, 1, &chunk(b"writer")));
        assert_eq!(
            r.get(5).unwrap().program.as_bytes(),
            b"sink_transcript_writer"
        );
        assert!(r.name(6, control::USER, 0, &chunk(b"bob")));
        assert!(!r.name(6, control::USER, 1, &chunk(b"tail")));
        assert!(!r.name(6, control::USER, 2, &chunk(b"x")));
        assert_eq!(r.get(6).unwrap().user.as_bytes(), b"bob");
        assert_eq!(Name::new(&[b'n'; 40]).as_bytes().len(), control::NAME_MAX);
    }

    /// A full table refuses a new badge rather than evicting a live one, and forgetting a badge
    /// gives its slot back.
    #[test]
    fn a_full_table_refuses_and_forget_frees() {
        let mut r = Registry::new();
        for b in 1..=ENTRIES as u64 {
            assert!(r.entry(b).is_some());
        }
        assert!(r.entry(ENTRIES as u64 + 1).is_none());
        assert!(!r.name(ENTRIES as u64 + 1, control::PROGRAM, 0, &chunk(b"late")));
        assert!(
            r.entry(3).is_some(),
            "an existing badge is still found when the table is full"
        );
        r.forget(3);
        assert!(r.get(3).is_none());
        assert!(r.entry(ENTRIES as u64 + 1).is_some());
    }
}
