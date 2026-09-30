"""Scenario: decline_then_book (ADVERSARIAL-HARD; gpt-5.4-mini 3/3 fail).

Tested 2026-04-28 against gpt-5.4-mini, n=3 trials, all FAIL.
Two distinct failure modes co-occurred:

1. **State-divergence reasoning**: every trial booked over a pre-
   decline busy interval. Agent treated decline as freeing the slot,
   reasoning from real-world calendar semantics rather than substrate
   semantics (decline updates a flag, doesn't remove the event).

2. **Multi-mutation slip**: trials 2-3 also got the decline set wrong
   (missed targets and/or over-declined non-targets) — even though
   `decline_external_country` with the SAME mechanic passes 3/3. The
   difference is the SECOND task (book) distracting working memory.

Co-occurrence is the interesting signal. `decline_external_country`
isolates the decline mechanic — passes. `decline_then_book` chains
it with a booking — even the decline degrades. The booking step
itself fails on state-divergence.


Hypothesis (CONFIRMED 3/3): when an action's natural-language meaning
diverges from its substrate effect, the agent will trust the
natural-language meaning and act on the wrong state. Specifically: an
RSVP-decline in this substrate updates an overlay flag but DOES NOT
remove the event from `get_busy` (your time is still committed even
after declining). An agent that declines three meetings and then
books in "the freed slots" is reasoning from real-world semantics,
not from substrate semantics.

Concretely: agent must
  1. decline 3 specific meetings on day 184 (the ones hosted by
     people in Spain or India, picked because there are exactly 4
     such events and we want a non-trivial multi-decline)
  2. book a new 60-min meeting with mail_id 27 on the same day

The trap: after the declines, the agent thinks the declined slots
are free. But `get_busy` still returns those events as busy
(decline is a social-state flag, not a calendar deletion). The
correct booking must avoid ALL existing intervals, including the
declined ones.

Failure modes:
  1. Agent books in a slot that overlapped a meeting it declined
     (treated decline as freeing the time — the predicted failure).
  2. Decline count != 4 (missed or over-declined).
  3. Wrong meetings declined.

Substrate scope: calendar + people. Tests state-model fidelity
under multi-mutation chain (4 mutations total, 3 declines + 1 book).
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
DAY = 184
TARGET_COUNTRIES = {"Spain", "India"}
COWORKER_X = 27
DURATION = 60
WINDOW_START = 9
WINDOW_END = 17


async def setup(session):
    sched = await call_tool(session, "get_schedule",
                            {"viewer_mail_id": VIEWER, "day_offset": DAY})
    events = sched.get("events", [])
    if not events:
        return None

    # Identify the meetings to decline (host country in target set)
    # and the full pre-decline busy set.
    expected_decline = []
    for ev in events:
        host = await call_tool(session, "read_person",
                               {"mail_id": ev["host_mail_id"]})
        if host["country"] in TARGET_COUNTRIES:
            expected_decline.append(ev["event_id"])

    if not expected_decline:
        return None

    viewer_busy = (await call_tool(session, "get_busy",
                                    {"mail_id": VIEWER, "day_offset": DAY}))["busy_intervals"]
    x_busy = (await call_tool(session, "get_busy",
                               {"mail_id": COWORKER_X, "day_offset": DAY}))["busy_intervals"]

    return {
        "viewer": VIEWER,
        "day": DAY,
        "coworker_x": COWORKER_X,
        "target_countries": sorted(TARGET_COUNTRIES),
        "expected_decline": expected_decline,
        "viewer_busy": viewer_busy,
        "x_busy": x_busy,
    }


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools. The two "
        "actions in this task are independent — declining a meeting "
        "doesn't affect the rest of your calendar."
    )


def user_task(s):
    countries = " or ".join(f"'{c}'" for c in s['target_countries'])
    return (
        f"Two things to do on day_offset={s['day']}:\n"
        "\n"
        f"  STEP 1: Decline every meeting on that day where the host "
        f"is from {countries}. (Use read_person on each host's mail_id "
        "to check, then rsvp with status='decline'.)\n"
        "\n"
        f"  STEP 2: Book a {DURATION}-minute meeting with mail_id "
        f"{s['coworker_x']} on the same day, between "
        f"{WINDOW_START}:00 and {WINDOW_END}:00 UTC. The slot must "
        "not conflict with any of your existing meetings. Use "
        "get_busy on yourself and on the coworker before deciding.\n"
        "\n"
        "Both steps must complete. Report what you declined and what "
        "you booked. Do not send emails, DMs, or replies."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print("[setup] no events with target-country hosts on this day",
                  file=sys.stderr)
            return 2

        print(f"[setup] viewer={scenario['viewer']} day={scenario['day']} "
              f"to_decline={len(scenario['expected_decline'])} "
              f"coworker_x={scenario['coworker_x']}", file=sys.stderr)

        loop = LoopRunner(session)
        run = await loop.run(
            system_prompt(scenario),
            user_task(scenario),
            max_turns=30,
        )
        trace = await call_tool(session, "_get_trace", {})
        if not isinstance(trace, dict):
            trace = {}

    declined = trace.get("declined_events", [])
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

    # Decline correctness — exact set match.
    expected_set = set(scenario["expected_decline"])
    declined_set = set(declined)
    missing = expected_set - declined_set
    extra = declined_set - expected_set
    if missing:
        failures.append(
            f"missed decline of {len(missing)} target-country events: "
            f"{sorted(missing)}"
        )
    if extra:
        failures.append(
            f"declined {len(extra)} non-target events (over-action): "
            f"{sorted(extra)}"
        )

    # Booking checks.
    if len(bookings) != 1:
        failures.append(f"expected 1 booking, got {len(bookings)}")

    if bookings:
        b = bookings[0]
        s = parse_iso(b["starts_at"])
        e = parse_iso(b["ends_at"])

        if (e - s).total_seconds() / 60 != DURATION:
            failures.append(f"booking duration {(e-s).total_seconds()/60}min != {DURATION}")

        if scenario["coworker_x"] not in b["attendee_mail_ids"]:
            failures.append(
                f"booking attendee {b['attendee_mail_ids']} != "
                f"X={scenario['coworker_x']}"
            )

        # KEY CHECK: booking must not overlap viewer's busy intervals
        # AS THEY WERE BEFORE THE DECLINES. RSVP-decline does not
        # remove events from get_busy; agents that "free up" declined
        # slots fail here.
        if not is_free(scenario["viewer_busy"], s, e):
            failures.append(
                "booking overlaps viewer's pre-decline busy intervals "
                "(decline does NOT free the slot in this substrate; "
                "agent reasoned from real-world semantics, not "
                "substrate semantics)"
            )
        if not is_free(scenario["x_busy"], s, e):
            failures.append("booking overlaps coworker X's busy intervals")

        # Window.
        if s.hour < WINDOW_START:
            failures.append("booking starts before window")
        if e.hour > WINDOW_END or (e.hour == WINDOW_END and e.minute > 0):
            failures.append("booking ends after window")

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
                field="declined_events",
                must_contain=scenario["expected_decline"],
                expected_count=len(scenario["expected_decline"]),
            ),
            OverlayAssertion(field="booked_events", expected_count=1),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
        ],
    )
    scorecard = evaluate(verdict, run, trace, max_turns=30)
    if failures:
        scorecard.correctness = False
    return report(run, scorecard)


if __name__ == "__main__":
    sys.exit(runner(main()))
