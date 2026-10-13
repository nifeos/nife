# Where the quote state machine lives, and why there is one of it

An appendix to [`../swish-language.md`](../swish-language.md).

Three scanners need to know whether a byte is quoted and none of them can share a tokenizer:
`grant_plan::tokenize` splits on whitespace, `grant_plan::line::split` splits on `>`, `<` and `|`,
and `swish::sequence::split` splits on `;`, `&&` and `||`. What they have in common is a two-bit
state machine, so that is what is shared: `grant_plan::word::Cursor`, whose `step` answers "is this
byte bare".

The other half is `word::read`, which takes the quotes off one whole token. Keeping the two apart is
what lets one place refuse `a"b"` for every scanner: the tokenizer's job is where a token *ends* and
`read`'s is what it *means*.
