#!/usr/bin/env python3
"""results.py: read a jh7110_entropy board-check log, fill in the results table, and judge it.

    results.py summarize <log> [--capture <out.bin>]   # the filled results table, on stdout
    results.py check [--allow-model] <results.md>      # judge a filled table against its criteria
    results.py fill <results.md> <row> <value>          # set a row the person fills, in place
    results.py template                                # the empty table (results-template.md)
    results.py --selftest                              # what this file promises, on synthetic logs

`summarize` keeps only `board-check:` records whose CRC-32 matches (the payload's console.rs writes
them), takes the last run in the log, extracts the capture, runs the statistics on it and prints
the table with every value and verdict filled in. Two rows are the person's to fill: how the board
came up before the run (R4) and any outside statistical suite (9g). `fill` sets one of them.

`check` reads the values back out of a table, recomputes every verdict from them, and exits 1 when
a row fails, a row is blank, a verdict in the table disagrees with the recomputed one, or the run
was the QEMU model (unless --allow-model). A verdict is PASS or FAIL against the row's criterion,
or RECORDED for a row that answers a question rather than meeting a bar.

The statistics are a measurement, never a proof: the FIPS 140-2 power-up battery over 20,000-bit
blocks (as rngtest runs it), Shannon entropy per byte, a chi-square over byte values, the mean byte,
the lag-1 serial correlation, and repeated 32-byte generations. See
notes/jh7110-entropy-board-check.md.

Name: provisional.
"""

import hashlib
import itertools
import math
import os
import re
import sys
import tempfile
import zlib

FORMAT = 1
RECORD = re.compile(r"board-check: (.*) \*([0-9a-f]{8})$")
MEBIBYTE = 1 << 20
# STAT bits 19 to 23, just above LAST_RESEED's three bits: one set would say the field is wider.
STATUS_ABOVE_LAST_RESEED = 0x00F8_0000
INTERRUPT_STATUS_DOCUMENTED = 0x1F


def parse_log(text):
    """The verified records of the last run in `text`, the count of damaged ones, and whether the
    board's boot script reported fetching the payload over the network."""
    runs, damaged, came_from_net = [], 0, None
    for raw in text.split("\n"):
        line = raw.rstrip("\r")
        if "nife: payload came from " in line:
            came_from_net = line.rsplit(" ", 1)[-1].strip()
        at = line.find("board-check: ")
        if at < 0:
            continue
        match = RECORD.match(line[at:])
        if not match or zlib.crc32(match.group(1).encode("latin-1")) != int(match.group(2), 16):
            damaged += 1
            continue
        payload = match.group(1)
        if payload.startswith("begin "):
            runs.append([])
        if runs:
            runs[-1].append(payload)
    return (runs[-1] if runs else []), damaged, came_from_net


def fields(payload):
    """`kind`, then a dictionary of its key=value tokens."""
    kind, *rest = payload.split(" ")
    return kind, dict(token.split("=", 1) for token in rest if "=" in token)


def statistics(data):
    """The capture's statistics, as a dictionary of numbers."""
    n = len(data)
    result = {"bytes": n}
    if n < 2:
        return result
    counts = [0] * 256
    for byte in data:
        counts[byte] += 1
    result["entropy"] = -sum(c / n * math.log2(c / n) for c in counts if c)
    expected = n / 256
    chi = sum((c - expected) ** 2 / expected for c in counts)
    result["chi_square"] = chi
    k = 255  # Wilson and Hilferty's normal approximation to the chi-square upper tail
    z = ((chi / k) ** (1 / 3) - (1 - 2 / (9 * k))) / math.sqrt(2 / (9 * k))
    result["chi_square_p"] = 0.5 * math.erfc(z / math.sqrt(2))
    result["mean"] = sum(i * c for i, c in enumerate(counts)) / n
    xs, ys = data[:-1], data[1:]
    mx, my = sum(xs) / len(xs), sum(ys) / len(ys)
    cov = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    vx = sum((x - mx) ** 2 for x in xs)
    vy = sum((y - my) ** 2 for y in ys)
    result["serial_correlation"] = cov / math.sqrt(vx * vy) if vx and vy else 1.0
    blocks = [data[i:i + 32] for i in range(0, n - n % 32, 32)]
    result["repeated_generations"] = len(blocks) - len(set(blocks))
    blocks_tested = failed = 0
    for start in range(0, n - 2499, 2500):
        blocks_tested += 1
        failed += 0 if fips_block(data[start:start + 2500]) else 1
    result["fips_blocks"], result["fips_failed"] = blocks_tested, failed
    return result


# FIPS 140-2 section 4.9.1's run-length intervals, for runs of 1 to 5 and of 6 or more.
RUN_BOUNDS = [(2315, 2685), (1114, 1386), (527, 723), (240, 384), (103, 209), (103, 209)]


def fips_block(block):
    """Whether one 20,000-bit block passes the monobit, poker, runs and long-run tests."""
    bits = "".join(format(b, "08b") for b in block)
    ones = bits.count("1")
    if not 9725 < ones < 10275:
        return False
    nibbles = [0] * 16
    for b in block:
        nibbles[b >> 4] += 1
        nibbles[b & 15] += 1
    poker = 16 / 5000 * sum(f * f for f in nibbles) - 5000
    if not 2.16 < poker < 46.17:
        return False
    runs = {"0": [0] * 6, "1": [0] * 6}
    for value, group in itertools.groupby(bits):
        length = sum(1 for _ in group)
        if length >= 26:
            return False
        runs[value][min(length, 6) - 1] += 1
    return all(lo <= runs[v][i] <= hi for v in "01" for i, (lo, hi) in enumerate(RUN_BOUNDS))


def extract(records):
    """Every value the table reports, from one run's records, plus the capture bytes."""
    v, capture, seen = {}, bytearray(), set()
    for payload in records:
        kind, f = fields(payload)
        if kind == "capture":
            _, sequence, digits = payload.split(" ")
            if sequence not in seen:
                seen.add(sequence)
                capture += bytes.fromhex(digits)
            continue
        if kind == "begin":
            v.update(crate_commit=f.get("crate_commit"), model=f.get("model"),
                     format=f.get("format"), timebase_hertz=f.get("timebase_hertz"),
                     timebase_from_tree=f.get("timebase_from_tree"))
        elif kind == "window":
            v["window"] = f"{f.get('trng')} {f.get('compatible')} from_tree={f.get('from_tree')}"
        elif kind == "clock_step":
            if f.get("kind") == "deassert":
                v["reset_released"] = f.get("released")
            elif f.get("kind") == "enable":
                ok = int(f.get("after", "0"), 16) >> 31 if "after" in f else 0
                v["clocks_enabled"] = str(min(int(v.get("clocks_enabled", "1")), ok))
        elif kind == "clock_plan":
            v["reset_released"] = v["clocks_enabled"] = "model"
        elif kind == "end":
            v["complete"] = f.get("complete")
        elif kind == "first_reseed":
            v["first_reseed"] = f"done={f.get('done')} seeded={f.get('seeded')}"
            v["last_reseed_after_reseed"] = f.get("last_reseed")
        elif kind == "generation":
            v[f"generation_{f.get('index')}"] = f.get("outcome")
            v[f"bytes_{f.get('index')}"] = f.get("bytes", "")
        elif kind == "reset_state":
            for key in ("seeded", "last_reseed", "r256", "mode", "features", "bconf", "istat"):
                v[f"reset_{key}"] = f.get(key)
        elif kind == "bring_up":
            v["bring_up_residue"], v["r256_after"] = f.get("istat_residue"), f.get("r256")
        elif kind == "overlap":
            v[f"overlap_{f.get('scenario')}"] = " ".join(
                f"{key}={f.get(key)}" for key in ("together", "together_loose", "accepted",
                                                   "words_changed", "timeouts") if key in f)
        elif kind == "status_bits":
            v["unnamed_seen"] = f.get("unnamed_seen")
            v["status_seen"] = f.get("seen")
            v["last_reseed_values"] = f.get("last_reseed_values_seen")
        elif kind == "timing":
            v[f"timing_{f.get('kind')}"] = " ".join(
                f"{key}={f.get(key)}" for key in ("minimum", "median", "p99", "maximum",
                                                   "timeouts") if key in f)
        elif kind == "alarm":
            v[f"alarm_{f.get('kind')}"] = " ".join(
                f"{key}={f.get(key)}" for key in ("latched", "readback", "istat_seen",
                                                   "istat_residue") if key in f)
        elif kind == "long_run":
            v["long_run"] = " ".join(f"{key}={f[key]}" for key in (
                "generations", "ready", "all_zero", "lockups", "unseeded", "wrong_output_width",
                "timeouts"))
        elif kind == "lockup":
            v["lockup"] = " ".join(f"{key}={f.get(key)}" for key in (
                "bit_after_reseed", "bit_after_acknowledge", "next_outcome"))
    return v, bytes(capture)


def kv(text):
    """The key=value tokens of a table cell."""
    return dict(t.split("=", 1) for t in (text or "").split() if "=" in t)


def hexint(text):
    return int(text, 16)


def width_meaning(values):
    """Check 3f: which value of BCONF bit 3 means 256 bits, read off the reset width."""
    r256, bconf = int(values["3a"]), hexint(values["3d"])
    return "1 means 256-bit" if (bconf >> 3) & 1 == r256 else "0 means 256-bit"


def widest(values):
    """Check 3g: FEATURES.MAX_RAND_LENGTH and BCONF.MAX_PRNG_LEN, read off 3c and 3d."""
    features, bconf = hexint(values["3c"]), hexint(values["3d"])
    return f"max_rand_length={features & 3} max_prng_len={(bconf >> 2) & 1}"


def passes(condition):
    return "PASS" if condition else "FAIL"


def micro(values, row, key):
    hertz = int(values["R5"].split()[0])
    return round(int(kv(values[row])[key]) * 1e6 / hertz, 2)


def judge_capture(values, row, test):
    s = kv(values[row])
    return passes(test(s)) if s else "FAIL"


# Every row: id, what it measures, the criterion as a reader sees it, and how to judge the values.
# A judge returns PASS, FAIL or RECORDED; `None` means the person fills the row and it is RECORDED.
ROWS = [
    ("R1", "crate commit the payload was built against", "40 hex digits",
     lambda v: passes(re.fullmatch(r"[0-9a-f]{40}", v["R1"]) is not None)),
    ("R2", "model (1 means the QEMU rehearsal)", "0 on the board",
     lambda v: passes(v["R2"] == "0")),
    ("R3", "how the payload was loaded: the boot script's `payload came from`, or typed at the prompt",
     "net, or prompt", lambda v: passes(v["R3"] in ("net", "prompt"))),
    ("R4", "how the board came up before this run (cold power-on, or what)", "recorded", None),
    ("R5", "timebase in hertz, and whether the device tree gave it", "from_tree=1",
     lambda v: passes(kv(v["R5"]).get("from_tree") == "1")),
    ("R6", "TRNG window, compatible, and whether the tree named it", "0x1600c000, from_tree=1",
     lambda v: passes(v["R6"].startswith("0x1600c000 ") and v["R6"].endswith("from_tree=1"))),
    ("R7", "clocks enabled and reset released", "clocks=1 released=1",
     lambda v: passes(v["R7"] == "clocks=1 released=1")),
    ("R8", "the run reached its end record", "1", lambda v: passes(v["R8"] == "1")),
    ("R9", "records dropped for a bad checksum", "recorded", lambda v: "RECORDED"),
    ("1a", "first reseed completed by polling, and STAT.SEEDED after it", "done=1 seeded=1",
     lambda v: passes(v["1a"] == "done=1 seeded=1")),
    ("1b", "first generation's outcome", "ready", lambda v: passes(v["1b"] == "ready")),
    ("1c", "second generation's outcome", "ready", lambda v: passes(v["1c"] == "ready")),
    ("1d", "first generation's 32 bytes", "not all zero",
     lambda v: passes(re.fullmatch(r"[0-9a-f]{64}", v["1d"]) is not None and set(v["1d"]) != {"0"})),
    ("1e", "second generation's 32 bytes", "not all zero, and differ from 1d",
     lambda v: passes(re.fullmatch(r"[0-9a-f]{64}", v["1e"]) is not None
                      and set(v["1e"]) != {"0"} and v["1e"] != v["1d"])),
    ("2a", "STAT.SEEDED as the board leaves reset", "recorded", lambda v: "RECORDED"),
    ("2b", "STAT.LAST_RESEED as the board leaves reset", "recorded (7 is unseeded)",
     lambda v: "RECORDED"),
    ("2c", "STAT.LAST_RESEED after the first reseed", "recorded", lambda v: "RECORDED"),
    ("3a", "STAT.R256 before the mode write", "recorded", lambda v: "RECORDED"),
    ("3b", "MODE before the mode write", "recorded", lambda v: "RECORDED"),
    ("3c", "FEATURES", "recorded", lambda v: "RECORDED"),
    ("3d", "BCONF", "recorded", lambda v: "RECORDED"),
    ("3e", "STAT.R256 after MODE_256_BIT_OUTPUT is written", "1",
     lambda v: passes(v["3e"] == "1")),
    ("3f", "which value of BCONF bit 3 means 256 bits", "derived from 3a and 3d",
     lambda v: "RECORDED" if v["3f"] == width_meaning(v) else "FAIL"),
    ("3g", "FEATURES bits 1:0 and BCONF bit 2, the widest output each says the block was built for",
     "max_rand_length=1 max_prng_len=1 (both 256-bit), as read in 3c and 3d",
     lambda v: passes(v["3g"] == widest(v) == "max_rand_length=1 max_prng_len=1")),
    ("4a", "RAND_RDY with RAND_GENERATING, RAND_RDY acknowledged first (1000 trials)",
     "timeouts=0; together recorded",
     lambda v: passes(kv(v["4a"]).get("timeouts") == "0")),
    ("4b", "RAND_RDY with RAND_GENERATING, second GENERATE over a latched RAND_RDY",
     "timeouts=0; together recorded",
     lambda v: passes(kv(v["4b"]).get("timeouts") == "0")),
    ("5a", "STAT bits outside every field the crate names, ever seen", "recorded",
     lambda v: "RECORDED"),
    ("5b", "LAST_RESEED values seen (bit n set means value n was seen)", "recorded",
     lambda v: "RECORDED"),
    ("5c", "STAT bits 19 to 23 (above a 3-bit LAST_RESEED) ever set", "recorded",
     lambda v: "RECORDED" if v["5c"] == str(int(hexint(v["5a"]) & STATUS_ABOVE_LAST_RESEED != 0))
     else "FAIL"),
    ("6a", "generate to RAND_RDY, ticks", "timeouts=0",
     lambda v: passes(kv(v["6a"]).get("timeouts") == "0")),
    ("6b", "generate to RAND_RDY, microseconds (median, maximum)", "recorded",
     lambda v: "RECORDED" if v["6b"] == f"{micro(v, '6a', 'median')} {micro(v, '6a', 'maximum')}"
     else "FAIL"),
    ("6c", "reseed to SEED_DONE, ticks", "timeouts=0",
     lambda v: passes(kv(v["6c"]).get("timeouts") == "0")),
    ("6d", "reseed to SEED_DONE, microseconds (median, maximum)", "recorded",
     lambda v: "RECORDED" if v["6d"] == f"{micro(v, '6c', 'median')} {micro(v, '6c', 'maximum')}"
     else "FAIL"),
    ("6e", "one read of the time CSR, minimum ticks", "recorded", lambda v: "RECORDED"),
    ("7a", "ISTAT after the bring-up clear", "no documented bit left",
     lambda v: passes(hexint(v["7a"]) & INTERRUPT_STATUS_DOCUMENTED == 0)),
    ("7b", "ISTAT bit 3 with AUTO_RQSTS=1", "recorded; residue has no documented bit",
     lambda v: passes(hexint(kv(v["7b"])["istat_residue"]) & INTERRUPT_STATUS_DOCUMENTED == 0)),
    ("7c", "ISTAT bit 2 with AUTO_AGE=1", "recorded; residue has no documented bit",
     lambda v: passes(hexint(kv(v["7c"])["istat_residue"]) & INTERRUPT_STATUS_DOCUMENTED == 0)),
    ("8a", "long run of driver-path generations", "all_zero=0 wrong_output_width=0 timeouts=0",
     lambda v: passes(all(kv(v["8a"]).get(k) == "0"
                          for k in ("all_zero", "wrong_output_width", "timeouts")))),
    ("8b", "first LFSR_LOCKUP: does a reseed clear it, and does the next generation serve",
     "next_outcome=ready, or none seen",
     lambda v: "RECORDED" if v["8b"] == "none seen"
     else passes(kv(v["8b"]).get("next_outcome") == "ready")),
    ("9a", "capture: bytes whose records verified", "bytes>=1048576",
     lambda v: judge_capture(v, "9a", lambda s: int(s["bytes"]) >= MEBIBYTE)),
    ("9b", "FIPS 140-2 blocks failed", "at most 3",
     lambda v: judge_capture(v, "9b", lambda s: int(s["fips_failed"]) <= 3)),
    ("9c", "Shannon entropy, bits per byte", "at least 7.99",
     lambda v: judge_capture(v, "9c", lambda s: float(s["entropy"]) >= 7.99)),
    ("9d", "chi-square over byte values, upper-tail p", "between 0.001 and 0.999",
     lambda v: judge_capture(v, "9d", lambda s: 0.001 < float(s["chi_square_p"]) < 0.999)),
    ("9e", "mean byte and lag-1 serial correlation", "mean within 0.5 of 127.5, abs(r) < 0.005",
     lambda v: judge_capture(v, "9e", lambda s: abs(float(s["mean"]) - 127.5) < 0.5
                             and abs(float(s["serial_correlation"])) < 0.005)),
    ("9f", "repeated 32-byte generations in the capture", "0",
     lambda v: judge_capture(v, "9f", lambda s: s["repeated_generations"] == "0")),
    ("9g", "outside statistical suite, if one was run (name and summary)", "recorded", None),
]


def table(values, verdicts):
    lines = ["| id | measure | value | criterion | verdict |", "|---|---|---|---|---|"]
    for row_id, measure, criterion, _ in ROWS:
        value = values.get(row_id, "") or "_"
        lines.append(f"| {row_id} | {measure} | {value} | {criterion} | {verdicts.get(row_id, '_')} |")
    return "\n".join(lines) + "\n"


TITLE = "# jh7110_entropy board check: results\n\n"


def summarize(text, capture_out=None):
    """The filled table for the last run in the log `text`."""
    records, damaged, came_from = parse_log(text)
    v, capture = extract(records)
    if capture_out:
        with open(capture_out, "wb") as f:
            f.write(capture)
    s = statistics(capture)
    stats = {
        "9a": f"bytes={s['bytes']}",
        "9b": f"fips_failed={s.get('fips_failed', 0)} fips_blocks={s.get('fips_blocks', 0)}",
        "9c": f"entropy={s.get('entropy', 0):.6f}",
        "9d": f"chi_square={s.get('chi_square', 0):.2f} chi_square_p={s.get('chi_square_p', 0):.4f}",
        "9e": f"mean={s.get('mean', 0):.4f} serial_correlation={s.get('serial_correlation', 1):.6f}",
        "9f": f"repeated_generations={s.get('repeated_generations', 0)}",
    }
    values = {
        "R1": v.get("crate_commit"), "R2": v.get("model"), "R3": came_from,
        "R5": f"{v.get('timebase_hertz')} from_tree={v.get('timebase_from_tree')}",
        "R6": v.get("window"),
        "R7": f"clocks={v.get('clocks_enabled')} released={v.get('reset_released')}",
        "R8": v.get("complete"), "R9": str(damaged), "1a": v.get("first_reseed"),
        "1b": v.get("generation_1"), "1c": v.get("generation_2"),
        "1d": v.get("bytes_1"), "1e": v.get("bytes_2"),
        "2a": v.get("reset_seeded"), "2b": v.get("reset_last_reseed"),
        "2c": v.get("last_reseed_after_reseed"), "3a": v.get("reset_r256"),
        "3b": v.get("reset_mode"), "3c": v.get("reset_features"), "3d": v.get("reset_bconf"),
        "3e": v.get("r256_after"), "4a": v.get("overlap_acknowledged"),
        "4b": v.get("overlap_stale_ready"), "5a": v.get("unnamed_seen"),
        "5b": v.get("last_reseed_values"), "6a": v.get("timing_generate"),
        "6c": v.get("timing_reseed"), "6e": kv(v.get("timing_time_read")).get("minimum"),
        "7a": v.get("bring_up_residue"), "7b": v.get("alarm_requests"),
        "7c": v.get("alarm_age"), "8a": v.get("long_run"),
        "8b": v.get("lockup", "none seen" if "long_run" in v else None),
    }
    values.update(stats if capture else {})
    try:
        values["3f"] = width_meaning(values)
        values["3g"] = widest(values)
        values["5c"] = str(int(hexint(v["status_seen"]) & STATUS_ABOVE_LAST_RESEED != 0))
        values["6b"] = f"{micro(values, '6a', 'median')} {micro(values, '6a', 'maximum')}"
        values["6d"] = f"{micro(values, '6c', 'median')} {micro(values, '6c', 'maximum')}"
    except (KeyError, TypeError, ValueError):
        pass
    verdicts = judge(values)
    return TITLE + table(values, verdicts)


def judge(values):
    """Each row's verdict from the values, or `_` where the values cannot be judged."""
    verdicts = {}
    for row_id, _, _, rule in ROWS:
        if not values.get(row_id) or values[row_id] == "_":
            verdicts[row_id] = "_"
        elif rule is None:
            verdicts[row_id] = "RECORDED"
        else:
            try:
                verdicts[row_id] = rule(values)
            except (KeyError, TypeError, ValueError, IndexError, ZeroDivisionError):
                verdicts[row_id] = "FAIL"
    return verdicts


def read_table(text):
    """The values and verdicts a results table holds, by row id."""
    values, verdicts = {}, {}
    for line in text.splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) == 5 and cells[0] and cells[0] not in ("id", "---"):
            values[cells[0]], verdicts[cells[0]] = cells[2], cells[4]
    return values, verdicts


# Rows a QEMU rehearsal cannot meet: no boot script, no person, no JH7110 window or clocks.
MODEL_EXEMPT = {"R2", "R3", "R4", "R6", "R7", "9g"}


def fill(text, row_id, value):
    """`text` with row `row_id`'s value set to `value` and its verdict recomputed."""
    if "|" in value or not value.strip():
        raise ValueError("a value is not empty and holds no `|`")
    values, _ = read_table(text)
    values[row_id] = value.strip()
    verdict = judge(values)[row_id]
    out, found = [], False
    for line in text.splitlines(keepends=True):
        cells = line.split("|")
        if len(cells) == 7 and cells[1].strip() == row_id:
            cells[3], cells[5], found = f" {value.strip()} ", f" {verdict} ", True
            line = "|".join(cells)
        out.append(line)
    if not found:
        raise ValueError(f"no row {row_id}")
    return "".join(out)


def check(text, allow_model=False):
    """The problems with a filled table, one string each; empty when it passes."""
    values, written = read_table(text)
    problems = []
    exempt = MODEL_EXEMPT if allow_model and values.get("R2") == "1" else set()
    computed = judge(values)
    for row_id, measure, _, _ in ROWS:
        if row_id in exempt:
            continue
        if row_id not in values:
            problems.append(f"{row_id}: row missing")
        elif computed[row_id] == "_":
            problems.append(f"{row_id}: not filled ({measure})")
        elif computed[row_id] == "FAIL":
            problems.append(f"{row_id}: FAIL ({measure}: {values[row_id]})")
        elif written.get(row_id) != computed[row_id]:
            problems.append(f"{row_id}: the table says {written.get(row_id)}, the values say "
                            f"{computed[row_id]}")
    return problems


def synthetic_log(capture, damage=True, model=0, lockup=True):
    """A log in the payload's format, for the selftest."""
    def rec(payload):
        return f"board-check: {payload} *{zlib.crc32(payload.encode()):08x}\r\n"
    lines = ["U-Boot noise\r\n", "nife: payload came from net\r\n",
             rec(f"begin format=1 model={model} crate_commit={'ab' * 20} hart=1 "
                 "device_tree=0x86000000 tree_parsed=1 timebase_hertz=4000000 timebase_from_tree=1"),
             rec("window trng=0x1600c000 size=0x4000 from_tree=1 compatible=starfive,trng status_okay=0"),
             rec("clock_step kind=enable index=15 before=0x00000000 after=0x80000000"),
             rec("clock_step kind=enable index=16 before=0x00000000 after=0x80000000"),
             rec("clock_step kind=deassert id=3 assert_before=0x8 status_before=0x0 status_after=0x8 released=1"),
             rec("reset_state stat=0x00070100 istat=0x0 mode=0x0 smode=0x100 ie=0x0 features=0x0000000d "
                 "bconf=0x00000034 auto_rqsts=0x0 auto_age=0x0 seeded=0 last_reseed=7 r256=0"),
             rec("bring_up istat_residue=0x00000000 mode=0x8 stat=0x00070108 r256=1"),
             rec("first_reseed done=1 ticks=40 polls=9 stat=0x00000308 seeded=1 last_reseed=0"),
             rec(f"generation index=1 outcome=ready ticks=9 bytes={'11' * 32}"),
             rec(f"generation index=2 outcome=ready ticks=9 bytes={'22' * 32}"),
             rec("overlap scenario=acknowledged trials=1000 snapshots=4000 together=0 together_loose=3 timeouts=0"),
             rec("overlap scenario=stale_ready trials=1000 accepted=1000 words_changed=1000 together=1000 together_loose=1000 timeouts=0"),
             rec("timing kind=time_read minimum=1"),
             rec("timing kind=generate samples=1000 timeouts=0 minimum=8 median=9 p99=12 maximum=20 polls_median=4"),
             rec("timing kind=reseed samples=100 timeouts=0 minimum=30 median=40 p99=50 maximum=60 polls_median=9"),
             rec("alarm kind=requests written=1 readback=0x1 latched=1 istat_seen=0x00000009 seed_done_seen=0 last_reseed_values=0x01 istat_residue=0x00000000"),
             rec("alarm kind=age written=1 readback=0x1 latched=0 latched_after_ticks=0 istat_seen=0x0 istat_residue=0x00000000 stat=0x308"),
             rec("long_run generations=1048576 ready=1048575 all_zero=0 lockups=1 unseeded=0 wrong_output_width=0 timeouts=0")]
    if lockup:
        lines.append(rec("lockup index=4999 istat=0x11 reseed_done=1 bit_after_reseed=1 bit_after_acknowledge=0 next_outcome=ready"))
    lines.append(rec("status_bits seen=0x40070b08 unnamed_seen=0x00000000 last_reseed_values_seen=0x81"))
    for i in range(0, len(capture), 32):
        lines.append(rec(f"capture {i // 32} {capture[i:i + 32].hex()}"))
    if damage:
        lines.append("board-check: capture 99999 00ff *00000000\r\n")
    lines += [rec(f"capture_end lines={len(capture) // 32} gaps=0"), rec("end complete=1")]
    return "".join(lines)


def counter_bytes(n):
    """`n` deterministic, well-mixed bytes (SHA-256 in counter mode), so the selftest's statistics
    rows have one answer on every run rather than failing one run in a few hundred by chance."""
    out = bytearray()
    for i in itertools.count():
        if len(out) >= n:
            return bytes(out[:n])
        out += hashlib.sha256(i.to_bytes(8, "little")).digest()


def selftest():
    good = counter_bytes(MEBIBYTE + 4096)
    filled = summarize(synthetic_log(good))
    assert any(p.startswith("R4: not filled") for p in check(filled))
    filled = fill(fill(filled, "R4", "cold power-on by the person"), "9g", "not run")
    problems = check(filled)
    assert problems == [], problems
    assert "| R9 | records dropped for a bad checksum | 1 |" in filled, filled
    assert "| 3f | which value of BCONF bit 3 means 256 bits | 1 means 256-bit |" in filled, filled
    # A hand edit that breaks a criterion is caught, and so is a verdict that does not follow.
    assert any(p.startswith("3e: FAIL") for p in check(re.sub(r"(\| 3e \|[^|]*\|) 1 ", r"\1 0 ", filled)))
    assert any(p.startswith("1b:") for p in check(filled.replace("| ready | PASS |", "| ready | FAIL |", 1)))
    # A model run is refused unless asked for, and a short or blank table is refused.
    model = summarize(synthetic_log(good, model=1))
    assert any(p.startswith("R2: FAIL") for p in check(model))
    assert not any(p.startswith("R2") for p in check(model, allow_model=True))
    assert any("not filled" in p for p in check(template()))
    # Zeros fail the statistics; a short capture fails the size row.
    zeros = summarize(synthetic_log(bytes(MEBIBYTE)))
    for row in ("9b", "9c", "9d", "9f"):
        assert any(p.startswith(f"{row}: FAIL") for p in check(zeros)), row
    short = summarize(synthetic_log(counter_bytes(4096)))
    assert any(p.startswith("9a: FAIL") for p in check(short))
    # No lockup in the long run reads as none seen, which is recorded, not failed.
    assert "| none seen |" in summarize(synthetic_log(good, lockup=False))
    # The committed template is this file's own.
    here = os.path.dirname(os.path.abspath(__file__))
    with open(os.path.join(here, "results-template.md"), encoding="utf-8") as f:
        assert f.read() == template(), "results-template.md differs from `results.py template`"
    with tempfile.TemporaryDirectory() as directory:
        out = os.path.join(directory, "capture.bin")
        summarize(synthetic_log(good[:64]), capture_out=out)
        with open(out, "rb") as f:
            assert f.read() == good[:64]
    print("results.py selftest: ok")


def template():
    return TITLE + table({}, {})


def main(argv):
    if argv == ["--selftest"]:
        selftest()
        return 0
    if argv == ["template"]:
        sys.stdout.write(template())
        return 0
    if len(argv) in (2, 4) and argv[0] == "summarize":
        capture_out = argv[3] if len(argv) == 4 and argv[2] == "--capture" else None
        with open(argv[1], encoding="latin-1") as f:
            sys.stdout.write(summarize(f.read(), capture_out))
        return 0
    if len(argv) == 4 and argv[0] == "fill":
        with open(argv[1], encoding="utf-8") as f:
            text = fill(f.read(), argv[2], argv[3])
        with open(argv[1], "w", encoding="utf-8") as f:
            f.write(text)
        return 0
    paths = [a for a in argv[1:] if a != "--allow-model"]
    if argv and argv[0] == "check" and len(paths) == 1 and len(argv) - len(paths) <= 2:
        with open(paths[0], encoding="utf-8") as f:
            problems = check(f.read(), allow_model="--allow-model" in argv)
        for problem in problems:
            print(f"results.py: {problem}")
        print(f"results.py: {'FAIL' if problems else 'PASS'}, {len(problems)} problem(s)")
        return 1 if problems else 0
    sys.stderr.write(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
