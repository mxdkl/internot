"""Scenario: decline_then_book_chain (M2 ADVERSARIAL-HARD; gpt-5.4-mini 3/3 fail).

Tested 2026-04-28 against gpt-5.4-mini, n=3 trials, all FAIL with
multiple violations per trial (the compound surface fires multiple
modes at once, not just one):
  - Trial 1: STATE-DIVERGENCE on meeting #1 (booked over a
    declined slot)
  - Trial 2: STATE-DIVERGENCE on meeting #2 + per-attendee X
    constraint dropped
  - Trial 3: multi-mutation slip (missed 1 decline) +
    STATE-DIVERGENCE on #2 + per-attendee Y dropped + window
    violation (4 distinct violations in one run)

Compound conjunction CONFIRMED — the failure modes amplify rather
than mask each other. Useful pattern for designing diagnostic-rich
scenarios going forward.



Targets the conjunction of two characterized failure modes:
  - STATE-DIVERGENCE (from decline_then_book 3/3 fail): agent
    treats RSVP-decline as freeing the slot in get_busy. Books over
    declined times.
  - FAN-OUT × CONSTRAINT-SATISFACTION (from
    back_to_back_safety 2/3 fail, schedule_1on1s 3/3, etc.):
    multiple coupled bookings drop joint constraints.

This scenario chains them: agent must decline events, THEN book a
contiguous pair of meetings. The pair has to:
  - respect viewer's pre-decline busy (decline doesn't free)
  - respect each attendee's busy
  - be back-to-back (booking 2 starts exactly when booking 1 ends)

Three constraint surfaces stacked. `decline_then_book` produced
3/3 fail with a single follow-up booking; with TWO bookings and
contiguity, predicted failure is at least as hard. The interesting
data is per-trial breakdown: does the agent fail on
state-divergence (the easy-to-test trap), fan-out (overlapping
bookings), or both?

Failure modes (any flips correctness):
  1. Decline set incorrect (multi-mutation slip).
  2. Booking 1 overlaps viewer's pre-decline busy intervals
     (STATE-DIVERGENCE).
  3. Booking 2 starts at a time other than booking 1's end
     (contiguity violated, FAN-OUT).
  4. Booking 2 overlaps viewer's pre-decline busy intervals
     (STATE-DIVERGENCE compounded).
  5. Booking 1 overlaps X's busy / booking 2 overlaps Y's busy
     (per-attendee constraint dropped).
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
LONG_DURATION = 60
SHORT_DURATION = 30
WINDOW_START = 9
WINDOW_END = 17


async def setup(session):
    """Identify the events to decline (target-country hosts on DAY)
    and pick two free coworkers + a feasible back-to-back slot that
    respects viewer's busy intervals, including the to-be-declined
    ones (decline doesn't free the slot per substrate semantics)."""
    sched = await call_tool(session, "get_schedule",
                            {"viewer_mail_id": VIEWER, "day_offset": DAY})
    events = sched.get("events", [])
    if not events:
        return None

    expected_decline = []
    for ev in events:
        host = await call_tool(session, "read_person",
                               {"mail_id": ev["host_mail_id"]})
        if host["country"] in TARGET_COUNTRIES:
            expected_decline.append(ev["event_id"])
    if len(expected_decline) < 2:
        return None

    viewer_busy = (await call_tool(session, "get_busy",
                                    {"mail_id": VIEWER, "day_offset": DAY}))["busy_intervals"]

    # Find two coworkers + a slot triple where:
    #   viewer free for [start, start + 90]
    #   X free for      [start, start + 60]
    #   Y free for      [start + 60, start + 90]
    dms = await call_tool(session, "list_dms",
                          {"viewer_mail_id": VIEWER, "limit": 25})
    candidates = [d["other_party"]["mail_id"] for d in dms.get("dms", [])
                  if d["other_party"]["mail_id"] != VIEWER]
    if len(candidates) < 2:
        return None

    base = datetime(2025, 1, 1) + timedelta(days=DAY)
    busy_cache = {}

    async def busy(mid):
        if mid not in busy_cache:
            r = await call_tool(session, "get_busy",
                                {"mail_id": mid, "day_offset": DAY})
            busy_cache[mid] = r["busy_intervals"]
        return busy_cache[mid]

    chosen = None
    for i, x in enumerate(candidates):
        x_busy = await busy(x)
        for y in candidates[i + 1:]:
            y_busy = await busy(y)
            for h in range(WINDOW_START, WINDOW_END):
                for m in (0, 30):
                    s1 = base + timedelta(hours=h, minutes=m)
                    e1 = s1 + timedelta(minutes=LONG_DURATION)
                    s2 = e1
                    e2 = s2 + timedelta(minutes=SHORT_DURATION)
                    if e2 > base + timedelta(hours=WINDOW_END):
                        continue
                    if (is_free(viewer_busy, s1, e2)
                            and is_free(x_busy, s1, e1)
                            and is_free(y_busy, s2, e2)):
                        chosen = (x, y, x_busy, y_busy)
                        break
                if chosen:
                    break
            if chosen:
                break
        if chosen:
            break
    if chosen is None:
        return None
    x, y, x_busy, y_busy = chosen

    return {
        "viewer": VIEWER,
        "day": DAY,
        "target_countries": sorted(TARGET_COUNTRIES),
        "expected_decline": expected_decline,
        "invitee_x": x,
        "invitee_y": y,
        "viewer_busy": viewer_busy,
        "x_busy": x_busy,
        "y_busy": y_busy,
    }


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools. The two "
        "groups of actions are independent — declining a meeting "
        "doesn't change the times those events occupy on your "
        "calendar (it only updates a flag). Plan the bookings as if "
        "the events were still there."
    )


def user_task(s):
    countries = " or ".join(f"'{c}'" for c in s['target_countries'])
    return (
        f"Two things to do on day_offset={s['day']}:\n"
        "\n"
        f"  STEP 1: Decline every meeting on that day where the host "
        f"is from {countries}. (Use read_person on each host's "
        "mail_id to check, then rsvp with status='decline'.)\n"
        "\n"
        f"  STEP 2: Book TWO meetings BACK-TO-BACK with these "
        "constraints:\n"
        f"    - meeting #1: {LONG_DURATION}-minute with mail_id "
        f"{s['invitee_x']}\n"
        f"    - meeting #2: {SHORT_DURATION}-minute with mail_id "
        f"{s['invitee_y']}, starting EXACTLY when meeting #1 ends\n"
        f"    - both fully within {WINDOW_START}:00..{WINDOW_END}:00 UTC\n"
        f"    - both respect MY existing busy intervals (use "
        "get_busy on me to check)\n"
        f"    - meeting #1 respects mail_id {s['invitee_x']}'s busy\n"
        f"    - meeting #2 respects mail_id {s['invitee_y']}'s busy\n"
        "\n"
        "Both steps must complete. Report what you declined and "
        "what you booked. Do not send emails, DMs, or replies."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print("[setup] couldn't satisfy decline + back-to-back "
                  "feasibility on this seed", file=sys.stderr)
            return 2

        print(f"[setup] viewer={scenario['viewer']} day={scenario['day']} "
              f"to_decline={len(scenario['expected_decline'])} "
              f"X={scenario['invitee_x']} Y={scenario['invitee_y']}",
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

    # === Decline set (multi-mutation slip surface) ===
    expected_set = set(scenario["expected_decline"])
    declined_set = set(declined)
    missing = expected_set - declined_set
    extra = declined_set - expected_set
    if missing:
        failures.append(
            f"missed {len(missing)} target-country events: "
            f"{sorted(missing)[:3]}"
        )
    if extra:
        failures.append(
            f"declined {len(extra)} non-target events (over-action): "
            f"{sorted(extra)[:3]}"
        )

    # === Bookings count + structure (fan-out surface) ===
    if len(bookings) != 2:
        failures.append(f"expected 2 bookings, got {len(bookings)}")

    if len(bookings) >= 2:
        b1, b2 = bookings[0], bookings[1]
        s1 = parse_iso(b1["starts_at"])
        e1 = parse_iso(b1["ends_at"])
        s2 = parse_iso(b2["starts_at"])
        e2 = parse_iso(b2["ends_at"])

        # Durations.
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

        # Attendees.
        if scenario["invitee_x"] not in b1["attendee_mail_ids"]:
            failures.append(
                f"meeting #1 attendee != X={scenario['invitee_x']}"
            )
        if scenario["invitee_y"] not in b2["attendee_mail_ids"]:
            failures.append(
                f"meeting #2 attendee != Y={scenario['invitee_y']}"
            )

        # Contiguity (fan-out trap).
        if s2 != e1:
            failures.append(
                f"FAN-OUT: meeting #2 starts {s2.isoformat()} but "
                f"meeting #1 ends {e1.isoformat()} (zero gap required)"
            )

        # State-divergence trap: bookings vs PRE-decline busy.
        # `decline` only updates an overlay flag; the events stay on
        # get_busy. Agents reasoning from real-world calendar
        # semantics ("declined = free") will fail here.
        if not is_free(scenario["viewer_busy"], s1, e1):
            failures.append(
                f"STATE-DIVERGENCE: meeting #1 overlaps viewer's "
                f"pre-decline busy intervals (decline does NOT free "
                f"the slot in this substrate)"
            )
        if not is_free(scenario["viewer_busy"], s2, e2):
            failures.append(
                f"STATE-DIVERGENCE: meeting #2 overlaps viewer's "
                f"pre-decline busy intervals"
            )

        # Per-attendee respects.
        if not is_free(scenario["x_busy"], s1, e1):
            failures.append("meeting #1 overlaps X's busy")
        if not is_free(scenario["y_busy"], s2, e2):
            failures.append("meeting #2 overlaps Y's busy")

        # Window.
        win_start = datetime(2025, 1, 1) + timedelta(days=scenario['day']) + timedelta(hours=WINDOW_START)
        win_end = datetime(2025, 1, 1) + timedelta(days=scenario['day']) + timedelta(hours=WINDOW_END)
        if s1 < win_start or e2 > win_end:
            failures.append(
                f"chain {s1.isoformat()}..{e2.isoformat()} outside "
                f"window {win_start.isoformat()}..{win_end.isoformat()}"
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
            OverlayAssertion(
                field="declined_events",
                must_contain=scenario["expected_decline"],
                expected_count=len(scenario["expected_decline"]),
            ),
            OverlayAssertion(field="booked_events", expected_count=2),
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
