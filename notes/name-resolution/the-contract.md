# The resolver's contract: three parties, one endpoint

An appendix to [Name resolution](../name-resolution.md). `crates/domain_name_resolution_contract`
holds what the resolver, its clients and their spawner agree on, under §248 (the name resolver is
its own confined program). Three parties share the resolver's endpoint, each with a different
capability to it:

```text
  spawner ──unbadged WRITE──┐          (GRANT: badge B may resolve names in zone Z)
                            ▼
  client ──WRITE, badge B──► domain_name_resolver ──Stack──► net_stack ──► a name server
  (ATTACH a page, then RESOLVE)    │
                                   └──entropy──► the transaction id
```

- The spawner holds the one unbadged capability and mints a badged copy per client. It tells the
  resolver what each badge may resolve with `OPERATION_GRANT` (`grant_messages`). Only badge 0 is
  read as a grant, so a client cannot widen its own.
- A client delegates one page (`OPERATION_ATTACH_PAGE_FRAME`), writes a host name into it, and
  `CALL`s `OPERATION_RESOLVE`. The reply is a `status` (`Outcome`) and the TTL, and the addresses
  are in the page.
- The resolver keeps a `Grants` table and judges each name against its caller's zone before it
  sends anything.

A grant is a zone, compared label by label (`domain_name_system::Name::is_within`). Granted
`nife.test`, a client may ask about `packages.nife.test` and not `evilnife.test`. One host name is
an exact grant, and the empty zone is any name. The grant applies to the name asked, not to the
CNAME chain the answer follows.
