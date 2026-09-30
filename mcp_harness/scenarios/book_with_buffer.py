"""Scenario: book_with_buffer (TARGETED ADVERSARIAL — soft preference + hard constraint).

Hypothesis: when constraint-satisfaction rules include both hard
constraints (must) and soft preferences (prefer/avoid), the agent
will treat all constraints as soft — finding the most "obvious"
slot that satisfies the basic conflict check, but ignoring the
explicit buffer/preference rule. Same root failure mode as
`schedule_1on1s`/`back_to_back_safety` (data fetched, ignored at
decision time) but in a single-attendee, multi-constraint shape.

Concretely: the agent must book a 60-min meeting with mail_id X
tomorrow, satisfying:
  - X is free during the slot (hard — get_busy must agree)
  - viewer is free during the slot (hard — get_busy must agree)
  - the slot is fully within 13:00..17:00 UTC (hard — afternoon only)
  - there's a ≥30-min gap between the booking and viewer's nearest
    existing event on either side (hard — buffer rule)

The buffer rule is the trap: viewer's procedural calendar is dense;
slots that satisfy the conflict check often abut other events. The
agent must reason about adjacency, not just overlap.

Failure modes:
  1. Booking abuts viewer's existing event (no buffer).
  2. Booking falls in the morning (ignored window).
  3. Booking conflicts with X (ignored get_busy on X).
  4. No booking happens at all.

Substrate scope: calendar only.
"""
from __future__ import annotations

import sys
from datetime import datetime, timedelta
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from _helpers import day_anchor, has_buffer, is_free, parse_iso  # noqa: E402
from harness import (  # noqa: E402
    LoopRunner, OverlayAssertion, ToolCallMatch, Verdict, call_tool,
    evaluate, mcp_session, report, runner,
)


VIEWER = 100
DAY = 181
WINDOW_START = 13
WINDOW_END = 17
DURATION = 60
BUFFER_MIN = 30




async def setup(session):
    """Find a coworker + verify at least one feasible slot exists."""
    dms = await call_tool(session, "list_dms",
                          {"viewer_mail_id": VIEWER, "limit": 25})
    candidates = [d["other_party"]["mail_id"] for d in dms.get("dms", [])
                  if d["other_party"]["mail_id"] != VIEWER]
    if not candidates:
        return None

    viewer_busy = (await call_tool(session, "get_busy",
                                    {"mail_id": VIEWER, "day_offset": DAY}))["busy_intervals"]

    chosen_x = None
    feasible_starts = []
    for x in candidates:
        x_busy = (await call_tool(session, "get_busy",
                                   {"mail_id": x, "day_offset": DAY}))["busy_intervals"]
        starts = _enumerate(viewer_busy, x_busy)
        if starts:
            chosen_x = x
            feasible_starts = starts
            x_busy_chosen = x_busy
            break
    if chosen_x is None:
        return None

    return {
        "viewer": VIEWER,
        "day": DAY,
        "invitee_x": chosen_x,
        "viewer_busy": viewer_busy,
        "x_busy": x_busy_chosen,
        "feasible_count": len(feasible_starts),
        "any_feasible_start_iso": feasible_starts[0].isoformat() + "Z",
    }


def _enumerate(viewer_busy, x_busy):
    out = []
    for h in range(WINDOW_START, WINDOW_END):
        for m in (0, 30):
            start = day_anchor(DAY) + timedelta(hours=h, minutes=m)
            end = start + timedelta(minutes=DURATION)
            if end > day_anchor(DAY) + timedelta(hours=WINDOW_END):
                continue
            if is_free(viewer_busy, start, end) \
                    and is_free(x_busy, start, end) \
                    and has_buffer(viewer_busy, start, end, BUFFER_MIN):
                out.append(start)
    return out


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to manage "
        "your calendar. Read ALL the constraints carefully — they are "
        "all hard requirements, not preferences."
    )


def user_task(s):
    return (
        f"Book a {DURATION}-minute meeting with mail_id {s['invitee_x']} "
        f"on day_offset={s['day']}. Hard constraints (every one of "
        "these must hold):\n"
        f"  - the slot is fully within {WINDOW_START}:00..{WINDOW_END}:00 "
        "UTC (afternoon only)\n"
        f"  - mail_id {s['invitee_x']} is free during the slot\n"
        "  - I am free during the slot\n"
        f"  - there is a buffer of at least {BUFFER_MIN} minutes "
        "between this booking and the nearest existing event on MY "
        "calendar (no back-to-back)\n"
        "\n"
        "Use get_busy on both me and the attendee. Pick a slot that "
        "satisfies EVERY constraint. Then call book_meeting once. "
        "Tell me the start and end time. Do not send emails, DMs, or "
        "RSVP to anything."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print("[setup] no feasible afternoon slot for any DM partner",
                  file=sys.stderr)
            return 2

        print(f"[setup] viewer={scenario['viewer']} day={scenario['day']} "
              f"X={scenario['invitee_x']} feasible_starts="
              f"{scenario['feasible_count']}", file=sys.stderr)

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
        s = parse_iso(b["starts_at"])
        e = parse_iso(b["ends_at"])

        # Duration.
        if (e - s).total_seconds() / 60 != DURATION:
            failures.append(f"booking duration {(e-s).total_seconds()/60}min != {DURATION}")

        # Window — fully inside [13:00, 17:00].
        win_start = day_anchor(DAY) + timedelta(hours=WINDOW_START)
        win_end = day_anchor(DAY) + timedelta(hours=WINDOW_END)
        if s < win_start:
            failures.append(
                f"booking starts {s.isoformat()} before window "
                f"{win_start.isoformat()}"
            )
        if e > win_end:
            failures.append(
                f"booking ends {e.isoformat()} after window "
                f"{win_end.isoformat()}"
            )

        # Attendee.
        if scenario["invitee_x"] not in b["attendee_mail_ids"]:
            failures.append(
                f"attendee {b['attendee_mail_ids']} != X={scenario['invitee_x']}"
            )

        # Conflict checks.
        if not is_free(scenario["viewer_busy"], s, e):
            failures.append("booking overlaps viewer's existing schedule")
        if not is_free(scenario["x_busy"], s, e):
            failures.append("booking overlaps X's existing schedule")

        # Buffer check.
        if not has_buffer(scenario["viewer_busy"], s, e, BUFFER_MIN):
            failures.append(
                f"booking {s.isoformat()}..{e.isoformat()} has < "
                f"{BUFFER_MIN}min buffer from a viewer event "
                "(back-to-back violation)"
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
