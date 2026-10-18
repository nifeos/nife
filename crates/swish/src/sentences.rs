//! **The fixed sentences**: `pwd`, `help`, a refusal, the image and vouch sentences, the usage
//! lines, and [`write_outcome`], what a spawn's answer reads as.
//!
//! Moved out of the crate root by milestone 884 (the shell logic crate is split along its seams).
//!
//! Name: provisional (milestone 884, 2026-10-12 UTC), for calef's naming pass.

use super::*;

/// Write where the shell is, relative to its own root. A shell holding no directory has no position
/// to print, and the caller says so with [`Say::NoDirectory`] instead of calling this.
///
/// A shell holding a second tree prints the same kind of path: the second tree is mounted in the
/// one tree the shell presents, so standing inside it prints the mount path (calef, 2026-09-26,
/// for milestone 154 (a process that holds two directory capabilities)).
pub fn write_pwd(holdings: &Holdings, out: &mut dyn FnMut(&[u8])) {
    let mut buf = [0u8; nav::RENDER_MAX];
    let n = holdings.cwd.render(&mut buf);
    out(b"  ");
    out(&buf[..n]);
    out(b"\n");
}

/// The shell's own help text.
pub fn write_help(out: &mut dyn FnMut(&[u8])) {
    out(b"  help                    this text\n");
    out(b"  echo <text>             print <text>\n");
    out(b"  caps                    print this shell's whole endowment\n");
    out(b"  caps <command>          preview what that command would grant\n");
    out(b"  time <command>          run it and say how long it took (WALL clock, not CPU)\n");
    out(b"  xargs <command>         run it once per BATCH when its pattern matches too many\n");
    out(b"  cd [path]               move inside the directory you hold ('cd' is your root)\n");
    out(b"  pwd                     where you are, relative to YOUR root\n");
    out(b"  ls [path]               list a directory you can reach\n");
    out(b"  mkdir <path>            make a directory\n");
    out(b"  touch <path>            create if absent, then bump the modification time to now\n");
    out(
        b"  touch -t <instant> <path>   ...to an instant you assert (RFC 3339), if you hold the right\n",
    );
    out(
        b"  bind <target> <name>    name a position you already reached; /<name>/... reaches it too\n",
    );
    out(
        b"  apropos <word>          name the installed pages that mention it (it grants nothing)\n",
    );
    out(b"  jig install <file>      install every program of a package this image vouches for\n");
    out(b"  jig install <name>      fetch a package and install it\n");
    out(
        b"  jig remove <package>    every program it installed goes; its bytes stay for rollback\n",
    );
    out(b"  jig remove <pkg>@<ver>  one version; refused if a program would lose its default\n");
    out(b"  jig rollback            make the generation before the live one live again\n");
    out(b"  vouch <file>            the owner vouches for these bytes; a rollback undoes it\n");
    out(b"  user suspend <name>     the owner suspends an identity: its logins and schedule end\n");
    out(b"  user resume <name>      the owner lets a suspended identity back in\n");
    out(b"  rm [-rfv] <path>        a PROGRAM, granted the directory holding what you name\n");
    // Two spaces rather than the column the rest of this block keeps: these two names run past the
    // description column (26), so aligning them would leave no separator at all. Both were ratified
    // after this table was laid out, `least_authority_demo` by milestone 175 and
    // `memory_grant_depleter` on 2026-09-13, and a table that reflows every other line to suit two
    // entries costs more than two columns of ragged edge.
    out(b"  least_authority_demo <n>  spawn a process that returns n*n\n");
    out(b"  memory_grant_depleter --mem N  grant a process N pages from this shell's budget\n");
    out(b"  date                    print the wall-clock time\n");
    out(b"  printenv                print the inert configuration page (TZ, LANG, TERM)\n");
    out(b"  uuid                    a version-4 UUID, from the entropy service it is granted\n");
    out(b"  reboot                  sync the filesystem and restart the machine\n");
    out(b"  wc                      count lines, words and bytes on its INPUT\n");
    out(b"  mdr <page>              render markdown from its INPUT (apropos names the pages)\n");
    out(b"  <prog> <name>           grant a process one file, and only that file\n");
    out(b"\n  operators (milestone 50). > and | are the same mechanism: a different\n");
    out(b"  capability in a program's output slot, which it cannot look behind.\n");
    out(b"  a | b                   a's output becomes b's input; the pipe is an endpoint\n");
    out(b"  a > name                a's output becomes a file it can append to and nothing else\n");
    out(b"  a >> name               the same, without emptying the file first\n");
    out(b"  a < name                a file's bytes become a's input\n");
    out(b"  a 2> name               a's DECLARED second stream, if it has one (date does)\n");
    out(b"  echo hello | wc         a builtin can lead a pipeline: this shell writes the bytes\n");
    out(b"\n  quoting (milestone 67). it decides what a WORD is, and a word is often the\n");
    out(b"  thing you are granting. it never widens what you may name.\n");
    out(b"  'text'  \"text\"          one word, spaces and operators included\n");
    out(b"  wc \"my notes.txt\"       ... which is the only way to name a file with a space\n");
    out(b"  echo \"*.txt\"            quoted, so it is ONE name and not the set it matches\n");
    out(b"\n  sequencing. a connector carries one bit and no capability: each command is\n");
    out(b"  planned from scratch against what this shell holds.\n");
    out(b"  a ; b                   run b whatever a did\n");
    out(b"  a && b                  run b only if a succeeded\n");
    out(b"  a || b                  run b only if a did not\n");
    out(b"  echo $?                 0 it ran, 1 it failed, 2 THIS SHELL REFUSED IT\n");
    out(b"\n  naming a resource grants it; a program that names nothing can touch nothing.\n");
}

/// Write a refusal in the capability model's voice. The fixed half is `grant_plan`'s (host-tested so
/// the wording cannot drift); the shell supplies the program name where one helps.
///
/// ```
/// use grant_plan::{Command, Refusal};
/// use swish::write_refusal;
///
/// let render = |line: &[u8], refusal| {
///     let Command::Run(spec) = grant_plan::parse(line) else { panic!("not an invocation") };
///     let mut s = Vec::new();
///     write_refusal(&spec, refusal, &mut |b| s.extend_from_slice(b));
///     String::from_utf8(s).unwrap()
/// };
///
/// // A name that resolved to nothing is worth repeating back. `cat` is a command a Unix hand
/// // would reach for and this shell does not have, so the line reads as a fact about the name.
/// assert!(render(b"cat 7", Refusal::NoSuchProgram).starts_with("  cat: "));
/// // A line of nothing but flags names no program, so the bare refusal is all there is to print.
/// // An empty name followed by a colon would be worse.
/// assert_eq!(
///     render(b"--mem 16", Refusal::NoSuchProgram),
///     "  no such program (try 'help' for the builtins)\n"
/// );
/// ```
pub fn write_refusal(spec: &RunSpec, refusal: Refusal, out: &mut dyn FnMut(&[u8])) {
    out(b"  ");
    // A named-but-unresolvable program, or an un-grantable resource, name the offending thing. A
    // line of nothing but flags (`--mem 16`) names no program at all, and printing an empty name
    // followed by a colon would be worse than printing the bare refusal.
    match refusal {
        Refusal::NoSuchProgram if !spec.prog.is_empty() => {
            out(spec.prog);
            out(b": ");
        }
        Refusal::NoSuchProgram => {}
        _ => {
            if let Some(p) = Program::from_name(spec.prog) {
                out(p.name().as_bytes());
                out(b": ");
            }
        }
    }
    out(refusal.message().as_bytes());
    out(b"\n");
}

/// **What the prompt says about a job the kernel killed** (milestone 235). One sentence, shared by
/// every place in the shell that can read [`spawnproto::JOB_FAULTED`] off the result endpoint (an
/// answer, a byte stream being printed, a byte stream going into a file, a pipeline stage's ack),
/// because a person meeting the same event in four places should not meet four wordings.
pub const FAULTED_SENTENCE: &[u8] = b"  that command faulted and was killed before it answered\n";

/// **What the prompt says when the progenitor refuses bytes nobody vouched for** (DECISIONS §219 (how the shell names an installed program to the spawner),
/// milestone 198 (a package manager) rung 3a). One sentence for every place that can read
/// [`spawnproto::SPAWN_UNVOUCHED`], for [`FAULTED_SENTENCE`]'s reason. It names both halves of the
/// rule: the digest was not found, and what would have let it run anyway is a capability, not a
/// setting. That capability is §219's gate D2 (`spawnproto::RUN_UNVOUCHED_SLOT`), and a shell
/// meets this sentence only when it does not hold it.
pub const UNVOUCHED_SENTENCE: &[u8] = b"  refused: those bytes are not among the installed programs, and running unvouched bytes needs a capability this session does not hold\n";

/// **What the prompt says when a file's own manifest refuses the line** (milestone 597
/// (a program carries its manifest in an ELF note), provisional), one sentence per
/// [`grant_plan::ImageRefusal`]. The shell prints it at the prompt when it can tell before sending
/// anything, and for the progenitor's [`spawnproto::SPAWN_REFUSED_BY_MANIFEST`] when it cannot.
pub fn image_refusal_sentence(r: grant_plan::ImageRefusal) -> &'static [u8] {
    match r {
        grant_plan::ImageRefusal::Unreadable => {
            b"  refused: that file carries a manifest note that cannot be read (malformed, a later version, or two of them)\n"
        }
        grant_plan::ImageRefusal::NotCarried => {
            b"  refused: its manifest asks for a file, a directory, an input, an option, a second stream or the std runtime, which a file run by its path cannot be given yet\n"
        }
        grant_plan::ImageRefusal::ExceedsVouch => {
            b"  refused: nobody vouched for those bytes, and their manifest asks the line for more than unvouched bytes may hold\n"
        }
    }
}

/// What the prompt says for [`spawnproto::SPAWN_REFUSED_BY_MANIFEST`], when the shell did not see
/// the refusal coming: the file changed after the shell read it, or the progenitor's verdict on
/// the vouch differs from what the note alone could say.
pub const REFUSED_BY_MANIFEST_SENTENCE: &[u8] =
    b"  refused: the progenitor read those bytes' own manifest, and it does not allow this line\n";

/// What the prompt says for [`spawnproto::SPAWN_NO_DISPLAY`] (milestone 715 (provisional)): the
/// sentence milestone 632 (graphics on demand) printed before sending anything, when the shell
/// could see from its own slots that it held no display. It holds none on any boot now, so the
/// spawn service says it and this is the same sentence, word for word.
pub const NO_DISPLAY_SENTENCE: &[u8] =
    b"  no display on this boot; there is nothing to launch a graphical terminal session from\n";

/// **What `vouch` says about the edit it asked for** (DECISIONS §221 (the boot prompt is the
/// owner's console)). `status` and `live` are the progenitor's reply
/// (`spawnproto::activation_reply`). Every answer names the generation live afterwards, because
/// after a refusal that is still the fact a person needs: nothing changed, and this is what is in
/// force. Install, remove and rollback are `jig`'s since milestone 809 (the package client becomes
/// a program), and it says them itself.
pub fn write_vouch(status: spawnproto::ActivationStatus, live: u64, out: &mut dyn FnMut(&[u8])) {
    use spawnproto::ActivationStatus as S;
    let said: &[u8] = match status {
        S::Done => b"  vouched",
        S::NotExecutable => b"  refused: those bytes are not a program this machine runs",
        S::StoreFailed => b"  could not write the installed programs",
        _ => b"  the progenitor could not take that request",
    };
    out(said);
    if live == 0 {
        out(b"; nothing is installed\n");
    } else {
        out(b"; generation ");
        write_num(live, out);
        out(b" is live\n");
    }
}

/// What a malformed `vouch` line is answered with, sending nothing.
pub const VOUCH_USAGE: &[u8] =
    b"  vouch <file>: a path to a program, whose last name is at most sixteen bytes\n";

/// What a malformed `user` line is answered with, changing nothing (milestone 152 (durable delegation)).
pub const USER_USAGE: &[u8] = b"  user suspend <name> | user resume <name>\n";

/// Report what the spawned program did, in terms of the grant it was given.
pub fn write_outcome(e: &Endowment, answer: u64, out: &mut dyn FnMut(&[u8])) {
    if answer == spawnproto::SPAWN_FAILED {
        out(b"  could not spawn (the progenitor is out of memory)\n");
        return;
    }
    if answer == spawnproto::SPAWN_UNVOUCHED {
        out(UNVOUCHED_SENTENCE);
        return;
    }
    if answer == spawnproto::SPAWN_REFUSED_BY_MANIFEST {
        out(REFUSED_BY_MANIFEST_SENTENCE);
        return;
    }
    if answer == spawnproto::SPAWN_NO_DISPLAY {
        out(NO_DISPLAY_SENTENCE);
        return;
    }
    // **The job ran and the kernel killed it** (milestone 235). A different fact from the line
    // above and worth different words: nothing ran there, something ran here. The sentence says
    // "before it answered" because that is the whole reason this word exists; the kernel's own
    // report of the killed thread carries the pc.
    if answer == spawnproto::JOB_FAULTED {
        out(FAULTED_SENTENCE);
        return;
    }
    match e.program {
        Program::LeastAuthorityDemo => {
            out(b"  a process at EL0 computed ");
            write_num(e.arg, out);
            out(b"*");
            write_num(e.arg, out);
            out(b" = ");
            write_num(answer, out);
            out(b"\n");
        }
        Program::MemoryGrantDepleter => {
            out(b"  the process mapped ");
            write_num(answer, out);
            out(b" pages out of the ");
            write_num(e.mem_pages, out);
            out(b"-page budget you granted (the rest paid for its page tables)\n");
        }
        // Milestone 632 (graphics on demand): the session's one word says it ended. It is always 0
        // today (`components/src/graphical_terminal.rs`'s `end`), and the number is printed anyway
        // so a future status has somewhere to show rather than a sentence that lies about it.
        Program::GraphicalTerminal => {
            out(b"  the graphical terminal session ended, status ");
            write_num(answer, out);
            out(b"\n");
        }
        // **Every other program answers somewhere else, so there is nothing to render here.** Only a
        // program whose manifest says `OutputSpec::Words` answers in a register, and those are
        // the arms above. Supervised jobs report through the job frame and the interruptible path;
        // byte-stream programs (`date`, `wc`, `ps`, ...) are drained by the byte-stream reader
        // before this is reached.
        //
        // **A wildcard, deliberately, since milestone 150.** This match used to name all thirteen
        // programs so that adding one was a compile error here, and eleven times in thirteen the
        // arm that error asked for was `=> {}`: an edit in a crate the person adding a byte-stream
        // program had no reason to open, which nominally forced a decision and in practice forced
        // a keystroke. The question the exhaustive match was really asking ("does your program
        // answer in a register, and if so, how does it read?") is now asked of exactly the programs
        // it applies to, by `every_program_that_answers_in_words_renders_its_answer` below, which
        // fails for a `Words` program that falls through to this arm.
        _ => {}
    }
}

#[cfg(test)]
mod tests;
