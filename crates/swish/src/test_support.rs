//! **What more than one test file leans on**: collecting a renderer's output, parsing an
//! invocation, a fake directory listing, and a planned endowment. Split out of the crate root's
//! tests by milestone 884 (the shell logic crate is split along its seams).
//!
//! Name: provisional (milestone 884, 2026-10-12 UTC), for calef's naming pass.

use std::string::String;
use std::vec::Vec;

use crate::*;

/// Run a renderer and collect what it wrote. Every test reads the shell's own output,
/// which is the thing that used to need a booted kernel and a terminal to see at all.
pub(crate) fn shown(f: impl FnOnce(&mut dyn FnMut(&[u8]))) -> String {
    let mut buf: Vec<u8> = Vec::new();
    f(&mut |b| buf.extend_from_slice(b));
    String::from_utf8(buf).expect("the shell writes ASCII")
}

pub(crate) fn spec_of(line: &[u8]) -> RunSpec<'_> {
    match grant_plan::parse(line) {
        Command::Run(spec) => spec,
        other => panic!("expected an invocation, parsed {other:?}"),
    }
}

/// A directory holding `names`, standing in for the FS request the shell would make.
pub(crate) fn listing(names: &[&'static [u8]]) -> NameSet {
    let mut set = NameSet::empty();
    for n in names {
        assert!(set.push(n, false), "fixture too large for a NameSet");
    }
    set
}

pub(crate) fn endowment(prog: Program) -> Endowment {
    Endowment {
        program: prog,
        arg: 0,
        mem_pages: 0,
        file: None,
        dir: None,
        flags: 0,
        sink: line::Sink::Report,
        source: line::Source::None,
        diagnostics: match prog.manifest().output.diagnostics_slot() {
            Some(_) => line::Diagnostics::Printed,
            None => line::Diagnostics::None,
        },
        reports: true,
        interruptible: false,
        writes_while_reading: false,
    }
}
