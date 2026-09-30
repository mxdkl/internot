"""Scenario: temporal_projection (READ — people service, Tier 4 surface).

The first scenario that exercises `read_person_at_time`, the new Tier 4
view that projects a person's age + lifecycle_phase to an arbitrary
time anchor. Chains:

  find_people(peak-career contributor in Information industry)
      → take first match
      → read_person_at_time(mail_id, "2035-06-30T09:00:00Z")
      → report the projected lifecycle_phase_label

Why this scenario matters: it proves the TEMPORAL substrate is
observable end to end — agents can reason about a person's state at
times other than the simulation's DEFAULT_NOW.

Pairs with the Rust dry-run in `internot/tests/temporal_projection_chain.rs`
which verifies the same chain composes deterministically without
API spend.
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    ToolCallMatch, Verdict, call_tool, run_with_verdict, runner,
)


TARGET_INDUSTRY_IDX = 0           # Information
TARGET_ENGAGEMENT = "contributor"
TARGET_LIFECYCLE_PHASE = 3        # peak-career (40-49)
PROJECTION_DATE = "2035-06-30T09:00:00Z"  # 10 years past DEFAULT_NOW
EXPECTED_FUTURE_PHASE_LABEL = "late-career"  # 50-59 → phase 4


async def setup(session):
    """Pre-find a peak-career contributor + ground-truth their projected
    phase at the future date so the verdict can spot hallucination.
    """
    cohort = await call_tool(session, "find_people", {
        "industry_idx": TARGET_INDUSTRY_IDX,
        "engagement": TARGET_ENGAGEMENT,
        "lifecycle_phase": TARGET_LIFECYCLE_PHASE,
        "limit": 1,
        "scan_budget": 500_000,
    })
    matched = cohort.get("matched", [])
    if not matched:
        return None

    target = matched[0]
    target_mail_id = target["mail_id"]

    proj = await call_tool(session, "read_person_at_time", {
        "mail_id": target_mail_id,
        "when": PROJECTION_DATE,
    })

    return {
        "target_mail_id": target_mail_id,
        "expected_label": proj["lifecycle_phase_label"],
        "expected_age": proj["age"],
    }


def system_prompt(s):
    return (
        "You are a demographer querying a procedural population. "
        "Use the MCP tools to find people and project their lifecycle "
        "phase to a specified future date. Be concise."
    )


def user_task(s):
    return (
        f"Use `find_people` to find a contributor in industry_idx="
        f"{TARGET_INDUSTRY_IDX} (Information industry) with "
        f"lifecycle_phase={TARGET_LIFECYCLE_PHASE} (peak-career). "
        "Take the FIRST result. Then call `read_person_at_time` on "
        f"their mail_id with when=\"{PROJECTION_DATE}\" to project "
        "10 years forward. In your final message, copy the value of "
        "`lifecycle_phase_label` from that response verbatim — just "
        "the label string, nothing else."
    )


def verdict(s) -> Verdict:
    return Verdict(
        tool_calls_must=[
            ToolCallMatch(
                name="find_people",
                args_must_contain={
                    "industry_idx": TARGET_INDUSTRY_IDX,
                    "engagement": TARGET_ENGAGEMENT,
                    "lifecycle_phase": TARGET_LIFECYCLE_PHASE,
                },
                must_call_at_least=1,
            ),
            ToolCallMatch(
                name="read_person_at_time",
                args_must_contain={"when": PROJECTION_DATE},
                must_call_at_least=1,
            ),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="rsvp"),
            ToolCallMatch(name="send_dm"),
            ToolCallMatch(name="book_meeting"),
        ],
        # The expected label is deterministic per the Rust dry-run:
        # peak-career (40-49) + 10y → 50-59 → "late-career". Asserting
        # the literal label catches hallucination AND verifies the
        # agent actually read the right field.
        final_must_contain=[s["expected_label"]],
    )


async def main() -> int:
    return await run_with_verdict(
        setup_fn=setup,
        system_prompt_fn=system_prompt,
        user_task_fn=user_task,
        verdict_fn=verdict,
        max_turns=10,
    )


if __name__ == "__main__":
    sys.exit(runner(main()))
