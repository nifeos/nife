---
status: NOT-STARTED
raised: 2026-09-24
promoted_from: a-virtio-slot-comes-back-when-its-driver-dies
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 647. A virtio slot should come back when its driver dies

*Title and slug are drafts.*
Raised by milestone 198 (a package manager)'s rung 3a, whose package-fetch test ran the virtio
device table out after its thirty-third slot: both QEMU legs panicked with `more virtio devices
than MAX_DEVICES` in CI, and `MAX_DEVICES` took its tenth bump. `kernel/src/virtio.rs`'s own comment
on `MAX_DEVICES` names the unregister as "the next lane's work item"; a bump is not that.
**Name provisional**: this file's stem is a lane's coinage.

Reusing a slot means a stale `Object::Virtio` capability must not resolve to the
device that took its slot next. That needs a generational name on the table, the machinery region
slots and thread ids already use. It changes what a capability means, and DECISIONS §16 (object
revocation) treats that as a design fork rather than a task. The seventh receipt in the comment also
names the other half: whether a transport can be handed to a second driver after the first
programmed the device. It also asks how that meets `entropy_service::ensure`'s one-service-per-device rule.

## The measurement

`MAX_DEVICES` counts every transport a boot has *ever* registered, never how many exist. The
aarch64 machine has five devices and the suite registers thirty-four. Ten bumps are recorded in the
comment, each a receipt. The memory reason one lane gave for refusing a bump was fixed on
2026-08-16 (frames come back with `Holding::release_or_fail`). The counter is now the only
ceiling, and it binds on every lane that adds a confined-device test.

## Exit criterion

A test that registers and releases more transports than `MAX_DEVICES` in one boot passes, and a
capability to a released device is refused rather than resolving to its successor.

## Index row

The virtio device table never reuses a slot, so every confined-device test costs one for the rest
of the boot; ten bumps so far. Proposed: a generational unregister on driver death.
