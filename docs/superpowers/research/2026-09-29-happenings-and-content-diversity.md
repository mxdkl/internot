# Happenings and content diversity: what to generate, how to keep it varied, how to render it faithfully

**Date:** 2026-09-29
**Scope:** research survey for the "happenings log" architecture. A deterministic log of projects, meetings, requests, deadlines, appointments, social plans and purchases is derived from the social graph and time. Every service is a view of that log. An LLM (DeepSeek) only rewrites structured facts into prose, and its output is cached and checked.
**Builds on:** [`2026-05-14-communication-patterns.md`](2026-05-14-communication-patterns.md) (tie strength → rate, mode choice, burstiness, IPP sampling, topic mixes per tie kind). Not repeated here; §0 corrects parts of it.
**Method:** web research across five questions (Sept 2026). Numbers are quoted from sources that were fetched and read. "(inferred)" or "(i)" marks our own derivations and recommendations. The Enron baselines in §3.3 were computed for this survey; the method is given so they can be reproduced (script not committed).

## Summary: what is directly usable

1. **The ATUS activity lexicon is the life-happening taxonomy.** 17 top-level categories, 3-tier 6-digit codes. ATUS 2025 gives "% of people doing X on an average day" by age, sex, employment and children, which converts directly into per-person rates (§1.1–1.2).
2. **Work calendar marginals exist.** Median meeting 35 min; 45% are 30 min; 48% recurring; 49% of one-offs are 2-person; 35% booked less than 24 h ahead (Flowtrace, 1.3M meetings). 1:1s: 40% weekly, 25% biweekly, 18% monthly (Reclaim). ICs about 11 h/week in meetings, managers about 7 h more (Clockwise).
3. **The glue between services has numbers.** 52% of random Enron emails contain a request for action (Lampert 2010). Meetings average about 3.8 action items (AMI). About 41% of to-dos are never done (iDoneThis). 80% of reminders fire within 24 h (Cortana, 576k reminders).
4. **Model each happening type as a parametrized storylet**: roles, a precondition query over graph and time, soft trait- and tie-weighted rules, and effects that spawn child happenings. Variety must come from *different causes and casts*, not more random slots (Tracery's "oatmeal problem", Façade, Comme il Faut, Versu) (§2).
5. **Don't simulate with LLM agents.** Generative Agents cost thousands of dollars for 25 agents over 2 days; OrgForge's LLM planners make runs non-reproducible. Use deterministic generators plus pacing, cooldowns and story sifting.
6. **Diversity must live in the facts, not the sampler.** Same-model LLM outputs are above 0.8 cosine similarity in 79% of cases, and DeepSeek-V3 vs GPT-4o is 0.81 (Artificial Hivemind). 67–99.7% of LLM outputs contain POS templates vs 36–83% of human text (§3.2).
7. **We have real Enron baselines and a templated positive control.** 1,000 person-written Enron emails: gzip compression ratio 2.45–2.60, self-BLEU 0.19, masked-opener top-1 share 1.8%. An old-Internot-style templated inbox: 13.2 / 0.97 / 15.3%. §6 turns these into pass/fail thresholds.
8. **Placeholder rendering plus a closed-world validator is the reliable recipe.** Zero-shot LLM data-to-text has at least one error in 61–86% of outputs (Kasner & Dušek 2024); controlled systems reach about 1% slot errors (E2E), and placeholders make that check exact (§4).
9. **Rendering is cheap.** deepseek-flash costs about $0.00017 per email attempt off-peak, so about $3 renders 13–15k emails. Cache forever and ship the cache with published seeds, because LLM output is not reproducible.
10. **Style knobs have evidence behind them.** 75% of real email has no greeting; org culture moves "no greeting" from 17% to 59%. Personal mail is informal 56% of the time vs 21% for business. Median reply latency is 47 min, internal mail is answered 2× faster than external, and the median thread is 2 messages (§5).

---

## 0. Corrections to the 2026-05-14 communication-patterns notes

| Claim there | Finding | Action |
|---|---|---|
| "Email ~30 received/day at the median knowledge worker (Radicati)" | Microsoft 365 telemetry 2025: **117 received/day**; Fisher 2006 (cited in Mark 2016): 87/day. Radicati's per-user figure is paywalled and unverified. | Split interpersonal mail (about 15–40/day, i) from bulk mail; drop the attribution. |
| Money "2–10 transactions/day" | Atlanta Fed 2025 diary: **47 payments/consumer/month (≈1.5/day)**, average $141. | Use ≈1.5/day. |
| Chat/SMS "μ≈ln(20 chars), median 7–10 words" (these contradict each other) | NUS SMS: median **36 chars / 7 words** (lognormal σ≈0.86); dating-app chat median 8 words. | μ = ln 36 chars. |
| Email "lognormal μ≈ln(80 words), σ≈1.2" | Yahoo replies (Kooti 2015): **median 43, mean 153, mode 5 words**; 30% over 100 words, which implies σ≈1.6. | μ = ln 43, σ≈1.6 for replies; initiating mail longer (§5.1). |
| "Goh & Choi et al. … vocabularies overlap <10%" | No year or venue; nothing found. | Treat as unverified. |
| Calendar invites 1–5/day | Roughly consistent, but **57% of meetings are ad hoc with no invite** (Microsoft 2025). | Ad hoc calls show up only as chat traces. |
| Tasks "backlog in steady state" | About 41% of to-do items are never completed (iDoneThis, weak source). | Backlogs grow and get pruned. |

Not re-checked: the per-tie topic percentage tables, "~100 SMS/day", Enron per-person means.

---

## 1. What happens in people's lives and work, as data

### 1.1 ATUS activity lexicon (the life taxonomy)

The BLS ATUS 2024 lexicon is "a 3-tiered classification system with 17 first-tier categories". Codes are 6 digits: 2 each for major category, tier 2 and tier 3.

**Tier 1:** 01 Personal care · 02 Household · 03 Caring for household members · 04 Caring for non-household members · 05 Work · 06 Education · 07 Consumer purchases · 08 Professional & personal care services · 09 Household services · 10 Government & civic · 11 Eating & drinking · 12 Socializing & leisure · 13 Sports · 14 Religious · 15 Volunteer · 16 Telephone calls · 18 Traveling.

**Tier-2/3 codes that map onto happenings:**
- 0209 Household management: financial management, organization & planning, mail, household email.
- 07 Purchases: groceries, gas, food, other shopping, 0702 researching purchases.
- 08 Services: childcare, banking, legal, 0804 medical, 0805 personal care, real estate, veterinary.
- 09 Household services: home repair, lawn, pet, vehicle.
- 100103 "Obtaining licenses & paying fines, fees, taxes".
- 1202 attending or hosting parties and ceremonies; 1204 arts & entertainment.
- 16 Telephone calls is split at tier 3 by *counterparty*: family, friends/neighbors, education providers, salespeople, care providers, household-service providers, government. This is a ready-made list of correspondent kinds.

Most tier-2 groups carry "Waiting associated with…" leaves, and 18 Traveling mirrors every category. The lexicon's own convention is that each happening can spawn travel and waiting sub-events.

### 1.2 ATUS 2025 (released 2026-06-25; population 15+)

| Activity | Avg h/day | % doing it on an average day | h if done | Implied rate (i) |
|---|---|---|---|---|
| Working & work-related | 3.32 | 41.5% | 7.98 | — |
| Household activities | 1.99 | 80.9% | 2.46 | — |
| Household management | 0.17 | 18.6% | 0.92 | ≈1.3 bill/admin sessions/week |
| Consumer goods purchases | 0.30 | 34.8% | 0.87 | ≈2.4 shopping trips/week |
| Professional & personal care services | 0.10 | 7.8% | 1.33 | ≈28 appointments/year |
| Caring for household members | 0.49 | 22.0% | 2.23 | — |
| Caring for non-household members | 0.15 | 7.6% | 1.97 | ≈0.5/week (aging parents etc.) |
| Socializing & communicating | 0.58 | 30.4% | 1.92 | ≈2.1 occasions/week |
| Telephone, mail & email (personal) | 0.19 | 19.2% | 1.01 | — |

**Work:** 81% of employed people work on an average weekday, 30% on a weekend day. 35% worked at home on days worked (51% with a bachelor's degree, 19% with high school only).

**Children:** caring for household members takes 2.37 h/day when the youngest child is under 6, vs 0.08 h with no children. Leisure is 3.46 vs 5.73 h.

**Age (Table 3):**
- Work peaks at 35–44 (4.83 h).
- Caring peaks at 35–44 (1.23 h).
- Purchasing rises with age (0.38 → 0.88 h).
- Leisure bottoms out at 35–44 (3.89 h).

**Trend:** socializing on an average day fell from 38% of people (2015) to 30% (2025).

This gives life-stage modulation that can key off the people crate's age, children and employment (i).

### 1.3 Work communication volume

- **Microsoft 365 telemetry (to 2025-02-15):**
  - 117 emails received per day, "most … skimmed in under 60 seconds"; 153 Teams messages per weekday.
  - Mass email (20+ recipients) up 7%; 1:1 threads down 5%; more than 50 messages outside core hours.
- **WTI 2023:** 57% of work time is communicating; the top-quartile email user spends 8.8 h/week on email.
- **Mark et al. CHI 2016** (40 information workers): about 1.5 h/day on email, checked 77 times a day.
- **McKinsey 2012:** email management takes about 28% of the workweek.

Implication (i): a realistic inbox is mostly bulk and automated mail plus a thinner interpersonal stream. Generate and test the two layers separately.

### 1.4 Meetings and calendar composition

- **Flowtrace 2024** (1.3M meetings):
  - Duration: median 35 min; 45% are 30 min; 12% run over 60 min.
  - Recurring: 48% of meetings; average 28 min; 45% scheduled for 15 min; 29% have 7+ people.
  - One-off: average 41 min; 49% have 2 people; 90% have 2–6.
  - Lead time: 35% created less than 24 h ahead; 8% more than a week ahead. About 60% have no agenda.
  - Weekly load clusters at about 8, 16 and 19 h/week (Flowtrace reads these as ICs, managers, leaders).
- **Microsoft 2025:**
  - 50% of meetings fall 9–11 am or 1–3 pm.
  - Tuesday carries 23% of the week's meeting load, Friday 16%.
  - 57% of meetings are ad hoc calls; about a third cross time zones.
- **Clockwise 2022** (80k developers): ICs 10.9 h/week, managers about 7 h more, "largely due to … one-to-one meetings". About 35% of teams run daily standups.
- **Reclaim** (15k users, heavy-calendar bias): 1:1s average 42.9 min. Each week 42.4% are rescheduled and 29.6% cancelled.
- **Perlow et al. HBR 2017:** executives spend about 23 h/week in meetings.

### 1.5 Requests, commitments, action items: the glue between services

- **Requests in email.** Lampert, Dale & Paris 2010 annotated 664 random Enron messages (κ=0.681). Of the 505 unanimously labelled, **52.08% contain a request for action**.
- **Requests generate work.** In Avocado (Yang et al. SIGIR 2017), request emails get replies 14.81% of the time vs 7.45%, but more slowly (median 81 vs 55 min). Commitments barely change reply rate (8.32% vs 8.78%). Requests lead to tasks and to delayed replies.
- **Action items.** The AMI corpus has 101 annotated meetings with **381 action items (≈3.8 per meeting)**. These are mostly scenario-based design meetings, so treat the figure as an order of magnitude.
- **Workflow logs as happening templates.** BPI Challenge 2020 is TU/e travel and expense data from 2017–18: e.g. 10,500 domestic declarations (56,437 events) and 7,065 travel permits (86,581 events). The approval chain is employee → travel admin → budget owner → supervisor (→ director), and a rejection leads to resubmission. That gives real multi-step request flows for expense, travel and purchase happenings (i).

### 1.6 Tasks and to-dos

**Graus et al. UMAP 2016** (576,080 Cortana reminders, 92k users):

| Reminder type | Share | Breakdown / examples |
|---|---|---|
| Go somewhere | 33.0% | errands 83%; context switches 17% ("leave for airport") |
| Chores | 23.8% | recurring 66.5% (trash, "pay rent/bills"); one-off ("renew passport", "submit timesheet") |
| Communicate | 21.1% | "call mom" 95%; coordinate 5% ("make doctor's appointment") |
| Manage an ongoing process | 12.9% | — |
| Manage own activity | 6.3% | — |
| Eat / take medicine | 2.8% | — |

- 25% of reminders fire within the hour, 80% within 24 h.
- **50 verbs cover 60.9% of reminders**, so a small verb × object grammar is realistic.

**Other task data:**
- Wunderlist logs (MS-LaTTE): home and work dominate task locations, grocery store third; completion peaks on weekday evenings.
- iDoneThis (weak vendor data): 41% of items never completed; 50% of completed ones finish within a day.
- Reclaim: ICs complete 53.5% of the week's planned tasks.

### 1.7 Household logistics and life admin

- **Medical:** 3.2 physician office visits per person per year (NAMCS 2019); 84.7% of adults saw a doctor in the past year (NHIS 2025).
- **Dental:** 66.7% of adults had an exam in the past year.
- **Payments:** 47 per month, $6,656/month total (Atlanta Fed 2025).
- **Subscriptions** (C+R 2022, n=1,000): people guessed $86/month; itemized actual spend was $219. 42% kept paying for an unused service; 72% use auto-pay.
- **Parcels:** 171 per household per year (Pitney Bowes 2025; includes business parcels, so an upper bound).
- **E-commerce:** 17.1% of US retail sales (Census, Q2 2026).

---

## 2. Event and story generation that stays diverse

### 2.1 Grammars and the oatmeal problem

- **Tracery** (Compton, Kybartas & Mateas 2015): recursive symbol expansion with modifiers. *Push-pop actions* `[hero:#name#]` bind a sampled value once, so it stays consistent through the output; the grammar acts as a blackboard.
- **Compton's "10,000 bowls of oatmeal":** content can be mathematically unique yet perceptually identical.
  - *Perceptual differentiation* (not identical to the last one) is cheap. *Perceptual uniqueness* needs "evidence of process and forces", i.e. visible causes and history.
  - Grammars are weak at constraints between distant elements; parametric generators give "something 'new', but never something surprising".
- The old Internot inbox (pair-hash topics plus templates) was exactly this failure (i).

### 2.2 Storylets, quality-based and salience-based narrative

- A **storylet** is content plus preconditions on world state plus effects (Emily Short 2016, 2019; Kreminski & Wardrip-Fruin, ICIDS 2018). Content is modular and additive.
- **Salience-based selection** fires the most specific eligible variant; generic fallbacks guarantee coverage.
- **Parametrized storylets** (Starfreighter): preconditions are *database queries that bind characters to roles*. One storylet matches many bindings, and the binding changes both text and effects.
- **Repeatability is a per-storylet design axis:** never, always, or authored cooldown. *Reigns* deals weighted cards and re-deals relevant ones; *Glass*'s waypoint transitions are used up once played.
- **Mapping (i):** happening type = parametrized storylet (precondition = graph+time query, cast bound to roles, effects appended to the log).

### 2.3 Drama managers and narrative planning

- **Façade** (Mateas & Stern 2005):
  - About 2,500 joint dialog behaviours, 27 beats; one run uses at most 25% of the content.
  - The authors: "thousands of different beat sequences possible; however, since most beats are causally independent, the number of meaningfully different beat sequences are few". Variation *within* beats mattered more.
- **IPOCL** (Riedl & Young, JAIR 2010): plots need causal soundness *and* character intentionality. Every action needs a character goal that motivates it.
- **Lesson (i):** shuffling independent happenings doesn't create different lives. Causal chains do (request → meeting → tasks → slip → follow-up), with a motivated initiator.

### 2.4 Simulation, emergence and story sifting

- **Versu** (Evans & Short 2014): autonomous characters choose actions by summing their desires.
  - **Social practices** (greeting, meal, family) are authored independently of characters, coordinate agents through roles, and run concurrently.
  - Relationships are role evaluations ("how well is X playing colleague").
- **Comme il Faut / Prom Week** (McCoy et al.):
  - Characters have traits (permanent), statuses (time-limited), and directed 0–100 networks for romance, friendship and coolness. The networks are separate from public relationships, so asymmetric tension can be represented.
  - A cultural knowledge base (x likes, y dislikes, some thing) supplies topics to bond or fight over.
  - A **social-facts history** of labelled exchanges is queried by time window ("You know when you made fun of my SAT score?"), a "compounding effect of history".
  - Hard preconditions are sparse; soft influence rules are many (e.g. `romance>66 ∧ inarticulate → +3`), grouped into reusable "microtheories".
- **Talk of the Town** (Ryan): simulated from 1839 to 1979, producing 300–500 NPCs with social, family and work networks. Beliefs spread, mutate and get forgotten.
- **Ryan's dissertation (2018):** raw simulation output "will almost always lack story structure". Dwarf Fortress and The Sims work because players curate. Tarn Adams: "you need to do post-processing or investigation to find any good moments". This is **story sifting**.
- **Felt (2019) and Winnow (AIIDE 2021):** Datalog-style patterns over an event database (e.g. "violation of hospitality"). Winnow matches incrementally, so it finds storyful sequences *while they unfold*.

### 2.5 Quest grammars and pacing directors

- **Doran & Parberry 2011** (750+ MMO quests):
  - 9 NPC motivations, e.g. Conquest 20.2%, Equipment 18.5%, Knowledge 18.3%, Protection 18.2%, Serenity 13.7%.
  - Each motivation has 2–7 strategies; each strategy is a verb-noun pair of 1–6 actions that expand recursively (a BNF grammar).
  - "Motive → strategy → steps across services" transfers directly (i).
- **Skyrim / Fallout 4 radiant quests:** "repetitive and boring" templates with no motive or history. They "shrink [the world] by reminding you … you are in a video game".
- **RimWorld storytellers:** state-dependent event choice (wealth, recent deaths, time since the last major event).
  - Cassandra: 4.6-day on / 6-day off phases, at least 1.9 days between major threats, about 8.5 major threats/year, minor events every 4.8 days on average.
  - This is a *pacing director* laid over random draws.
- **Nemesis system:** history attached to specific entities (a returning captain with scars) turns generic enemies into characters.
- **Wildermyth:** casting ("a leader, a hothead, and a goofball"), plus inline markup that varies lines by personality or relationship.

### 2.6 The LLM era: cost and homogenization

- **Generative Agents** (Park et al. 2023): retrieval = recency + importance + relevance. 25 agents × 2 days cost "thousands of dollars". Failure modes: overly formal dialogue, embellishment (invented details), norm errors.
- **1,000-person agents** (Park et al. 2024): agents built from 2-hour interviews reproduce General Social Survey answers 85% as accurately as the people themselves two weeks later, with less bias than demographic-only agents. Dense per-person facts beat demographic tags.
- **Homogenization:**
  - InstructGPT (not base GPT-3) reduces writing diversity (Padmakumar & He, ICLR 2024).
  - AI ideas raise similarity between writers by 10.7% (Doshi & Hauser 2024).
  - LLM plots recur across generations and across models ("Echoes in AI", 2025).
- **Diversity levers:** PersonaHub's 1B personas (Ge et al. 2024); verbalized sampling, 1.6–2.1× diversity (Zhang et al. 2025).

### 2.7 Synthetic-world prior art

| System | Where facts come from | Lesson for Internot |
|---|---|---|
| **OrgForge** (arXiv 2603.14997) | Python engine with a `SimEvent` bus; 32+ event types (incident, 1on1, pr_review, async_question, watercooler_chat, employee_departed…). Graph dynamics: stress spread, edge decay γ=0.97/day, escalation by shortest path. The LLM writes prose from exact event facts; state is read only from JSON fields; per-person "voice cards" (stress > 80 → terse). | Prose-to-event fidelity 0.996 vs 0.540 for chained LLMs. **Chaining spreads invented facts faithfully**: cross-document agreement 0.85 but fidelity to truth 0.54. But planning uses 5 LLM calls per day, so runs aren't reproducible. Internot's planning must be deterministic. |
| **ARE / Gaia2** (arXiv 2509.17158) | PersonaHub persona → app dependency graph → about 400K tokens of free text per universe (Llama 3.3 70B), then structured decoding. 10 universes. | "More complex inter-app dependencies remain unhandled". Text-first is the opposite order to Internot's structure-first. |
| **AgentMercury** (arXiv 2608.20634) | 4,783 LLM-built environments with executable cross-service invariants. | Construction success rose from 3.3% to 83.3% only after fine-tuning. LLM-built worlds need checks; construction-time coherence avoids them. |
| **Synthea** (JAMIA 2018) | JSON state-machine modules: 7 control and 11 clinical state types; direct, distributed, conditional and complex transitions; 7-day timestep; attributes shared across modules. Parameterized from public statistics. | First validation found diabetes amputations **4000×** the national rate and 20% of type-2 diabetes onsets in infancy. **Calibration tests against public marginals are mandatory.** |
| **ESL-Bench** (arXiv 2604.02834) | An LLM plans the sparse events; a simulation drives the dense indicators; an event log carries impact parameters. | "All ground-truth answers are programmatically computable" from the log, the same property Internot wants. |

### 2.8 What makes variety at scale (synthesis, i)

1. **Different causes, stakes and consequences**, not more slot values.
2. **State-conditioned selection** with tiered specificity.
3. **Casting by trait and relationship**, including asymmetric ties and shared-object topics.
4. **Compounding, labelled history** that later content refers back to.
5. **Soft many-factor scoring** rather than hard rules.
6. **Pacing directors:** busy and quiet phases, refractory gaps.
7. **Novelty memory:** cooldowns, used-up content, draw without replacement.
8. **Sifting:** simulate mundane life, surface the storyful chains.
9. **Surface variation last:** it buys differentiation only.

---

## 3. Measuring repetition and diversity

### 3.1 Metrics

**Lexical (form):**
- **distinct-n** (Li et al. 2016): unique / total n-grams. Length-biased; use a fixed token budget, or EAD (Liu et al. 2022).
- **Self-BLEU** (Texygen 2018): cost grows with the square of the sample size.
- **Shaib et al. 2024** (arXiv 2403.00553):
  - **gzip compression ratio (CR)** = raw bytes / gzip bytes; higher means more repetitive. Under 1 s vs hours for self-BLEU.
  - **N-gram diversity** = Σ distinct-n for n = 1..4.
  - **Self-repetition** = mean over documents of log(Σ occurrences of the document's 4-grams in other documents + 1).
  - **Homogenization** = mean pairwise ROUGE-L or BERTScore.
  - CR correlates 0.7–0.9 with n-gram diversity but only weakly with self-BLEU. Report CR, self-repetition and self-BLEU together, *always with length*. Package: `pip install diversity`.
- **Syntactic templates** (Shaib et al. 2024, arXiv 2407.00211): a template is one of the top-100 POS n-grams (n = 4–8); template rate is the share of texts containing one.
  - At n = 6, human references: 46.4% (Rotten Tomatoes), 36.0% (CNN/DM). Models: 97.0–99.6% and 83.2–97.4%.
  - Sampling lowers OLMo's rate to 72–77%.
  - 76% of model templates appear in pretraining data, vs 35% of human ones.

**Content:**
- **Vendi score** (Friedman & Dieng 2023): exp(entropy of the eigenvalues of K/n), the "effective number of distinct items". It grows with n, so fix n.
- **Tevet & Berant 2021:** n-gram metrics measure *form*; embeddings are needed for *content*. Decoding parameters mostly change form.
- **MAUVE:** needs about 5k vs 5k samples and is for relative comparison only.

**LLM tics:**
- Antislop (ICLR 2026): some patterns are more than 1,000× more frequent in LLM output than in human text.
- EQ-Bench slop score = 60% slop words + 25% "not X but Y" + 15% slop trigrams (lists published).
- Kobak et al. 2024 (15M PubMed abstracts): 2024 excess ratio 25.2 for "delves", 9.1 for "underscores".

### 3.2 Evidence that LLM rendering needs gates

- **Artificial Hivemind** (Jiang et al., NeurIPS 2025): same-model pairwise similarity is above 0.8 in 79% of cases at T=1.0, and 61.2% even at T=2.0 with min-p. Cross-model similarity is 0.71–0.82 (DeepSeek-V3 vs GPT-4o 0.81). Switching model or raising temperature doesn't buy diversity.
- **BARE** (arXiv 2502.01697), synthetic Enron-style emails, mean pairwise cosine: GPT-4o 0.574; GPT-4o with personas 0.580 (*no better*); Llama-3.1-70B base 0.350.
- **Kirk et al. 2023:** RLHF reduces output diversity. **NoveltyBench 2025:** larger models in a family are often *less* diverse.

### 3.3 Enron baselines (computed for this survey)

**Method:**
- 5,700 rows from HF `corbt/enron-emails` (60 random blocks of 100, 37 mailboxes).
- Quoted and forwarded text cut ("Original Message", "Forwarded by", "From:"/"To:" lines, ">" lines). Bodies under 5 words dropped; de-duplicated on (sender, body), leaving 4,740 bodies.
- Tokens: lowercase `[a-z0-9']+` plus single punctuation marks.
- Values are the mean of 5 random N=1,000 subsets (2 subsets for self-BLEU and near-duplicates).
- "Templated" is a positive control: 8 templates with random name, topic and day slots, i.e. the old Internot failure.

| Metric (N=1,000 unless stated) | Enron, all | Enron, 5–400 words | Templated |
|---|---|---|---|
| Median words/email (p10–p90) | 51 (11–266) | 45 (11–179) | — |
| distinct-2 (on a fixed 20k-token budget) | 0.409 (0.606) | 0.479 (0.631) | 0.015 |
| gzip CR | 2.60 | 2.45 | 13.2 |
| Self-repetition (4-gram) | 2.95 | 2.73 | 7.09 |
| Self-BLEU-4 (150 docs) | 0.190 | 0.191 | 0.968 |
| Near-duplicate rate (3-gram Jaccard ≥ 0.5, within 500) | 4.9% | 4.5% | 99% |
| Masked 4-token opener: top-1 share / unique ratio (N=500) | 1.8% / 0.92 | — | 15.3% / 0.044 |
| POS 6-gram template rate / CR on POS tags | 0.51 / 5.56 | 0.47 (20–120 words) | 1.00 / 30.2 |
| TF-IDF Vendi / N (N=500) | 0.78 | — | 0.035 |
| MATTR (window 50) | 0.717 | 0.746 | **0.755** (misses templating) |
| LLM-cliché hits per 1k words / % emails with any hit | 0.21 / 2.6% | — | — |

**Other baselines from the same sample:**
- **Per inbox** (27 folders × 40 received emails): distinct-2 median 0.586 (p10 ≈ 0.39); CR median 2.63 (p90 ≈ 3.2); unique-opener ratio 0.95. Bulk-mail folders are outliers (CR 5.2–6.0).
- **Per sender:** human authors distinct-2 0.66–0.77, CR 2.07–2.56. Two bots: CR 14.7 and 16.3, one reusing the same opener 90% of the time.
- **Subjects** (EnronSubjects, n=176,738): 45.9% no prefix, 42.6% "Re:", 11.4% "Fw:"; median 3 words; original subjects 64.6% distinct; the top subject ("lunch") is 0.18%.

### 3.4 Pitfalls

- **Length:** distinct-n and TTR fall with length and CR rises. Compare at matched N and length strata.
- **gzip window:** DEFLATE only looks back 32 KB, so CR flattens after about 1k emails (2.415 at 100, 2.616 at 1k, 2.614 at 3k); lzma keeps rising. Fix N, the compressor and a shuffled order.
- **MATTR/TTR measure richness within a document.** They rate the templated inbox *higher* than Enron, so they're useless as sameness tests.
- **Mask entities before opener checks.** Varying names hide a repeated frame: the unmasked top-1 opener share is only 2.1% even on the templated corpus.
- **Separate person-to-person and org mail.** Bots are legitimately templated (CR 14–16).
- **Prose metrics can pass while every email is about the same thing.** Content diversity must be tested on the happenings themselves (§6.A).

---

## 4. Grounded LLM rendering

### 4.1 What goes wrong, in numbers

- **E2E NLG challenge** (Dušek et al. 2020):
  - Slot error rate (SER) = (missed + added + wrong + repeated) / slots, estimated by regex.
  - Template system 0.00%; best neural systems with reranking 1.08–1.26%; seq2seq *without* semantic control 12.5–27.9%.
  - "Attempts at diversity may hurt semantic accuracy": the most diverse system had 12.48% SER.
  - **Omission is the most common error**, and systems that invent less tend to omit more.
- **Kasner & Dušek 2024** (zero-shot LLMs):
  - At least one error in 85.6% (Llama 2), 81.2% (Mistral), 75.6% (Zephyr) and 60.6% (GPT-3.5) of outputs.
  - GPT-3.5 averages 1.39 errors per output: 0.65 contradictions and 0.49 unverifiable additions.
  - Sparse inputs get padded with invented facts. Models "commonly mention identifiers, timestamps, files".
  - GPT-4-as-judge agrees with humans at r=0.26 per word but 0.93 per domain: good in aggregate, not per output.
- **Thomson & Reiter 2020:** 15–21 factual errors per 300-word sports summary. Of 418 errors, 184 are numbers and 105 names (69% combined), exactly the classes placeholders make checkable.
- **ToTTo:** human-judged faithfulness 93.6% for references vs 73.6–76.2% for BERT-to-BERT.
- **PARENT:** in WikiBio, 62% of *references* contain information not in the table.
- **Rebuffel et al. 2021:** word-level hallucination 23.8% in gold references, 10.1% in the best-BLEU system, 1.43% with controlled decoding.
- **ConStory-Bench 2026:** 0.11–0.54 consistency errors per 10k words (DeepSeek-V3.2: 0.541), clustered 40–60% of the way through. Keep calls short and stateless.
- **Dušek et al. 2019:** cleaning training data improved semantic correctness by up to 97%. By analogy, every few-shot example must be exactly faithful to its own facts (i).

### 4.2 Placeholders and constrained output

- **Placeholders ("delexicalization"; Wen et al. 2015, SC-LSTM):** generate with slot tokens, then fill in the values. SER becomes an *exact* placeholder count instead of a regex estimate (E2E paper, footnote 29).
- **Grammar around slots (i):** supply pre-inflected variants (`{P1.first}`, `{P1.poss}`, `{D1.weekday}`, `{D1.rel}`).
- **Lexically constrained decoding** (NeuroLogic A*esque) needs the model's token probabilities, so it isn't available through a hosted API.
- **DeepSeek structured output:**
  - JSON mode guarantees valid JSON only, and "may occasionally return empty content".
  - Strict tool calls (`/beta`, `strict: true`) enforce the schema, including `pattern`.
  - A malformed-JSON-in-strict-mode report (issue #1069) was closed as stale, so parse defensively and retry.
- **Thinking mode** must be disabled, for temperature control and cost.

### 4.3 Checkers, from cheap to expensive

| Checker | Accuracy / cost | Use |
|---|---|---|
| Closed-world rules: placeholder audit, digit/date/money leaks, world gazetteer (`aho-corasick` crate), date parsing (`interim` crate) | Exact for the classes covered; microseconds | Render-time gate |
| Entity precision vs source, prec_s (Nan et al. 2021) | Human gold summaries score only 79–97%; filtered training raised a model from 93.6% to 98.2% | Offline audit |
| Two-way NLI (Dušek & Kasner 2020, roberta-large-mnli) | 0.91 accuracy on E2E; 97.8% vs 99.5% for a hand-made slot script | Offline audit; catches paraphrased facts |
| SummaC / AlignScore / MiniCheck | SummaC 74.4% balanced accuracy; MiniCheck-770M matches GPT-4 at 400× lower cost | Offline audit only |
| FActScore (split into atomic facts, check each) | Automatic estimate within 2% of human | Occasional deep audit |
| spaCy `en_core_web_trf` NER | F1 90.19 | Secondary to the world's own gazetteer |

### 4.4 Cost (DeepSeek pricing page, checked 2026-09-29)

| Model | Input, cache hit | Input, cache miss | Output |
|---|---|---|---|
| deepseek-flash, off-peak | $0.003/M | $0.15/M | $0.60/M |
| deepseek-v4-pro | $0.022/M | $0.66/M | $1.98/M |

- Peak prices are double (01–04 and 06–10 UTC, weekdays). The prefix cache is best-effort and is cleared within hours to days.
- **Per email (i):** 1,500 cached prompt + 300 fresh packet + 200 output tokens ≈ **$0.00017 per attempt** (about 5,900 per $1). At 1.3 attempts per email, **$3 renders about 13–15k emails**.

### 4.5 Keeping a fictional world consistent (synthesis)

- **Facts flow one way.** A reply's facts come from its parent's structured record, never from the parent's prose (OrgForge: prose chaining "propagates fabricated facts faithfully"). Quoted text is copied byte for byte from the cached render (i).
- **Pre-format every surface form** (weekday, relative date, currency, file name); send no raw ids or ISO timestamps.
- **Give the renderer an explicit `allowed_refs` list** plus a small filler allowance. Otherwise sparse packets get invented history.
- **The cache is the determinism mechanism.** Ship it with published seeds (i).
- **Perturbation becomes trivial:** change one slot value ("Dana Kimm", a wrong weekday) and the output is a locally plausible contradiction (coherence spec §5) (i).

---

## 5. Realistic email and message style variation

### 5.1 Length

- **Yahoo replies** (Kooti et al. WWW 2015; 16B emails, 2M users):
  - Overall: median 43 words, mean 153, mode 5.
  - Median by age: 17 words (13–19), 21 (20–35), 31 (36–50), 40 (51+).
  - By device: phone 20 words, desktop 60.
- **Chat and SMS:** NUS SMS median 36 chars / 7 words (p90 22 words); dating-app chat median 8 words, 37% contain "?".
- **Avocado reply rate by body length:** rises from 4.65% at 1 word to 9.64% at 40 words, then falls to 2.82% at 500 words.

### 5.2 Threads and reply behavior

- **Thread size:**
  - Enron (Klimt & Yang 2004): 61.6% of emails are in threads; mean thread 4.10, median 2; 55.6% of threads have 2 messages.
  - Yahoo: mean 3.76, median 2.
- **Latency (Yahoo):** mode 2 min, **median 47 min**, mean 1,157 min; over 90% within a day. Replies speed up through a thread except the slow last one. Median by age: 13 min (teens) to 47 min (51+).
- **Avocado** (279 accounts, 938k emails):
  - 53% of emails are non-dyadic. **Only 6.5–7.7% of thread-initial emails get any reply** (broadcasts included).
  - Internal vs external: reply rate 7.76% vs 2.26%; median latency 66 vs 135 min.
  - Weekend: reply rate about 4% vs about 7%; latency ×13–30.
  - 3–5 recipients get the most replies (about 10%). Latency rises from 44 to 95 min as recipients go from 1 to 8.
- **Latency shape** (Kalman et al. 2006): power law, slopes −1.74 to −2.04; Enron mean 28.8 h. No reply within 10× the mean means no reply (over 95% confidence).
- **Overload** (Kooti): the share of received mail answered drops from about 25% to under 5% at about 100/day, and replies get shorter.

### 5.3 Formality, hierarchy and politeness

- **Formality** (Peterson et al. 2011, Enron): model it as binary (annotators agree 85.7% at 2 levels, 43.5% at 4).
  - Personal emails are informal 56.0% of the time vs 21.3% for business.
  - Informality rises with contact volume: 24% (1–10 emails) to 37.5% (over 100).
  - 1 recipient → 32% informal; 3–5 recipients → 16.5%.
  - A request lowers informality (19.0% vs 28.6%).
  - By role: trader 33% informal, lawyer 7%.
- **Hierarchy:**
  - Gilbert 2012: upward phrases include "thought you would", "attach", "thoughts on", "sounds good"; downward include "let's discuss", "please send", "be sure", "fyi".
  - Prabhakaran & Rambow 2017: superiors write fewer, *shorter* messages with about 42% fewer inform acts; subordinates start more threads.
- **Politeness** (Danescu-Niculescu-Mizil et al. 2013, 10,957 requests):
  - Up: gratitude +0.87, deference +0.78, "could/would you" +0.47.
  - Down: direct start −0.43, "please" at the start −0.30.
  - Wikipedia editors become less polite after promotion to admin.

### 5.4 Greetings, sign-offs and subjects

- **Greetings** (HackerNoon/Tatemae 2026, Enron + a Git list, 56.7k messages; blog):
  - **75.1% of messages have no greeting**, 15.6% a name only, 9.3% a greeting word. Our Enron sample: 7.4% start with a greeting word.
  - Fraudulent mail uses "Dear"/"Hello" 3.3× more often, and "Best regards" closes 38.7% of it vs 3.5% of human mail. Stock LLM salutations are a realism tell (i).
- **Org culture dominates** (Waldvogel 2007, two NZ workplaces, 515 emails):
  - No greeting: 59% (educational org) vs 17% (manufacturing plant). No closing formula: 34% vs a closing formula on 75% of plant mail.
  - Status: 45% of upward messages have no greeting vs 70% of downward; 69% of upward messages sign off vs 57% of downward.
  - Distance: greeting plus name goes to 76% of distant colleagues vs 48% of close ones.
- **Sign-off effect on replies** (Boomerang 2017, vendor data, relative effects only): "thanks in advance" 65.7% replied vs 47.5% baseline and 51.2% for "best".

### 5.5 Style parameter table

Sources: W = Waldvogel, P = Peterson, K = Kooti, Y = Yang, PR = Prabhakaran & Rambow. (i) = inferred. Length is the median words of an initiating message.

| Relationship | Length | P(greeting) | P(sign-off) | P(informal) | P(reply) | Median latency |
|---|---|---|---|---|---|---|
| manager → report | 25–40 (i; shorter than upward, PR) | 0.3–1.0 by venue (W) | 0.57 (W) | 0.22–0.32 (P) | 0.6–0.8 (i) | 30–60 min (i) |
| report → manager | 40–70 (i) | 0.55–0.75 (W) | 0.69 (W) | 0.16–0.18 (P) | 0.4–0.6 (i) | 60–120 min (i) |
| peer, same team | 30–45 (K) | 0.4–0.9 by venue (W) | 0.62–0.65 (W) | 0.22, up to 0.37 for frequent pairs (P) | 0.5–0.7 (i) | 47–66 min (K, Y) |
| cross-team | 50–80 (i) | 0.5–0.9 (W) | 0.65–0.89 (W) | 0.20–0.24 (P) | 0.3–0.5 (i) | 66–135 min (i) |
| external / vendor | 80–150 (i) | 0.9 (i) | 0.9 (i) | < 0.10 (i) | 0.02–0.05 (Y) | 135 min (Y) |
| friend | email 20–40 (i); chat 7–8 words | 0.3 (i) | 0.3–0.4 (i) | ≥ 0.56 (P) | 0.7+ (i) | 16–47 min (K) |
| family | 17–40 by age (K) | 0.3 (i) | 0.3 (i) | ≥ 0.56 (P) | 0.7+ (i) | 13–47 min (K) |

**Multipliers:**
- Request: informality × 0.66 (P); latency × 1.5 (Y).
- Attachment: latency × 1.75 (K).
- Phone vs desktop: length × 0.33, latency × 0.45 (K).
- Weekend: reply rate × 0.57 (Y).
- Overloaded recipient: reply fraction falls from 25% to under 5% (K).
- 1 → 8 recipients: latency × 2.2 (Y).

Latency is power-law (Kalman); treat anything past 10× the mean as never answered.

---

## 6. Diversity and faithfulness test suite

All tests are deterministic over fixed seeds and live in `internot/tests/`. Only D runs on the cached LLM render corpus; A–C and E1–E3 need no LLM call. Thresholds are stated relative to the §3.3 baselines so they can be recalibrated.

**A. Happening-level diversity** (catches "same topic, different words")

| # | Check | Threshold |
|---|---|---|
| A1 | Normalized Shannon entropy of happening kinds touching a viewer over 30 days, across 200 viewers | median ≥ 0.7; no viewer below 0.5 (i) |
| A2 | Share of a viewer's person-to-person threads from the single most common kind | ≤ 0.35 (i) |
| A3 | Distinct (kind, object) pairs among a viewer's last 40 person-to-person threads | ≥ 0.9 of threads (i) |
| A4 | Cooldown invariant: exact (kind, object, cast) repeats within 8 weeks, excluding cadence kinds | 0 (hard) |
| A5 | Jaccard of (kind, object) multisets for two random viewers at *different* venues | ≤ 0.1 (i) |
| A6 | Share of work threads whose happening links to a parent or child in another service | ≥ 0.5 (i) |

**B. Calibration against real marginals** (the Synthea lesson)

| # | Marginal | Target band | Source |
|---|---|---|---|
| B1 | Meeting hours/week: IC; manager minus IC | IC 6–14; manager IC+4 to IC+10 | Clockwise; Flowtrace |
| B2 | Recurring share of meetings; 2-person share of one-offs | 0.40–0.56; 0.40–0.60 | Flowtrace (48%, 49%) |
| B3 | Duration mode; share over 60 min | 30 min; ≤ 0.15 | Flowtrace (45%, 12%) |
| B4 | Meetings booked less than 24 h ahead | 0.25–0.45 | Flowtrace (35%) |
| B5 | Tuesday / Friday meeting load | ≥ 1.2 | Microsoft 2025 (23% vs 16%) |
| B6 | Requests among person-to-person work emails | 0.35–0.60 | Lampert (52%) |
| B7 | Action items per meeting (meetings that have any) | mean 2–5 | AMI (3.8) |
| B8 | Thread length median; mean | 2; 3.5–4.5 | Klimt & Yang; Kooti |
| B9 | Reply latency median; mean/median ratio | 30–120 min; ≥ 5 | Kooti; Yang; Kalman |
| B10 | Task items never completed | 0.3–0.5 | iDoneThis; Reclaim |
| B11 | Medical visits/person/year; payments/month | 2–4.5; 35–60 | NAMCS; Atlanta Fed |
| B12 | Internal emails with no greeting; spread across venues | 0.45–0.85 overall; spread ≥ 0.3 | HackerNoon (75%); Waldvogel |

**C. Cross-service coherence** (true by construction, still asserted)
- C1: every entity or happening an AVM references resolves in its owning service (meeting → `get_schedule`, file → `list_drive`, task → `list_tasks`). Zero dangling references.
- C2: every rendered date or time equals its slot's source value after slots are filled.
- C3: the coherence spec's `perturb` profiles make C1/C2 fail, proving the tests can see planted contradictions.

**D. Prose diversity** (person-to-person thresholds; org mail is tested separately and only for coherence)

| # | Metric (method as in §3.3) | Pass | Enron / templated |
|---|---|---|---|
| D1 | gzip CR (N=1,000, 20–120 words); n-gram diversity | CR ≤ 2.9; NGD ≥ 1.9 | 2.45 / 13.2 |
| D2 | Self-repetition (4-gram), N=1,000 | ≤ 3.5 | 2.73 / 7.09 |
| D3 | Self-BLEU-4 (150 docs); near-duplicate rate (within 500) | ≤ 0.30; ≤ 5% | 0.19, 4.5% / 0.97, 99% |
| D4 | Masked opener top-1 share and unique ratio (N=500); same for sign-offs; per inbox of 40, unique ratio ≥ 0.85 in 90% of inboxes | ≤ 3%; ≥ 0.85 | 1.8%, 0.92 / 15.3%, 0.044 |
| D5 | POS 6-gram template rate; CR on POS tags | ≤ 0.65; ≤ 6.5 | 0.51, 5.56 / 1.0, 30.2 |
| D6 | Per inbox (40 emails): distinct-2 and CR | ≥ 0.45 and ≤ 3.3 in 90% of inboxes | median 0.586, 2.63 |
| D7 | Mean pairwise embedding cosine (fixed embedder, N=500) vs Enron under identical settings; cheap proxy TF-IDF Vendi/N | ≤ Enron + 0.05; ≥ 0.6 | Vendi/N 0.78 / 0.035 |
| D8 | LLM-tic lexicon (Kobak + EQ-Bench lists + email clichés) | ≤ 0.45 hits per 1k words; ≤ 6% of emails; no phrase above 10× its Enron rate | 0.21, 2.6% |

D thresholds are Enron plus a margin (i). D5 and D7 are where default LLM prose is most likely to fail.

**E. Faithfulness**

| # | Check | Pass |
|---|---|---|
| E1 | Placeholder SER after validation (invented slot ids; missing `must_mention`) | 0 (hard) |
| E2 | Leaks outside placeholders: digits, month/weekday names, relative-time words, currency, gazetteer names, capitalized words not on the allow-list | 0 (hard) |
| E3 | History or commitment phrases ("as discussed", "attached", "I'll send") without a matching `allowed_refs` or attachment slot | 0 (hard) |
| E4 | First-attempt validator failure rate; template-fallback rate | tracked; fallback ≤ 2% (i) |
| E5 | Offline audit, 200 renders per prompt version: entity prec_s; OrgForge-style Δ = 0.35·entity + 0.45·NLI + 0.20·numeric | prec_s = 1.0; Δ ≥ 0.98 (i; OrgForge 0.996) |
| E6 | Every few-shot example in the prompt validates against its own AVM | 100% |

---

## 7. Construction recommendations

### 7.1 The happening record

A happening is a pure function of `(kind, anchor, epoch, index)`. Nothing is stored except overlay mutations.

```rust
struct Happening {
    id: HappeningId,            // packs kind, anchor, epoch, index, child path
    kind: HappeningKind,        // §7.3; life kinds carry an ATUS code
    anchor: Anchor,             // Person(u32) | Dyad(u32,u32) | Venue(u32) | Account(u32, vendor)
    motive: Motive,             // why the initiator started it (Doran-style)
    cast: SmallVec<[(Role, u32); 6]>,   // 32-bit person refs, bound by query
    object: ObjectRef,          // deliverable / vendor / provider / file, by reconstruction
    parent: Option<HappeningId>,
    times: Times,               // created, scheduled, due: each f(id)
    labels: LabelSet,           // helped, slipped, favor_owed, conflict… for callbacks
}
fn status_at(h: &Happening, t: DateTime<Utc>) -> Status   // proposed → scheduled → done/slipped/cancelled
fn children(h: &Happening) -> impl Iterator<Item = Happening>   // deterministic spawn rules
```

**Id sketch (i):** kind:6 + anchor_tag:2 + anchor:64 + epoch:16 + index:12 + child_role:6 + child_idx:8 = 114 bits. That fits u128 with the overlay sentinel bit spare, and follows the project's 32-bit-person-ref and reference-by-reconstruction rules.

### 7.2 Three generator families, enumerated per fixed epoch

1. **Cadence-driven** (recurring meetings, bills, subscriptions, payroll, haircuts). Occurrence k is at `t0 + k·period + jitter(hash)`. Each instance can be cancelled or rescheduled (1:1s: 30% cancelled, 42% rescheduled, Reclaim).
2. **Poisson roots** (projects, requests, appointments, social plans, purchases, trips).
   - Rate λ(anchor, t) comes from role, tie strength, life stage (§1.2) and the pacing director (§7.5).
   - **Enumerate each fixed epoch independently**: ISO week for requests and social plans, month for appointments and purchases, quarter for projects.
   - Any query window then maps to a fixed set of epochs, so slices recombine. This removes the Phase-0 `enumerate_events` index-reset limitation and the `tie.since` anchor workaround.
3. **Spawned children** (`hash(parent_id, role, i)`):
   - project → kickoff, docs, 3–10 tasks, milestones, status mail;
   - meeting → Poisson(≈3.8) action items → tasks plus a follow-up email;
   - request → reply (probability from §5.5, power-law latency) plus a task with probability p;
   - appointment → booking, confirmation, reminder, calendar block, bill;
   - social plan → chat, calendar event, sometimes a money split.

**Lookback:** a view over `[t1, t2]` enumerates roots from `t1 − horizon(kind)` (e.g. 120 days for projects), so children in the window see their parents.

### 7.3 Happening catalog (first cut; rates per person)

| Kind | Anchor | Cast | Rate / cadence | Spawns → services |
|---|---|---|---|---|
| Project | Venue (team) | owner = senior member; contributors = team slice | 1–4 active per IC (i); duration lognormal, weeks | kickoff, docs (files), tasks, milestones (calendar), status mail |
| Recurring meeting (standup, sync, 1:1, all-hands) | Venue or manager dyad | venue slice; manager + report | ≈48% of meetings; 1:1 cadence 40/25/18% weekly/biweekly/monthly | calendar series; occasional notes file |
| One-off meeting (review, decision, cross-team, vendor, interview, farewell) | Project or dyad | half are 2-person | IC 8–14 total meetings/week; manager 16–25 | invite (mail; chat for close ties), notes, action items → tasks |
| Ad hoc call | Strong-tie dyad | 2 | ≈57% of meetings | chat trace only |
| Request (info, review, approval, file) | Dyad | initiator driven by project need or role | ≈52% of work emails carry one | thread → task → reply or slip |
| Approval workflow (expense, travel, purchase) | Person + org chain | employee → admin → budget owner → supervisor (BPIC) | a few per quarter (i) | notification mail, approver task, reimbursement (money) |
| Deadline / milestone | Project | owner | per project | reminder mail, status meeting, crunch in the pacing director |
| Review cycle (performance, planning) | Venue | manager + reports | 1–2/year; quarterly | calendar, docs, mail |
| Broadcast / notification | Venue or org | — | fills total inbox to about 90–120/day (i) | mail only; tested as org mail |
| Appointment (medical, dental, personal care, vet, car, contractor) | Person + provider | provider = org entity | ≈28 encounters/year; medical ≈3.2 | booking, calendar, reminder, bill |
| Bill / subscription | Account | vendor | monthly or annual; about $219/month in subscriptions | transaction, receipt mail, price-change mail |
| Purchase / order | Person | vendor | ≈47 payments/month; parcels ≤ 14/household/month | money, order/ship/deliver mail, occasional return task |
| Errand / chore | Person | — | Graus mix (go somewhere 33%, chores 24%, communicate 21%) | tasks (80% due within 24 h) |
| Social plan (dinner, party, game, concert) | Friend dyad or clique | strong ties; shared hobbies | ≈2/week (ATUS); more if young or childless | chat, calendar, money split |
| Family / care logistics | Kin cluster | parents / children | only with children (caring 1.46–2.37 h/day) | chat, calendar, forms (files), tasks |
| Trip | Person (+ companions) | — | a few per year (i) | bookings (mail), calendar, out-of-office, expense workflow if work |
| Government / civic (taxes, renewals) | Person | agency | yearly tax season; renewals every few years | tasks, mail, money |
| Life event (move, new job, marriage, baby) | Person | existing `life_events` | — | raises the rates of the kinds above for months |

**Vocabulary without hand-curated tables (i):**
- **Work objects:** O*NET Task Statements (18,796 rows keyed by O*NET-SOC, CC BY 4.0). People already carry SOC codes, so an accountant's deliverables come from accountant task statements.
- **Life-happening kinds:** ATUS codes.
- **Task verbs:** Graus's reminder verbs.
- **Approval flows:** BPIC 2020.

### 7.4 Casting: binding happenings to relationships and traits

- **Preconditions query the graph:** `ties_of(v)` filtered by kind, strength ≥ θ and venue; v's manager; co-attendees of the parent meeting.
- **Among the eligible, choose with soft weights** (Comme il Faut style), then pick by `hash(anchor, epoch, i, "cast")`:

  ```
  weight = base(kind)
         × f(tie_strength)
         × g(initiator traits)    // conscientious → more status mail, fewer slips;
                                  // extraverted → more social plans
         × h(role distance)
         × recency penalty
  ```

- **Asymmetry is allowed:** per-direction weights mean one side can initiate most exchanges (CiF directed networks).
- **Shared-object topics:** personal threads draw on the intersection or contrast of the two people's hobbies (`hobbies_of`), the CiF cultural-knowledge trick.

### 7.5 Pacing, novelty and history

- **Pacing director per person:**
  - Load `L(t) = smooth(person, t) + Σ deadline bumps`, which scales request and meeting rates.
  - A Cassandra-style minimum gap between "major" happenings (deadline, conflict, slip) for the same person.
  - Quiet phases from trips, weekends and life events.
- **Stateless novelty: a keyed-permutation shuffle bag.** The object for occurrence k of `(anchor, kind)` is `pool[perm_{hash(anchor,kind)}(k mod |pool|)]`, using a small Feistel permutation. Nothing repeats until the pool is exhausted (the Reigns "deck" as a pure function). Test A4 checks it.
- **Compounding history:** happenings carry labels. Each message packet carries up to 3 `allowed_refs` (earlier happenings on the same dyad or project, chosen by salience), so "per our call Tuesday" is *true*.
- **Sifting for scenarios:** Felt/Winnow-style patterns over the log surface storyful chains as seeds for adversarial scenarios: a manager request that conflicts with a standing rule, a double-booked commitment, an overdue follow-up chain, a favor owed and called in.

### 7.6 How services derive from the log

| Service | View of the log |
|---|---|
| mail | Communicative acts of happenings (request, reply, status, invite cover, receipt, reminder, broadcast). Thread = (happening, dyad or recipient set); message ids pack the act index. |
| calendar | Happenings with a scheduled time and attendees. Status comes from `status_at` (cancelled or rescheduled instances). |
| tasks | Action items and requests assigned to the viewer, plus errands and chores. `status_of(task, now)` = the child happening's status; about 40% never complete. |
| files | Artifacts: project docs, meeting notes, receipts and invoices, forms. Owner and share-set come from the cast. |
| money | Purchases, bills, subscriptions, reimbursements, splits. Amounts are `f(id)`, rendered via slots. |
| chat | Short coordination for strong ties: social plans, ad hoc calls, quick requests. Mode rule as in the 2026-05-14 notes §2. |

Every AVM field traces back to a happening field, which is what makes test C1 checkable.

### 7.7 Rendering pipeline (cheap, faithful, varied)

```
render(act) -> Rendered:
  key = (ns, prompt_version, model_version, act.id)
  if cache.hit(key): return cache[key]
  packet = build_packet(act)                 // slots with pre-formatted variants,
                                             // allowed_refs, must_mention, style
  for attempt in 0..3:
      out = deepseek_flash(system=STATIC_PROMPT,   // cached prefix: rules, style guide,
                           user=packet_json,       // rotated few-shot examples
                           thinking=off,
                           temperature=0.7 + 0.3*hash(act.id, attempt),
                           response=json{subject, body})   // placeholders only
      errs = validate(out, packet)          // E1-E3: slot audit, leak regexes,
                                            // gazetteer, history-phrase gate
      if errs.empty(): break
      packet.feedback = errs
  text = if errs.empty() { fill_slots(out, packet) } else { template_render(packet) }
  assert gazetteer_scan(text) ⊆ packet.entities    // second line of defence
  cache.put(key, {placeholder_text, slot_map, text, fallback: !errs.empty()})
```

**Where variety comes from** (all hash-derived, so free):
- **Per-person idiolect, fixed for life:** greeting habit, sign-off form ("Best, D" / name only / none) at §5.4–5.5 rates, signature block, verbosity from Big Five, capitalization and typo propensity, emoji use in chat.
- **Per-venue culture knob:** greeting and closing rates (Waldvogel: 17% vs 59% bare).
- **Per message:** length band from §5.5 × device × request multipliers; which optional facts to mention and in what order; a rotated few-shot pool keyed by register.
- **An explicit list of phrases to avoid:** the D8 lexicon plus stock LLM salutations.

**Replies:** the prior message is passed as a structured act plus its cached text. Quoted blocks are copied byte for byte.

**Optional (i):** render a short thread in one call for consistent voices, still validating each message against its own act.

### 7.8 Budget (i)

At about $0.00022 per delivered email:
- Main coherence run: 30 seeds × 4 scenarios × about 50 visible messages ≈ 6k renders ≈ $1.30.
- D-suite corpus: about 1.5k renders ≈ $0.35.
- Prompt iteration: 5 versions × 200 audit renders ≈ $0.25.
- **Total under $3.**

Guards: render lazily (only what views return), run off-peak, put the static prompt first, keep thinking off, and enforce a hard spend cap in the harness (coherence spec §10).

### 7.9 Build order (fits the rebuild phases)

1. **`internot/src/happenings/`** (not substrate, per the frozen-substrate rule): the record, id packing, the three generator families, epoch enumeration. Tests A1–A6 and B1–B12 run on the log alone.
2. **Services** (rebuild Phases B–E) become views of the log. Add C1–C2 to each round-trip test.
3. **Renderer:**
   - packet builder, validator (E1–E3), template fallback;
   - a DeepSeek client via the existing OpenAI-compatible `OpenAiClient`;
   - the D-suite and E4–E6 on a 1.5k-render corpus.
4. **Sifting queries** for scenario seeding, wired to the perturbation layer (coherence spec §5) through slot edits.

---

## Sources

**Time use, work, life admin**
- BLS ATUS 2025: https://www.bls.gov/news.release/atus.nr0.htm · https://www.bls.gov/news.release/atus.t01.htm · https://www.bls.gov/news.release/atus.t03.htm · https://www.bls.gov/news.release/atus.t04.htm · https://www.bls.gov/news.release/atus.t08a.htm
- ATUS 2024 lexicon: https://www.bls.gov/tus/lexicons/lexiconnoex2024.pdf · https://www.bls.gov/tus/lexicons.htm
- Microsoft WTI 2025 and 2023: https://www.microsoft.com/en-us/worklab/work-trend-index/breaking-down-infinite-workday · https://www.microsoft.com/en-us/worklab/work-trend-index/will-ai-fix-work
- Mark et al. 2016: https://www.microsoft.com/en-us/research/publication/email-duration-batching-and-self-interruption-patterns-of-email-use-on-productivity-and-stress/
- McKinsey 2012: https://www.mckinsey.com/industries/technology-media-and-telecommunications/our-insights/the-social-economy
- Radicati press release (no per-user figure): https://www.einpresswire.com/article/751597875/the-radicati-group-releases-email-statistics-report-2024-2028
- Flowtrace: https://www.flowtrace.co/collaboration-blog/50-meeting-statistics · https://www.flowtrace.co/collaboration-blog/state-of-meetings-report
- Clockwise via Computerworld: https://www.computerworld.com/article/1612747/for-developers-too-many-meetings-too-little-focus-time.html
- Reclaim: https://reclaim.ai/blog/productivity-report-one-on-one-meetings · https://reclaim.ai/blog/task-management-trends-report
- Perlow et al. HBR 2017: https://hbr.org/2017/07/stop-the-meeting-madness
- Graus et al. 2016: https://www.microsoft.com/en-us/research/wp-content/uploads/2017/01/umap-2016-graus-et-al.pdf
- MS-LaTTE: https://ar5iv.labs.arxiv.org/html/2111.06902
- iDoneThis via HuffPost: https://www.huffpost.com/entry/forty-one-percent-of-tasks-on-to-do-lists-are-never-done_b_9308978
- Lampert et al. 2010: https://aclanthology.org/N10-1142.pdf
- Yang et al. 2017 (Avocado): https://www.microsoft.com/en-us/research/wp-content/uploads/2017/04/sigir17a.pdf
- AMI action items (via Liu et al. 2023): https://arxiv.org/pdf/2303.16763
- BPI Challenge 2020: https://www.tf-pm.org/competitions-awards/bpi-challenge/2020 · https://figshare.com/articles/dataset/BPI_Challenge_2020_Domestic_Declarations/12692543/1
- O*NET Task Statements: https://www.onetcenter.org/dictionary/29.0/excel/task_statements.html
- CDC FastStats: https://www.cdc.gov/nchs/fastats/physician-visits.htm · https://www.cdc.gov/nchs/fastats/dental.htm
- Atlanta Fed payments diary: https://www.atlantafed.org/banking-and-payments/consumer-payments/survey-and-diary-of-consumer-payment-choice
- C+R subscriptions: https://www.crresearch.com/blog/subscription-service-statistics-and-costs/
- Pitney Bowes: https://www.pitneybowes.com/us/shipping-index.html
- Census e-commerce: https://www.census.gov/retail/ecommerce.html

**Narrative generation and simulation**
- Compton, "So you want to build a generator": https://galaxykate0.tumblr.com/post/139774965871/so-you-want-to-build-a-generator
- Tracery: http://www.galaxykate.com/pdfs/ComptonKybartasMateas15-Tracery%20An%20Author-Focused%20Generative%20Text%20Tool.pdf
- Emily Short: https://emshort.blog/2016/04/12/beyond-branching-quality-based-and-salience-based-narrative-structures/ · https://emshort.blog/2019/11/29/storylets-you-want-them/ · https://emshort.blog/2019/05/21/curating-simulated-storyworlds-james-ryan/
- Kreminski & Wardrip-Fruin: https://mkremins.github.io/publications/Storylets_SketchingAMap.pdf
- Felt: https://github.com/mkremins/felt · Winnow: https://mkremins.github.io/publications/Winnow_AIIDE2021.pdf
- Façade: https://cdn.aaai.org/AIIDE/2005/AIIDE05-016.pdf · IPOCL: https://arxiv.org/abs/1401.3841 · Versu: https://www.cs.uky.edu/~sgware/reading/papers/evans2014versu.pdf
- Comme il Faut: https://www.gameaipro.com/GameAIPro/GameAIPro_Chapter43_An_Architecture_for_Character-Rich_Social_Simulation.pdf
- Talk of the Town: https://www.gameaipro.com/GameAIPro3/GameAIPro3_Chapter37_Simulating_Character_Knowledge_Phenomena_in_Talk_of_the_Town.pdf
- Ryan dissertation: https://escholarship.org/uc/item/1340j5h2 · Tarn Adams Q&A: https://www.gamedeveloper.com/design/q-a-dissecting-the-development-of-i-dwarf-fortress-i-with-creator-tarn-adams
- Doran & Parberry: https://www.pcgworkshop.com/archive/doran2011prototype.pdf · radiant quests: https://www.forbes.com/sites/insertcoin/2016/01/11/how-to-fix-fallout-4s-maddening-never-ending-radiant-quests/
- RimWorld: https://rimworldwiki.com/wiki/Cassandra_Classic · https://rimworldwiki.com/wiki/AI_Storytellers
- Nemesis system: https://www.gamedeveloper.com/design/core-system-analysis-of-middle-earth-shadow-of-war · Wildermyth: https://turnbasedlovers.com/10-turns-interview/with-wildermyth-developer/
- Generative Agents: https://arxiv.org/abs/2304.03442 · 1,000 people: https://arxiv.org/abs/2411.10109
- Padmakumar & He: https://arxiv.org/abs/2309.05196 · Doshi & Hauser: https://www.science.org/doi/10.1126/sciadv.adn5290 · Echoes in AI: https://arxiv.org/abs/2501.00273
- PersonaHub: https://arxiv.org/abs/2406.20094 · Verbalized Sampling: https://arxiv.org/abs/2510.01171

**Synthetic worlds**
- OrgForge: https://arxiv.org/html/2603.14997 · ARE/Gaia2: https://arxiv.org/html/2509.17158 · AgentMercury: https://arxiv.org/abs/2608.20634 · ESL-Bench: https://arxiv.org/abs/2604.02834
- Synthea: https://academic.oup.com/jamia/article/25/3/230/4098271 · https://github.com/synthetichealth/synthea/wiki/Generic-Module-Framework

**Diversity metrics**
- Shaib et al.: https://arxiv.org/html/2403.00553 · https://arxiv.org/html/2407.00211 · https://github.com/cshaib/diversity
- distinct-n: https://aclanthology.org/N16-1014/ · EAD: https://arxiv.org/abs/2202.13587 · Texygen: https://arxiv.org/abs/1802.01886
- Vendi: https://arxiv.org/abs/2210.02410 · Tevet & Berant: https://arxiv.org/abs/2004.02990 · MAUVE: https://github.com/krishnap25/mauve
- Antislop: https://arxiv.org/abs/2510.15061 · EQ-Bench slop: https://eqbench.com/slop-score.html · Kobak et al.: https://arxiv.org/html/2406.07016v1
- Artificial Hivemind: https://arxiv.org/html/2510.22954 · BARE: https://arxiv.org/pdf/2502.01697 · Kirk et al.: https://arxiv.org/abs/2310.06452 · NoveltyBench: https://arxiv.org/abs/2504.05228
- RFC 1951: https://www.rfc-editor.org/rfc/rfc1951 · Enron data: https://huggingface.co/datasets/corbt/enron-emails

**Grounded rendering**
- E2E: https://arxiv.org/pdf/1901.07931 · Semantic noise: https://arxiv.org/abs/1911.03905 · NLI accuracy: https://aclanthology.org/2020.inlg-1.19.pdf
- WebNLG+ 2020: https://aclanthology.org/2020.webnlg-1.7.pdf · ToTTo: https://arxiv.org/pdf/2004.14373 · PARENT: https://aclanthology.org/P19-1483.pdf
- Rebuffel et al.: https://arxiv.org/pdf/2102.02810 · Thomson & Reiter: https://arxiv.org/html/2011.03992 · Puduppully et al.: https://arxiv.org/abs/1809.00582 · Wen et al.: https://arxiv.org/abs/1508.01745
- Kasner & Dušek: https://arxiv.org/html/2401.10186 · ConStory-Bench: https://arxiv.org/html/2603.05890v1
- SummaC: https://arxiv.org/abs/2111.09525 · AlignScore: https://arxiv.org/abs/2305.16739 · MiniCheck: https://arxiv.org/abs/2404.10774 · FActScore: https://arxiv.org/abs/2305.14251
- Nan et al.: https://aclanthology.org/2021.eacl-main.235.pdf · spaCy trf: https://huggingface.co/spacy/en_core_web_trf · NeuroLogic A*esque: https://arxiv.org/abs/2112.08726
- Rust crates: https://lib.rs/crates/interim · https://lib.rs/crates/aho-corasick
- DeepSeek: https://api-docs.deepseek.com/quick_start/pricing · https://api-docs.deepseek.com/guides/kv_cache · https://api-docs.deepseek.com/guides/json_mode · https://api-docs.deepseek.com/guides/tool_calls/ · https://api-docs.deepseek.com/guides/thinking_mode · https://github.com/deepseek-ai/DeepSeek-V3/issues/1069

**Email and message style**
- Kooti et al.: https://arxiv.org/abs/1504.00704 · Klimt & Yang: https://www.ceas.cc/papers-2004/168.pdf · Kalman et al.: https://academic.oup.com/jcmc/article/12/1/1-23/4582956
- Peterson et al.: https://aclanthology.org/W11-0711/ · Gilbert: http://eegilbert.org/papers/cscw12.hierarchy.gilbert.pdf
- Prabhakaran & Rambow: https://arxiv.org/abs/1706.03441 · https://aclanthology.org/P14-2056/
- Danescu-Niculescu-Mizil et al.: https://arxiv.org/abs/1306.6078 · Waldvogel: https://nl.ijs.si/janes/wp-content/uploads/2014/09/waldvogel07.pdf
- HackerNoon (blog): https://hackernoon.com/three-quarters-of-email-begins-with-nothing · https://hackernoon.com/nobody-actually-writes-best-regards
- Boomerang (vendor): https://blog.boomerangapp.com/2017/01/how-to-end-an-email-email-sign-offs/
- NUS SMS: https://github.com/WING-NUS/nus-sms-corpus · Zhang & Yasseri: https://arxiv.org/abs/1607.03320 · EnronSent: https://wstyler.ucsd.edu/enronsent/
