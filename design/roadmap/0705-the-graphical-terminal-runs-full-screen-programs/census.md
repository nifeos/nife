# Part 2: What the three programs need that is missing

This appendix belongs to [milestone 705 (the graphical terminal runs full-screen programs)](../0705-the-graphical-terminal-runs-full-screen-programs.md); it holds the escape-sequence census and the replay measurement.

Escape-sequence census, taken on the host (macOS, `TERM=xterm`, 80x24, vim 9.1, less from the OS). The
harness answered `ESC[6n` and `ESC[>c` like an xterm would.

| program | session | sequences it sent that `Vt` does not act on |
|---|---|---|
| vim | open, `:split`, scroll, `:q!` | `?1049h/l` (x1 each), `r` DECSTBM (x9, `1;24` and `1;11`), `>c` (x1), `6n` (x2), `?1h` + `ESC =`, `t` (x4), `?2004h`, `?1004h`, `>4;2m` |
| less | open, page, `b`, `k`, `q` | `ESC M` reverse index (x24), `?1049h/l` (x1 each), `?1h` + `ESC =` |

The inventory, each with how a program uses it.

- DECSTBM scrolling region (`CSI r`). vim sets `scroll_region = TRUE` whenever terminfo has `csr`
  (`src/term.c:3499-3503`) and emits `CSI 1;11 r` to scroll one window of a split (captured). `xterm`
  and `vt100` terminfo both define `csr=\E[%i%p1%d;%p2%dr`. Today a line feed always scrolls the whole
  screen (`line_feed`, `lib.rs:1328`).
- Reverse index (`ESC M`, terminfo `ri`). `less` scrolls backward with `ri` or `il1`, whichever is
  cheaper (`screen.c:1730-1745`, `sc_addline`), and only falls back to a repaint when both are absent.
  `ESC M` falls into the swallow-everything arm (`lib.rs:1386-1390`). Measured: after `space space b`
  and after `G k`, 22 of 24 rows differ from the oracle, and both repeat. This is the one gap I saw
  break a real program deterministically.
- Alternate screen (`?1049`, `?47`, `?1047`). Both programs enter with `smcup=\E[?1049h` and leave
  with `rmcup`. Without it the program works and leaves its last screen in the user's scrollback and
  the prompt under it. It needs a second grid, a saved cursor and a clear on entry.
- Cursor position report (DSR 6, `CSI 6 n`). vim sends `u7=\E[6n` in `check_terminal_behavior`
  (`term.c:4238-4245`) to learn ambiguous-width behavior. Whether it waits for a reply, and for how long, I did not
  measure (the harness always answered). real `kilo` sends it only when
  the window size is otherwise unavailable (from memory).
- Device attributes (`CSI c`, `CSI > c`). vim sends `ESC[>c` (`term.c:528`, `4226-4230`) to
  identify the terminal and pick feature sets; `xterm` terminfo carries `u8`/`u9` for the same query.
  Without a reply it assumes less.
- Insert and delete, and cursor addressing. `il dl ich dch ech` (`CSI L M @ P X`), `hpa`/`vpa`
  (`CSI G`, `CSI d`), scroll by n (`CSI S`, `CSI T`), and `sc`/`rc` (`ESC 7`, `ESC 8`) are all in the
  `xterm` entry and all swallowed today.
- Small mode switches. DECTCEM (`?25`, cursor hide), DECAWM (`?7`, procps `top` toggles it with
  `rmam`/`smam`, `src/top/top.c:173-174`), DECCKM and `ESC =` (`smkx`, application cursor keys).
  The last matters on the input side: with `?1h` set a real terminal sends `ESC O A` for Up, and
  `video_terminal::keymap` always sends `CSI A`. vim accepts both; whether the others do I did not check.

`top` is the nearest to working. procps `top` uses `clear`, `ed`, `el`, `cup`, `home`, reverse,
cursor hide and show, and autowrap off (`top.c:160-179`, `801-817`); it enters no alternate screen. Its
gap is DECAWM, which only matters when a line is wider than the grid.

What the replay measured. Each session was replayed through `Vt` and its final 24 rows compared
with `tmux capture-pane`. vim, eight scenarios (open, `G`, `ggdd`, Ctrl-E, `5dd`, and split-window
variants): every run matched except the split-window `Ctrl-E x3, Ctrl-Y` case, which differed in 10 of 24
rows in two of three runs. less: opening and paging forward matched; backward scroll (`b`, and `G`
then `k`) differed in every run, 22 of 24 rows. The harness is timing-sensitive (keys land 0.8 s apart
against the oracle's 0.6 s), and I did not find out why vim agreed as often as it did, so read it as
"the failures are real, the agreement is not a guarantee". The throwaway harness lives in the lane's scratch directory and is not committed.
