"""Scenario: dm_about_thread_recipient (TARGETED ADVERSARIAL — channel discrimination).

Hypothesis: when the user asks the agent to act on Service A about a
person discovered via Service B, the agent will conflate the two
channels — e.g., asked to DM the sender of an email, it'll instead
reply to the email. Tool-use models often default to the most
recently-loaded surface for the next mutation, regardless of the
explicit channel instruction.

Concretely: the agent must read the latest received inbox thread to
identify the sender, then send a DM (not a reply email) to that
sender's mail_id. The body is a specific phrase. The trap is the
agent invoking `reply` on the email thread instead of `send_dm` to
the same person.

Failure modes:
  1. Agent calls `reply` on the email thread (channel confusion —
     wrong mutation type).
  2. Agent calls `send_dm` to the wrong person (didn't actually
     check who the sender was).
  3. Agent calls both `reply` AND `send_dm` (over-action — can't
     decide).
  4. Agent's DM body doesn't contain the required phrase.

Substrate scope: mail + chat + people. Cross-service.

Distinct from `dm_a_coworker` (which just sends a DM to anyone)
and the calendar adversarial scenarios (which test constraint
satisfaction). This one tests that the agent picks the RIGHT
mutation surface from multiple available ones.
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
REQUIRED_PHRASE = "will reply by EOD"


async def setup(session):
    """Pick the latest inbox thread; identify its sender (the
    from_mail_id != viewer). The agent must DM that sender."""
    out = await call_tool(session, "get_inbox",
                          {"viewer_mail_id": VIEWER, "limit": 5})
    threads = out.get("threads", [])
    if not threads:
        return None

    # Find the first thread where the sender isn't the viewer (so
    # there's actually a different person to DM).
    target = None
    for t in threads:
        if t["from_mail_id"] != VIEWER:
            target = t
            break
    if target is None:
        return None

    return {
        "viewer": VIEWER,
        "target_thread_id": target["thread_id"],
        "target_sender_mail_id": target["from_mail_id"],
        "target_sender_handle": target["from_handle"],
        "target_subject": target["subject"],
        "required_phrase": REQUIRED_PHRASE,
    }


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to manage "
        "your communication. Pay attention to which channel I ask "
        "you to use — email and direct messaging are separate."
    )


def user_task(s):
    return (
        f"Look at my most recent received inbox thread (thread_id "
        f"{s['target_thread_id']}, from {s['target_sender_handle']}, "
        f"mail_id {s['target_sender_mail_id']}, subject "
        f"\"{s['target_subject']}\").\n"
        "\n"
        f"Send a DIRECT MESSAGE to that sender (mail_id "
        f"{s['target_sender_mail_id']}) — use the send_dm tool, NOT "
        "the reply tool. The DM body must contain the exact phrase: "
        f"'{REQUIRED_PHRASE}'.\n"
        "\n"
        "Important: do NOT reply to the email thread. Do NOT compose "
        "a new email. Do NOT touch the calendar. The task is a "
        "single send_dm call. When done, tell me the dm_id and "
        "message_id you got back."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print("[setup] no suitable inbox thread", file=sys.stderr)
            return 2

        print(f"[setup] viewer={scenario['viewer']} "
              f"target_sender={scenario['target_sender_mail_id']} "
              f"thread={scenario['target_thread_id']}",
              file=sys.stderr)

        loop = LoopRunner(session)
        run = await loop.run(
            system_prompt(scenario),
            user_task(scenario),
            max_turns=15,
        )
        trace = await call_tool(session, "_get_trace", {})
        if not isinstance(trace, dict):
            trace = {}

    sent_dms = trace.get("sent_dms", [])
    replied = trace.get("replied_threads", [])
    composed = trace.get("composed_messages", [])

    failures: list[str] = []

    if not sent_dms:
        failures.append("no DM was sent (the required action was skipped)")
    if len(sent_dms) > 1:
        failures.append(f"sent {len(sent_dms)} DMs; expected exactly 1")

    if replied:
        failures.append(
            f"replied to {len(replied)} email thread(s); the task "
            f"required DM only"
        )
    if composed:
        failures.append(
            f"composed {len(composed)} new email(s); the task required "
            f"DM only"
        )

    # Verify the DM body contained the required phrase by inspecting
    # the agent's send_dm call arguments.
    dm_body_ok = False
    dm_target_ok = False
    for tc in run.tool_calls:
        if tc.name != "send_dm":
            continue
        body = tc.arguments.get("body", "")
        other = tc.arguments.get("other_mail_id")
        if REQUIRED_PHRASE in body:
            dm_body_ok = True
        if other == scenario["target_sender_mail_id"]:
            dm_target_ok = True
        # Allow dm_id-based send_dm too — we'd need to resolve dm_id
        # back to participants; skip that check unless other_mail_id is
        # supplied. (The user task explicitly asks for "send_dm to mail_id
        # X", so we expect other_mail_id to be set.)

    if sent_dms and not dm_body_ok:
        failures.append(
            f"DM body did not contain required phrase '{REQUIRED_PHRASE}'"
        )
    if sent_dms and not dm_target_ok:
        failures.append(
            f"DM was not addressed to target_sender mail_id "
            f"{scenario['target_sender_mail_id']}"
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
            OverlayAssertion(field="sent_dms", expected_count=1),
        ],
        # Hard rejection of the wrong channels:
        overlay_must_not=[
            OverlayAssertion(field="replied_threads",
                             expected_count=0,
                             must_not_contain=[scenario["target_thread_id"]]),
            OverlayAssertion(field="composed_messages",
                             expected_count=0),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="rsvp"),
            ToolCallMatch(name="book_meeting"),
        ],
    )
    scorecard = evaluate(verdict, run, trace, max_turns=15)
    if failures:
        scorecard.correctness = False
    return report(run, scorecard)


if __name__ == "__main__":
    sys.exit(runner(main()))
