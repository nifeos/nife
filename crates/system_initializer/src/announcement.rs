//! What the progenitor says: `announce`, the one sentence at a time it prints at the terminal
//! before the shell starts, and `sentence`, which composes one without an allocator. A private
//! module of the crate root, under §279 (the system-initializer crate root splits into private
//! modules).
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

use super::*;

/// **Say one sentence at the terminal**, through the line discipline, the way the shell does: stage
/// the bytes in the shell's output page (mapped here at [`PROGENITOR_SHELL_OUTPUT_VIRTUAL_ADDRESS`]) and `CALL` `OPERATION_WRITE`.
///
/// The only thing this process ever prints, and it is called before the shell is started so nothing
/// else is writing that page. It exists for the negative control: a claim about what the progenitor can no
/// longer do is worth only as much as the check behind it, and only the holder can run that check.
pub(super) fn announce(term_ep: u64, text: &[u8]) {
    let out = PROGENITOR_SHELL_OUTPUT_VIRTUAL_ADDRESS as *mut u8;
    for (i, &b) in text.iter().enumerate() {
        // SAFETY: the shell's output frame is mapped read/write here, and one line is far under a
        // page.
        unsafe { core::ptr::write_volatile(out.add(i), b) };
    }
    call(
        term_ep,
        proto::req(proto::OPERATION_WRITE, text.len() as u64),
        0,
    );
}

/// The buffer one printed sentence is composed in. A page would fit, but the sentences are one line
/// each and a fixed frame-local buffer is what keeps [`sentence`] free of an allocator.
pub(super) const SENTENCE: usize = 192;

/// Compose `prefix name name ... suffix` into `buf`, skipping empty names.
///
/// A hand-rolled `write!` because there is no allocator and `core::fmt` would drag its machinery
/// into a program whose whole job is to be small. It **truncates** rather than failing: a diagnostic
/// cut short still names the first thing that went wrong, and the alternative is a refusal that says
/// nothing at all.
pub(super) fn sentence<'b>(
    buf: &'b mut [u8; SENTENCE],
    prefix: &[u8],
    names: &[&str],
    suffix: &[u8],
) -> &'b [u8] {
    fn push(buf: &mut [u8; SENTENCE], n: &mut usize, src: &[u8]) {
        for &b in src {
            if *n < SENTENCE {
                buf[*n] = b;
                *n += 1;
            }
        }
    }
    let mut n = 0usize;
    push(buf, &mut n, prefix);
    for name in names {
        if name.is_empty() {
            continue;
        }
        push(buf, &mut n, b" ");
        push(buf, &mut n, name.as_bytes());
    }
    push(buf, &mut n, suffix);
    &buf[..n]
}
