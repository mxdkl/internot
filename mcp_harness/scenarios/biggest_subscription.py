"""Scenario: biggest_subscription (READ — money service).

Agent must scan the viewer's transactions, find the largest
Subscription-category charge, and report the vendor name. Verdict:
list_transactions called; final message contains the actual vendor.

Tests money's transaction listing + filtering reasoning over a
heterogeneous list.
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    ToolCallMatch, Verdict, call_tool, run_with_verdict, runner,
)


def cat_label(t):
    c = t.get("category")
    return list(c.keys())[0] if isinstance(c, dict) else c


async def setup(session):
    """Find a viewer whose default-limit transaction list contains
    subscription charges. Setup queries the same default the agent
    will see (no `limit` param) so the answer is well-defined in
    that view.
    """
    for v in [100, 1000, 4096, 8192, 12288, 16384]:
        out = await call_tool(session, "list_transactions",
                              {"viewer_mail_id": v})
        txns = out.get("transactions", [])
        subs = [t for t in txns if cat_label(t) == "Subscription"]
        if not subs:
            continue
        biggest = max(subs, key=lambda t: abs(t["amount_cents"]))
        return {
            "viewer": v,
            "expected_vendor": biggest["vendor"],
            "expected_amount_cents": abs(biggest["amount_cents"]),
            "subscription_count": len(subs),
        }
    return None


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to read "
        "your bank transactions. Be concise."
    )


def user_task(s):
    return (
        "Look at my recent bank transactions. Find my largest "
        "Subscription-category charge and tell me which vendor it's "
        "from. Do NOT call any write tools. Answer me directly in "
        "your final chat message with just the vendor name."
    )


def verdict(s) -> Verdict:
    return Verdict(
        tool_calls_must=[
            ToolCallMatch(name="list_transactions", must_call_at_least=1),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="rsvp"),
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
        ],
        final_must_contain=[s["expected_vendor"]],
    )


async def main() -> int:
    return await run_with_verdict(
        setup_fn=setup,
        system_prompt_fn=system_prompt,
        user_task_fn=user_task,
        verdict_fn=verdict,
        max_turns=8,
    )


if __name__ == "__main__":
    sys.exit(runner(main()))
