"""The forged-COMMIT attack — the audit's find — now bounces at the gate.

Before envelope authentication, the Commit arm believed any self-declared
`from`: a raw socket holding no keys could send a victim three COMMITs for
'evil' impersonating 7001/7002/7003 — count 3 > 2f — and the victim would
DECIDE a value nobody ever proposed or prepared. Agreement and validity
broken in the NORMAL CASE, no view change involved: the certificate machinery
(M5) never even ran, because it guards the view-change path, not the decision
rule. This was the sharpest lesson of the audit: signing the *evidence* is
not the same as authenticating the *envelopes* the protocol counts.

With the authentication layer, every message must verify against the known
key of its claimed sender before any arm sees it. The forged COMMITs die at
the gate, three DROPPED lines per victim, and nobody decides anything."""
import json, socket, time
import common as c

procs = c.launch()
print(">> nobody proposes; a keyless raw socket forges 3 COMMITs for 'evil'")
print("   impersonating 7001, 7002, 7003 — at every node (2f+1 if believed)")

fake_sig = "00" * 64
for victim in c.ALL:
    for imp in ["7001", "7002", "7003"]:
        forged = json.dumps({"Commit": {
            "from": f"127.0.0.1:{imp}", "view": 0, "m": "evil", "sig": fake_sig,
        }}) + "\n"
        s = socket.create_connection(("127.0.0.1", int(victim)))
        s.sendall(forged.encode())
        s.close()

time.sleep(1.5)
logs = c.collect(procs, settle=0.5)

decided = {p: c.decisions(logs[p]) for p in c.ALL}
dropped = {p: len([l for l in logs[p] if l.startswith("DROPPED")]) for p in c.ALL}
for p in c.ALL:
    print(f"   node {p}: decided {decided[p] or 'NOTHING'}, "
          f"dropped {dropped[p]} unauthenticated message(s)")

if all(not decided[p] for p in c.ALL) and all(dropped[p] >= 3 for p in c.ALL):
    print("\n   VERDICT: FORGERY BOUNCED — every impersonated COMMIT died at the")
    print("   envelope gate; no node decided. Pre-auth, this attack fabricated a")
    print("   unanimous-looking decision out of three spoofed messages, in the")
    print("   normal case, with the certificate defenses never involved.")
else:
    print(f"\n   VERDICT: UNEXPECTED — decided={decided}, dropped={dropped}")
