"""Scenario: llm_render_thread (READ — mail service, opt-in LLM rendering).

First scenario that exercises the `llm_render: true` opt-in on the
read_thread MCP tool. Verifies:
  1. The agent can opt into LLM-rendered email content via a tool param
  2. The server doesn't error when the flag is set
  3. The agent reports the LLM-rendered subject (deterministic per id)
  4. The agent did NOT fabricate — the subject must literally come from
     the server's response

This is the bridge from "substrate is observable" to "substrate is
observable WITH realistic LLM-rendered content." Cache means re-runs
of this scenario incur zero LLM cost on the server side.

Setup pre-finds a thread + asks the server with llm_render=true so
the verdict can pin the literal subject the agent will see.

First-run cost (server side): ~5 chat completions for the messages
in the thread. Subsequent runs: $0 (cached).
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    ToolCallMatch, Verdict, call_tool, run_with_verdict, runner,
)


async def setup(session):
    """Find a viewer with at least one thread; pre-render via the
    server with llm_render=true to ground-truth the expected subject.
    """
    for viewer in [8197, 12_288, 16_384, 20_480, 24_576]:
        inbox = await call_tool(session, "get_inbox",
                                {"viewer_mail_id": viewer, "limit": 5})
        threads = inbox.get("threads", [])
        if not threads:
            continue
        target_thread_id = threads[0]["thread_id"]
        rendered = await call_tool(session, "read_thread", {
            "viewer_mail_id": viewer,
            "thread_id": target_thread_id,
            "llm_render": True,
        })
        msgs = rendered.get("messages", [])
        if not msgs:
            continue
        return {
            "viewer": viewer,
            "thread_id": target_thread_id,
            "expected_subject": msgs[0]["subject"],
        }
    return None


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to read "
        "your mail. When reading a thread, set `llm_render: true` to "
        "get realistic LLM-rendered subject and body lines instead of "
        "deterministic templates. Be concise."
    )


def user_task(s):
    return (
        f"Open thread {s['thread_id']} using `read_thread` with "
        "`llm_render` set to true. In your final message, copy the "
        "subject of the FIRST message in that thread verbatim — exactly "
        "as the tool returned it, character for character."
    )


def verdict(s) -> Verdict:
    return Verdict(
        tool_calls_must=[
            ToolCallMatch(
                name="read_thread",
                args_must_contain={
                    "thread_id": s["thread_id"],
                    "llm_render": True,
                },
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
        final_must_contain=[s["expected_subject"]],
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
