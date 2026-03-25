# 11 - Novel Generation Quality Roadmap

> Chinese summary: [11-novel-quality-roadmap.zh-CN.md](./11-novel-quality-roadmap.zh-CN.md)

## Objective

This roadmap focuses on turning MuMuNovel from a prompt-heavy generator into a
repeatable story production system with a stable feedback loop:

1. Clarify story intent before generation.
2. Carry structured guidance through generation and regeneration.
3. Evaluate output with explicit quality gates.
4. Feed repair signals and long-term memory back into future drafts.

---

## Current Building Blocks

### 1. Project-level defaults already have a unified resolver
- Request-level values can now merge cleanly with project defaults.
- Empty strings no longer silently override meaningful defaults.
- Reference: `backend/app/services/project_generation_defaults.py:24`

### 2. Story guidance already exists as a structured object
- `StoryGenerationGuidance` can already express creative mode, story focus,
  plot stage, story brief, quality preset, and quality notes.
- This is the right foundation for a shared story packet.
- Reference: `backend/app/services/chapter_quality_context_service.py:27`

### 3. Prompt assembly already supports quality-oriented prompt blocks
- Prompt construction already supports quality preferences, creative mode,
  story brief, and repair target blocks.
- Regeneration can also consume `prompt_quality_kwargs` from project context.
- Reference: `backend/app/services/prompt_service.py:396`
- Reference: `backend/app/services/prompt_service.py:6573`

### 4. The API layer already connects guidance to regeneration
- Chapter regeneration already resolves story guidance and builds prompt-quality
  arguments before execution.
- This means project defaults, temporary overrides, and repair guidance now meet
  in one execution path.
- Reference: `backend/app/api/chapters.py:6286`
- Reference: `backend/app/api/chapters.py:6321`

### 5. The frontend now exposes regeneration quality controls
- Users can explicitly set creative mode, story focus, plot stage,
  story creation brief, quality preset, and quality notes.
- This creates a usable UI foundation for guided high-quality regeneration.
- Reference: `frontend/src/components/ChapterRegenerationModal.tsx:212`
- Reference: `frontend/src/components/ChapterRegenerationModal.tsx:513`

### 6. The memory layer is ready to evolve into a consistency layer
- The current memory service is already a valid entry point for state-aware
  storytelling memory.
- It can evolve toward character state, world rules, foreshadowing ledgers,
  and consistency constraints.
- Reference: `backend/app/services/memory_service.py:1080`

---

## Main Gaps

### Gap A: generation intent is still too request-shaped
Current parameters are stronger than before, but still spread across request
payloads, project defaults, prompt helpers, and repair guidance.

The system needs a stable `StoryPacket` (or `GenerationIntent`) object that can
hold, at minimum:
- chapter objective
- conflict progression target
- relationship change target
- foreshadow/payoff obligations
- constraints and prohibitions
- quality priorities

Without this, batch generation, caching, evaluation, and model adaptation will
become harder to maintain.

### Gap B: evaluation is still adjacent to generation, not a hard gate
Repair guidance exists, but the system still behaves mostly like:
- generate content
- inspect quality
- optionally regenerate

The stronger loop should become:
- generate draft
- evaluate against acceptance rules
- auto-select repair strategy
- regenerate or patch weak dimensions
- save only acceptable output

### Gap C: memory is not yet a strong consistency contract
High-quality long-form fiction depends on consistency more than raw word count.
The memory layer should become explicitly stratified:
- world rules
- character state
- relationship state
- foreshadowing ledger
- recent chapter local context

### Gap D: there are not enough long-form health metrics
Single-chapter quality is not enough for long serial fiction.
The platform also needs:
- recent main-plot advancement density
- unresolved foreshadow inventory
- character arc stagnation signals
- emotional curve monotony signals
- pacing imbalance across recent chapters

---

## Recommended Architecture

### Layer 1: Project Defaults Layer
Use project-level defaults as the canonical source for stable creative bias and
quality preferences.

### Layer 2: Story Packet Layer
Create a shared structured object for chapter generation, outline generation,
batch generation, and regeneration.

### Layer 3: Prompt Assembly Layer
Keep prompt construction modular and block-based. Avoid giant string assembly
that mixes constraints, guidance, style, and repair instructions.

### Layer 4: Quality Gate Layer
Introduce acceptance scoring and rule-driven repair routing.
This is the key step from ?can generate text? to ?can reliably ship chapters.?

### Layer 5: Memory and Consistency Layer
Promote memory from optional retrieval to a state-aware consistency system.

### Layer 6: Feedback Loop Layer
Feed manual edits, failure samples, and repair outcomes back into defaults,
prompt strategy, and quality rules.

---

## Phased Roadmap

### Phase 1: unify the generation contract (P0)
Goal:
- unify outline generation, chapter generation, batch generation, and
  regeneration under the same input contract

Actions:
- introduce `StoryPacket` or `GenerationIntent`
- merge project defaults, request overrides, and repair guidance into it
- align field names across frontend and backend
- snapshot the object for debugging and quality forensics

Expected impact:
- lower prompt drift
- simpler orchestration
- easier future model routing

### Phase 2: add hard quality gates (P0)
Goal:
- move from ?generate then inspect? to ?generate and only save if accepted?

Actions:
- define minimum acceptance dimensions for chapter quality
- classify outcomes into pass / auto-repair / manual-attention-needed
- map weak dimensions directly to repair strategies and focus areas
- prefer targeted repair over full rewrite when possible

Expected impact:
- fewer weak chapters being saved
- more stable lower-bound quality in long runs

### Phase 3: evolve memory into continuity control (P1)
Goal:
- enforce continuity across character, world, relationship, and foreshadowing

Actions:
- split memory into world, character, relationship, foreshadow, and local context
- extract structured state changes after each generated chapter
- compare planned chapter intent with existing memory before generation
- add foreshadow states such as seeded / pending / paid off / stale

Expected impact:
- fewer out-of-character moments
- fewer world-rule conflicts
- fewer forgotten setup/payoff chains

### Phase 4: add long-form pacing control (P1)
Goal:
- optimize the reading experience across a sequence of chapters, not only per chapter

Actions:
- add volume-level pacing plans
- measure recent chapter progression density and emotional variation
- warn when the story is over-padded or under-escalated
- constrain outline generation with chapter function distribution

Expected impact:
- less middle-section collapse
- stronger reader-perceived momentum

### Phase 5: build an experimentation and feedback platform (P2)
Goal:
- move from intuition-based tuning to evidence-based tuning

Actions:
- create A/B sample sets for quality presets and creative modes
- log high-friction manual edits by category
- classify failure cases such as slow pacing, over-explaining, weak hooks,
  OOC behavior, and unpaid foreshadowing
- build regression reports for prompt blocks and quality rules

Expected impact:
- measurable prompt improvements
- more stable iteration velocity

---

## Suggested Priorities for the Next Two Weeks

### Priority 1
- implement `StoryPacket`
- route chapter generation, outline generation, and regeneration through it
- add a repair-strategy mapping table for failed quality dimensions

### Priority 2
- add character state snapshots and foreshadow ledgers to memory
- add continuity-risk warnings before saving chapters

### Priority 3
- add volume pacing plans
- expose a lightweight chapter-quality trend API for dashboards

---

## Suggested Metrics

### Generation quality metrics
- average chapter score
- percentage of chapters below acceptance threshold
- regeneration trigger rate
- pass rate after regeneration

### Long-form continuity metrics
- main-plot advancement density across recent chapters
- unresolved foreshadow count
- character-state conflict count
- outline drift count

### Throughput metrics
- average generation time per chapter
- average regeneration count per chapter
- post-generation manual edit ratio

---

## Recommended Next Engineering Tasks

1. Add a shared `StoryPacket` schema and use it in generation and regeneration.
2. Add an evaluation-to-repair mapping table in
   `chapter_quality_context_service`.
3. Extend `memory_service` with structured character-state and
   foreshadow-state writes.
4. Add volume pacing input to outline and chapter generation.
5. Expose a chapter-quality trend API for the frontend.

---

## Key References

- `backend/app/services/project_generation_defaults.py:24`
- `backend/app/services/chapter_quality_context_service.py:27`
- `backend/app/services/prompt_service.py:396`
- `backend/app/services/prompt_service.py:6573`
- `backend/app/api/outlines.py:1824`
- `backend/app/api/chapters.py:6286`
- `backend/app/services/memory_service.py:1080`
- `frontend/src/components/ChapterRegenerationModal.tsx:212`
- `frontend/src/components/ChapterRegenerationModal.tsx:513`
