# xenon, 2026-10-21 (UTC): milestone 890's boot of `main`

One boot that milestone 890 (riscv64 and x86_64 join the shared boot path) added. `main` at
`5ee9b4afb`, built with `cargo xtask uefi-image` and copied to a FAT32 stick as
`/EFI/BOOT/BOOTX64.EFI`. calef booted it at about 23:03 UTC with F12 at POST.

The boot was from the stick, not over the network as the block's runbook says: the bench step for
a wired xenon waits, and the image is the same file either way. No monitor was attached. With one
attached, this commit's hand-over panics on xenon before the prompt (milestone 242 (USB host and HID, because on
commodity hardware the keyboard is not a UART)'s bench README, and pull request #2528).

The console was read by a raw reader (115200 8N1) for the whole evening and cleaned with
`briefs/bench-session.md` step 6's `tr`. The two lines typed at the prompt were written by a second,
write-only open of the port, one byte every 30 ms:

| UTC | sent |
|---|---|
| 23:04:36 | `echo typed on the wire` and a carriage return |
| 23:05:20 | `echo hello` and a carriage return |

| file | what it shows |
|---|---|
| `boot-to-prompt.log` | the tour, the hand-over and the prompt, with both lines echoed and run |

What the block asks for, from the log:

- `io apic     : id 2 at 0xfec00000 up, version 0x20, 120 redirection entries, gsi base 0`
- `device irq  : pit irq 0 -> gsi 2 on vector 0x32: 21 interrupts in ~0.2s at 100 Hz`
- `smp: 4 core(s) online`, after `smp: 4 core(s) in the madt`
- every tour line, `nife self-test: 10 of 13 passed, 3 absent: blockdev clock trng`
- the prompt, then `echo hello` printing `hello`
