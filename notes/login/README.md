# Login: the appendices to notes/login.md

[`notes/login.md`](../login.md) carries the design: authentication that hands back capabilities,
the caretaker per login, reclaiming a session, the terminal, and measured boot. These files carry
the detail its prose budget has no room for.

- [`budgets-and-reclamation.md`](budgets-and-reclamation.md): the budgets `components/src/login.rs`
  splits at start-up and why each has its own parent, what it takes to give a channel or a region
  back completely, the capability-table ordering, and the measurements behind each number.

*Name: provisional, minted 2026-10-24 (UTC) by the milestone 929 (comments in the Linux style across first-party Rust) lane
comments/929-components-02.
The directory follows §212 (a prose budget)'s siting default, `notes/X/` beside `notes/X.md`.
calef names directories.*
