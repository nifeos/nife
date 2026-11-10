# startup_page_contract

The page that tells a process where its memory windows were mapped. A spawner writes it before the
process runs and maps it read-only at one fixed address; the process reads each window's address
from it instead of compiling the address in. `no_std`; one dependency, `address_space_map`, for the
page size and the band the page sits in.

## The contract

- The page sits at `PAGE_VIRTUAL_ADDRESS` (`0x1800_0000`), in the runtime band. It is the only
  address in the contract.
- It opens with `MAGIC` (the ASCII text `STARTPG1`), then the window count as a little-endian `u32`
  and four zero bytes. Entries follow from byte 16, sixteen bytes each: the kind (`u32`), the length
  in pages (`u32`) and the first page's address (`u64`), all little-endian. A page holds up to
  `MAXIMUM_WINDOWS` (255).
- `VERSION` (1) is the digit the magic carries and changes only with the entry format.
- A reader takes the kinds it knows and skips the others, so a spawner may add kinds and move
  windows without breaking an earlier program. The kinds a spawner in this tree hands out are in
  `kind`, one number each, never reused; zero names none.
- `write(page, windows)` makes the page and refuses a list a reader could misread: a duplicate
  kind, a window of zero pages, an unaligned address, or more windows than fit.
  `StartupPage::parse(bytes)` reads one, and `StartupPage::mapped()` reads the one at the fixed
  address.

## EXAMPLES

```rust
use startup_page_contract::{PAGE_BYTES, StartupPage, Window, kind, write};

let mut page = [0u8; PAGE_BYTES];
let doorbells = Window {
    kind: kind::NON_VOLATILE_MEMORY_EXPRESS_DOORBELL_PAGE,
    address: 0x9c_0000,
    pages: 1,
};
write(&mut page, &[doorbells]).unwrap();
let read = StartupPage::parse(&page).unwrap();
assert_eq!(read.window(kind::NON_VOLATILE_MEMORY_EXPRESS_DOORBELL_PAGE), Some(doorbells));
assert_eq!(read.window(kind::E1000E_DATA_PLANE), None);
```

## What is proven

Host tests only, no Kani harness. They hold that a written page reads back entry for entry, that a
reader skips a kind it does not know, that bytes which are not a page read as none, that the
writer refuses each malformed list and writes nothing for it, that a full page holds exactly
`MAXIMUM_WINDOWS`, and that every kind's number is distinct and nonzero.

## Building and testing

`cargo test -p startup_page_contract` runs the host tests and the examples.

## BUGS

- One page per address space: two programs sharing a space would share it, and two instances of one
  program in one space could not tell their windows apart.
- A process the kernel builds by hand, or one a userspace builder spawns, has no page yet.

## Minimum Rust version

Rust 1.88.

## License

MIT OR Apache-2.0.
