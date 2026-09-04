# Byzantine reliable broadcast — demos

Each script launches a 4-node cluster (`n = 4`, `f = 1`) of the compiled binary and drives the
designated sender over its standard input. Build first, then run any script:

```bash
cargo build
python3 demos/honest.py
python3 demos/equivocation.py
python3 demos/fault_tolerance.py
```

| Script | What it demonstrates |
|---|---|
| `honest.py` | **Validity + totality.** A correct sender broadcasts; every correct process delivers the same value. |
| `equivocation.py` | **Consistency (BCB4/BRB4).** A Byzantine sender tells one half of the cluster `attack` and the other half `retreat`; the correct processes never split — they all deliver one value, or all deliver nothing. Contrast the naive M1 broadcast, which split into `attack`/`retreat`/`retreat`. |
| `fault_tolerance.py` | **`n > 3f`.** One node is down (a crash is the mildest Byzantine fault); the remaining correct processes still reach the echo quorum of 3 and all deliver. |

The scripts share `common.py` (launch a cluster, drive the sender's stdin, collect deliveries).
They drive stdin through `subprocess` pipes rather than a shell pipeline, because a backgrounded
shell pipeline does not reliably deliver standard input to the sender process. Nodes log protocol
events to standard error (unbuffered), which the harness captures.

**A note on the equivocation outcome.** Which consistent outcome occurs — all correct processes
deliver the same value, or none deliver — depends on where the Byzantine sender's own echo lands
in the message race, and both are correct: consistency forbids only a *split*. The script prints
an explicit verdict.
