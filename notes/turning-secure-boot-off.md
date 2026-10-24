# Turning Secure Boot off, before booting the nife stick on a PC

*Name: provisional, minted 2026-10-18 (UTC) by maintainer/500-secure-boot-ruled.*

This is the first instruction of the trivial-install page (milestone 802 (the trivial install that
makes a second customer possible), rung 4), written so the page can carry it as it stands. Milestone
500 (a stick that boots with Secure Boot on, or a page that says how to turn it off) holds the
ruling behind it: nife does not sign its boot file, so a stranger turns Secure Boot off. Everything
a vendor's menu is said to contain below cites the vendor page it was read from, and the
[sources](#sources) list gives the date each was read. What was not confirmed is marked where a
reader meets it.

Why this step exists. Secure Boot is a firmware setting that lets a PC start only boot files
signed by a key the firmware trusts. Most PCs sold with Windows ship with it on (HP states this for
every HP computer made with Windows 11 [hp]). nife's boot file, `\EFI\BOOT\BOOTX64.EFI` on the
stick, is not signed, so with Secure Boot on the PC refuses it, often with a message about a
security violation (`notes/x86-uefi-boot.md`, as seen on a Dell).

## 1. Before you change anything, get your BitLocker recovery key

**If Windows on this PC uses BitLocker or Device Encryption, changing a firmware setting can make
Windows ask for a 48-digit recovery key the next time it starts. Without that key, Windows and
every file on its disk stay locked. Microsoft cannot give it to you.** In Microsoft's own words, "Microsoft Support doesn't have the ability to retrieve, provide, or recreate a lost
BitLocker recovery key" [ms-key].

What Microsoft documents, and why it applies here:

- Device Encryption is turned on when you first sign in or set up a PC with a Microsoft account, or
  a work or school account. It is not turned on automatically for a local account [ms-de]. So a
  PC can be encrypted without its owner having chosen it.
- BitLocker's default unlock is "TPM-only". In that mode, "if BitLocker detects changes to the BIOS
  or UEFI configuration ... BitLocker enters recovery mode", and BitLocker protects Secure Boot's
  integrity through the TPM's `PCR[7]` measurement [ms-cm]. Turning Secure Boot off is such a change.
  ASUS states the consequence plainly: "modifying BIOS settings may require you to enter the
  BitLocker key to unlock the drive" [asus-nb].

What to do, in Windows, before restarting:

1. Open Settings → Privacy & security → Device encryption [ms-de]. If the switch is on, or
   your PC shows BitLocker as on, continue. If neither page exists, the disk is likely not
   encrypted, and you can go to step 2.
2. Find the recovery key and write it down somewhere other than this PC. For a personal PC it is
   usually in your Microsoft account at `aka.ms/myrecoverykey`; on a work or school PC it is held
   by that organization, at `aka.ms/aadrecoverykey` [ms-key]. It may also be on a printout or a
   USB stick from when encryption was set up [ms-key]. On a work or school PC, ask the people who
   run it before changing its firmware at all.
3. Optionally, suspend BitLocker protection first. Microsoft documents suspending for planned
   firmware changes: the drive stays encrypted, and protection resumes on its own at the next
   restart [ms-ro]. *Not confirmed*: whether Windows Home shows a suspend control for Device
   Encryption was not read. Having the key written down is the step that matters.

If Windows does ask for the key after you return to it, type it in. Windows then starts normally.

## 2. Open the firmware settings

Either way works:

- From Windows 11: Settings → System → Recovery → Advanced startup → Restart now, then
  Troubleshoot → Advanced options → UEFI Firmware Settings → Restart [hp]. Microsoft's older
  instructions reach the same menu by holding Shift while choosing Restart [ms-sb].
- With a key at power-on, from the table in section 3. Press it repeatedly from the moment you
  press the power button until the setup screen appears.

## 3. Turn Secure Boot off

Find your PC's maker below. "Read" means the vendor's page was read in full; "excerpt" means only
a search engine's excerpt of the vendor's page was read, because the page itself could not be
fetched. A row marked "Read" says what the page says, and a step the page does not give is marked
*not confirmed* in the row.

| Maker | Setup key | Where the setting is, and what to choose | Boot menu key | Source |
|---|---|---|---|---|
| Dell | F2 at the Dell logo | Secure Boot → Secure Boot Enable: clear it, or set Secure Boot to Disabled | F12 | Read [dell-sb], [dell-f12] |
| HP consumer laptop | Esc, then F10 | System Configuration → Boot Options → Secure Boot → Disabled, then F10 to save | Esc, then F9 | Read [hp] |
| HP business laptop or workstation | Esc, then F10 | Security → Secure Boot Configuration: clear Secure Boot, then Main → Save Changes and Exit | Esc | Read [hp] |
| HP desktop | F10 | Security → Secure Boot Configuration, press F10 at the message, set Secure Boot to Disable | Esc | Read [hp] |
| Lenovo ThinkPad, ThinkCentre, ThinkStation | F1 at the logo | Security → Secure Boot → Secure Boot: Disabled, then F10 | not read | Read [lenovo-think] |
| Lenovo IdeaPad and other consumer models | F2 or Fn+F2, pressed repeatedly; or F12, then choose to enter setup from the Boot menu; or the Novo button, then BIOS Setup | *Not confirmed*: Lenovo's page says how to enter setup and nothing about Secure Boot, and no Lenovo-owned IdeaPad page naming the setting was found | F12 | Read [lenovo-idea] |
| ASUS laptop | hold F2, then press power | F7 for Advanced Mode, then Security → Secure Boot → Secure Boot Control: Disabled, then F10 | not read | Read [asus-nb] |
| Acer laptop | F2 at the logo | Security: set a supervisor password; then Boot → Secure Boot: Disabled, then F10. Acer advises removing the supervisor password afterward. *Not confirmed*: the page sits behind a bot check and could not be read | not read | Excerpt [acer] |
| Microsoft Surface | Hold volume-up, press and release power, keep holding until the setup screen appears | Security → Secure Boot → Change Configuration. Microsoft says Secure Boot can be disabled there [ms-surface-mgmt], but its consumer pages show only the keyset "Microsoft only", the *on* setting [ms-surface-red], so the name of the *off* choice is *not confirmed* | not read | Read [ms-surface-red], [ms-surface-uefi], [ms-surface-cert], [ms-surface-mgmt] |
| MSI laptop | Delete at the MSI logo | Security → Secure Boot → Disable, then F10 | F11 at the MSI logo | Read [msi-nb] |
| MSI desktop motherboard | not read | Security → Secure Boot (or Settings → Security → Secure Boot). *Not confirmed*: MSI's pages give no steps for turning it off | not read | Read [msi-statement], [msi-z370] |
| Gigabyte laptop | F2 | Security → the default Secure Boot setting: Disabled; Save & Exit → Save Changes → Yes; then Security → Delete All Secure Boot Variables → Yes, then OK to restart | not read | Read [gigabyte-nb] |
| Gigabyte desktop motherboard | Delete | Advanced Mode → Boot → Secure Boot. *Not confirmed*: Gigabyte's page gives steps for turning it on only | not read | Read [gigabyte-mb] |
| Anything else | often F1, F2, F12 or Esc | "usually in either the Security tab, the Boot tab, or the Authentication tab" | varies | Read [ms-sb] |

Notes on the rows added from the vendor pages:

- **Surface.** Microsoft says that changing Secure Boot's certificate keyset on a Surface "will
  trigger a BitLocker recovery scenario" [ms-surface-cert]. That is the one Microsoft sentence read
  that ties a Secure Boot change to BitLocker recovery, for the keyset change it describes. A
  Surface also shows a red bar and an unlocked padlock at start when Secure Boot is off, which is
  the expected result [ms-surface-red]. A UEFI password, if one was set, is asked for before any of this and
  Microsoft cannot reset it [ms-surface-uefi].
- **Gigabyte laptop.** The last step, *Delete All Secure Boot Variables*, erases the stored keys.
  The page lists it as part of turning Secure Boot off; its effect on turning Secure Boot back on
  is *not confirmed*. The page's related-models list shows two laptops, the P34G and P34G v2, so
  the steps are not confirmed for other Gigabyte laptops.
- **Desktop motherboards (MSI, Gigabyte).** The Gigabyte page says Secure Boot is shown only after
  CSM Support is set to Disabled [gigabyte-mb]. MSI says its boards have shipped since 2023 with
  Secure Boot Enabled and the image execution policy set to "Always Execute", and that it will
  move new BIOS releases to "Deny Execute" [msi-statement]. *Not confirmed*: whether a board set to
  "Always Execute" boots nife's unsigned file with Secure Boot on. If so, the stranger would not
  need this step on such a board, and a later BIOS release can take that away.

Two warnings that hold on every PC:

- **Leave the boot mode on UEFI.** Do not turn on *Legacy Support*, *Legacy Boot* or *CSM*. HP's
  instructions go on to enable Legacy Support after Secure Boot is off [hp], and Microsoft's
  mention a Compatibility Support Module [ms-sb]; both are for older systems. nife's stick boots
  only through UEFI, so enabling either can stop the stick from being offered at all.
- **HP asks for a code.** After saving, an HP PC shows a code, on desktops in an "Operating
  System Boot Mode Change" message. Type it and press Enter. On desktops nothing appears as you
  type, which HP says is expected [hp].

## 4. Check, then boot the stick

To confirm from Windows, run `msinfo32`; Secure Boot State reads Off [dell-sb], [asus-nb].
Then put the stick in, restart, press the boot menu key from the table, and choose the USB stick.

## What turning it off costs

- With Secure Boot off, software that installs itself before Windows starts is no longer blocked.
  HP also says running Windows 11 without Secure Boot "might cause instability and prevent Windows
  from making updates" [hp].
- Microsoft warns that after installing other software, Secure Boot "may be difficult to
  re-activate ... without restoring your PC to the factory state" [ms-sb].
- You can turn it back on once you stop booting nife from the stick, by the same menu. A disk nife
  is installed on needs it off for as long as nife boots from it.

## BUGS

- **No stranger has followed this page, and only one PC has been checked against it**: xenon, a
  Dell OptiPlex 7050, whose menu reads *Secure Boot → Secure Boot Enable* with Disabled and Enabled
  (`notes/xenon-firmware.md`). Every other row is the vendor's own page, not a machine.
- Dell's page describes turning Secure Boot on, not off; the off steps are that page's steps
  reversed. Dell's page names two forms, a checkbox on OptiPlex, Precision, Wyse and XPS and an
  Enabled setting on Alienware, Inspiron and Vostro [dell-sb].
- **Acer is still an excerpt.** On 2026-10-21 (UTC) Acer's knowledge base page returned a bot
  check ("performing security verification") to the browser, and a plain fetch returned HTTP 403.
  The page was not read and no way around the check was tried. A search of `acer.com` found no
  Acer-owned page on the subject, only Acer Community forum posts, which are the same site and are
  user answers; none is cited.
- **No Lenovo-owned page names the IdeaPad Secure Boot setting.** The one cited says how to enter
  setup. A search of `lenovo.com` for the setting returned Lenovo Community forum threads only.
- Two further pages were not opened: ASUS's motherboard page and HP's Esc menu outside its Secure
  Boot page.
- Several vendor pages, such as Gigabyte's motherboard page and MSI's, say how to turn Secure Boot
  on or give only the state it ships in. Those rows carry no off step, and say so.
- That turning Secure Boot off triggers BitLocker recovery is an inference from Microsoft's
  `PCR[7]` and "changes to the BIOS or UEFI configuration" sentences [ms-cm], and ASUS's general
  statement [asus-nb]. The only Microsoft page read that says it of a Secure Boot change is the
  Surface one, and it says it of the certificate keyset change [ms-surface-cert]. None says it of
  turning Secure Boot off in those words.

## Sources

Each was read on 2026-10-18 (UTC), in full unless the table above says excerpt, except the entries
marked 2026-10-21 (UTC) (the Lenovo IdeaPad page and every Surface, MSI and Gigabyte entry).

- [ms-key] Microsoft Support, "Find your BitLocker recovery key",
  `support.microsoft.com/en-us/windows/find-your-bitlocker-recovery-key-6b71ad27-0b89-ea08-f143-056f5ab347d6`.
- [ms-de] Microsoft Support, "Device encryption in Windows",
  `support.microsoft.com/en-us/windows/device-encryption-in-windows-cf7e2b6f-3e70-4882-9532-18633605b7df`.
- [ms-cm] Microsoft Learn, "BitLocker countermeasures",
  `learn.microsoft.com/en-us/windows/security/operating-system-security/data-protection/bitlocker/countermeasures`.
- [ms-ro] Microsoft Learn, "BitLocker recovery overview",
  `learn.microsoft.com/en-us/windows/security/operating-system-security/data-protection/bitlocker/recovery-overview`.
- [ms-sb] Microsoft Learn (archived, Windows 8.1), "Disabling Secure Boot",
  `learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-8.1-and-8/dn481258(v=win.10)`.
- [dell-sb] Dell, "How to Enable Secure Boot on Your Dell Device", KB 000190116,
  `dell.com/support/kbdoc/en-us/000190116`.
- [dell-f12] Dell, "Flashing the BIOS from the F12 One-Time Boot Menu", KB 000128928 (excerpt;
  cited only for the F12 menu's name), `dell.com/support/kbdoc/en-us/000128928`.
- [hp] HP, "HP PCs - Secure Boot (Windows 11)", `support.hp.com/us-en/document/ish_6930187-6931079-16`.
- [lenovo-think] Lenovo, "How to enable Secure Boot on Think branded systems", HT509044,
  `support.lenovo.com/us/en/solutions/ht509044`.
- [lenovo-idea] Lenovo, "Recommended way to enter BIOS - ideapad", HT500216, last modified
  2026-07-27, read in full 2026-10-21 (UTC), `support.lenovo.com/us/en/solutions/ht500216`.
- [asus-nb] ASUS, "How to Enable/Disable Secure Boot", FAQ 1050047,
  `asus.com/us/support/FAQ/1050047/`.
- [acer] Acer Community knowledge base, "Enable or disable Secure Boot on an Acer notebook",
  article 88 (excerpt read 2026-10-18; not readable 2026-10-21, see BUGS),
  `community.acer.com/en/kb/articles/88-enable-or-disable-secure-boot-on-an-acer-notebook`.
- [ms-surface-red] Microsoft Support, "Surface turns on but shows a red bar and an unlocked lock
  icon", read 2026-10-21 (UTC),
  `support.microsoft.com/en-us/surface/startup/surface-turns-on-but-shows-a-red-bar-and-an-unlocked-lock-icon`.
- [ms-surface-uefi] Microsoft Support, "How to use Surface UEFI", read 2026-10-21 (UTC),
  `support.microsoft.com/en-us/surface/drivers-firmware/how-to-use-surface-uefi`.
- [ms-surface-cert] Microsoft Support, "Surface Secure Boot Certificates", read 2026-10-21 (UTC),
  `support.microsoft.com/en-us/surface/drivers-firmware/surface-secure-boot-certificates`.
- [ms-surface-mgmt] Microsoft Learn, "Manage Surface UEFI settings", last updated 2026-07-07, read
  2026-10-21 (UTC) (the Secure Boot paragraph; its figures show menus and were not read),
  `learn.microsoft.com/en-us/surface/manage-surface-uefi-settings`.
- [msi-nb] MSI, "[Troubleshooting] Secure Boot Violation Error at Startup", FAQ 11370, updated
  2026-08-25, applies to all MSI notebooks, read 2026-10-21 (UTC), `msi.com/faq/faq-11370`.
- [msi-statement] MSI, "MSI Statement on Secure Boot", 2023-01-19, read 2026-10-21 (UTC),
  `us.msi.com/blog/statement-on-secure-boot`.
- [msi-z370] MSI, "Is secure boot enabled or disabled by default on this motherboard?", FAQ 3115,
  2018-08-28, Z370 boards only, read 2026-10-21 (UTC), `us.msi.com/faq/3115`.
- [gigabyte-nb] Gigabyte, "How to disable Secure Boot in BIOS?", FAQ 3001, read 2026-10-21 (UTC),
  `gigabyte.com/us/Support/Consumer/FAQ/3001`.
- [gigabyte-mb] Gigabyte, "How to Enable Secure Boot and TPM 2.0 on GIGABYTE AM4 300/400/500series
  & sTRX4 TRX40series Motherboards", FAQ 4395, 2025-10-01, read 2026-10-21 (UTC),
  `gigabyte.com/Support/Consumer/Faq/4395`.
