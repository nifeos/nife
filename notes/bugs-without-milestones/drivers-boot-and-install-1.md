# Drivers, boot and install (part 1 of 2)

Entries from the [survey](README.md) of `BUGS` sections that are real future work with no milestone. Where is the file whose `BUGS` section holds the entry. Size is the reader's guess: S under a day, M a lane, L more than one lane.

| Where | Gap | Proposed milestone | Size |
|---|---|---|---|
|  0525-...:134 and design/roadmap/0554-a-good-upgrade-sticks.md | chooser's table write goes primary-first (window with no parseable copy, no journal) and nothing reads the backup table when the primary fails | Chooser table write is backup-first and readers fall back to the backup | M  |
|  components/src/entropy.rs, crates/entropy_contract/src/lib.rs | nothing checks a byte after bring-up; continuous TRNG health test is only decision 0137 (proposed), no milestone | Continuous entropy health tests (decision 0137) | M  |
|  components/src/jh7110_entropy.rs | POLL_TRIES and LOCKUP_RETRIES unmeasured; MODE.R256 never read back on radon | jh7110_entropy: bench-read R256 and size the poll bounds | S  |
|  components/src/system_installer.rs | disks under ~700 MiB refused but untested; clone keeps source disk/partition GUIDs; no firmware boot entry written | system_installer: small-disk test, re-draw GUIDs on clone | S  |
|  components/src/usb_keyboard_driver.rs, crates/extensible_host_controller_interface/src/lib.rs | root ports only (no hubs), no key repeat/lock keys/LEDs, events dropped during stall recovery; 0242 lists them as limits, 0618 excludes hubs | USB keyboard: hubs, key repeat, lock LEDs | L  |
|  crates/address_space_map/src/lib.rs | too-large image reported as "could not spawn" rather than check_image's reason | Userspace loader reports check_image's reason | S  |
|  crates/board_console/src/progress.rs | field names after the marker head (tasks=, ticks_median=...) are string literals nowhere shared; a kernel rename leaves tests green and parse empty | Share job-mix field names between kernel and parser | S  |
|  crates/board_console/src/screen.rs | nothing checks bitmap_font for duplicate glyphs, so glyph lookup can silently prefer the earlier char | bitmap_font: gate against duplicate glyphs | S  |
|  crates/board_console/src/stop.rs | soak markers are string literals checked by a reader; hoisting into boot_ladder would make them agree by construction | Hoist soak markers into boot_ladder | S  |
|  crates/e1000e/src/pch/mod.rs | flow control neither configured nor advertised (FreeBSD does both); 0494 does not name it | e1000e PCH: flow control | S  |
|  crates/installed_programs/src/lib.rs | no retention for old generations; one page holds only about two dozen programs; atomicity of the `current` write on RedoxFS unchecked | installed_programs: retention, capacity, atomic current | M  |
|  crates/screen_console/src/lib.rs | On xenon the screen shows a grid instead of text; finding and experiment are written up but no milestone owns the fix. | Find and fix the xenon grid on the firmware screen | M  |
|  crates/system_initializer/src/children.rs | A caretaker that dies before answering parks the progenitor; no receive with a deadline is used there (milestone 106 (A wait that ends on either the interrupt or the deadline) gave a timer and notification, 321 is a different corpse problem). | The progenitor's caretaker handshake gets a deadline | M  |
|  crates/system_initializer/src/client_windows.rs (2nd bullet) | A caretaker on a window that was once bound gets EBADF forever; nothing in the file protocol reopens a revoked badge. | A way to reopen a revoked badge, or avoid the fallback | S  |
|  crates/system_initializer/src/layout.rs | The image is held three times while a child is built; hashing during the copy would halve the pool; entry says "a choice made on effort". | Hash the image as the loader fills the child | M  |
|  crates/system_initializer/src/lib.rs (DHCP bullet) | A boot with a NIC waits for DHCP before the prompt with no console to say why. | Say why the prompt is waiting on DHCP | S  |
|  crates/system_initializer/src/lib.rs (capability table bullet) | Exhausting the capability table inside `build_child` halts in silence. 778 (sizing) does not make it loud. | Capability table exhaustion in build_child reports itself | S  |
|  crates/usb/src/lib.rs (repeat and Caps Lock bullets) | No key repeat, no Caps Lock or LEDs. Milestone 242 (PARTIAL) only "Recorded" them as BUGS. | USB keyboard typematic repeat, Caps Lock and LEDs | M  |
|  crates/video_terminal/src/palette.rs (defaults bullet) | Default fg/bg should be points at 12 and 8; said to wait on 142's wider rendition, which is BUILT (rich attributes), so it is unblocked. | Point DEFAULT_FG and DEFAULT_BG at Solarized base0 on base03 | S  |
|  design/roadmap/0205-foreign-program-arguments.md | Progenitor install path stack margin (112 bytes over main) never measured; next change to `spawn_service` can overflow | Measure and gate the progenitor install stack margin | S  |
|  design/roadmap/0205-foreign-program-arguments.md | A pipeline with a `std` head strands its region until reboot (LIFO pool, head reaped first) | Non-LIFO return of job regions | M  |
|  design/roadmap/0205-foreign-program-arguments.md | Installed `rg` (about 2.6 MiB) cannot run by path: image transfer capped at 64 pages (`IMAGE_MAX_PAGES`) | Raise the image transfer ceiling for installed programs | M  |
|  design/roadmap/0223-a-card-checker.md | card checker ignores the boot script and extlinux config, the other two ways a card is wrong (218 changed which a card carries) | The card checker also reads the boot script | S  |
|  design/roadmap/0225-run-the-soak-on-the-boards.md | radon reset once at t=105 s with no reset sequence printed; firmware, power and a silent kernel reset path all open; no watchdog | Find why radon reset without saying so (E27) | M  |
|  design/roadmap/0227-gicv3-driver.md | GICv2 SGI has no barrier before SGIR (recorded in drivers/gic.rs BUGS, left unchanged) | A barrier before the GICv2 SGI write | S  |
|  design/roadmap/0242-usb-host-and-a-keyboard-that-is-not-a-uart.md | keyboard behind a hub, or with one built in, is not found; 618 excludes hubs beyond the root explicitly | USB hub enumeration for the keyboard driver | M  |
|  design/roadmap/0242-usb-host-and-a-keyboard-that-is-not-a-uart.md | no typematic repeat, Caps Lock or LEDs (crates/usb BUGS) | USB keyboard repeat, Caps Lock and LEDs | S  |
|  design/roadmap/0261-el0-nvme-on-xenon.md | confined driver can aim DMA at the admin submission ring and ring admin doorbells (stride 0); §86 (Whether an NVMe driver can leave the kernel, and what capability would let i) option 4 doorbell validator would close it, no milestone | NVMe doorbell validator (§86 option 4) | L  |
|  design/roadmap/0287-bootstrap-installs-a-working-qemu.md | a prefix built from a different .qemu-version is silently ignored; nothing says "stale build" | bootstrap reports a stale QEMU prefix | S  |
|  design/roadmap/0303-x86-64-fs-disk.md | `find_block_device_n(2)` counts to a disk by position and shifts silently when a fixture is missing; the wiring should name the disk | Block lookup by named disk, not ordinal | M  |
|  design/roadmap/0318-nvme-test-on-real-geometry.md | the NVMe test writes blocks 37 and 38 of whatever disk is attached, with no check it is a disk safe to write | NVMe boot test refuses a disk not marked as a test disk | S  |
|  design/roadmap/0525-a-bad-upgrade-cannot-brick-the-machine.md | two disks carrying boot slots is a refusal; needs the device-path parser `place_boot_file` also wants | Chooser identifies its own disk by device path | M  |
|  kernel/src/arch/x86_64/amd_vi.rs | Nothing revokes a device's domain in production and a re-attach leaks the old tables; "same gap every IOMMU driver here has" | IOMMU domains are revoked and their tables freed on re-attach | M  |
|  kernel/src/drivers/gic.rs | GICv2 `send_sgi` has no `dsb ishst` before the `SGIR` write; fix is one line, deferred only to keep icount baselines | The GICv2 reschedule IPI gets its barrier | S  |
|  kernel/src/extensible_host_controller_interface.rs | only the first xHCI on the bus is brought up; a keyboard on a second controller is never found | Bring up every xHCI controller, not the first | S  |
|  kernel/src/self_test.rs (last bullet) | a self-test whose operation never returns (`yield_now`, `map_page`, `spin_for` on a stopped counter) hangs the boot; needs a watchdog | The boot self-test has a watchdog | M  |
|  kernel/src/user/install_service.rs (and notes 0515 BUGS) | install is not crash-atomic: power cut between partitioner and `mkfs` leaves an unmountable disk; never measured | Make the installer survive a power cut, or measure it | M  |
|  notes/amd-vi.md (5th bullet) | production revocation of AMD-Vi mappings is still open after 767 closed four of five gaps; 764 does not plan it | AMD-Vi revocation in the production path | M  |
|  notes/bench-runbook.md (UTF-8 bullet) | `script/board-console` logs contain invalid UTF-8 under sustained output; `tr`/`awk` fail and `grep` silently sees nothing; cause undiagnosed | `board-console` writes logs ordinary tools can read | S  |
|  notes/bench-runbook.md (two-process bullet) | two `board-console` processes on one serial port silently split the stream; no lock | `board-console` refuses a second opener on the same port | S  |
|  notes/board-console.md (OpenSBI bullet) | does not recognize an OpenSBI trap dump | `board-console` recognizes an OpenSBI trap dump | S  |
|  notes/device-tree.md | `memory_regions` matches `/memory` by name, not `device_type`; unbalanced `FDT_END_NODE` is walked, not refused | Make the device-tree walkers strict (device_type, nesting balance) | S  |
|  notes/e1000e.md | Unexplained x86_64 AMD-Vi hang; the UDP round-trip test's wait is unbounded (bounding it turns hang into a failure) | Bound the e1000e UDP test's wait | S  |
|  notes/framebuffer-contract.md | Every `gpu_driver` spawner must receive `FLUSHED` or the driver serves one flush; nothing enforces it | Make an unread `FLUSHED` unrepresentable | M  |
|  notes/jh7110-clock-and-reset.md | vendor `rstgen` node path has never been exercised by a fixture | Fixture for the rstgen-only device tree | S  |
|  notes/non-volatile-memory-express.md | one command per fs block, no PRP lists, one command in flight, one namespace (milestone 55 (Time Machine: SMB3 with Apple's extensions, and mDNS) it names is REMOVED) | NVMe PRP lists and real queue depth | M  |
|  notes/serial-less-output.md (4 GiB bullet) | `uefi_loader::find_screen` never checks the framebuffer base is below 4 GiB; a high aperture would go unnoticed | Loader refuses (or maps) a framebuffer base above 4 GiB | S  |
|  notes/timer-capability.md (EL0 comparator, xenon HPET bullets) | The cheap EL0 `CNTP_CVAL_EL0` experiment never ran; xenon's HPET is unconfirmed | Run the EL0 comparator probe; read xenon's ACPI table list for HPET | S  |
|  notes/two-trees.md | `grant_plan::plan` and its callees take a ~1 KiB `Holdings` by value; passing `&Holdings` is the stated fix | Pass `Holdings` by reference in the grant planner | S  |
|  notes/usb.md | a device refused after boot is refused silently (one message on the report endpoint) | Report a post-boot USB refusal on the console | S  |
|  notes/usb.md | driver is mapped USBLEGSUP/USBLEGCTLSTS; a compromised driver can hand the controller back to SMI. Only a proposal file exists (`proposals/the-xhci-driver-is-not-mapped-the-legacy-support-capability.md`), no milestone | Withhold the xHCI legacy-support capability from the driver | S  |
|  notes/visionfive2.md | `an_interrupt_that_arrives_before_the_wait_is_not_lost` fails on the board; PLIC/hart affinity path suspected | Fix the lost-interrupt test failure on the VisionFive 2 | M  |
|  notes/visionfive2.md | userspace input driver reads the UART at QEMU's byte stride (shell input is garbage on the board); a keystroke reaching source 32 is unobserved (two entries merged) | Make the input driver read the board's UART and observe a keystroke | M  |
|  roadmap 0590 | NIC boot blocks on DHCP before a prompt; no-DHCP boot hangs with no console | The progenitor does not wait for a lease before the prompt | S  |
|  roadmap 0595 | an image is held three times while built (13.5 of 48 MiB for 4 MiB image); hash while copying | The image loader hashes as it copies | M  |
|  roadmap 0600 | a graphical stack that fails to come up traps the progenitor with no sentence | A failed driver report prints a named sentence | S  |
|  roadmap 0628 | input driver polls so an x86_64 core never idles at a prompt | The input driver sleeps on an interrupt | M  |
|  roadmap 0685 | no boot drives seven concurrent directory-granted jobs through the real progenitor | A boot test for seven concurrent directory jobs | S  |
|  roadmap 0764 | without `EFR.IASup` init flushes 65,536 domain ids one command each; cost unmeasured | Measure and bound AMD-Vi domain flush | S  |
|  roadmap 0767 | NVMe, xHCI, e1000e grants stay read-write though some queues are device-read only | Tighten DMA grants to queue direction | M  |
|  roadmap 0773 | prompt does not say the session is lost at power-off | Live session says it is temporary at boot | S  |
|  roadmap 0773 | 256 MiB RAM disk size is a guess | Size the live RAM disk by measurement | S  |
|  roadmap 0773 | non-nife NVMe (Windows) hashes never compared under G2 | Stick boot leaves a foreign NVMe byte-identical | M  |
|  roadmap 0773 | G1 can write a stranger's disk (spending a try rewrites GPT from a "read-only" stick) | The live stick never writes the internal disk | M  |
|  roadmap 0803 | other QEMU assumptions on the aarch64 boot path not swept beyond four files | Sweep the aarch64 boot path for QEMU assumptions | M  |
|  roadmap 0805 | a reset does not quiesce devices; in-flight DMA is cut off | Reboot quiesces devices | M  |
|  roadmap 0811 | whether kernel must reset the I/O queue pair before a server restart is unmeasured | Measure NVMe server restart state | S  |
|  roadmap 0890 | `serial_driver` shipped and started by nothing; `board_console` has not read radon's profile | Start or drop serial_driver; radon console profile | S  |
|  uefi_loader/src/chooser.rs | power cut mid table write, write order, and fallback to the backup table are all untested or unimplemented | Chooser survives a power cut and reads the backup table | M  |
|  uefi_loader/src/chooser.rs (slot image) | an image started from a slot has no boot file so cannot install to a second disk; `booted_from_nvme` assumes NVMe | Let a slot-booted image find its boot file and check its disk | S  |
|  uefi_loader/src/device_tree_from_acpi.rs (PCI) | no PCI or virtio-mmio on an ACPI aarch64 machine (needs `_CRS`/`_PRT`); a present bus is invisible (578 covers discovery, not AML) | Describe PCI on ACPI aarch64 machines | L  |
