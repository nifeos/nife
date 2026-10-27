//! **The log service's state machine**: what one message did, and the `Log` that handles it.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// What [`Log::handle`] asks its caller to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handled {
    /// Nothing: the message was stored, registered, or refused and counted.
    Nothing,
    /// A registered reader asked for records from `cursor`. The caller fills the reader's
    /// `window` with [`Log::fill`] and then signals that window's notification.
    Read {
        /// Which reader window.
        window: u8,
        /// The cursor the reader passed.
        cursor: u64,
    },
    /// A read from a badge that is not a registered reader. Nothing to fill and nobody to signal.
    Refused,
}

/// The whole service's state. `const`-constructible so a `no_std` program holds it in a `static`.
///
/// # Examples
///
/// ```
/// use system_log::{Log, Handled};
/// use system_log_contract::{control, read};
/// use byte_sink_contract::pack;
///
/// let mut log = Log::new();
/// for (badge, program) in [(1u32, &b"alpha"[..]), (2, &b"beta"[..])] {
///     for m in control::name_messages(badge, control::PROGRAM, program).into_iter().flatten() {
///         log.handle(0, m.0, m.1, m.2, 0);
///     }
/// }
/// log.handle(0, control::word(control::OPERATION_READER, control::SCOPE_SYSTEM, 0, 9), 0, 0, 0);
///
/// let (a, b) = (b"first half, then", b"one\n");
/// let (w0, w1, w2, _) = pack(a);
/// log.handle(1, w0, w1, w2, 10);
/// let (w0, w1, w2, _) = pack(b"hello from beta\n");
/// log.handle(2, w0, w1, w2, 20);
/// let (w0, w1, w2, _) = pack(b);
/// log.handle(1, w0, w1, w2, 30);
///
/// let mut window = [0u8; read::WINDOW_BYTES];
/// let asked = log.handle(9, read::request(), 0, 0, 40);
/// assert_eq!(asked, Handled::Read { window: 0, cursor: 0 });
/// log.fill(9, 0, &mut window);
/// let (next, len, status) = read::header(&window);
/// let text = core::str::from_utf8(&window[read::DATA..read::DATA + len]).unwrap();
/// assert_eq!(status, read::status::OK);
/// assert_eq!(next, 2);
/// let mut lines = text.lines();
/// let beta = lines.next().unwrap();
/// let alpha = lines.next().unwrap();
/// assert!(beta.starts_with(r#"{"seq":0,"time":20,"source":2,"program":"beta","user":null,"#));
/// assert!(alpha.ends_with(r#""severity":"info","msg":"first half, thenone"}"#));
/// ```
pub struct Log {
    registry: Registry,
    assembler: Assembler,
    pub(crate) store: Store,
    /// Messages refused: malformed byte-sink words, a body kind other than text, or control from a
    /// badge that is not 0. Counted rather than stored, and exposed for the service's tests.
    pub refused: u64,
}

impl Default for Log {
    fn default() -> Self {
        Self::new()
    }
}

impl Log {
    /// An empty log.
    pub const fn new() -> Self {
        Log {
            registry: Registry::new(),
            assembler: Assembler::new(),
            store: Store {
                ring: Ring::new(),
                next_seq: 0,
                scratch: [0; json::LINE_MAX],
            },
            refused: 0,
        }
    }

    /// **Handle one received message**: the three words a `SEND` carried, the badge the kernel
    /// delivered beside them, and the time it arrived.
    ///
    /// Badge 0 is the spawner, which holds the one unbadged capability: its messages are
    /// [`control`] words. Any other badge is a writer's bytes or a reader's request.
    pub fn handle(&mut self, badge: u64, w0: u64, w1: u64, w2: u64, now: u64) -> Handled {
        if badge == 0 {
            self.control(w0, w1, w2);
            return Handled::Nothing;
        }
        let operation = w0 >> byte_sink_contract::OPERATION_SHIFT;
        if operation == read::OPERATION_READ {
            return match self.registry.get(badge).and_then(|e| e.reader) {
                Some(r) => Handled::Read {
                    window: r.window,
                    cursor: w1,
                },
                None => Handled::Refused,
            };
        }
        if byte_sink_contract::kind(w0) != byte_sink_contract::KIND_TEXT {
            self.refused += 1;
            return Handled::Nothing;
        }
        let mut bytes = [0u8; INLINE_MAX];
        match unpack(w0, w1, w2, &mut bytes) {
            Msg::Bytes(n) => {
                let level = byte_sink_contract::severity(w0);
                // Split the borrow: the assembler emits while the store takes the line.
                let Log {
                    assembler,
                    registry,
                    store,
                    ..
                } = self;
                assembler.push(badge, &bytes[..n], level, |line| {
                    store.line(registry, line, now);
                });
            }
            Msg::Eof => {
                if let Some(line) = self.assembler.finish(badge) {
                    self.store.line(&self.registry, line, now);
                }
            }
            Msg::Malformed => self.refused += 1,
        }
        Handled::Nothing
    }

    /// One spawner word. A malformed one is counted, never half-applied.
    fn control(&mut self, w0: u64, w1: u64, w2: u64) {
        let (operation, field, arg, badge) = control::fields(w0);
        let ok = match operation {
            control::OPERATION_NAME if badge != 0 && arg < 2 && field <= control::USER => {
                let mut chunk = [0u8; 16];
                chunk[..8].copy_from_slice(&w1.to_le_bytes());
                chunk[8..].copy_from_slice(&w2.to_le_bytes());
                self.registry
                    .name(badge as u64, field, arg as usize, &chunk)
            }
            control::OPERATION_STREAM if badge != 0 && arg <= severity::DEBUG as u64 => self
                .registry
                .entry(badge as u64)
                .map(|e| e.default_severity = arg as u8)
                .is_some(),
            control::OPERATION_READER if badge != 0 && field <= control::SCOPE_SYSTEM => self
                .registry
                .entry(badge as u64)
                .map(|e| {
                    e.reader = Some(registry::Reader {
                        window: arg as u8,
                        system: field == control::SCOPE_SYSTEM,
                    });
                })
                .is_some(),
            control::OPERATION_FORGET if badge != 0 => {
                // A partial line dies with its writer's registration rather than being stamped
                // later under a name nobody holds.
                let _ = self.assembler.finish(badge as u64);
                self.registry.forget(badge as u64);
                true
            }
            _ => false,
        };
        if !ok {
            self.refused += 1;
        }
    }

    /// **Store one F3 record**, already stamped: the path the kernel's records will take once
    /// milestone 342 (the kernel and the `console` server drive one UART from two address spaces)
    /// builds its ring, with [`record::flags::KERNEL`] set and `program` given as `kernel`. The
    /// header's `seq` is replaced by the service's own, so the ring's numbering stays dense whoever
    /// wrote the record.
    pub fn ingest(
        &mut self,
        header: &Header,
        text: &[u8],
        program: Option<Name>,
        user: Option<Name>,
    ) {
        self.store.record(header, text, program, user);
    }

    /// **Answer a read**: write whole JSONL lines from `cursor` into `window` (a
    /// [`read::WINDOW_BYTES`] page) and its header. `badge` is the reader's, and decides the scope:
    /// a system reader gets every record, a per-user reader only those stamped with its own user.
    pub fn fill(&self, badge: u64, cursor: u64, window: &mut [u8]) {
        let Some(entry) = self.registry.get(badge) else {
            read::put_header(window, cursor, 0, read::status::REFUSED);
            return;
        };
        let Some(reader) = entry.reader else {
            read::put_header(window, cursor, 0, read::status::REFUSED);
            return;
        };
        let scope = if reader.system {
            None
        } else {
            Some(entry.user.as_bytes())
        };
        let (next, n) =
            self.store
                .ring
                .read(cursor, scope, &mut window[read::DATA..read::WINDOW_BYTES]);
        read::put_header(window, next, n as u32, read::status::OK);
    }

    /// How many records the ring holds now, for tests and gauges.
    pub fn held(&self) -> usize {
        self.store.ring.held()
    }
}
