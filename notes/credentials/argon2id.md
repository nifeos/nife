# Credentials: Argon2id, and why the service depends rather than writes

*Name: provisional, after the algorithm it records.*

An appendix to [`notes/credentials.md`](../credentials.md). It records the dependency decision, the
known-answer vectors, a bug found in the dependency, and the cost parameters with their
measurements. The crates were renamed after this was written: `cred` is now `credentialer` and
`cred_proto` is `credential_contract`.

DECISIONS §46 draws the line at **exposure**: write the thing whose correctness you can argue from
the spec, depend on the thing whose correctness is won by many people checking it against published
answers over many years. A password KDF is squarely the second kind. So this takes a dependency,
and the amendment to §46 says depend rather than *vendor*: a vendored copy is invisible to
`cargo-deny` and `cargo-audit`, and crypto is the code that most needs to be visible to them.

The crate is RustCrypto's `argon2` 0.5.3, `default-features = false`, plus `subtle` for
constant-time comparison and `zeroize` so the library scrubs its own memory. The whole graph is
nine crates, all RustCrypto core, and it passes `deny.toml` unchanged: advisories, licenses, bans,
sources. `argon2` was already in this tree, in the redoxfs_server workspace, underneath RedoxFS's
encryption path, so the license question had an answer before the question was asked.

Argon2id specifically, not Argon2i or Argon2d: it is RFC 9106 §4's recommendation, it is what
OWASP puts first, and it is the variant that resists both a side-channel adversary (the Argon2i
half) and a time-memory tradeoff (the Argon2d half). The alternatives were scrypt, which is the
older answer to the same question with no advantage here, and PBKDF2, which is not memory-hard at
all and would make a GPU attack cheap.

### The vectors are the point

A dependency whose answers you never check is a dependency you have merely hoped about. So
`crates/credentialer`'s tests run:

- RFC 9106 §5.3's Argon2id vector (m=32 KiB, t=3, p=4, with a secret key and associated data);
- the reference implementation's vectors (phc-winner-argon2 `src/test.c`) at its two smallest
  memory settings, through `credentialer`'s own `kdf` function, so they pin our wiring (algorithm, version,
  tag length, and the no-allocation entry point) and not only the library's arithmetic.

If a version bump changes an answer, these fail before anything else does.

### A bug we found in the dependency

The exhaustive record-corruption test (every byte position crossed with four bit patterns) panicked
inside `argon2`. `Params::new` evaluates `m_cost < p_cost * 8` before it range-checks `p_cost`,
so a `p_cost` above `u32::MAX / 8` overflows the multiply.

- In release it wraps, the later bound check still fires, and the answer is correct.
- In debug, which is what `cargo xtask test` builds the userspace programs as, overflow checks
  are on and it panics.

A credential service that a cost value can kill is a login outage anybody can cause. `Cost::new`
therefore enforces Argon2's documented ranges itself, in an order that cannot overflow, before the
value crosses the boundary. This is DECISIONS §31's confine-what-you-did-not-write rule arriving at
a much smaller scale than a C component, and it is also the argument for sweeping every byte rather
than picking a few interesting ones: nobody would have hand-written that case.

### The cost parameters, and where they fall short

Wired: m = 4 MiB, t = 3, p = 1. That is below OWASP's recommendation of 19 MiB / t=2, and
the entry says so where the constant is defined rather than leaving it as an absence.

The reason is the machine. The whole system under test is 128 MiB of QEMU RAM, the filesystem
server alone reserves 8 MiB of it, and `kernel/src/user.rs` already records that three 8 MiB
untypeds do not fit. Three passes over 4 MiB is 12 MiB-passes of work against OWASP's 38. So this
is roughly a third of the recommended cost, not a token gesture. It is genuinely memory-hard: an
attacker's GPU still has to find 4 MiB per guess, which is where Argon2's advantage over PBKDF2
lives.

**The parameters travel in the record, not in the code.** Raising them on real hardware is a
provisioning change plus a budget change, and both fail loudly rather than silently.

Measured on the development machine (Apple Silicon, native, not under QEMU):

| build | m=1 MiB t=2 | m=4 MiB t=3 | m=8 MiB t=3 |
|---|---|---|---|
| optimized | 2.0 ms | **5.3 ms** | 7.3 ms |
| unoptimised | 25.8 ms | 66.7 ms | 135.0 ms |

Which is why `Cargo.toml` compiles `credentialer`, `argon2`, `blake2` and `digest` at
`opt-level = 2` even in a debug build. That is `measured_boot`'s precedent (the measured-boot
SHA-256) applied to a function that costs an order of magnitude more, before TCG multiplies it
again. Argon2 is memory-hard on purpose; an unoptimised build does not make an attacker's job
harder, only ours.
