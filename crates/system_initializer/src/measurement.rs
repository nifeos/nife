//! Measured boot (milestone 104 (the measurement continues past init)): `measured`, the one call
//! every component load goes through. See notes/system-initializer.md, "The measured load".
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

/// Read a program `name` out of the archive and measure it against `table`, or refuse it.
///
/// The three-way answer is [`measured_boot::verdict`]'s, where a host test reaches it, since this
/// crate has none; `script/swish-check` proves the boot makes this call with the table the kernel
/// measured.
pub(super) fn measured<'a>(
    fs: &nifefs::Fs<'a>,
    table: &str,
    name: &str,
) -> measured_boot::Verdict<'a> {
    measured_boot::verdict(table, name, fs.read(name))
}
