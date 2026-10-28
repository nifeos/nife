# The error paths no test reaches: the tables

Appendix to [untested-error-paths.md](../untested-error-paths.md), generated 2026-10-04 (UTC) by
`helpers/error_paths.py` from the coverage report of merge-queue run 37167533332 at base
`1a145fcaa`. Regenerate rather than edit: the EXAMPLES section of the note has the commands.
The `crates/system_initializer` rows link to that base as GitHub permalinks, which cannot drift.
Milestone 843 (the system-initializer crate root is split along its seams) moved the file they
cite, and §277 (cite a heading or a symbol, never a line number) exempts a pinned link.

## Measured: every host crate with an error path

Result paths are the `?`, `Err`, `.ok_or` and refusal-code family; Option paths are `?` in an
Option function, `return None` and `let Some(..) else`. Unmapped means llvm-cov gave the line no
region, so neither verdict is known.

| crate | Result paths | not reached | Option paths | not reached | unmapped |
|---|---:|---:|---:|---:|---:|
| TOTAL | 1150 | 586 | 437 | 156 | 15 |
| device_tree_blob | 114 | 102 | 1 | 0 | 4 |
| uefi_loader | 108 | 94 | 33 | 25 | 0 |
| stick_maker | 88 | 61 | 14 | 4 | 5 |
| portable_executable | 58 | 46 | 0 | 0 | 0 |
| activation_set | 50 | 33 | 5 | 0 | 0 |
| machine_discovery | 53 | 31 | 58 | 20 | 0 |
| timetable | 67 | 30 | 34 | 13 | 1 |
| walk_pricing | 27 | 23 | 0 | 0 | 0 |
| elf | 50 | 20 | 2 | 1 | 0 |
| grant_plan | 93 | 17 | 22 | 5 | 0 |
| swish | 35 | 17 | 7 | 0 | 0 |
| board_console | 31 | 16 | 32 | 12 | 1 |
| calendar | 39 | 15 | 0 | 0 | 0 |
| globally_unique_identifier_partition_table | 45 | 14 | 12 | 6 | 1 |
| paging | 24 | 12 | 6 | 3 | 0 |
| jh7110_clock_and_reset | 9 | 9 | 4 | 0 | 0 |
| package_archive | 27 | 7 | 3 | 1 | 1 |
| file_allocation_table | 10 | 6 | 0 | 0 | 0 |
| http_response | 20 | 5 | 5 | 1 | 0 |
| credentialer | 18 | 5 | 3 | 0 | 0 |
| jh7110_entropy | 4 | 4 | 0 | 0 | 0 |
| manifest_note | 35 | 3 | 0 | 0 | 0 |
| subtree_scope | 17 | 3 | 0 | 0 | 0 |
| nifefs | 10 | 2 | 1 | 0 | 0 |
| documentation | 5 | 2 | 12 | 5 | 0 |
| line_editor | 4 | 2 | 16 | 13 | 0 |
| memory_corruption_canary_gate | 2 | 2 | 0 | 0 | 0 |
| filesystem_contract | 33 | 1 | 23 | 5 | 1 |
| capability | 10 | 1 | 3 | 2 | 1 |
| free | 5 | 1 | 0 | 0 | 0 |
| sealed_pair | 4 | 1 | 2 | 0 | 0 |
| system_log | 4 | 1 | 4 | 1 | 0 |
| network_time_protocol | 13 | 0 | 1 | 0 | 0 |
| clock_contract | 7 | 0 | 0 | 0 | 0 |
| component_plan | 7 | 0 | 1 | 0 | 0 |
| measured_boot | 7 | 0 | 4 | 1 | 0 |
| environment_contract | 3 | 0 | 2 | 0 | 0 |
| address_space_map | 2 | 0 | 0 | 0 | 0 |
| memory_regions | 2 | 0 | 14 | 1 | 0 |
| schedule_store | 2 | 0 | 3 | 0 | 0 |
| abi | 1 | 0 | 3 | 0 | 0 |
| program_argument_contract | 1 | 0 | 10 | 5 | 0 |
| block_roster | 1 | 0 | 8 | 4 | 0 |
| firmware_configuration | 1 | 0 | 1 | 0 | 0 |
| inter_process_communication | 1 | 0 | 0 | 0 | 0 |
| pmap | 1 | 0 | 0 | 0 | 0 |
| ps | 1 | 0 | 1 | 0 | 0 |
| thread_wake_handshake | 1 | 0 | 0 | 0 | 0 |
| boot_slot | 0 | 0 | 8 | 4 | 0 |
| compositor | 0 | 0 | 2 | 0 | 0 |
| counter_frequency_contract | 0 | 0 | 1 | 0 | 0 |
| cpu_set | 0 | 0 | 2 | 0 | 0 |
| credential_contract | 0 | 0 | 7 | 2 | 0 |
| current_cpu_contract | 0 | 0 | 1 | 0 | 0 |
| generational_table | 0 | 0 | 8 | 2 | 0 |
| glob | 0 | 0 | 4 | 0 | 0 |
| intrusive_queue | 0 | 0 | 1 | 0 | 0 |
| job_mix | 0 | 0 | 1 | 0 | 0 |
| login_contract | 0 | 0 | 4 | 1 | 0 |
| machine_statistics_contract | 0 | 0 | 1 | 0 | 0 |
| non_volatile_memory_express | 0 | 0 | 12 | 5 | 0 |
| page_frames | 0 | 0 | 4 | 1 | 0 |
| pci | 0 | 0 | 3 | 1 | 0 |
| screen_console | 0 | 0 | 9 | 4 | 0 |
| system_log_contract | 0 | 0 | 1 | 0 | 0 |
| top | 0 | 0 | 1 | 0 | 0 |
| video_terminal | 0 | 0 | 10 | 3 | 0 |
| virtio | 0 | 0 | 5 | 5 | 0 |
| vmstat | 0 | 0 | 1 | 0 | 0 |
| work_steal_slot | 0 | 0 | 1 | 0 | 0 |

| kind | paths | not reached |
|---|---:|---:|
| question | 636 | 490 |
| err_return | 402 | 69 |
| err_value | 10 | 1 |
| err_arm | 52 | 23 |
| refusal | 45 | 1 |
| status_arm | 5 | 2 |
| question_option | 193 | 131 |
| option_else | 89 | 0 |
| none_return | 155 | 25 |

## Unmeasured: the kernel, the services and the boot-time crates

| code | Result paths | Option paths |
|---|---:|---:|
| TOTAL | 1159 | 602 |
| components | 380 | 170 |
| kernel | 321 | 305 |
| redoxfs_server | 237 | 16 |
| kernel (arch) | 86 | 68 |
| system_initializer | 84 | 37 |
| supervision_contract | 24 | 0 |
| cryptography_provider | 20 | 0 |
| hot_swap_contract | 5 | 2 |
| user_mode_runtime | 2 | 4 |

| kind | paths |
|---|---:|
| question | 405 |
| err_return | 300 |
| err_value | 10 |
| err_arm | 287 |
| refusal | 138 |
| status_arm | 19 |
| question_option | 231 |
| option_else | 265 |
| none_return | 106 |

## The 75 cleanup paths, all unmeasured

An error path whose body calls something that releases (free, delete, revoke, unmap, discard,
destroy, reclaim). None is in a host crate, so no coverage run has a verdict on any of them.

| where | kind | function |
|---|---|---|
| `kernel/src/sched.rs:421` | none_return | `insert_at_in_place` |
| `kernel/src/sched.rs:5171` | err_return | `configure_thread_control_block` |
| `kernel/src/sched.rs:5176` | err_return | `configure_thread_control_block` |
| `kernel/src/self_test.rs:404` | option_else | `frames` |
| `kernel/src/syscall.rs:706` | err_return | `memory_region_map` |
| `kernel/src/syscall.rs:844` | err_arm | `memory_region_split` |
| `kernel/src/syscall.rs:984` | err_arm | `address_space_map_into` |
| `kernel/src/syscall.rs:1071` | err_return | `page_frame_map` |
| `kernel/src/syscall.rs:1076` | err_arm | `page_frame_map` |
| `kernel/src/user.rs:194` | none_return | `new` |
| `kernel/src/user.rs:199` | option_else | `new` |
| `kernel/src/user.rs:387` | err_return | `map_physical` |
| `kernel/src/user.rs:467` | option_else | `user_address_space_create` |
| `components/src/login.rs:1177` | refusal | `serve_login` |
| `components/src/login.rs:1327` | refusal | `serve_login` |
| `components/src/login.rs:1500` | option_else | `` |
| `components/src/login.rs:1723` | option_else | `open_schedule` |
| `components/src/login.rs:1743` | none_return | `open_schedule` |
| `components/src/login.rs:1783` | none_return | `open_schedule` |
| `components/src/login.rs:1825` | err_arm | `store_caretaker` |
| `components/src/login.rs:1854` | none_return | `store_caretaker` |
| `components/src/login.rs:1911` | err_arm | `connect` |
| `components/src/login.rs:1915` | err_arm | `connect` |
| `components/src/login.rs:1920` | err_arm | `connect` |
| `components/src/login.rs:1938` | none_return | `connect` |
| `components/src/login.rs:1989` | err_arm | `mint` |
| `components/src/login.rs:1993` | err_arm | `mint` |
| `components/src/login.rs:2021` | err_arm | `mint` |
| `components/src/login.rs:2042` | none_return | `mint` |
| `components/src/login.rs:2071` | none_return | `mint` |
| `components/src/net_stack.rs:642` | err_arm | `arm_listener` |
| `components/src/net_stack.rs:764` | refusal | `` |
| `components/src/net_stack.rs:812` | err_arm | `` |
| `components/src/rm.rs:257` | err_return | `empty` |
| `components/src/swish.rs:710` | err_return | `descend` |
| `components/src/swish.rs:2297` | err_arm | `assemble_argv` |
| `components/src/swish.rs:2318` | err_arm | `fresh_page` |
| `components/src/swish.rs:2410` | err_return | `words_grant` |
| `components/src/swish.rs:2581` | err_arm | `run_image` |
| `components/src/swish.rs:2597` | option_else | `run_image` |
| `components/src/swish.rs:3135` | err_arm | `spawn` |
| `components/src/swish.rs:3149` | err_arm | `spawn` |
| `components/src/swish.rs:4125` | err_arm | `run_pipeline` |
| `components/src/swish.rs:4854` | option_else | `spawn_interruptible` |
| `components/src/timetable.rs:975` | err_arm | `fire_with_grant` |
| `components/src/timetable.rs:999` | none_return | `fire_with_grant` |
| `components/src/timetable.rs:1003` | none_return | `fire_with_grant` |
| `crates/user_mode_runtime/src/lib.rs:560` | none_return | `retype_sleeper` |
| [`crates/system_initializer/src/lib.rs` line 4025](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4025) | none_return | `build_caretaker` |
| [`crates/system_initializer/src/lib.rs` line 4352](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4352) | err_arm | `build_graphical_terminal_session` |
| [`crates/system_initializer/src/lib.rs` line 4392](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4392) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4394](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4394) | err_arm | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4416](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4416) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4426](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4426) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4431](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4431) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4438](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4438) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4442](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4442) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4446](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4446) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4448](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4448) | err_arm | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4478](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4478) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4485](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4485) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4490](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4490) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4496](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4496) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4510](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4510) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4514](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4514) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4518](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4518) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4539](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4539) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4571](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4571) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4578](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4578) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4583](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4583) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4628](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4628) | err_return | `graphical_terminal_session_children` |
| [`crates/system_initializer/src/lib.rs` line 4712](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L4712) | err_arm | `copy_args` |
| [`crates/system_initializer/src/lib.rs` line 5047](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L5047) | option_else | `bound_channel` |
| [`crates/system_initializer/src/lib.rs` line 5074](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L5074) | option_else | `job_channel` |
| [`crates/system_initializer/src/lib.rs` line 5836](https://github.com/nifeos/nife/blob/1a145fcaa920346bd03bc4ef1d7e3ebb675dd2a0/crates/system_initializer/src/lib.rs#L5836) | err_return | `stage_pages` |
