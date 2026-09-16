"""M3b — the OR-Set showdown: the three abilities that justify the tags.

Scene 1: remove then RE-ADD works (a 2P-Set is stuck at false forever).
Scene 2: a stale replica's gossip cannot resurrect a removed element.
Scene 3: concurrent remove-vs-add heals to ADD WINS on both replicas.
"""

from common import build, run, verdict

build()

out = run("or_set")
print(out)

verdict("a RE-ADDS banana:   a contains banana? true" in out,
        "re-add after remove works — the fresh tag is unknown to every tombstone")
verdict("stale b gossips in: a contains banana? false" in out,
        "stale gossip bounced — old tags hit the tombstones, no resurrection")
verdict("a: true, b: true" in out,
        "concurrent remove vs add healed to ADD WINS on both replicas")
