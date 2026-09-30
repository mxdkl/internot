"""Scenario: graph_traversal (READ — people service, Tier 3 surface).

Exercises Tier 3 graph stubs end to end. The recovered neighbor
mail_ids (spouse / manager / mentor / frequent_collab) live as flat
fields on PersonAvm — agents traverse the workplace graph by
chaining find_people → read_person(employee) → read_person(manager)
without needing a separate "read_neighbors" tool.

Chain:
  find_people(industry+engagement filter) → take first match
      → read_person(matched_id) ← the employee's PersonAvm has
        manager_mail_id pre-recovered
      → read_person(manager_mail_id) ← traverse the graph
      → tell me the manager's name and city

Verdict pins:
  - find_people called with the cohort filter
  - read_person called twice (once per person)
  - final_must_contain: the manager's literal name (deterministic
    via setup ground-truth)

Pairs with internot/tests/graph_traversal_chain.rs which verified
the substrate-level invariants AND caught a real coherence bug
(country_idx_of was hashing on member_idx instead of workplace bits,
so manager and employee in the same workplace got different
countries — fixed in the same change as this scenario landed).
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    ToolCallMatch, Verdict, call_tool, run_with_verdict, runner,
)


TARGET_INDUSTRY_IDX = 0
TARGET_ENGAGEMENT = "contributor"


async def setup(session):
    """Pre-find an employee + their manager so the verdict can pin
    the manager's literal name.
    """
    cohort = await call_tool(session, "find_people", {
        "industry_idx": TARGET_INDUSTRY_IDX,
        "engagement": TARGET_ENGAGEMENT,
        "limit": 1,
        "scan_budget": 100_000,
    })
    matched = cohort.get("matched", [])
    if not matched:
        return None
    employee = matched[0]

    manager = await call_tool(session, "read_person", {
        "mail_id": employee["manager_mail_id"],
    })
    expected_full = f"{manager['name_first']} {manager['name_last']}"

    return {
        "employee_mail_id": employee["mail_id"],
        "manager_mail_id": employee["manager_mail_id"],
        "expected_manager_name": expected_full,
    }


def system_prompt(s):
    return (
        "You are an analyst querying a procedural population. Use "
        "the MCP tools to navigate workplace relationships. Be concise."
    )


def user_task(s):
    return (
        f"Use `find_people` with industry_idx={TARGET_INDUSTRY_IDX} "
        f"and engagement=\"{TARGET_ENGAGEMENT}\" (limit 1). The "
        "returned PersonAvm has a `manager_mail_id` field. Call "
        "`read_person` on that mail_id to look up the manager. In "
        "your final message, copy the manager's full name (first "
        "and last, exactly as the tool returned them) verbatim."
    )


def verdict(s) -> Verdict:
    return Verdict(
        tool_calls_must=[
            ToolCallMatch(
                name="find_people",
                args_must_contain={
                    "industry_idx": TARGET_INDUSTRY_IDX,
                    "engagement": TARGET_ENGAGEMENT,
                },
                must_call_at_least=1,
            ),
            ToolCallMatch(
                name="read_person",
                args_must_contain={"mail_id": s["manager_mail_id"]},
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
        final_must_contain=[s["expected_manager_name"]],
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
