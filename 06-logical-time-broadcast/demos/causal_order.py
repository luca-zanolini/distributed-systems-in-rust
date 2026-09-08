"""Causal delivery (the question/answer anomaly, repaired).

6003 processes messages ORIGINATING at 6000 only after an 800 ms delay (a slow
link, simulated). 6000 broadcasts 'question'; 6001, having delivered it,
broadcasts 'answer' — causally after. At 6003 the answer physically arrives
first; the vector-clock check parks it ('Holding'), the question arrives and is
delivered, and the drain then releases the answer. Every node ends with the same
causally-consistent order. (Under M2's Lamport clocks the same schedule
delivered answer-before-question at 6003.)"""
import common as c
import time

procs = c.launch({"6003": ["--slow-from", "127.0.0.1:6000", "800"]})
print(">> 6000 broadcasts 'question'  (6003 is slow on messages from 6000)")
c.drive(procs, "6000", "bcast question")
time.sleep(0.3)
print(">> 6001, having delivered it, broadcasts 'answer' — causally AFTER the question")
c.drive(procs, "6001", "bcast answer")
c.report(procs, settle=2.0)
print("   => watch node 6003: Holding 'answer' -> Delivered question -> Delivered answer.")
