# The boot stick at argon's bench

An appendix to [`notes/boot-stick.md`](../boot-stick.md), holding its argon bench check. Nothing
here has been run.

## argon (Jetson TX1)

The aarch64 boot file cannot run nife on argon yet, and the check is still worth making. The
aarch64 kernel is linked at `0x4008_0000`, which is QEMU `virt`'s RAM; argon's RAM starts at
`0x8000_0000`, so the loader will refuse to place the kernel and print what the firmware's map has
there. That is milestone 127's port to do, for `booti` as much as for this. What the bench learns
now is whether L4T's U-Boot runs our loader and hands it a device tree, which is U1's premise:

```
Tegra210 (P2371-2180) # usb start
Tegra210 (P2371-2180) # load usb 0:1 ${kernel_addr_r} /EFI/BOOT/BOOTAA64.EFI
Tegra210 (P2371-2180) # bootefi ${kernel_addr_r} ${fdtcontroladdr}
```

| What argon prints | What it means |
|---|---|
| `nife uefi_loader: milestone 87`, then `uefi_loader: wanted 0x0000000040080000..` | **The expected result**: bootefi ran our loader, the tree was offered, and the kernel's load address is the only thing in the way. U1's premise holds on argon |
| `uefi_loader: the firmware offers no device tree` | L4T's `bootefi` was not given a tree; retry with the tree's address as the second argument, from `printenv fdt_addr_r` or `fdtcontroladdr` |
| `Unknown command 'bootefi'` | argon takes U2 until its U-Boot is newer |

(The prompt text is Tegra's recalled default, not read off argon.)
