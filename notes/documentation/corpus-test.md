# What the corpus test caught, and what it could not see

An appendix to [`notes/documentation.md`](../documentation.md), which states the lesson.
`every_character_survives` checks that every letter and digit of the corpus reaches the rendered
output, in order.

## A dropped image destination, which it caught

A link and an image both show their destination, because a terminal cannot follow one or draw
the other, so the path is the only thing a reader can act on. Rendered as `[image: alt text]`
alone, an image names a picture the reader has no way to find. `every_character_survives` is a
subsequence check over the source, so a dropped destination is exactly what it exists to catch, and
notes/project-metrics.md, which carries an image, trips it if the path goes missing.

## The bug the corpus test could not see, and the honest filter that kept it findable

A fenced code block opened inside a block quote never closed, from phase 1 until 2026-08-18. The
closing test ran against the raw line, so this:

> A transcript quoted from somewhere else, with its own fence inside the quote:
>
> ```text
> $ mdr glob.md | wc
>   1 4 26
> ```
>
> and prose after it, which used to render as code.

left the renderer in code mode to the end of the document. One quoted transcript misrendered every
line after it.

`every_character_survives` could not find it, and the reason is worth keeping: verbatim output
loses no characters, so a page rendered entirely as code still passes a subsequence check. What the
test *did* see was the opening fence's info string going missing, because its filter excluded a
fence by looking for backticks after `trim_start` and a quoted fence does not start with one. That
is the renderer being right about one line inside a page it was then ruining.

The filter was left wrong on purpose, with a `BUGS` comment saying why. Widening it would have
silenced one false failure by hiding three hundred misrendered lines, and nobody had answered
whether the renderer kept its quote state across a nested fence. It did not. So the answer came
first and the filter second, which is the order that entry existed to enforce.

And the corpus test still cannot guard it, which was measured rather than assumed. Reverting the
fix leaves this page ruined from the block above onward, and `every_character_survives` passes.
Verbatim output loses no characters, and `Renderer::has_unclosed_fence` (added here, and the
strongest thing the corpus check can assert) misses it too, because a bare closing fence three
sections later matches the stuck one and lets the renderer out. A unit test is the guard. The lesson
generalizes past this bug: a subsequence check proves nothing was dropped and nothing about what
was ruined, so a renderer wants both kinds of test and this one had only the first.
