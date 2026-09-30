"""Scenario: find_top_task (READ — tasks service).

Agent must find the highest-priority OPEN/IN_PROGRESS task and
report its title. Verdict: list_tasks must be called; final message
must contain the actual title of the top task.

Tests the tasks service's list_tasks view + the agent's ability to
read a sorted list and pick the top entry.
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    ToolCallMatch, Verdict, call_tool, run_with_verdict, runner,
)


PRIORITY_RANK = {"Urgent": 4, "High": 3, "Medium": 2, "Low": 1}


async def setup(session):
    """Find a viewer with at least one task. The server's list_tasks
    returns Open/InProgress first, ordered by earliest due — so
    "the top task on my list" is just `tasks[0]` from the server's
    response. Match that exactly.
    """
    for v in [100, 1000, 4096, 8192, 12288, 16384]:
        out = await call_tool(session, "list_tasks",
                              {"viewer_mail_id": v, "limit": 25})
        tasks = out.get("tasks", [])
        if not tasks:
            continue
        top = tasks[0]
        return {
            "viewer": v,
            "expected_title": top["title"],
        }
    return None


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to manage "
        "your work. Be concise."
    )


def user_task(s):
    return (
        "Look at my task list and tell me the title of the FIRST task "
        "shown there (the top of the list — already sorted by the "
        "tool). Do NOT call any write tools (no rsvp, compose_email, "
        "send_dm, etc.). Answer me directly in your final chat "
        "message with just that task title."
    )


def verdict(s) -> Verdict:
    return Verdict(
        tool_calls_must=[
            ToolCallMatch(name="list_tasks", must_call_at_least=1),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="rsvp"),
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
            ToolCallMatch(name="mark_read"),
        ],
        # Final message must include the literal task title.
        final_must_contain=[s["expected_title"]],
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
