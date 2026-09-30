"""Regression test: exercise verify_solver on three paths — happy,
substrate-reject, and verdict-fail. No OpenAI calls; just MCP
substrate. Run: `uv run test_verify_solver.py`.

The substrate-reject case is the bug class that motivated solver
verification (book_meeting hour-only made several scenarios'
canonical answers structurally unreachable). If verify_solver stops
catching this, that bug class is back."""
from __future__ import annotations

import asyncio
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from harness import call_tool, verify_solver
from scenarios.decline_day import setup, verdict, solver as good_solver


async def bad_solver_substrate_reject(session, s):
    """Deliberately call book_meeting with an out-of-range hour. The
    substrate should reject it; verify_solver should report this."""
    await call_tool(session, "book_meeting", {
        "viewer_mail_id": s["viewer"],
        "attendee_mail_ids": [s["viewer"]],
        "day_offset": s["day"],
        "hour": 99,
        "duration_minutes": 30,
    })


async def bad_solver_verdict_fail(session, s):
    """Performs no actions. The verdict expects N declines so it'll
    fail."""
    return


async def main() -> int:
    print("[smoke] testing happy path (good solver)…")
    ok, reason = await verify_solver(
        setup_fn=setup, solver_fn=good_solver, verdict_fn=verdict,
    )
    print(f"  result: ok={ok} reason={reason!r}")
    if not ok:
        print("  EXPECTED True; got False")
        return 1

    print("[smoke] testing substrate-reject path (bad solver)…")
    ok, reason = await verify_solver(
        setup_fn=setup, solver_fn=bad_solver_substrate_reject,
        verdict_fn=verdict,
    )
    print(f"  result: ok={ok} reason={reason!r}")
    if ok:
        print("  EXPECTED False; got True")
        return 1

    print("[smoke] testing verdict-fail path (no-op solver)…")
    ok, reason = await verify_solver(
        setup_fn=setup, solver_fn=bad_solver_verdict_fail, verdict_fn=verdict,
    )
    print(f"  result: ok={ok} reason={reason!r}")
    if ok:
        print("  EXPECTED False; got True")
        return 1

    print("\n[smoke] all 3 cases behaved as expected")
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
