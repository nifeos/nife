/* The ncurses screen test the vt replay harness records (milestone 705's step 0a).
 *
 * The census's ncurses test was a throwaway program that is not kept; this is the replacement,
 * drawing the same subjects on one screen: a box and its line-drawing characters, 256-colour
 * fills, a scrolling window, insert and delete in place, and a flash. It draws once, waits for
 * one key, and exits on it, so the session's one checkpoint is the drawn screen. Record it with:
 *
 *     cargo xtask vt-replay --record ncurses-test
 *
 * which compiles this file against Homebrew's ncursesw 6.6 (NIFE_NCURSES_HOME overrides the
 * prefix). Name provisional.
 */

#include <ncurses.h>

int main(void)
{
    initscr();
    start_color();
    use_default_colors();
    cbreak();
    noecho();

    /* Four fills from the 256-colour cube and ramp, the palette work of the plan's step 8. */
    for (short i = 0; i < 4; i++)
        init_pair((short)(i + 1), COLOR_WHITE, (short)(16 + i * 73));
    for (short i = 0; i < 4; i++) {
        attron(COLOR_PAIR((short)(i + 1)));
        mvhline(1 + i, 2, ' ', 20);
        attroff(COLOR_PAIR((short)(i + 1)));
    }

    /* The box and its line-drawing characters, the plan's step 5. */
    WINDOW *frame = newwin(12, 34, 0, 26);
    box(frame, ACS_VLINE, ACS_HLINE);
    mvwaddstr(frame, 1, 2, "line drawing: ");
    waddch(frame, ACS_ULCORNER);
    waddch(frame, ACS_LTEE);
    waddch(frame, ACS_HLINE);
    waddch(frame, ACS_RTEE);
    waddch(frame, ACS_URCORNER);
    mvwaddstr(frame, 2, 2, "0123456789");
    wnoutrefresh(frame);

    /* A five-row scrolling window: four rows written in, then four more that push them up. */
    WINDOW *scroll = newwin(5, 34, 13, 26);
    scrollok(scroll, TRUE);
    for (int i = 0; i < 4; i++)
        mvwprintw(scroll, i, 1, "scrolled row %d", i);
    for (int i = 4; i < 8; i++) {
        wscrl(scroll, 1);
        mvwprintw(scroll, 4, 1, "scrolled row %d", i);
    }
    wnoutrefresh(scroll);

    /* Insert and delete in place, the plan's step 3: each line reads ABD with B in the middle. */
    mvaddstr(19, 2, "insert: ACD");
    move(19, 11);
    insch('B');
    mvaddstr(20, 2, "delete: ABXD");
    move(20, 12);
    delch();

    mvaddstr(21, 2, "any key exits");
    refresh();
    flash();
    getch();
    endwin();
    return 0;
}
