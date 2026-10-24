# A program that checks its own loaded image

An appendix to [`notes/elf.md`](../elf.md), which explains how the kernel loads an ELF file and what
the loader refuses. This file holds what `crates/loaded_image_check` proves about a loaded image from
inside the process, and why it fails the way it does.

## What each check would catch

- `.rodata` holds the bytes the file said it did, so the read-only segment was mapped from the file
  rather than zeroed.
- `.data` holds its initializer and is writable, so the loader copied it rather than pointing at the
  file's pages read-only.
- `.bss` reads zero before anything writes it. Those bytes are not in the file, so a loader that
  forgot them would leave whatever the frame allocator handed over.
- The stack survives eight frames of recursion, so `sp` was set to real, writable memory with room
  under it.

None of this needs a capability. Every byte it reads belongs to the process already, which is why
the milestone-7 test spawns the self-checker with no authority at all.

## How it says no

It traps. A program whose loader is broken has no reliable way to report anything, because the
endpoint it would report on may not be mapped either. The kernel turns the trap into a fault and
kills the thread, which the test counts.

`fail` is a parameter rather than a call into `user_mode_runtime`. That keeps the crate
host-buildable: the caller hands over its own way to die.

## What it cannot see

The markers are `#[unsafe(no_mangle)]` statics, so exactly one copy of the crate may be linked into a
binary. Two would be a duplicate symbol at link time rather than a silent wrong answer. It does mean
the crate cannot be a transitive dependency of something a program also depends on directly.

It cannot check the executable segment. A loader that mapped `.text` writable would pass every check
here. The `W^X` assertions in `system_tests/src/user/elf_loader_tests.rs` catch that instead.
