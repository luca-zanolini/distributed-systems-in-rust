"""The attack that motivates signatures: one forged VIEWCHANGE poisons the
view change, and four correct nodes unanimously decide a value NOBODY proposed.

Our VIEWCHANGE carries a bare CLAIM of a prepare certificate — `(view, value)`
with no evidence — and the sender identity is a self-declared string. This
script exploits both gaps at once: a raw socket (not a node at all) injects

    VIEWCHANGE 127.0.0.1:7002 1 0 evil

into every node: an impersonation of 7002 claiming a certificate for value
'evil' in view 0 that never existed. When the real timers fire and view 1 is
entered, the new leader's (correct!) max-prepared-view selection adopts the
forged claim and re-proposes 'evil'; the (correct!) quorum machinery then
amplifies the lie into a unanimous decision. Even the impersonated 7002
decides it — its genuine VIEWCHANGE arrived after the forgery and lost
first-testimony-wins.

Strong validity (CCGR Module 5.11, BC2) is violated on screen. Real PBFT
closes the gap by making the view-change message carry the certificate itself
— 2f+1 SIGNED prepares — so the leader verifies instead of believes. That is
Milestone 5."""
import socket, time
import common as c

procs = c.launch()
print(">> BYZANTINE leader 7000 equivocates — view 0 wedges, nobody prepares anything")
c.drive(procs, "7000", "bcast equiv attack retreat")
time.sleep(1.0)

forged = "VIEWCHANGE 127.0.0.1:7002 1 0 evil\n"
print(f">> INJECTING forged message into every node: {forged.strip()!r}")
print("   (sent by a raw Python socket impersonating 7002 — no such certificate")
print("    was ever formed; 'evil' has never been proposed by any process)")
for p in c.ALL:
    s = socket.create_connection(("127.0.0.1", int(p)))
    s.sendall(forged.encode())
    s.close()

print(">> waiting out the timeout: view 1 is entered, the leader consults the claims ...")
logs = c.collect(procs, settle=7.0)
c.report(logs, faulty={"7000"})

adopted = any("adopting prepared value" in l for l in logs["7001"])
ok, vals = c.verdict_agreement(logs, faulty={"7000"})
if vals == {"evil"}:
    print("\n   VERDICT: POISONED — the cluster unanimously decided 'evil', a value no")
    print("   process ever proposed, planted by one 35-byte forged message."
          + (" The leader" if adopted else ""))
    if adopted:
        print("   dutifully 'adopted the prepared value from view 0' — every correct")
        print("   mechanism (selection rule, quorums) worked exactly as designed and")
        print("   faithfully amplified the lie.")
    print("   Moral: the view change makes the leader TRUST a certificate, so the")
    print("   certificate must be UNFORGEABLE (2f+1 signed prepares) — or the trust")
    print("   is a weapon. First-hand votes need no signatures; forwarded evidence")
    print("   (hearsay) does. This is Milestone 5's reason to exist.")
else:
    print(f"\n   VERDICT: UNEXPECTED — decided values: {vals or 'none'}")
