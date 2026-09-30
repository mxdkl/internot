#!/usr/bin/env python3
"""Build adjective/noun pools for handle generation.

Source: EFF Diceware short word lists (CC-BY 3.0).
We pick the first 512 entries that are pure ASCII letters, lowercase, no spaces.

Run: python3 internot/build_pools.py
Outputs: internot/data/sites/handle_pools/{adjectives,nouns}.json
"""

from __future__ import annotations

import json
import re
import urllib.request
from pathlib import Path

EFF_SHORT_URL = "https://www.eff.org/files/2016/09/08/eff_short_wordlist_1.txt"

REPO_ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = REPO_ROOT / "internot" / "data" / "sites" / "handle_pools"

CURATED_ADJECTIVES = [
    "silver", "crimson", "mossy", "velvet", "ember", "feral", "amber",
    "azure", "scarlet", "indigo", "russet", "violet", "tawny", "umber",
    "ivory", "jade", "midnight", "saffron", "smoke", "frost", "coral",
    "cobalt", "pearl", "ochre", "olive", "rust", "slate", "sapphire",
    "ash", "bronze", "copper", "gold", "neon", "noir", "pewter", "ruby",
    "topaz", "vermilion", "wine", "denim", "chartreuse", "fuchsia",
    "magenta", "ultraviolet", "obsidian", "porcelain", "alabaster",
    "marigold", "lilac", "pomegranate", "tangerine", "blush", "moss",
    "willow", "thistle", "fern", "bramble", "bough", "vine", "bloom",
    "feathered", "winged", "lithe", "swift", "lazy", "drowsy", "lucid",
]
CURATED_NOUNS = [
    "otter", "lantern", "cipher", "orbit", "archive", "feather",
    "compass", "harbor", "meadow", "lighthouse", "atlas", "mosaic",
    "echo", "current", "petal", "wraith", "obelisk",
    "garden", "fountain", "study", "salon", "library", "gallery",
    "veranda", "balcony", "atrium", "loft", "haven", "solace", "nook",
    "circuit", "telegraph", "kiln", "forge", "ledger", "almanac",
    "manifest", "treatise", "symphony", "sonnet", "chronicle", "fable",
    "carbon", "brass", "graphite", "linen", "marble",
    "thicket", "grove", "knoll", "ridge", "vale", "burrow", "warren",
    "hearth", "chimney", "rooftop", "courtyard", "boulevard",
    "alleyway", "promenade",
]


def fetch_eff_words() -> list[str]:
    try:
        with urllib.request.urlopen(EFF_SHORT_URL, timeout=10) as r:
            text = r.read().decode("utf-8")
    except Exception as e:
        print(f"warn: failed to fetch EFF list ({e}); using curated only")
        return []
    words = []
    for line in text.splitlines():
        parts = line.split()
        if len(parts) == 2 and re.fullmatch(r"[a-z]+", parts[1]):
            words.append(parts[1])
    return words


def main() -> None:
    eff = fetch_eff_words()
    out = OUTPUT_DIR
    out.mkdir(parents=True, exist_ok=True)
    # Build pools by concatenating curated heads with EFF tail, dedup, take first 512.
    adj_seen, noun_seen = set(), set()
    adjectives, nouns = [], []
    for w in CURATED_ADJECTIVES + eff:
        if w not in adj_seen and len(w) <= 12:
            adj_seen.add(w); adjectives.append(w)
        if len(adjectives) >= 512: break
    for w in CURATED_NOUNS + eff:
        if w not in noun_seen and w not in adj_seen and len(w) <= 12:
            noun_seen.add(w); nouns.append(w)
        if len(nouns) >= 512: break
    while len(adjectives) < 512: adjectives.append(f"adj{len(adjectives):03d}")
    while len(nouns)      < 512: nouns.append(f"noun{len(nouns):03d}")
    (out / "adjectives.json").write_text(json.dumps(adjectives, indent=2))
    (out / "nouns.json").write_text(json.dumps(nouns, indent=2))
    print(f"wrote {len(adjectives)} adjectives + {len(nouns)} nouns")


if __name__ == "__main__":
    main()
