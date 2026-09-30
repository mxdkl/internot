"""Shared verdict helpers for scenario files.

Calendar-heavy scenarios all need to parse ISO timestamps from
get_busy/get_schedule responses, check for slot conflicts, and
construct day-anchored datetimes for window checks. Before this
module these were re-implemented as `_parse`/`_free`/`_day_anchor`/
`_time_at`/`_has_buffer` in every scenario file. Extracted here so
new scenarios can import the canonical versions.

Naming convention: short, no underscore prefix (these ARE the public
helpers, no longer per-file privates).

Time semantics: all returned datetimes are NAIVE UTC (tzinfo=None),
matching the convention the scenarios used pre-extraction. The
substrate sends ISO strings with 'Z' suffix; we parse and strip
tzinfo for arithmetic compatibility.
"""
from __future__ import annotations

from datetime import datetime, timedelta
from typing import Iterable, Mapping


# Procedural epoch — day_offset 0 maps to this UTC midnight. Mirrors
# the Rust-side `epoch()` constant in internot's calendar derivation.
EPOCH = datetime(2025, 1, 1, 0, 0, 0)


def parse_iso(iso: str) -> datetime:
    """Parse an ISO-8601 timestamp like '2025-07-01T13:00:00Z' into a
    naive UTC datetime (tzinfo stripped)."""
    return datetime.fromisoformat(iso.replace("Z", "+00:00")).replace(tzinfo=None)


def day_anchor(day_offset: int) -> datetime:
    """UTC midnight at `day_offset` days after EPOCH."""
    return EPOCH + timedelta(days=day_offset)


def time_at(day_offset: int, hour: int, minute: int = 0) -> datetime:
    """Specific time on a given day, e.g. 13:30 on day 181."""
    return day_anchor(day_offset) + timedelta(hours=hour, minutes=minute)


def is_free(
    busy_intervals: Iterable[Mapping[str, str]],
    start: datetime,
    end: datetime,
) -> bool:
    """True iff [start, end) overlaps no interval in `busy_intervals`.

    Each entry must have 'starts_at' and 'ends_at' as ISO strings.
    Half-open semantics: an event ending exactly at `start` does NOT
    block, and an event starting exactly at `end` does NOT block.
    """
    for b in busy_intervals:
        bs = parse_iso(b["starts_at"])
        be = parse_iso(b["ends_at"])
        if bs < end and start < be:
            return False
    return True


def has_buffer(
    busy_intervals: Iterable[Mapping[str, str]],
    start: datetime,
    end: datetime,
    buffer_minutes: int,
) -> bool:
    """True iff no event in `busy_intervals` ends within `buffer_minutes`
    before `start` OR starts within `buffer_minutes` after `end`.

    Used by `book_with_buffer`-style scenarios that require a gap
    around a new booking. Must be combined with `is_free` for full
    safety (this checks adjacency, not overlap)."""
    pre = start - timedelta(minutes=buffer_minutes)
    post = end + timedelta(minutes=buffer_minutes)
    for b in busy_intervals:
        bs = parse_iso(b["starts_at"])
        be = parse_iso(b["ends_at"])
        # Event ends inside [pre, start) — too close before us.
        if pre <= be < start:
            return False
        # Event starts inside (end, post] — too close after us.
        if end < bs <= post:
            return False
    return True
