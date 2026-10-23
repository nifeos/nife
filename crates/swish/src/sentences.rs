//! **The fixed sentences**: `pwd`, `help`, a refusal, the image and vouch sentences, the usage
//! lines, and [`write_outcome`], what a spawn's answer reads as.
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
    out(b"  <command> &             run a supervised job in the background (jobs, wait, kill, fg)\n");
    out(b"  jobs                    the background jobs this shell holds\n");
    out(b"  wait [%N]               until that job (or every job) finishes; ^C stops waiting\n");
    out(b"  kill [-9] %N            ask job N to stop; -9 tears it down and takes its memory back\n");
    out(b"  fg [%N]                 watch job N (or the newest) in the foreground; ^Z suspends it\n");
    out(b"  bg [%N]                 let a suspended job run on in the background\n");
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
    // Two spaces rather than the column: these two names run past the description column (26).
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
/// // A name that resolved to nothing is worth repeating back.
/// assert!(render(b"cat 7", Refusal::NoSuchProgram).starts_with("  cat: "));
/// // A line of nothing but flags names no program, so the bare refusal is all there is to print.
/// assert_eq!(
///     render(b"--mem 16", Refusal::NoSuchProgram),
///     "  no such program (try 'help' for the builtins)\n"
/// );
/// ```
pub fn write_refusal(spec: &ParsedInvocation, refusal: Refusal, out: &mut dyn FnMut(&[u8])) {
    out(b"  ");
    // A named-but-unresolvable program, or an un-grantable resource, name the offending thing. A
    // line of nothing but flags (`--mem 16`) names no program at all, and printing an empty name
    // followed by a colon would be worse than printing the bare refusal.
    match refusal {
        Refusal::NoSuchProgram if !spec.program.is_empty() => {
            out(spec.program);
            out(b": ");
        }
        Refusal::NoSuchProgram => {}
        _ => {
            if let Some(p) = Program::from_name(spec.program) {
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
/// (a program carries its manifest in an ELF note)), one sentence per
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
        grant_plan::ImageRefusal::NetworkNotGranted => {
            b"  refused: its manifest asks for the network, and no owner's grant of the network is recorded for it among the installed programs\n"
        }
    }
}

/// What the prompt says for [`spawnproto::SPAWN_REFUSED_BY_MANIFEST`], when the shell did not see
/// the refusal coming: the file changed after the shell read it, or the progenitor's verdict on
/// the vouch differs from what the note alone could say.
pub const REFUSED_BY_MANIFEST_SENTENCE: &[u8] =
    b"  refused: the progenitor read those bytes' own manifest, and it does not allow this line\n";

/// What the prompt says for [`spawnproto::SPAWN_NO_DISPLAY`] (milestone 715 (the spawn service
/// holds the display grants and the shell holds none)): the shell holds no display, so the spawn
/// service decides this and the shell prints it.
pub const NO_DISPLAY_SENTENCE: &[u8] =
    b"  no display on this boot; there is nothing to launch a graphical terminal session from\n";

/// **What `vouch` says about the edit it asked for** (DECISIONS §221 (the boot prompt is the
/// owner's console)). `status` and `live` are the progenitor's reply
/// (`spawnproto::activation_reply`). Every answer names the generation live afterwards, because
/// after a refusal that is still the fact a person needs: nothing changed, and this is what is in
/// force. Install, remove and rollback are `jig`'s (milestone 809 (the package client becomes a
/// program)), and it says them itself.
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

/// **What the prompt says when a spawn was not built**, by the progenitor's word: a program this
/// image does not carry ([`spawnproto::SPAWN_NOT_IN_IMAGE`]) is named as that, and anything else
/// is the job pool. Every site that answers a failed spawn reads it here, so the two cannot drift
/// apart one call site at a time.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/image-needs-jig.
#[must_use]
pub const fn spawn_failure_sentence(answer: u64) -> &'static [u8] {
    if answer == spawnproto::SPAWN_NOT_IN_IMAGE {
        b"  could not spawn (this image was built without that program)\n"
    } else {
        b"  could not spawn (the progenitor is out of memory)\n"
    }
}

/// What a malformed `vouch` line is answered with, sending nothing.
pub const VOUCH_USAGE: &[u8] =
    b"  vouch <file>: a path to a program, whose last name is at most sixteen bytes\n";

/// What a malformed `user` line is answered with, changing nothing (milestone 152 (durable delegation)).
pub const USER_USAGE: &[u8] = b"  user suspend <name> | user resume <name>\n";

/// Report what the spawned program did, in terms of the grant it was given.
pub fn write_outcome(e: &Endowment, answer: u64, out: &mut dyn FnMut(&[u8])) {
    if answer == spawnproto::SPAWN_FAILED || answer == spawnproto::SPAWN_NOT_IN_IMAGE {
        out(spawn_failure_sentence(answer));
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
    // The job ran and the kernel killed it (milestone 235): something ran, unlike the lines above.
    if answer == spawnproto::JOB_FAULTED {
        out(FAULTED_SENTENCE);
        return;
    }
    match e.program {
        Program::LeastAuthorityDemo => {
            out(b"  a process at EL0 computed ");
            write_num(e.program_argument, out);
            out(b"*");
            write_num(e.program_argument, out);
            out(b" = ");
            write_num(answer, out);
            out(b"\n");
        }
        Program::MemoryGrantDepleter => {
            out(b"  the process mapped ");
            write_num(answer, out);
            out(b" pages out of the ");
            write_num(e.memory_grant_pages, out);
            out(b"-page budget you granted (the rest paid for its page tables)\n");
        }
        // Always 0 today (`components/src/graphical_terminal.rs`'s `end`); printed so a future
        // status has somewhere to show.
        Program::GraphicalTerminal => {
            out(b"  the graphical terminal session ended, status ");
            write_num(answer, out);
            out(b"\n");
        }
        // Every other program answers somewhere else: only `OutputDeclaration::Words` programs
        // answer in a register. `every_program_that_answers_in_words_renders_its_answer` fails for
        // one that falls through to this wildcard.
        _ => {}
    }
}

#[cfg(test)]
mod tests;
