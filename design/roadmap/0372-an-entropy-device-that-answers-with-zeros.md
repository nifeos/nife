---
status: NOT-STARTED
raised: 2026-09-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 372. Nothing in QEMU can hand the entropy service a bufferful of zeros

Filed as a proposal on 2026-09-04 by the
`maintainer/ready-on-a-dead-device` lane; promoted by milestone 433 on 2026-09-19. Checked against
the tree that day and unchanged in both directions. `entropy_contract::readiness` still refuses
`READY` on an all-zero first bufferful and still has that decision tested only on the host
(`an_all_zero_first_bufferful_is_never_ready`). No QEMU runner attaches an `rng-random` backend
of any kind: the aarch64 runner's only mention of one is a comment saying QEMU defaults to the
host's `/dev/urandom`. The two consumers of the readiness word, `components/src/entropy.rs` and
`components/src/jh7110_entropy.rs`, still have no machine that can make either of them fail.

QEMU already has the device (`-object rng-random,filename=/dev/zero`), the service
already has the check, and the only new thinking is how a wiring names *which* virtio-rng it wants.

**What the work is.** `entropy_contract::readiness` decides the readiness word from the first bufferful,
and refuses `READY` when every byte of it is zero (milestone 159's block, "Fixed 2026-09-04"). That
decision is host-tested in `entropy_contract` (the repository `nifeos/entropy_contract`), and the drivers' three calls to it are not tested
anywhere: no machine this repository boots can produce a device that answers with zeros. The one that
did was radon, whose TRNG has a gated clock, and it is not a machine CI can run.

The shape. QEMU can make one: `-object rng-random,filename=/dev/zero,id=zeros` behind a second
`virtio-rng-device`. What is missing is a way for a test to reach *that* device rather than the real
one. `entropy_service::Bus` picks a transport and the scan takes the first virtio-rng it
finds on it. So this is a runner line plus a way to name which device a wiring should take, and the
end of it is a test asserting that the service reports `bringup_failure(STEP_FIRST_ALL_ZERO)` and
then answers `NO_ENTROPY` to every request.

**Why it was not done in the lane that wanted it.** It lands in the test-wiring hotspot AGENTS.md
names (`kernel/src/user/tests.rs`, the QEMU runners, `entropy_service.rs`). A permanently-dead
entropy device sitting on a bus every other entropy test scans is a way to make those tests flaky
for a reason unrelated to what they assert. That is a design question about how a wiring names a
device, and it is larger than the defect it would have covered.

What it would also unlock. The same lever tests the *dry* device path (`filename=/dev/null`, or a
`rng-random` that never answers), which is likewise only ever exercised by a device nobody has.

## Index row

`entropy_contract::readiness` refuses `READY` when the first bufferful is all zeros, which is the
check that caught radon's gated-clock TRNG. No machine this repository boots can produce a
device that answers that way, so the drivers' three calls to it are tested nowhere. QEMU can make
one with `-object rng-random,filename=/dev/zero` behind a second `virtio-rng-device`. What is
missing is a way for a test to reach that device rather than the real one, because the scan takes
the first virtio-rng on the transport. This is a runner line plus a way for a wiring to name
which device it wants. The lane that wanted it refused to bolt it on. A permanently dead entropy
device sitting on a bus every other entropy test scans is a way to make those tests flaky for
reasons unrelated to what they assert, and it lands in the test-wiring hotspot. The same lever tests
the dry-device path (`filename=/dev/null`), which is likewise only ever exercised by a device
nobody has.
