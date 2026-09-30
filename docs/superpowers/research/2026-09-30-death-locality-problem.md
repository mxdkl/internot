# Death needs the partner: the 1 µs budget versus the father-survival rule

**Date:** 2026-09-30
**Context:** kinship benchmarks after the harness fixes (single-call timing, thermal settling). Spec §16.2 budgets `death` / `alive_at` at **< 1 µs p99 cold**, "prefix-sum search plus a few hashes". Measured p99 is **2.1–2.6 µs**.

**Status:** **Decided by the founder, 2026-09-30: option B, with the father's constraint at conception.**
- The founder pointed out that a father need only be alive at conception, about 9 months before the birth, and that reality is fuzzy here anyway.
- Built the same day:
  - a man's death depends only on his own cells;
  - `father()` is the mother's partner at conception (`GESTATION_DAYS` = 266 before the birth), so children born during the pregnancy after his death keep him;
  - only children conceived after his death have no in-world father.
- **Results:**
  - `death` p99 went from 2.1–2.6 µs to 0.78 µs, and `union` improved 13%.
  - Posthumous births are 0.8% of the 1860 cohort and 0.2% of the 1950 cohort (historically ~1% in high-mortality eras).
  - Children conceived after the mother's partner died are 6.7% of the 1860 cohort, 3.4% of 1920, 1.2% of 1950 and 0.5% of 2010 (`examples/realism_report.rs`). Widowed re-partnering would give them stepfathers.
  - Male cohort e0 lost the old rule's bias: +1.8 years for 1860 and +0.5 for 1950.
- The `mother` gate is set to 2 µs (founder, 2026-09-30).

## 1. The cause

The spec (§6, "Death cell ... nested after the plan end so that **mothers** are alive at every planned birth") requires only mothers to survive their fertility plan. The R1 plan's D-R1.2 went further: **fathers** must survive their partner's plan too. That was my addition, and it was never checked against the budget.

A woman's plan is on her own side, so her constraint is local. A man's constraint is his partner's plan end, and finding the partner means a chain of cold loads across two blocks plus kin repair. The two-stage draw already skips the partner whenever the man's own draw clears a per-cell bound. But the rest cost 2–3 µs.

Measured over 400,000 uniform ids (prototype world). All figures are % of all `death` calls.

| Path | Women | Men | Cost |
|---|---|---|---|
| Fast: own draw clears the bound | 42.3 | 47.4 | p50 0.39 µs |
| Group bound accepts (partner's repair group; no repair resolved) | — | **2.88** | ~2.3 µs |
| Exact required age computed, draw kept | 5.69 | **0.30** | women local; men ~2.5 µs |
| Redraw with the exact required age | 0.72 | **0.73** | women local; men ~2.9 µs |

**3.9% of calls need the man's partner.** The redraws alone (0.73%) need the exact partner under any exact scheme. So a p99 below 1 µs is out of reach unless the partner lookup itself costs < 1 µs. `union` is 2.0 µs at p50.

## 2. Why engineering doesn't close it

- A lookup is a chain of dependent cold loads. On this machine that is **~95 ns per load over 128 MB**, and 13 ns within L3.
- The partner path is ~20 dependent loads: the man's block, cohort, sub-cells and cell; the partner slice; the woman's block, cell and plans; then the leaves of 2–3 women.
- Tried or measured today:
  - **Huge pages** (`MADV_COLLAPSE` on the whole heap): 10–20% on every lookup.
  - **An inverted per-offset leaf index for `mother`:** −43% instructions, no change in cycles, +31 MB. Reverted: the scan it replaced was sequential and prefetched, so it was nearly free.
- Flattening layouts into hot headers and arenas might cut the chain by a third. That still leaves the partner path at ~1.5 µs.

## 3. Options

**A. Keep the father-survival rule; approve an exception.** Death p99 cold is then about the partner-path cost: ~2.5 µs today, perhaps ~1.5 µs after layout work.
- Exact.
- Partnered men's mortality is slightly flattered: no man dies before his partner's last planned birth. The never-partnered residual only corrects this for natives.

**B. Follow the spec: only mothers survive their plan.** A man's death depends only on his own cells, so every death is local and fits the budget.
- The cost is births after the father's death. The union ends at the first death (spec §6), so those children's `father()` is `None` (a posthumous window of ~9 months could keep the father).
- Share of a partnered man's children born after the calendar year of his (now unconditioned) death, by his birth decade:

  | Father's birth decade | Men who die before their partner's last planned birth | Children born after the father's death |
  |---|---|---|
  | 1820s–1860s | 8–13% | 5.6–8.0% |
  | 1870s–1890s | 4–7% | 2.6–4.8% |
  | 1900s–1930s | ~2% | 1.2–1.5% |
  | 1950s on | ≤ 1% | 0.4–0.7% |

- Historically, most such widows' later children had a new husband as their father. With widowed re-partnering deferred, the model shows them as having an unknown father. The artifact sits in 19th-century cohorts, i.e. the great-grandparents of today's living people.
- It also removes the survival flattering in A.

**C. Put the plan end on the couple (edge) partition, as part of R1c.**
- R1c already moves dissolution onto a partition of each partner slice, which both partners can read.
- The plan end is the same kind of attribute: both endpoints need it, the woman for births and the man for survival. Keying the sub-cells by (dissolution year, plan-end year) and conditioning the woman's plan on them makes a man's required age local and exact.
- **Risk:** kin repair works within sub-cells, and this makes them ~20× finer. Many would hold one or two couples, which is the residual-conflict regime the three-layer repair was built for (R1b outcome). This needs a prototype on the tiny world before anyone can say it works.
- A coarse plan-end band doesn't help. Redraws still need the exact partner, and any band wider than a year leaves too many men needing it.

## 4. Recommendation

**B.** It is the spec as written, meets the budget with no new mechanism, removes the survival bias in A, and its artifact is confined to 19th-century cohorts. Widowed re-partnering, deferred by the founder on 2026-09-30, would later give those children stepfathers.

C is the principled exact version, and can be revisited when widowed re-partnering is designed, since both need couple-level attributes.

## 5. Related budget: `mother`

`perf/budgets.toml` gates `mother` at 0.7 µs. I set that figure from batched measurements, which were flattered by cache reuse. Spec §16.2's budget for `mother_of` (one hop) is **< 5 µs cold**. Measured single-call p99 is 1.3–1.4 µs. Aligning the gate to the spec is a budget raise over the file and so needs the founder's approval.

## 6. Other numbers from today

- **World memory:** 152 MB RSS for the prototype world (11.1M ever born). Cells ~42.5 MB, cohorts ~17 MB, birth tables ~6 MB, plus the ledger.
- **One-hop p99 against the 5 µs spec cap:**

  | Query | p99 |
  |---|---|
  | father | 4.5 µs |
  | union | 4.9 µs |
  | children | 4.7 µs |
  | siblings | 3.5 µs (4 µs gate) |

  Those have no real margin, whatever is decided here. The layout work (hot headers, arenas, fewer dependent loads) targets them next.
