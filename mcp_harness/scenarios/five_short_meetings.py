"""Scenario: five_short_meetings (ADVERSARIAL-HARD; gpt-5.4-mini 3/3 fail).

Tested 2026-04-28 against gpt-5.4-mini, n=3 trials, all FAIL.
Together with `schedule_1on1s` (3/3 fail) and `back_to_back_safety`
(2/3 fail), this confirms the FAN-OUT failure pattern:
gpt-5.4-mini's constraint-satisfaction failures are NOT about
constraint complexity — they're about fan-out. Single-booking
multi-constraint scenarios pass (`book_with_buffer` 3/3,
`reschedule_cascade` 3/3); multi-booking mutual-non-overlap
scenarios fail.

Notable in trial 1: the agent self-diagnosed the failure in its
final message — "the 09:00–09:30 slot overlaps between two of the
newly booked meetings, so the set is not fully conflict-free across
all five" — but committed the bookings anyway. The model can
articulate the constraint, recognize the violation, AND still
produce the violating output. That's a particularly useful failure
artifact: it suggests the failure isn't constraint comprehension,
it's joint-search/decision quality.


Hypothesis: gpt-5.4-mini's failure mode in `schedule_1on1s` (3/3
fail) and `back_to_back_safety` (2/3 fail) is FAN-OUT — when
constraint satisfaction spans multiple simultaneous bookings or
multiple attendees, the agent loses track of inter-booking
constraints. Single-booking variants like `book_with_buffer` (3/3
pass) work fine. This scenario pushes fan-out further: 5 attendees,
5 separate bookings, with mutual non-overlap constraint.

Concretely: agent must book FIVE separate 15-minute 1:1s tomorrow,
one with each of 5 coworkers, all within 9:00-17:00 UTC, on 30-min
boundaries. Constraints:
  - each respects viewer's busy
  - each respects that attendee's busy
  - the 5 bookings don't overlap each other (mutual non-overlap)

Failure modes:
  1. Two bookings overlap each other (fan-out failure — couldn't
     track inter-booking constraints).
  2. A booking overlaps the viewer's existing schedule (ignored
     get_busy data the agent fetched).
  3. A booking overlaps an attendee's existing schedule (same).
  4. Window/duration violations.

If gpt-5.4-mini fails this scenario at a higher rate than
back_to_back_safety (5 attendees × 5 bookings vs 2 attendees × 2
bookings), the fan-out hypothesis is confirmed.
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
DAY = 181
WINDOW_START = 9
WINDOW_END = 17
DURATION = 30
NUM_INVITEES = 5


async def setup(session):
    dms = await call_tool(session, "list_dms",
                          {"viewer_mail_id": VIEWER, "limit": 25})
    candidates = [d["other_party"]["mail_id"] for d in dms.get("dms", [])
                  if d["other_party"]["mail_id"] != VIEWER]
    if len(candidates) < NUM_INVITEES:
        return None
    invitees = candidates[:NUM_INVITEES]

    viewer_busy = (await call_tool(session, "get_busy",
                                    {"mail_id": VIEWER, "day_offset": DAY}))["busy_intervals"]
    invitee_busy = {}
    for m in invitees:
        r = await call_tool(session, "get_busy",
                             {"mail_id": m, "day_offset": DAY})
        invitee_busy[m] = r["busy_intervals"]

    return {
        "viewer": VIEWER,
        "day": DAY,
        "invitees": invitees,
        "viewer_busy": viewer_busy,
        "invitee_busy": invitee_busy,
    }


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to manage "
        "your calendar. Plan ahead — these meetings need to fit "
        "together as well as individually."
    )


def user_task(s):
    return (
        f"Book {NUM_INVITEES} separate {DURATION}-minute 1:1 meetings "
        f"on day_offset={s['day']}, one with each of these coworkers "
        f"(mail_ids): {s['invitees']}.\n"
        "\n"
        "Constraints (every one of them):\n"
        f"  - each meeting is exactly {DURATION} minutes\n"
        f"  - each starts on a 30-minute boundary at or after "
        f"{WINDOW_START}:00 and ends at or before {WINDOW_END}:00 UTC\n"
        "  - each respects MY existing busy intervals\n"
        "  - each respects that specific attendee's busy intervals\n"
        f"  - the {NUM_INVITEES} meetings must not overlap each other\n"
        "\n"
        "Use get_busy on each person before deciding. Then call "
        "book_meeting once per coworker. Tell me all five meeting "
        "times in your final message. Do not send emails, DMs, or "
        "RSVPs."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print("[setup] viewer doesn't have 5 DM partners",
                  file=sys.stderr)
            return 2

        print(f"[setup] viewer={scenario['viewer']} day={scenario['day']} "
              f"invitees={scenario['invitees']}", file=sys.stderr)

        loop = LoopRunner(session)
        run = await loop.run(
            system_prompt(scenario),
            user_task(scenario),
            max_turns=30,
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
                "event_id": res["event_id"],
                "starts_at": res["starts_at"],
                "ends_at": res["ends_at"],
                "attendee_mail_ids": tc.arguments.get("attendee_mail_ids", []),
                "duration_minutes": tc.arguments.get("duration_minutes"),
            })
        except Exception:
            continue

    failures: list[str] = []
    if len(bookings) != NUM_INVITEES:
        failures.append(f"expected {NUM_INVITEES} bookings, got {len(bookings)}")

    seen_invitees = set()
    parsed = []
    for b in bookings:
        s = parse_iso(b["starts_at"])
        e = parse_iso(b["ends_at"])
        parsed.append((b, s, e))
        if (e - s).total_seconds() / 60 != DURATION:
            failures.append(
                f"booking {b['event_id']}: duration "
                f"{(e-s).total_seconds()/60}min != {DURATION}"
            )
        for m in b["attendee_mail_ids"]:
            seen_invitees.add(m)
        # Window.
        if s.hour < WINDOW_START:
            failures.append(f"booking {b['event_id']} starts before window")
        if e.hour > WINDOW_END or (e.hour == WINDOW_END and e.minute > 0):
            failures.append(f"booking {b['event_id']} ends after window")
        if s.minute not in (0, 30):
            failures.append(
                f"booking {b['event_id']} starts at :{s.minute:02d} "
                "(not on 30-min boundary)"
            )
        # Viewer conflict.
        if not is_free(scenario["viewer_busy"], s, e):
            failures.append(f"booking {b['event_id']} overlaps viewer's existing schedule")
        # Attendee conflict.
        for m in b["attendee_mail_ids"]:
            ib = scenario["invitee_busy"].get(m, [])
            if not is_free(ib, s, e):
                failures.append(
                    f"booking {b['event_id']} overlaps attendee {m}'s schedule"
                )

    missing = set(scenario["invitees"]) - seen_invitees
    if missing:
        failures.append(f"never booked with: {sorted(missing)}")

    # Mutual non-overlap.
    for i in range(len(parsed)):
        for j in range(i + 1, len(parsed)):
            (bi, si, ei) = parsed[i]
            (bj, sj, ej) = parsed[j]
            if si < ej and sj < ei:
                failures.append(
                    f"bookings {bi['event_id']} and {bj['event_id']} "
                    "overlap each other (fan-out failure)"
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
            OverlayAssertion(field="booked_events", expected_count=NUM_INVITEES),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
            ToolCallMatch(name="rsvp"),
        ],
    )
    scorecard = evaluate(verdict, run, trace, max_turns=30)
    if failures:
        scorecard.correctness = False
    return report(run, scorecard)


if __name__ == "__main__":
    sys.exit(runner(main()))
