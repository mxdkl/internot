"""Scenario: find_then_decline (HYBRID — disambiguation + write).

Setup picks a viewer + day with multiple events. Agent is told the
name of one specific person and must decline ONLY their meeting,
leaving the other(s) untouched. Verdict checks BOTH:

- overlay_must: target event_id appears in declined_events
- overlay_must_not: other event_ids do NOT appear in declined_events
- tool_calls_must_not: rsvp must NOT be called with the wrong event_ids

This is the canonical disambiguation scenario: it's not enough to
do the right thing if the agent ALSO does the wrong thing. Both
layers of signal cover that.
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    OverlayAssertion, ToolCallMatch, Verdict, call_tool, run_with_verdict,
    runner,
)


async def setup(session):
    """Find a viewer + day with ≥ 2 events tied to different people."""
    for v_seed in [100, 1000, 4096, 8192, 12288, 16384, 20480]:
        for day in [180, 56, 14, 7, 30, 60, 90, 120]:
            sched = await call_tool(session, "get_schedule",
                                    {"viewer_mail_id": v_seed, "day_offset": day})
            evs = sched.get("events", [])
            if len(evs) < 2:
                continue
            # Pick the first as target; the rest as distractors.
            target = evs[0]
            others = evs[1:]
            target_partner = (
                target["attendee_handles"][0]
                if target["attendee_handles"]
                else target["host_handle"]
            )
            return {
                "viewer": v_seed,
                "day": day,
                "target_event_id": target["event_id"],
                "target_partner": target_partner,
                "other_event_ids": [e["event_id"] for e in others],
            }
    return None


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to manage "
        "your calendar. Read carefully — only act on what's asked."
    )


def user_task(s):
    return (
        f"Look at day_offset={s['day']} on my calendar. Decline ONLY "
        f"the meeting where {s['target_partner']} is involved. Leave "
        "every other meeting on this day alone — do not RSVP on them "
        "at all. Do NOT send any emails, DMs, or replies. Then answer "
        "me directly in your final chat message saying which event "
        "you declined."
    )


def verdict(s) -> Verdict:
    return Verdict(
        overlay_must=[
            OverlayAssertion(
                field="declined_events",
                must_contain=[s["target_event_id"]],
                expected_count=1,  # exactly one decline; no extras
            ),
        ],
        overlay_must_not=[
            OverlayAssertion(
                field="declined_events",
                must_not_contain=s["other_event_ids"],
            ),
            # Tentative/Accept on any of these would also be wrong.
            OverlayAssertion(
                field="accepted_events",
                must_not_contain=[s["target_event_id"]] + s["other_event_ids"],
            ),
            OverlayAssertion(
                field="tentative_events",
                must_not_contain=[s["target_event_id"]] + s["other_event_ids"],
            ),
        ],
        # Belt-and-suspenders: assert rsvp wasn't called on the wrong
        # ids, AND no stray sends (the agent shouldn't email/DM/reply
        # as a side-effect of "decline this meeting").
        tool_calls_must_not=[
            ToolCallMatch(
                name="rsvp",
                args_must_contain={"event_id": eid},
            )
            for eid in s["other_event_ids"]
        ] + [
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
        ],
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
