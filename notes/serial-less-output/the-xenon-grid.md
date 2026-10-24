# The grid on xenon's screen

*Name: provisional, for the finding it holds.*

On the one real machine that has run `screen_console`, xenon, the screen shows a regular grid
instead of text. It was seen on 2026-09-04 and again on 2026-09-17. The cause is in the painting
path. It is not scrolling, and it is not the halt path.

## What the tour run rules out

The discriminating run was the tour image on 2026-09-17
(`bench/xenon-2026-09-17/tour-display-225100.log`, video `IMG_4145`). It printed about 40 lines onto
a 135-row screen, so nothing scrolled. It ended at a live shell rather than a panic, so the machine
was not halted. The grid appeared anyway.

## What the kernel's geometry says

The geometry is not wrong. That boot reported `8294400` bytes of framebuffer, exactly
1920 x 1080 x 4. So `Framebuffer::span` and a 7680-byte stride agree with each other and with the
panel.

## The hypothesis, and why it is only one

The screen shows single pixels at regular vertical intervals, in columns at regular horizontal
intervals, and the columns lean. A constant lean is the signature of a fixed per-row drift. The
address this crate computes for row *n* would differ from the address the display scans out for
row *n*. A pitch disagreement produces exactly that.

This is read off a photograph. It is not a measurement, and counting columns in a JPEG is not
evidence this tree accepts.

## The experiment that would settle it

It needs no bench time to write. Paint one horizontal run of known length at a known row before
printing anything. A correct pitch shows one line. A mismatched pitch shows a line that steps or
wraps, and the step is the delta.
