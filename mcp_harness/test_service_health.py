"""Hit every service's read views via MCP and verify they return
plausible data. No OpenAI required.
"""
from __future__ import annotations

import asyncio
import sys

from harness import call_tool, mcp_session


async def main() -> int:
    async with mcp_session() as session:
        # Pick a viewer with rich state — we know 100 has a populated
        # workplace, inbox, and DMs from earlier smoke tests.
        viewer = 100

        checks = [
            ("read_person",            {"mail_id": viewer}),
            ("get_inbox",              {"viewer_mail_id": viewer}),
            ("get_schedule",           {"viewer_mail_id": viewer, "day_offset": 180}),
            ("list_drive",             {"viewer_mail_id": viewer, "limit": 5}),
            ("list_tasks",             {"viewer_mail_id": viewer, "limit": 5}),
            ("list_accounts",          {"viewer_mail_id": viewer}),
            ("list_transactions",      {"viewer_mail_id": viewer, "limit": 5}),
            ("list_dms",               {"viewer_mail_id": viewer, "limit": 5}),
        ]

        all_ok = True
        for name, args in checks:
            out = await call_tool(session, name, args)
            ok = isinstance(out, dict) and not out.get("error")
            non_empty = False
            if ok:
                # Check the named primary collection is non-empty.
                key_map = {
                    "read_person": "name_first",
                    "get_inbox": "threads",
                    "get_schedule": "events",
                    "list_drive": "files",
                    "list_tasks": "tasks",
                    "list_accounts": "accounts",
                    "list_transactions": "transactions",
                    "list_dms": "dms",
                }
                k = key_map.get(name, "")
                v = out.get(k) if k else None
                non_empty = (
                    v is not None and (
                        (isinstance(v, list) and len(v) > 0) or isinstance(v, str)
                    )
                )
            status = "✓" if (ok and non_empty) else "✗"
            print(f"  {status} {name:<22} -> {'ok' if ok else 'ERR'}, "
                  f"{'non-empty' if non_empty else 'EMPTY'}")
            if not ok or not non_empty:
                all_ok = False

        print()
        return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
