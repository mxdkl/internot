"""Shared scaffolding for OpenAI MCP scenarios.

Every scenario file imports from here and writes ~50-100 lines of
domain-specific setup + prompt + Verdict. The harness handles:

- spawning internot-mcp as a stdio subprocess
- listing MCP tools and converting them to OpenAI function schemas
- running the OpenAI tool-call loop, capturing every tool call into
  a `RunResult.tool_calls` list (free signal — no extra round trip)
- evaluating a typed `Verdict` against the run's overlay trace +
  tool history + final message
- producing a multi-axis `Scorecard` (no LLM-as-judge anywhere)

Programmatic only. The verdict DSL is intentionally narrow — overlay
state, tool calls, and final-message substrings cover ~all our
current scenario shapes.
"""
from __future__ import annotations

import asyncio
import json
import os
import sys
from contextlib import asynccontextmanager
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, AsyncIterator, Optional

from dotenv import load_dotenv
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client
from openai import OpenAI

REPO_ROOT = Path(__file__).resolve().parent.parent
MCP_BINARY = REPO_ROOT / "target" / "release" / "internot-mcp"


# ============================================================================
# MCP client
# ============================================================================

def _load_api_key() -> str:
    load_dotenv(REPO_ROOT / ".env")
    key = os.environ.get("OPENAI_API_KEY")
    if not key:
        sys.exit("missing OPENAI_API_KEY in env or repo .env")
    return key


@asynccontextmanager
async def mcp_session() -> AsyncIterator[ClientSession]:
    """Spawn internot-mcp and yield an initialized MCP client."""
    if not MCP_BINARY.exists():
        sys.exit(
            f"MCP binary not built. Run:\n"
            f"  cargo build -p internot_mcp --release"
        )
    # env=None in StdioServerParameters means "pass no env vars" — NOT
    # "inherit from parent". The internot-mcp binary needs to see at
    # least PATH (for any subprocess work) and OPENAI_API_KEY (for
    # the opt-in `llm_render: true` rendering path on read tools).
    # Pass the harness's full os.environ.
    params = StdioServerParameters(
        command=str(MCP_BINARY), args=[], env=dict(os.environ)
    )
    async with stdio_client(params) as (read_stream, write_stream):
        async with ClientSession(read_stream, write_stream) as session:
            await session.initialize()
            yield session


async def call_tool(
    session: ClientSession,
    name: str,
    arguments: dict[str, Any],
) -> Any:
    """Call an MCP tool; return the parsed JSON payload (or raw text)."""
    result = await session.call_tool(name, arguments)
    text_parts = [
        c.text for c in result.content if getattr(c, "type", None) == "text"
    ]
    text = "\n".join(text_parts)
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return text


# ============================================================================
# Tool-call trace + run result
# ============================================================================

@dataclass
class ToolCall:
    name: str
    arguments: dict[str, Any]
    result_text: str


@dataclass
class RunResult:
    tool_calls: list[ToolCall] = field(default_factory=list)
    final_message: Optional[str] = None
    turns: int = 0


def _mcp_tools_to_openai(mcp_tools: list[Any]) -> list[dict[str, Any]]:
    """Convert MCP `Tool` list → OpenAI Chat Completions tool schemas."""
    out: list[dict[str, Any]] = []
    for t in mcp_tools:
        out.append({
            "type": "function",
            "function": {
                "name": t.name,
                "description": t.description or "",
                "parameters": t.inputSchema,
            },
        })
    return out


class LoopRunner:
    """Run an OpenAI tool-call loop using the MCP server's tool surface."""

    def __init__(
        self,
        session: ClientSession,
        model: str = "gpt-5.4-mini",
        skip_tools: Optional[set[str]] = None,
        verbose: bool = False,
    ):
        self.session = session
        # Per-run model override via env var. Lets cross-model
        # experiments swap the model without editing every scenario.
        self.model = os.environ.get("MCP_HARNESS_MODEL", model)
        # Verdict-only tools (e.g. "_get_trace") are hidden from the agent;
        # the harness reads them itself after the run.
        self.skip_tools = skip_tools or {"_get_trace"}
        # When True, prints each tool's result text after every call —
        # useful for walking through a scenario by hand.
        self.verbose = verbose or os.environ.get("MCP_HARNESS_VERBOSE") == "1"
        # Load .env before constructing the OpenAI client — it
        # validates OPENAI_API_KEY at construction time.
        _load_api_key()
        self.client = OpenAI()

    async def run(
        self,
        system_prompt: str,
        user_task: str,
        max_turns: int = 12,
    ) -> RunResult:
        mcp_tools = (await self.session.list_tools()).tools
        openai_tools = _mcp_tools_to_openai(
            [t for t in mcp_tools if t.name not in self.skip_tools]
        )

        messages: list[dict[str, Any]] = [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_task},
        ]
        result = RunResult()
        for turn in range(max_turns):
            result.turns = turn + 1
            resp = self.client.chat.completions.create(
                model=self.model,
                messages=messages,
                tools=openai_tools,
                tool_choice="auto",
            )
            msg = resp.choices[0].message
            messages.append(msg.model_dump(exclude_none=True))

            if not msg.tool_calls:
                result.final_message = msg.content
                break

            for call in msg.tool_calls:
                name = call.function.name
                try:
                    args = json.loads(call.function.arguments or "{}")
                except json.JSONDecodeError:
                    args = {}
                print(
                    f"[turn {turn + 1}] → {name}({json.dumps(args)})",
                    file=sys.stderr,
                )
                tool_result = await self.session.call_tool(name, args)
                text_parts = [
                    c.text for c in tool_result.content
                    if getattr(c, "type", None) == "text"
                ]
                result_text = "\n".join(text_parts)
                if self.verbose:
                    # Pretty-print JSON results; truncate to keep
                    # walkthroughs legible.
                    pretty = result_text
                    try:
                        pretty = json.dumps(json.loads(result_text), indent=2)
                    except json.JSONDecodeError:
                        pass
                    snippet = pretty if len(pretty) <= 1500 else pretty[:1500] + "\n  …(truncated)"
                    indented = "\n".join(f"      {ln}" for ln in snippet.splitlines())
                    print(f"[turn {turn + 1}] ← result:\n{indented}", file=sys.stderr)
                result.tool_calls.append(
                    ToolCall(name=name, arguments=args, result_text=result_text)
                )
                messages.append({
                    "role": "tool",
                    "tool_call_id": call.id,
                    "content": result_text,
                })
        return result


# ============================================================================
# Verdict DSL
# ============================================================================

@dataclass
class OverlayAssertion:
    """Match against the world's cumulative mutation trace.

    `field` names a vec-of-strings field on `MutationTrace`
    (composed_messages, replied_threads, declined_events,
    accepted_events, tentative_events, booked_events, marked_read,
    sent_dms).
    """
    field: str
    must_contain: list[str] = field(default_factory=list)
    must_not_contain: list[str] = field(default_factory=list)
    expected_count: Optional[int] = None
    min_count: Optional[int] = None
    max_count: Optional[int] = None


@dataclass
class ToolCallMatch:
    """Match against the agent's tool-call history."""
    name: str
    args_must_contain: dict[str, Any] = field(default_factory=dict)
    must_call_at_least: int = 1
    """How many calls to `name` (with matching args) the agent must make."""


@dataclass
class Verdict:
    """A scenario's expected behavior, expressed as orthogonal assertions.

    - overlay_must / overlay_must_not: world state checks (best for writes)
    - tool_calls_must / tool_calls_must_not: agent action checks
      (best for reads + disambiguation)
    - final_must_contain: substring matches in final message (use sparingly)
    """
    overlay_must: list[OverlayAssertion] = field(default_factory=list)
    overlay_must_not: list[OverlayAssertion] = field(default_factory=list)
    tool_calls_must: list[ToolCallMatch] = field(default_factory=list)
    tool_calls_must_not: list[ToolCallMatch] = field(default_factory=list)
    final_must_contain: Optional[list[str]] = None


# ============================================================================
# Scoring
# ============================================================================

@dataclass
class AssertionResult:
    description: str
    passed: bool
    detail: str = ""


@dataclass
class Scorecard:
    correctness: bool
    safety: bool
    efficiency_ratio: float       # turns_used / budget; lower is better
    over_action_count: int        # extra mutations beyond expected
    communication: bool
    assertion_results: list[AssertionResult] = field(default_factory=list)

    @property
    def composite(self) -> float:
        if not self.correctness or not self.safety:
            return 0.0
        eff = max(0.0, 1.0 - min(1.0, self.efficiency_ratio))
        comm = 1.0 if self.communication else 0.0
        return 0.7 + 0.2 * eff + 0.1 * comm


def _matches_args(call_args: dict[str, Any], must: dict[str, Any]) -> bool:
    """call_args ⊇ must (every key/value in must must match call_args)."""
    for k, v in must.items():
        if k not in call_args or call_args[k] != v:
            return False
    return True


def _check_overlay(
    trace: dict[str, Any],
    a: OverlayAssertion,
    must_succeed: bool,
) -> AssertionResult:
    items = trace.get(a.field, [])
    if not isinstance(items, list):
        return AssertionResult(
            description=f"overlay {a.field}",
            passed=False,
            detail=f"trace field {a.field} is not a list",
        )
    fails: list[str] = []
    for v in a.must_contain:
        if v not in items:
            fails.append(f"missing {v!r}")
    for v in a.must_not_contain:
        if v in items:
            fails.append(f"unexpectedly contains {v!r}")
    if a.expected_count is not None and len(items) != a.expected_count:
        fails.append(f"len {len(items)} != expected {a.expected_count}")
    if a.min_count is not None and len(items) < a.min_count:
        fails.append(f"len {len(items)} < min {a.min_count}")
    if a.max_count is not None and len(items) > a.max_count:
        fails.append(f"len {len(items)} > max {a.max_count}")
    label = ("must" if must_succeed else "must_not") + f" overlay.{a.field}"
    if fails:
        return AssertionResult(label, False, "; ".join(fails))
    return AssertionResult(label, True)


def _check_tool_call(
    calls: list[ToolCall],
    m: ToolCallMatch,
    must_succeed: bool,
) -> AssertionResult:
    matches = [
        c for c in calls
        if c.name == m.name and _matches_args(c.arguments, m.args_must_contain)
    ]
    label = (
        ("must" if must_succeed else "must_not") +
        f" tool_call {m.name}({m.args_must_contain})"
    )
    if must_succeed:
        if len(matches) >= m.must_call_at_least:
            return AssertionResult(label, True, f"{len(matches)} call(s)")
        return AssertionResult(
            label, False,
            f"{len(matches)} call(s), need {m.must_call_at_least}",
        )
    else:
        if not matches:
            return AssertionResult(label, True)
        return AssertionResult(
            label, False,
            f"forbidden tool called {len(matches)} time(s)",
        )


def evaluate(
    verdict: Verdict,
    run: RunResult,
    trace: dict[str, Any],
    max_turns: int,
) -> Scorecard:
    results: list[AssertionResult] = []

    for a in verdict.overlay_must:
        results.append(_check_overlay(trace, a, must_succeed=True))
    for a in verdict.overlay_must_not:
        # Same checker; "must not contain" semantics live inside the
        # assertion (must_not_contain field).
        results.append(_check_overlay(trace, a, must_succeed=False))
    for m in verdict.tool_calls_must:
        results.append(_check_tool_call(run.tool_calls, m, must_succeed=True))
    for m in verdict.tool_calls_must_not:
        results.append(_check_tool_call(run.tool_calls, m, must_succeed=False))

    final_ok = True
    if verdict.final_must_contain is not None:
        msg = run.final_message or ""
        missing = [s for s in verdict.final_must_contain if s not in msg]
        if missing:
            final_ok = False
            results.append(AssertionResult(
                "final_must_contain", False, f"missing: {missing}"
            ))
        else:
            results.append(AssertionResult("final_must_contain", True))

    correctness_results = [
        r for r in results
        if r.description.startswith("must overlay")
        or r.description.startswith("must tool_call")
        or r.description == "final_must_contain"
    ]
    safety_results = [
        r for r in results
        if r.description.startswith("must_not ")
    ]
    correctness = all(r.passed for r in correctness_results) if correctness_results else True
    safety = all(r.passed for r in safety_results) if safety_results else True

    # Over-action: count of extra entries in any overlay_must field
    # whose expected_count was set.
    over = 0
    for a in verdict.overlay_must:
        if a.expected_count is not None:
            actual = len(trace.get(a.field, []))
            over += max(0, actual - a.expected_count)

    return Scorecard(
        correctness=correctness,
        safety=safety,
        efficiency_ratio=run.turns / max_turns if max_turns > 0 else 0.0,
        over_action_count=over,
        communication=final_ok,
        assertion_results=results,
    )


# ============================================================================
# Reporting
# ============================================================================

def report(run: RunResult, scorecard: Scorecard) -> int:
    print()
    print(f"[trace] {len(run.tool_calls)} tool call(s) over {run.turns} turn(s)")
    for tc in run.tool_calls:
        print(f"  {tc.name}({json.dumps(tc.arguments)})")
    if run.final_message:
        print()
        print(f"[final] {run.final_message}")
    print()
    print("[verdict]")
    for r in scorecard.assertion_results:
        mark = "✓" if r.passed else "✗"
        line = f"  {mark} {r.description}"
        if r.detail:
            line += f"  — {r.detail}"
        print(line)
    print()
    print("[scorecard]")
    print(f"  correctness        : {scorecard.correctness}")
    print(f"  safety             : {scorecard.safety}")
    print(f"  efficiency_ratio   : {scorecard.efficiency_ratio:.2f}")
    print(f"  over_action_count  : {scorecard.over_action_count}")
    print(f"  communication      : {scorecard.communication}")
    print(f"  composite          : {scorecard.composite:.2f}")
    print()
    overall = scorecard.correctness and scorecard.safety
    print(f"[result] {'PASS' if overall else 'FAIL'}")
    return 0 if overall else 1


# ============================================================================
# Convenience entry point for scenarios
# ============================================================================

async def verify_solver(
    *,
    setup_fn,
    solver_fn,
    verdict_fn,
) -> tuple[bool, str]:
    """Run the canonical actions, check the verdict accepts them.

    Use BEFORE the agent run to structurally confirm the scenario is
    solvable on the current substrate. Catches the bug class where a
    canonical answer is unreachable (e.g. book_meeting at XX:30 when
    the substrate only accepted hour-aligned starts).

    Returns (passed, reason). On False, the scenario itself is broken
    — either the substrate can't perform the canonical actions, or
    the verdict doesn't accept them. Either way, an agent run would
    be misleading.

    Contract: setup_fn must be deterministic and read-only/sampling
    (it'll be called again in the agent's fresh session, and both
    must yield the same scenario data). solver_fn performs the
    canonical mutations directly via call_tool(session, ...).
    """
    async with mcp_session() as session:
        scenario = await setup_fn(session)
        if scenario is None:
            return False, "setup returned None"
        try:
            await solver_fn(session, scenario)
        except Exception as e:
            return False, f"solver raised: {type(e).__name__}: {e}"
        trace = await call_tool(session, "_get_trace", {})
        if not isinstance(trace, dict):
            trace = {}
    verdict = verdict_fn(scenario)
    score = evaluate(verdict, RunResult(), trace, max_turns=1)
    if score.correctness:
        return True, ""
    failed = [r.description for r in score.assertion_results if not r.passed]
    return False, f"verdict not satisfied by canonical actions: {failed}"


async def run_with_verdict(
    *,
    setup_fn,
    system_prompt_fn,
    user_task_fn,
    verdict_fn,
    solver_fn=None,
    model: str = "gpt-5.4-mini",
    max_turns: int = 12,
    verbose: bool = False,
) -> int:
    """Standard scenario shape: setup → prompt → run → verdict → report.

    `setup_fn(session) -> dict`   : pick targets, return scenario dict
    `system_prompt_fn(scenario)`  : str
    `user_task_fn(scenario)`      : str
    `verdict_fn(scenario)`        : Verdict
    `solver_fn(session, scenario)`: optional. If provided, run the
        canonical actions in a fresh session BEFORE the agent and
        confirm the verdict accepts them. Bails (exit 3) if not — this
        is a scenario/substrate bug, not an agent bug, and an agent
        run would just produce a misleading FAIL.
    """
    if solver_fn is not None:
        ok, reason = await verify_solver(
            setup_fn=setup_fn,
            solver_fn=solver_fn,
            verdict_fn=verdict_fn,
        )
        if not ok:
            print(f"[solver] FAIL — {reason}", file=sys.stderr)
            print("[solver] scenario or substrate is broken; skipping "
                  "agent run", file=sys.stderr)
            return 3
        if verbose or os.environ.get("MCP_HARNESS_VERBOSE") == "1":
            print("[solver] OK — canonical actions satisfy verdict",
                  file=sys.stderr)

    async with mcp_session() as session:
        scenario = await setup_fn(session)
        if scenario is None:
            print("[setup] no candidate found", file=sys.stderr)
            return 2
        if verbose or os.environ.get("MCP_HARNESS_VERBOSE") == "1":
            print(f"[setup] scenario = {json.dumps(scenario, indent=2)}",
                  file=sys.stderr)

        loop = LoopRunner(session, model=model, verbose=verbose)
        run = await loop.run(
            system_prompt_fn(scenario),
            user_task_fn(scenario),
            max_turns=max_turns,
        )

        trace = await call_tool(session, "_get_trace", {})
        if not isinstance(trace, dict):
            trace = {}

    verdict = verdict_fn(scenario)
    scorecard = evaluate(verdict, run, trace, max_turns)
    return report(run, scorecard)


def runner(coro):
    """Convenience: scenarios end with `sys.exit(runner(main()))`."""
    return asyncio.run(coro)
