---
status: NOT-STARTED
raised: 2026-09-24
promoted_from: xenon-may-carry-amt
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 653. xenon may carry Intel AMT, which would power-cycle it and give it a console with nobody at the desk

*Title and slug are drafts.*
Raised by the lane that brought milestone 249 (the boot lottery is sampled by a person walking to the board)'s reboot to `x86_64`,
while pricing hang recovery that needs no person. Milestone 87 (the x86_64 bare-metal machine) chose a smart plug over management
firmware (`design/roadmap/0087-x86-machine.md`, "Remote power cycling by smart plug, not by management
firmware"), and no plug was ever bought. This asks whether the hardware already on the desk makes
the plug unnecessary.

Enabling AMT is a firmware change, and it needs a cable onto calef's network.
Both are his.

## What the tree and the vendor say

- The machine. notes/xenon-firmware.md: Dell OptiPlex 7050 Micro, BIOS 1.27.0, Core i5-7500T.
  `Enable MEBx Hotkey` is ticked and `Enable USB Provision` is unticked.
- The parts qualify. Intel lists the i5-7500T as "Intel vPro Platform" eligible (ark, SKU 97121).
  The 7050 Micro uses the Q270 chipset and an i219-LM NIC, which Dell's spec sheet says "is required
  to support Intel vPro".
- The open question. Dell sold the 7050 Micro with one of three factory options: Intel vPro (AMT
  11.0), Standard Manageability, or No Out-of-Band. Out-of-band management "cannot be upgraded
  post-purchase". An MEBx hotkey rules out the third, as an inference. Nobody knows which of the
  other two xenon has. Ctrl-P at POST answers it in a minute, and so does a Dell service-tag
  lookup of `25XNBM2`.

## What it would give, if full AMT

Remote power on, off and reset, over WS-MAN on ports 16992 and 16993. Serial-over-LAN, which would
be xenon's console without a USB cable. Remote KVM on full vPro only. Tools that run from patagonia:
`amtterm` and `amttool` for SOL and power, `wsmancli`. MeshCommander is discontinued. (The
capability list and the tools are recalled, not read, and should be checked before anyone relies on
them.)

## What enabling it costs, and whose each step is

1. At the desk (a firmware change, calef's): Ctrl-P into MEBx, change the default password, turn
   on the network interface, and set an address.
2. A network (calef's): a cable from the onboard i219-LM to a network patagonia can reach.
   notes/xenon-firmware.md records that xenon "is not on a network any lane can reach".
3. Optional (a firmware change): set Deep Sleep Control to Disabled. Its recorded setting,
   enabled in S4 and S5, turns off remote manageability while the machine is off.

## Caveats worth pricing before saying yes

- The ME shares the i219 with the host. nife's own e1000e driver resetting the NIC may drop AMT
  traffic mid-session (recalled, not read). A soak that uses the network would need a test.
- AMT 11 carried INTEL-SA-00075 (the 2017 authentication bypass; recalled). Check the ME firmware
  version before AMT goes on any network.
- It is xenon's alone. radon and argon have nothing like it, so it does not remove the case for
  smart plugs (milestone 224 (nothing can power-cycle radon, so a hung soak needs a person)) or watchdogs (milestone 593 (a wedged kernel resets itself)).

Recommendation: press Ctrl-P at the next bench session. That costs nothing and changes nothing.
Decide on the network after that, with the answer in hand.

## Index row

xenon is a Dell OptiPlex that may carry Intel AMT, which would power-cycle it and give it a console with nobody at the desk. Proposed: find out, since enabling it makes the smart plug unnecessary; it needs a firmware change and a network cable that only calef can supply.
