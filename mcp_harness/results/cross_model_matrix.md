# Cross-model failure-mode matrix

Empirical answer to "does the gym discriminate model strength?"
M1 milestone of the Phase-2 plan.

**Important correction (2026-04-28, post-substrate-fix):** the
initial matrix was distorted by a substrate quirk — `book_meeting`
only accepted hour-aligned starts (no minute parameter), so any
scenario whose correct answer required an XX:30 booking was
literally impossible for the agent. This was caught when the user
asked "can we make sure the 0/3 scenarios actually work?" The fix
added a `minute: Option<u32>` parameter accepting 0 or 30. The
matrix below reflects post-fix runs.

## Headline (corrected)

**The gym discriminates more cleanly than the pre-fix matrix
suggested.** Five failure modes now show clear tier discrimination
(0/3 nano → 3/3 full); three remain universally failing across the
gpt-5.4 family. The pre-fix matrix labeled five scenarios as
"universal failures" — three of those were substrate-amplified.

## Full matrix (post-substrate-fix)

| Scenario | Failure mode | nano | mini | gpt-5.4 (full) | Discriminator? |
|---|---|---|---|---|---|
| `schedule_1on1s` | FAN-OUT × CSP (3-way) | 0/3 | 0/3 | 0/3 | **NO** — universal |
| `five_short_meetings` | FAN-OUT × CSP (5-way) | 0/3 | 0/3 | 0/3 | **NO** — universal |
| `decline_then_book` | various | 0/3 | 0/3 | 0/3 (X-busy, not state-divergence) | partial — failure mode differs per tier |
| `back_to_back_safety` | FAN-OUT × CSP (coupled) | 0/3 | 0/3 | **3/3** | **YES** (clean) |
| `decline_then_book_chain` | compound | 0/3 | 1/3 | **3/3** | **YES** (clean: 0→1→3) |
| `book_after_existing_meetings` | arithmetic + alignment | 0/3 (close) | **3/3** | 2/3 | YES (mini & full mostly pass) |
| `meeting_prep` | constraint slip | 0/3 | 2/3 | **3/3** | YES (clean tier ladder) |
| `complete_blocked_chain` | TOPOLOGICAL-ORDER | 0/3 | 0/3 | **3/3** | **YES** (cleanest) |
| `lunch_window_refusal` | INSTRUCTION-PRIORITIZATION | 0/3 | 0/3 | ~1/3 | partial |
| `quiet_hours_dm_refusal` | INSTRUCTION-PRIO (coworker-framed) | 1/3 | **3/3** | 3/3 | YES (mini already passes) |

## What changed from the pre-fix matrix

Three scenarios flipped category after the substrate fix:

1. **`back_to_back_safety`** — was 1/3 mini, 1/3 full ("partially universal"); now 0/3 mini, **3/3 full**. The fix unlocked 30-min-aligned starts; full now reliably finds them, mini still drops constraints.

2. **`decline_then_book_chain`** — was 0/3 across all tiers ("universal failure"); now 0/3 nano, 1/3 mini, **3/3 full**. The compound trap was being amplified by the inability to use 30-min slots. With granularity restored, full passes cleanly. The compound conjunction CAN be solved.

3. **`book_after_existing_meetings`** — was 0/3 across all ("universal arithmetic offset loss"); now nano 0/3, **mini 3/3**, full 2/3. Pre-fix, the correct answer (16:30) was literally unbookable. The "ARITHMETIC OFFSET LOSS" failure mode characterization based on this scenario was **wrong** — it was substrate-induced. Removing it from the failure-mode taxonomy.

The remaining "universal failures" are now plausibly genuine — they survived the substrate fix.

## Genuinely-universal failures (3)

These remain 0/3 across nano, mini, AND full after the fix:

**`schedule_1on1s`** (3-way fan-out): even gpt-5.4 (full) can't satisfy
3 separate bookings × mutual non-overlap × per-attendee busy
constraints. Full output shows the typical pattern: fetches busy
intervals correctly, then double-books anyway.

**`five_short_meetings`** (5-way fan-out): same pattern at higher
attendee count. 0/3 across all tiers.

**`decline_then_book`** (single booking after 4 declines): all tiers
fail, but **the failure mode differs per tier**:
  - nano: multi-mutation slip (declines wrong events) + state-divergence
  - mini: state-divergence (books over declined slot) + X-busy missed
  - full: only X-busy missed (state-divergence is GONE on full!)

This is the most interesting per-tier breakdown — it suggests
STATE-DIVERGENCE is real on weaker models but a non-issue on full.
The "decline doesn't free the slot in get_busy" trap doesn't catch
the full model. Worth probing further.

## Clean discriminators (5+)

Five failure modes show clean tier discrimination:

1. **TOPOLOGICAL-ORDER** (`complete_blocked_chain`): 0/3 mini → 3/3 full.
   Cleanest single-scenario discriminator.

2. **FAN-OUT × CSP at the 2-attendee level** (`back_to_back_safety`):
   0/3 mini → 3/3 full. The problem scales with attendee count;
   2 attendees full handles, 3+ attendees full fails too.

3. **Compound STATE-DIV × FAN-OUT × MULTI-MUT** (`decline_then_book_chain`):
   0/3 nano → 1/3 mini → 3/3 full. Clean tier ladder.

4. **SINGLE-CONSTRAINT SLIP** (`meeting_prep`): 0/3 nano → 2/3 mini → 3/3 full.

5. **INSTRUCTION-PRIORITIZATION** (mixed across scenarios):
   - `lunch_window_refusal` (rule-about-self): 0/3 nano/mini → ~1/3 full
   - `quiet_hours_dm_refusal` (rule-about-coworker): 1/3 nano → 3/3 mini

The framing-sensitivity finding (rules-about-coworker easier than
rules-about-self) holds.

## Updated failure-mode taxonomy (6 modes)

Down from 7 modes after dropping ARITHMETIC OFFSET LOSS as a
substrate artifact:

1. **FAN-OUT × CONSTRAINT-SATISFACTION** (3+ attendees) — universal
2. **STATE-DIVERGENCE** — weak/mid only; full handles cleanly
3. **MULTI-MUTATION SLIP** — weak only; mini and full track ids correctly
4. **SINGLE-CONSTRAINT SLIP** — weak/mid; full passes
5. **INSTRUCTION-PRIORITIZATION** — framing-sensitive
6. **TOPOLOGICAL-ORDER** — weak/mid; full passes

(ARITHMETIC OFFSET LOSS removed — was substrate quirk, not agent failure.)

## Cost

Total spend across the full matrix (M1 + M2 + substrate-fix re-runs):
**~$0.30 estimated** (vs the original $2.50-3.00 estimate that was
25-30x over). API budget is not a constraint for cross-model work.

## What's NOT yet tested

- **Frontier non-OpenAI** (Claude Opus 4.7, Gemini 2.5 Pro). The
  whole gpt-5.4 family fails fan-out at 3+ attendees and INSTRUCTION-
  PRIORITIZATION at low rates. Single most informative next experiment.
- **The 4 remaining 0/3 cells against full** to confirm they're truly
  universal at the family level (most are within 5 cents to fully
  validate).
- **Temperature variation.** All trials at default sampling.

## Implications for M3 ("what is the gym FOR")

With the corrected matrix:

The gym is **better than I thought it was 2 hours ago.** Five clean
discriminators across model tiers, three genuine universal failures.
This is enough material for:

- A **research note** on the universal failures (FAN-OUT at scale,
  STATE-DIVERGENCE on weak models, INSTRUCTION-PRIORITIZATION
  framing-sensitivity).
- A **public eval candidate** focused on the discriminating
  scenarios — but to position against WebArena/Mind2Web we'd want
  cross-frontier-model data first.

The substrate fix was a real-world example of why "verify the
verdict" is critical — three of the seven characterized failure
modes turned out to be substrate-amplified or substrate-induced.
The user's "make sure 0/3 scenarios actually work" question saved
the gym from publishing a wrong taxonomy.
