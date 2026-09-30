"""Scenario: book_after_existing_meetings (ADVERSARIAL-HARD; gpt-5.4-mini 3/3 fail).

Tested 2026-04-28 against gpt-5.4-mini, n=3 trials, all FAIL.
Failure mode is precise and consistent across all trials.

Original hypothesis: max-of-sequence confusion (latest-starting vs
latest-ending). FALSIFIED — every trial identified latest_end=15:30
correctly. The model handles max-of-sequence over `ends_at` reliably.

Actual failure mode (NEW pattern not seen before): ARITHMETIC OFFSET
LOSS. Every trial books at 16:00 instead of 16:30. The agent
identifies the source value (latest_end=15:30) correctly, then skips
or shortens the +1hr transformation — likely substituting a
plausible-sounding "right after" rule for the explicit "+1 hour"
instruction. The model maintains the correct anchor in working
memory but loses the operator applied to it.

Distinct from STATE-DIVERGENCE (decline_then_book) where the agent
mis-models how an action affects state. Distinct from FAN-OUT
(schedule_1on1s, etc.) where multiple bookings conflict. Here it's a
single booking with a single derived parameter — and the derivation
is what fails.

Suggests a fifth characterized failure mode for the gym: derived
quantities are unreliable. Even simple `latest_end + 1h` arithmetic
across tool calls degrades, even though the model handles
`meeting_day - 1` (in `meeting_prep`) reliably. Hypothesis worth
testing in follow-up scenarios: model is reliable on integer
day-arithmetic but unreliable on hour/minute-arithmetic.


Concretely: agent must find the meeting on day 185 with the
LATEST end time, then book a 30-min meeting with mail_id 27 starting
exactly 1 hour after that meeting ends, respecting X's free
intervals and the 9:00-17:00 window.

Failure modes:
  1. Agent picks wrong "latest" (latest-starting instead of
     latest-ending).
  2. Agent skips the 1-hour offset.
  3. Booking conflicts with X's existing schedule.
  4. Booking falls outside the window.

Substrate scope: calendar only.
"""
from __future__ import annotations

import sys
from datetime import datetime, timedelta
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from _helpers import is_free, parse_iso  # noqa: E402
from harness import (  # noqa: E402
    LoopRunner, OverlayAssertion, ToolCallMatch, Verdict, call_tool,
    evaluate, mcp_session, report, runner,
)


VIEWER = 100
COWORKER_X = 27
DURATION = 30
OFFSET_HOURS = 1
WINDOW_START = 9
WINDOW_END = 17
DAYS_TO_TRY = [183, 188, 182, 184, 185, 181, 186, 187]


async def setup(session):
    for day in DAYS_TO_TRY:
        sched = await call_tool(session, "get_schedule",
                                {"viewer_mail_id": VIEWER, "day_offset": day})
        events = sched.get("events", [])
        if not events:
            continue

        latest_end = max(parse_iso(ev["ends_at"]) for ev in events)
        expected_start = latest_end + timedelta(hours=OFFSET_HOURS)
        expected_end = expected_start + timedelta(minutes=DURATION)

        base = datetime(2025, 1, 1) + timedelta(days=day)
        win_end = base + timedelta(hours=WINDOW_END)
        if expected_end > win_end:
            continue
        if expected_start.minute not in (0, 30):
            continue  # offset doesn't land on a 30-min boundary

        x_busy = (await call_tool(session, "get_busy",
                                   {"mail_id": COWORKER_X, "day_offset": day}))["busy_intervals"]
        if not is_free(x_busy, expected_start, expected_end):
            continue

        return {
            "viewer": VIEWER,
            "day": day,
            "coworker_x": COWORKER_X,
            "expected_start_iso": expected_start.isoformat() + "Z",
            "expected_end_iso": expected_end.isoformat() + "Z",
            "latest_existing_end_iso": latest_end.isoformat() + "Z",
            "events_count": len(events),
            "x_busy": x_busy,
        }
    return None


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools. Read the "
        "request carefully — the offset is from the LATEST event's "
        "END time, not its start time."
    )


def user_task(s):
    day = s['day']
    return (
        f"On day_offset={day}, look at my schedule. Find the "
        "meeting with the LATEST END TIME (the one that finishes "
        "last, regardless of when it starts).\n"
        "\n"
        f"Then book a {DURATION}-minute meeting with mail_id "
        f"{s['coworker_x']} that starts exactly {OFFSET_HOURS} hour "
        "after that latest-ending meeting ends.\n"
        "\n"
        "Constraints:\n"
        f"  - the new meeting must be fully within {WINDOW_START}:00 "
        f"and {WINDOW_END}:00 UTC\n"
        f"  - mail_id {s['coworker_x']} must be free during the slot "
        "(use get_busy)\n"
        "  - start on a 30-min boundary\n"
        "\n"
        "Tell me the latest existing meeting's end time and the new "
        "meeting's start time. Do not send emails, DMs, or replies."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print("[setup] day's schedule lacks feasible slot for "
                  "max-end + 1hr", file=sys.stderr)
            return 2

        print(f"[setup] viewer={scenario['viewer']} day={scenario['day']} "
              f"events={scenario['events_count']} "
              f"latest_end={scenario['latest_existing_end_iso']} "
              f"expected_start={scenario['expected_start_iso']}",
              file=sys.stderr)

        loop = LoopRunner(session)
        run = await loop.run(
            system_prompt(scenario),
            user_task(scenario),
            max_turns=20,
        )
        trace = await call_tool(session, "_get_trace", {})
        if not isinstance(trace, dict):
            trace = {}

    bookings = []
    for tc in run.tool_calls:
        if tc.name != "book_meeting":
            continue
        try:
            import json
            res = json.loads(tc.result_text)
            if not res.get("ok"):
                continue
            bookings.append({
                "starts_at": res["starts_at"],
                "ends_at": res["ends_at"],
                "attendee_mail_ids": tc.arguments.get("attendee_mail_ids", []),
            })
        except Exception:
            continue

    failures: list[str] = []
    if len(bookings) != 1:
        failures.append(f"expected 1 booking, got {len(bookings)}")

    if bookings:
        b = bookings[0]
        actual_start = parse_iso(b["starts_at"])
        expected_start = parse_iso(scenario["expected_start_iso"])

        if actual_start != expected_start:
            failures.append(
                f"booking start {actual_start.isoformat()} != "
                f"expected {expected_start.isoformat()} (latest event "
                f"ends at {scenario['latest_existing_end_iso']}, plus "
                f"{OFFSET_HOURS}hr)"
            )
        actual_end = parse_iso(b["ends_at"])
        if (actual_end - actual_start).total_seconds() / 60 != DURATION:
            failures.append(f"duration != {DURATION}")
        if scenario["coworker_x"] not in b["attendee_mail_ids"]:
            failures.append(
                f"attendee {b['attendee_mail_ids']} != X"
            )
        if not is_free(scenario["x_busy"], actual_start, actual_end):
            failures.append("booking overlaps X's busy")

    print()
    print("[constraints]")
    if not failures:
        print("  ✓ all constraints satisfied")
    else:
        for f in failures:
            print(f"  ✗ {f}")

    verdict = Verdict(
        overlay_must=[
            OverlayAssertion(field="booked_events", expected_count=1),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
            ToolCallMatch(name="rsvp"),
        ],
    )
    scorecard = evaluate(verdict, run, trace, max_turns=20)
    if failures:
        scorecard.correctness = False
    return report(run, scorecard)


if __name__ == "__main__":
    sys.exit(runner(main()))
