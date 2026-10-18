# Reconciliation for the 2026-10-03 audit: seven items from the component survey

The maintainer read the delegated component survey behind
[the report](../2026-10-03-eight-constants-and-thirteen-components.md) and found seven items its
findings table did not carry. Each is confirmed with a home or refuted with evidence here, at the
tree the follow-up read (`b9b793f5f`, the report's own landing commit). The report's findings 10
to 16 are the summaries; this is the evidence.

## (a) Confirmed: the shell keeps the display devices for its whole life

`crates/system_initializer/src/lib.rs`, the `slots` table in the login block: the progenitor
places `g.gpu` and `g.keyboard` with `WRITE | GRANT`, `g.gpu_irq` and `g.keyboard_irq` with
`READ | GRANT`, and the DMA run, the surface and the keyboard DMA page with
`READ | WRITE | GRANT`, at `spawnproto::SHELL_GPU_SLOT` onward. `components/src/swish.rs`,
`delegate_display`: "We keep our own copies ... so the session can be run again once it ends."

Can the shell map those frames? Yes. `map_page_frame` in `swish.rs` passes the `tables` region
the shell holds, and the kernel's `MAP_RW` check (`kernel/src/syscall.rs`, `page_frame_map`)
asks for `WRITE` on the frame, which the shell has on all three pages. Can two parties wait on
one interrupt? Yes. `irq_notify` (`kernel/src/sched.rs`) wakes the one waiter `rendezvous.signal()`
returns; a `RECV` needs `READ`, which the shell has on both interrupt rendezvous. Milestone 603
(an interrupt's endpoint refuses every send) refuses sends, not receives.

The report's trigger answer said no component took device authority. True of the thirteen; the
authority moved into the shell, an existing component, which is the case the trigger cannot count
and the report should have named. Home: finding 10, a BUGS entry in `swish.rs`, and the proposal
`design/roadmap/0715-the-spawn-service-holds-the-display-grants-and-the-shell-holds-none.md`.

## (b) Confirmed: a real capability passes the `NO_CAP` guard

`kernel/src/syscall.rs`, the `SEND_CAP` arm: `WRITE` on the endpoint, `GRANT` on the source, and
the narrowed rights a subset of the source's. Any client of a server holds the first; a rendezvous
retyped from the client's own budget gives it the second. `ipc_recv_cap` installs the capability
and returns its slot in `x1`. `user_mode_runtime::reply` is `invoke(slot, abi::reply::REPLY, ..)`
and `REPLY` is `0`, which is `SEND` on a rendezvous, `SIGNAL` on a notification, `ARM` on a timer,
`MAP` on a region or frame, `MAP_INTO` on an address space, `CONFIGURE` on a thread control block
and `WAIT` on an interrupt (`crates/abi`). `components/src/compositor.rs` calls
`reply(reply_slot, ..)` with no guard; `components/src/net_stack.rs` guards `NO_CAP` only.

So a client that `SEND_CAP`s its own rendezvous parks the server in `SEND` for the life of the
machine, and every non-Reply delivery leaves a slot in the server's table (a Reply is one-shot and
deleted on use; nothing else is). Not demonstrated under QEMU. Home: finding 11, a BUGS entry at
`abi::rendezvous::RECV_CAP`, the proposal
`design/roadmap/0706-a-call-server-can-tell-a-reply-from-a-delegation.md` with the options
costed for calef, and a risk 7 input in the report.

## (c) Confirmed and fixed: a stale `outgoing_cap` survived teardown

`kernel/src/sched.rs`: `ipc_send_cap` stages the delegation in `Thread::outgoing_cap` when it
parks; `ipc_recv_cap` takes it on the `FromSender` arm. `reclaim_region`'s drain calls
`set_ipc_aborted` and wakes each parked sender, and `set_ipc_aborted` only aborted the handshake.
The plain `SEND` park path writes the mailbox and parks without touching `outgoing_cap`. So a
sender whose `SEND_CAP` was aborted by teardown and then plain-`SEND`s on another rendezvous hands
the staged capability to that rendezvous's `RECV_CAP` receiver. The three sweeps at revocation
clear it only when its object is the revoked one.

Fixed in `set_ipc_aborted`, which now clears `outgoing_cap`: an abort means no receiver will take
it, and the sender keeps its source copy. Test:
`system_tests/src/user/recv_cap_attack_tests.rs`,
`a_send_cap_aborted_by_teardown_stages_nothing_for_a_later_plain_send`, on all three ISAs in CI.

## (d) Confirmed and fixed: `caps` read the placeholder's manifest

`crates/swish/src/lib.rs`, `write_preview_rows`, took `e.prog.manifest().machine` and `.share`
where every other row reads `m`, the manifest its caller passes: the note's for an image, the row's
for an archive program. A note can declare both (`crates/capability_manifest_note`, fields 53 and 54). Fixed
to read `m`; the host test `caps_of_an_image_names_its_provenance_the_gate_and_what_the_note_asks`
gains the case, red before the fix.

## (e) Confirmed and fixed: the moved `cfg(test)`

`system_tests/src/user.rs`: milestone 634 (a plain SEND received by RECEIVE_CAP never hands the
receiver a sender-chosen slot) inserted `mod recv_cap_attack_tests;` between `#[cfg(test)]` and
`mod revocation_in_flight_tests;`. Both now carry their own attribute, and `script/lint`'s "tests
the suite cannot see" check refuses a bare `mod` in that file: run against the pre-fix file it
names line 944, against the fixed one it passes.

## (f) Confirmed and recorded: kernel deliveries read as the spawner or a writer

`crates/system_log/src/lib.rs`, `Log::handle`: badge `0` is control, anything else a writer. An
interrupt signal and a bound notification deliver with word 3 zero; a death message under §26
(the fault endpoint: thread death becomes a message a supervisor holds) delivers the fault address
there. Recorded in the crate's BUGS as a rule on the intake endpoint, ahead of
milestone 342 (the kernel and the `console` server drive one UART from two address spaces), which
is the first to wire the service in.

## (g) Refuted: the terminal's 4 KiB `line` array fits its stack

`components/src/graphical_terminal.rs` holds `[0u8; 4096]` at two sites, one per arm, never both
live. The session is built by `build_child` (`crates/system_initializer/src/lib.rs`,
`build_graphical_terminal_session`), and every child it builds gets `CHILD_STACK_PAGES`, twelve
pages, 48 KiB, mapped down from `supervision_protocol::CHILD_STACK_VA`. The "one 4 KiB stack page"
in `system_log`'s BUGS is the kernel test spawner's, not the progenitor's. CI's stack-frames job
bounds any frame at a third of a thread stack. No finding.
