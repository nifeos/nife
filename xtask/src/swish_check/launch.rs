//! **What a leg does after its script**: keystrokes over the monitor, the reboot phase, the
//! graphical terminal's launch, and the `rg` walk.
//!
//! Cut out of `swish_check.rs` unchanged by milestone 847 (the swish-check harness is split
//! along its seams).
//!
//! Name: provisional (milestone 847's lane, 2026-10-11 UTC; §280 (the swish-check harness
//! splits into submodules, with its script in one)).

use std::io::Write;
use std::time::{Duration, Instant};

use super::*;

/// **The graphical launch, folded into the plain legs' boots** (milestone 632 (graphics on
/// demand)). The boot is the **normal UART system on every boot**, and graphics
/// is *launched*: this leg types `graphical_terminal` at the swish prompt and verifies what the
/// launched session puts on the screen, read back with a `screendump`.
///
/// 1. [`swish_check_boot`] boots the normal system with a virtio-gpu attached, reading the UART
///    transcript: the swish prompt must appear on the serial console before anything is typed,
///    the assertion that the boot stayed minimal.
/// 2. [`launch_graphical_terminal`] types `graphical_terminal` over the UART. The shell asks
///    (`spawnproto::GRAPHICS_BIT`), the progenitor builds the session's stack from the devices it
///    holds, and the session prints its own `$ ` prompt on the screen.
/// 3. It presses one key and requires its echo on the screen: a real round trip through a line
///    discipline and the session's own echo.
///
/// [`Keystrokes::Device`] attaches a virtio-keyboard and presses a key with the QEMU monitor's
/// `sendkey`. [`Keystrokes::Serial`] attaches **no** keyboard and types the same byte down the
/// UART, the configuration every target machine has (milestone 192 (a keyboard on real silicon)'s
/// option A, at launch): the session reads the boot's line discipline raw, the shell's own §227
/// (how Tab reaches the shell) shape, and paints the echo itself. Neither the screen nor the
/// session can tell which source a byte came by, so a change that made it matter turns exactly one
/// run red.
///
/// **Any `$ ` prompt on the screen is the proof**: a capability-slot collision anywhere in the
/// session's build (the shell's held grants, the progenitor receiving them, the children built from
/// one region, the session's own table) fails in total silence. It looks for `$ ` anywhere in the
/// decoded grid, not at a predicted row, and `$ a` after the key press proves the keystroke's
/// round trip. There is no known picture to compare against, so this proves the launch, less than
/// [`swish_check_leg`] proves of a boot.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Keystrokes {
    /// A virtio-input device, pressed with the monitor's `sendkey`. Milestone 177.
    Device,
    /// The guest's own UART, typed down the child's stdin (`-serial stdio`). Milestone 192,
    /// option A, and the only source any of the three real machines has.
    Serial,
}

/// The firmware line a reset must come back through, where the architecture has firmware of its own
/// between the reset and the kernel: OpenSBI on riscv64, OVMF and `uefi_loader` on `x86_64`. aarch64
/// `virt` boots `-kernel` with nothing in between, so there the second banner is the proof.
fn firmware_marker(arch: &str) -> Option<&'static str> {
    match arch {
        "riscv64" => Some("OpenSBI"),
        "x86_64" => Some("uefi_loader"),
        _ => None,
    }
}

/// **Type `reboot` and see the machine come back** (milestone 805 (`reboot` at the prompt), the
/// phase that ends every architecture's first boot). It writes a file, types `reboot`, and asks
/// for, in order:
///
/// - `reboot`'s own report that its sync was answered (the block server's count of completed
///   `blk::FLUSH`es, through its sync-only capability), before the kernel's first `reboot:` line;
/// - the kernel's attempt line, and no refusal;
/// - the firmware's line where there is firmware ([`firmware_marker`]);
/// - a second prompt banner, then `wc reboot.txt` answering `1 3 18`.
///
/// Every error names the step, prefixed `reboot phase:`, so a red leg says it was the reboot and
/// not a scripted line. The read-back cannot fail for want of a sync under QEMU, which outlives a
/// guest reset with its host cache intact; the sync line is the evidence the sync happened.
pub(super) fn reboot_phase(
    arch: &str,
    stdin: &mut std::process::ChildStdin,
    seen: &std::sync::Arc<std::sync::Mutex<String>>,
) -> Result<String, String> {
    use RebootPhase as P;
    let mut phase = RebootPhase { seen, stdin };
    if !phase.wait_prompt(P::LINE_SECS) {
        return Err(P::failure(
            "the first boot was not at a bare prompt when the phase began",
        ));
    }
    let at = phase.len();
    phase.type_line(P::WRITE_LINE)?;
    if phase
        .wait_after(at, &format!("{}\n", P::WRITE_LINE), P::LINE_SECS)
        .is_none()
        || !phase.wait_prompt(P::LINE_SECS)
    {
        return Err(P::failure(&format!(
            "`{}` never came back to a prompt",
            P::WRITE_LINE
        )));
    }
    let typed = phase.len();
    phase.type_line("reboot")?;
    let Some(asked) = phase.wait_after(typed, P::KERNEL_ASKED, P::LINE_SECS) else {
        return Err(P::failure(
            "the kernel never said it was asked to restart the machine",
        ));
    };
    let before = phase.text()[typed..asked].to_string();
    if !before.contains(P::SYNCED) {
        return Err(P::failure(&format!(
            "`reboot` reached the kernel without reporting a completed sync (no {:?} \
             before the kernel's line): {:?}",
            P::SYNCED,
            before.trim()
        )));
    }
    phase.come_back(arch, asked)?;
    let at = phase.len();
    phase.type_line(P::READ_LINE)?;
    if phase.wait_after(at, P::READ_ANSWER, P::LINE_SECS).is_none() {
        return Err(P::failure(&format!(
            "`{}` after the reset did not answer {:?}: the file the first boot \
             wrote is not there",
            P::READ_LINE,
            P::READ_ANSWER
        )));
    }
    let synced = before
        .lines()
        .find(|l| l.contains(P::SYNCED))
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(format!(
        "swish-check ({arch}): typed `reboot`; {synced}; the machine reset{} and came back to a \
         prompt, and `{}` read back what the first boot wrote",
        firmware_marker(arch).map_or(String::new(), |fw| format!(" through {fw}")),
        P::READ_LINE
    ))
}

/// The console [`reboot_phase`] types at and reads, and what it types and waits for.
struct RebootPhase<'a> {
    seen: &'a std::sync::Arc<std::sync::Mutex<String>>,
    stdin: &'a mut std::process::ChildStdin,
}

impl RebootPhase<'_> {
    const BOOT_SECS: u64 = SWISH_CHECK_BOOT_SECS;
    const LINE_SECS: u64 = SWISH_CHECK_X86_LINE_SECS;
    const WRITE_LINE: &str = "echo survives a reboot > reboot.txt";
    const READ_LINE: &str = "wc reboot.txt";
    // `survives a reboot` and its newline: 18 bytes, 3 words, 1 line.
    const READ_ANSWER: &str = "1 3 18";
    // Without the `reboot:` prefix on purpose. The shell draws its next prompt once `reboot`'s
    // report has ended, while the kernel writes these lines straight to the UART, so the prompt can
    // land inside the kernel's first line: merge group 37554001967 (2026-10-07 UTC) read
    // "reboo$t:  the kernel was asked to restart the machine" on riscv64. The tail is the claim.
    const KERNEL_ASKED: &str = "the kernel was asked to restart the machine";
    const KERNEL_REFUSED: &str = "every reset route was refused";
    const SYNCED: &str = "reboot: filesystem synced";

    /// Every error this phase returns names it, so a red leg says it was the reboot and not a
    /// scripted line.
    fn failure(why: &str) -> String {
        format!("reboot phase: {why}")
    }

    fn text(&self) -> String {
        degauge(&self.seen.lock().expect("transcript lock"))
    }

    fn len(&self) -> usize {
        self.text().len()
    }

    /// Where `needle` first appears after `from` in the degauged transcript, waiting up to `secs`.
    fn wait_after(&self, from: usize, needle: &str, secs: u64) -> Option<usize> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            let t = self.text();
            if let Some(at) = t.get(from..).and_then(|rest| rest.find(needle)) {
                return Some(from + at);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        None
    }

    fn wait_prompt(&self, secs: u64) -> bool {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            if self.seen.lock().expect("transcript lock").ends_with("$ ") {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }

    fn type_line(&mut self, line: &str) -> Result<(), String> {
        writeln!(self.stdin, "{line}")
            .and_then(|()| self.stdin.flush())
            .map_err(|e| format!("reboot phase: could not type `{line}`: {e}"))
    }

    /// **From the kernel's restart line to a bare prompt on the second boot**: the firmware's line
    /// where there is firmware, no refusal, a second banner, and on `x86_64` the hand-over report
    /// and an Enter.
    fn come_back(&mut self, arch: &str, asked: usize) -> Result<(), String> {
        let resumed = match firmware_marker(arch) {
            Some(fw) => match self.wait_after(asked, fw, Self::BOOT_SECS) {
                Some(at) => at,
                None => {
                    let refused = self.text()[asked..].contains(Self::KERNEL_REFUSED);
                    return Err(Self::failure(&if refused {
                        "the firmware refused every reset route".to_string()
                    } else {
                        format!("no {fw:?} line after the reset: the machine did not come back")
                    }));
                }
            },
            None => asked,
        };
        if self.text()[asked..].contains(Self::KERNEL_REFUSED) {
            return Err(Self::failure("the firmware refused every reset route"));
        }
        let Some(second) = self.wait_after(resumed, "nife capability shell", Self::BOOT_SECS)
        else {
            return Err(Self::failure(&format!(
                "no second prompt banner within {}s of the reset",
                Self::BOOT_SECS
            )));
        };
        if arch == "x86_64" {
            if self
                .wait_after(second, X86_HAND_OVER_REPORT, Self::BOOT_SECS)
                .is_none()
            {
                return Err(Self::failure(
                    "the second x86_64 boot never finished its hand-over report",
                ));
            }
            self.type_line("")?;
        }
        if !self.wait_prompt(Self::LINE_SECS) {
            return Err(Self::failure("the second boot never reached a bare prompt"));
        }
        Ok(())
    }
}

/// **The screendump decoded back into rows of text, in the terminal's own scheme.**
///
/// The decoder is `board_console::screen::decode_exact` (milestone 377 (two screendump decoders,
/// in two crates, reading the same font), which deleted this leg's own `parse_ppm`,
/// `decode_cell` and `scanout_rows`); what this call site contributes is the
/// three things that belong to the leg and not to a decoder:
///
/// - **The geometry assertion `scanout_rows` used to carry**, kept here where it belongs: this
///   leg grades the graphical terminal's own surface, and a scanout of any other size is a
///   `SET_SCANOUT` bug, not a near miss. `Err` rather than a full grid of `?`s, so a wrong-sized
///   dump is skipped and the next poll tries again, exactly as before the merge.
/// - **The colors**: `video_terminal::Attr::DEFAULT`'s, read from that crate so the scheme
///   stays the terminal's own choice rather than a constant copied here.
/// - **The font**: `glyph_atlas::REGULAR` at `video_terminal`'s cell, drawn 1:1, the same function
///   `Vt::pixel` paints with and [`super::super::scanout`]'s exact text check grades.
///
/// The alphabet is the caller's: this leg only ever searches for what it itself types.
fn screen_rows(bytes: &[u8], alphabet: &[u8]) -> Result<board_console::screen::Screendump, String> {
    let (fg, bg) = video_terminal::Attr::DEFAULT.colours();
    let screen = board_console::screen::decode_exact(
        bytes,
        video_terminal::CELL_W,
        video_terminal::CELL_H,
        1,
        alphabet,
        |ch, x, y| glyph_atlas::REGULAR.cell_pixel(ch, x, y, fg, bg),
    )
    .map_err(|e| e.to_string())?;
    if (screen.width, screen.height) != (graphics_contract::WIDTH, graphics_contract::HEIGHT) {
        return Err(format!(
            "scanout is {}x{}, the surface is {}x{}",
            screen.width,
            screen.height,
            graphics_contract::WIDTH,
            graphics_contract::HEIGHT
        ));
    }
    Ok(screen)
}

/// **Launch `graphical_terminal` from the prompt a boot is sitting at, and read the screen**
/// (the second and third steps of [`Keystrokes`]'s doc). Called by [`swish_check_boot`] once its
/// script has finished and the prompt is back, on a boot that has a virtio-gpu attached; `sock` is
/// that gpu's monitor socket, which `screendump` reads. `Ok` carries the sentence to print.
pub(super) fn launch_graphical_terminal(
    arch: &str,
    keystrokes: Keystrokes,
    stdin: &mut std::process::ChildStdin,
    seen: &std::sync::Arc<std::sync::Mutex<String>>,
    sock: &str,
) -> Result<String, String> {
    // **Launch.** `graphical_terminal\n` over the UART, exactly as a person would. The prompt is
    // out and nothing was echoed since it appeared, so the line is read by the prompt, not
    // buffered ahead of it.
    stdin
        .write_all(b"graphical_terminal\n")
        .and_then(|()| stdin.flush())
        .map_err(|e| format!("could not type the launch command: {e}"))?;
    // Where the UART transcript stood at the launch, so a launch that fails can say what the shell
    // and the progenitor printed after it rather than only that the screen stayed dark.
    let launched_at = seen.lock().expect("transcript lock").len();

    // `a`..`z`, `0`..`9`, space and `$`: every byte this check's own reads look for, plus enough of
    // the alphabet that a decode failure names the wrong character instead of silently reading `?`
    // for one this check simply never bothered to include.
    let mut alphabet: Vec<u8> = (b'a'..=b'z').collect();
    alphabet.extend(b'0'..=b'9');
    alphabet.push(b' ');
    alphabet.push(b'$');

    // **The session's prompt, on the screen.** This is the silent-failure assertion: a collision
    // anywhere from the shell's seven slots through the session build fails without a word, so any
    // `$ ` decoded off the scanout at all disproves one. The deadline covers the session's whole
    // build, not just its first paint.
    let shot = workspace_root().join(format!("target/gpu-swish-check-{arch}.ppm"));
    let deadline = Instant::now() + Duration::from_secs(SWISH_CHECK_BOOT_SECS);
    let mut prompt_row: Option<String> = None;
    while Instant::now() < deadline && prompt_row.is_none() {
        if screendump(sock, &shot)
            && let Ok(bytes) = std::fs::read(&shot)
            && let Ok(screen) = screen_rows(&bytes, &alphabet)
        {
            prompt_row = screen.rows.into_iter().find(|r| r.contains("$ "));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let Some(before) = prompt_row else {
        return Err(format!(
            "`graphical_terminal` was launched but no `$ ` prompt reached the scanout within \
             {SWISH_CHECK_BOOT_SECS}s (see {}). A capability-slot collision fails in silence, so \
             this is the check's central assertion. The UART after the launch: {:?}",
            shot.display(),
            &seen.lock().expect("transcript lock")[launched_at..],
        ));
    };
    eprintln!("swish-check ({arch}, graphical): session prompt found: {before:?}");

    // The one keystroke, the same key (and the same reason) the kernel test's own keyboard test
    // uses: `video_terminal::script::HOST_KEY` is the one definition of which key, so a driver
    // that mapped the evdev code wrong fails in exactly one place instead of two. The serial arm
    // sends the same key as the byte it already is, since a UART carries no scancode for a keymap
    // to get wrong; `HOST_KEY_BYTE` is that byte, defined beside `HOST_KEY` so the two spellings
    // of one key cannot drift.
    match keystrokes {
        Keystrokes::Device => sendkey(sock, video_terminal::script::HOST_KEY),
        Keystrokes::Serial => stdin
            .write_all(&[video_terminal::script::HOST_KEY_BYTE])
            .and_then(|()| stdin.flush())
            .map_err(|e| format!("could not type into the UART: {e}"))?,
    }

    let want = format!("$ {}", video_terminal::script::HOST_KEY);
    let deadline = Instant::now() + Duration::from_secs(SWISH_CHECK_LINE_SECS);
    let mut typed_row: Option<String> = None;
    while Instant::now() < deadline && typed_row.is_none() {
        if screendump(sock, &shot)
            && let Ok(bytes) = std::fs::read(&shot)
            && let Ok(screen) = screen_rows(&bytes, &alphabet)
        {
            typed_row = screen.rows.into_iter().find(|r| r.contains(&want));
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    match typed_row {
        Some(after) => {
            let by = match keystrokes {
                Keystrokes::Device => {
                    "the session's `keyboard_driver`, through its line discipline"
                }
                Keystrokes::Serial => {
                    "the boot's line discipline in raw mode, echoed by the session itself"
                }
            };
            Ok(format!(
                "swish-check ({arch}, graphical): the session's prompt reached the screen, and a \
                 key press reached it back through {by}: {after:?}"
            ))
        }
        None => {
            let blame = match keystrokes {
                Keystrokes::Device => {
                    "the session's `keyboard_driver` came up but its bytes are not reaching the \
                     discipline, or the host's `sendkey` is not reaching the device"
                }
                Keystrokes::Serial => {
                    "the session is not reading the boot discipline raw, or the byte written to \
                     the runner's stdin is not reaching the guest's UART"
                }
            };
            Err(format!(
                "the session's prompt appeared ({before:?}) but the key press ({:?}) never echoed \
                 back within {SWISH_CHECK_LINE_SECS}s (see {}): {blame}",
                video_terminal::script::HOST_KEY,
                shot.display(),
            ))
        }
    }
}

/// **`bytes` as text, holding back a character the read cut in two.** A pipe read ends wherever the
/// 1024-byte buffer fills, so a multi-byte character can straddle two reads, and decoding each read
/// alone turned the halves into two replacement characters: `§` arrived as `��` in one line of
/// `rg`'s walk on 2026-10-07 (milestone 121), the only one of 349 that differed from the host's.
/// An incomplete sequence at the end waits in `split` for the next read; bytes that are invalid
/// anywhere else are still replaced, as before.
pub(super) fn whole_characters(split: &mut Vec<u8>, bytes: &[u8]) -> String {
    split.extend_from_slice(bytes);
    let keep = match std::str::from_utf8(split) {
        Ok(_) => 0,
        // `error_len() == None` is exactly "the input ended inside a character".
        Err(e) if e.error_len().is_none() => split.len() - e.valid_up_to(),
        Err(_) => 0,
    };
    let tail = split.split_off(split.len() - keep);
    let text = String::from_utf8_lossy(split).into_owned();
    *split = tail;
    text
}

/// **The line that walks [`crate::disk::RIPGREP_CORPUS`]**, typed in `/search` on the second boot:
/// `rg` by its bare name, the program `jig install ripgrep` installed from basalt's index, looking
/// for [`crate::disk::RIPGREP_WALK_PATTERN`]. Spelled out because a `Line` wants a `&'static str`;
/// a test holds it to the two constants the disk is staged from.
pub(super) const RG_WALK_LINE: &str = "rg BUGS crates";

/// **What [`RG_WALK_LINE`] would grant, asked before it runs**: `caps` previews the same words.
pub(super) const RG_WALK_PREVIEW: &str = "caps rg BUGS crates";

/// **Where [`RG_WALK_LINE`] is typed**, [`crate::disk::RIPGREP_WALK_HOME`]; skipped with it, since
/// the directory is on the disk only beside `rg`.
pub(super) const RG_WALK_CD: &str = "cd /search";

/// **How long [`RG_WALK_LINE`] may take**, typed to prompt-back, on every leg (milestone 121,
/// 2026-10-07 UTC). Measured on patagonia with other lanes building (load 15 to 60):
///
/// | leg | the walk | load average |
/// |---|---:|---:|
/// | aarch64, HVF | 14.9 s, 54.1 s | 24, 38 |
/// | riscv64, TCG | 40.8 s | about 20 |
/// | `x86_64`, TCG | 345.8 s, 432.1 s | 15 to 20 |
///
/// Ten minutes covers the slowest with room, and a hang still reports inside one CI job. Why the
/// emulated `x86_64` leg is eight to twenty times the others is not measured; the likeliest cost is
/// the one [`SWISH_CHECK_X86_LINE_SECS`] names, the screen path painting each of 349 lines through
/// TCG. CI's legs are measured in milestone 121's block (lane milestone/121-rg-ci).
pub(super) const RG_WALK_SECS: u64 = 600;

/// **How long any other line naming `rg` may take** (the three milestone 595 typed, and its `caps`).
/// Loading a 2.8 MiB image is most of it: under the same load those lines took 3.8 to 30.1 s on
/// aarch64 and up to 38.7 s on `x86_64`, so the shared 30 s bound failed them on two legs before
/// this. Where the time goes inside a spawn of that size is not measured (milestone 121's BUGS).
///
/// The two lines that install `rg` from basalt and run it (milestone 810 (`ripgrep` is packaged
/// in basalt and installed with `jig`)) take it too: fetching 2.8 MiB over software TLS under TCG
/// took 21.0 s on aarch64, 28.7 s on riscv64 and 46.1 s on x86_64 under TCG, on patagonia under
/// load (2026-10-12 UTC).
pub(super) const RG_LINE_SECS: u64 = 90;

/// **Whether `rg`'s answer at the prompt is the host's answer**, as a set of lines: every line the
/// host's `rg` printed must appear exactly as often in `answer`, and nothing else may. A set and
/// not a sequence, because the order `rg` walks a directory in is the order the filesystem lists
/// it, and RedoxFS and APFS list differently; nothing about the order is a claim this line makes.
/// `Ok` carries a sentence for the log, `Err` the failure with the first differences.
pub(super) fn rg_walk_verdict(answer: &str, expected: &str) -> Result<String, String> {
    use std::collections::BTreeMap;
    let count = |text: &str| {
        let mut lines: BTreeMap<String, usize> = BTreeMap::new();
        // The shell indents the first line of an answer (`  crates/abi/...`, every leg), and every
        // line `rg` prints here starts with the path, so leading blanks are never `rg`'s.
        for line in text.lines().map(|l| l.trim_end_matches('\r').trim_start()) {
            *lines.entry(line.to_string()).or_default() += 1;
        }
        lines
    };
    let (got, want) = (count(answer), count(expected));
    let wanted: usize = want.values().sum();
    if wanted == 0 {
        return Err(
            "the host's `rg` found nothing in the corpus, so there is nothing to compare".into(),
        );
    }
    let differ = |a: &BTreeMap<String, usize>, b: &BTreeMap<String, usize>| {
        a.iter()
            .filter(|(line, n)| b.get(*line).copied().unwrap_or(0) < **n)
            .map(|(line, _)| line.clone())
            .collect::<Vec<_>>()
    };
    let (missing, extra) = (differ(&want, &got), differ(&got, &want));
    let files = want
        .keys()
        .filter_map(|l| l.split_once(':').map(|(f, _)| f))
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    if missing.is_empty() && extra.is_empty() {
        Ok(format!(
            "`{RG_WALK_LINE}` printed the host's {wanted} lines from {files} files, no more and no fewer"
        ))
    } else {
        Err(format!(
            "`{RG_WALK_LINE}` differs from the host's `rg` over the same corpus ({wanted} lines from \
             {files} files): {} missing, {} extra; first missing {:?}, first extra {:?}",
            missing.len(),
            extra.len(),
            &missing[..missing.len().min(3)],
            &extra[..extra.len().min(3)],
        ))
    }
}

/// **What the prompt printed in response to the first `line` at or after `from`**, plus where to
/// resume looking. `None` when that line is not in the transcript at all.
///
/// `kernel::user::pipeline_service::answer` does this inside the guest and this does it on the
/// host, for the same reason: an assertion should be able to name the command it is about instead
/// of counting lines.
pub(super) fn swish_check_answer<'a>(
    transcript: &'a str,
    from: usize,
    line: &str,
) -> Option<(&'a str, usize)> {
    let echo = format!("$ {line}\n");
    let at = from + transcript[from..].find(&echo)? + echo.len();
    let rest = &transcript[at..];
    // The answer runs to the next prompt, which is always at the start of a line, and `rest` begins
    // at one because the echo consumed its own newline. So a command that printed nothing has an
    // empty answer rather than swallowing the line after it, which is the case `>` and `>>` are
    // and which the first version of this got wrong.
    let end = if rest.starts_with("$ ") {
        0
    } else {
        rest.find("\n$ ").map(|e| e + 1).unwrap_or(rest.len())
    };
    Some((&rest[..end], at + end))
}

#[cfg(test)]
mod tests;
