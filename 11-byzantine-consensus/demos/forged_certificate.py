"""The forged-certificate attack, upgraded to its strongest form: an INSIDER.

History of this demo, which is the module's story in miniature:
- Unauthenticated protocol (pre-M5): a keyless raw socket claimed a prepared
  value in a VIEWCHANGE and all four nodes decided 'evil'.
- M5 signed the certificates: the same keyless injection died at verify_cert.
- Envelope authentication (this version): a keyless attacker cannot even get
  a message PAST THE GATE any more — so the only remaining forger is an
  INSIDER: a real, key-holding Byzantine node. Node 7002 uses its `fakecert`
  command to broadcast a VIEWCHANGE inside a perfectly valid signed envelope,
  claiming a certificate for 'evil' whose inner signatures are garbage.

The envelope gate passes it (the envelope IS genuine — 7002 really said
this). Identity was never the problem here; the LIE is in the forwarded
evidence. Only certificate verification can catch that — and does: the new
leader rejects every inner signature and finds no verifiable prepared value.
Defense in depth, each layer catching exactly what it can see:
    gate = who speaks;  verify_cert = whether the hearsay checks out."""
import time
import common as c

procs = c.launch()
print(">> BYZANTINE leader 7000 equivocates — view 0 wedges")
c.drive(procs, "7000", "bcast equiv attack retreat")
time.sleep(1.0)
print(">> INSIDER 7002 broadcasts a validly-signed VIEWCHANGE claiming a forged")
print("   certificate for 'evil' (garbage inner signatures)")
c.drive(procs, "7002", "fakecert evil")

print(">> waiting out the timeout: view 1 is entered, the leader checks the evidence ...")
logs = c.collect(procs, settle=7.0)
c.report(logs, faulty={"7000", "7002"})

leader_log = logs["7001"]
rejects = [l for l in leader_log if l.startswith("cert:")]
fresh = any("expected None" in l for l in leader_log)   # entry with no forced value
ok, vals = c.verdict_agreement(logs, faulty={"7000", "7002"})
print(f"\n   leader 7001's verification log ({len(rejects)} rejection lines):")
for l in rejects[:6]:
    print(f"      {l}")

if "evil" not in vals and rejects and fresh:
    print("\n   VERDICT: FORGERY BOUNCED — the insider's envelope was genuine, so the")
    print("   gate rightly passed it; the lie lived in the forwarded evidence, and")
    print("   verify_cert rejected every inner signature. No verifiable prepared")
    print("   value; 'evil' never decided. Envelope auth stops outsiders and")
    print("   impersonation; certificate verification stops lying insiders. Both")
    print("   layers are necessary; neither alone is sufficient.")
elif "evil" in vals:
    print("\n   VERDICT: POISONED — 'evil' was decided; verification failed to protect.")
else:
    print(f"\n   VERDICT: UNEXPECTED — decided={vals or 'none'}, "
          f"rejections={len(rejects)}, fresh-proposal log={'yes' if fresh else 'no'}")
