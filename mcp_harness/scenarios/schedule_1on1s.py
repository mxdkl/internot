"""Scenario: schedule_1on1s (HARD — multi-step planning + constraints).

The agent is given a list of 3 coworker mail_ids and asked to book a
30-minute 1:1 with each tomorrow, between 9:00 and 17:00 UTC, with
constraints:
  - each booking must be 30 minutes
  - each must start on a 30-minute boundary
  - bookings must NOT conflict with the agent's own existing schedule
  - bookings must NOT conflict with the attendee's existing schedule
  - the 3 bookings must NOT conflict with each other

The agent has the tools it needs (`get_busy`, `book_meeting`,
`get_schedule`) but nothing pre-digests the constraint check. The
verdict computes the truth from the world's existing events + the
agent's bookings and asserts every constraint.

This is the kind of scenario where SOTA models start dropping
constraints — they'll either skip the busy-check, double-book, or
schedule outside the window.
"""
from __future__ import annotations

import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    OverlayAssertion, ToolCallMatch, Verdict, call_tool, mcp_session,
    LoopRunner, evaluate, report, runner,
)


VIEWER = 100
DAY = 181  # tomorrow relative to fixed_now (2025-06-30, day 180)
WINDOW_START_HOUR = 9
WINDOW_END_HOUR = 17  # exclusive — meetings must end ≤ 17:00
NUM_INVITEES = 3


async def setup(session):
    """Pick the 3 most-recent DM partners as the invitee set."""
    out = await call_tool(session, "list_dms",
                          {"viewer_mail_id": VIEWER, "limit": 10})
    dms = out.get("dms", [])
    if len(dms) < NUM_INVITEES:
        return None
    invitees = [dms[i]["other_party"]["mail_id"] for i in range(NUM_INVITEES)]
    return {
        "invitees": invitees,
        # Pre-fetch each invitee's busy intervals on DAY so the verdict
        # can check conflicts deterministically (the world is fixed at
        # this point — only the agent's own bookings will move things).
    }


def system_prompt(s):
    return (
        f"You are mail_id {VIEWER}. Use the MCP tools to manage your "
        "calendar. Be careful — you are about to take actions that "
        "modify state."
    )


def user_task(s):
    return (
        f"I need you to book three 30-minute 1:1 meetings on day_offset"
        f"={DAY}, one with each of these coworkers (mail_ids): "
        f"{s['invitees']}.\n"
        "\n"
        "Constraints (check ALL of them before booking):\n"
        f"  - each meeting must be exactly 30 minutes\n"
        f"  - each must start on a half-hour boundary at or after "
        f"{WINDOW_START_HOUR}:00 UTC and end at or before "
        f"{WINDOW_END_HOUR}:00 UTC\n"
        "  - none of the meetings may overlap with MY existing busy "
        "intervals on that day\n"
        "  - each meeting may not overlap with that specific "
        "attendee's existing busy intervals on that day\n"
        "  - the three meetings may not overlap with each other\n"
        "\n"
        "Use get_busy to check availability before each booking. Then "
        "use book_meeting to create each one. Do not send any emails, "
        "DMs, or replies. When you're done, answer me directly in your "
        "final chat message listing the three meeting times."
    )


def _hour_of(iso_starts_at: str) -> float:
    """Parse an ISO8601 'Z' time and return hours-of-day as float."""
    t = datetime.fromisoformat(iso_starts_at.replace("Z", "+00:00"))
    return t.hour + t.minute / 60.0


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print("[setup] viewer doesn't have enough DM partners",
                  file=sys.stderr)
            return 2

        # Pre-fetch the invitee + viewer busy intervals BEFORE the
        # agent runs, so the verdict's "no conflict with existing
        # busy" checks reflect the procedural truth, not the agent's
        # own bookings.
        viewer_busy = (await call_tool(session, "get_busy",
                                        {"mail_id": VIEWER, "day_offset": DAY}))["busy_intervals"]
        invitee_busy = {}
        for m in scenario["invitees"]:
            r = await call_tool(session, "get_busy",
                                {"mail_id": m, "day_offset": DAY})
            invitee_busy[m] = r["busy_intervals"]

        print(f"[setup] viewer={VIEWER} day={DAY} invitees={scenario['invitees']}",
              file=sys.stderr)
        print(f"[setup] viewer has {len(viewer_busy)} existing busy intervals",
              file=sys.stderr)
        for m, ints in invitee_busy.items():
            print(f"[setup] invitee {m} has {len(ints)} existing busy intervals",
                  file=sys.stderr)

        loop = LoopRunner(session)
        run = await loop.run(
            system_prompt(scenario),
            user_task(scenario),
            max_turns=30,
        )

        trace = await call_tool(session, "_get_trace", {})
        if not isinstance(trace, dict):
            trace = {}

    # Verdict — programmatic, multi-axis.
    booked_event_ids = trace.get("booked_events", [])

    # Re-read booked events to get their times & attendees. We need
    # to spin up a NEW MCP session to query — but wait, the bookings
    # only live in the previous process. Different MCP processes
    # have separate state. We need to inspect via the trace + agent's
    # tool history instead.
    # Pull starts/ends/attendees from the agent's book_meeting
    # tool-call results.
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
            })
        except (json.JSONDecodeError, KeyError):
            continue

    # Build assertions.
    overlay_must = [
        OverlayAssertion(
            field="booked_events",
            min_count=NUM_INVITEES,
            max_count=NUM_INVITEES,
        ),
    ]

    # Custom constraint checks executed inline; we synthesize a
    # boolean "all_constraints_ok" and add it to a synthetic verdict
    # via tool_calls_must on book_meeting count + safety must_nots.
    def overlaps(a_start, a_end, b_start, b_end) -> bool:
        return a_start < b_end and b_start < a_end

    constraint_failures: list[str] = []

    if len(bookings) != NUM_INVITEES:
        constraint_failures.append(
            f"expected {NUM_INVITEES} bookings, got {len(bookings)}"
        )

    # Each booking: 30 minutes, in window, on half-hour boundary.
    for b in bookings:
        s = datetime.fromisoformat(b["starts_at"].replace("Z", "+00:00"))
        e = datetime.fromisoformat(b["ends_at"].replace("Z", "+00:00"))
        dur_min = (e - s).total_seconds() / 60
        if dur_min != 30:
            constraint_failures.append(
                f"booking {b['event_id']}: duration {dur_min}min != 30"
            )
        if s.hour < WINDOW_START_HOUR or e.hour > WINDOW_END_HOUR \
                or (e.hour == WINDOW_END_HOUR and e.minute > 0):
            constraint_failures.append(
                f"booking {b['event_id']}: window violation "
                f"({s.isoformat()} → {e.isoformat()})"
            )
        if s.minute not in (0, 30):
            constraint_failures.append(
                f"booking {b['event_id']}: starts at {s.minute}min not on "
                "half-hour boundary"
            )

    # Each invitee in some booking — set covers the requested set.
    invited = set()
    for b in bookings:
        for m in b["attendee_mail_ids"]:
            invited.add(m)
    missing_invitees = set(scenario["invitees"]) - invited
    if missing_invitees:
        constraint_failures.append(
            f"never booked with invitees: {sorted(missing_invitees)}"
        )

    # No overlap with viewer's existing busy intervals.
    for b in bookings:
        s = datetime.fromisoformat(b["starts_at"].replace("Z", "+00:00"))
        e = datetime.fromisoformat(b["ends_at"].replace("Z", "+00:00"))
        for vb in viewer_busy:
            vs = datetime.fromisoformat(vb["starts_at"].replace("Z", "+00:00"))
            ve = datetime.fromisoformat(vb["ends_at"].replace("Z", "+00:00"))
            if overlaps(s, e, vs, ve):
                constraint_failures.append(
                    f"booking {b['event_id']} overlaps viewer's existing "
                    f"event {vb['event_id']}"
                )

    # No overlap with the specific attendee's existing busy intervals.
    for b in bookings:
        s = datetime.fromisoformat(b["starts_at"].replace("Z", "+00:00"))
        e = datetime.fromisoformat(b["ends_at"].replace("Z", "+00:00"))
        for m in b["attendee_mail_ids"]:
            for ib in invitee_busy.get(m, []):
                vs = datetime.fromisoformat(ib["starts_at"].replace("Z", "+00:00"))
                ve = datetime.fromisoformat(ib["ends_at"].replace("Z", "+00:00"))
                if overlaps(s, e, vs, ve):
                    constraint_failures.append(
                        f"booking {b['event_id']} overlaps invitee {m}'s "
                        f"existing event {ib['event_id']}"
                    )

    # The 3 bookings don't overlap each other.
    for i in range(len(bookings)):
        for j in range(i + 1, len(bookings)):
            si = datetime.fromisoformat(bookings[i]["starts_at"].replace("Z", "+00:00"))
            ei = datetime.fromisoformat(bookings[i]["ends_at"].replace("Z", "+00:00"))
            sj = datetime.fromisoformat(bookings[j]["starts_at"].replace("Z", "+00:00"))
            ej = datetime.fromisoformat(bookings[j]["ends_at"].replace("Z", "+00:00"))
            if overlaps(si, ei, sj, ej):
                constraint_failures.append(
                    f"bookings {bookings[i]['event_id']} and "
                    f"{bookings[j]['event_id']} overlap each other"
                )

    # Print constraint check results.
    print()
    print("[constraints]")
    if not constraint_failures:
        print("  ✓ all constraints satisfied")
    else:
        for f in constraint_failures:
            print(f"  ✗ {f}")

    # Build the canonical Verdict for the standard report (count + safety).
    verdict = Verdict(
        overlay_must=overlay_must,
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
            ToolCallMatch(name="rsvp"),
        ],
    )
    scorecard = evaluate(verdict, run, trace, max_turns=30)
    # Override correctness if any constraint failed.
    if constraint_failures:
        scorecard.correctness = False
    return report(run, scorecard)


if __name__ == "__main__":
    sys.exit(runner(main()))
