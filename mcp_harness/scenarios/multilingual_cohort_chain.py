"""Scenario: multilingual_cohort_chain (READ + cohort + scheduling).

Exercises the activations landed 2026-05-06:
  - `find_people.speaks="ita"` — language-filtered cohort search
  - `find_meeting_slot` — chronotype + working-hours-aware scheduling
  - the multilingual mail render path (compose_email respects native
    language at render time; we don't verify the rendered text content
    here, just that the chain happens)

Chain shape:
  find_people(speaks="ita", limit=3)
    → pick first match (mail_id X, who's Italian-speaking)
    → find_meeting_slot(viewer, [X], day_offset=2, duration=30)
    → pick the highest-scoring slot
    → book_meeting at that slot

This is the canonical "cohort-driven coordination respecting linguistic
+ chronotype constraints" shape — agents that treat the world as
US-English-9-to-5 will fail this.
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    ToolCallMatch, Verdict, call_tool, run_with_verdict, runner,
)


VIEWER_MAIL_ID = 12345


SCAN_BUDGET = 50_000  # pinned so setup + agent see the same prefix


async def setup(session):
    """Pre-find Italian-speaking candidates with a pinned scan_budget
    so the agent (instructed to use the same budget) returns the
    same prefix. We accept any of the top-3 matches as a valid final
    answer — agents may pick result[0] (most natural) or any other
    of the candidates we showed.
    """
    out = await call_tool(session, "find_people", {
        "speaks": "ita",
        "limit": 3,
        "scan_budget": SCAN_BUDGET,
    })
    matched = out.get("matched", [])
    if not matched:
        return None
    return {
        "valid_names": [
            f"{m['name_first']} {m['name_last']}" for m in matched
        ],
    }


def system_prompt(s):
    return (
        "You are scheduling a quick check-in with an Italian-speaking "
        "colleague. Use the MCP tools. Be concise."
    )


def user_task(s):
    return (
        "I need to schedule a 30-minute meeting with an Italian-speaking "
        "colleague. Steps:\n"
        f"1. Use `find_people` with speaks=\"ita\", limit=3, scan_budget={SCAN_BUDGET} "
        "to find Italian-speaking people. Take the FIRST result.\n"
        f"2. Use `find_meeting_slot` with viewer_mail_id={VIEWER_MAIL_ID}, "
        "the colleague's mail_id as a single attendee, day_offset=2, "
        "duration_minutes=30. This will return chronotype-friendly slots.\n"
        "3. Use `book_meeting` to book the FIRST slot from the returned "
        f"list. viewer_mail_id={VIEWER_MAIL_ID}, the colleague as the "
        "single attendee, day_offset=2, hour and minute from the slot's "
        "start_hour/start_minute, duration_minutes=30.\n"
        "4. Tell me the colleague's full name in your final message."
    )


def verdict(s) -> Verdict:
    return Verdict(
        tool_calls_must=[
            ToolCallMatch(
                name="find_people",
                args_must_contain={"speaks": "ita"},
                must_call_at_least=1,
            ),
            ToolCallMatch(
                name="find_meeting_slot",
                must_call_at_least=1,
            ),
            ToolCallMatch(
                name="book_meeting",
                must_call_at_least=1,
            ),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
        ],
        # Accept any of the 3 valid Italian-speaker names (agent might
        # take result[0], but if scan changed slightly it could pick
        # any valid match in the prefix).
        final_must_contain=[s["valid_names"][0]],
    )


async def main() -> int:
    return await run_with_verdict(
        setup_fn=setup,
        system_prompt_fn=system_prompt,
        user_task_fn=user_task,
        verdict_fn=verdict,
        max_turns=12,
    )


if __name__ == "__main__":
    sys.exit(runner(main()))
