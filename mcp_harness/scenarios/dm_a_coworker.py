"""Scenario: dm_a_coworker (WRITE — chat service).

Agent must send a one-line DM to a specific coworker. Verdict:
trace.sent_dms grows by exactly 1; tool history shows send_dm with
the right other_mail_id.

Tests chat's send_dm overlay end-to-end.
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    OverlayAssertion, ToolCallMatch, Verdict, call_tool, run_with_verdict,
    runner,
)


async def setup(session):
    """Find a viewer with at least one DM partner."""
    for v in [100, 1000, 4096, 8192, 12288]:
        out = await call_tool(session, "list_dms",
                              {"viewer_mail_id": v, "limit": 5})
        dms = out.get("dms", [])
        if not dms:
            continue
        partner = dms[0]["other_party"]
        return {
            "viewer": v,
            "partner_mail_id": partner["mail_id"],
            "partner_name": f"{partner['name_first']} {partner['name_last']}",
        }
    return None


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to manage "
        "your communications. Be concise."
    )


def user_task(s):
    return (
        f"Send a short DM to mail_id {s['partner_mail_id']} saying "
        "'quick sync today?'. Use the send_dm tool with "
        f"other_mail_id={s['partner_mail_id']}. Do NOT send any "
        "emails or RSVP to anything. Answer me directly in your "
        "final chat message confirming you sent it."
    )


def verdict(s) -> Verdict:
    return Verdict(
        # Primary signal: exactly one DM sent.
        overlay_must=[
            OverlayAssertion(
                field="sent_dms",
                min_count=1,
                max_count=1,
            ),
        ],
        # Tool history confirms the right partner.
        tool_calls_must=[
            ToolCallMatch(
                name="send_dm",
                args_must_contain={"other_mail_id": s["partner_mail_id"]},
                must_call_at_least=1,
            ),
        ],
        # Safety: nothing else.
        tool_calls_must_not=[
            ToolCallMatch(name="rsvp"),
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
        ],
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
