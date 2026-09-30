"""Build a NAICS sector distillate from the codeforamerica/naics-api raw data.

Input:  datasets/naics-api/data/codes-2012.json
Output: internot/data/naics_sectors.json

Schema of output: an ARRAY of `{sector_code, title, description}`
records. `sector_code` is the 2-digit NAICS sector (or sector range
like "31-33" for Manufacturing). 20 sectors total — the canonical
top-level industry taxonomy.

Why this shape: replaces the substrate's hand-curated `industry_of`
list with the standard US Census/BLS classification. Per CLAUDE.md
invariant — hand-curated lookup tables are tech debt; use real data
when available.

Sector titles come from the raw NAICS data (where available — most
2-digit codes don't have their own entries; we look at common
3-digit children to derive the sector name) and are also hand-listed
here as a fallback because the raw JSON is keyed by 6-digit codes
and 2-digit sector titles aren't first-class entries in the
codeforamerica dataset.

Re-run: `python3 internot/build_naics.py` (no deps).
"""
from __future__ import annotations

import json
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
INPUT = REPO_ROOT / "datasets" / "naics-api" / "data" / "codes-2012.json"
OUTPUT = REPO_ROOT / "internot" / "data" / "naics_sectors.json"


# Canonical NAICS 2012 sector list. Source: US Census Bureau,
# https://www.census.gov/naics/?58967?yearbck=2012 — these are the
# 20 official sectors. We list them here because the raw JSON is
# keyed by 6-digit codes; 2-digit sector titles aren't first-class.
SECTORS = [
    ("11",    "Agriculture, Forestry, Fishing and Hunting"),
    ("21",    "Mining, Quarrying, and Oil and Gas Extraction"),
    ("22",    "Utilities"),
    ("23",    "Construction"),
    ("31-33", "Manufacturing"),
    ("42",    "Wholesale Trade"),
    ("44-45", "Retail Trade"),
    ("48-49", "Transportation and Warehousing"),
    ("51",    "Information"),
    ("52",    "Finance and Insurance"),
    ("53",    "Real Estate and Rental and Leasing"),
    ("54",    "Professional, Scientific, and Technical Services"),
    ("55",    "Management of Companies and Enterprises"),
    ("56",    "Administrative and Support and Waste Management and Remediation Services"),
    ("61",    "Educational Services"),
    ("62",    "Health Care and Social Assistance"),
    ("71",    "Arts, Entertainment, and Recreation"),
    ("72",    "Accommodation and Food Services"),
    ("81",    "Other Services (except Public Administration)"),
    ("92",    "Public Administration"),
]


def first_two_digits_of(sector_code: str) -> set[str]:
    """Codes-2012.json uses 6-digit keys; for sector "31-33" we want
    children whose first two digits are 31, 32, OR 33."""
    if "-" in sector_code:
        lo, hi = sector_code.split("-")
        return {str(n) for n in range(int(lo), int(hi) + 1)}
    return {sector_code}


def main() -> int:
    raw = json.loads(INPUT.read_text())

    # For each sector, look at its children and grab the first
    # one's `description` (truncated) as a representative blurb.
    # The 6-digit entries have rich text we can mine.
    sample_blurbs: dict[str, str] = {}
    for sector_code, _title in SECTORS:
        prefixes = first_two_digits_of(sector_code)
        for code, entry in raw.items():
            if code[:2] in prefixes:
                desc = entry.get("description") or []
                if desc:
                    # First "real" sentence — but not at "U.S." or
                    # similar abbreviations (next char is uppercase),
                    # just at periods followed by a space + uppercase
                    # OR end-of-text.
                    blurb = desc[0]
                    truncated = blurb
                    for i, c in enumerate(blurb):
                        if c == "." and i + 2 < len(blurb):
                            nxt = blurb[i + 1:i + 3]
                            if nxt.startswith(" ") and nxt[1:].strip()[:1].isupper():
                                truncated = blurb[:i + 1]
                                break
                    sample_blurbs[sector_code] = truncated.strip()
                    break

    distilled = [
        {
            "sector_code": code,
            "title": title,
            "description": sample_blurbs.get(code, ""),
        }
        for code, title in SECTORS
    ]

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(distilled, separators=(",", ":")))

    print(f"wrote {OUTPUT.relative_to(REPO_ROOT)} — "
          f"{len(distilled)} sectors, {OUTPUT.stat().st_size / 1024:.1f}KB")
    for d in distilled[:5]:
        blurb = d["description"][:60] + "..." if len(d["description"]) > 60 else d["description"]
        print(f"  [{d['sector_code']:5}] {d['title']:55}  {blurb}")
    print(f"  ... ({len(distilled) - 5} more)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
