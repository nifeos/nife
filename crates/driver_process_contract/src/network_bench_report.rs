//! **Text the network bench boot prints for a `net_stack` process**, carried as three-word
//! reports: the drain client's trace on the client's report endpoint, then, when the client says
//! so, the server's on the server's. A program has no console of its own, so the kernel prints
//! what arrives. Each report is a [`Report`], made by [`Report::pack`] and read by
//! [`Report::unpack`].
//!
//! Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.

/// How many text bytes one report carries.
///
/// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
pub const TEXT_BYTES: usize = 16;

/// One report of the text stream.
///
/// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    /// The next `length_bytes` bytes of text (1 to [`TEXT_BYTES`]), UTF-8 split anywhere, lines ending
    /// in `\n`.
    Text {
        /// The bytes; those past `length_bytes` are zero.
        bytes: [u8; TEXT_BYTES],
        /// How many of them count.
        length_bytes: usize,
    },
    /// The text has ended. `server_follows` says the server's own text comes next, on the
    /// server's report endpoint; only the drain client's end sets it.
    End {
        /// Whether to read the server's report endpoint next.
        server_follows: bool,
    },
}

impl Report {
    /// The reports that carry `text`, an [`Report::End`] not included.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn text(text: &[u8]) -> impl Iterator<Item = Report> + '_ {
        text.chunks(TEXT_BYTES).map(|chunk| {
            let mut bytes = [0u8; TEXT_BYTES];
            bytes[..chunk.len()].copy_from_slice(chunk);
            Report::Text {
                bytes,
                length_bytes: chunk.len(),
            }
        })
    }

    /// The three words that carry this report: a text report's length, then its bytes in two
    /// little-endian words; an end's zero, then whether the server follows.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn pack(&self) -> [u64; 3] {
        match *self {
            Report::Text {
                bytes,
                length_bytes,
            } => {
                let word = |i: usize| {
                    let mut w = [0u8; 8];
                    w.copy_from_slice(&bytes[i * 8..i * 8 + 8]);
                    u64::from_le_bytes(w)
                };
                [length_bytes as u64, word(0), word(1)]
            }
            Report::End { server_follows } => [0, u64::from(server_follows), 0],
        }
    }

    /// The report `words` carry. A length past [`TEXT_BYTES`] is clamped, so a confused sender
    /// prints garbage rather than running the reader past its buffer.
    ///
    /// Name: provisional, minted 2026-11-28 (UTC) by lane/e1000e-rx-latency.
    pub fn unpack(words: [u64; 3]) -> Report {
        if words[0] == 0 {
            return Report::End {
                server_follows: words[1] == 1,
            };
        }
        let mut bytes = [0u8; TEXT_BYTES];
        bytes[..8].copy_from_slice(&words[1].to_le_bytes());
        bytes[8..].copy_from_slice(&words[2].to_le_bytes());
        Report::Text {
            bytes,
            length_bytes: (words[0] as usize).min(TEXT_BYTES),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the client splits, the kernel joins byte for byte, and the end says who is next.
    #[test]
    fn text_survives_the_words() {
        let text = b"gap       : n 3, mean 310 us\nsecond line\n";
        let mut joined = Vec::new();
        for r in Report::text(text) {
            match Report::unpack(r.pack()) {
                Report::Text {
                    bytes,
                    length_bytes,
                } => joined.extend_from_slice(&bytes[..length_bytes]),
                Report::End { .. } => panic!("text packed as an end"),
            }
        }
        assert_eq!(joined, text);
        let end = Report::End {
            server_follows: true,
        };
        assert_eq!(Report::unpack(end.pack()), end);
    }
}
