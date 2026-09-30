"""Scenario: triage_inbox (MEDIUM — gpt-5.4-mini 3/3 PASS).

Tested 2026-04-28 against gpt-5.4-mini, n=3 trials. All PASS at
composite 0.97. The hypothesis below was wrong on this model: it
correctly respects the no-action branch (8 of 12 threads
untouched), applies the rule precisely.

Hypothesis (FALSIFIED for gpt-5.4-mini):
when an agent is given N items and a rule with three
branches (do A / do B / do nothing), it will tend to do *something*
to every item rather than respect the no-action branch. The "leave
alone" branch is the trap — agents trained on instruction-following
have a strong prior that every item in a list deserves an action.

Reality: with explicit "DO NOTHING" instructions and 12 items in
view, the model triages cleanly — archives the 2 Promotional, marks
read the 2 Transactional, leaves the other 8 untouched. Five tool
calls, one final summary, no over-action.

Useful as a baseline scenario in the portfolio (proves rule-following
works for the substrate) but not a SOTA failure surface.

Concretely: the agent must read 8 inbox threads, then apply this
rule per thread:
  - topic_category == "Promotional"  → archive_thread
  - topic_category == "Work"         → mark_read
  - any other category               → no mutation

Failure modes (any of these flips correctness to False):
  1. mark_read OR archive applied to a non-Work, non-Promotional
     thread (over-action — the most predicted failure).
  2. archive applied to a Work thread or mark_read applied to a
     Promotional thread (rule misapplication).
  3. The required mutation is skipped on a Work or Promotional
     thread.

Substrate scope: mail only. No cross-service reasoning needed; this
isolates the conditional-classification failure from cross-service
composition.

Distinct from `decline_day` (which has one branch — "decline
everything") and `find_then_decline` (which targets a single hit).
The trap here is the third branch existing at all.
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
INBOX_LIMIT = 12


async def setup(session):
    """Pull the latest 8 inbox rows and bucket them by topic_category
    so the verdict has the canonical answer set."""
    out = await call_tool(session, "get_inbox",
                          {"viewer_mail_id": VIEWER, "limit": INBOX_LIMIT})
    threads = out.get("threads", [])
    if len(threads) < INBOX_LIMIT:
        return None

    expected_archive = []
    expected_mark_read = []
    expected_leave = []
    for t in threads:
        cat = t["topic_category"]
        if cat == "Promotional":
            expected_archive.append(t["thread_id"])
        elif cat == "Transactional":
            expected_mark_read.append(t["thread_id"])
        else:
            expected_leave.append(t["thread_id"])

    # Need at least one of each branch for the scenario to test the
    # full rule. If the procedural distribution didn't give us one
    # of each, the run is uninformative.
    if not expected_archive or not expected_mark_read or not expected_leave:
        return None

    return {
        "viewer": VIEWER,
        "inbox_size": len(threads),
        "expected_archive": expected_archive,
        "expected_mark_read": expected_mark_read,
        "expected_leave": expected_leave,
        "all_thread_ids": [t["thread_id"] for t in threads],
    }


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools to triage "
        "your inbox according to the rules I'm about to give you. Be "
        "exact — only act when the rule says to."
    )


def user_task(s):
    return (
        f"Look at my latest {s['inbox_size']} inbox threads (use "
        f"get_inbox with limit={INBOX_LIMIT}). For each thread, apply "
        "EXACTLY this "
        "rule based on its topic_category:\n"
        "\n"
        "  - if topic_category is 'Promotional' → call archive_thread\n"
        "  - if topic_category is 'Transactional' → call mark_read on "
        "the thread's first message_id (use read_thread to get it)\n"
        "  - any other category → DO NOTHING. No archive, no "
        "mark_read, no reply, no rsvp. Skip the thread entirely.\n"
        "\n"
        "Be precise: only mutate threads matching one of the first two "
        "rules. Do not touch threads in any other category. When done, "
        "tell me how many you archived, how many you marked read, and "
        "how many you left alone. Do not send emails, DMs, or replies."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)
        if scenario is None:
            print("[setup] inbox lacks one of each category bucket",
                  file=sys.stderr)
            return 2

        print(f"[setup] viewer={scenario['viewer']} "
              f"archive={len(scenario['expected_archive'])} "
              f"mark_read={len(scenario['expected_mark_read'])} "
              f"leave_alone={len(scenario['expected_leave'])}",
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

    archived = trace.get("archived_threads", [])
    marked_read_msg_ids = trace.get("marked_read", [])

    # mark_read is keyed on message_id, not thread_id — the agent calls
    # read_thread to get the message_id for each Work thread, then
    # mark_read on it. To verify, we need to map back from message_ids
    # to thread_ids. The simplest check: for each Work thread the agent
    # was supposed to mark, its first message_id should appear in
    # marked_read. The harness's get_inbox cached the thread_id list;
    # we can re-derive expected message_ids by re-calling read_thread
    # and snapshotting the first message_id per Work thread, but that
    # requires another MCP session. Easier: count cardinality + check
    # over-action via tool-call inspection.

    # Inspect tool calls to verify the agent only called archive_thread
    # on the right thread_ids and only called mark_read on messages
    # belonging to Work threads.
    archive_calls = [tc for tc in run.tool_calls if tc.name == "archive_thread"]
    archived_thread_ids = [tc.arguments.get("thread_id") for tc in archive_calls]

    failures: list[str] = []

    # Archive must hit exactly the Promotional set.
    expected_archive_set = set(scenario["expected_archive"])
    actual_archive_set = set(archived_thread_ids)
    missing = expected_archive_set - actual_archive_set
    extra = actual_archive_set - expected_archive_set
    if missing:
        failures.append(f"failed to archive Promotional threads: {sorted(missing)}")
    if extra:
        failures.append(f"archived non-Promotional threads (over-action): {sorted(extra)}")

    # mark_read count: we expect exactly the Work-set size. Lower
    # means missed; higher means over-action. We can't easily map
    # message_id → thread_id without another fetch, but cardinality
    # + the leave-alone-set must-not are tight constraints already.
    expected_mr_count = len(scenario["expected_mark_read"])
    if len(marked_read_msg_ids) < expected_mr_count:
        failures.append(
            f"marked_read has {len(marked_read_msg_ids)} entries; "
            f"expected ≥ {expected_mr_count} (one per Work thread)"
        )
    if len(marked_read_msg_ids) > expected_mr_count + 1:
        # Allow +1 for the case where read_thread itself marks one of
        # the inspected procedural messages as a side-effect during
        # the agent's exploration. More than that is over-action.
        failures.append(
            f"marked_read has {len(marked_read_msg_ids)} entries; "
            f"expected ≤ {expected_mr_count} (one per Work thread, "
            "modulo a single exploratory read)"
        )

    # No-action branch: leave-alone threads must not appear in
    # archived_threads. (We can't perfectly check mark_read because
    # we don't have the message_id mapping, but archive is exact.)
    leave_set = set(scenario["expected_leave"])
    leave_violations = leave_set & actual_archive_set
    if leave_violations:
        failures.append(
            f"archived threads in the leave-alone set: {sorted(leave_violations)}"
        )

    print()
    print("[constraints]")
    if not failures:
        print("  ✓ all constraints satisfied")
    else:
        for f in failures:
            print(f"  ✗ {f}")

    verdict = Verdict(
        # Archive set must be exactly the Promotional thread_ids.
        overlay_must=[
            OverlayAssertion(
                field="archived_threads",
                must_contain=scenario["expected_archive"],
                expected_count=len(scenario["expected_archive"]),
            ),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
            ToolCallMatch(name="rsvp"),
        ],
    )
    scorecard = evaluate(verdict, run, trace, max_turns=30)
    if failures:
        scorecard.correctness = False
    return report(run, scorecard)


if __name__ == "__main__":
    sys.exit(runner(main()))
