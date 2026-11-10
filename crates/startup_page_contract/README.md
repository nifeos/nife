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
  in pages (`u32`) and the first page's virtual address (`u64`), all little-endian. A page holds
  up to `MAXIMUM_WINDOWS` (255).
- `VERSION` (1) is the digit the magic carries and changes only with the entry format.
- A reader takes the kinds it knows and skips the others, so a spawner may add kinds and move
  windows without breaking an earlier program. Each kind is a number minted by the contract crate
  that both the spawner and the program link (`driver_process_contract` mints the drivers'), never
  reused; zero names none.
- `write(page, windows)` makes the page and refuses a list a reader could misread: a duplicate
  kind, a window of zero pages, an unaligned address, or more windows than fit.
  `StartupPage::parse(bytes)` reads one, and `StartupPage::mapped()` reads the one at the fixed
  address.
- `StartupWindow` is the reader a program keeps as a `static`: it names a kind and the bytes the
  program uses, and `virtual_address()` reads the page on first use and panics, naming the kind,
  when the page is missing, lists no such window, or lists one too short.

## EXAMPLES

```rust
use startup_page_contract::{PAGE_BYTES, StartupPage, Window, WindowKind, write};

const DOORBELL_PAGE: WindowKind = WindowKind(2);
const DATA_PLANE: WindowKind = WindowKind(3);

let mut page = [0u8; PAGE_BYTES];
let doorbells = Window {
    kind: DOORBELL_PAGE,
    virtual_address: 0x9c_0000,
    pages: 1,
};
write(&mut page, &[doorbells]).unwrap();
let read = StartupPage::parse(&page).unwrap();
assert_eq!(read.window(DOORBELL_PAGE), Some(doorbells));
assert_eq!(read.window(DATA_PLANE), None);
```

## What is proven

Host tests only, no Kani harness. They hold that a written page reads back entry for entry, that a
reader skips a kind it does not know, that bytes which are not a page read as none, that the
writer refuses each malformed list and writes nothing for it, that a full page holds exactly
`MAXIMUM_WINDOWS`, and that the page sits in the runtime band clear of its neighbors.

## Building and testing

`cargo test -p startup_page_contract` runs the host tests and the examples.

## BUGS

- One page per address space: two programs sharing a space would share it, and two instances of one
  program in one space could not tell their windows apart.
- A process the kernel builds by hand, or one a userspace builder spawns, has no page yet.
- Each contract crate mints its own kinds, and nothing checks numbers across crates. Today only
  `driver_process_contract` mints any, and its own test holds them apart.
- `StartupWindow::virtual_address` reads the page at its fixed address, which no host process maps,
  so no host test reaches that one line; its logic is `virtual_address_in`, which host tests drive.
  The booted system tests of the e1000e and non-volatile memory express drivers reach them only
  through it.

## Minimum Rust version

Rust 1.88.

## License

MIT OR Apache-2.0.
