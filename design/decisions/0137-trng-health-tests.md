---
status: DECIDED
raised: 2026-09-01
decided: 2026-10-28
ratified_by: calef
---

# 137. A hardware TRNG with no published health-test claim

Ruled 2026-10-28 (UTC): C, continuous SP 800-90B health tests in the driver with a hard action on
failure: fall back to another source if one is present, otherwise refuse and fail loudly; min-entropy
assumed at one bit per byte, recorded as a named constant with its reason. Bytes still arrive
untransformed under §44's contract; the claim is weaker and truer.

Raised 2026-09-01 by the maintainer, from milestone 159 (a real hardware entropy source: the
JH7110's TRNG)'s (a real hardware entropy source: the JH7110's TRNG) lane. That lane found the
question and correctly declined to answer it: a developer does not write a decision.

## What is being decided

Whether nife performs its own health tests on hardware entropy, and what it does when one fails.

The JH7110's datasheet makes no SP 800-90B, FIPS 140, or AIS-31 claim anywhere. It offers a
random number generator and says nothing about what standard, if any, its output has been validated
against. That is not unusual for a SoC of this class, and it is the situation nife is actually in
rather than a hypothetical.

So there is a gap between what the entropy service promises its callers and what the device
promises us, and this decision is about who closes it.

## Why it is a decision rather than an implementation detail

Entropy is the case where a silent failure is worst. A file server that breaks stops working. A
random number generator that breaks keeps working, returns plausible-looking bytes, and everything
built on it is compromised without a single error. The failure has no symptom, which is why every
standard in this area is mostly about detecting it.

It is also the *move fast on what can be undone* tenet's irreversible category, twice over. Keys
derived from bad entropy do not become good later, and a claim about this system's randomness, once
published, cannot be withdrawn from whoever read it.

## The options

A. Trust the device. Read bits, serve them. Simplest, and defensible for a demonstrator that
makes no cryptographic claim. Requires that nothing in this tree implies otherwise, which is the
part to check rather than assume.

B. Continuous health tests in the driver, SP 800-90B's shape: a repetition count test and an
adaptive proportion test on the raw stream, with a defined action on failure. Well-specified,
implementable without certification, and the standard exists precisely because vendors do not always
say what theirs does. Costs a real design decision about the failure action.

C. Treat the hardware as one source among several and mix, which is what milestone 162 (real hardware
entropy on x86_64 and aarch64: RDSEED and RNDRRS) and the existing backends make possible. A compromised source degrades the
pool rather than defining it. This is what most operating systems actually do.

The failure action is the harder half of B and C, and it is a capability question. Refusing to serve
is a denial of service that could brick a boot, while serving flagged bytes moves the decision to a
caller who may not check. §31 (the foreign-language seam)'s headline and the confinement work are
the neighborhood this sits in.

## Recommendation

C, with B's tests as the thing that decides a source's weight rather than a hard gate, and A
explicitly recorded as what the tree does today so the gap is visible rather than implied.

The reason is that C is the only option that stays honest when the answer to "is this device good"
is unknown, which is the actual situation. B alone turns an unknown into a boot-time failure, and A
alone turns it into a claim nobody checked.

**This is a recommendation and not a decision**, because it touches what the system tells a caller
about its own randomness, which is the category this project treats as irreversible.

## What is blocked until this is answered

Nothing today, and that is worth stating plainly so this does not read as urgent. Milestone 159's
driver serves bytes with no health test and its `BUGS` says so. What is blocked is any claim, in
documentation or a benchmark, that nife provides cryptographic-quality randomness on radon.
