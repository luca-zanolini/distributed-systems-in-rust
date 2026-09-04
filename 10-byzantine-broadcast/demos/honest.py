"""Validity + totality (happy path): a correct sender broadcasts, and EVERY correct
process delivers the same value."""
import common as c

procs = c.launch(c.ALL)
print(">> correct sender 6000 broadcasts 'hello'")
c.drive(procs, "6000", "bcast hello")
delivered = c.collect(procs)
c.report(delivered)
print("   => all correct processes delivered 'hello' (BRB1 validity, BRB5 totality).")
