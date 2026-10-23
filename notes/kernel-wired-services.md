# The services the kernel wires

`kernel/src/user.rs` is the index of the kernel's user-mode code, and most of its children are the
wiring of one confined service each. The kernel's part is the same every time: allocate the frames,
mint the endpoints, hand each process a `Spawn` literal, and know nothing about what the processes
then do. The wiring is the argument, because which capability lands in which table is the
isolation the service claims. This note carries the grant shapes the module doc comments link to.
[Userspace](userspace.md) is the introduction to user mode itself.

## The RedoxFS filesystem service

```text
  disk ──virtio──► block server ──blk IPC──► FS server ──file IPC──► client ──► report to kernel
```

Three confined processes and the endpoints and shared pages that wire them. The kernel never sees a
filesystem operation, an opcode or a byte of file data. The FS server owns RedoxFS and its own heap;
the block server owns the DMA confinement; the client holds only a directory capability. The service
drives the second MMIO block disk, the RedoxFS image; the first is the nifefs disk the driver tests
use.

## The block-device roster and the disk surveyor

Two authorities every other operating system hands out as one: a read-only mapping listing what
block devices exist, and a block-service endpoint for exactly one of them. A program with the first
can see the machine's disks and open none of them; a program with the second was handed one disk and
has no way to name a second. The kernel scans the buses, writes the page, confines one device under a
block server and spawns. It never reads a partition table: every byte of GPT judgment is in
`crates/globally_unique_identifier_partition_table`, tested on the host against tables `sgdisk` and
macOS `diskutil` wrote. See [block devices](block-devices.md).

## The display service

```text
  virtio-gpu ──virtio (PCIe, behind the IOMMU)──► gpu_driver ──display IPC──► painter
       │                                              │                             │
       └──── DMA: the whole region ───────────────────►│                             │
                                   the surface (pages 1..) ─────── shared ──────────┘
```

A framebuffer does not fit the single DMA page the disk and NIC drivers get, so the region is
`1 + graphics_contract::SURFACE_PAGE_FRAMES` contiguous frames: page 0 for the rings and control
buffers, the rest for the surface. Registering the whole run as the driver's DMA region keeps the
framebuffer inside the grant. The shadow-ring validator bounds every descriptor to it, and
`iommu::confine` maps exactly it, so the device can reach the pixels and nothing else. The client
maps only the surface frames. It never sees page 0, so it cannot touch a descriptor ring, and it
holds no `Virtio` capability, no interrupt and no physical address. See
[the framebuffer contract](framebuffer-contract.md).

## The compositor

```text
  display (or a kernel stand-in) ──gfx FLUSH(damage)──► compositor ◄──one doorbell──── window clients
                                           the scanout, shared ──┘  │                 (a surface each)
                                                                    └─► one input endpoint per focusable
```

One screen and several mutually distrusting clients. The isolation is a property of the shape of
the grants:

- Every client's control page and surface are its own frames, mapped at the same virtual addresses
  in every client, so a neighbor's surface is not somewhere a client can reach by guessing.
- The clients' frames are one contiguous run on purpose, so the page just past a client's grant
  really is its neighbor's memory. That makes the compositor's
  attack test a fair one: the attacker has the exact address, the bytes are physically adjacent,
  and the mapping is the only thing in its way.
- The screen and the window list are mapped read-only, and only into a client granted them. That
  mapping is the screenshot capability and the enumeration capability; there is no verb for either.

See [the compositor](compositor.md).

## The keyboard service

```text
  virtio-input ──virtio (PCIe, IOMMU)──► keyboard_driver ──the input ring──► whoever maps it
                                          └──doorbell COMMIT──► "look at the surfaces"
```

The driver gets the device, its interrupt, its DMA page, the doorbell and the ring page. It does not
get any client's endpoint, so it cannot choose who receives what it types: that is focus, and focus
is the compositor's decision, expressed as which input capabilities it uses. The ring is what makes
typing possible at all, since the doorbell every client holds is content-free, and a client ringing
it forever could not produce one character. In the test the kernel plays the compositor, holding the
doorbell and the ring, so the bytes a real keyboard produced are a value it can compare. The boot
calls `wire_device` alone, and the progenitor builds the driver.

## The clock service

The RTC's registers, the wall clock's offset and the propose endpoint, in one confined process. The
kernel finds the RTC in the device tree by `compatible`, allocates one frame for the clock page and
hands the service the registers, the page and an endpoint. It does not read the clock or know the
time. One portable binary carries both RTC drivers and the machine says which it has, so every
architecture runs the same test. See [the clock](clock.md).

## The entropy service

A virtio-rng device, its DMA page, its interrupt and the request endpoint clients hold. The kernel
never reads the device and holds no entropy of its own. The authority split is one sentence: the
service holds the device, and a client holds an endpoint that means "you may obtain randomness". A
client cannot program the queue, map the page the device writes into, or ask for anything the
service did not ask on its behalf. See [entropy](entropy.md).

## The credential service

| process | slot 0 | what that means |
|---|---|---|
| `credentialer` | the provision endpoint (READ) and the verify endpoint (READ, slot 1) | holds the store |
| `credentialer_test_client` provisioner | the provision endpoint (WRITE) | may write the store, until the seal |
| `credentialer_test_client` client | the verify endpoint (WRITE) | may ask a question about the store |
| `credentialer_test_client` attacker | the verify endpoint (WRITE) | the identical endowment, used otherwise |

The kernel never sees a secret, holds no store and computes no hash. It creates two endpoints, two
frames and a budget, and hands each process a different subset. Two frames and not one, because the
provisioner writes plaintext secrets into its page and a client sharing that frame would read them.
Argon2id is arithmetic on `u64`s, so every architecture runs the same binaries. See
[credentials](credentials.md).

## The NTP client

The client gets five slots: a report endpoint, the socket contract's endpoint, a budget, the clock
service's propose endpoint and the entropy service's endpoint. It gets no mapping of the clock page
in either direction, which is the whole difference between it and a Unix `ntpd` running as root. The
test server holds `READ` on the endpoint the client holds `WRITE` on: substituting the peer at a
capability boundary is how a capability system tests a client, and the client cannot tell. See
[NTP](ntp.md).

## The install offer

The offer a booted stick makes is `x86_64` only, and the gap is the loader's. It needs a copy of the
file this machine booted from, `memory::boot_file_region`. That is absent wherever `uefi_loader`
cannot hand over a second module, and a device-tree handoff has one initial ramdisk slot. It also
needs a console it can read a line back from, `console::read_line`, and the aarch64 PL011 driver
has no receive path.
It is a `cfg` rather than a body that declines on two architectures, because a module that compiled
everywhere and could only decline on two of three would read as portable. See
[installing](installing.md).
