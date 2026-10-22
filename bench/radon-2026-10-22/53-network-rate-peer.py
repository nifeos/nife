"""Serve 100 MiB per connection on :9494, like the note's nc loop, and sample the sending
socket's TCP_CONNECTION_INFO (macOS, no root) every 5 ms: radon's advertised window as this
side sees it, RTT, and the count of segments radon sent back (its ACKs)."""
import socket, struct, sys, threading, time

TCP_CONNECTION_INFO = 0x106
FMT = "<BBBBIIIIIIIIIIIIIQQQQQQQ"  # struct tcp_connection_info, netinet/tcp.h
TOTAL = 104857600
out = open(sys.argv[1], "a")

def info(s):
    b = s.getsockopt(socket.IPPROTO_TCP, TCP_CONNECTION_INFO, struct.calcsize(FMT))
    v = struct.unpack(FMT, b)
    return dict(state=v[0], snd_wscale=v[1], rcv_wscale=v[2], maxseg=v[7], cwnd=v[9],
                snd_wnd=v[10], rttcur=v[13], srtt=v[14], txpackets=v[17], txbytes=v[18],
                retx=v[19], rxpackets=v[20])

ls = socket.socket(); ls.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
ls.bind(("0.0.0.0", 9494)); ls.listen(1)
while True:
    c, addr = ls.accept()
    t0 = time.time(); done = [False]; samples = []
    def sample():
        while not done[0]:
            try: samples.append((time.time() - t0, info(c)))
            except OSError: break
            time.sleep(0.005)
    th = threading.Thread(target=sample); th.start()
    buf = bytes(65536); sent = 0
    try:
        while sent < TOTAL:
            sent += c.send(buf[: min(len(buf), TOTAL - sent)])
        c.shutdown(socket.SHUT_WR)
        while c.recv(4096): pass
    except OSError as e:
        out.write(f"  transfer ended early: {e}\n")
    t1 = time.time(); done[0] = True; th.join(); last = samples[-1][1] if samples else info(c)
    c.close()
    wnds = {}
    for _, s in samples: wnds[s["snd_wnd"]] = wnds.get(s["snd_wnd"], 0) + 1
    rtts = sorted(s["srtt"] for _, s in samples if s["srtt"])
    out.write(f"connection from {addr[0]} at {time.strftime('%H:%M:%S', time.gmtime(t0))} UTC\n")
    out.write(f"  {sent} bytes in {t1 - t0:.3f} s = {sent * 8 / (t1 - t0) / 1e6:.3f} Mbit/s (sender side, to close)\n")
    out.write(f"  wscale snd {last['snd_wscale']} rcv {last['rcv_wscale']}, maxseg {last['maxseg']}\n")
    out.write(f"  radon's advertised window, sampled every 5 ms (value: samples): "
              f"{sorted(wnds.items(), key=lambda kv: -kv[1])[:8]}\n")
    if rtts:
        out.write(f"  srtt ms p50 {rtts[len(rtts)//2]} p90 {rtts[int(len(rtts)*0.9)]} max {rtts[-1]}\n")
    out.write(f"  segments sent {last['txpackets']}, received from radon {last['rxpackets']}, "
              f"retransmitted bytes {last['retx']}\n")
    if last["rxpackets"]:
        out.write(f"  mean gap between radon's segments {(t1 - t0) * 1000 / last['rxpackets']:.3f} ms\n")
    out.write(f"  cwnd at end {last['cwnd']}\n"); out.flush()
