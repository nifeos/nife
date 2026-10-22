# xenon, 2026-10-21 (UTC): milestone 321's suites on four real cores

Three boots for milestone 321 (a corpse that did not park, on four real cores): the x86_64 test
images on xenon, an Intel Core i5-7500T with four cores and no hyperthreading. On silicon there is
no `NIFE_SMP`; the kernel brings up what the MADT lists, and every boot printed `smp: 4 core(s)
online`. Each image came from `cargo xtask uefi-test` at `main` `5ee9b4afb` (which also passed
both suites under OVMF on patagonia), was copied to a stick and booted with F12 at POST. The
console was read by a raw reader that sends nothing and cleaned with `briefs/bench-session.md`
step 6's `tr`.

| file | UTC | image | result |
|---|---|---|---|
| `321-kernel-suite.log` | ~23:20 | the kernel's own tests | `test result: ok. 138 passed, 13 skipped`; every skip names hardware xenon lacks (AMD-Vi, a device tree, a virtio disk) |
| `321-system-tests-e1000e-ceiling.log` | ~23:23 | `system_tests`, unfiltered | 47 passed before `e1000e_tests::a_client_completes_a_udp_round_trip_through_the_e1000e_nic` spun past the harness's 90 s ceiling with no cable in the card; the suite stopped there, before `survey_tests` (pull request #2530) |
| `321-survey-tests.log` | ~23:43 | `system_tests` built with `NIFE_TEST_FILTER=survey_tests` | `running 6 of 385 tests`, `test result: ok. 6 passed` |

The test this milestone is about, `a_dead_child_is_still_in_the_domain_until_it_is_reaped`, passed
in the third boot in 33 ms. That is one clean run in one, on four real cores. The original failure
was also seen once, so one pass says nothing about its frequency. The harness cannot repeat a test
within a boot, so each further run costs a boot.
