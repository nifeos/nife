# The entropy contract's readiness rule, and its two fill loops

`crates/entropy_contract` defines the request and reply that carry random bytes out of the entropy
service, under §44 (entropy is a capability). [entropy.md](../entropy.md) has the shape, why the
bytes ride in the reply, and why one number cannot collide. This page holds the reasons behind the
readiness rule and the two ways a client fills a buffer, which the crate's doc comments link.

## Ready means bytes in hand

The service sends `READY` on its readiness endpoint only once its first bytes are in hand, and that
is a claim about the bytes, not about the handshake. A service that finished its register sequence
and holds nothing, or holds a bufferful of zeros, has not met the condition. A JH7110 TRNG whose
block clock is gated completes its register sequence and reads zeros
(`design/roadmap/0159-jh7110-trng-driver.md`). Every backend therefore calls `readiness` rather than
composing the word itself.

`readiness` takes an iterator rather than a slice because the three backends hold their first
bufferful in three different places: an array from an instruction, an array from a register file,
and a DMA page only volatile reads can see. An empty iterator means the device gave nothing, which
is a different answer from a bufferful of zeros, and a bench session reading a report word needs to
tell them apart.

## Refusing an all-zero draw

An all-zero first draw is read as a dead device, and the source is condemned for the boot. That is
wrong with probability 2^-64 for the instruction source, 2^-256 for the JH7110 and 2^-2048 for
virtio. It is the right way round: a false "dead" costs one boot's entropy, while a false "alive"
costs every secret derived from it, silently.

The check is bring-up only. A source that degrades, or latches a nonzero answer, afterwards is not
caught by it. Continuous health testing is `design/decisions/0137-trng-health-tests.md`'s to
decide.

## Two fill loops, both deliberate

`fill_with` refuses a short answer: any reply that is not a count, or that delivers fewer bytes than
were asked for, ends the fill with `None`. The `CALL` is a parameter so the crate stays a contract
that makes no syscall; `user_mode_runtime::entropy::fill` passes a real one, and the refusals are
tested on the host against fakes.

The std PAL (`patches/std-nife/overlay/std/src/sys/random/nife.rs`) does not use it. It accepts a
partial count and asks again, and panics where `fill_with` refuses. Both are deliberate.
