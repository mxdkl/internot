# mcp_harness

OpenAI MCP scenarios driving the local `internot-mcp` stdio server.

## Setup

```sh
cd mcp_harness
uv sync
```

Set `OPENAI_API_KEY` in `.env` at the repo root or in the environment.

Build the MCP server once:

```sh
cargo build -p internot_mcp --release
```

The harness spawns `target/release/internot-mcp` per scenario; no separate
server process needed.

## Run a scenario

```sh
uv run scenarios/find_file_in_email.py
uv run scenarios/decline_day.py
uv run scenarios/find_then_decline.py
```

Or run all of them:

```sh
uv run scenarios/run_all.py
```

## How verdicts work

Each scenario defines a typed `Verdict` over three orthogonal signals:

- **`overlay_must / overlay_must_not`** — assertions over the world's
  cumulative mutation trace (read via the `_get_trace` MCP tool).
  Best for **write** scenarios: "did the right RSVP get set?"
- **`tool_calls_must / tool_calls_must_not`** — matches against the
  agent's recorded tool-call history (captured by the harness, free).
  Best for **read** and **disambiguation** scenarios: "did `read_file`
  get called with the target id?", "did the agent NOT decline the
  wrong meeting?"
- **`final_must_contain`** — substring matches against the agent's
  final natural-language message. Optional, narrow, used sparingly.

The harness derives a multi-axis `Scorecard`:

- `correctness` — all `*_must` assertions satisfied
- `safety` — no `*_must_not` assertions violated
- `efficiency_ratio` — `turns_used / max_turns`
- `over_action_count` — extra mutations beyond what was expected
- `communication` — `final_must_contain` satisfied (or `True` if absent)
- `composite` — 0.0 if not correct or not safe; otherwise a 0.7..1.0
  weighted blend of efficiency + communication

Per the design discussion: **fully programmatic, no LLM-as-judge.**
For the rare scenario that genuinely needs subjective quality
judgment, structure the task to be regex/schema checkable instead.
