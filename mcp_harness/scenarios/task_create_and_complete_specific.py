"""Scenario: task_create_and_complete_specific (TARGETED — multi-mutation state tracking).

Hypothesis: when the agent makes N similar create calls and then
must operate on ONE SPECIFIC result by identity (not position or
recency), it loses track of which returned id corresponds to which
input. The tasks substrate is well-suited to test this — `create_task`
returns a task_id, and `mark_done` requires that exact id.

Concretely: agent must
  1. create 3 tasks with titles A, B, C (due in 3 days, priority Medium)
  2. mark task B (specifically) as Done

Failure modes:
  1. mark_done called with task A's id or task C's id (wrong task_id).
  2. mark_done called multiple times (over-action).
  3. mark_done not called at all.
  4. Wrong number of tasks created.

Substrate scope: tasks only. Tests state-tracking across multiple
mutations of the same shape — a common pattern in batch operations.
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    LoopRunner, OverlayAssertion, ToolCallMatch, Verdict, call_tool,
    evaluate, mcp_session, report, runner,
)


VIEWER = 100
DUE_DAY_OFFSET = 188  # 8 days from fixed_now (day 180)
TITLES = ["Draft Q3 plan", "Review architecture", "Schedule offsite"]
TARGET_TITLE = TITLES[1]  # Mark the MIDDLE one done — not first, not last


async def setup(session):
    """No discovery needed — substrate accepts any create_task call.
    Just packages the constants for the prompt."""
    return {
        "viewer": VIEWER,
        "due_day_offset": DUE_DAY_OFFSET,
        "titles": TITLES,
        "target_title": TARGET_TITLE,
    }


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools. When you "
        "create multiple things and then operate on a specific one, "
        "track the returned ids carefully."
    )


def user_task(s):
    titles_str = "\n".join(f"  - {t}" for t in s["titles"])
    return (
        f"Two things to do:\n"
        "\n"
        f"  STEP 1: Create {len(s['titles'])} tasks for me, all "
        f"assigned to me (mail_id {s['viewer']}), all due "
        f"day_offset={s['due_day_offset']}, all priority Medium. "
        "Create them with these exact titles, in this order:\n"
        f"{titles_str}\n"
        "\n"
        f"  STEP 2: Mark the task with title '{s['target_title']}' as "
        "Done. Use mark_done with the task_id you got back from STEP "
        "1's create_task call for that title.\n"
        "\n"
        "Do not send emails, DMs, or replies. Tell me the task_id of "
        "the one you marked done."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)

        print(f"[setup] viewer={scenario['viewer']} target_title="
              f"{scenario['target_title']!r}", file=sys.stderr)

        loop = LoopRunner(session)
        run = await loop.run(
            system_prompt(scenario),
            user_task(scenario),
            max_turns=15,
        )
        trace = await call_tool(session, "_get_trace", {})
        if not isinstance(trace, dict):
            trace = {}

    # Inspect tool calls to map title → task_id from create_task
    # results, then check that mark_done was called with the right id.
    title_to_id: dict[str, str] = {}
    for tc in run.tool_calls:
        if tc.name != "create_task":
            continue
        try:
            import json
            res = json.loads(tc.result_text)
            if not res.get("ok"):
                continue
            title = tc.arguments.get("title")
            tid = res.get("task_id")
            if title and tid:
                title_to_id[title] = tid
        except Exception:
            continue

    mark_done_calls = [tc for tc in run.tool_calls if tc.name == "mark_done"]
    mark_done_ids = [tc.arguments.get("task_id") for tc in mark_done_calls]

    failures: list[str] = []

    if len(title_to_id) != len(scenario["titles"]):
        failures.append(
            f"created {len(title_to_id)} distinct titled tasks; "
            f"expected {len(scenario['titles'])}"
        )

    missing_titles = set(scenario["titles"]) - set(title_to_id.keys())
    if missing_titles:
        failures.append(f"never created: {sorted(missing_titles)}")

    expected_target_id = title_to_id.get(scenario["target_title"])

    if len(mark_done_calls) == 0:
        failures.append("mark_done was never called")
    elif len(mark_done_calls) > 1:
        failures.append(
            f"mark_done called {len(mark_done_calls)} times (over-action)"
        )
    elif expected_target_id is None:
        # Already flagged above as a missing create.
        pass
    else:
        if mark_done_ids[0] != expected_target_id:
            # Try to identify which task got marked instead.
            mismatched_title = None
            for title, tid in title_to_id.items():
                if tid == mark_done_ids[0]:
                    mismatched_title = title
                    break
            failures.append(
                f"marked the wrong task done — expected "
                f"'{scenario['target_title']}' (task_id="
                f"{expected_target_id}), got task_id="
                f"{mark_done_ids[0]} which is "
                f"{('title=' + repr(mismatched_title)) if mismatched_title else 'not one of the created tasks'}"
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
            OverlayAssertion(field="created_tasks",
                             expected_count=len(scenario["titles"])),
            OverlayAssertion(field="marked_done", expected_count=1),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
            ToolCallMatch(name="rsvp"),
        ],
    )
    scorecard = evaluate(verdict, run, trace, max_turns=15)
    if failures:
        scorecard.correctness = False
    return report(run, scorecard)


if __name__ == "__main__":
    sys.exit(runner(main()))
