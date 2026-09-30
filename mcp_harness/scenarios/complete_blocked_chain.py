"""Scenario: complete_blocked_chain (ADVERSARIAL-HARD; gpt-5.4-mini 3/3 fail).

Tested 2026-04-28 against gpt-5.4-mini, n=3 trials, all FAIL with
IDENTICAL pattern across trials: agent misses task 229376 (the
chain root) and over-marks 2 unrelated tasks (229380, 229404).
Identical-across-trials suggests deterministic model failure
rather than sampling noise.

Hypothesis CONFIRMED. Establishes TOPOLOGICAL-ORDER as the 7th
characterized failure mode in the gym's taxonomy. Mode definition:
when an agent must execute N actions in a partial order (some must
complete before others), it executes them in list/iteration order
rather than dependency order, AND drops the topological root from
its action plan.



Hypothesis: when an agent must execute N actions in a partial order
(some actions depend on others completing first), it will execute
them in list/iteration order rather than dependency order. The
substrate doesn't enforce that mark_done requires unblocking — the
agent must enforce the topology itself.

Concretely: pick a viewer with ≥2 currently-blocked Open/InProgress
tasks. Tell the agent: for every blocked task, mark its blockers
done first, then mark the task done. The set of mark_done calls
must include both the blocked tasks AND their blockers, ordered so
each blocker precedes anything it blocks.

Failure modes:
  1. mark_done called on a blocked task BEFORE its blocker is
     marked done (topological violation — the predicted failure).
  2. A blocker is never marked done.
  3. A blocked task is never marked done.
  4. mark_done called on a task that's neither in the blocked set
     nor a blocker (over-action).

Substrate scope: tasks only. Tests order-dependent reasoning
without cross-service composition — isolating the topological-
ordering failure mode.

NOT YET VALIDATED against any model — authored substrate-only.
Run with $0.50 of API budget when available.
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    LoopRunner, OverlayAssertion, ToolCallMatch, Verdict, call_tool,
    evaluate, mcp_session, report, runner,
)


CANDIDATE_VIEWERS = [56, 67, 61, 72, 70, 85] + list(range(50, 250))
MIN_BLOCKED_OPEN = 2


async def setup(session):
    """Find a viewer with ≥2 currently-blocked Open/InProgress tasks.
    For each blocked task, fetch its blockers via read_task. Build
    the canonical (blocker → blocked) edges so the verdict can
    enforce topological order."""
    chosen = None
    for v in CANDIDATE_VIEWERS:
        out = await call_tool(session, "list_tasks",
                              {"viewer_mail_id": v, "limit": 100})
        tasks = out.get("tasks", [])
        blocked_open = [t for t in tasks
                        if t.get("is_blocked")
                        and t["status"] in ("Open", "InProgress")]
        if len(blocked_open) >= MIN_BLOCKED_OPEN:
            chosen = (v, blocked_open)
            break
    if chosen is None:
        return None
    viewer, blocked_open = chosen

    # Per-task blocker enumeration via read_task (the blocked_by
    # field). Some blockers might be Done already and don't need to
    # be re-marked; the SET we expect mark_done to cover is:
    # (blocked tasks) ∪ (any blockers that are not already Done).
    must_mark = set()
    edges = []  # (blocker_id, blocked_id) — every blocker must precede its blocked
    for t in blocked_open:
        rt = await call_tool(session, "read_task",
                             {"viewer_mail_id": viewer, "task_id": t["task_id"]})
        must_mark.add(t["task_id"])
        for b_id in rt.get("blocked_by", []):
            # Only blockers that aren't already Done need mark_done
            # to satisfy the "mark blockers first" rule.
            b_read = await call_tool(session, "read_task",
                                     {"viewer_mail_id": viewer, "task_id": b_id})
            if b_read["status"] not in ("Done", "Cancelled"):
                must_mark.add(b_id)
                edges.append((b_id, t["task_id"]))

    return {
        "viewer": viewer,
        "blocked_task_ids": [t["task_id"] for t in blocked_open],
        "must_mark": sorted(must_mark),
        "edges": edges,
    }


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools. When one "
        "task depends on another, you must complete the prerequisite "
        "before the dependent task — order matters."
    )


def user_task(s):
    return (
        "Look at my task list (use list_tasks with limit=100). For "
        "every task with is_blocked=true that is currently Open or "
        "InProgress:\n"
        "\n"
        "  STEP 1: read_task on it to find its blocked_by list (the "
        "task_ids of its prerequisites).\n"
        "  STEP 2: for each blocker that is not already Done, "
        "mark_done it FIRST — blockers must finish before the task "
        "they block.\n"
        "  STEP 3: only THEN mark_done the originally-blocked task.\n"
        "\n"
        "Order matters: every blocker mark_done call must come BEFORE "
        "the call for the task it blocks. Do not mark_done any task "
        "that isn't in this set (the blocked tasks plus their "
        "outstanding prerequisites). Do not send emails, DMs, or "
        "replies."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print(f"[setup] no viewer with ≥{MIN_BLOCKED_OPEN} actionable "
                  "blocked tasks", file=sys.stderr)
            return 2

        print(f"[setup] viewer={scenario['viewer']} "
              f"blocked_count={len(scenario['blocked_task_ids'])} "
              f"must_mark_count={len(scenario['must_mark'])} "
              f"edges={len(scenario['edges'])}", file=sys.stderr)

        loop = LoopRunner(session)
        run = await loop.run(
            system_prompt(scenario),
            user_task(scenario),
            max_turns=40,
        )
        trace = await call_tool(session, "_get_trace", {})
        if not isinstance(trace, dict):
            trace = {}

    # The trace has the SET of marked_done message_ids (note: tasks's
    # mark_done flag uses task_id directly; trace.marked_done is the
    # task_id list). But order matters here, so consult run.tool_calls
    # for the chronological sequence.
    mark_done_call_ids: list[str] = []
    for tc in run.tool_calls:
        if tc.name == "mark_done":
            tid = tc.arguments.get("task_id")
            if tid:
                mark_done_call_ids.append(tid)

    failures: list[str] = []

    must_mark_set = set(scenario["must_mark"])
    actual_set = set(mark_done_call_ids)

    missing = must_mark_set - actual_set
    if missing:
        failures.append(
            f"never marked done {len(missing)} required tasks: "
            f"{sorted(missing)[:3]}{'...' if len(missing) > 3 else ''}"
        )

    extra = actual_set - must_mark_set
    if extra:
        failures.append(
            f"marked done {len(extra)} tasks not in the required set "
            f"(over-action): {sorted(extra)[:3]}"
        )

    # Topological order: for every (blocker, blocked) edge, the
    # blocker's mark_done call must appear BEFORE the blocked task's.
    # Use the index-of-first-occurrence in the call sequence.
    index_of: dict[str, int] = {}
    for i, tid in enumerate(mark_done_call_ids):
        index_of.setdefault(tid, i)

    for blocker, blocked in scenario["edges"]:
        b_idx = index_of.get(blocker)
        d_idx = index_of.get(blocked)
        if b_idx is None or d_idx is None:
            # Already flagged above as missing.
            continue
        if b_idx >= d_idx:
            failures.append(
                f"topological violation: {blocked} marked done at "
                f"position {d_idx} BEFORE its blocker {blocker} at "
                f"position {b_idx}"
            )

    print()
    print("[constraints]")
    if not failures:
        print("  ✓ all constraints satisfied")
    else:
        for f in failures:
            print(f"  ✗ {f}")

    verdict = Verdict(
        overlay_must=[
            OverlayAssertion(
                field="marked_done",
                must_contain=scenario["must_mark"],
                expected_count=len(scenario["must_mark"]),
            ),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
            ToolCallMatch(name="rsvp"),
            ToolCallMatch(name="book_meeting"),
        ],
    )
    scorecard = evaluate(verdict, run, trace, max_turns=40)
    if failures:
        scorecard.correctness = False
    return report(run, scorecard)


if __name__ == "__main__":
    sys.exit(runner(main()))
