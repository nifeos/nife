# The pages beside notes/crate-reviews.md

[`notes/crate-reviews.md`](../crate-reviews.md) is the index: what every package ran, what is the
same for all of them, and one row per crate. Each crate's package is a file here, and a new package
is a new file plus a row in the index, never a section in it, because every lane that writes one
would otherwise add to the same page.

- [`file-allocation-table-public-api.md`](file-allocation-table-public-api.md): every public item
  of `file_allocation_table` and its signature, beside its package.
- Every other file: one crate's package, with its names, proposed items and open questions.

*Name: provisional, minted by lane milestone/813-review-package. The directory follows §212 (a
prose budget)'s siting default, `notes/X/` beside `notes/X.md`.*
