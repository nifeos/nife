"""Read the JSON `clippy-runs.sh` kept and print the census's lint counts and distributions.

    python3 notes/lint-census-2026-10-09/clippy_counts.py OUT_DIR

A hit is one (file, line, column, lint), deduplicated across the five configurations, so a site
compiled for three architectures counts once. Only files `scope.in_scope` admits are counted.
"""

import collections
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import scope  # noqa: E402

RUNS = ('host', 'a64', 'rv', 'x86', 'redox', 'nest-host', 'nest-a64')

# The crates that take bytes from a less-trusted party: a disk, the network, a device, or a peer
# process across an IPC contract. The census's own reading, for an architect to amend.
PARSERS = {
    'disk and image formats': ['redoxfs_server', 'nifefs', 'globally_unique_identifier_partition_table',
                               'file_allocation_table', 'package_archive', 'elf', 'manifest_note',
                               'boot_slot'],
    'network': ['domain_name_system', 'http_response', 'network_time_protocol', 'socket_contract',
                'domain_name_resolution_contract'],
    'device-supplied': ['device_tree_blob', 'machine_discovery', 'pci', 'usb',
                        'extensible_host_controller_interface', 'e1000e', 'designware_ethernet',
                        'designware_mobile_storage', 'non_volatile_memory_express', 'virtio'],
    'peer-process contracts': ['file_system_contract', 'system_log_contract', 'byte_sink_contract',
                               'graphics_contract', 'compositor', 'login_contract',
                               'credential_contract', 'supervision_protocol', 'hot_swap_contract'],
}
PANICS = ('unwrap_used', 'expect_used', 'panic', 'unreachable', 'todo', 'unimplemented')


def read(out):
    hits = {}
    for run in RUNS:
        path = os.path.join(out, f'{run}.json')
        if not os.path.exists(path):
            continue
        with open(path) as f:
            for line in f:
                try:
                    m = json.loads(line)
                except ValueError:
                    continue
                if m.get('reason') != 'compiler-message':
                    continue
                msg = m['message']
                code = (msg.get('code') or {}).get('code')
                spans = [s for s in msg['spans'] if s.get('is_primary')]
                if not code or not spans:
                    continue
                fn = spans[0]['file_name']
                if run == 'redox' and not fn.startswith('/'):
                    fn = 'redoxfs_server/' + fn
                if fn.startswith('/'):
                    fn = os.path.relpath(fn)
                if scope.in_scope(fn):
                    key = (fn, spans[0]['line_start'], spans[0]['column_start'], code.split('::')[-1])
                    hits[key] = msg['message']
    return hits


def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(p / 100 * len(xs)))]


def crate_of(path):
    if path.startswith('redoxfs_server/'):
        return 'redoxfs_server'
    return path.split('/')[1] if path.startswith('crates/') else None


def main():
    hits = read(sys.argv[1])
    tree, kernel = collections.Counter(), collections.Counter()
    for (fn, _, _, lint) in hits:
        tree[lint] += 1
        kernel[lint] += fn.startswith('kernel/src/')
    print(f'{"lint":34} {"in scope":>9} {"kernel":>7}')
    for lint in sorted(tree):
        print(f'{lint:34} {tree[lint]:9} {kernel[lint]:7}')

    print('\nkernel panic sites by where they are')
    for lint in PANICS + ('indexing_slicing',):
        rows = [fn for (fn, _, _, l) in hits if l == lint and fn.startswith('kernel/src/')]
        svc = sum(1 for f in rows if re.match(r'kernel/src/user/\w+_service\.rs$', f))
        print(f'  {lint:18} {len(rows):4}  boot services {svc:4}  user.rs '
              f'{rows.count("kernel/src/user.rs"):3}  sched.rs {rows.count("kernel/src/sched.rs"):3}  '
              f'arch/ {sum(1 for f in rows if f.startswith("kernel/src/arch/")):3}')

    print('\nparsers: indexing_slicing, arithmetic_side_effects, and the panics')
    for group, crates in PARSERS.items():
        row = collections.Counter()
        for (fn, _, _, lint) in hits:
            if crate_of(fn) in crates:
                row['panics' if lint in PANICS else lint] += 1
        print(f'  {group:24} {len(crates):2} crates  indexing {row["indexing_slicing"]:4}  '
              f'arithmetic {row["arithmetic_side_effects"]:4}  panics {row["panics"]:3}')

    trunc = collections.Counter()
    for (fn, _, _, lint), text in hits.items():
        if lint == 'cast_possible_truncation':
            m = re.search(r'casting `(\w+)` to `(\w+)`', text)
            trunc[m.groups() if m else ('?', '?')] += 1
    wide = sum(v for (a, b), v in trunc.items() if b == 'usize' and a in ('u64', 'i64'))
    print(f'\ncast_possible_truncation: {sum(trunc.values())}, of which 64-bit to usize: {wide}')

    for lint, label in (('too_many_lines', 'function length, code lines'),
                        ('cognitive_complexity', 'cognitive complexity')):
        for name, pred in (('in scope', lambda f: True), ('kernel', lambda f: f.startswith('kernel/src/'))):
            xs = [int(re.search(r'\((\d+)/', t).group(1)) for (f, _, _, l), t in hits.items()
                  if l == lint and pred(f)]
            if xs:
                print(f'{label} ({name}): n {len(xs)} p50 {pct(xs, 50)} p90 {pct(xs, 90)} '
                      f'p95 {pct(xs, 95)} p99 {pct(xs, 99)} max {max(xs)}; over 25 '
                      f'{sum(x > 25 for x in xs)}, 50 {sum(x > 50 for x in xs)}, 100 '
                      f'{sum(x > 100 for x in xs)}, 150 {sum(x > 150 for x in xs)}, 200 '
                      f'{sum(x > 200 for x in xs)}')


    nest = [f for (f, _, _, l) in hits if l == 'excessive_nesting']
    if nest:
        print(f'excessive_nesting: {len(nest)} blocks in {len(set(nest))} files, '
              f'{sum(f.startswith("kernel/src/") for f in nest)} in the kernel')


if __name__ == '__main__':
    main()
