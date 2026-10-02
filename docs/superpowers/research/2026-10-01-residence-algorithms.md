# Residence: forward and inverse access, the obstruction, and constructions

**Date:** 2026-10-01
**For:** Phase 2, residence. **Data and targets:** `research/2026-10-01-residence-data-and-targets.md`. **Earlier decision:** `research/2026-09-30-residence-enumeration-problem.md` (B now, C prototyped).
**How it was gathered:** a research pass over the local-access and random-object literature (arXiv, Crossref, OpenAlex, author pages; WebSearch exhausted), plus analysis. Markings: [read] full text or relevant section read; [abs] abstract only; [rec] recalled; [est] estimate.

## 1. The problem

Three requirements hold together:
1. **Forward:** `address(household, t)`, pure and cheap.
2. **Inverse:** `roster(tract N, t)` returns exactly the households whose forward answer is in N, at a cost not proportional to the population.
3. **Realism:**
   - household moves by age and life event;
   - moves mostly local (distance decay relative to the current address);
   - new households near the parents' current address (at 26, 30% in the same tract as at 16, 58% within 10 miles);
   - places growing and shrinking over the decades.

## 2. What the literature offers

**Local access to random walks** (Biswas, Pyne & Rubinfeld, ITCS 2022, arXiv:2102.07740) [read]:
- gives the position of *one* walk at time t;
- is stateful;
- is polylog only on abelian Cayley graphs, via counts of generator steps over dyadic intervals (the discrete Brownian bridge).

It is forward-only: nothing on many walkers or occupancy.

**Huge random objects** (Goldreich, Goldwasser & Nussboim; Biswas, Rubinfeld & Yodpinyanee) [read §9 / int]:
- The only inverse queries are for **state-keyed** dynamics: iterated permutations p^{±m}(x), and preimages of a random mapping.
- Every state moves under one shared map; walkers carry no randomness of their own.
- The interchange process (stirring, Harris's graphical construction) is the same family. It is local both ways because updates ignore occupants. It stops being local the moment moves depend on who is moving.

**Spatial point processes and count trees** (GGN §5; Funke et al., arXiv:1710.07565, communication-free distributed generation [read]):
- give exact counts, ranks, nesting and growth on the place side;
- don't bind place slots to household identities that kinship has already fixed.

**Matching supply and demand locally:**
- LCAs, parking functions and random sequential adsorption resolve conflicts once both sides can be listed; they don't produce the lists.
- In the plane, exactly matching independent random supply and demand forces heavy-tailed distances:
  - infinite mean in d = 2 (Holroyd, Pemantle, Peres & Schramm, arXiv:0712.1867) [abs];
  - a tail of about 1/r is optimal (Timár, arXiv:0909.1090) [read].
- The escape is to equalize counts by construction, as the kinship ledger does.

**Genealogical placement** (Markov chains indexed by trees, branching random walks, spatial Λ-Fleming–Viot) [abs/rec]: ancestry is local backward. Descendants in a region need their subtree.

No local-access result answers the inverse query with walker-keyed randomness (search not exhaustive).

## 3. The obstruction (analysis)

**Two ways to key randomness:**
- **Walker-keyed:** timing and destination come from the household's own keys. Forward is local; the inverse must *find* walkers.
- **Site-keyed:** randomness lives on places. Both directions are local, but choosing movers by attributes means reading occupants, which is a roster query.

**The catchment argument.**
- Suppose a move channel admits into N households located at time s in a catchment U(N), by a rule that depends on where they are.
- An exact roster must evaluate every household that was in U(N) at some s ≤ t. Exactness gives no discount for low probability.
- Unless "the households in U at s that take this channel" is the preimage of a **static dense index**, finding them needs roster(U, s). Unrolling that gives a backward light cone.
- The cone is finite exactly when some unit C ⊇ N is **closed**: every channel into C that depends on a dynamic origin has its catchment inside C. The roster then costs the enumeration of C's history.

**The unavoidable trade-off.** Per channel, choose one:
- a **static index** (cost ∝ |N|, destinations relative only to static anchors), or
- a **closed unit** (destinations relative to dynamic places up to the unit's scale, cost ∝ the unit's history).

Kin edges are the cheapest dynamic channels, because the world already looks them up locally in both directions (`children`, `mother`, `union`). A destination relative to a relative's or co-member's current address can be inverted from the destination side by walking kin edges from the closed unit's members.

## 4. Constructions

**P1: closed basins, kin channels, itineraries (recommended).**
- **Geography:** tracts ⊂ basins (about county-sized: large counties split, tiny ones merged) ⊂ nation. Boundaries are fixed; attractiveness w(tract, year) varies by era.
- **Channels** (events in the household's stream):

  | # | Channel | Destination | Index |
  |---|---|---|---|
  | 1 | Seeds (founders, immigrant arrivals) | static plans by era | static |
  | 2 | Formation | the basin of a co-member household at formation (parents for leaving home, a partner for a union, the union for a partner leaving at divorce); tract relative to that household's | kin, dynamic |
  | 3 | Local move | tract relative to the current one, in the basin, kernel × w | self, closed unit |
  | 4 | Kin move | near a parent's or child's current basin (returns, elders) | kin, dynamic |
  | 5 | Long move | next itinerary stop (below) | static |

- **Forward** walks the household's events. Channels 2 and 4 evaluate a co-member's address at an earlier time (well-founded, since time strictly decreases). Cost: depth × (household lookup + walk), roughly 0.03–0.15 ms p50 and about 1 ms p99 [est].
- **Roster(N ⊂ B, t):**
  1. Build B's closure E(B): seeds placed in B, itinerary preimages of B's slots, then repeatedly the households that members' co-members formed in or moved into B. Each candidate is confirmed by a forward call.
  2. Filter E(B) by address at t.
  - |E(B)| ≈ 5× B's current households [est], with about 10–15 checks per current household.
  - First touch of a 20k-household basin ≈ 6 s [est], once per process (memo about 6 MB). After that, a roster costs ∝ |B|, or ∝ |N| with a per-basin index.
- **Exact both ways.** The gate: roster equals brute force for every tract at many dates on the tiny world.
- **Realism given up:**
  1. The first long move is static (no origin dependence).
  2. Formation never crosses basins: a child setting up in the next county needs a long move.
  3. No dwelling capacity: tract populations are calibrated in expectation.
  4. Itinerary length is a technical bound K.

**Itinerary flows** (the static part of P1, and the workable form of a counted market):
- Each formation cohort's potential households form a dense domain D_c per lineage region. Stop 1 = ψ_c(D_c), a static bijection to (basin, slot) tiered around the lineage region.
- Stop k ≥ 2 = F_{c,k}(stop k−1), a bijection driven by an integer origin → destination plan. It uses Coupling-style slices with a hierarchical gravity plan (level shares, then weights within the level), so there are no basin × basin tables.
- Later long moves are therefore exactly relative to the previous stop. The inverse at Q enumerates the origins in Q's blocks. Forward costs about k × 1–2 µs [est].

**P2: static only** (option B plus itineraries):
- Rosters cost ∝ |N| uncached (about 20–50 checks per resident [est]); forward costs µs.
- Gives up "near parents" for the children of anyone whose parents moved. Choose it only if uncached O(|N|) rosters are a hard requirement.

**Counted market per (region, year)** (spec §8.1): exact only if the origin side is counted jointly with kinship blocks (the multiregional ledger). Several GB at 51 regions [est]. It buys exact interstate flows by origin and exact regional totals; within states, P1-like machinery is still needed.

## 5. The coupling with kinship

The ledger pairs partners in markets by lineage region. Residence doesn't feed back, so couples' previous homes are as far apart as the lineage region allows: today, two placeholder regions.
- **Realistic partner geography** (most couples grew up in the same metro) needs the ledger's markets to see where people live. That is the counted, multiregional ledger, at some granularity.
- **Static lineage regions drift from residence over generations:** about a third of adults live outside their birth state, and migrants' children inherit the mother's lineage. So a finer static lineage region doesn't fix it.
- Under P1 alone, the "move in with partner" channel produces some long-distance union moves. The distance between partners before the union is the measured cost.

## 6. Sources

**Read:**
- Biswas, Pyne, Rubinfeld, ITCS 2022, https://arxiv.org/abs/2102.07740
- Goldreich, Goldwasser, Nussboim §9, https://www.wisdom.weizmann.ac.il/~oded/PSX/toro2.pdf
- Funke et al., https://arxiv.org/abs/1710.07565
- Timár, https://arxiv.org/abs/0909.1090

**Abstracts:**
- Biswas, Rubinfeld, Yodpinyanee (arXiv:1711.10692); Naor & Nussboim (RANDOM 2007); Even, Levi, Medina & Rosén (TALG 2021); Dong & Mani (arXiv:2409.03951); Anand & Jerrum (arXiv:2106.15992)
- Holroyd, Pemantle, Peres & Schramm (arXiv:0712.1867); Hoffman, Holroyd & Peres (arXiv:math/0505668); Ajtai, Komlós & Tusnády (1984); Penrose (CMP 2001); Konheim & Weiss (1966); Amble & Knuth (1974)
- Benjamini & Peres (1994); Barton, Etheridge & Véber (arXiv:0904.0210); Harris (1972)
- Alessandretti, Aslak & Lehmann (Nature 2020); Simini et al. (Nature 2012)
- Karger et al. (STOC 1997); Lamping & Veach (arXiv:1406.2294); Fakcharoenphol, Rao & Talwar (STOC 2003)

Full list and markings are in the research pass's report (scratchpad `residence-algo/`).
