"""Scenario: upload_three_files (MEDIUM; gpt-5.4-mini 3/3 PASS).

Tested 2026-04-28 against gpt-5.4-mini, n=3 trials, all PASS at
composite 0.97. The model maps parameters per-call correctly across
3 fan-out uploads.

Together with `task_create_and_complete_specific` (3/3 pass), this
sharpens the failure-mode characterization: multi-mutation fan-out
ALONE doesn't break gpt-5.4-mini. The failures we see in calendar
fan-out scenarios are the conjunction of FAN-OUT × CONSTRAINT
SATISFACTION. The model can do per-item parameter mapping reliably
across N similar calls (proven on tasks and now files); it's the
joint search across N items' constraints that breaks.

Useful baseline closing the files-mutation coverage gap; not a SOTA
failure surface.


Hypothesis: when the user gives N items each with several distinct
attributes (name + kind + collection), the agent might preserve some
attributes correctly while permuting others — essentially treating
the per-item attribute sets as fungible rather than item-specific.
We've seen task-side multi-mutation state tracking work cleanly
(`task_create_and_complete_specific`); this tests the same pattern
on the files surface with richer per-call parameters.

Concretely: agent must upload 3 files with these exact (name, kind,
collection, size) tuples:
  - 'tax-return.pdf'    (Pdf, Personal, ~120000 bytes)
  - 'Q3-budget.xlsx'    (Spreadsheet, Work, ~50000 bytes)
  - 'vacation.jpg'      (Image, Personal, ~3500000 bytes)

Failure modes:
  1. Wrong number of uploads.
  2. A file uploaded with mismatched (name → kind) — e.g.,
     'vacation.jpg' uploaded as Document.
  3. A file uploaded into the wrong collection.

Substrate scope: files only. Tests fan-out in a non-calendar context;
also closes a coverage gap (files mutations have no scenario today).
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

EXPECTED_FILES = [
    {"name": "tax-return.pdf", "kind": "Pdf", "collection": "Personal", "size_bytes": 120000},
    {"name": "Q3-budget.xlsx", "kind": "Spreadsheet", "collection": "Work", "size_bytes": 50000},
    {"name": "vacation.jpg", "kind": "Image", "collection": "Personal", "size_bytes": 3500000},
]


async def setup(session):
    return {"viewer": VIEWER, "files": EXPECTED_FILES}


def system_prompt(s):
    return (
        f"You are mail_id {s['viewer']}. Use the MCP tools. Each "
        "file's attributes are item-specific — don't mix them up."
    )


def user_task(s):
    lines = []
    for f in s["files"]:
        lines.append(
            f"  - name='{f['name']}', kind={f['kind']}, "
            f"collection={f['collection']}, size_bytes={f['size_bytes']}"
        )
    return (
        f"Upload these {len(s['files'])} files to my drive, one "
        "upload_file call per file. Each file's parameters are "
        "specific to it — preserve them exactly:\n"
        "\n"
        + "\n".join(lines) + "\n"
        "\n"
        "Do not send emails, DMs, or replies. Tell me the file_ids "
        "you got back."
    )


async def main() -> int:
    async with mcp_session() as session:
        scenario = await setup(session)

        loop = LoopRunner(session)
        run = await loop.run(
            system_prompt(scenario),
            user_task(scenario),
            max_turns=15,
        )
        trace = await call_tool(session, "_get_trace", {})
        if not isinstance(trace, dict):
            trace = {}

    upload_calls = [tc for tc in run.tool_calls if tc.name == "upload_file"]

    failures: list[str] = []

    if len(upload_calls) != len(EXPECTED_FILES):
        failures.append(
            f"expected {len(EXPECTED_FILES)} upload_file calls, got "
            f"{len(upload_calls)}"
        )

    # Match by name (strict) and check that each uploaded file's
    # attributes match the expected tuple for that name.
    by_name = {tc.arguments.get("name"): tc.arguments for tc in upload_calls}
    for f in EXPECTED_FILES:
        args = by_name.get(f["name"])
        if args is None:
            failures.append(f"missing upload for name={f['name']!r}")
            continue
        if args.get("kind") != f["kind"]:
            failures.append(
                f"{f['name']!r}: kind={args.get('kind')!r} != "
                f"expected {f['kind']!r} (parameter permuted across files)"
            )
        if args.get("collection") != f["collection"]:
            failures.append(
                f"{f['name']!r}: collection={args.get('collection')!r} != "
                f"expected {f['collection']!r}"
            )
        if args.get("size_bytes") != f["size_bytes"]:
            failures.append(
                f"{f['name']!r}: size_bytes={args.get('size_bytes')!r} != "
                f"expected {f['size_bytes']!r}"
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
            OverlayAssertion(field="uploaded_files",
                             expected_count=len(EXPECTED_FILES)),
        ],
        tool_calls_must_not=[
            ToolCallMatch(name="compose_email"),
            ToolCallMatch(name="reply"),
            ToolCallMatch(name="send_dm"),
            ToolCallMatch(name="rsvp"),
        ],
    )
    scorecard = evaluate(verdict, run, trace, max_turns=15)
    if failures:
        scorecard.correctness = False
    return report(run, scorecard)


if __name__ == "__main__":
    sys.exit(runner(main()))
