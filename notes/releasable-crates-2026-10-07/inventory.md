# Inventory of the workspace's crates, 2026-10-07

*Name: ratified 2026-10-18 (calef, convention-rulings Rule 3).*

Appendix to [the note](../releasable-crates-2026-10-07.md). Generated at base `cd2f93de` by
`python3 notes/releasable-crates-2026-10-07/inventory.py` from the repository root, which reads
`cargo metadata` for the graph. Code lines exclude blank and `//` lines under `src/`. "Kani" counts
`#[kani::proof` attributes. "Falsified" counts `falsifications/*.patch`. "nife crates in its graph"
is normal and build dependencies that are workspace members. The class column is this lane's
judgment, held in the script: A standalone today, B standalone after the cut named, C nife-only.
Every crate is MIT OR Apache-2.0 through `license.workspace`, and every library builds `no_std`
(directly or through `cfg_attr(not(test), no_std)`).

| crate | class | code lines | Kani | falsified | nife crates in its graph | cut |
|---|---|---|---|---|---|---|
| abi | C | 358 | 0 | 0 | none |  |
| activation_set | C | 909 | 0 | 0 | measured_boot |  |
| address_space_identifier | A | 107 | 3 | 3 | none |  |
| address_space_map | C | 281 | 0 | 0 | none |  |
| argument_protocol | C | 221 | 0 | 0 | none |  |
| bitmap_font | A | 397 | 0 | 0 | none |  |
| block_roster | C | 198 | 0 | 0 | none |  |
| board_console | C | 3765 | 0 | 0 | bitmap_font, boot_ladder, job_mix, screen_console |  |
| boot_ladder | C | 9 | 0 | 0 | none |  |
| boot_slot | C | 185 | 0 | 0 | globally_unique_identifier_partition_table |  |
| byte_sink_protocol | C | 219 | 0 | 0 | none |  |
| c_seam | C | 156 | 0 | 0 | address_space_map |  |
| calendar | A | 1091 | 11 | 11 | none |  |
| capability | C | 764 | 14 | 14 | none |  |
| capability_witness_protocol | C | 14 | 0 | 0 | none |  |
| clock_protocol | C | 446 | 0 | 0 | none |  |
| component_plan | C | 1248 | 5 | 5 | abi |  |
| compositor | C | 865 | 0 | 0 | graphics_protocol |  |
| confined_fuzz_protocol | C | 28 | 0 | 0 | abi, address_space_map |  |
| coremark | A | 160 | 0 | 0 | none |  |
| counter_frequency_protocol | C | 96 | 0 | 0 | none |  |
| cpu_set | A | 90 | 0 | 0 | none |  |
| credential_protocol | C | 322 | 3 | 3 | none |  |
| credentialer | C | 716 | 0 | 0 | none |  |
| current_cpu_protocol | C | 123 | 0 | 0 | address_space_map |  |
| designware_ethernet | B | 2513 | 2 | 2 | address_space_map, device_tree_blob | inline `pair_page`; take the register region from the caller, not the tree |
| designware_mobile_storage | B | 2645 | 4 | 4 | address_space_map, device_tree_blob, filesystem_protocol | inline `pair_page` and `BLOCK_SIZE`; take the region from the caller |
| device_tree_blob | A | 763 | 4 | 4 | none |  |
| direct_memory_access_validator | C | 678 | 7 | 7 | none |  |
| documentation | C | 1846 | 0 | 0 | none |  |
| domain_name_system | A | 578 | 3 | 3 | none |  |
| e1000e | B | 2151 | 4 | 4 | address_space_map | inline `address_space_map::pair_page`, its one use |
| elf | A | 1198 | 8 | 8 | none |  |
| entropy_protocol | C | 198 | 0 | 0 | none |  |
| environment_protocol | C | 294 | 0 | 0 | none |  |
| extensible_host_controller_interface | A | 1191 | 3 | 3 | none |  |
| file_allocation_table | A | 479 | 0 | 0 | none |  |
| filesystem_protocol | C | 2640 | 3 | 3 | none |  |
| firmware_configuration | C | 213 | 0 | 0 | machine_discovery |  |
| free | C | 195 | 0 | 0 | address_space_map, machine_statistics_protocol |  |
| generational_table | A | 297 | 4 | 4 | none |  |
| glob | A | 650 | 6 | 6 | none |  |
| globally_unique_identifier_partition_table | A | 1195 | 8 | 8 | universally_unique_identifier |  |
| grant_plan | C | 6689 | 0 | 1 | argument_protocol, glob |  |
| graphics_protocol | C | 222 | 0 | 0 | none |  |
| http_response | A | 320 | 0 | 0 | none |  |
| inter_process_communication | C | 1057 | 14 | 13 | intrusive_fifo |  |
| intrusive_fifo | A | 178 | 1 | 1 | none |  |
| jh7110_clock_and_reset | B | 863 | 0 | 0 | device_tree_blob | take the register regions from the caller, not the tree |
| jh7110_entropy | B | 516 | 4 | 4 | device_tree_blob | take the register region from the caller, not the tree |
| job_mix | C | 259 | 0 | 0 | none |  |
| line_editor | B | 1533 | 0 | 0 | component_plan | move the `component_plan` wiring into the program that serves it |
| loaded_image_check | C | 45 | 0 | 0 | none |  |
| login_protocol | C | 328 | 0 | 0 | credential_protocol, filesystem_protocol |  |
| machine_discovery | C | 4646 | 16 | 16 | device_tree_blob |  |
| machine_statistics_protocol | C | 199 | 0 | 0 | address_space_map, current_cpu_protocol |  |
| manifest_note | C | 638 | 2 | 2 | grant_plan |  |
| measured_boot | C | 440 | 0 | 0 | elf |  |
| memory_corruption_canary_gate | C | 321 | 0 | 0 | none |  |
| memory_regions | C | 1028 | 4 | 4 | generational_table |  |
| name_resolution_protocol | C | 474 | 0 | 0 | domain_name_system |  |
| network_time_protocol | A | 684 | 7 | 7 | none |  |
| nifefs | C | 354 | 2 | 2 | none |  |
| non_volatile_memory_express | A | 733 | 8 | 8 | none |  |
| package_archive | C | 677 | 2 | 2 | measured_boot |  |
| page_frames | A | 247 | 5 | 5 | none |  |
| paging | A | 2464 | 37 | 41 | none |  |
| pci | A | 1109 | 8 | 8 | none |  |
| pgrep | C | 349 | 0 | 0 | abi, glob, ps |  |
| pmap | C | 302 | 0 | 0 | abi |  |
| portable_executable | A | 778 | 0 | 0 | none |  |
| ps | C | 479 | 0 | 0 | abi |  |
| schedule_store | C | 212 | 0 | 0 | none |  |
| screen_console | C | 1316 | 0 | 0 | bitmap_font, machine_discovery |  |
| sealed_pair | C | 437 | 0 | 0 | measured_boot, nifefs |  |
| slabtop | C | 157 | 0 | 0 | address_space_map |  |
| soak_page | C | 84 | 0 | 0 | address_space_map |  |
| socket_protocol | C | 285 | 0 | 0 | none |  |
| std_runtime_protocol | C | 72 | 0 | 0 | none |  |
| stick_maker | C | 2602 | 0 | 0 | measured_boot |  |
| subtree_scope | C | 466 | 5 | 5 | filesystem_protocol |  |
| supervision_protocol | C | 343 | 0 | 0 | abi, address_space_map, counter_frequency_protocol, elf, user_mode_runtime |  |
| swap_protocol | C | 487 | 0 | 0 | abi, address_space_map, component_plan, user_mode_runtime |  |
| swish | C | 3429 | 0 | 0 | activation_set, documentation, environment_protocol, filesystem_protocol, glob, grant_plan, std_runtime_protocol |  |
| system_initializer | C | 3169 | 0 | 0 | abi, activation_set, address_space_map, byte_sink_protocol, credential_protocol, elf, entropy_protocol, filesystem_protocol, grant_plan, graphics_protocol, http_response, line_editor, login_protocol, machine_statistics_protocol, manifest_note, measured_boot, nifefs, package_archive, socket_protocol, std_runtime_protocol, supervision_protocol, user_mode_runtime, video_terminal |  |
| system_log | C | 1214 | 0 | 0 | byte_sink_protocol, system_log_protocol |  |
| system_log_protocol | C | 655 | 0 | 0 | none |  |
| thread_wake_handshake | C | 641 | 0 | 0 | none |  |
| timetable | C | 2894 | 10 | 10 | calendar, grant_plan |  |
| top | C | 129 | 0 | 0 | abi, machine_statistics_protocol, ps, uptime |  |
| universally_unique_identifier | A | 213 | 2 | 2 | none |  |
| uptime | C | 118 | 0 | 0 | none |  |
| usb | A | 583 | 3 | 3 | none |  |
| user_mode_heap | C | 134 | 0 | 0 | none |  |
| user_mode_runtime | C | 794 | 0 | 0 | abi, address_space_map, counter_frequency_protocol, current_cpu_protocol, entropy_protocol, user_mode_heap |  |
| video_terminal | B | 2608 | 0 | 0 | bitmap_font | `bitmap_font` released with it |
| virtio | C | 715 | 0 | 0 | abi, address_space_map, filesystem_protocol, nifefs, user_mode_runtime |  |
| vmstat | C | 112 | 0 | 0 | machine_statistics_protocol |  |
| walk_pricing | C | 375 | 0 | 0 | filesystem_protocol |  |
| work_steal_slot | C | 356 | 0 | 0 | none |  |
