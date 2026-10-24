# The e1000e system tests: why the lease is the confinement test

An appendix to [notes/e1000e.md](../e1000e.md). The doc comments in
`system_tests/src/user/e1000e_tests.rs` state what each gate proves. This page holds why the DHCP
test carries the confinement claim.

*Name: provisional, named for its content. calef names files.*

The lease test asserts two things together. The address lands in slirp's 10.0.2.0/24, which only a
real DHCP exchange through the device can produce. And the IOMMU unit that owns the NIC was
translating while it happened. On this device the queue pages carry the ring base registers, so
the driver at EL0 can point the rings anywhere. The IOMMU is therefore the whole of the DMA
confinement. A pass without it would prove the driver works and say nothing about the claim.
