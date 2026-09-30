# Real social graph structure — what the empirical record says, and what Internot should build

**Date:** 2026-05-14
**Question:** Should each Internot person have **one** social graph with kind-labeled, weighted edges (family / friend / coworker / neighbor / classmate ...), or **multiple** disjoint layers (multiplex)? The choice is architectural and load-bearing — every downstream service (mail, calendar, chat, contacts UI, scenario authoring) reads through it.

This document surveys the empirical and theoretical record, then commits to an answer.

---

## 1. Dunbar's hierarchy — the layered concentric structure

The classical claim, due to Robin Dunbar and collaborators, is that human social networks have a **concentric, scale-invariant layered structure**, with cumulative-inclusive layer sizes roughly:

- 5 (intimate / support clique)
- 15 (sympathy group)
- 50 (band / "good friends")
- 150 (community / "casual friends" — *the* Dunbar number)
- 500 (acquaintances)
- 1500 (faces with names you can recognize)

Each layer is roughly 3× the size of the one inside it, and emotional closeness / contact frequency / kinship density all drop monotonically as you move outward. Hill & Dunbar (2003) established the inner-layer sizes from Christmas-card-list data and other small-scale studies; subsequent reviews extended the hierarchy to 500/1500.

The most compelling large-N empirical replication is **Mac Carron, Kaski & Dunbar (2016) "Calling Dunbar's numbers"** [[arxiv]](https://arxiv.org/abs/1604.02400) [[ScienceDirect]](https://www.sciencedirect.com/science/article/pii/S0378873316301095). They analysed 6 billion calls made by 35 million European mobile-phone subscribers in 2007, filtered to egos with ~100 reciprocated contacts, and clustered alters by call frequency. The cluster means came in at **4.1, 11.0, 29.8, 128.9** — strikingly close to the predicted 5 / 15 / 50 / 150, with strong evidence for inner and outer layers and high variability in the middle. Roberts, Dunbar, Pollet & Kuppens (2009) "Exploring variation in active network size" confirmed an absolute upper bound on total network size and a hard inverse relationship between mean emotional closeness and size — the cognitive-load constraint that motivates the hierarchy in the first place.

**The 2021 challenge.** Lindenfors, Wartel & Lind (2021) "'Dunbar's number' deconstructed" (Biology Letters) [[link]](https://royalsocietypublishing.org/doi/10.1098/rsbl.2021.0158) re-ran the neocortex-ratio regression with modern Bayesian and phylogenetic least-squares methods on the original primate dataset and got point estimates of 69–109 and 16–42 with 95% CIs of 4–520 and 2–336. Their conclusion: the theoretical scaffolding (extrapolating from primate neocortex ratios to human group size) is too noisy to pin down any single number. **Crucially, Lindenfors et al. do not deny the layered structure** — they question whether 150 specifically is privileged. The empirical layered structure observed in Mac Carron 2016 and Saramäki et al. 2014 stands.

**Saramäki et al. (2014) "Persistence of social signatures"** (PNAS) [[link]](https://www.pnas.org/doi/10.1073/pnas.1308540110) is independently important: tracking 30 students across an 18-month school-to-university transition with combined mobile-phone + survey data, they showed that **the *shape* of an ego's communication distribution across alters is individually distinctive and persistent even as the *identity* of alters turns over**. Some egos are "broadcasters" who spread effort thinly across many alters; others are "concentrators" who pour most communication into 3–5. The distribution is a personal fingerprint.

**Implication for Internot.** Each procedural person needs a stable per-ego "social signature" (one or two scalar parameters that fix where on the broadcaster-concentrator axis they fall, plus the implied tier sizes). The layered structure is *not* optional — it's the dominant first-order property of real ego networks and the only thing that gives later services (inbox prioritisation, calendar conflict resolution, chat affinity) a coherent gradient to read.

---

## 2. Ego network composition by tie type

What fills those layers? Marsden's 1987 GSS "Core Discussion Networks of Americans" [[psycnet]](https://psycnet.apa.org/record/1988-13825-001) is the canonical baseline: mean discussion-network size of 3 confidants, **more than half of alters are kin**, networks are dense (22% have all-pairs-close alters) and homogeneous (<10% have racial diversity).

McPherson, Smith-Lovin & Brashears (2006) "Social Isolation in America" re-ran the GSS 19 years later [[Sage]](https://journals.sagepub.com/doi/10.1177/000312240607100301) and reported:

- Mean discussion-network size dropped from 2.94 (1985) → 2.08 (2004).
- Kin share of confidants **rose from 49% to 54%**.
- Non-kin ties (voluntary associations, neighbours) decayed faster than kin ties.
- Modal respondent in 2004: zero confidants.

Fischer's "To Dwell Among Friends" (1982) [[UChicago Press]](https://press.uchicago.edu/ucp/books/book/chicago/T/bo5962418.html) introduced the egocentric methodology that Marsden later scaled, and decomposed network composition into kin / neighbour / coworker / organisation-member / friend. Fischer found composition varies sharply with urbanity, education, and life-stage — students are friend-heavy, parents are kin-and-neighbour heavy, retirees are kin-heavy.

Wellman's East York studies (1979 onwards) and Wellman & Wortley (1990) "Different Strokes from Different Folks" [[U Chicago]](https://www.journals.uchicago.edu/doi/10.1086/229572) push further on a key point: **support is specialised by tie type**, not by tie strength alone. Strong ties give emotional aid and small services; accessible ties (neighbours, coworkers) give goods and services; immediate kin (parents, adult children, siblings) dominate large-services and financial aid. Wellman's slogan: "most intimate ties are kin and friends; most routinely-seen ties are neighbours and coworkers."

Bidart, Degenne & Grossetti (long-running French panel, summarised in their 2018 work [[Springer]](https://link.springer.com/article/10.1007/s11205-015-0987-5)) added a meeting-context decomposition: **family is the origin of ~44% of relationships, work ~10%, friends-of-friends ~14%, school/leisure the remainder**. Critically, they observe that "when someone becomes a friend or a relative, the original context of the relation is forgotten" — *meeting context is a strong predictor at tie genesis but decays as a description once the tie matures*. That has direct implications for whether layers are static or dynamic.

**Quantitative anchors to encode for Internot:**

| Bucket | Share of strong ties (US, 2004 GSS) |
|---|---|
| Kin (parents, siblings, adult children, spouse) | ~54% |
| Coworkers | ~10% (origin); much less in *current* close-tie descriptions |
| Neighbours | <10% in confidant networks |
| Friends (no other label) | ~25–30% |
| Other (church, voluntary org, ...) | balance |

Life-stage modifiers are large: students push the friend share up to ~50%+; new parents push kin up; retirees push kin further up and shed coworker ties almost entirely.

---

## 3. Multiplex vs monoplex — the architectural question

The literature distinguishes:

- **Monoplex network:** a single graph G = (V, E). Each edge may carry attributes (weight, type, timestamp).
- **Multiplex network:** a set of graphs {G_1, G_2, …, G_k} over a shared vertex set V, where each G_α = (V, E_α) corresponds to a *layer* (friendship, kinship, coauthorship, calls, ...). Edges in different layers are formally distinct objects.
- **General multilayer network:** generalisation where layers can themselves have structure (time slices, hierarchical aspects) and edges can cross layers (interlayer edges).

The canonical formalisms are:

- **Kivelä et al. (2014) "Multilayer networks"** in *Journal of Complex Networks* [[arxiv]](https://arxiv.org/abs/1309.7233) [[pdf]](https://cosnet.bifi.es/wp-content/uploads/2014/11/jcomplexnetw-2014-Kivel%C3%A4-203-71.pdf) — the definitive review and translation dictionary across the zoo of related concepts (multiplex, multirelational, interconnected, network of networks).
- **Boccaletti et al. (2014) "The structure and dynamics of multilayer networks"** in *Physics Reports* [[arxiv]](https://arxiv.org/abs/1407.0742) — companion review emphasising structural measures (overlap, edge entropy, multidegree) and dynamics on multilayer substrates.

### How much edge overlap is there between layers?

This is the load-bearing empirical question. If overlap is near-zero, multiplex is the natural encoding (the layers really are different relationships). If overlap is high, monoplex with type labels is the natural encoding (one graph, edges happen to carry multiple labels).

The empirical answer is **somewhere in the middle, and the position varies by domain**:

- **Szell, Lambiotte & Thurner (2010) "Multirelational organization of large-scale social networks in an online world"** (PNAS) [[link]](https://www.pnas.org/doi/10.1073/pnas.1004008107) [[arxiv]](https://arxiv.org/abs/1003.5137) measured six interaction types (friendship, communication, trade, enmity, aggression, punishment) among 300,000 players of the MMO *Pardus*. They found **substantial but partial overlap** — positive layers (friendship/communication/trade) overlap strongly with each other but only weakly with negative layers (enmity/aggression/punishment). The community structure detected on each layer differs, and stacking layers reveals overlapping communities that no single layer alone surfaces. This is the cleanest empirical case for multilayer formalism in social systems.

- **Workplace multiplex studies** (Methot et al. 2016 "Are Workplace Friendships a Mixed Blessing?" *Personnel Psychology* [[Wiley]](https://onlinelibrary.wiley.com/doi/abs/10.1111/peps.12109)) collect *advice* + *friendship* edges separately and treat their intersection as "multiplex". In a 168-employee sample, the share of ties that are simultaneously advice AND friendship is non-trivial but minority — most advice ties are not friendships, most friendships are not the agent's primary advice contact. Strength of tie correlates with multiplexity (Verbrugge 1979 [[ScienceDirect]](https://www.sciencedirect.com/science/article/abs/pii/S0378873321000587)).

- **Bidart et al.** on French panel data: only about half of respondents report viewing any given tie as multiplex at the descriptive level — spouses are called "friends" only when they're also work or activity partners.

- **Family-of-origin and workplace overlap** is structurally near-zero for most respondents (you don't generally work with your parents), but **friend / coworker overlap** is high in early career and **friend / neighbour overlap** is high in stable residential areas.

**Synthesising:** real social systems are multiplex in the formal sense (multiple distinguishable relation types exist), but most *individual edges* carry only one or two labels, not all of them. The layers are not orthogonal random graphs over a shared vertex set — they share enough that a node's community position is consistent across layers (you tend to socialise with people you share other contexts with). This is exactly what Feld's 1981 focus theory predicts: shared foci (school, workplace, family of origin, neighbourhood) generate ties that *are* labelled by the focus.

### The implementation tradeoff

Two extreme designs:

**A. Strict multiplex.** N parallel graphs over the procedural population, one per relation type (kin, classmate, coworker, neighbour, friend, romantic, online-acquaintance, ...). Each graph has its own degree distribution and clustering coefficient. Cross-layer overlap emerges as a measured statistic.

- *Pros:* clean conceptual mapping; supports layer-specific algorithms (Louvain on the friendship layer); future-proof for layers we haven't invented yet.
- *Cons:* per-edge storage cost; "is X my friend?" requires checking every layer; tracking that "Alice is my coworker AND friend" requires either explicit overlap edges or post-hoc intersection at query time; bit-packing layouts must encode which layer an edge lives in, which is fine for one and miserable for N.

**B. Monoplex with typed/weighted edges.** One graph over the procedural population. Each edge carries a bitmask of relation kinds (kin / classmate / coworker / neighbour / friend / ...) and a tie-strength scalar. "Is X my friend?" is a single edge lookup + bitmask test.

- *Pros:* one degree distribution to procedurally generate; multiplexity emerges as the bitmask popcount; queries are direct; Internot's bit-packing discipline maps naturally onto an edge record.
- *Cons:* if two layers have wildly different statistical properties (e.g. kinship is a forest, friendship is a power-law graph), forcing them through one degree distribution loses fidelity; some pairs may have a relationship in *one* dimension that's effectively independent of another (an internet acquaintance you trade with but have never met) and storing "no edge" vs "edge with only one bit set" is the same operation, but the *probability of an edge existing at all* differs by layer.

The empirical literature, charitably read, supports a **hybrid**:

- **A small number of fundamentally different generative processes** (kinship is a forest growing from procedural marriage/parentage events; coworker is determined by shared workplace_seed; classmate by shared school cohort; neighbour by shared city + spatial proximity; friend is the only *elective* layer overlaid on top).
- **A single per-person edge list** that materialises whatever induced edges those processes produce, with a kind-mask bitfield per edge.

This is essentially "monoplex with typed edges, but the typed edges are *derived* from a small number of contextual foci", which is also how the Stage Manager paradigm already handles cross-service coherence in Internot.

---

## 4. Homophily

McPherson, Smith-Lovin & Cook (2001) "Birds of a Feather" *Annual Review of Sociology* [[link]](https://www.annualreviews.org/content/journals/10.1146/annurev.soc.27.1.415) is the canonical review. Key claims:

- Homophily structures **every** type of tie measured: marriage, friendship, work, advice, support, information transfer, exchange, co-membership.
- Strength ordering: **race/ethnicity > age > religion > education > occupation > gender**. Race is by far the strongest divider in US data; gender is by far the weakest (mixed-gender networks are common except in very intimate ties).
- Two mechanisms, both real:
  - **Choice homophily** — the agent's preference for similar alters.
  - **Induced homophily** — homogeneity of the foci themselves (schools sort by neighbourhood + class; workplaces sort by occupation + education; voluntary associations by interest). Feld (1981) "The Focused Organization of Social Ties" [[link]](https://www.jstor.org/stable/2778746) is the foundational theory; **induced typically dominates choice empirically** (you can only befriend people you meet, and the foci do most of the sorting before choice gets a vote).

**Implication for Internot.** Procedural friend ties between persons A and B should be sampled with a probability that depends on (a) shared focus (workplace_seed equality, city equality, schoolmate cohort overlap) and (b) feature similarity (age, education_level, industry, archetype) — both are present in the procedural floor, and both should weight the friend-edge probability. This is the `edge::geometric` / `edge::cosine` / `edge::block` machinery in `procedural_core` doing exactly its job.

---

## 5. Transitivity, clustering, weak ties

Granovetter (1973) "The Strength of Weak Ties" [[pdf]](https://www.cs.cmu.edu/~jure/pub/papers/granovetter73ties.pdf) is the most cited sociology paper of the 20th century. Two propositions:

1. **The triangle hypothesis.** If A has strong ties to B and C, then B-C is likely to exist (transitivity / triadic closure). The "forbidden triad" (strong A-B, strong A-C, no B-C) is rare.
2. **The bridge hypothesis.** A tie that *bridges* between otherwise-disconnected groups must be weak. Therefore weak ties carry the novel information and connect the small-world structure.

Onnela et al. (2007) "Structure and tie strengths in mobile communication networks" (PNAS) [[link]](https://www.pnas.org/doi/10.1073/pnas.0610245104) confirmed this on a 7M-node phone network with stunning empirical clarity: **the higher the tie's call-volume strength, the higher the neighbourhood overlap of its endpoints**, and **removing weak ties (low-volume edges) causes a phase transition that disconnects the network, while removing strong ties leaves the giant component intact**. The result is exact what Granovetter predicted, three decades after the prediction.

Burt (1992) "Structural Holes" generalises: the *position* of a tie matters more than its strength. A tie that spans a structural hole between two otherwise-disconnected clusters yields brokerage advantage. Not all weak ties bridge structural holes, but bridging ties tend to be weak.

**Clustering coefficients in real social networks** are characteristically high (0.1–0.5) versus 1/N for an Erdős–Rényi random graph at the same density. Mislove et al. (2007) "Measurement and Analysis of Online Social Networks" [[pdf]](https://mislove.org/publications/SocialNetworks-IMC.pdf) measured LiveJournal at ~5M nodes, 77M edges, mean degree 17, clustering coefficient **0.3**, characteristic path length 5.9. Facebook (McAuley & Leskovec 2012 [[arxiv]](https://arxiv.org/abs/1210.8182)) ego networks have local clustering coefficients typically in the 0.2–0.6 range.

**Implication for Internot.** Friend-tie generation must induce triadic closure substantially above random. The natural way to get this for free is to draw friend ties from shared foci — two coworkers in the same workplace_seed who are both connected to a third coworker are likely to be mutually connected because the focus already connected them. We don't need to explicitly model triadic closure if the foci do it for us. Cross-context friend ties (the "weak" / "bridge" ties) need a separate generative pathway with intentionally lower density and higher path-length impact — `edge::block` with low p_out is the natural primitive.

---

## 6. Community structure

Blondel et al. (2008) Louvain [[pdf]](https://perso.uclouvain.be/vincent.blondel/research/louvain.html) and Traag, Waltman & van Eck (2019) Leiden [[Nature]](https://www.nature.com/articles/s41598-019-41695-z) are the workhorse algorithms. Modularity Q ∈ [-1, 1] measures edge density inside detected communities vs. a degree-preserving null. Real social networks score Q ≈ 0.3–0.7; random graphs score near 0.

**In real social data, detected communities map remarkably well onto foci.** Workplaces, schools, sports clubs, churches, families, and dorms all light up as Louvain communities. Yang & Leskovec (2012) "Defining and Evaluating Network Communities based on Ground-truth" [[arxiv]](https://arxiv.org/abs/1205.6233) used self-reported group memberships (Facebook groups, LiveJournal communities, Friendster interest groups) as ground truth and showed Louvain/Leiden recover them reasonably — and notably, that **community overlaps occur most often at community *cores*, not peripheries**, the opposite of the textbook assumption.

**Are families "communities" in the network sense?** Yes, with caveats:

- A nuclear family + grandparents is a small, dense clique. It will detect as a community.
- An extended family includes "cousins of cousins" that are not connected to each other — the boundary is soft.
- Families overlap with multiple other communities (the spouse is also a coworker / fellow churchgoer / neighbour) — so the family-as-community is rarely *exclusive*.

For Internot: a family produced by procedural marriage/parentage events naturally satisfies the structural definition of a community. We don't need to label it as such; we need to make sure family-tie density is high enough that any community-detection invocation by an agent finds it.

---

## 7. Real dataset values worth pinning

These are the empirical anchors a synthetic generator should hit within an order of magnitude. Internot will not directly use any of these graphs; it should produce structurally similar artifacts.

| Dataset | N | M | <k> | C (clustering) | <ℓ> (path length) |
|---|---|---|---|---|---|
| Facebook ego networks (McAuley & Leskovec 2012) | 4039 (sample) | 88,234 | ~44 | ~0.6 (egocentric) | ~3.7 |
| LiveJournal (Mislove 2007) | 5.2M | 77M | 17 | 0.3 | 5.9 |
| Pokec (Mislove via SNAP) | 1.6M | 30M | ~19 | ~0.1 | ~5 |
| Friendster (Yang & Leskovec) | 65M | 1.8B | ~55 | ~0.16 | ~5.6 |
| Onnela 2007 phone | ~7M | ~20M | ~3.3 (recip.) | ~0.2 | ~10 |
| Banerjee Indian villages | ~250/village × 43 | varies | ~9 | ~0.3 | ~3 |

The patterns: clustering 0.1–0.6, path length 3–10, mean degree 3–55 depending on whether the data captures intimate ties (low <k>) or weak-acquaintance ties (high <k>). All exhibit heavy-tailed degree distributions, all have giant components, all are small-world.

The Banerjee et al. (2013) "The Diffusion of Microfinance" (Science) [[link]](https://www.science.org/doi/10.1126/science.1236498) dataset is particularly valuable because it is **multiplex by design** — surveyors collected separate networks for borrow-rice, borrow-kerosene, give-advice, lend-money, social-visit, family etc. across 43 villages. Cross-layer overlap is partial and asymmetric: family edges are dense and overlap moderately with social-visit; advice and money edges overlap strongly with each other but only modestly with family. The Banerjee data is arguably the cleanest published illustration that **real social networks *are* multiplex but the layers are *correlated*, not independent**.

---

## 8. Lifecycle of a tie

Roberts & Dunbar (2011) "The costs of family and friends: an 18-month longitudinal study" [[ScienceDirect]](https://www.sciencedirect.com/science/article/abs/pii/S1090513810000966) tracked relationship decay across the school-to-university transition. Findings:

- Kin ties decay slowly and survive geographic disruption. Friendship ties decay rapidly without active maintenance (≥ monthly communication for emotional closeness to hold).
- Feeling psychologically close at time t1 is *not* sufficient to prevent decay by t2 (~18 months) in the absence of communication.
- Cost-of-maintenance ordering: emotional aid > information sharing > co-presence. The first decays fastest.

Saramäki et al. (2014) — already discussed — showed that the *shape* of the personal effort distribution persists even as the *identity* of alters in the lower tiers rotates. The 5-closest set is more stable than the 15-set, which is more stable than the 50-set, etc.

**Implication for Internot.** Tie strength should be a function of `(last_contact, contact_frequency_band, kinship_bit)`, with kinship buffering decay heavily. The procedural floor can recover this by computing a per-(ego, alter) "freshness" score from communication history (which the mail/chat services already store in the procedural session overlay).

---

## 9. Putting it together — the recommendation for Internot

**Recommendation: one monoplex graph over the procedural population, with kind-mask + strength on each edge, where the edge set is *induced* by a small set of procedural foci plus one elective overlay.**

### The architecture

Each person has a procedural ego network derived as follows:

1. **Family edges** (kind = `KIN`). Derived from marriage and parentage events in the existing life-events timeline. Already in the substrate — these are deterministic. Family forms a forest plus marriage edges; clustering is high inside nuclear units and lower across degrees of kinship. No new layer needed: walk the family graph procedurally.

2. **Workplace edges** (kind = `COWORKER`). For each (industry, city, workplace_seed) triple, enumerate the workforce via `Space::find()` pushdown — anyone sharing those three bit fields is a coworker. This is **already free** because the slot layout is already designed for it. No materialised edge needed; the membership is a query.

3. **School / education edges** (kind = `CLASSMATE`). Cohort-based: people with the same (graduation_year, university_seed, field_of_study) at university-grad level are classmates. Again derivable from existing person attributes.

4. **Neighbour edges** (kind = `NEIGHBOUR`). Same (city, neighborhood_seed) — currently not in the layout but a 4-bit subdivide of `city` would suffice when needed. Gate this on a scenario.

5. **Elective friend edges** (kind = `FRIEND`). This is the only layer that needs explicit edge generation. Friend ties form preferentially between people who share at least one focus (induced homophily via Feld), with probability modulated by feature similarity (`edge::cosine` over personality dimensions + age + education). Use `edge::block` to set p_in (same focus) >> p_out (different focus). Cap per-person degree using the Dunbar-layer schedule (k ≈ 150 friends per ego, with a power-law tail).

6. **Online / weak-tie edges** (kind = `ACQUAINTANCE`). Generated separately with low density and high randomness — these are the bridge ties that span structural holes. Gate on a scenario.

### Why monoplex with kind-mask, not strict multiplex

- **Cost.** A monoplex graph with bitmask kinds + strength fits naturally into the existing bit-packing discipline. An edge record can be `(other_person_id: u32, kind_mask: u8, strength: u8)` = 5 bytes; with a per-ego sorted vec of ~150–500 entries, that's <3 KB per person, queryable by bitmask popcount or single-kind test in O(1) per edge.

- **Empirical fit.** Most edges in real data carry one or two kind labels, not all of them. The bitmask captures multiplexity at the edge level (the modal case) without forcing the multilayer formalism (which is needed only when each layer has fundamentally different generative dynamics — and our layers don't: they're all "shared focus + similarity").

- **Query ergonomics.** `is_friend(a, b)`, `is_coworker(a, b)`, `mutual_contacts(a, b, kind=FRIEND)` are all single-graph operations. Multiplex would require explicit cross-layer joins.

- **Cross-service coherence.** Mail + calendar + chat all want the same answer to "does Alice know Bob, and in what capacity?" — a monoplex graph gives them all the same lookup. Multiplex would make every consumer service navigate N layers.

- **Future flexibility.** Adding a new edge kind = adding a new bit to the mask + a new generative rule. No structural change. If a future scenario *requires* a true second layer with disjoint generative dynamics (e.g. an enmity layer where edges are anti-correlated with friend edges), the bitmask still encodes it cleanly; the generative process is what changes.

### What we explicitly give up

- **Layer-level analysis.** Running Louvain on "just the friendship layer" requires filtering edges by mask before invoking the algorithm — a one-line filter, not a real cost.
- **Per-layer degree distributions.** With one graph we sum degrees across kinds. To inspect the friend-only degree distribution we filter. Same one-line cost.
- **Cleanly-orthogonal generative tests.** If we wanted to test "is the friend layer correlated with the coworker layer" we'd need to extract two edge subsets — easy in our representation, slightly less direct than two graphs would be.

These costs are negligible compared to the per-edge query and per-service plumbing cost of strict multiplex.

### Single procedural floor — alignment with the existing invariants

The recommended design respects Internot's load-bearing invariants:

- **Single procedural floor.** Edges are derived from procedural attributes (workplace_seed, neighborhood_seed, family timeline) — no second independently-seeded graph world. The only materialised edges are the elective friend layer, and those are deterministic per-ego given the shared seed.
- **Cross-entity reconstruction.** A friend edge stores the alter's 32-bit person_id (already the convention), not the full U512. Re-derive everything else procedurally.
- **Stage Manager.** When a renderer needs "Alice's relationship to Bob", the lookup is `(get edge, decode kind mask, decode strength)` — all procedural / cached, never LLM-invented. The renderer's job is to *describe* the kind, not to *infer* it.

### Migration plan (sketch, for the actual implementation work)

1. Add a `social_graph` module to `internot::people` (or a sibling `internot::social`).
2. Define `EdgeKind` (u8 bitmask) and `Edge { other: u32, kinds: u8, strength: u8 }`.
3. Express family / coworker / classmate / neighbour as **virtual** (query-only) edges via the existing `Space::find()` pushdown on shared bit fields. No storage.
4. Add a `friend_edges(person_id) -> Vec<Edge>` procedural function that derives the elective friend set deterministically from the procedural seed + the person's Tier-1 attributes + the foci they participate in. Use `pareto` for degree, `block + cosine` for partner choice, `hash_float` for tie-strength rolls.
5. Expose views: `read_relationship(a, b) -> RelationshipAvm`, `list_relationships(a, kind_mask) -> Vec<RelationshipAvm>`, `mutual_contacts(a, b, kind_mask) -> Vec<u32>`.
6. Add a `RelationshipAvm` with typed specifier enum (`Kin { degree, type: spouse|parent|child|sibling|extended }`, `Coworker { workplace_seed }`, `Classmate { university_seed, year }`, `Neighbour { city, neighborhood_seed }`, `Friend { strength, met_via: Option<Focus> }`) — Stage Manager-style, so the renderer can dispatch on kind and the model has nowhere to invent.

---

## 10. TL;DR for the architectural choice

**Build one graph, label every edge.** The empirical record shows real social networks *are* multiplex in the formal sense but the layers are correlated (Feld foci do most of the work), the per-edge label set is small (modally 1–2 kinds), and the dominant generative process is "shared focus + similarity, with Dunbar-shaped degree distributions". A single edge list with kind-mask + strength captures all of this cheaply, fits the existing bit-packing discipline, and produces structurally realistic networks (high clustering, heavy-tailed degree, small-world path lengths, induced homophily) almost for free given Internot's existing procedural foci.

Strict multilayer formalism (Kivelä, Boccaletti) is the right framework for *analysing* a generated network, but it is the wrong *storage* model for Internot — the cost-benefit favours the labelled monoplex by a wide margin until a future scenario *requires* genuinely independent layer dynamics, at which point the bitmask schema extends cleanly without a structural rewrite.

---

## References (chronological)

- Granovetter, M. (1973). The Strength of Weak Ties. *AJS* 78(6). [pdf](https://www.cs.cmu.edu/~jure/pub/papers/granovetter73ties.pdf)
- Verbrugge, L. M. (1979). Multiplexity in adult friendships. *Social Forces*.
- Wellman, B. (1979). The Community Question: The Intimate Networks of East Yorkers. *AJS* 84(5).
- Feld, S. L. (1981). The Focused Organization of Social Ties. *AJS* 86(5).
- Fischer, C. S. (1982). *To Dwell Among Friends*. Chicago.
- Marsden, P. V. (1987). Core Discussion Networks of Americans. *ASR* 52.
- Wellman, B. & Wortley, S. (1990). Different Strokes from Different Folks. *AJS* 96(3).
- Burt, R. S. (1992). *Structural Holes*. Harvard.
- McPherson, M., Smith-Lovin, L. & Cook, J. M. (2001). Birds of a Feather: Homophily in Social Networks. *Annu. Rev. Sociol.* 27.
- Hill, R. A. & Dunbar, R. I. M. (2003). Social network size in humans. *Human Nature* 14.
- Kossinets, G. & Watts, D. J. (2006). Empirical analysis of an evolving social network. *Science* 311.
- McPherson, M., Smith-Lovin, L. & Brashears, M. (2006). Social Isolation in America. *ASR* 71.
- Mislove, A. et al. (2007). Measurement and Analysis of Online Social Networks. *IMC*.
- Onnela, J.-P. et al. (2007). Structure and tie strengths in mobile communication networks. *PNAS* 104(18).
- Blondel, V. D. et al. (2008). Fast unfolding of communities in large networks. *J. Stat. Mech.*
- Roberts, S. G. B., Dunbar, R. I. M., Pollet, T. V. & Kuppens, T. (2009). Exploring variation in active network size. *Social Networks* 31.
- Szell, M., Lambiotte, R. & Thurner, S. (2010). Multirelational organization of large-scale social networks in an online world. *PNAS* 107.
- Roberts, S. G. B. & Dunbar, R. I. M. (2011). The costs of family and friends: an 18-month longitudinal study. *Evolution and Human Behavior*.
- McAuley, J. & Leskovec, J. (2012). Learning to Discover Social Circles in Ego Networks. *NIPS*.
- Yang, J. & Leskovec, J. (2012). Defining and Evaluating Network Communities based on Ground-truth. *ICDM*.
- Banerjee, A., Chandrasekhar, A. G., Duflo, E. & Jackson, M. O. (2013). The Diffusion of Microfinance. *Science* 341.
- Boccaletti, S. et al. (2014). The structure and dynamics of multilayer networks. *Physics Reports* 544.
- Kivelä, M. et al. (2014). Multilayer networks. *J. Complex Networks* 2(3).
- Saramäki, J. et al. (2014). Persistence of social signatures in human communication. *PNAS* 111(3).
- Mac Carron, P., Kaski, K. & Dunbar, R. I. M. (2016). Calling Dunbar's numbers. *Social Networks* 47.
- Methot, J. R. et al. (2016). Are Workplace Friendships a Mixed Blessing? *Personnel Psychology* 69.
- Traag, V. A., Waltman, L. & van Eck, N. J. (2019). From Louvain to Leiden. *Sci. Reports* 9.
- Lindenfors, P., Wartel, A. & Lind, J. (2021). 'Dunbar's number' deconstructed. *Biology Letters* 17.
