//! **The output side**: [`Sink`], where echo bytes go, and [`expand_output`], the newline
//! translation applied to what an application prints.
//!
//! Name: provisional (milestone 878's lane, 2026-10-12 UTC; §295 (the line editor's crate root
//! splits into module files)), for the module and the file.

/// Where echo bytes go. The server implements this over its console channel; the tests
/// implement it over a terminal model.
pub trait Sink {
    /// Write echo bytes out. Never called with an empty slice, so an implementation may treat a
    /// call as a message to send. Called synchronously from the byte-feeding methods below, so a
    /// `Sink` that blocks blocks the whole discipline.
    fn put(&mut self, bytes: &[u8]);
}

/// Output-side processing: translate `\n` to `\r\n` so applications can speak Unix newlines to
/// a serial terminal that wants a carriage return. Everything else, ANSI included, passes
/// through untouched: the wire belongs to the application when it is printing.
pub fn expand_output(source: &[u8], out: &mut impl Sink) {
    let mut from = 0;
    for (i, &b) in source.iter().enumerate() {
        if b == b'\n' {
            if from < i {
                out.put(&source[from..i]);
            }
            out.put(b"\r\n");
            from = i + 1;
        }
    }
    if from < source.len() {
        out.put(&source[from..]);
    }
}

#[cfg(test)]
mod tests;
