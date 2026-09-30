"""Scenario: meeting_prep (ADVERSARIAL-HARD; gpt-5.4-mini 2/3 fail).

Tested 2026-04-28 against gpt-5.4-mini, n=3 trials. Result: 2 FAIL,
1 PASS.

Original hypothesis (FALSIFIED): cross-service date arithmetic
(meeting_day - 1 for the prep task) was expected to be the trap.
It wasn't — every single trial computed `due_day_offset=184`
correctly from `meeting_day=185`. The model handles `N - 1`
reliably across tool calls.

Actual failure mode (CONFIRMED 2/3): the BOOKING step fails for
the same reason as `schedule_1on1s` and `back_to_back_safety` —
agent picks 15:00 (the "natural" mid-afternoon time) without
checking that the viewer is busy at 15:00. The procedural calendar
on day 185 has only one feasible afternoon slot for both
participants (16:00–17:00); 2 of 3 trials booked at 15:00 anyway.
Trial 3 found 16:00 and passed.

Lesson for portfolio design: pairing a hard step with an easy step
doesn't always create a new failure mode — sometimes it just
re-runs the existing one. The scenario remains useful as a baseline
showing that gpt-5.4-mini's date-arithmetic across tool calls is
NOT a failure surface (worth knowing) and that the busy-interval
ignoring failure persists even when the agent only has to satisfy
ONE booking constraint after fetching the data.


Concretely: agent must
  1. book a 60-min afternoon meeting with mail_id X on day 185
  2. create a task assigned to viewer titled "Prep for #X"
     due on day 184 (the day BEFORE the meeting), priority High

The trap: the agent uses day 185 for the task too, or uses "today"
(180), or some other plausible-but-wrong date. Computing
`meeting_day - 1` is trivial — but only if the agent actually
performs the step instead of substituting one of the other dates
in scope.

Failure modes:
  1. Task due_day_offset != 184 (off-by-one or wrong source date).
  2. Task title doesn't match exactly.
  3. Task priority != High.
  4. Task assignee != viewer.
  5. Booking is morning, not afternoon, OR conflicts with X.
  6. Booking missing OR task missing.

Substrate scope: calendar + tasks. Two mutations, two services,
one dependency between them.
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
MEETING_DAY = 185
EXPECTED_TASK_DUE_DAY = MEETING_DAY - 1
WINDOW_START = 13
WINDOW_END = 17
DURATION = 60
EXPECTED_TITLE = "Prep for #27"
EXPECTED_PRIORITY = "High"
COWORKER_X = 27


async def setup(session):
    """Verify the booking is feasible — at least one afternoon slot
    on day 185 where both viewer and X are free."""
    viewer_busy = (await call_tool(session, "get_busy",
                                    {"mail_id": VIEWER, "day_offset": MEETING_DAY}))["busy_intervals"]
    x_busy = (await call_tool(session, "get_busy",
                               {"mail_id": COWORKER_X, "day_offset": MEETING_DAY}))["busy_intervals"]

    base = datetime(2025, 1, 1, 0, 0, 0) + timedelta(days=MEETING_DAY)
    feasible_starts = []
    for h in range(WINDOW_START, WINDOW_END):
        for m in (0, 30):
            start = base + timedelta(hours=h, minutes=m)
            end = start + timedelta(minutes=DURATION)
            if end > base + timedelta(hours=WINDOW_END):
                continue
            if is_free(viewer_busy, start, end) and is_free(x_busy, start, end):
                feasible_starts.append(start)
    if not feasible_starts:
        return None

    return {
        "viewer": VIEWER,
        "meeting_day": MEETING_DAY,
        "expected_task_due_day": EXPECTED_TASK_DUE_DAY,
        "coworker_x": COWORKER_X,
        "viewer_busy": viewer_busy,
        "x_busy": x_busy,
        "feasible_count": len(feasible_starts),
    }


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools. Pay "
        "attention to dates — when one task's date depends on "
        "another's, do the arithmetic."
    )


def user_task(s):
    return (
        f"Two things to do, in order:\n"
        "\n"
        f"  STEP 1: Book a {DURATION}-minute afternoon meeting with "
        f"mail_id {s['coworker_x']} on day_offset={s['meeting_day']}. "
        f"The slot must be fully within {WINDOW_START}:00..{WINDOW_END}:00 "
        "UTC, with both of us free (use get_busy on each).\n"
        "\n"
        f"  STEP 2: Create a task assigned to me (mail_id {s['viewer']}) "
        f"titled exactly '{EXPECTED_TITLE}', priority {EXPECTED_PRIORITY}, "
        "due the DAY BEFORE the meeting (so I prep one day in advance).\n"
        "\n"
        "Compute the due day from STEP 1's meeting day. Do not send "
        "emails, DMs, or replies. Tell me both event_id and task_id "
        "when done."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print("[setup] no feasible afternoon slot for X on day 185",
                  file=sys.stderr)
            return 2

        print(f"[setup] viewer={scenario['viewer']} meeting_day={MEETING_DAY} "
              f"expected_task_due={EXPECTED_TASK_DUE_DAY} "
              f"feasible_starts={scenario['feasible_count']}",
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

    create_calls = [tc for tc in run.tool_calls if tc.name == "create_task"]

    failures: list[str] = []

    if len(bookings) != 1:
        failures.append(f"expected 1 booking, got {len(bookings)}")
    if len(create_calls) != 1:
        failures.append(f"expected 1 create_task call, got {len(create_calls)}")

    if bookings:
        b = bookings[0]
        s = parse_iso(b["starts_at"])
        e = parse_iso(b["ends_at"])

        # Check meeting day — should be day 185.
        days_since_epoch = (s - datetime(2025, 1, 1)).days
        if days_since_epoch != MEETING_DAY:
            failures.append(
                f"booking is on day {days_since_epoch}, expected day "
                f"{MEETING_DAY}"
            )
        if (e - s).total_seconds() / 60 != DURATION:
            failures.append(f"booking duration != {DURATION}")
        if scenario["coworker_x"] not in b["attendee_mail_ids"]:
            failures.append(
                f"booking attendee {b['attendee_mail_ids']} != "
                f"X={scenario['coworker_x']}"
            )
        # Window.
        if s.hour < WINDOW_START:
            failures.append("booking starts before afternoon window")
        if e.hour > WINDOW_END or (e.hour == WINDOW_END and e.minute > 0):
            failures.append("booking ends after afternoon window")
        # Conflicts.
        if not is_free(scenario["viewer_busy"], s, e):
            failures.append("booking overlaps viewer busy")
        if not is_free(scenario["x_busy"], s, e):
            failures.append("booking overlaps X busy")

    if create_calls:
        ct = create_calls[0]
        args = ct.arguments
        if args.get("due_day_offset") != EXPECTED_TASK_DUE_DAY:
            failures.append(
                f"task due_day_offset={args.get('due_day_offset')} "
                f"!= expected {EXPECTED_TASK_DUE_DAY} (the day before "
                f"the meeting on day {MEETING_DAY}). Likely substituted "
                "the meeting day or 'today' for the derivation."
            )
        if args.get("title") != EXPECTED_TITLE:
            failures.append(
                f"task title={args.get('title')!r} != expected "
                f"{EXPECTED_TITLE!r}"
            )
        if args.get("priority") != EXPECTED_PRIORITY:
            failures.append(
                f"task priority={args.get('priority')} != "
                f"{EXPECTED_PRIORITY}"
            )
        if args.get("assignee_mail_id") != VIEWER:
            failures.append(
                f"task assignee={args.get('assignee_mail_id')} != "
                f"viewer {VIEWER}"
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
            OverlayAssertion(field="created_tasks", expected_count=1),
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
