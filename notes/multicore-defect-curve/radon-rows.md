# radon's exposure rows, and the notes behind them

*An appendix to [`notes/multicore-defect-curve.md`](../multicore-defect-curve.md), which defines the
columns and how a log becomes a row. This holds radon's rows, and what each bench evening's rows
mean and what they leave out.*

| id | start (UTC) | machine | arch | cores | build | workload | hours | crossings | beats | result | source |
|---|---|---|---|---|---|---|---|---|---|---|---|
| E1 | 2026-09-03 ~20:04 | radon | riscv64 | 4 | not recorded | soak-test | ~0.33 | ~3,000 at beat 12, final not recorded | not recorded | clean as far as recorded | [`soak.md`](../soak.md), "radon, on real silicon" |
| E2 | 2026-09-03 ~20:24 | radon | riscv64 | 4 | not recorded | soak-test | 2.97 | 5,507 | 2,137 | clean | [`soak.md`](../soak.md), "The three-hour run" |
| E3 | 2026-09-04 ~00:06 | radon | riscv64 | 4 | not recorded | soak-test, census build | ~0.4, end not recorded | not recorded | not recorded | clean as far as recorded | [`soak.md`](../soak.md), "The three-hour run" |
| E4 | 2026-09-25 ~01:02 | radon | riscv64 | 4 | `9e879f1e7` | soak-test | 8.16 | 4,108,581 | 5,818 | clean | `bench/radon-2026-09-25/soak-8h.log`; [`visionfive2.md`](../visionfive2.md), "The eight-hour soak, 2026-09-25" |
| E5 | 2026-10-09 ~18:34 | radon | riscv64 | 4 | `bef2b40dc` | soak-test-reboot | 0.03 | 20,474 | 23 | clean | `bench/radon-2026-10-10/soak-boot2.log` |
| E6 | 2026-10-09 ~18:37 | radon | riscv64 | 4 | `bef2b40dc` | soak-test | 10.28 | 5,569,327 | 7,344 | clean | `bench/radon-2026-10-10/soak-boot2.log` |
| E7 | 2026-10-10 ~12:11 | radon | riscv64 | 4 | `bef2b40dc` | soak-test | 3.88 | 610,897 | 2,763 | clean | `bench/radon-2026-10-10/soak-boot3.log` |
| E8 | 2026-10-10 ~18:12 | radon | riscv64 | 4 | `23f0c8d87` | soak-test-reboot | 0.03 | 104 | 24 | clean | `bench/radon-2026-10-10/592-optionb-four-cycles.log` |
| E9 | 2026-10-10 ~18:15 | radon | riscv64 | 4 | `23f0c8d87` | soak-test-reboot | 0.03 | 25,223 | 24 | clean | `bench/radon-2026-10-10/592-optionb-four-cycles.log` |
| E10 | 2026-10-10 ~18:18 | radon | riscv64 | 4 | `23f0c8d87` | soak-test-reboot | 0.03 | 22,116 | 24 | clean | `bench/radon-2026-10-10/592-optionb-four-cycles.log` |
| E11 | 2026-10-10 ~18:21 | radon | riscv64 | 4 | `23f0c8d87` | soak-test-reboot | 0.03 | 3,493 | 24 | clean | `bench/radon-2026-10-10/592-optionb-four-cycles.log` |
| E12 | 2026-10-10 ~18:24 | radon | riscv64 | 4 | `23f0c8d87` | soak-test-reboot, ended by power-off | 0.03 | 16,462 | 22 | clean | `bench/radon-2026-10-10/592-optionb-four-cycles.log` |
| E13 | 2026-10-20 ~19:58 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot | 0.03 | 25,488 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E14 | 2026-10-20 ~20:01 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot | 0.03 | 23,645 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E15 | 2026-10-20 ~20:04 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot | 0.03 | 3,924 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E16 | 2026-10-20 ~20:07 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E17 | 2026-10-20 ~20:10 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot | 0.03 | 12,820 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E18 | 2026-10-20 ~20:13 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot | 0.03 | 27,502 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E19 | 2026-10-20 ~20:16 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot | 0.03 | 23,899 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E20 | 2026-10-20 ~20:19 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot | 0.03 | 20,549 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E21 | 2026-10-20 ~20:22 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot | 0.03 | 3,933 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E22 | 2026-10-20 ~20:25 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot (not crossing) | 0.03 | 23 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E23 | 2026-10-20 ~20:28 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot | 0.03 | 26,469 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E24 | 2026-10-20 ~20:30 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot | 0.03 | 23,907 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E25 | 2026-10-20 ~20:33 | radon | riscv64 | 4 | `22c19e1d5` | soak-test-reboot | 0.03 | 26,080 | 24 | clean | `bench/radon-2026-10-20/soak-reboot-draws.log` |
| E26 | 2026-10-21 ~01:31 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 23,877 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E27 | 2026-10-21 ~01:33 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot (not crossing) | 0.03 | 22 | 21 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log`, an unexplained reset (`notes/soak/radon-2026-10-21.md`) |
| E28 | 2026-10-21 ~01:35 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot (not crossing) | 0.03 | 17 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E29 | 2026-10-21 ~01:37 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 23,790 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E30 | 2026-10-21 ~01:39 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 24,211 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E31 | 2026-10-21 ~01:42 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 25,438 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E32 | 2026-10-21 ~01:44 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 24 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E33 | 2026-10-21 ~01:46 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 3,950 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E34 | 2026-10-21 ~01:48 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 14,052 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E35 | 2026-10-21 ~01:50 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E36 | 2026-10-21 ~01:53 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E37 | 2026-10-21 ~01:55 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 20,648 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E38 | 2026-10-21 ~01:57 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 23,859 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E39 | 2026-10-21 ~01:59 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 23,803 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E40 | 2026-10-21 ~02:01 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 3,890 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E41 | 2026-10-21 ~02:04 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot (not crossing) | 0.03 | 16 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E42 | 2026-10-21 ~02:06 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 26,944 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E43 | 2026-10-21 ~02:08 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 24,547 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E44 | 2026-10-21 ~02:10 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 26,203 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E45 | 2026-10-21 ~02:13 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E46 | 2026-10-21 ~02:15 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 3,774 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E47 | 2026-10-21 ~02:17 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 20,414 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E48 | 2026-10-21 ~02:19 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 14,652 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E49 | 2026-10-21 ~02:21 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot | 0.03 | 25,728 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` |
| E50 | 2026-10-21 ~02:24 | radon | riscv64 | 4 | `cda4ceb55` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-21/soak-reboot-draws.log` and `soak-stop.log` |
| E51 | 2026-10-21 ~17:55 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 10,223 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E52 | 2026-10-21 ~17:57 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 10,116 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E53 | 2026-10-21 ~17:59 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 65 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E54 | 2026-10-21 ~18:02 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 24,169 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E55 | 2026-10-21 ~18:04 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 23,988 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E56 | 2026-10-21 ~18:06 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 11,545 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E57 | 2026-10-21 ~18:09 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 25,733 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E58 | 2026-10-21 ~18:11 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 9,666 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E59 | 2026-10-21 ~18:13 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E60 | 2026-10-21 ~18:16 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 3,782 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E61 | 2026-10-21 ~18:18 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 20,344 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E62 | 2026-10-21 ~18:20 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot (not crossing) | 0.03 | 22 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E63 | 2026-10-21 ~18:23 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot (not crossing) | 0.03 | 21 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E64 | 2026-10-21 ~18:25 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 10,400 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E65 | 2026-10-21 ~18:27 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E66 | 2026-10-21 ~18:30 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 23,738 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E67 | 2026-10-21 ~18:32 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 9,681 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E68 | 2026-10-21 ~18:34 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 27,003 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E69 | 2026-10-21 ~18:37 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 10,100 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E70 | 2026-10-21 ~18:39 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 26,207 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E71 | 2026-10-21 ~18:41 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 3,863 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E72 | 2026-10-21 ~18:44 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 26,019 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E73 | 2026-10-21 ~18:46 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 24,180 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E74 | 2026-10-21 ~18:48 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 6,998 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E75 | 2026-10-21 ~18:51 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 23,655 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E76 | 2026-10-21 ~18:53 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 25,517 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E77 | 2026-10-21 ~18:55 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot (not crossing) | 0.03 | 17 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E78 | 2026-10-21 ~18:58 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 24,044 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E79 | 2026-10-21 ~19:00 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 23,833 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E80 | 2026-10-21 ~19:02 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 24,506 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E81 | 2026-10-21 ~19:05 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot (not crossing) | 0.03 | 25 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E82 | 2026-10-21 ~19:07 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 10,441 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E83 | 2026-10-21 ~19:09 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E84 | 2026-10-21 ~19:12 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 24,167 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E85 | 2026-10-21 ~19:14 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot | 0.03 | 27,358 | 24 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E86 | 2026-10-21 ~19:16 | radon | riscv64 | 4 | `0e0866d93` | soak-test-reboot (not crossing) | 0.01 | 54 | 7 | clean | `bench/radon-2026-10-21/225-pmic-report-draws.log` |
| E87 | 2026-10-22 ~18:37 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 25,633 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E88 | 2026-10-22 ~18:39 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 3,919 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E89 | 2026-10-22 ~18:41 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 9,384 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E90 | 2026-10-22 ~18:44 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 24,310 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E91 | 2026-10-22 ~18:46 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 23,633 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E92 | 2026-10-22 ~18:48 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 17 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E93 | 2026-10-22 ~18:51 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 10,360 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E94 | 2026-10-22 ~18:53 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 23,707 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E95 | 2026-10-22 ~18:55 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 4,236 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E96 | 2026-10-22 ~18:57 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 24,365 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E97 | 2026-10-22 ~19:00 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 24,266 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E98 | 2026-10-22 ~19:02 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 10,048 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E99 | 2026-10-22 ~19:04 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 4,031 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E100 | 2026-10-22 ~19:07 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 25,514 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E101 | 2026-10-22 ~19:09 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 21,090 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E102 | 2026-10-22 ~19:11 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 23,797 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E103 | 2026-10-22 ~19:14 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E104 | 2026-10-22 ~19:16 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 28 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E105 | 2026-10-22 ~19:18 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 23,830 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E106 | 2026-10-22 ~19:21 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 20,469 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E107 | 2026-10-22 ~19:23 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 23,958 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E108 | 2026-10-22 ~19:25 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 24,009 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E109 | 2026-10-22 ~19:28 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 24,294 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E110 | 2026-10-22 ~19:30 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 10,287 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E111 | 2026-10-22 ~19:32 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E112 | 2026-10-22 ~19:35 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 25,543 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E113 | 2026-10-22 ~19:37 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 6,183 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E114 | 2026-10-22 ~19:39 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 23,726 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E115 | 2026-10-22 ~19:42 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 22 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E116 | 2026-10-22 ~19:44 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 24,006 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E117 | 2026-10-22 ~19:46 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E118 | 2026-10-22 ~19:49 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 24,196 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E119 | 2026-10-22 ~19:51 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 3,983 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E120 | 2026-10-22 ~19:53 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 10,173 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E121 | 2026-10-22 ~19:56 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 24,310 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E122 | 2026-10-22 ~19:58 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 3,873 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E123 | 2026-10-22 ~20:00 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 20 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E124 | 2026-10-22 ~20:02 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 22 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E125 | 2026-10-22 ~20:05 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 24,413 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E126 | 2026-10-22 ~20:07 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 23 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E127 | 2026-10-22 ~20:09 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 24,046 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E128 | 2026-10-22 ~20:12 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 151 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E129 | 2026-10-22 ~20:14 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 23,968 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E130 | 2026-10-22 ~20:16 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 22 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E131 | 2026-10-22 ~20:19 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 25,713 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E132 | 2026-10-22 ~20:21 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 984 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E133 | 2026-10-22 ~20:23 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 24 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E134 | 2026-10-22 ~20:26 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 25,821 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E135 | 2026-10-22 ~20:28 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 4,326 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E136 | 2026-10-22 ~20:30 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot (not crossing) | 0.03 | 19 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E137 | 2026-10-22 ~20:33 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.03 | 23,890 | 24 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |
| E138 | 2026-10-22 ~20:35 | radon | riscv64 | 4 | `80fd9428c` | soak-test-reboot | 0.00 | 381 | 2 | clean | `bench/radon-2026-10-22/249-lottery-draws.log` |

E26 through E50 are 2026-10-21's twenty-five reboot draws, one capture
([`../soak/radon-2026-10-21.md`](../soak/radon-2026-10-21.md)). E27's draw reset itself at about
t=105 s without printing the kernel's reset sequence. It is `clean` by the rule above, which reads
only the last beat; milestone 225 (run the soak on radon, argon and xenon)'s `BUGS` carries the reset.

E51 through E86 are one capture, with what the PMIC recorded at each draw:
[`../soak/radon-2026-10-21-pmic.md`](../soak/radon-2026-10-21-pmic.md).

E5 through E7 are one bench evening, 2026-10-09 to 2026-10-10 (milestone 225's redraw boots,
`bef2b40dc`, tree-identical to main `237cef6bc`). E5 is the 120-second draw of the rebooting soak that milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write) ended. Between its last beat and E6's `U-Boot SPL` banner, the board reset itself through OpenSBI's PMIC write and came back. That is 592's green outcome. E6 began on the self-reboot's netboot fetch and ended at its watcher's deadline. The board then soaked unwatched for about 7 hours until E7; those crossings are not recorded and are not evidence. E7's boot followed one discarded boot from the card. Its fetch failed because radon's ethernet cable was unplugged (`phy_startup() failed: -110` on both controllers). That boot never reached a heartbeat and is not a row. Zero
defects on all three.

The §259 ledger (at least 10 million crossings over at least 3 boots): counting the three plain
boots only, E4 + E6 + E7 = 10,288,805 crossings over 3 boots, which meets it. E5's reboot draw adds 20,474 on top. The series milestone 249 (the boot lottery is sampled by a person walking to the board) wants
is also unblocked by E5's reset, so future boots need no plug cycle.

E8 through E12 are milestone 592's option-B evening, `23f0c8d87`, one capture. Four 120-second draws
were each ended by the direct AXP15060 write (the reset milestone 592 (radon's cold reboot dies in
OpenSBI's PMIC write) built, proven twice-deterministic in the same log). A fifth boot ended when
calef powered the board off. The crossings spread across five identical 120-second builds, 104 to
25,223, is the boot-lottery distribution the series wanted. Milestone 249 (the boot lottery is
sampled by a person walking to the board) had never sampled it: a 243x range on identical silicon
and build, zero defects throughout.

E13 through E25 are one capture of thirteen 120-second draws, `22c19e1d5`, one power-on and every
later boot started by the direct AXP15060 write (13 of 13 resets clean). E16 and E22 are excluded
from the curve: their crossings stayed at 19 and 23 from the first beat to the last, so those soaks
never crossed cores. The other eleven add 218,216 crossings, zero defects. Their start times are
the U-Boot countdowns in `bench/radon-2026-10-20/soak-driver.log`. The account is
`bench/radon-2026-10-20/README-225.md`.

The start times are Pacific local times from `notes/soak.md` converted to UTC; the note does not
state its zone. The conversion is inferred from the commits that recorded them (2026-09-03 20:45 UTC
and 2026-09-04 00:11 UTC), which only a Pacific reading fits. None of the three logs is
in the tree and none records its build, which is why their columns are thin. So radon's curve
starts at roughly three and a half hours and 5,500-plus crossings with zero defects, which is a
confidence about one architecture and one workload, not a verdict about anything.

E4 is the first row with a log in the tree and a build recorded, and on the crossings axis it dwarfs
the three before it. It has 4.1 million against about 8,500 recorded in total, from one boot of 8.16
hours that drew the fast arrangement. radon's running total is in the curve note.

E87 to E138 are milestone 249 (the boot lottery is sampled by a person walking to the board)'s
series of 2026-10-22: 52 draws from one build, each reset by the draw before it, with the JH7110
watchdog armed on every draw. Their start times are spread evenly between the capture's first and
last draws, so each is good to about a minute. They add 685,080 crossings over 40 crossing draws.
Twelve are "not crossing": the six that drew zero clean cores and six that drew one, all frozen
at 17 to 28 crossings. E138 was disarmed by hand at t=65 s and left soaking, so its row is cut at
two beats; the soak it became is a later row.
