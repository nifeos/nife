# Probing an AWS Graviton metal instance

The runbook for one paid hour on a `c7g.metal` (Graviton3, Neoverse V1, 64 cores), the first ARM
machine under calef's ruling "Rent first." (2026-10-21 UTC, on #2517). The hour answers four
questions with primary evidence, read off the machine rather than off documentation:

1. Does an SMMUv3 sit in front of the PCIe devices, and does the OS see it? Which stream IDs do
   ENA and NVMe use?
2. Is the PMU cycle counter usable?
3. Which exception level does the firmware enter the OS at, and what is the GIC and ITS layout?
4. Does UEFI output reach the EC2 serial console? And what does nife's own boot disk print there?

What is already known, and from where, is in
[the survey's AWS appendix](../aarch64-modern-board-survey/aws-graviton-metal.md). It answers all
four only by inference or from third parties.

Everything here is generic. Account-specific facts (the account number, keys, the region chosen,
the budget's address) are kept off-tree by the maintainers.

**Not yet run.** Every AWS step is a reading of AWS's documentation on 2026-10-21 (UTC), listed
under Sources. The scripts were exercised against a stub `aws` that answers every call, so their
control flow and teardown are tested. Nothing has touched a real account.

## Who does what

| Step | Who | Needs |
|---|---|---|
| Create the AWS account, with MFA on the root user | calef | a card and a phone |
| Run `bootstrap.sh` once in CloudShell, then `aws configure` | calef | the account |
| Wait for the vCPU quota | AWS | hours to days |
| `run preflight`, `run all`, read the transcript | anyone with the profile | the workstation awake for about 90 minutes |
| Retire the access key | anyone with the profile | nothing |

The scripts are in [`aws-graviton/`](aws-graviton/): `bootstrap.sh` prepares the account, `run`
drives the probe, `probe.sh` runs on the instance.

## 1. The account

Sign up at aws.amazon.com and turn on MFA for the root user. Then open CloudShell (the terminal icon
in the console's top bar) in the chosen region. Fetch `bootstrap.sh` at the commit this file merged
in, read it, and run it:

```sh
curl -fsSLO https://raw.githubusercontent.com/nifeos/nife/<commit>/notes/rented-metal/aws-graviton/bootstrap.sh
sh bootstrap.sh <region> <email for budget alerts>
```

CloudShell already holds the root user's credentials, so nothing is typed into it but those two
arguments. The script's last line prints an access key ID and secret for the IAM user `nife-metal`.
On the workstation (`brew install awscli` first):

```sh
aws configure --profile nife-metal      # paste the two values; region as chosen; output json
```

Record the account number, region and budget address in lab.

## 2. Least privilege

`bootstrap.sh` attaches one managed policy, `nife-metal-probe`, to the user. Its rules:

- Reads (`Describe*`, console output, the quota, the budget, public SSM parameters) are allowed
  everywhere, so `run check` can look in every region.
- Every create (instance, volume, network interface, snapshot, image, key pair, security group)
  must carry the tag `nife-probe=aws-graviton`, and only in the chosen region.
- Every destructive or attaching action, and the serial console key push, needs that tag on its
  target. The user cannot touch anything made by hand.
- An explicit Deny holds `RunInstances` to `c7g.metal`. Widening it is an edit to `bootstrap.sh`
  and a new policy version.
- The user may list and delete its own access keys, and nothing else in IAM.

Root does the three account-level steps itself: serial console access, the budget and the quota
request. So the everyday key needs none of those permissions.

## 3. Region

Any region that offers `c7g.metal` and the serial console. Check with:

```sh
aws ec2 describe-instance-type-offerings --location-type availability-zone \
    --filters Name=instance-type,Values=c7g.metal --region <region>
```

A US region is the cheap choice; the price below is us-east-1's. Older regions use the serial
endpoint `serial-console.ec2-instance-connect.<region>.aws`, which `run` assumes. Newer ones use
`ec2-serial-console.<region>.api.aws` (set `SERIAL_HOST`).

## 4. The budget comes first

`bootstrap.sh` creates the budget before it creates the user, so no key exists until the alarm
does. It is a $10 monthly cost budget that emails at 25% actual, 100% actual and 100% forecast.
`run preflight` refuses to continue when the account has no budget.

A budget is a backstop, not a stop. AWS bills hours late, so the alarm can arrive long after an
instance should have died. The stops are in `run`: see section 5.

## 5. The run

Get the boot disk from the newest green `main` CI run, which keeps it 14 days. A local
`cargo xtask acpi-boot` writes the same file to `target/cloud-image/nife-aarch64.img`.

```sh
gh run download -R nifeos/nife -n nife-aarch64-cloud-image -D ~/nife-metal-disk \
    "$(gh run list -R nifeos/nife -w ci.yml -b main -s success -L 1 --json databaseId -q '.[0].databaseId')"
notes/rented-metal/aws-graviton/run preflight
notes/rented-metal/aws-graviton/run all ~/nife-metal-disk/nife-aarch64.img
```

`run preflight` reads, and fails with the reason, until the quota is at least 64 vCPUs, serial
console access is on and a budget exists. `run all` then does this, writing to
`~/nife-metal-runs/<UTC time>/` (outside any worktree, because a merged worktree gets pruned):

| Phase | What happens | Expected minutes |
|---|---|---|
| Linux launch | Ubuntu 24.04 on `c7g.metal`, an ed25519 key, ssh open to this workstation's address only | 20, the metal boot |
| Serial capture | from launch, the whole Linux boot, UEFI included if it reaches the port | (alongside) |
| Linux probe | `probe.sh` over ssh; the raw ACPI tables copied back | 5 |
| Disk | a 1 GiB gp3 volume written with the image, snapshotted, registered as a UEFI arm64 AMI | 5 |
| Linux teardown | terminate, wait | 5 |
| nife boot | a second `c7g.metal` from that AMI, serial console captured for 25 minutes | 25 |
| Teardown and check | everything tagged deleted; every region checked | 5 |

Without an image argument the run stops after the Linux probe, and question 4 is answered for Linux
only.

Three things end a forgotten instance, all on the workstation's side but the first:

- The Linux instance runs `shutdown -h +100` from its user data and launches with shutdown behavior
  `terminate`, so it ends itself after 100 minutes even if the workstation dies.
- An exit trap in `run` tears down and checks on any exit, Ctrl-C included.
- A detached watchdog runs `run teardown` at 180 minutes even if the shell that started it is gone.
  It does not survive the workstation sleeping.

The nife instance has only the last two, since nothing tells nife to power off.

Everything lands in `transcript.txt`: the driver's log, the probe's output, both serial captures
with terminal escapes removed, the console output API's copy, and the serial host key's
fingerprint. Compare that fingerprint with AWS's table for the region (Sources). The account
number is replaced with `<account>` in every file, and the run fails if it survives anywhere.
Commit the transcript and `acpi-tables.tar` under `bench/aws-c7g-metal-<date>/` with a README, as
`bench/patagonia-hvf-2026-10-21/` does.

## 6. The serial console by hand

`run` does this itself. By hand, for a look while a run is going:

```sh
aws ec2-instance-connect send-serial-console-ssh-public-key --instance-id <id> \
    --serial-port 0 --ssh-public-key file://key.pub
ssh -i key <id>.port0@serial-console.ec2-instance-connect.<region>.aws   # within 60 seconds
```

One session per instance, about an hour long, 30 seconds before a new one can start. A second
connection fails while `run` holds one.

## 7. Reading the answers

### Question 1: the SMMU and stream IDs

In `IORT.dsl`, find the PCI root complex node. Its ID mappings send requester IDs to an output
reference.

- If the reference is an SMMUv3 node, an SMMU sits in front of the devices. That node gives its base
  address, its model and its flags, and its own mapping to the ITS group.
- If the root complex maps straight to the ITS group, there is no SMMU in that path. That is a
  finding, not a probe failure.

A device's stream ID is its requester ID (`bus << 8 | device << 3 | function`, from `lspci -D`)
minus the mapping's input base, plus its output base. ENA and the NVMe controllers are vendor
`1d0f`.

The kernel's side confirms it: `arm-smmu-v3` lines in dmesg (with a features mask), one IOMMU group
per device, and each group's `type`. `identity` means passthrough; `DMA` or `DMA-FQ` means
translation is on. The SMMU being in the tables but no device in a group is a third outcome worth
recording as it stands.

### Question 2: the cycle counter

`perf stat -e cycles` counting in both `cycles:u` and `cycles:k` says the counter runs at EL0 and
at the kernel's level. Graviton3 runs at 2.6 GHz, so cycles per second near that rules out a
synthesized counter. Under HVF, by contrast, it ticks at 128 times the generic timer
([the HVF note](../arm-plan-under-hvf.md)).

`pmccntr` reads `PMCCNTR_EL0` from EL0 twice. The first read should trap, because Linux does not
grant EL0 access by default. The second comes after a user-readable perf event is open, and should
print a rate near 2,600 MHz. A trap there too, with `cap_user_rdpmc 1`, means something above Linux
denies the counter. It also prints the sanitized `ID_AA64*` registers Linux shows EL0.

### Question 3: exception level, GIC and ITS

dmesg says `CPU: All CPU(s) started at EL2` or `EL1`, and KVM says whether VHE is in use. In
`APIC.dsl` (the MADT), the GIC distributor entry gives the GIC version. The redistributor and ITS
entries give their addresses, and the GICC entries give one MPIDR per CPU. A GICC entry with a
nonzero GICV or GICH address, or a VGIC maintenance interrupt, describes EL2 virtualization
support. `GTDT.dsl` gives the timer interrupts, including the EL2 timer's.

### Question 4: the serial console

`serial-linux.log` answers it for the firmware. Firmware lines before GRUB's mean UEFI output
reaches the port. A log that starts at the kernel's first line means it does not. The console
output API's copy (`console-nife.txt`) is a second witness, from AWS's own buffer.

`serial-nife.log` is nife's boot. It reads the way
[the Oracle runbook's section 5](oracle-a1.md#5-read-the-console) describes, with one difference.
Metal RAM almost certainly does not start at QEMU `virt`'s `0x4000_0000`. So a `uefi_loader:`
refusal to place the kernel, with the firmware's memory map printed, is the expected result until
the loader can place the kernel anywhere. That memory map is itself evidence. A firmware `Shell>`
prompt instead means the AMI registration or the disk is wrong.

## 8. Teardown, and proof nothing bills

`run all` tears down and checks on its own. To repeat either by hand:

```sh
notes/rented-metal/aws-graviton/run teardown     # everything tagged, in the profile's region
notes/rented-metal/aws-graviton/run check        # every region, tagged or not; exit 1 if anything is left
```

`run check` lists any instance not terminated, and every volume, owned snapshot, owned image and
Elastic IP, in every enabled region. It exits 0 only when all are zero. Those are everything this
probe can bill for after the instances end.

When no further run is planned, retire the key, then delete the local profile:

```sh
aws iam delete-access-key --user-name nife-metal \
    --access-key-id "$(aws configure get aws_access_key_id --profile nife-metal)" --profile nife-metal
```

The budget, the user and the policy cost nothing while idle. Closing the account is calef's call.

## Costs and risks

Prices are us-east-1 on-demand, from a price aggregator (secondary) and AWS's own pricing pages,
read 2026-10-21.

| Item | What it costs or risks | What limits it |
|---|---|---|
| `c7g.metal` | $2.32 an hour, per second | Two instances, about 60 and 30 minutes: about $3.50 |
| The metal boot | AWS says up to 20 minutes or more before the instance is reachable, billed as running time | Counted in the row above; `run` waits up to an hour for ssh |
| The 60-second minimum | each launch bills at least one minute | Two launches: under 10 cents |
| A forgotten instance | $2.32 an hour, about $56 a day | The self-shutdown (Linux only), the exit trap, the watchdog, then the budget email hours later |
| EBS volumes | gp3 at about $0.08 per GB-month: an 8 GB and a 1 GB root, for an hour | Deleted on termination |
| EBS snapshot storage | about $0.05 per GB-month; the snapshot holds only the image's written blocks, a few MB | Deleted at teardown; `run check` finds a survivor |
| The registered AMI | nothing beyond its snapshot | Deregistered at teardown |
| Data egress | $0.09 per GB past AWS's free allowance; this run sends out a few MB | Nothing to limit |
| Public IPv4 | $0.005 an hour per instance | Released at termination |
| The budget | free; AWS charges only for budgets with actions and for reports | n/a |
| The vCPU quota | a new account may run 5 vCPUs of standard instances; `c7g.metal` needs 64 | `bootstrap.sh` asks for 64 on day one; AWS may take days or ask why |
| Capacity | a zone can refuse with `InsufficientInstanceCapacity` | Set `AZ` to another zone that offers the type |
| The serial session | AWS ends it at about an hour, and once per maintenance event | `run` reconnects every 10 seconds; early firmware output before the first connect can be lost, which the console output API's copy partly covers |
| A leaked access key | it can do only what section 2 allows, in one region, to tagged resources and one instance type | Retire it after the run |

A clean run should cost about $4. A run whose workstation dies mid-way can cost about $8 before the
watchdog's 180 minutes, and more if the workstation sleeps.

## BUGS

- Nothing here has met a real account. The IAM policy is the likeliest thing to be wrong.
  `RunInstances` checks permissions on several resource types at once, and a missing one fails
  with an encoded message (`aws sts decode-authorization-message` reads it, given the permission,
  which this user lacks). The first run corrects this file.
- The nife instance cannot end itself. If the workstation dies or sleeps during its boot, it bills
  until someone runs `run teardown`. A CloudWatch alarm that terminates on a failed status check
  could close that, and is untested.
- Whether a metal instance reaches `running` before its firmware prints is unknown. If it does
  not, the serial capture starts too late for the earliest UEFI lines.
- `probe.sh` depends on Ubuntu's package names. Every answer it needs is also read from `/sys`,
  `/proc` and dmesg, so a package failure costs only the decompiled tables (the raw ones are in
  the tarball).

## Sources, read 2026-10-21 (UTC)

- AWS, [Connect to the EC2 Serial Console](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/connect-to-serial-console.html): the key push, the `<id>.port0` user, 60 seconds, one session of about an hour, endpoints and fingerprints per region.
- AWS, [Configure access to the EC2 Serial Console](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/configure-access-to-serial-console.html): the account-level switch.
- AWS, [Nitro instances](https://docs.aws.amazon.com/ec2/latest/instancetypes/ec2-nitro-instances.html): the metal boot time; UEFI and ACPI.
- AWS, [Boot modes](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/ami-boot.html): `--boot-mode uefi`.
- AWS, [AWS Budgets pricing](https://aws.amazon.com/aws-cost-management/aws-budgets/pricing/): budgets that only notify are free.
- AWS, [VPC pricing](https://aws.amazon.com/vpc/pricing/): $0.005 an hour per public IPv4 address.
- AWS, [EC2 on-demand pricing](https://aws.amazon.com/ec2/pricing/on-demand/): per-second billing, 60-second minimum.
- AWS, [instance type quotas](https://docs.aws.amazon.com/ec2/latest/instancetypes/ec2-instance-quotas.html) (through a mirror and the CLI examples): quota `L-1216C47A`, default 5 vCPUs.
- instances.vantage.sh, [c7g.metal](https://instances.vantage.sh/aws/ec2/c7g.metal) (secondary): $2.32 an hour.

*Name: provisional (the AWS Graviton probe's lane, 2026-10-21 UTC), after the machine family it
probes. The directory `aws-graviton/` follows §212's siting default.*
