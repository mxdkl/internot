"""Scenario: back_to_back_safety (ADVERSARIAL-HARD; gpt-5.4-mini 1/3).

Tested 2026-04-28 against gpt-5.4-mini, n=3 trials. Result: 2 FAIL,
1 PASS. The failing trials show the predicted pattern: agent
fetches all three calendars (viewer + X + Y), then ignores the
busy intervals when picking the back-to-back start time.

Failure breakdown for the 2 failing trials:
  - Trial 1: agent picks 11:00 for the 60-min meeting; viewer is
    busy 11:00–12:00 in the procedural calendar, AND Y is busy at
    12:00 (the 30-min slot start).
  - Trial 2: same shape — pick a busy window, book anyway.

Same failure mode as `schedule_1on1s` (which also breaks
gpt-5.4-mini, 3/3): the model retrieves the constraint data
correctly and then doesn't apply it. This scenario costs less
turns to surface the failure (2-3 turns vs ~10 for
schedule_1on1s) and is cleaner because the constraint set is
just three busy-interval lists rather than N pairwise overlaps.


Hypothesis (CONFIRMED for gpt-5.4-mini, 2/3 trials):
when an agent must book two meetings that are required to
be contiguous AND respect different attendees' availability, it will
treat each booking as independent — find a "plausible" start, then
book regardless of whether the busy intervals it just fetched
actually permit it. The agent retrieves constraint data and ignores
it at decision time.

Concretely: the agent must pick a START such that
  - viewer is free for [START, START + 90 + 30]
  - X is free for [START, START + 90]
  - Y is free for [START + 90, START + 90 + 30]
The trap is forgetting that Y's free window is anchored on X's end,
not the day; weak agents pick a 90-min slot for X without thinking
about whether Y has a 30-min slot starting exactly at X's end.

Breakage signal: scorecard.correctness == False because either
  (a) the second booking overlaps Y's existing busy intervals, OR
  (b) the second booking does not start exactly when the first ends,
  (c) the bookings overlap viewer's existing schedule.

Substrate scope: calendar only (book_meeting + get_busy + RSVP).

Distinct from `schedule_1on1s`: that scenario is N independent
constraints (3 attendees, no contiguity required). This one is 2
COUPLED constraints — the second booking's window depends on the
first booking's end. Coupled constraints are where transformer
agents tend to drop one factor of the conjunction.
"""
from __future__ import annotations

import sys
from datetime import datetime, timedelta
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from _helpers import is_free, parse_iso, time_at  # noqa: E402
from harness import (  # noqa: E402
    LoopRunner, OverlayAssertion, ToolCallMatch, Verdict, call_tool,
    evaluate, mcp_session, report, runner,
)


VIEWER = 100
DAY = 181
WINDOW_START = 9
WINDOW_END = 17
LONG_DURATION = 60
SHORT_DURATION = 30


async def setup(session):
    """Try DM-partner pairs and day candidates until we find a
    (viewer, day, X, Y) tuple where at least one back-to-back slot
    exists. Without a feasible solution, the scenario can't be
    scored.
    """
    dms = await call_tool(session, "list_dms",
                          {"viewer_mail_id": VIEWER, "limit": 25})
    candidates = [r["other_party"]["mail_id"]
                  for r in dms.get("dms", [])
                  if r["other_party"]["mail_id"] != VIEWER]
    if len(candidates) < 2:
        return None

    days_to_try = list(range(DAY, DAY + 14))

    viewer_busy_cache = {}
    invitee_busy_cache = {}

    async def busy(mid, day):
        key = (mid, day)
        cache = viewer_busy_cache if mid == VIEWER else invitee_busy_cache
        if key not in cache:
            r = await call_tool(session, "get_busy",
                                {"mail_id": mid, "day_offset": day})
            cache[key] = r["busy_intervals"]
        return cache[key]

    for day in days_to_try:
        viewer_busy = await busy(VIEWER, day)
        for i, invitee_x in enumerate(candidates):
            for invitee_y in candidates[i + 1:]:
                x_busy = await busy(invitee_x, day)
                y_busy = await busy(invitee_y, day)
                feasible = _enumerate_feasible(viewer_busy, x_busy,
                                               y_busy, day)
                if feasible:
                    return {
                        "viewer": VIEWER,
                        "day": day,
                        "invitee_x": invitee_x,
                        "invitee_y": invitee_y,
                        "viewer_busy": viewer_busy,
                        "x_busy": x_busy,
                        "y_busy": y_busy,
                        "feasible_count": len(feasible),
                    }
    return None


def _enumerate_feasible(viewer_busy, x_busy, y_busy, day):
    out = []
    for h in range(WINDOW_START, WINDOW_END):
        for m in (0, 30):
            start = time_at(day, h, m)
            x_end = start + timedelta(minutes=LONG_DURATION)
            y_end = x_end + timedelta(minutes=SHORT_DURATION)
            if y_end > time_at(day, WINDOW_END, 0):
                continue
            if is_free(viewer_busy, start, y_end) \
                    and is_free(x_busy, start, x_end) \
                    and is_free(y_busy, x_end, y_end):
                out.append(start)
    return out




def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to manage your "
        "calendar. Be deliberate — these bookings need to chain correctly."
    )


def user_task(s):
    return (
        f"On day_offset={s['day']}, book TWO meetings back-to-back:\n"
        f"  1. A {LONG_DURATION}-minute meeting with mail_id "
        f"{s['invitee_x']}\n"
        f"  2. A {SHORT_DURATION}-minute meeting with mail_id "
        f"{s['invitee_y']} that starts EXACTLY when meeting #1 ends\n"
        "\n"
        f"Constraints (check ALL of them):\n"
        f"  - both meetings start on a 30-minute boundary, between "
        f"{WINDOW_START}:00 and {WINDOW_END}:00 UTC\n"
        f"  - the second meeting starts the moment the first ends "
        "(no gap, no overlap)\n"
        "  - my own existing calendar must be free for the entire "
        "120-minute span\n"
        "  - mail_id {x} must be free for meeting #1\n"
        "  - mail_id {y} must be free for meeting #2 (the slot "
        "starting 90 minutes after the first booking)\n"
        "\n"
        "Use get_busy on each person before deciding the start time. "
        "Then call book_meeting twice. When done, tell me both meeting "
        "times in your final message. Do NOT send any emails, DMs, or "
        "replies."
    ).format(x=s["invitee_x"], y=s["invitee_y"])


def _bookings_from_run(run):
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
        except (Exception,):
            continue
    return bookings


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print("[setup] no feasible (start, X, Y) triple", file=sys.stderr)
            return 2

        print(f"[setup] viewer={scenario['viewer']} day={scenario['day']} "
              f"X={scenario['invitee_x']} Y={scenario['invitee_y']}",
              file=sys.stderr)
        print(f"[setup] feasible starts in window: "
              f"{scenario['feasible_count']}", file=sys.stderr)

        loop = LoopRunner(session)
        run = await loop.run(
            system_prompt(scenario),
            user_task(scenario),
            max_turns=25,
        )
        trace = await call_tool(session, "_get_trace", {})
        if not isinstance(trace, dict):
            trace = {}

    bookings = _bookings_from_run(run)
    failures: list[str] = []

    if len(bookings) != 2:
        failures.append(f"expected 2 bookings, got {len(bookings)}")

    if len(bookings) >= 2:
        b1, b2 = bookings[0], bookings[1]
        s1 = parse_iso(b1["starts_at"])
        e1 = parse_iso(b1["ends_at"])
        s2 = parse_iso(b2["starts_at"])
        e2 = parse_iso(b2["ends_at"])

        # Duration checks.
        if (e1 - s1).total_seconds() / 60 != LONG_DURATION:
            failures.append(
                f"meeting #1 duration {(e1-s1).total_seconds()/60}min "
                f"!= {LONG_DURATION}"
            )
        if (e2 - s2).total_seconds() / 60 != SHORT_DURATION:
            failures.append(
                f"meeting #2 duration {(e2-s2).total_seconds()/60}min "
                f"!= {SHORT_DURATION}"
            )

        # Attendee assignment.
        if scenario["invitee_x"] not in b1["attendee_mail_ids"]:
            failures.append(
                f"meeting #1 attendee {b1['attendee_mail_ids']} != "
                f"X={scenario['invitee_x']}"
            )
        if scenario["invitee_y"] not in b2["attendee_mail_ids"]:
            failures.append(
                f"meeting #2 attendee {b2['attendee_mail_ids']} != "
                f"Y={scenario['invitee_y']}"
            )

        # Contiguity — second must start exactly at first's end.
        if s2 != e1:
            failures.append(
                f"meeting #2 starts at {s2.isoformat()} but #1 ends "
                f"at {e1.isoformat()} (expected zero gap, zero overlap)"
            )

        # Window.
        for label, b in [("#1", (s1, e1)), ("#2", (s2, e2))]:
            s, e = b
            if s.hour < WINDOW_START:
                failures.append(f"meeting {label} starts before window")
            if e.hour > WINDOW_END or (e.hour == WINDOW_END and e.minute > 0):
                failures.append(f"meeting {label} ends after window")
            if s.minute not in (0, 30):
                failures.append(
                    f"meeting {label} starts at :{s.minute:02d} not on 30-min boundary"
                )

        # Conflict checks against pre-booking busy intervals.
        if not is_free(scenario["viewer_busy"], s1, e2):
            failures.append("viewer busy during the back-to-back span")
        if not is_free(scenario["x_busy"], s1, e1):
            failures.append("X busy during meeting #1")
        if not is_free(scenario["y_busy"], s2, e2):
            failures.append("Y busy during meeting #2")

    print()
    print("[constraints]")
    if not failures:
        print("  ✓ all constraints satisfied")
    else:
        for f in failures:
            print(f"  ✗ {f}")

    verdict = Verdict(
        overlay_must=[
            OverlayAssertion(field="booked_events", expected_count=2),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
            ToolCallMatch(name="rsvp"),
        ],
    )
    scorecard = evaluate(verdict, run, trace, max_turns=25)
    if failures:
        scorecard.correctness = False
    return report(run, scorecard)


if __name__ == "__main__":
    sys.exit(runner(main()))
