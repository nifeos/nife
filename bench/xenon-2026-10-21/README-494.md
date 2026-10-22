# xenon, 2026-10-21 (UTC): milestone 494's I219 bring-up, with no cable

Supporting evidence for milestone 494 (a driver for the network card a PC actually has), not its
exit criterion. xenon had no Ethernet cable that evening, so the bench step (a DHCP lease and a
timed transfer) did not run, and `PCH_PROVEN_ON_SILICON` stays `false`.

The I219's PCH bring-up did run on xenon, inside milestone 321 (a corpse that did not park, on four
real cores)'s unfiltered `system_tests` boot: `main` at `5ee9b4afb`, `cargo xtask uefi-test`'s
`system_tests` image, booted from a stick at about 23:23 UTC. The whole log is
`321-system-tests-e1000e-ceiling.log`, added by pull request #2536. The lines this milestone cares
about:

```
test system_tests::user::e1000e_tests::a_client_completes_a_udp_round_trip_through_the_e1000e_nic ...   e1000e: UlpByMe { cleared: true }
  e1000e: PhyReached { at: Directly, id: 0x015400a0 }
  e1000e: LcdConfigured { pairs: 22 }
```

So, on silicon: the Management Engine left ULP when asked, the PHY answered directly in PCIe mode
with `0x015400a0` (the identifier the runbook expects), and the LCD configuration loaded 22 pairs
from the NVM. The test then waited for a link that no cable could bring and hit the harness's 90 s
ceiling (pull request #2530 makes it report no link instead).
