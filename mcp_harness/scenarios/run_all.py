"""Run every scenario in this directory and report a summary table.

Each scenario is a sibling Python script that exits 0 on PASS and
non-zero on FAIL or setup failure (2). We invoke them via
`uv run` (or python3 -m if uv isn't present) to share the parent
venv but isolate each scenario's process so MCP subprocess state
doesn't leak.
"""
from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent

SCENARIOS = [
    "find_file_in_email.py",
    "find_top_task.py",
    "biggest_subscription.py",
    "decline_day.py",
    "dm_a_coworker.py",
    "find_then_decline.py",
]


def main() -> int:
    results: list[tuple[str, int, float]] = []
    for name in SCENARIOS:
        print(f"\n{'═' * 70}\n  {name}\n{'═' * 70}", flush=True)
        t0 = time.time()
        proc = subprocess.run(
            ["uv", "run", "python", str(HERE / name)],
            cwd=HERE.parent,
        )
        elapsed = time.time() - t0
        results.append((name, proc.returncode, elapsed))

    print(f"\n{'═' * 70}\n  Summary\n{'═' * 70}")
    print(f"{'scenario':<32} {'result':<10} {'elapsed':>10}")
    print("-" * 54)
    pass_count = 0
    for name, rc, elapsed in results:
        result = (
            "PASS" if rc == 0
            else "SKIP" if rc == 2
            else f"FAIL({rc})"
        )
        if rc == 0:
            pass_count += 1
        print(f"{name:<32} {result:<10} {elapsed:>9.1f}s")
    print(f"\n{pass_count}/{len(results)} scenarios passed")
    return 0 if pass_count == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
