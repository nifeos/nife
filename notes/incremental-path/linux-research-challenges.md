# What Linux's own design costs the incremental path

An appendix of [the incremental path](../incremental-path.md). The parent keeps the paper's argument
and its four steps. This file keeps the paper's research challenges, one per step,
which the parent's answer leans on. Quotations are from the paper's PDF, listed in
[sources.md](sources.md).

## The challenges, one per step

The research challenges are honest about what Linux's own design costs them. This is the section
that a hostile reader would skip and should not.

- Modularity. *"Because of its focus on performance, Linux often lacks strict module boundaries
  between components."* The worked case is TCP state, *"found throughout generic socket code and
  data structures"*.
- Type safety. The VFS `write_begin`/`write_end` pair passes `void` pointers, and error values cast
  to pointers.
- Ownership. The generic `inode`'s `i_size` field *"is only maybe protected, according to the
  relevant comment"*. Its synchronization requirements *"are different depending on the function
  in the file system"*.
- Functional correctness. `buffer_head` has *"16 state flags"* whose valid combinations are hard
  even to enumerate. And the paper admits candidly that *"it is almost impossible to know the right
  interfaces for composed verification before anything is built."*
