"""The forgery, replayed against Milestone 5's signed certificates: it bounces.

Before M5, a VIEWCHANGE carried a bare CLAIM of a prepared value, and this same
injection made all four nodes decide 'evil' — a value nobody proposed. Now a
certificate is the evidence itself: 2f+1 signatures over the canonical statement
`PREPARE:{view}:{value}`, verified against the trusted-setup public keys before
the selection rule may believe it.

The attacker here does its best: a syntactically perfect JSON ViewChange,
impersonating 7002, whose certificate exercises every defense branch at once —
a duplicate signer, an unknown signer, malformed hex, and well-formed-but-invalid
signatures. Expected outcome: the new leader logs one rejection per bad
signature, finds no verifiable prepared value, and awaits a fresh proposal.
Nobody ever decides 'evil'.

Remaining honest gap (documented in the README): the ViewChange ENVELOPE is
still unsigned — the forged message does occupy 7002's slot in the view-change
tally (identity is only as strong as the channel). What it can no longer do is
smuggle a VALUE into the protocol: values now travel only inside verifiable
certificates."""
import json, socket, time
import common as c

procs = c.launch()
print(">> BYZANTINE leader 7000 equivocates — view 0 wedges, nobody prepares anything")
c.drive(procs, "7000", "bcast equiv attack retreat")
time.sleep(1.0)

zeros = "00" * 64   # valid hex, valid length, invalid signature
forged = json.dumps({
    "ViewChange": {
        "from": "127.0.0.1:7002",
        "newview": 1,
        "prepared": {
            "view": 0,
            "value": "evil",
            "sigs": [
                ["127.0.0.1:7000", zeros],      # known signer, garbage signature
                ["127.0.0.1:7000", zeros],      # duplicate signer
                ["127.0.0.1:9999", zeros],      # unknown signer
                ["127.0.0.1:7002", "zz"],       # malformed hex
                ["127.0.0.1:7003", zeros],      # known signer, garbage signature
            ],
        },
    }
}) + "\n"
print(">> INJECTING a well-formed forged ViewChange (impersonating 7002) claiming a")
print("   certificate for (view 0, 'evil') — 5 signatures, all of them lies")
for p in c.ALL:
    s = socket.create_connection(("127.0.0.1", int(p)))
    s.sendall(forged.encode())
    s.close()

print(">> waiting out the timeout: view 1 is entered, the leader checks the evidence ...")
logs = c.collect(procs, settle=7.0)
c.report(logs, faulty={"7000"})

leader_log = logs["7001"]
rejects = [l for l in leader_log if l.startswith("cert:")]
fresh = any("no prepared value" in l for l in leader_log)
ok, vals = c.verdict_agreement(logs, faulty={"7000"})
print(f"\n   leader 7001's verification log ({len(rejects)} rejection lines):")
for l in rejects:
    print(f"      {l}")

if "evil" not in vals and rejects and fresh:
    print("\n   VERDICT: FORGERY BOUNCED — every signature in the forged certificate was")
    print("   rejected (duplicate / unknown / malformed / invalid), the selection rule")
    print("   found no verifiable prepared value, and 'evil' was never proposed, let")
    print("   alone decided. The same injection that poisoned the unauthenticated")
    print("   protocol now dies at the verify_cert boundary: certificates are evidence,")
    print("   not claims — hearsay made checkable by 2f+1 signatures.")
elif "evil" in vals:
    print("\n   VERDICT: POISONED — 'evil' was decided; verification failed to protect.")
else:
    print(f"\n   VERDICT: UNEXPECTED — decided={vals or 'none'}, "
          f"rejections={len(rejects)}, fresh-proposal log={'yes' if fresh else 'no'}")
