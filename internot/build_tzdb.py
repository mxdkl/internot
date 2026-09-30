"""Build a country → timezone distillate from raw tzdb data.

Input:  datasets/tzdb/raw-time-zones.json
Output: internot/data/country_timezones.json

Schema of output: a JSON OBJECT keyed by ISO country code (uppercase
2-letter), with values `{tz, offset_minutes, primary_city}`.

Why this shape: the substrate's `country_of(person_id)` already maps
each person to a country code. Adding `timezone_of(person)` as
`country → tz lookup` is a one-line derivation that gives every
person realistic working-hours coverage without needing per-city tz
data (which would require joining geonamescache against IANA zone
boundaries, a heavier dataset).

Country-with-multiple-timezones policy: when a country spans several
zones (US, RU, BR, AU, CA, ...), pick the zone with the LARGEST
mainCities list as the canonical "primary" zone. Imperfect — a
person in Anchorage will get reported as Eastern, not Alaska — but
realistic for the most common cases. Per-city tz can come later via
boundary data if a scenario demands it.

Re-run: `python3 internot/build_tzdb.py` (no deps).
"""
from __future__ import annotations

import json
from collections import defaultdict
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
INPUT = REPO_ROOT / "datasets" / "tzdb" / "raw-time-zones.json"
OUTPUT = REPO_ROOT / "internot" / "data" / "country_timezones.json"


# Countries spanning multiple IANA zones — picked by population
# weight (most populous timezone) since the raw data has no
# population signal and the alphabetic tiebreaker grabs random
# fringe zones (e.g. Pacific/Honolulu for US, Europe/Samara for RU).
# Stick to the tz string; the rest is computed from the matching
# raw zone's metadata.
MULTI_TZ_OVERRIDES = {
    "US": "America/New_York",
    "CA": "America/Toronto",
    "RU": "Europe/Moscow",
    "BR": "America/Sao_Paulo",
    "AU": "Australia/Sydney",
    "CN": "Asia/Shanghai",
    "MX": "America/Mexico_City",
    "AR": "America/Argentina/Buenos_Aires",
    "ID": "Asia/Jakarta",
    "KZ": "Asia/Almaty",
    "CD": "Africa/Kinshasa",
    "GL": "America/Nuuk",
    "FM": "Pacific/Pohnpei",
    "PF": "Pacific/Tahiti",
}


def main() -> int:
    raw = json.loads(INPUT.read_text())
    by_country: dict[str, list[dict]] = defaultdict(list)
    by_name: dict[str, dict] = {}
    for zone in raw:
        cc = (zone.get("countryCode") or "").upper()
        if not cc:
            continue
        by_country[cc].append(zone)
        by_name[zone["name"]] = zone

    distilled: dict[str, dict] = {}
    for cc, zones in by_country.items():
        # If we have an explicit override (multi-tz country), use that.
        override_tz = MULTI_TZ_OVERRIDES.get(cc)
        if override_tz and override_tz in by_name:
            primary = by_name[override_tz]
        else:
            # Primary zone: the one with the most main-cities entries
            # (ties broken by alphabetical zone name for determinism).
            primary = max(
                zones,
                key=lambda z: (len(z.get("mainCities", [])), z["name"]),
            )
        cities = primary.get("mainCities") or []
        distilled[cc] = {
            "tz": primary["name"],
            "offset_minutes": primary.get("rawOffsetInMinutes", 0),
            "primary_city": cities[0] if cities else "",
        }

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(distilled, separators=(",", ":"), sort_keys=True))

    print(f"wrote {OUTPUT.relative_to(REPO_ROOT)} — "
          f"{len(distilled)} countries, {OUTPUT.stat().st_size / 1024:.1f}KB")
    # Sample some well-known countries so a human can sanity-check.
    for cc in ("US", "GB", "JP", "BR", "RU", "AU", "IT", "IN"):
        if cc in distilled:
            d = distilled[cc]
            print(f"  {cc}: {d['tz']:32} {d['offset_minutes']:+5}min  "
                  f"({d['primary_city']})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
