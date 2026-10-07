#!/usr/bin/env python3
"""A/B gate for the society world (internot_society::mono): run two
mono_report binaries alternately
(MULT=1, then MULT=1000 BOUNDS_ONLY=1), take medians over rounds, and print
each metric side by side with the change. Usage: ab.py BASE NEW [ROUNDS]."""
import re, statistics, subprocess, sys, os

base, new = sys.argv[1], sys.argv[2]
rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 3

def run(binary, env):
    e = dict(os.environ); e.update(env)
    return subprocess.run([binary], env=e, capture_output=True, text=True, timeout=1800).stdout

def metrics(out, tag):
    m = {}
    for line in out.splitlines():
        if (g := re.search(r"build ([\d.]+) s; \+(\d+) MB", line)):
            m[f"{tag} build s"] = float(g[1]); m[f"{tag} MB"] = float(g[2])
        if (g := re.search(r"alive (\S+): (\d+) \(.*\), ([\d.]+) ms", line)):
            m[f"{tag} count {g[1]} ms"] = float(g[3]); m[f"{tag} count {g[1]} =" ] = g[2]
        if (g := re.search(r"alive (\S+): between .*?, ([\d.]+) ms", line)):
            m[f"{tag} bounds {g[1]} ms"] = float(g[2])
        if (g := re.search(r"enumerated \d+ in [\d.]+ s on 1 thread \(([\d.]+) ns", line)):
            k = f"{tag} enum ns/person"
            m.setdefault(k + " #", 0); m[k + " #"] += 1
            m[k + str(m[k + " #"])] = float(g[1])
        if (g := re.search(r"^\s+(\w+): p50 ([\d.]+) µs, p99 ([\d.]+) µs, mean ([\d.]+) µs \((\w+)\)", line)):
            m[f"{tag} {g[1]} p50"] = float(g[2]); m[f"{tag} {g[1]} p99"] = float(g[3]); m[f"{tag} {g[1]} answers"] = g[5]
    return {k: v for k, v in m.items() if not k.endswith(" #")}

runs = {"base": [], "new": []}
for r in range(rounds):
    for name, b in (("base", base), ("new", new)) if r % 2 == 0 else (("new", new), ("base", base)):
        m = metrics(run(b, {"MULT": "1"}), "x1")
        m.update(metrics(run(b, {"MULT": "1000", "BOUNDS_ONLY": "1"}), "x1000"))
        runs[name].append(m)

keys = [k for k in runs["base"][0] if k in runs["new"][0]]
fresh = [k for k in runs["new"][0] if k not in runs["base"][0]]
print(f"{'metric':40} {'base':>14} {'new':>14} {'change':>8}")
for k in keys:
    a = [x[k] for x in runs["base"]]; b = [x[k] for x in runs["new"]]
    if isinstance(a[0], str):
        print(f"{k:40} {a[0]:>14} {b[0]:>14} {'same' if a[0] == b[0] else 'DIFF':>8}")
    else:
        ma, mb = statistics.median(a), statistics.median(b)
        ch = (mb - ma) / ma * 100 if ma else 0.0
        print(f"{k:40} {ma:>14.3f} {mb:>14.3f} {ch:>+7.1f}%")

for k in fresh:
    b = [x[k] for x in runs["new"]]
    print(f"{k:40} {'—':>14} {(b[0] if isinstance(b[0], str) else statistics.median(b)):>14} {'new':>8}")
