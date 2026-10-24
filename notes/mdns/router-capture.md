# mDNS: the measured reference, captured 2026-08-15

*Name: provisional.*

An appendix to [`notes/mdns.md`](../mdns.md). The code it describes was retired on 2026-09-15; read
it in the past tense.

All three service types were captured from calef's router (GL.iNet GL-BE9300 running OpenWrt,
Samba with `vfs_fruit`, the working family Time Machine server) by sending one-shot PTR queries
from the dev machine. The router answered from `192.168.8.1:5353`; the full hex is in
`crates/multicast_dns_protocol/src/tests.rs` as the primary test vectors. Decoded:

| Record | `_smb._tcp` | `_adisk._tcp` | `_device-info._tcp` |
|---|---|---|---|
| PTR target | `GL-BE9300.<type>.local` | same | same |
| SRV port | **445** | **0** | **0** |
| SRV target | `GL-BE9300.local` | same | same |
| TXT | one empty string | `dk0=adVN=alice,adVF=0x82`, `dk1=adVN=bob,adVF=0x82` (user names replaced by placeholders), `sys=adVF=0x100` | `model=MacSamba` |

Plus an A record (`192.168.8.1`) and an AAAA in every response. Findings that beat the documentation:

- One `_adisk` instance for all shares. The disks are `dkN=` entries inside a single TXT record.
  Emitting one announcement per share would be wrong.
- Two disks is correct. Of the three users the roadmap block names, one is on Windows and has no
  share, so the reference advertises two (calef confirmed, 2026-08-15).
- `model=MacSamba`, not `TimeCapsule`. The router's own Samba config sets
  `fruit:model = TimeCapsule`, and its mDNS advertisement says `MacSamba` anyway: the SMB-side AAPL
  model and the `_device-info` TXT are separate knobs, and the working reference runs with them
  disagreeing. Whatever `fruit:model` buys, it is not this record. The crate therefore takes the
  model as data.
- `_adisk` and `_device-info` advertise SRV port 0. They carry flags, not a connectable service.
- Legacy unicast shape confirmed (RFC 6762 section 6.7). Our queries came from an ephemeral port, and
  the router echoed the ID, included the question, put all five records in the answer section, set
  no cache-flush bits, and capped every TTL at 10.

The flag values are copied as measured; the meaning of the `adVF` bits is not decoded here, and
does not need to be until something wants to emit different ones.
