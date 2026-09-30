# The residence enumeration problem

**Date:** 2026-09-30 (overnight, autonomous session)
**Status:** **Decided by the founder, 2026-09-30: B now, with C prototyped as its upgrade** (§5). D2 is revised: the ledger counts unions and births, not moves. Found while detailing the Phase 1 / R1 prototype.
**Context:** spec `2026-09-29-society-as-a-function.md` §8, "L4 residence".

## 1. The problem, framed precisely

We need three things together:
1. **Forward:** where does person `x` live at time `t`? A pure function of `(seed, x, t)`.
2. **Inverse:** who lives in neighbourhood `N` (or at address `a`) at time `t`? This must be exactly the people whose forward answer is `N`, at cost proportional to `|N|` rather than to the population.
3. **Realistic moves:** destinations depend on where the household lives *now*. About 54% of moves stay within the county, 40% of inter-county moves go under 50 miles, and 60% of adults live within 30 miles of a parent (sources in `time-consistent-evolution` §5.1 and `kinship-and-households` §5.1).

In the literature's terms, this is random access to the occupancy of a large system of random walkers whose jumps depend on their position, with forward and inverse queries, statelessly. Each pair of requirements is easy; all three together are not.

## 2. Why each known tool fails one requirement

| Tool | Forward | Inverse | Moves depend on current place |
|---|---|---|---|
| Static bijection per spell (`ψ_k(spell) → (address, era)`) | ✓ | ✓, O(K · eras) per address | ✗: the destination is fixed per spell, independent of the current address |
| Static bijections plus a dynamic index choice weighted by distance (spec §8.2) | ✓ | ✓ | only partly: K random candidates rarely include one near the current address. For a mid-size metro (≈5% of its region), K = 16 gives only ≈55% odds that any candidate is local |
| Candidate systems anchored on a static attribute (lineage region) | ✓ | ✓ | ✗ for migrants: roughly 30–40% of people grew up outside their lineage region, and their "local" candidates sit where their ancestors lived |
| Relative moves (`next = current + offset`) | ✓ | ✗: there is no static preimage | ✓ |
| Vacancy chains (White 1970, applied to housing by Lansing et al. 1969) | ✓ if simulated | needs conflict resolution between simultaneous chains, which is sequential | ✓ |
| Counting moves in the ledger (spec §8.1, multiregional demography) | ✓ | ✓ for dense counted spaces | ✓ at the counted granularity |

The last row is the only one that satisfies all three, and it is what the spec chose. Detailing it exposed its cost:
- **Distance decay needs fine counting.** Moves must be counted at the granularity where destinations depend on origin. Distance decay operates at metro and county scale, so counting must be per metro, not per state.
- **Counting must be joint with lineage.** Kinship needs counts per *birth block* (who is whose child), while residence needs counts per *place*. Exact consistency needs their joint distribution, roughly birth block × place × year. At metro granularity that is ~10⁹–10¹⁰ cells for a US-scale world. At state granularity it is feasible (~10⁸ sparse cells, a few hundred MB), but then moves within a state have no distance decay.
- **Children must move with their mothers.** Where a child lives at leaving home is derived from the mother, so child blocks must nest by the mother's trajectory cell (spec §5.4 step 1). That multiplies the joint table again.

## 3. What does not depend on this decision

Kinship can be exact by **lineage region**: the mother's birth region, not her place of residence. The ledger then counts only unions and births:
- unions per (lineage region, year, female cohort, male cohort), plus a cross-region pool;
- births per (mother block, year).

It needs no migration. It is small (tens of MB even for 51 regions) and can be computed lazily per market. This is being prototyped tonight (R1), because every option below uses it.

Two design findings from working it out:
1. **Deaths don't need quota cells.** Deaths link no blocks. They can be keyed draws conditioned on the survival the person's other cells require (to union start; for parents, to the plan end). Duality and life bounds still hold exactly. Only unions and births need exact integer accounting. This removes the biggest term in the per-block storage estimate.
2. **Two independent partitions of one union cell.** Partner cohort (for couplings) and fertility plan (for births) can be two separate partitions of the same union-start cell, via two keyed permutations. Only the *marginal* counts must be exact; nothing ever needs their intersection. That avoids the product blow-up of nesting one inside the other.

## 4. Options for residence

| Option | Idea | Exact? | Realism | Cost | Risk |
|---|---|---|---|---|---|
| **A. Ledger-counted at state level, bijections within states** | Spec §8 as written, regions = states | yes | Interstate flows and co-location exact. Within-state moves have no distance decay, so a move across the state is as likely as one across town | hundreds of MB of tables; a novel multistate build | high: novel, and the minors-follow-mother nesting is intricate |
| **B. Lineage-anchored candidate tiers** | Kinship by lineage region. Residence dynamic; each household's candidates come in tiers: same city, same lineage region, national | yes | Good for people living in their lineage region (~60–70%), poor for migrants' children | small | medium: the realism tests on move distance and distance to parents may fail for the migrant share |
| **C. Hierarchical addressing** | A new household's first address hangs under its parents' address slot (`(parent slot, spawn index)`). Later moves use candidate tiers relative to that ancestry | yes | Leaving home stays near parents by construction (the most important locality fact) | roster cost grows with how many generations stay in one city; needs a prototype | medium-high: unproven |
| **D. Relax residence exactness** | Forward is exact; rosters are sampled or approximate | **no**: breaks G2 for residence | best | small | violates an approved guarantee |

## 5. Recommendation and the decision needed

**Recommendation:** B now, with C prototyped as its upgrade. A is fallback-only.
- B keeps every guarantee and is cheap. Its realism gap is measurable with the existing targets:
  - share of moves under 50 miles;
  - distance to the nearest parent.
- C attacks exactly that gap, with leaving home near parents by construction.
- A buys exact interstate accounting at high complexity, and still has no within-state distance decay, which is where most moves happen.

**Decision needed (founder):** accept B/C (kinship by lineage region, residence dynamic) instead of the spec's §8.1 region-counted residence? Choosing B/C revises D2 slightly: the ledger counts unions and births but not moves.

**Until then:** the R1 prototype builds kinship by lineage region, which all options share, and gives residence a placeholder: lineage region plus a hashed city, no moves. Nothing built tonight is wasted under any option.

## Sources

- Lansing, Clifton & Morgan, *New Homes and Poor People: A Study of Chains of Moves* (1969): housing vacancy chains. Recalled; the web-search budget was exhausted, so this was not re-verified.
- White, *Chains of Opportunity* (1970), via `work-and-organizations` §5.1.
- Rogers, *Introduction to Multiregional Mathematical Demography* (1975): the multiregional cohort-component framework behind option A. Recalled, not re-verified.
- Move-distance and parent-proximity targets: `time-consistent-evolution` §5.1, §7; `kinship-and-households` §5.1, §8.
