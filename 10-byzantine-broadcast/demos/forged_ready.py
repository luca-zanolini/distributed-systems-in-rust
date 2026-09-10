"""Authenticated links, demonstrated by attack: forged READYs bounce.

Before the authentication layer, `from` was a self-declared string, so a raw
socket — not a node at all, holding no keys — could send a victim three READYs
for 'evil' signed by nobody, from three impersonated identities. Three READYs
is 2f+1: past the delivery threshold. The victim would DELIVER a value the
designated sender never sent — integrity gone; aimed at two victims with two
different values, consistency gone. The <= f assumption is only meaningful
over an unforgeable identity space (Sybil / Douceur 2002); self-declared
strings are the opposite of that.

With the layer in place, every message must carry a signature — over the
domain-separated statement 'READY:{m}' — that verifies against the KNOWN key
of its claimed sender. The forged messages die at the gate, one DROPPED line
each, and no node delivers anything.

Note what signatures do NOT change: the equivocation demo behaves exactly as
before, because the equivocating sender owns its key and happily signs BOTH
'SEND:attack' and 'SEND:retreat'. Signatures prevent IMPERSONATION, not
equivocation — defeating equivocation is the protocol's job (echo quorums),
not the crypto's. Authentication is the floor the protocol stands on, not a
substitute for it."""
import socket, time
import common as c

procs = c.launch(c.ALL)
print(">> nobody broadcasts; a keyless raw socket forges 3 READYs for 'evil'")
print("   impersonating 6001, 6002, 6003 — at every node (2f+1 = past the")
print("   delivery threshold, if believed)")

fake_sig = "00" * 64   # valid hex, valid length, signed by nobody
for victim in c.ALL:
    for imp in ["6001", "6002", "6003"]:
        forged = f"READY 127.0.0.1:{imp} {fake_sig} evil\n"
        s = socket.create_connection(("127.0.0.1", int(victim)))
        s.sendall(forged.encode())
        s.close()

time.sleep(1.0)
logs = {}
for p, proc in procs.items():
    proc.terminate()
    text, _ = proc.communicate(timeout=5)
    logs[p] = text.splitlines()

delivered = {p: [l for l in logs[p] if "Delivered message:" in l] for p in c.ALL}
dropped = {p: len([l for l in logs[p] if l.startswith("DROPPED")]) for p in c.ALL}
for p in c.ALL:
    print(f"   node {p}: delivered {delivered[p] or 'NOTHING'}, "
          f"dropped {dropped[p]} unauthenticated message(s)")

if all(not delivered[p] for p in c.ALL) and all(dropped[p] >= 3 for p in c.ALL):
    print("\n   VERDICT: FORGERY BOUNCED — every impersonated READY died at the")
    print("   verify_envelope gate; nobody delivered. Pre-auth, this exact attack")
    print("   made any victim deliver an arbitrary value with three spoofed")
    print("   messages. The protocol above the gate is unchanged — authentication")
    print("   is a layer, and this demo is the layer earning its keep.")
else:
    print(f"\n   VERDICT: UNEXPECTED — delivered={delivered}, dropped={dropped}")
