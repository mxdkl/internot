# Re-partnering under exact counts: the problem and the options

**Date:** 2026-09-30
**Context:** R1c in `plans/2026-09-30-r1-kinship-prototype.md`; spec §6.1 ("Re-partnering. Dissolved and widowed people enter pools per (region, band) … A second harmonic-mean table U2 pairs them"). Found while designing R1c after R1a, R1b-1 and R1b-2 were built.

**Status:** **Decided by the founder, 2026-09-30: §4 item 1 (divorce re-partnering via a couple-level dissolution partition), with widowed re-partnering deferred.** Not built yet; prototype on the tiny world first.

## 1. What exactness requires

Second unions must satisfy the same guarantees as first unions:
- **(G1) Reciprocity:** both partners compute the same union.
- **(G2) Ledger closure:** counts per (woman's block, man's block, year) are exact integers that both sides read.
- **(G3) Availability:** only people whose earlier union has ended, and who are alive, are in a second-union cell.
- **(G4) Locality:** every fact is a closed-form lookup.

(G2) and (G3) together mean the ledger must know, as exact integers, how many people of each block and sex become available in each year. There are two sources of availability.

**Dissolution.** Today a union's dissolution band is part of the woman's plan leaf, and plans are a partition of the woman's cell that is independent of the partner partition (D-R1.3). So:
- the women who separate in year `y` are counted exactly per woman block;
- the men are not. How many men of block `j` separate in year `y` depends on the intersection of two independent keyed partitions, a hypergeometric random variable rather than a ledger integer.

**Widowhood.** Deaths are keyed per-person draws conditioned on required survival (D-R1.2), so no one knows how many widows or widowers a block has in a year. Counting them would need death assignments partitioned jointly per couple, i.e. (man's death year × woman's death year) per plan leaf. That is a 2-D blow-up of every leaf.

## 2. Why the obvious fixes fail

| Fix | Why it fails |
|---|---|
| Plans per partner slice (nest plans inside slices) | Kin repair permutes men across slices, so a woman's plan would no longer match her partner's slice, and the men's dissolution counts break. Attaching the plan to the man's slot instead makes `mother()` depend on kin repair, and kin repair calls `mother()`, recursing through generations. |
| Partner slices nested inside plan leaves | Dissolution per (i, j, year) becomes exact. But kin repair must then stay within one plan leaf. Leaves of one or two couples are common, so the residual conflicts that forced the three-layer repair (R1b outcome) return, now at plan-leaf granularity. |
| Men's availability from expected counts | A man designated for a second union might still be married: this breaks (G3). |
| Divorced women remarry never-partnered men only (men's side counted in first-union cells) | Exact, but divorced men never remarry, while real men remarry *more* than women (64% vs 52%, Pew 2014). |
| Counted partnered deaths, jointly per couple | Exact widowhood, at 110 × 110 death-year pairs per leaf. Infeasible. |

## 3. The underlying structure

The constraint is a contingency-table problem with three margins that must hold at once, each in a different partition: woman × plan (births), woman × man block (couples), and man × plan (the men's availability). Independent keyed partitions give two of them exactly. The third is exact only if one partition is nested in another. Nesting then fixes which count-preserving moves exist, and kin repair needs a group of at least two to move in.

Framed that way, the question is: **what is the smallest nesting that keeps all three margins integral while leaving kin repair groups of at least two almost everywhere?**

Candidate framings to research:
- **Leaf-major pairing with repair across leaves of equal dissolution.** Only the dissolution dimension of a plan must match across the swap. Swapping men between women whose plans have the same dissolution *year* keeps the men's dissolution counts exact even if parity differs. Groups would be (cell, dissolution year) ranges. Parity and spacing can stay independent of the partner partition if births are counted on the woman's side only, which they are.
- **Divorce as a couple-level partition, separate from the fertility plan, with fertility conditioned on it.** Couples get (dissolution year) from a partition of the *slice*, which is exact on both sides. Each woman's fertility plan is then drawn from a catalog conditioned on her dissolution year. That makes it a partition of the (cell × dissolution year) sub-cells: nested, with exact births per sub-cell. Kin repair groups within (cell, dissolution year) sub-cells.
- **Widowhood.** Model widowed re-partnering only where it is demographically large (widowed under 55, mostly men). Count those deaths by a partition of the partnered at ages under 55 per (block, sex, year), with the rest of the lifetime keyed as now. A two-stage idea, like the death draw.

## 4. Recommendation (for the founder)

1. **Build divorce re-partnering first, via the second framing.** Dissolution becomes a partition of each partner slice, and the fertility plan is conditioned on it per (cell, dissolution year) sub-cell. Kin repair groups within sub-cells, with the three-layer repair (women's side, men's side for free couples, ledger deferral of isolated couples) applied per sub-cell.
   - It changes D-R1.3 from "plan independent of partner" to "dissolution coupled to partner, the rest of the plan independent".
   - Prototype it on the tiny world first, with the exhaustive kinship suite, and measure repair residuals at sub-cell granularity.
2. **Widowed re-partnering:** decide whether it is worth the partial counted-death machinery. A realism gap remains either way: without it, the widowed never re-partner. Pew: about a fifth to a quarter of the widowed remarry, far fewer than the divorced.
3. **Same-sex unions (U_ss, ~1.5% of couples)** are independent of 1–2 and can go first. The only design question is kin repair within same-sex cells, which is the same mechanics with one sex.

Until then, R1 has one union per person. `siblings()` relies on that; its exhaustive test fails as soon as it stops holding.

## Sources

- Pew Research Center (2014), *Four-in-ten couples are saying "I do," again*: remarriage 57% (men 64, women 52), median 3.7 years after divorce.
- `research/2026-09-29-kinship-and-households.md` §8 (targets), §9 (base construction), SOCSIM notes.
- R1 plan, "R1b-1 and R1b-2 step A outcome" (three-layer kin repair and why pairs alone leave residuals).
