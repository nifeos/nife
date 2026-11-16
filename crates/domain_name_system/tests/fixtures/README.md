# Captured replies

Each `.dns` file is one raw DNS reply, exactly the bytes a recursive server sent over UDP, with
nothing added. `tests/replies.rs` reads them with `include_bytes!`.

| file | question | transaction id |
|---|---|---|
| `example.com.dns` | `example.com` | `0x1111` |
| `www.github.com.dns` | `www.github.com` | `0x2222` |
| `nosuch.invalid.dns` | `nosuch.invalid` | `0x3333` |

## Capturing one again

The query is the one `Query::request` writes: the header with recursion desired and one question,
then the name, type A and class IN. Send it to a recursive server and keep the first datagram back.
This needs Python 3 and a network that reaches the server; change `HOST`, `ID` and `OUT` for each
capture, and a new fixture takes its transaction id from `ID`.

```sh
python3 - <<'PY'
import socket, struct

HOST, ID, OUT = "example.com", 0x1111, "example.com.dns"
name = b"".join(bytes([len(p)]) + p.encode() for p in HOST.split(".")) + b"\0"
query = struct.pack(">HHHHHH", ID, 0x0100, 1, 0, 0, 0) + name + struct.pack(">HH", 1, 1)
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.settimeout(5)
sock.sendto(query, ("1.1.1.1", 53))
reply, _ = sock.recvfrom(4096)
open(OUT, "wb").write(reply)
PY
```

Check a capture by building the query with `Query::request` for the same name and id and comparing
it with the `query` above. Then add the file to `tests/replies.rs` with the checks it should pass.
