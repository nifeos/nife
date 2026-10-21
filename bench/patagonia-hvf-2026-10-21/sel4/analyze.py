import json, sys, statistics as st
N = 10000; TICK = 1e9 / 24e6
def load(p):
    s = open(p).read().split('JSON OUTPUT')[1].strip()
    return json.JSONDecoder().raw_decode(s)[0][0]['Results']
def key(r): return (r['Function'].replace('seL4_Call', 'X').replace('seL4_ReplyRecv', 'X'), r['Same vspace?'], r['IPC length'])
mode = sys.argv[1]
for p in sys.argv[2:]:
    rs = load(p)
    if mode == 'pmu':
        print(p)
        for r in rs: print('  %-22s %-15s same=%-5s len=%-2s min %6d median %8.1f mean %8.1f sd %8.1f' % (r['Function'], r['Direction'], r['Same vspace?'], r['IPC length'], r['Min'], r['Median'], r['Mean'], r['Stddev']))
        continue
    calls = {key(r): r for r in rs if r['Direction'] == 'client->server' and r['Function'] != 'seL4_Send'}
    reps = {key(r): r for r in rs if r['Direction'] == 'server->client'}
    out = []
    for k in calls:
        if k not in reps: continue
        a, b = calls[k], reps[k]
        for stat in ('Min', 'Median'):
            A, B = a[stat], b[stat]
            rt = (A + B) / (2 * N - 1) * TICK
            c = (N * A - (N - 1) * B) / (2 * N - 1) * TICK
            out.append('%s %s same=%s len=%s rt %.1f ns call %.1f reply %.1f' % (stat, k[0], k[1], k[2], rt, c, rt - c))
    print(p); print('\n'.join('  ' + o for o in out))
