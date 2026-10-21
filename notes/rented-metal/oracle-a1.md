# Booting nife on an Oracle Always Free Ampere A1 instance

The runbook for milestone 578 (an ACPI discovery path for aarch64)'s item 7. It puts the aarch64
boot file on a disk Oracle Cloud Infrastructure (OCI) will import and boots it on a
`VM.Standard.A1.Flex` instance. The serial console then shows what the firmware handed the loader. It is an appendix
to [`notes/rented-metal/aarch64.md`](aarch64.md), whose last section says why the experiment is
worth an evening.

Everything here is generic. Account-specific facts (tenancy, home region, compartment, bucket,
OCIDs, keys) are kept off-tree by the maintainers.

**Not yet run.** The OCI steps are a reading of Oracle's documentation and of two other systems'
published recipes, all read on 2026-10-21 (UTC) and listed under Sources. The disk itself is
booted by `cargo xtask acpi-boot` on every pull request.

## What a person does, and what is already done

| Step | Who | Needs |
|---|---|---|
| Build the disk | anyone, or CI | a checkout, or a CI run's artifact |
| Make an OCI account | a person | a card for identity; see Costs |
| Upload, import, launch | a person | the account |
| Read the console history | a person | the instance |
| Clean up | a person | the account |

## 1. The disk

`cargo xtask acpi-boot` builds the release `BOOTAA64.EFI`, writes it onto
`target/cloud-image/nife-aarch64.img`, and boots that file under EDK2 with `acpi=on` and a GICv3,
which is the description an A1 instance gives. The disk is a GPT with one 300 MiB EFI system
partition (FAT32) holding `\EFI\BOOT\BOOTAA64.EFI`. The writer is `xtask/src/cloud_image.rs`. CI's
`boot-check` job uploads the same file as the `nife-aarch64-cloud-image` artifact, so a person with
no aarch64 build can download it instead.

OCI imports QCOW2 or VMDK, not raw. Convert with `qemu-img` (Homebrew's `qemu` has it):

```sh
cargo xtask acpi-boot                       # about 3 minutes cold; ends "booted ... to the progenitor" twice
qemu-img convert -f raw -O qcow2 target/cloud-image/nife-aarch64.img target/cloud-image/nife-aarch64.qcow2
```

The QCOW2 is uncompressed with QEMU's default 64 KiB clusters, which is what NetBSD's working
recipe uploads. If the import or the boot fails on QCOW2, the fallback is a stream-optimized VMDK,
which Talos users report as the format that boots on A1 when their QCOW2 did not:

```sh
qemu-img convert -f raw -O vmdk -o subformat=streamOptimized \
    target/cloud-image/nife-aarch64.img target/cloud-image/nife-aarch64.vmdk
```

## 2. The account

Sign up at oracle.com/cloud/free. The home region is chosen at sign-up and cannot be changed, and
Always Free compute exists only there, so pick one near you with A1 capacity (see Risks). A new
account starts as a Free Trial with credits for 30 days, then drops to Always Free unless upgraded.
**Do this experiment inside the trial.** Oracle's Always Free page says an instance from an image
not on its Always Free list needs a paid account or a Free Trial with credits left. A custom image
is not on that list.

## 3. Upload and import

1. Object Storage → Buckets → Create bucket, in the home region, Standard tier. Upload
   `nife-aarch64.qcow2` (about 16 MB; far inside the 20 GB free allowance).
2. Compute → Custom images → Import image. Operating system Linux; Import from an Object
   Storage bucket; pick the bucket and object; Image type QCOW2; Launch mode
   Paravirtualized mode (the only mode Arm shapes support for imported images). Wait for
   Available.
3. On the image, Edit image capabilities: under Firmware, untick BIOS so only UEFI_64
   remains. An aarch64 machine has no BIOS, and both NetBSD's and Talos's recipes needed this.
4. On the image, Edit details → Compatible shapes: clear everything except
   `VM.Standard.A1.Flex`.

Oracle's import page says the image "must be set up for BIOS boot" with an MBR. That sentence is
for x86 shapes; the GPT's protective MBR satisfies the import check, and step 3 is what makes the
A1 firmware boot it.

## 4. Launch

Create instance from the custom image. Shape `VM.Standard.A1.Flex`, 1 OCPU and 6 GB,
which is half the Always Free allowance, so a second try can run beside it. Leave the boot
volume at its default (50 GB, inside the 200 GB free Block Volume allowance). Networking: accept
the default VCN and subnet; nife will not use it. An SSH key is not needed for what follows; OCI
may insist on one, in which case let it generate one and discard it.

## 5. Read the console

About two minutes after Running: Instance → Console history → Capture console history,
then download it. Console history is the serial port's recent output, firmware messages included,
and needs no key. Capture again after a few more minutes if it ends mid-boot.

What the text means:

- Success is `uefi_loader: no device tree; describing this machine from ACPI`, a `nife self-test:`
  line with no `FAILED:` list, `nife: handing the system to the userspace progenitor.` and a `$`
  prompt. Then item 7 is done, and 578 can go BUILT. Under QEMU with the same description the verdict
  is `8 of 13 passed, 5 absent: blockdev deviceirq clock trng entropy`. An A1 is expected to show
  the same absences, and `entropy : NONE ... there is no login` after the handover. The loader
  writes no PCI node (`uefi_loader/src/device_tree_from_acpi.rs`'s BUGS) and an Altra core
  has no seed instruction. Those are known, not findings.
- A `uefi_loader:` refusal to place the kernel with the firmware's memory map printed, or silence
  after `exiting boot services`: most likely RAM does not start at `virt`'s `0x4000_0000`, which is
  milestone 127 (the seL4 machine: a Jetson TX1)'s link-address limit. A1 guests are QEMU/KVM (NetBSD reads `QEMU` and
  `KVM Virtual Machine` from the DMI tables), so `virt`'s map is likely but unproven.
- The firmware's `Shell>` prompt, or no `nife uefi_loader` banner: the firmware did not find the
  file. Recheck step 3, then try the VMDK. If the shell is up, the interactive console (below) can
  start the file by hand: `FS0:` then `\EFI\BOOT\BOOTAA64.EFI`. That is a finding about OCI's
  boot entries, not about nife.
- Nothing at all: the console history has no firmware output either, so the image settings are
  wrong before any nife code ran.

Paste the text into milestone 578's pull request or a lane's notes; it is the evidence.

The interactive console, only if the history is not enough: Instance → Console connection →
Create local connection, with an RSA public key (Oracle refuses other key types). Copy serial
console connection for Linux/Mac gives an `ssh` command that proxies through port 443. One client
at a time; a dropped client holds the console for about five minutes; sessions end after 24 hours.

## 6. Clean up

Terminate the instance and tick Permanently delete the attached boot volume. Then delete the
custom image and the bucket object. Nothing left behind then counts against any allowance.

## Risks and costs

| Risk | What happens | What to do |
|---|---|---|
| A1 capacity | "Out of host capacity" on launch. Oracle's own page says it is temporary and suggests another availability domain or waiting. Users report popular home regions running out for days | Try each availability domain; retry later. Upgrading to Pay As You Go is Oracle's other suggestion, and it makes the account billable |
| Custom image needs the trial | After 30 days an Always Free account may refuse a custom image | Run inside the trial window |
| Custom image storage | Oracle's pricing pages were not read (they refused the fetcher). Custom images are believed to bill as Object Storage, a few cents a month for this size | Delete the image after the run |
| Upgrading to Pay As You Go | A1 use within 2 OCPU and 12 GB stays free; anything above, another shape, or a resource outside the home region bills | Stay at 1 OCPU / 6 GB in the home region; set a budget alert of $1 under Billing → Budgets |
| Idle reclamation | Oracle may reclaim an Always Free instance idle for 7 days | Irrelevant to a two-minute boot; terminate it anyway |
| Trial ends with >20 GB in Object Storage | Every object is deleted | The image is about 16 MB |
| Card on file | Sign-up asks for a card to verify identity, and an upgrade bills it | Do not upgrade unless capacity forces it |

Nothing in sections 1 to 6 should cost money on a Free Trial account if section 6 is done.

## BUGS

- None of sections 2 to 6 has been run. Every OCI step is from documentation and from NetBSD's
  and Talos's published recipes. The first run corrects this file.
- Oracle's documentation does not name Arm shapes on its serial console page, and its import
  page describes x86 requirements (BIOS, MBR). That A1 boots an imported UEFI image rests on the
  NetBSD and Talos reports, which are secondary.
- OCI's paravirtualized devices are measured only under QEMU. The boot volume and network are
  virtio over PCIe on a KVM guest. With a `virtio-blk-pci` disk and a `virtio-net-pci` NIC added
  to the `acpi=on`, GICv3 boot on 2026-10-21, nife reports `pcie : none` and reaches the prompt,
  because the loader describes no host bridge from ACPI. So the devices are ignored, not driven,
  and nothing an A1 instance attaches over PCIe can stop the boot.

## Sources, read 2026-10-21 (UTC)

- Oracle, [Importing Custom Linux-Based Images](https://docs.oracle.com/en-us/iaas/Content/Compute/Tasks/importingcustomimagelinux.htm): VMDK or QCOW2, 1 TB maximum, launch modes, the import fields.
- Oracle, [Bring Your Own Image](https://docs.oracle.com/en-us/iaas/Content/Compute/References/bringyourownimage.htm): "On AMD and Arm-based shapes ... imported images are supported in paravirtualized mode only."
- Oracle, [Editing Image Capabilities](https://docs.oracle.com/iaas/Content/Compute/Tasks/configuringimagecapabilities.htm): `Compute.Firmware` takes `BIOS` or `UEFI_64`.
- Oracle, [Instance Console Connections](https://docs.oracle.com/en-us/iaas/Content/Compute/References/serialconsole.htm): RSA keys, port 443, one client, 24 hours, console history.
- Oracle, [Always Free Resources](https://docs.oracle.com/en-us/iaas/Content/FreeTier/freetier_topic-Always_Free_Resources.htm): A1 allowance, home region, 200 GB Block Volume, 20 GB Object Storage, the Always Free image list, capacity, reclamation.
- NetBSD, [NetBSD Arm on Oracle Cloud](https://blog.netbsd.org/tnf/entry/netbsd_arm_on_oracle_cloud): QCOW2, Paravirtualized, A1 only, BIOS unticked; DMI says QEMU/KVM.
- Sidero Labs, [Talos on Oracle](https://docs.siderolabs.com/talos/v1.14/platform-specific-installations/cloud-platforms/oracle) and [siderolabs/talos#12557](https://github.com/siderolabs/talos/issues/12557): a QCOW2 that dropped to the UEFI shell, a stream-optimized VMDK that booted, BIOS unticked.

*Name: provisional (milestone 578's lane, 2026-10-21 UTC), after the shape it boots on.*
