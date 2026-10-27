//! **The shell's own endowment**: [`write_holdings`] (what `caps` with no tail prints), the
//! census of held slots, the `bind` rows and the configuration values.
//!
//! Name: provisional (milestone 884, 2026-10-12 UTC), for calef's naming pass.

use super::*;

/// **Write the shell's whole endowment**: the introspection that makes "reading one literal tells
/// you a process's authority" real, pointed at the shell itself.
///
/// `budget_pages` is what the progenitor granted at boot rather than what is left, because there is no syscall
/// that reports the remainder. The caller passes its own constant and the row says "(initial)".
///
/// `clock` is the slot holding this shell's read-only clock page, or `None` for a wiring that
/// granted none (milestone 86 (`time`: the shell times a command)). A slot number rather than a
/// boolean, because a shell wired without a filesystem has one fewer capability below it.
pub fn write_holdings(
    budget_pages: u64,
    holdings: Holdings,
    clock: Option<u64>,
    config: Option<ConfigPage>,
    out: &mut dyn FnMut(&[u8]),
) {
    out(b"  this shell holds, and nothing else:\n");
    out(b"    cap 0  endpoint  terminal   read lines, write text\n");
    out(b"    cap 1  endpoint  spawn      direct the progenitor to start a program\n");
    out(b"    cap 2  endpoint  result     read a spawned program's answer\n");
    out(b"    cap 3  untyped   ");
    write_num(budget_pages, out);
    out(b" pages  the memory it grants with --mem (initial)\n");
    // The heap is mapped from that budget first, so this row counts it (milestone 47 (navigation
    // and naming), calef's ruling).
    out(b"           the first ");
    write_num(HEAP_MAX_BYTES / 1024, out);
    out(b" KiB of them, and their page tables, are the shell's own heap\n");
    match (&holdings.mounted, holdings.directory) {
        (Some(sd), _) => {
            // Two rows, one tree: which tree serves a path is otherwise kept out of sight (calef,
            // 2026-09-26), and this table is the one place that says it.
            out(b"    cap 4  endpoint  directory  the files it can narrow into per-file grants\n");
            out(b"    cap ");
            write_num(grant_plan::spawnproto::MOUNTED_TREE_SLOT, out);
            out(b" endpoint  directory  a second tree, mounted at ");
            let mut buf = [0u8; nav::RENDER_MAX];
            let n = sd.mount().render(&mut buf);
            out(&buf[..n]);
            out(b"\n");
            out(b"    namespace: one tree, with the second mounted in it (milestone 154)\n");
        }
        (None, true) => {
            out(b"    cap 4  endpoint  directory  the files it can narrow into per-file grants\n");
        }
        (None, false) => {
            out(b"    (no directory capability: a name on the line has nothing to narrow)\n");
        }
    }
    // `bind`'s rows are additive, a further section; a one-grant shell gets its `namespace:`
    // header here so a bound row is never printed without one (milestone 154 (a process that
    // holds two directory capabilities)).
    if holdings.binds.iter().next().is_some() {
        if holdings.mounted.is_none() {
            out(b"    namespace: names bound beside the one root ('bind', milestone 47)\n");
        }
        for entry in holdings.binds.iter() {
            write_bind_row(entry, out);
        }
    }
    // The rights are the row: `READ` without `GRANT`, so nothing typed here hands a clock to a
    // child (DECISIONS §43 (reading the clock is a page)).
    match clock {
        Some(slot) => {
            out(b"    cap ");
            write_num(slot, out);
            out(b"  frame     clock      READ only, NOT delegable: 'time' measures with\n");
            out(b"                                  it and no command can be handed it\n");
        }
        None => {
            out(b"    (no clock here: the progenitor endows 'date' a read-only clock page, this shell was\n");
            out(b"     granted none, so 'time' has nothing to measure with)\n");
        }
    }
    // The configuration page has the clock's rights for the clock's reason (DECISIONS §111 (inert
    // configuration is a validated page)), and no row when absent.
    if let Some(page) = config {
        out(b"    cap ");
        write_num(grant_plan::SHELL_CONFIGURATION_SLOT, out);
        out(b" frame     config     READ only, NOT delegable: the page a child declaring\n");
        out(b"                                  config is endowed with, so 'caps' can show its values\n");
        write_config_values(&page, b"                                  ", out);
    }
    out(b"  it can name no devices and no other process. authority is what it holds.\n");
}

/// **The census under the table**: every slot below `below` (the caller passes the reserved fault
/// slot) that holds a capability, as the kernel reports it through `held`, not as the rows above
/// say. A capability no row names still shows up here. `unreachable_network_witness`'s census is
/// the same line, one program over. Name: provisional.
pub fn write_census(below: u64, held: &dyn Fn(u64) -> bool, out: &mut dyn FnMut(&[u8])) {
    out(b"  slots held:");
    for slot in 0..below {
        if held(slot) {
            out(b" ");
            write_num(slot, out);
        }
    }
    out(b"\n");
}

/// One row of `bind`'s own namespace section: the name it was filed under, and the path it
/// resolves to.
fn write_bind_row(entry: &nav::BindEntry, out: &mut dyn FnMut(&[u8])) {
    out(b"      bind ");
    out(entry.name());
    out(b" -> ");
    let mut buf = [0u8; nav::RENDER_MAX];
    let n = entry.pos().render(&mut buf);
    out(&buf[..n]);
    out(b"\n");
}

/// **The three inert-configuration keys, one per line**, in `printenv`'s spelling (`KEY=value`,
/// or `KEY (unset)` for a key the page does not carry). A page nobody assembled reads as three
/// unset keys, which is what [`ConfigPage`] answers for it and what the child would see.
pub(super) fn write_config_values(page: &ConfigPage, indent: &[u8], out: &mut dyn FnMut(&[u8])) {
    for (key, value) in [
        (b"TZ".as_slice(), page.tz()),
        (b"LANG".as_slice(), page.lang()),
        (b"TERM".as_slice(), page.term()),
    ] {
        out(indent);
        out(key);
        match value {
            Some(v) => {
                out(b"=");
                out(v.as_bytes());
            }
            None => out(b" (unset)"),
        }
        out(b"\n");
    }
}

#[cfg(test)]
mod tests;
