"""Scenario: find_file_in_email (READ).

Agent gets a thread_id and is asked to find the file mentioned in
its first message by name. Verdict: did the agent call `read_file`
with the same `file_id` that was in the message's `referenced_files`?

This is the canonical read scenario: nothing mutates, so overlay is
silent — the only signal lives in the tool-call trace.
"""
from __future__ import annotations

import sys
from pathlib import Path

# Allow `import harness` from this script's parent directory.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from harness import (  # noqa: E402
    OverlayAssertion, ToolCallMatch, Verdict, call_tool, run_with_verdict,
    runner,
)


async def setup(session):
    """Sweep viewers/threads until we find one whose first message
    references a file owned by the viewer themselves."""
    for ws in range(64):
        for mi in range(8):
            viewer = (ws << 12) | mi
            if viewer == 0:
                continue
            inbox = await call_tool(session, "get_inbox",
                                    {"viewer_mail_id": viewer})
            for thread in inbox.get("threads", []):
                tid = thread["thread_id"]
                t = await call_tool(session, "read_thread",
                                    {"viewer_mail_id": viewer, "thread_id": tid})
                for m in t.get("messages", []):
                    for f in m.get("referenced_files", []):
                        if f["owner_mail_id"] == viewer:
                            return {
                                "viewer": viewer,
                                "thread_id": tid,
                                "file_id": f["file_id"],
                                "file_name": f["name"],
                            }
    return None


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to read "
        "your inbox / drive / tasks. Be concise — call tools, read "
        "results, then answer."
    )


def user_task(s):
    return (
        f"Open thread {s['thread_id']}. The first message references "
        "at least one file. Find that file in your drive and call "
        "read_file with the matching file_id. End with a brief final "
        "message naming the file."
    )


def verdict(s) -> Verdict:
    return Verdict(
        # Read scenario — primary signal is the tool call.
        tool_calls_must=[
            ToolCallMatch(
                name="read_file",
                args_must_contain={"file_id": s["file_id"]},
                must_call_at_least=1,
            ),
        ],
        # Optional: final message should mention the actual file name.
        final_must_contain=[s["file_name"]],
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
