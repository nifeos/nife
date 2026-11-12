# Done when

An appendix to [the milestone](../1107-sealed-pair-outside-review.md).

- `inspect` or a sibling reads the loader's two words. `Seal` carries their verdict, and `is_sealed`
  includes it. `card_check` reads `nife-initial-ramdisk.img.loader-digests` beside the two images
  and says the set is three files. One test builds a card with stale words and sees it refused.
- A table with one bad line beside a good console line reports the bad line and the program it
  named, and does not name the console. `TableSeal::Unreadable` stays for bytes that fail UTF-8.
- `BOOT_PROGRAMS` is `[measured_boot::PROGENITOR]`.
- `Vouch` carries its digest, and no `Seal` can say "refused" with no digest.
- The refusal text states what the tool knows and names causes as causes.
- `Unreadable` implements `std::error::Error` and prints every `nifefs::Error` in words.
- `british_spelling.py`, `spelled_out.py`, `unit_symbols.py` and `standalone.py` from the template
  pass over the crate, the last one in the repository that receives it.
- `notes/boot-stick.md` names the two measured entries.
- The census row stays at none missed.
- Decision A appears in journey 5's table or in milestone 1017 (the contract crates leave in this order)'s order, and no repository
  exists for the crate before the two it builds on have theirs.
