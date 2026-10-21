# radon, 2026-10-21 (UTC): milestone 890's keystroke path

Two boots that milestone 890 (riscv64 and x86_64 join the shared boot path) added. calef powered
radon on once, at about 15:25 UTC. The second boot came from `reboot` typed at the first one's
prompt. Each image was netbooted (`script/board-netboot`); the console was watched by
`script/board-console --until none` for the whole run and cleaned with `briefs/bench-session.md`
step 6's `tr`.

| file | image | what it shows |
|---|---|---|
| `890-plic-probe.log` | `main` at `6d83f1838` plus the branch's bench probe (`--extra-features plic_completion_probe`), SEALED | the stuck gateway, a byte let through per probe completion, and `reboot` |
| `890-keystroke-fix.log` | the branch's fix without the probe, SEALED | whole lines typed at full speed, and `reboot` |

`board-console` sends nothing but the soak's stop byte, so every byte below was written by a second,
write-only open of the port while it read. The capture holds the board's echo, not these bytes.

## Boot 1: the probe

The probe printed one `plic-probe:` line every 5 s and then completed source 32 on every context
that enabled it.

| UTC | sent | what the next probe line said | echoed |
|---|---|---|---|
| 15:26:11 | `e` | `routed=1` | `e` |
| 15:26:18 | `c`, after a probe completion | `routed=2`, nothing waiting | `c` |
| 15:26:43, 15:26:44 | `h`, then `o` 0.5 s later | `routed=3 pending=false iir=0xc4 lsr=0x61`: `o` waiting in the UART, its interrupt raised, the PLIC forwarding nothing | `h`, then `o` after the next completion (`routed=4`) |
| 15:27:13 to 15:27:29 | ` hi` and a carriage return, one byte per 5.5 s | one routed per byte | `echo hi` ran: `hi` |
| 15:27:35 to 15:28:08 | `reboot` and a carriage return, one byte per 5.5 s | one routed per byte | the board reset through the AXP15060 |

## Boot 2: the fix

| UTC | sent | echoed |
|---|---|---|
| 15:28:38 | `echo hello` and a carriage return, in one write | `hello` |
| 15:28:41 | `echo the quick brown fox jumps over the lazy dog` and a carriage return, in one write | the line |
| 15:28:44 | `ls` and a carriage return, one byte per 300 ms | `ls`'s answer |
| 15:29:05 | `reboot` and a carriage return, in one write | the board reset through the AXP15060 |
