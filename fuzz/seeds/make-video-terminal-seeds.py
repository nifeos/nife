#!/usr/bin/env python3
"""Write the seeds for the `video_terminal_feed` fuzz target.

Every seed starts with the target's three header bytes (grid size, first cut, second cut), then the
stream. The hand-written ones are small and name one family of sequences each. Two more are the
output halves of recordings in crates/video_terminal/recordings (the NVTR format their README
describes), so the fuzzer starts from what real full-screen programs write. Run from the repository
root: python3 fuzz/seeds/make-video-terminal-seeds.py
"""
import pathlib
import struct

out = pathlib.Path("fuzz/seeds/video_terminal_feed")
out.mkdir(parents=True, exist_ok=True)
ESC = b"\x1b"
CSI = ESC + b"["

# size index (see SIZES in the target): 5 is 80x24, 0 is 1x1, 3 is 2x2.
hand = {
    "colors": (5, 85, 170, CSI + b"0m" + CSI + b"1;31mred" + CSI + b"0;32m green" + CSI + b"44m blue bg"
               + CSI + b"38;5;208morange" + CSI + b"38;2;10;200;30mtruecolor" + CSI + b"48;2;1;2;3m"
               + CSI + b"4:3m curly" + CSI + b"7m inverse" + CSI + b"0m\r\n"),
    "cursor-movement": (3, 60, 200, b"abc" + CSI + b"2A" + CSI + b"5C" + CSI + b"3B" + CSI + b"2D"
                        + CSI + b"10;20H" + CSI + b"H" + CSI + b"s" + CSI + b"u" + b"\x08\x08\t\t\r\n\n"
                        + CSI + b"0K" + CSI + b"1K" + CSI + b"2K" + CSI + b"J" + CSI + b"1J" + CSI + b"2J"),
    "alternate-screen": (5, 30, 128, CSI + b"?1049h" + CSI + b"2J" + CSI + b"H" + b"alternate screen"
                         + CSI + b"?25l" + CSI + b"?1049l" + CSI + b"?25h" + CSI + b"?7l" + CSI + b"?7h"),
    "scroll-region": (3, 100, 140, CSI + b"2;5r" + CSI + b"5;1H" + b"a\nb\nc\n" + CSI + b"2L" + CSI + b"M"
                      + CSI + b"3S" + CSI + b"2T" + CSI + b"r" + ESC + b"M" + ESC + b"D" + ESC + b"E"
                      + ESC + b"7" + ESC + b"8" + ESC + b"c"),
    "utf8": (0, 3, 4, "héllo 世界 \U0001f600 €".encode() + b"\xff\xc0\xe2\x82" + b"\xf0\x9f"),
    "strings": (5, 40, 90, ESC + b"]0;a window title" + b"\x07" + ESC + b"]8;;http://example.com" + ESC + b"\\link"
                + ESC + b"P1$r0m" + ESC + b"\\" + ESC + b"_apc" + ESC + b"\\" + ESC + b"^pm" + b"\x18after"),
    # The shape ripgrep prints with --color=always: path, line number and match styled apart.
    "ripgrep-style": (5, 120, 200,
                      CSI + b"0m" + CSI + b"1m" + CSI + b"35msrc/main.rs" + CSI + b"0m\n"
                      + CSI + b"0m" + CSI + b"32m12" + CSI + b"0m:    let " + CSI + b"0m" + CSI + b"1m"
                      + CSI + b"31mneedle" + CSI + b"0m = haystack.find(\"x\");\n"
                      + CSI + b"0m" + CSI + b"32m40" + CSI + b"0m-" + CSI + b"0m" + CSI + b"1m" + CSI + b"31mneedle"
                      + CSI + b"0m\n\n"),
    "tiny-grid-flood": (0, 1, 2, (b"x" * 40) + b"\n" * 5 + CSI + b"99;99H" + CSI + b"99999999999A" + CSI + b"1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16;17;18m"),
}
for name, (size, a, b, body) in hand.items():
    (out / f"{name}.bin").write_bytes(bytes([size, a, b]) + body)


def output_events(path):
    data = pathlib.Path(path).read_bytes()
    assert data[:4] == b"NVTR"
    at, chunks = 6, []
    while at < len(data):
        kind, length = data[at], struct.unpack_from("<I", data, at + 1)[0]
        at += 5
        if kind == 0:
            chunks.append(data[at : at + length])
        at += length
    return b"".join(chunks)


for name, limit in (("tmux", 4096), ("vttest-1", 6144)):
    body = output_events(f"crates/video_terminal/recordings/{name}.rec")[:limit]
    (out / f"recording-{name}.bin").write_bytes(bytes([5, 90, 200]) + body)
