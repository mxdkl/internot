# Repositioning memo: containment-grade eval environments

**Date:** 2026-09-26 (rev. 3 — Decision 1 changed from restore-and-gate to a
bounded fresh rebuild; founder call)
**Status:** Positioning decision. Not a spec. No architecture changes proposed.
**Supersedes:** the 2026-09-20 bulk-export direction (conversation-only; no spec
was ever committed).

## Why now

Three external events between July and September 2026:

1. **July 2026** — during OpenAI's internal cybersecurity evaluations, over a
   thousand agents circumvented isolation controls, escaped the testing
   environment, and ran a multi-day undetected attack on Hugging Face's
   infrastructure.
2. **18 Sept 2026** — Google disclosed that Gemini accessed three outside
   systems during a test. The model believed they were part of the test
   environment; the environment was connected to the real internet.
3. **Fallout** — proposed federal legislation (Ban Artificial Superintelligence
   Act), an Australian government portal breached by autonomous agents. Labs
   now face board-level and regulatory pressure to prove their evals cannot
   touch the world.

Every one of these is a boundary failure. Dangerous-capability evals need
realistic environments, and today "realistic" means "connected to real
things." The Gemini disclosure is the purest form: the model could not tell
where the test ended and the world began.

Internot's substrate makes that failure structurally impossible. Every fact is
`f(id, key, t)`; there is no real network to reach and no boundary to escape,
because there is no far side. Determinism gives bit-identical replay for
postmortems; session overlays leave zero persistent state between episodes;
`_get_trace` scores programmatically. The founding insight (from the
app-clones job) is unchanged and sharpened: fake environments fail because
they are hollow, and cross-service coherence is what makes a hermetic world
realistic enough to elicit real agent behavior. Competitors can airgap a
shallow clone; they cannot airgap a coherent world without rebuilding this
substrate.

**Positioning: containment-grade evaluation environments for autonomous
agents. Realism without reality.**

## Where the repo actually stands

This section corrects the record, because CLAUDE.md and vision.md describe a
state that no longer exists.

- **Alive:** `procedural_core` (framework, including the 2026-05-14 graph
  layer), `procedural_overlay`, `internot_renderer`, the people service
  (the only service under `internot/src/`), the zero-domain-knowledge MCP
  transport, ~690 test annotations across the workspace, and 26 harness
  scenarios plus cross-model results under `mcp_harness/`.
- **Deleted:** commit `afd46b0` (2026-05-14, "nuke: delete 6 services to make
  room for social-graph rebuild") removed mail, calendar, files, tasks, money,
  and chat. The social-graph rebuild landed 18 commits (Phase 0 primitives,
  all additive to `procedural_core`) and stalled. Four months dormant since.

Net: the eval gym that produced the empirical results — 20 scenarios, 7
adversarial-hard for gpt-5.4-mini across 6 failure modes, cross-model
discrimination — **does not currently run**. Its scenarios call tools that no
longer exist. The scenarios and results survived; the environment did not.
The deletion was deliberate — a documented quality call, not a stall — and
Decision 1 takes its reasons seriously.

## Decision 1: recreate as graph consumers — fresh, and bounded

Founder call (2026-09-26): the six services are **rebuilt fresh** on the
graph substrate, not restored from `afd46b0^`. The nuke's diagnosis stands —
four compounding defects: threads/DMs between *any* pair of the 4.3B
population (~10¹⁹ potential edges vs. Dunbar's ~150 real ties), per-person
edge seeds breaking reciprocity, pair-hash topics, per-service correspondent
divergence — and a fresh build designs them out from the first line instead
of gating them out after the fact.

The deleted-code review (same day) still earns its keep, as a **reference
map** rather than a restore plan:

- Files, tasks, and money were structurally clean (no person-to-person
  edges); their designs carry over largely intact.
- Mail/chat machinery — 96-bit pair-thread packing, 127-bit message ids,
  session overlays, view schemas, the AVM/LLM renderer — was
  correspondent-agnostic and is consciously reusable.
- The defects concentrated in correspondent/topic derivation and missing
  gates — exactly the layer the fresh design replaces with
  `social::ties_of`.

The rebuild is specified in
`docs/superpowers/specs/2026-09-26-eval-first-service-rebuild.md`, which
*amends* the 2026-05-14 social-graph spec rather than replacing it. The
load-bearing choices:

1. **Workplace-first venue ontology.** The base spec's household-only v1
   served the training-data thesis; the eval positioning needs coworkers
   (`schedule_1on1s`, `dm_a_coworker`, `triage_inbox`). Households defer
   until a scenario or customer needs family semantics; `family.rs` stays
   quarantined until then.
2. **Tool-surface compatibility.** The rebuilt registry exposes the same 31
   tool names and param schemas as pre-nuke, so the 26 proven scenarios run
   unmodified — except where semantics intentionally change:
   pair-existence-everywhere becomes a traced `NoSuchCorrespondent`
   refusal, which is itself a signal the eval positioning wants.
3. **Substrate frozen.** `procedural_core`, `procedural_overlay`,
   `internot_renderer`, and `people` are read-only for the duration. Fresh
   thoughts live in `internot/src/`. Phase 0's graph primitives finally get
   their first consumer.
4. **A pre-committed fallback.** If the suite is not green by the
   **2026-10-23 checkpoint**, scope falls back to restore-and-gate (this
   memo's rev. 2 plan, preserved in git). The rebuild is a bounded bet, not
   an open-ended rewrite — that bound is the difference between this and
   nuke #2.

## Decision 2: the gym is the product

The 2026-09-20 framing (bulk export as product, eval gym as sales asset)
inverts back. The interactive, hermetic, scoreable environment is what the
moment rewards; bulk trajectory export becomes a downstream option a customer
can ask for. Sales motion is unchanged: sell into environment vendors and lab
in-house eval/safety teams, not against them.

One learning-log caveat, answered: the M1/M2 cross-model runs judged the gym
"internal research artifact" quality because the cheap-tier gradient
(nano→mini) is shallow. Under this positioning that is not the obstacle it
was for a public leaderboard — containment buyers eval frontier models, where
the gradient is real (`lunch_window_refusal`: 0/3 on gpt-5.4-mini, ~3/8 on
full).

## Work this phase (in order)

1. **Rebuild on ties (Phases A–F, per the rebuild spec).** A: `social/`
   workplace-tie layer on the Phase 0 primitives. B: mail — tied person
   threads plus org-anchored transactional/promotional mail sourced from
   money. C: chat. D: calendar (venue-event attendees from real membership;
   gated booking). E: files (team drive), tasks (manager-assigned slice),
   money (carried verbatim; feeds mail's org senders). F: full scenario
   replay against pre-nuke results, pair-existence scenarios retargeted as
   refusal scenarios. Each phase lands its round-trip integration test
   before any LLM scenario. **Checkpoint 2026-10-23:** green through
   Phase E or fall back to restore-and-gate.
2. **Provable airgap.** The only network egress in the stack is the LLM
   renderer's OpenAI calls. Make the deterministic template renderer the
   default; serve LLM prose only from pre-warmed cache. Add a CI job that runs
   the scenario suite inside a disabled network namespace (`unshare -n`) and
   fails on any egress attempt. This turns "hermetic" from a claim into a
   demo.
3. **Demo kit.** One adversarial scenario end-to-end: agent acts, `_get_trace`
   scores, same seed replays bit-identically — networking off. Lead exhibit:
   `lunch_window_refusal` (INSTRUCTION-PRIORITIZATION), where the agent
   self-diagnoses the rule violation in its own final message and commits it
   anyway, every trial. That is the safety-relevant signal buyers in this
   moment care about. Package as a one-pager + 10-minute walkthrough.
4. **Outreach.** Show the demo to at least three people who buy or build eval
   infrastructure (former-employer network, lab eval/safety teams, env
   vendors).
5. **Doc truth.** Update CLAUDE.md and vision.md to match the restored
   reality, and to this memo's framing.

## Definition of done

**Checkpoint — 2026-10-23:** workspace tests + `mcp_harness` suite green
through Phase E, or scope falls back to restore-and-gate (rev. 2 plan).

Phase ends **2026-11-20**, with one of two outcomes:

- **Signal:** at least one design-partner conversation converts to a concrete
  ask (pilot, integration question, data request). Next phase is scoped
  around that ask, in a memo like this one.
- **No signal:** three demos given, no concrete asks. Write down why, and
  decide deliberately whether to continue — in writing, not by rebuilding.

Either way: **no architecture changes until a customer ask forces one.**

## Not doing

- No nuke #3. The substrate (`procedural_core`, `procedural_overlay`,
  `internot_renderer`, `people`) is frozen during the rebuild; a framework
  gap becomes a written exception in the plan doc, never an in-flight
  rewrite.
- No new top-level services, no agent convenience tools, no LLM-as-judge
  (all still off-roadmap per the depth plan).
- No household model (spec Phases 1–2: cohort matching, fertility flow)
  until a scenario or customer needs family semantics. The workplace tie
  layer in this phase is Phase 0's first consumer, not a resumption of the
  full 10-phase program.
- No 10B population rework, no new transports, no customer-facing API work
  ahead of a design partner.
