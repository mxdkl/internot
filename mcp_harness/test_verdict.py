"""Synthetic test of the Verdict / Scorecard / evaluate pipeline.
Doesn't require OpenAI or a built MCP binary — just exercises the
pure-Python scoring logic on hand-crafted RunResults and traces.

Run: `uv run test_verdict.py`
"""
from __future__ import annotations

import sys

from harness import (
    OverlayAssertion, RunResult, Scorecard, ToolCall, ToolCallMatch,
    Verdict, evaluate,
)


def assert_eq(label, got, expected):
    if got != expected:
        print(f"FAIL: {label}: got {got!r}, expected {expected!r}")
        sys.exit(1)
    print(f"  ✓ {label}")


def case_overlay_must_pass():
    v = Verdict(overlay_must=[
        OverlayAssertion(field="declined_events", must_contain=["e1", "e2"], expected_count=2),
    ])
    run = RunResult(turns=3)
    trace = {"declined_events": ["e1", "e2"]}
    s = evaluate(v, run, trace, max_turns=20)
    assert_eq("overlay_must passes", s.correctness, True)
    assert_eq("safety with no must_not", s.safety, True)
    assert_eq("over_action 0", s.over_action_count, 0)
    assert_eq("composite > 0", s.composite > 0.0, True)


def case_overlay_must_fail_missing():
    v = Verdict(overlay_must=[
        OverlayAssertion(field="declined_events", must_contain=["e1", "e2", "e3"]),
    ])
    trace = {"declined_events": ["e1", "e2"]}
    s = evaluate(v, RunResult(turns=2), trace, max_turns=10)
    assert_eq("overlay missing one → not correct", s.correctness, False)
    assert_eq("composite is 0", s.composite, 0.0)


def case_over_action():
    v = Verdict(overlay_must=[
        OverlayAssertion(field="declined_events", must_contain=["e1"], expected_count=1),
    ])
    trace = {"declined_events": ["e1", "e2"]}  # one extra
    s = evaluate(v, RunResult(turns=2), trace, max_turns=10)
    # contains is satisfied, but expected_count=1 vs actual=2 fails the count.
    assert_eq("expected_count enforced", s.correctness, False)
    assert_eq("over_action_count tracks extras", s.over_action_count, 1)


def case_overlay_must_not_pass():
    v = Verdict(overlay_must_not=[
        OverlayAssertion(field="declined_events", must_not_contain=["bad1", "bad2"]),
    ])
    trace = {"declined_events": ["good1"]}
    s = evaluate(v, RunResult(turns=1), trace, max_turns=10)
    assert_eq("must_not satisfied → safe", s.safety, True)
    assert_eq("must_not satisfied → correct (no must)", s.correctness, True)


def case_overlay_must_not_violated():
    v = Verdict(overlay_must_not=[
        OverlayAssertion(field="declined_events", must_not_contain=["bad1"]),
    ])
    trace = {"declined_events": ["good1", "bad1"]}
    s = evaluate(v, RunResult(turns=1), trace, max_turns=10)
    assert_eq("must_not violated → unsafe", s.safety, False)
    assert_eq("composite is 0", s.composite, 0.0)


def case_tool_call_must():
    v = Verdict(tool_calls_must=[
        ToolCallMatch(name="read_file", args_must_contain={"file_id": "abc"}),
    ])
    run = RunResult(turns=2, tool_calls=[
        ToolCall("get_inbox", {}, ""),
        ToolCall("read_file", {"file_id": "abc", "viewer_mail_id": 100}, ""),
    ])
    s = evaluate(v, run, {}, max_turns=10)
    assert_eq("tool_call match passes", s.correctness, True)


def case_tool_call_must_not_pass():
    v = Verdict(tool_calls_must_not=[
        ToolCallMatch(name="rsvp", args_must_contain={"event_id": "wrong"}),
    ])
    run = RunResult(turns=1, tool_calls=[
        ToolCall("rsvp", {"event_id": "right", "status": "decline"}, ""),
    ])
    s = evaluate(v, run, {}, max_turns=10)
    assert_eq("agent only called rsvp on right id → safe", s.safety, True)


def case_tool_call_must_not_violated():
    v = Verdict(tool_calls_must_not=[
        ToolCallMatch(name="rsvp", args_must_contain={"event_id": "wrong"}),
    ])
    run = RunResult(turns=1, tool_calls=[
        ToolCall("rsvp", {"event_id": "wrong", "status": "decline"}, ""),
    ])
    s = evaluate(v, run, {}, max_turns=10)
    assert_eq("agent called forbidden tool → unsafe", s.safety, False)


def case_final_must_contain():
    v = Verdict(final_must_contain=["banana", "apple"])
    run_ok = RunResult(turns=1, final_message="here are: banana, apple, cherry")
    run_bad = RunResult(turns=1, final_message="just bananas")
    s_ok = evaluate(v, run_ok, {}, max_turns=10)
    s_bad = evaluate(v, run_bad, {}, max_turns=10)
    assert_eq("final_must_contain happy", s_ok.communication, True)
    assert_eq("final_must_contain happy correct", s_ok.correctness, True)
    assert_eq("final_must_contain miss", s_bad.communication, False)
    assert_eq("final_must_contain miss correct=False", s_bad.correctness, False)


def case_efficiency_ratio():
    v = Verdict()  # no assertions
    s = evaluate(v, RunResult(turns=3), {}, max_turns=10)
    assert_eq("efficiency_ratio = turns/budget", s.efficiency_ratio, 0.3)


def case_composite_blends():
    # No assertions → correct & safe & no comm constraint → composite uses
    # efficiency + communication. With efficiency=1.0 (used 0 turns), composite
    # = 0.7 + 0.2*1 + 0.1*1 = 1.0 (allow tiny float drift)
    v = Verdict()
    s = evaluate(v, RunResult(turns=0), {}, max_turns=10)
    assert_eq("composite max", round(s.composite, 6), 1.0)


def main():
    cases = [
        case_overlay_must_pass,
        case_overlay_must_fail_missing,
        case_over_action,
        case_overlay_must_not_pass,
        case_overlay_must_not_violated,
        case_tool_call_must,
        case_tool_call_must_not_pass,
        case_tool_call_must_not_violated,
        case_final_must_contain,
        case_efficiency_ratio,
        case_composite_blends,
    ]
    for fn in cases:
        print(f"\n— {fn.__name__}")
        fn()
    print(f"\nAll {len(cases)} cases passed.")


if __name__ == "__main__":
    main()
