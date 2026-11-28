# xenon, 2026-11-28 (UTC): the I219 cabled run

One boot of the `network_bench` kernel on xenon's I219-LM (`8086:15e3`), cabled to a switch, run by
[notes/e1000e/the-i219-cabled-run.md](https://github.com/nifeos/record/blob/main/notes/e1000e/the-i219-cabled-run.md).
It is the boot that `e1000e_service::PCH_PROVEN_ON_SILICON` cites.

| item | value |
|---|---|
| nife commit | `4c86f224b39e1c36225aa04e8715b602ba2b49a0` |
| image SHA-256 | `812730dbe71d3569e5dcc7a24f40caa4c24060ebb2adbacfa27744775da16619` |
| booted | about 03:44 UTC |
| log | `i219-cabled-run.log`, SHA-256 `c9e0b784faa726f31ff0a3c0cee269efc415a57e6665ac9becdc68ca6be8935c` |
| verdict | `LEASED-AND-MEASURED`, 104857600 bytes in 22.200 s = 37.785 Mbit/s |

The log is the serial console as captured, with no line changed or removed. It keeps the NIC's MAC
address and the private LAN addresses it printed. Unit-specific details of the bench are kept
off-tree by the maintainers.
