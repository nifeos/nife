# xenon's firmware: the full transcription

An appendix to [`notes/xenon-firmware.md`](../xenon-firmware.md), which holds the settings nife
depends on. This is every page of the 70 photographs of 2026-09-04 (UTC), each entry with its `IMG_`
number. Where a setting has changed since, the parent's table is current and this is the record of
that day.

Ordered as the menu tree is, which is also roughly the order of the `IMG_` numbers.

The navigation tree is legible in full only on IMG_4025, because every other frame crops or blanks
it. Its entries, in order:

- General, System Information, Boot Sequence, Advanced Boot Options, UEFI Boot Path Security,
  Date/Time, System Configuration, Video, Security, Secure Boot,
  Intel® Software Guard Extensions™, Performance, Power Management, POST Behavior, Manageability,
  Virtualization Support, Wireless, Maintenance, System Logs, Advanced configurations.

### General

IMG_4025, IMG_4026, System Information (read-only)

| Field | Value |
|---|---|
| Bios version | 1.27.0 |
| Asset Tag | (none) |
| Ownership Tag | (blank) |
| | Signed Firmware Update is enabled |
| Memory Installed | 16384 MB |
| Memory Available | 16286 MB |
| Memory Speed | 2400 MHz |
| Memory Channel Mode | Dual |
| Memory Technology | DDR4 SDRAM |
| DIMM 1 Size / DIMM 2 Size | 8192 MB / 8192 MB |
| Slot1_M.2 | Mass Storage |
| Slot2_M.2 | Network |
| Processor Type | Intel(R) Core(TM) i5-7500T CPU @ 2.70GHz |
| Core Count | 4 |
| Processor ID | 906e9 |
| Current / Minimum / Maximum Clock Speed | 2.600 GHz / 0.800 GHz / 2.700 GHz |
| Processor L2 / L3 Cache | 1024 KB / 6144 KB |
| HT Capable | No |
| 64-Bit Technology | Yes (Intel EM64T) |
| SATA-0 | (none) |
| SATA-4 | (none) |
| M.2 PCIe SSD-0 | 256 GB |
| Video Controller | Intel HD Graphics |
| Audio Controller | RealTek ALC3234 |
| Wi-Fi Device | Intel Wireless |
| Bluetooth Device | Installed |

`HT Capable: No` is worth a second look: the i5-7500T has four cores and no hyperthreading, so the
four logical CPUs the boot tour enumerates are four physical ones.

IMG_4027, Boot Sequence. `Windows Boot Manager` present and unticked. Boot List Option:
Legacy ( ), UEFI (•), and Legacy is grayed. Buttons: Add Boot Option, Delete Boot Option, View.

IMG_4028, Advanced Boot Options. `Enable Legacy Option ROMs` unticked;
`Enable Attempt Legacy Boot` unticked and grayed.

IMG_4029, UEFI Boot Path Security. Always, Except Internal HDD (•); Always ( ); Never ( ).
Has no effect while no Admin password is set, and none is (IMG_4043).

IMG_4030, Date/Time. `09` / `04` / `26`, `07:07:45 PM`. The machine's clock at the moment of
photographing, not a setting.

### System Configuration

IMG_4031, Integrated NIC. `Enable UEFI Network Stack` unticked. Disabled ( );
Enabled (•); Enabled w/PXE ( ). So the LAN was visible to an OS, with no UEFI PXE path. Both
settings were changed at the bench on 2026-10-04, to the stack ticked and `Enabled w/PXE`; the
parent's table records that, and the photographs of it are owed.

IMG_4032, Serial Port. Disabled ( ); COM1 (•); COM2 ( ); COM3 ( ); COM4 ( ). Help text
verbatim:

> This option determines how the built-in serial port operates. It lets you avoid resource
> conflicts between devices by disabling or remapping the address of the port.
>
> COM1 = Port is configured at 3F8h with IRQ 4.
> COM2 = Port is configured at 2F8h with IRQ 3.
> COM3 = Port is configured at 3E8h with IRQ 4.
> COM4 = Port is configured at 2E8h with IRQ 3.
>
> Note: the operating system may allocate resources even though the setting is Disabled.

IMG_4033, SATA Operation. Disabled ( ); AHCI (•); RAID On ( ).

IMG_4034, Drives. `SATA-0` ticked, `SATA-4` ticked, `M.2 PCIe SSD-0` ticked.
Details: SATA-0 Type `(none)`, Device ID `(none)`; SATA-4 Type `(none)`, Device ID `(none)`;
M.2 PCIe SSD-0 Type `256 GB SSD`, Device ID `Micron 2450 NVMe 256GB`.

IMG_4035, SMART Reporting. `Enable SMART Reporting` ticked.

IMG_4036, USB Configuration. `Enable USB Boot Support` ticked;
`Enable Front USB Ports` ticked; `Enable Rear USB Ports` ticked.

IMG_4037, Front USB Configuration. `Front Port 1 w/Power Share(Bottom)*` ticked;
`Front Port 2(Top)*` ticked.

IMG_4038, Rear USB Configuration. `Rear Port 1(Bottom)*`, `Rear Port 2(Lower Middle)*`,
`Rear Port 3(Upper Middle)*`, `Rear Port 4(Top)*` all ticked.

IMG_4039, USB PowerShare. `Enable USB PowerShare` unticked.

IMG_4040, Audio. `Enable Audio` ticked, with `Enable Microphone` ticked and
`Enable Internal Speaker` ticked.

IMG_4041, Dust Filter Maintenance. Disabled (•); 15/30/60/90/120/150/180 days all ( ).

### Video

IMG_4042, Primary Display. Auto (•); Intel HD Graphics ( ).

### Security

IMG_4043, Admin Password. Old password field grayed, reading "Not Set". New and confirm
fields empty.

IMG_4044, System Password. Old password field grayed, reading "Not Set". New and confirm
fields empty. So neither password is set, which is why IMG_4029 and IMG_4055 have no effect.

IMG_4045, Strong Password. `Enable Strong Password` unticked.

IMG_4046, Password Configuration. Admin Password Min `04`, Max `32`; System Password Min `04`,
Max `32`.

IMG_4047, Password Bypass. Disabled (•); Reboot Bypass ( ).

IMG_4048, Password Change. `Allow Non-Admin Password Changes` ticked.

IMG_4049, UEFI Capsule Firmware Updates. `Enable UEFI Capsule Firmware Updates` ticked.

IMG_4050, TPM 2.0 Security. `TPM On` ticked; `PPI Bypass for Enable Commands` ticked;
`PPI Bypass for Disable Commands` unticked; `PPI Bypass for Clear Command` unticked;
`Clear` unticked; `Attestation Enable` ticked; `Key Storage Enable` ticked;
`SHA-256` ticked. Radio group below: Disabled ( ); Enabled (•).

IMG_4051, Computrace(R). Deactivate (•); Disable ( ); Activate ( ). The page states the
Absolute Anti-Theft solution is presently Deactivated, and that Activate and Disable are permanent.

IMG_4052, Chassis Intrusion. `Clear Intrusion Warning` unticked. Disabled ( );
Enabled ( ); On-Silent (•). The page reports that an intrusion has been detected, which the
BIOS event log (IMG_4093) also shows repeatedly.

IMG_4053, CPU XD Support. `Enable CPU XD Support` ticked.

IMG_4054, OROM Keyboard Access. Enabled (•); Disabled ( ); One Time Enable ( ).

IMG_4055, Admin Setup Lockout. `Enable Admin Setup Lockout` unticked.

IMG_4056, Master Password Lockout. `Enable Master Password Lockout` unticked.

IMG_4057, SMM Security Mitigation. `SMM Security Mitigation` unticked.

### Secure Boot

IMG_4058, Secure Boot Enable. Disabled (•); Enabled ( ). Help text verbatim:

> This option enables or disables the Secure Boot feature. For Secure Boot to be enabled, the
> system needs to be in UEFI boot mode and the Enable Legacy Option ROMs option needs to be turned
> off.

IMG_4059, Expert Key Management. `Enable Custom Mode` unticked. Custom Mode Key
Management: PK (•); KEK ( ); db ( ); dbx ( ). `Save to File` enabled; `Replace from File`,
`Append from File`, `Delete`, `Reset All Keys`, `Delete All Keys` all grayed out (Custom Mode is
off).

### Intel® Software Guard Extensions™

IMG_4060, Intel® SGX™ Enable. Disabled ( ); Enabled ( ); Software Controlled (•).

IMG_4061, Enclave Memory Size. 32MB ( ); 64MB ( ); 128MB (•). No effect while SGX is
Software Controlled.

### Performance

IMG_4062, Multi Core Support. All (•); 1 ( ); 2 ( ); 3 ( ).

IMG_4063, Intel® SpeedStep™. `Enable Intel® SpeedStep™` ticked.

IMG_4064, C-States Control. `C states` ticked.

IMG_4065, Limit CPUID Value. `Enable CPUID Limit` unticked.

IMG_4066, Intel® TurboBoost™. `Enable Intel® TurboBoost™` ticked.

SpeedStep, C-states and TurboBoost are all on, which is worth knowing before anybody quotes a
benchmark number off this machine. The clock is free to move between 0.800 GHz and Intel's 3.3 GHz
turbo ceiling for the i5-7500T, and the cores are free to sleep. The 2.700 GHz "Maximum Clock
Speed" above is the nominal clock, not the turbo ceiling.

### Power Management

IMG_4067, AC Recovery. Power Off ( ); Power On ( ); Last Power State (•).

IMG_4068, Auto On Time. Time `12:00 AM`. Disabled (•); Every Day ( ); Weekdays ( );
Select Days ( ). All seven day checkboxes unticked and grayed.

IMG_4069, Deep Sleep Control. Disabled ( ); Enabled in S5 only ( );
Enabled in S4 and S5 (•). The help text warns that with this enabled, Remote Wakeup and Remote
Manageability are disabled while shut down or hibernating.

IMG_4070, USB Wake Support. `Enable USB Wake Support` ticked.

IMG_4071, Wake on LAN/WLAN. Disabled (•); LAN Only ( ); LAN with PXE Boot ( );
WLAN Only ( ); LAN or WLAN ( ).

IMG_4072, Block Sleep. `Block Sleep (S3 State)` unticked.

IMG_4073, Intel Ready Mode. `Enable Intel Ready Mode` unticked.

### POST Behavior

IMG_4074, Adapter Warnings. `Enable Adapter Warnings` unticked.

IMG_4075, Numlock LED. `Enable Numlock LED` ticked.

IMG_4076, Keyboard Errors. `Enable Keyboard Error Detection` ticked.

IMG_4077, Fastboot. Minimal ( ); Thorough (•); Auto ( ). Thorough performs complete
hardware and configuration initialization during boot, which is the slow and safe end of this
setting and the right one for a bring-up.

IMG_4078, Extend BIOS POST Time. 0 seconds (•); 5 seconds ( ); 10 seconds ( ).

IMG_4079, Full Screen Logo. `Enable Full Screen Logo` unticked.

IMG_4080, Warnings and Errors. Prompt on Warnings and Errors (•); Continue on Warnings ( );
Continue on Warnings and Errors ( ). Help text verbatim, because this one has consequences for a
headless bench (see "The hazard a headless xenon will hit" in [the parent](../xenon-firmware.md)):

> The Warnings and Errors options cause the boot process to only pause when warnings or errors are
> detected, rather than stop, prompt and wait for user input. This feature is valuable in
> situations where the system may be remotely managed and therefore has no locally connected
> keyboard or console available for a user to respond; for example, in a headless configuration.
>
> Users may select to have the Power On Self-Test (POST) process either a) stop, prompt and wait
> for user input when warnings or errors are detected, or b) continue when warnings are detected
> but pause on errors, or c) continue when either warnings or errors are detected during the POST
> process.
>
> Note that errors deemed critical to the operation of the system hardware will always halt the
> system.

### Manageability

IMG_4081, USB Provision. `Enable USB Provision` unticked.

IMG_4082, MEBx Hotkey. `Enable MEBx Hotkey` ticked.

### Virtualization Support

IMG_4083, Virtualization. `Enable Intel Virtualization Technology` ticked.

IMG_4084, VT for Direct I/O. `Enable VT for Direct I/O` ticked. Help text verbatim:

> This option specifies whether a Virtual Machine Monitor (VMM) can utilize the additional hardware
> capabilities provided by Intel® Virtualization Technology for Direct I/O.
>
> [NOTE: Trusted Execution requires VT for Direct I/O to be enabled.]

IMG_4085, Trusted Execution. `Trusted Execution` unticked.

### Wireless

IMG_4086, Wireless Device Enable. `WLAN/WiGig` ticked; `Bluetooth®` ticked.

### Maintenance

IMG_4087, Service Tag. Set.

IMG_4088, Asset Tag. Empty.

IMG_4089, SERR Messages. `Enable SERR Messages` ticked.

IMG_4090, BIOS Downgrade. `Allow BIOS Downgrade` ticked.

IMG_4091, Data Wipe. `Wipe on Next Boot` unticked. Leave it that way: the page's own
warning is that the operation is not recoverable and cannot be terminated once started, across
internal SATA HDD/SSD, M.2 SATA SSD, M.2 PCIe SSD and eMMC.

IMG_4092, BIOS Recovery. `BIOS Recovery from Hard Drive` ticked;
`BIOS Auto-Recovery` unticked. Both are the firmware's stated defaults.

### System Logs

IMG_4093, BIOS Events. Twenty entries, oldest at the bottom of the visible list. Verbatim:

```
09/04/2026  22:00:09   Alert! Cover was previously removed.
09/04/2026  22:04:58   Alert! Cover was previously removed.
08/09/2025  19:45:43   Alert! Keyboard not found.
08/09/2025  20:19:41   Alert! Keyboard not found.
08/09/2025  20:28:37   Alert! Keyboard not found.
08/19/2025  13:47:59   Alert! Keyboard not found.
06/20/2026  18:00:39   Alert! Keyboard not found.
07/30/2026  00:42:02   Alert! Keyboard not found.
08/15/2026  10:07:36   Alert! Cover was previously removed.
08/15/2026  10:08:18   Alert! Cover was previously removed.
08/15/2026  10:08:42   Alert! Cover was previously removed.
08/15/2026  10:09:11   Alert! Cover was previously removed.
08/15/2026  10:09:30   Alert! Cover was previously removed.
08/15/2026  10:10:01   Alert! Cover was previously removed.
08/15/2026  10:15:38   Alert! Cover was previously removed.
08/15/2026  10:18:22   Alert! Cover was previously removed.
08/15/2026  10:19:48   Alert! Cover was previously removed.
08/15/2026  10:21:56   Alert! Cover was previously removed.
08/28/2026  06:00:05   Alert! Cover was previously removed.
08/28/2026  06:00:06   Alert! Keyboard not found.
```

The list is not in date order on the screen and is reproduced as it appeared. The two 09/04/2026
entries at the top are the first-light session itself. `Alert! Cover was previously removed` is
IMG_4052's `On-Silent` chassis intrusion setting doing what it says, and it will keep appearing
until somebody ticks `Clear Intrusion Warning`.

### Advanced configurations

IMG_4094, ASPM. Auto ( ); Disabled (•); L1 Only ( ).
