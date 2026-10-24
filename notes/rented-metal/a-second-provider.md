# What a second provider is a test of, and what it costs

*Name: provisional. After the parent's "What a second provider is a test of".*

An appendix to [`notes/rented-metal.md`](../rented-metal.md). It holds the case for a second
x86_64 provider as an experiment rather than redundancy, and the side-by-side table of how each
provider delivers a kernel, a console and power. It also holds the cost of running several vendors.

calef, 2026-09-23: *"a nife that runs on one cloud platform but not another is also its own form of
risk."* This section states that as an experiment, because framing it as redundancy would waste it.

The gap it sits in. Fatal risk 9 (the HAL is a fiction, and an architecture costs a restructure
rather than a port) rests on a claim about *"actual functioning on the three silicons"*. That is
about architectures. Two x86_64 providers are the same silicon with different firmware,
different boot delivery, and different tables at entry. A nife that boots on xenon and not on some
provider's metal falls through that gap entirely, and the HAL being a fiction would surface first
in that form, long before it surfaced as an architecture costing a restructure. Whether that widens
risk 9 or becomes its own entry is an architect's call; this file does not touch
`design/fatal-risks/README.md`.

The hypothesis, stated before the numbers exist, in the shape this tree already uses: nife boots
unchanged on two providers' x86_64 metal, and the only differences are in how the image and the
console are delivered. A failure is the valuable outcome. It would name a concrete assumption
nife makes about what a machine hands it at entry, and that assumption is currently invisible because
every x86_64 boot this project has ever done was on QEMU or on xenon.

Finding no difference is also a result, and it would weaken the case for the second account. This
file does not get to assume diversity.

## The fifth column, side by side, for x86_64

| | Scaleway Elastic Metal | OVHcloud Rise | Hetzner dedicated |
|---|---|---|---|
| How a custom kernel is delivered | rescue mode plus the serial console; documented as supporting *"the most exotic operating systems"* on the RV1, with `Reinstall a server`, `Configure custom disk partitioning` and `Use rescue mode` as the Elastic Metal how-tos | IPMI virtual media and netboot, with the OS installed through a KVM session | `installimage` from the rescue system, which wants a distribution tarball, **or** an ISO mounted over SMB through a KVM console **obtained by a support request** |
| Console channel | serial console, over SSH, activated per account | IPMI: HTML KVM, Java KVM, **serial over LAN in the browser and serial over LAN over SSH** | KVM console on request; no standing serial channel documented |
| Power channel | API `Start`, `Stop`, `Reboot` server actions | control panel and IPMI; API coverage not confirmed here | *"24h reset service via web interface"* |
| What the machine presents at entry | x86 firmware, provider-installed; not documented further | x86 firmware, vendor BMC present | x86 firmware, no BMC exposed by default |

These are not the same interface, and that is the finding. Scaleway hands you a serial line over
SSH and three REST verbs. OVH hands you a BMC, which is a different object with its own virtual
media and its own serial-over-LAN, and a genuinely different way of getting an image onto the disk.
Hetzner puts a human in the loop for the console. An automation built only against Scaleway would
express "provision a machine" as three REST calls and a serial SSH session, and would not even have
a verb for "mount this ISO through the BMC".

Where it is weaker than it sounds: the *firmware* row is the one that matters most for risk 9 and
it is the one this file could not fill in. None of the three publishes what their machines hand a
kernel at entry, which is exactly the kind of thing that gets discovered by printing it and getting
zero. So the anti-lock-in argument about *our tooling* is solidly evidenced by the table above; the
argument about *nife's own boot assumptions* is a hypothesis that buying the second machine tests,
and it is not evidence yet.

## What multi-vendor costs

Naming it, because pretending it is free would be the dishonest half.

- Three accounts, three billing relationships. Three invoices, three renewal dates, three places a
  forgotten machine accrues charges.
- Three provisioning APIs and three auth models. Scaleway's CLI and secret key, OVH's application
  key and consumer key, AWS's IAM. Any script that automates "get me a machine" has to speak all
  three or be written once per vendor.
- Three sets of credentials to hold and rotate, for one person, which is the kind of thing that
  quietly stops happening.
- Three console mechanisms and three ways to get an image onto a disk, which is the same fact as
  the benefit above read from the cost side. A second vendor is not a drop-in.

The honest balance. Two providers for x86_64 is worth it because the interfaces genuinely differ
and the difference is the experiment. A third x86_64 provider is not, and a second provider for
aarch64 or riscv64 is not purchasable today because there is barely a first.
