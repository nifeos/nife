# `swish` the language: the appendix to notes/swish-language.md

[`notes/swish-language.md`](../swish-language.md) carries quoting, sequencing, exit status, the
examples, what the guest test proves in brief, and BUGS. This directory carries the detail its prose
budget has no room for.

- [`the-guest-test.md`](the-guest-test.md): the wiring the guest test shares, why it shares it,
  and its four pairs of assertions.
- [`quote-state-machine.md`](quote-state-machine.md): where the quote state machine lives, and why
  the three scanners share it and not a tokenizer.
- [`running.md`](running.md): how the shell turns a planned line into spawn messages, the grants
  it can and cannot deliver, and the drains.
- [`witness.md`](witness.md): the shell's witness scripts, and why each line is there.

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*
