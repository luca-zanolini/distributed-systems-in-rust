"""What causal broadcast does NOT give: a total order.

6000 and 6002 broadcast concurrently (neither has delivered the other's message:
no causal relation). Causal delivery imposes no constraint between them, so
different nodes may deliver 'ping' and 'pong' in different orders — legitimately.
Compare their V vectors: both end identical, but the interleaving differs.
Agreeing on ONE order for causally unrelated messages is total-order broadcast —
equivalent to consensus (Module 07), and this module's planned second half."""
import common as c

procs = c.launch()
print(">> 6000 broadcasts 'ping' and 6002 broadcasts 'pong' CONCURRENTLY")
c.drive(procs, "6000", "bcast ping")
c.drive(procs, "6002", "bcast pong")
c.report(procs)
print("   => causally unrelated messages may be delivered in different orders per node;")
print("      causal broadcast is silent about them. One agreed order = total-order broadcast.")
