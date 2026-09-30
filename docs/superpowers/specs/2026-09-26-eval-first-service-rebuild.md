# Eval-first service rebuild — fresh design over the graph substrate

**Date:** 2026-09-26
**Status:** Design (pre-implementation)
**Base:** Amends [`2026-05-14-social-graph-substrate.md`](2026-05-14-social-graph-substrate.md).
Sections of the base spec not amended here carry over unchanged. Base §5
(`procedural_core::graph`) is **shipped** (Phase 0, 2026-05-14) — this spec
consumes it.
**Decision context:** `docs/memos/2026-09-26-repositioning-hermetic-evals.md`.
Founder call 2026-09-26: recreate the six deleted services fresh rather than
restore `afd46b0^`. The old code is a *reference implementation*, not a
restore target.

## 1 — What changed since the base spec

1. **Positioning.** Internot now sells containment-grade eval environments
   (see the memo). Buyers eval office-shaped agents; the proven adversarial
   scenarios (`schedule_1on1s`, `dm_a_coworker`, `triage_inbox`,
   `lunch_window_refusal`) are all workplace-flavored.
2. **Venue ontology inverts.** Base v1 was household-only, workplace deferred
   to v2. This spec makes **workplace the v1 venue**; the household model
   (base §6: cohort matching, fertility flow, `family.rs` reciprocity fix)
   defers until a scenario or customer needs family semantics. Base §6
   remains the design of record for when that happens.
3. **Phase 0 exists.** `procedural_core::graph` (`VenueSpace`, `Tie`,
   `tie_strength`, `comm_intensity`, `enumerate_events`,
   `stable_roommates_match`) is live, tested, and currently consumer-less.
   This rebuild is its first consumer.

## 2 — Design constraints (load-bearing)

1. **Tool-surface compatibility.** The rebuilt registry exposes the same
   tool names and param schemas as pre-nuke, so the 26 scenarios in
   `mcp_harness/` run unmodified except where semantics intentionally change
   (§6). Additive params are allowed; renames are not. The surface:
   - mail: `get_inbox`, `read_thread`, `compose_email`, `reply`,
     `mark_read`, `archive_thread`
   - calendar: `get_schedule`, `rsvp`, `book_meeting`, `get_busy`,
     `find_meeting_slot`
   - files: `list_drive`, `read_file`, `upload_file`
   - tasks: `list_tasks`, `read_task`, `mark_done`, `create_task`
   - money: `list_accounts`, `read_account`, `list_transactions`,
     `list_subscriptions`
   - chat: `list_dms`, `read_dm`, `send_dm`
   (plus the surviving people 5 + `_get_trace`.)
2. **Substrate frozen.** `procedural_core`, `procedural_overlay`,
   `internot_renderer`, and `internot/src/people` are read-only for the
   duration of the rebuild. A discovered framework gap becomes a written
   exception in the plan doc — never an in-flight rewrite. Fresh thoughts
   live in `internot/src/`.
3. **`social` is substrate, not a service.** `internot/src/social/` exports
   Rust functions consumed by the six services. It registers **no views** in
   v1 — agents perceive the graph through service behavior, which respects
   the standing "no agent convenience tools" rule. (It may still impl
   `Service` for space registration only.)
4. **Reference, don't restore.** `afd46b0^` is consulted deliberately. Copy
   what the 2026-09-26 review found clean: 96-bit pair-thread packing,
   127-bit message ids, session-overlay patterns, view schemas, the AVM/LLM
   renderer shape, files/tasks/money designs. Redesign what it found
   defective: correspondent derivation, topic seeds, missing gates.
5. **Round-trip test before LLM scenario.** Every phase lands with its
   `internot/tests/<svc>_round_trip.rs` before any harness scenario touches
   it (standing project rule).

## 3 — `internot/src/social/` v1 (workplace ties)

The canonical graph consumer layer. Modules mirror base §6.1 minus the
household machinery:

- `social/venue.rs` — the current workplace as a venue: venue id packs the
  existing slot bits `(industry:6, city:6, workplace_seed:8)`. Membership
  enumeration re-expresses `people::coworkers_of` through
  `graph::VenueSpace` (bit-pattern pushdown, O(|venue|)).
- `social/tie.rs` — `ties_of(world, viewer, t) -> Vec<Tie>` and
  `are_tied(world, a, b, t) -> Option<Tie>`. v1 tie kinds:
  `Coworker`, `Manager`, `Report` — role derived from career-arc role level
  (already in the U512 tiers), tiebroken by `member_idx`.
- `social/strength.rs` — `TieStrengthProfile` per kind. `Tie.since` derives
  from the viewer's career arc (latest join event at the current employer);
  strength ramps from `since` via `graph::tie_strength`. **No decay ties in
  v1** — `ExCoworker` requires past-venue enumeration (membership windows
  over career history) and is explicitly deferred (§7).
- `social/intensity.rs` — `graph::comm_intensity` fed by a
  `PersonalityProjection` built from the person's cached Big Five +
  chronotype attributes.
- `social/events.rs` — per-mode wrappers over `graph::enumerate_events`
  with namespaces `"mail"` / `"chat"` / `"cal"`. **Window convention:**
  because the Phase 0 enumerator's event index resets at `t_start` (v1
  limitation, does not slice-recombine), every service enumerates from the
  same canonical anchor — `tie.since` — and filters to its query window.
  This makes repeated view calls consistent by construction. Pin this with
  a regression test.
- Refusal plumbing: services gate reads-by-id and all mutations on
  `are_tied` (or org-anchor validity); failures return
  `ViewError::NoSuchCorrespondent` (new variant) and append to
  `MutationTrace.invalid_correspondent_attempts` (base §11). Refusal
  attempts are a first-class eval signal.

## 4 — Per-service fresh design (deltas from base §7)

### 4.1 mail
- **Person threads exist only over ties.** Enumeration walks `ties_of`;
  `thread_for_pair` is no longer total — it returns `Option`, `None` for
  non-tied pairs, end to end (read views included).
- **Topics from context, not pair hash.** Topic distribution keyed on
  (tie kind, shared venue, career context): coworker threads draw from
  work/scheduling/project vocab seeded by the venue's industry bits;
  Manager/Report ties skew directive/status-report. The base §7.1
  household topic tables apply when households land.
- **NEW — org-anchored threads.** Transactional and promotional mail
  sourced from `money`: each subscription emits receipts on its billing
  cadence; vendors emit promos at low procedural rates. This replaces the
  old random `TopicCategory::{Transactional, Promotional}` with
  cross-service truth — the Netflix charge in `list_transactions` and the
  Netflix receipt in `get_inbox` are the same procedural fact. Layout: a
  thread `anchor_type` carved from the 31 free bits of the 127-bit id
  space (calendar's anchor pattern is the precedent); exact widths in the
  plan doc.
- Volume/timing via `social::events` (mail mode). Machinery carried from
  reference: message packing, `MailSession`, view schemas, AVM + LLM
  renderer — AVM gains tie fields (kind, role distance → formality band).

### 4.2 chat
- DMs only over strong ties (`strength ≥ threshold` at `t`) — a subset of
  mail's person edges. Short-form; chat-mode intensity; topics skew
  coordination/immediate. Group rooms deferred. Same gate + refusal path.

### 4.3 calendar
- Keep the three anchors. **Venue events unchanged in concept** (standup /
  all-hands were already venue-derived) — now their attendee lists derive
  from venue membership slices (your standup = your team, not a hash).
  **Thread anchors only over tied threads.** `book_meeting` gated on
  `are_tied` per attendee. Personal anchors unchanged.

### 4.4 files
- Reference design carries (it was clean: owner-scoped slots, pareto
  counts, participant-owned cross-references). **NEW:** a
  shared-with-venue flag bit → "team drive": `list_drive` gains shared
  items with owner attribution (base §7.4's household-share, re-aimed at
  the workplace).

### 4.5 tasks
- Reference design carries (clean; time-varying `status_of` stays). **NEW:**
  a procedural slice of tasks carries `assigned_by` = a Manager-tie peer —
  feeds instruction-prioritization scenario design (conflicting
  directives from a manager vs. the launched viewer's own rules).

### 4.6 money
- Reference design carries essentially verbatim (read-only, no
  interpersonal edges). Becomes the **source of record for org senders**
  consumed by mail (§4.1). Restore `internot/data/mcc_codes.json` +
  `build_mcc.py` from `afd46b0^` (deleted as "orphaned"; no longer
  orphaned). Joint accounts / intra-household transfers wait for
  households.

### 4.7 trace
- Add `invalid_correspondent_attempts` (attempted peer + view name).
  Everything else unchanged.

## 5 — Phasing and definition of done

Sequential A → B, then B–E parallelizable in principle; F last.

| Phase | Scope | Done when |
|---|---|---|
| A | `social/` v1 + property tests (reciprocity via shared venue object, determinism, window-convention regression) | tests green |
| B | mail (tied person threads + org threads + renderer) | `mail_round_trip` green; tool schemas match §2.1 |
| C | chat | `chat_round_trip` green |
| D | calendar | `calendar_round_trip` green |
| E | files, tasks, money | round-trips green; registry smoke shows full §2.1 surface |
| F | scenario replay: full `mcp_harness` matrix vs. pre-nuke recorded results; pair-existence scenarios retargeted as refusal scenarios; one new refusal scenario (mail a stranger → `NoSuchCorrespondent` traced) | matrix documented in `mcp_harness/results/` |

**Checkpoint 2026-10-23 (pre-committed):** workspace + suite green through
Phase E, or scope falls back to restore-and-gate (the memo's rev. 2 plan,
preserved in git history). This is the bound on the bet — it is what makes
this rebuild different from nuke #2.

## 6 — Intentional semantic changes vs. pre-nuke

Scenarios relying on these must be retargeted in Phase F:

1. Composing / DMing / booking a non-tied person now refuses
   (`NoSuchCorrespondent`) instead of silently working.
2. `read_thread` on a constructed id for a non-tied pair returns the same
   refusal (totality removed at every entry point, not just mutations).
3. Transactional/promotional inbox items are now money-coherent (and so
   assertable cross-service in verdicts).

## 7 — Out of scope (explicit)

- Household venue + family ties (base §6 intact for later; `family.rs`
  stays quarantined until then).
- `ExCoworker` / past-venue membership windows (needs time-varying venue
  enumeration over career arcs — a real design, not a v1 afterthought).
- Group threads / rooms; hyperbolic weak ties (base §10); minors.
- Any new MCP tools, any `procedural_core` / overlay / renderer / people
  changes, LLM-as-judge.
