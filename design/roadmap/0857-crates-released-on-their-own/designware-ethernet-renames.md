# The renames in `designware_ethernet` before ratification

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md),
beside the crate's names record, [designware-ethernet-names.md](designware-ethernet-names.md). Each
ruling below renamed names in the crate before it publishes; the record's row for each new name
refuses its old spelling, and the crate's `Name:` line for it names the ruling.

*Name: provisional. Named for the record it sits beside.*

- Ruled 2026-10-26 (UTC), the crate's rename table: `Hw` becomes `Hardware`. `Mdio` and the module
  `mdio` become `ManagementDataInputOutput` and `management_data_input_output`, IEEE 802.3's phrase,
  with `doc(alias = "mdio")`. `Syscon`, `SYSCON_FIELD` and the `GMAC0_SYSCON_*` constants spell out
  `SystemController` and `SYSTEM_CONTROLLER_*`, with `doc(alias = "syscon")`; device-tree strings
  keep their spelling. `read_ext`, `write_ext`, `modify_ext` and `ext` become `read_extended`,
  `write_extended`, `modify_extended` and `extended`. `desc` and `set_desc` become `descriptor` and
  `set_descriptor`; `rx_cur` and `tx_cur`, `receive_cursor` and `transmit_cursor`; `rx_des3` and
  `tx_des3`, `receive_descriptor_word_3` and `transmit_descriptor_word_3`; `dev_read` and
  `dev_write`, `device_read` and `device_write`; `volt_mv`, `voltage_mv`. The simulator's `Nic`
  becomes `NetworkInterfaceController`, a `*_LEN` this crate authors becomes `*_LENGTH`, and
  `MAX_FRAME` becomes `MAXIMUM_FRAME`. The unit symbols `ps`, `mv`, `ua`, `us` and `ms` stay.
- Ruled 2026-10-26 (UTC), earned words: a `mac` that holds the six-byte address is a
  `station_address`, IEEE 802.3's term and the databook's, with `doc(alias = "mac_address")`
  (`Discovered`'s field, the handoff's field, and every parameter). `crc` earns its place and stays, as
  `cpu` does by precedent. A FIFO is a queue, matching the RQS and TQS fields that size it:
  `rx_fifo_bytes`, `tx_fifo_bytes` and `fifo_bytes` become `receive_queue_bytes`,
  `transmit_queue_bytes` and `queue_bytes`, with `doc(alias = "fifo")`.
- Ruled 2026-10-26 (UTC), terms of art: `rgmii` spells out as
  `reduced_gigabit_media_independent_interface` (`select_rgmii`, `Discovered`'s field `rgmii`,
  and the tests that name them), with
  `doc(alias = "rgmii")` and a doc that says what it is; register names copied from a source keep
  `RGMII`. The programmable burst length spells out by what it sets: `rx_pbl`, `tx_pbl` and
  `pbl_x8` become `receive_channel_burst_length`, `transmit_channel_burst_length` and
  `channel_burst_length_times_8`, with `doc(alias = "pbl")`. `burst_lengths`, the `BLEN` bits, a
  different setting, becomes `bus_burst_lengths`.
- Ruled 2026-10-26 (UTC), tree-wide: `rx` and `tx` spell out wherever this crate authors the
  name (`ReceiveVerdict`, `TransmitError`, `receive_verdict`, `RECEIVE_ENTRIES`, the `layout`
  offsets, the `Tails` methods, `DataPlane::receive_cursor`, `Settings`' fields). Register and bit
  names copied from a source (`CHAN_RX_CONTROL`, `TXFIFOSIZE`, `RX_CLK_EN`) keep their spelling.
- Ruled 2026-10-26 (UTC), tree-wide: `phy`, the chip, spells out as `physical_layer_device`, IEEE
  802.3's phrase, wherever this crate authors the name (`mainline_physical_layer_device`,
  `vendor_physical_layer_device`, `VENDOR_PHYSICAL_LAYER_DEVICE_NODE`, `physical_layer_device_id`,
  `Discovered`'s fields, the `Timeout` field and every parameter), with `doc(alias = "phy")`. The
  docs keep "PHY". Register names (`PHYSID1`, `PHY_ID_YT8531`) and device-tree strings keep their
  spelling.
