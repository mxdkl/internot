"""Verify each scenario's setup phase finds a candidate. No OpenAI
required — just exercises the MCP plumbing + the world's procedural
state to make sure scenarios won't all return 'no candidate found'.

Run: `uv run test_setup_paths.py`
"""
from __future__ import annotations

import asyncio
import sys

# Make scenarios/ importable as scenario_*.
sys.path.insert(0, "scenarios")

from harness import mcp_session


async def run_setups():
    from scenarios import find_file_in_email, decline_day, find_then_decline

    async with mcp_session() as session:
        for mod, name in [
            (find_file_in_email, "find_file_in_email"),
            (decline_day, "decline_day"),
            (find_then_decline, "find_then_decline"),
        ]:
            scenario = await mod.setup(session)
            if scenario is None:
                print(f"  ✗ {name}: no candidate found")
                return 1
            print(f"  ✓ {name}: {list(scenario.keys())}")
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(run_setups()))
