# Verus: the examples, runnable

An appendix to [`../verus.md`](../verus.md), holding its EXAMPLES. Every command here was run on
this Mac on 2026-09-20.

Reproduce the `asm!` boundary in Kani. Append to `kernel/src/arch/aarch64/timer.rs`, run, then
`git checkout` the file:

```rust
#[cfg(kani)]
mod scratch_proofs {
    #[kani::proof]
    fn the_rearm_scratch_probe() {
        let interval: u64 = kani::any();
        kani::assume(interval > 0 && interval < 1_000_000);
        super::rearm(interval);
    }
}
```

```console
$ cargo kani -p kernel -Z unstable-options --ignore-global-asm \
      --harness the_rearm_scratch_probe
...
Failed Checks: TerminatorKind::InlineAsm is not currently supported by Kani.
VERIFICATION:- FAILED
```

Run Verus at all, from nothing, on this Mac (about 450 MB and one toolchain):

```console
$ curl -sL -o verus.zip https://github.com/verus-lang/verus/releases/download/\
release%2F0.2026.09.20.aef82ed/verus-0.2026.09.20.aef82ed-arm64-macos.zip
$ unzip -q verus.zip -d verus
$ rustup toolchain install 1.98.1-aarch64-apple-darwin
$ verus/verus-arm64-macos/verus probe.rs --crate-type=lib
verification results:: 1 verified, 0 errors
```

The probe, which is `run_end_va`'s property in `no_std` Verus:

```rust
#![no_std]
use vstd::prelude::*;
verus!{
pub open spec fn last_of(va: nat, count: nat) -> int { va + (count - 1) * 4096 }

pub fn run_end_va(va: u64, count: u64) -> (r: Option<u64>)
    requires count >= 1,
    ensures match r {
        Some(last) => last as int == last_of(va as nat, count as nat),
        None => va as nat + (count as nat - 1) * 4096 > u64::MAX as nat,
    }
{
    let pages = (count - 1) as u128 * 4096u128;
    let last = va as u128 + pages;
    if last > u64::MAX as u128 { None } else { Some(last as u64) }
}
}
```

Watch a false axiom pass, which is the caveat that matters most:

```console
$ cat seam.rs
#![no_std]
use vstd::prelude::*;
verus!{
#[verifier::external_body]
pub fn read_counter() -> (r: u64) ensures r > 0 {
    let v: u64;
    unsafe { core::arch::asm!("mrs {}, cntvct_el0", out(reg) v) };
    v
}
pub fn nonzero_counter() -> (r: u64) ensures r >= 1 { read_counter() }
}
$ verus/verus-arm64-macos/verus seam.rs --crate-type=lib
verification results:: 1 verified, 0 errors
```

`CNTVCT_EL0` is zero at reset. The green line does not know that.
