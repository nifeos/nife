# The three faults the prompt walk exposed

*Name: provisional, for the parent's list of the walk's three faults. calef names files.*

An appendix to [`../ripgrep-on-nife.md`](../ripgrep-on-nife.md), holding its "The walk at the
prompt". The walk ran on 2026-10-07 (UTC) for milestone 121 (`ripgrep` on nife: enumeration as a
capability, and what the walk costs).

The walk exposed three faults, each fixed in the same change:

- The 16 MiB image could not hold the copy: RedoxFS spends a 4 KiB node on each of 936 entries.
  swish-check's image is now 32 MiB (`SWISH_CHECK_DISK_MIB`); every other image stays 16.
- The shell grants a word only below `/`, so the line runs after `cd /search`.
- The gate's transcript reader decoded each pipe read alone, so a `§` split across two reads
  became two replacement characters, the one line of 349 that differed. `whole_characters` holds
  the partial character for the next read, and a test splits one.
