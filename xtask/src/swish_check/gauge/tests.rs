//! Tests of the gauge filter, `degauge` and the redraw reader.

use super::*;

/// Feed `chunks` through a [`GaugeFilter`] and return what the checks would read, and the gauges.
fn filtered(chunks: &[&str]) -> (String, Vec<(usize, String)>) {
    let mut f = GaugeFilter::default();
    let (mut out, mut gauges) = (String::new(), Vec::new());
    for c in chunks {
        f.feed(c, &mut out, &mut gauges);
    }
    (out, gauges)
}

#[test]
fn the_stack_gauge_is_taken_out_from_between_a_prompt_and_its_echo() {
    // What aarch64 printed on the first run with the gauge in it, which read as no answer.
    let (out, gauges) = filtered(&[
        "  1 2 12\n$   progenitor stack: 22880 of 32768 bytes at peak, 9888 spare\n",
        "echo hello world > gate.txt\n$ ",
    ]);
    assert_eq!(out, "  1 2 12\n$ echo hello world > gate.txt\n$ ");
    assert_eq!(gauges.len(), 1);
    assert_eq!(gauges[0].0, "  1 2 12\n$ ".len());
    assert!(gauges[0].1.starts_with("progenitor stack: 22880"));
}

#[test]
fn a_gauge_split_across_reads_is_held_back_and_never_emitted() {
    // Every split point, because the reader thread's chunks fall wherever the UART's did.
    let whole = "$   progenitor stack: 19000 of 32768 bytes at peak, 13768 spare\nwc\n";
    for cut in 0..whole.len() {
        let mut f = GaugeFilter::default();
        let (mut out, mut gauges) = (String::new(), Vec::new());
        f.feed(&whole[..cut], &mut out, &mut gauges);
        let first = out.clone();
        f.feed(&whole[cut..], &mut out, &mut gauges);
        assert!(
            out.starts_with(&first),
            "emitted text was taken back at cut {cut}"
        );
        assert_eq!(out, "$ wc\n", "cut {cut}");
        assert_eq!(gauges.len(), 1, "cut {cut}");
    }
}

#[test]
fn a_bare_prompt_is_emitted_whole_although_its_space_could_start_a_gauge() {
    let (out, _) = filtered(&["commands: ...\n$ "]);
    assert!(out.ends_with("$ "));
    // The same prompt, read as two chunks, which is how the third local run with the gauge hung.
    let (out, _) = filtered(&["up 00:00:06\n$", " "]);
    assert!(out.ends_with("$ "));
    let (out, gauges) = filtered(&["$  ", " progenitor stack: 1 of 2 bytes at peak\n", "wc\n"]);
    assert_eq!(out, "$ wc\n");
    assert_eq!(gauges.len(), 1);
}

#[test]
fn degauge_takes_the_kernels_newline_with_a_spliced_gauge() {
    // #1377's riscv64 swish-check of 2026-10-03, verbatim: the stack gauge landed inside the
    // echo of the first line and its newline split the echo.
    let spliced = "$ echo  progenitor sta hck: 20576 of 49152 bytes at peak, 28576 spare\n\
                   ello world | wc\n  1 2 12\n";
    assert_eq!(degauge(spliced), "$ echo hello world | wc\n  1 2 12\n");
}

#[test]
fn a_slot_gauge_after_the_first_prompt_leaves_the_prompt_last() {
    // Milestone 152's swish-check transcript, verbatim but for the chunking: the slot peak
    // settled after the first prompt, and with only the stack gauge filtered the wait for
    // `$ ` saw the slot line after it and timed out.
    let (out, gauges) = filtered(&[
        "status\n$ ",
        "  capability slots: 24 of 32 at peak\n",
        "  progenitor stack: 20600 of 49152 bytes at peak, 28552 spare\n",
    ]);
    assert!(out.ends_with("$ "), "{out:?}");
    assert_eq!(gauges.len(), 2);
}

/// What the gate reads as `line`'s answer, from the UART's chunks, by the path the run takes.
fn answer_as_the_gate_reads(chunks: &[&str], line: &str) -> Option<String> {
    let (out, gauges) = filtered(chunks);
    let out = without_redraws(&out, &gauges);
    swish_check_answer(&out, 0, line).map(|(a, _)| a.to_string())
}

#[test]
fn a_console_redraw_beneath_a_gauge_is_not_read_as_a_second_prompt() {
    // Run 37167978481 (merge_group, 2026-10-04 UTC), aarch64, verbatim but for the chunking:
    // the 250 ms `OP_FLUSH` fell between the last typed character's echo and Enter's, so the
    // console drew a line end, the gauge and the partial line again, and the answer came under it.
    let chunks = [
        "          'quote a whole word'   and   ;  &&  ||   with  echo $?  for the status\n$ ",
        "packages/noteless/0.1.0/noteless",
        "\n  progenitor stack: 23784 of 49152 bytes at peak, 25368 spare\n",
        "$ packages/noteless/0.1.0/noteless",
        "\n  progenitor stack: 24568 of 49152 bytes at peak, 24584 spare\n",
        "  noteless: installed, and carrying no manifest note\n$ ",
    ];
    let answer = answer_as_the_gate_reads(&chunks, "packages/noteless/0.1.0/noteless");
    assert_eq!(
        answer.as_deref().map(str::trim),
        Some("noteless: installed, and carrying no manifest note")
    );
}

#[test]
fn a_redraw_beneath_two_kernel_lines_is_taken_out_once() {
    // Run 37153714653 (pull_request, 2026-10-03 UTC): the slot gauge and the stack gauge were
    // both queued, so one flush wrote both above the redraw, at one offset.
    let chunks = [
        "for the status\n$ packages/noteless/0.1.0/noteless\n",
        "  capability slots: 24 of 32 at peak\n",
        "  progenitor stack: 23784 of 49152 bytes at peak, 25368 spare\n",
        "$ packages/noteless/0.1.0/noteless\n",
        "  noteless: installed, and carrying no manifest note\n$ ",
    ];
    let (out, gauges) = filtered(&chunks);
    assert_eq!(gauges.len(), 2);
    assert_eq!(
        without_redraws(&out, &gauges),
        "for the status\n$ packages/noteless/0.1.0/noteless\n  noteless: installed, and \
         carrying no manifest note\n$ "
    );
}

#[test]
fn a_redraw_of_half_a_line_and_a_gauge_between_two_commands_are_left_alone() {
    // A flush mid-typing redraws a prefix, which the echo then completes; every reader already
    // finds the whole echo in it, so it stays as the terminal showed it.
    let (out, gauges) = filtered(&[
        "$ pack\n  progenitor stack: 1 of 2 bytes at peak, 1 spare\n$ pack",
        "ages/noteless/0.1.0/noteless\n  noteless\n$ ",
    ]);
    assert_eq!(without_redraws(&out, &gauges), out);
    // A gauge at a line start after a silent command is not a redraw: the next line differs.
    let (out, gauges) =
        filtered(&["$ echo hi > f\n  progenitor stack: 1 of 2 bytes at peak, 1 spare\n$ wc < f\n"]);
    assert_eq!(without_redraws(&out, &gauges), out);
}

#[test]
fn text_that_only_starts_like_the_gauge_is_let_through() {
    let (out, gauges) = filtered(&["$ echo   pro", "gress\n"]);
    assert_eq!(out, "$ echo   progress\n");
    assert!(gauges.is_empty());
}

#[test]
fn degauge_recovers_a_command_echo_the_gauge_was_spliced_into() {
    // #1371's CI failure, verbatim: the gauge landed byte by byte inside the echo of
    // `package install`, so neither `GaugeFilter` nor an exact search could find either.
    let spliced = "package   proinstgenitor sall tack: 31528 of 49152 bytes at peak, 17624 spare";
    let cleaned = degauge(spliced);
    assert!(
        cleaned.contains("package install"),
        "gauge was not cleanly separated from its echo: {cleaned:?}"
    );
    assert!(
        !cleaned.contains("progenitor") && !cleaned.contains("stack:"),
        "gauge text survived: {cleaned:?}"
    );
}

#[test]
fn degauge_removes_a_gauge_that_arrives_whole_between_two_lines() {
    // The ordinary case `GaugeFilter` handles live; `degauge`, a second pass at match time,
    // must not mishandle it.
    let text =
        "$ echo hi\nhi\n  progenitor stack: 22880 of 32768 bytes at peak, 9888 spare\n$ wc\n";
    let cleaned = degauge(text);
    // Newline and all, as `GaugeFilter` drops it (the blank line this left was the kernel's).
    assert_eq!(cleaned, "$ echo hi\nhi\n$ wc\n");
    assert!(cleaned.contains("$ wc\n"));
}

#[test]
fn degauge_takes_out_the_capability_slot_gauge_too() {
    let text = "$   capability slots: 17 of 24 at peak\n$ caps\n";
    let cleaned = degauge(text);
    assert!(!cleaned.contains("capability slots"));
    assert!(cleaned.contains("$ caps\n"));
}

#[test]
fn a_gauge_is_billed_to_the_command_whose_prompt_it_followed() {
    let t = "banner\n$ package install greeting\ninstalled\n$ ";
    assert_eq!(gauge_follows(t, t.len()), "package install greeting");
    assert_eq!(gauge_follows("banner\n$ ", 9), "(boot)");
}
