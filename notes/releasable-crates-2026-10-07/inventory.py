#!/usr/bin/env python3
"""Inventory of the workspace's crates for notes/releasable-crates-2026-10-07.md.

Run from the repository root:  python3 notes/releasable-crates-2026-10-07/inventory.py
It reads `cargo metadata` (the dependency graph, never a guess), counts code lines under src/
(blank and `//` lines excluded), Kani harnesses (`#[kani::proof` anywhere in the crate) and
falsification patches (`falsifications/*.patch`), and prints a markdown table. The class column
is this lane's judgment, held in CLASS below so a re-run reproduces it; a crate missing from
CLASS prints as C. Classes: A standalone today; B standalone after the cut named in CUT; C nife-only.
"""
import glob, json, os, re, subprocess

CLASS = {
    # A: no nife crate in its normal graph, and a reader outside nife has a use for it.
    **dict.fromkeys("""address_space_identifier bitmap_font calendar coremark cpu_set
        device_tree_blob domain_name_system elf extensible_host_controller_interface
        file_allocation_table generational_table glob globally_unique_identifier_partition_table
        http_response intrusive_fifo network_time_protocol non_volatile_memory_express
        page_frames paging pci portable_executable universally_unique_identifier usb""".split(), "A"),
    # B: one cut away; CUT says which.
    **dict.fromkeys("""designware_ethernet designware_mobile_storage e1000e jh7110_clock_and_reset
        jh7110_entropy line_editor video_terminal""".split(), "B"),
}
CUT = {
    "e1000e": "inline `address_space_layout::pair_page`, its one use",
    "designware_ethernet": "inline `pair_page`; take the register region from the caller, not the tree",
    "designware_mobile_storage": "inline `pair_page` and `BLOCK_SIZE`; take the region from the caller",
    "jh7110_entropy": "take the register region from the caller, not the tree",
    "jh7110_clock_and_reset": "take the register regions from the caller, not the tree",
    "line_editor": "move the `component_plan` wiring into the program that serves it",
    "video_terminal": "`bitmap_font` released with it",
}

def code_lines(d):
    n = 0
    for f in glob.glob(d + "/src/**/*.rs", recursive=True):
        for line in open(f, errors="ignore"):
            s = line.strip()
            if s and not s.startswith("//"):
                n += 1
    return n

def count(d, pattern):
    return sum(len(re.findall(pattern, open(f, errors="ignore").read()))
               for f in glob.glob(d + "/**/*.rs", recursive=True))

meta = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline"]))
root = meta["workspace_root"]
packages = {p["name"]: p for p in meta["packages"]}
print("| crate | class | code lines | Kani | falsified | nife crates in its graph | cut |")
print("|---|---|---|---|---|---|---|")
for name in sorted(packages):
    p = packages[name]
    d = os.path.dirname(p["manifest_path"])
    if not os.path.relpath(d, root).startswith("crates/"):
        continue
    inner = sorted({x["name"] for x in p["dependencies"]
                    if x["kind"] in (None, "build") and x["name"] in packages})
    fal = len(glob.glob(d + "/falsifications/*.patch"))
    print(f"| {name} | {CLASS.get(name, 'C')} | {code_lines(d)} | {count(d, r'#\[kani::proof')} "
          f"| {fal} | {', '.join(inner) or 'none'} | {CUT.get(name, '')} |")
