"""Reliable broadcast under a crashing sender (RB4 agreement).

Node 6000 broadcasts and is killed almost immediately — mid-fan-out. The eager
relay layer (every process re-broadcasts on first delivery) carries the message
to everyone anyway: all surviving correct processes deliver."""
import common as c
import time

procs = c.launch()
print(">> 6000 broadcasts 'hello' and is killed ~50ms later (crash mid-broadcast)")
c.drive(procs, "6000", "bcast hello")
time.sleep(0.05)
procs["6000"].kill()
c.report(procs, dead=("6000",))
print("   => every surviving process delivered: agreement despite the sender's crash (RB4).")
