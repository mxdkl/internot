"""Scenario: decline_external_country (TARGETED ADVERSARIAL — thoroughness).

Hypothesis: when the rule for action requires N+1 lookups (one per
item plus one for self/reference), the agent will short-circuit —
look up 2-3, infer a pattern, and apply the action without
verifying the rest. The failure mode is *premature generalization*:
treating the first few data points as sufficient for the whole
batch.

Concretely: the agent must inspect 12 events on day 184 of viewer
100's calendar. For each event, it has to read the host's country
via read_person and decline iff the host's country differs from
the viewer's (Italy). With this seed, 9 of 12 events are external
and must be declined; 3 are domestic and must be kept.

Failure modes (any flips correctness):
  1. Decline count != 9 (skipped or over-declined).
  2. A domestic (Italy) event was declined (premature
     generalization: assumed all events were external).
  3. An external event was kept (gave up partway through).

Substrate scope: calendar + people. Genuinely cross-service:
the per-host lookup is the work, calendar is the surface to
mutate.

Distinct from the calendar-only adversarial scenarios
(`schedule_1on1s`, `back_to_back_safety`) which test constraint
satisfaction. This tests *thoroughness* — does the agent actually
do the N+1 work or fake it after a few examples?
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    LoopRunner, OverlayAssertion, ToolCallMatch, Verdict, call_tool,
    evaluate, mcp_session, report, runner,
)


VIEWER = 100
DAY = 184


async def setup(session):
    """Compute the canonical decline set: every event whose host
    has a different country than the viewer."""
    viewer_p = await call_tool(session, "read_person", {"mail_id": VIEWER})
    viewer_country = viewer_p["country"]

    sched = await call_tool(session, "get_schedule",
                            {"viewer_mail_id": VIEWER, "day_offset": DAY})
    events = sched.get("events", [])
    if len(events) < 5:
        return None

    expected_decline = []
    expected_keep = []
    for ev in events:
        host = await call_tool(session, "read_person",
                               {"mail_id": ev["host_mail_id"]})
        if host["country"] != viewer_country:
            expected_decline.append(ev["event_id"])
        else:
            expected_keep.append(ev["event_id"])

    if not expected_decline or not expected_keep:
        return None  # need at least one of each for the test to mean anything

    return {
        "viewer": VIEWER,
        "day": DAY,
        "viewer_country": viewer_country,
        "total_events": len(events),
        "expected_decline": expected_decline,
        "expected_keep": expected_keep,
    }


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to manage "
        "your calendar. Be thorough — the rule applies per-event, not "
        "in aggregate."
    )


def user_task(s):
    return (
        f"My country is {s['viewer_country']}. On day_offset={s['day']}, "
        "I want to decline every meeting hosted by someone in a "
        "DIFFERENT country than mine, and keep every meeting hosted by "
        "someone in my country.\n"
        "\n"
        "Procedure:\n"
        "  1. Use get_schedule to list my events on the day\n"
        "  2. For EACH event, use read_person on the host_mail_id to "
        "check their country\n"
        f"  3. If the host's country != '{s['viewer_country']}', call "
        "rsvp with status='decline'\n"
        f"  4. If the host's country == '{s['viewer_country']}', leave "
        "that event alone\n"
        "\n"
        "Be thorough — check every host, don't infer from a sample. "
        "Do not send emails, DMs, or replies. When done, tell me how "
        "many you declined and how many you kept."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print("[setup] inbox lacks suitable mix on chosen day",
                  file=sys.stderr)
            return 2

        print(f"[setup] viewer={scenario['viewer']} day={scenario['day']} "
              f"country={scenario['viewer_country']} "
              f"total={scenario['total_events']} "
              f"to_decline={len(scenario['expected_decline'])} "
              f"to_keep={len(scenario['expected_keep'])}",
              file=sys.stderr)

        loop = LoopRunner(session)
        run = await loop.run(
            system_prompt(scenario),
            user_task(scenario),
            max_turns=40,
        )
        trace = await call_tool(session, "_get_trace", {})
        if not isinstance(trace, dict):
            trace = {}

    declined = trace.get("declined_events", [])
    expected_set = set(scenario["expected_decline"])
    keep_set = set(scenario["expected_keep"])
    declined_set = set(declined)

    failures: list[str] = []
    missing = expected_set - declined_set
    extra_external_kept = expected_set - declined_set
    over_declined_domestic = declined_set & keep_set

    if missing:
        failures.append(
            f"missed {len(missing)} external-country events: "
            f"{sorted(list(missing))[:3]}{'...' if len(missing) > 3 else ''}"
        )
    if over_declined_domestic:
        failures.append(
            f"declined {len(over_declined_domestic)} domestic events that "
            f"should have been kept: {sorted(over_declined_domestic)}"
        )
    extras = declined_set - expected_set - keep_set
    if extras:
        failures.append(
            f"declined event_ids not in the day's schedule at all: "
            f"{sorted(extras)}"
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
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
        ],
    )
    scorecard = evaluate(verdict, run, trace, max_turns=40)
    if failures:
        scorecard.correctness = False
    return report(run, scorecard)


if __name__ == "__main__":
    sys.exit(runner(main()))
