---
name: find-unknowns
description: "Systematically discover and resolve unknowns (ambiguities, blind spots, unstated quality criteria) before, during, and after implementing a task. Based on Anthropic's 'Field Guide to Claude: Finding Your Unknowns'. Triggers: 'find unknowns', 'blind spot pass', 'interview me', 'what am I missing', 'clarify before coding', 'de-risk this task', 'quiz me on the changes'."
color: "orchid"
---

# Find Unknowns

A discovery harness for the gap between the prompt ("the map") and the actual codebase, domain, and the user's real intent ("the territory"). That gap is where expensive rework comes from. This skill closes it cheaply — with questions, sketches, and plans instead of thrown-away implementations.

## The model

Every task has four kinds of information (Rumsfeld quadrants, adapted for agentic coding):

| | User knows it | User doesn't know it |
|---|---|---|
| **Stated / resolvable** | Known knowns — already in the prompt | Known unknowns — gaps they're aware of but haven't resolved |
| **Unstated** | Unknown knowns — "obvious" context they'd recognize but never articulate | Unknown unknowns — factors and quality bars nobody has considered |

Your job when this skill runs: move as much as possible from the right and bottom cells into the top-left — *before* implementation makes discovery expensive. Balance matters: over-specifying locks in suboptimal paths, under-specifying invites misaligned guesses. Prioritize the decisions that are expensive to reverse; leave the cheap-to-change ones open.

## Invocation

`/find-unknowns [mode] [task description]`

- **No mode given**: run the **default pre-implementation flow** (below) on the task described in the args, or on the task currently under discussion if args are empty.
- **Mode keyword given** (first word of args): run only that technique. Modes: `blindspots`, `brainstorm`, `interview`, `references`, `plan`, `notes`, `pitch`, `quiz`.

## Default pre-implementation flow

1. **Triage** — read the task and the relevant code/docs. Privately sort what you find into the four quadrants. Don't show the user the raw quadrant table; use it to decide which techniques below pay off. A trivial, well-specified task may need none — say so and offer to just implement.
2. **Blind spot pass** — surface the unknown unknowns (theirs *and* yours).
3. **Interview** — resolve the highest-stakes ambiguities, one question at a time.
4. **Plan or prototype** — depending on whether the risk is architectural (→ plan) or aesthetic/UX ("I'll know it when I see it" → brainstorm/prototype).
5. **Handoff** — summarize resolved decisions and remaining open questions, then ask whether to proceed to implementation (with `notes` mode active).

## Technique playbook

### `blindspots` — Blind spot pass
*For unknown unknowns. Best when the user is new to the codebase, domain, or task type.*

Explore the relevant code/domain, then tell the user what they likely don't know they don't know: hidden constraints, coupled subsystems, domain concepts the task silently depends on, quality dimensions they haven't mentioned (perf, i18n, migrations, error paths, accessibility…). For unfamiliar domains, teach the fundamentals just enough that the user can form opinions — the goal is to upgrade *their* map, not to show off yours. End with: "given these, here's what I'd now ask you" — feeding the interview.

### `brainstorm` — Brainstorms & prototypes
*For unknown knowns — criteria the user will only recognize on sight.*

Produce genuinely divergent options, not four shades of the same idea: e.g. 3–5 wildly different design directions, a throwaway mock with fake data before wiring the real thing, or "search the codebase and brainstorm 10 intervention points for X". Optimize for *reaction speed*: cheap, contrastive artifacts the user can point at and say "that one, but…". Small early pivots here are the whole point — they're far cheaper than a mid-implementation direction change.

### `interview` — Structured interview
*For known unknowns and flushing out unknown knowns.*

Interview the user **one question at a time** (use AskUserQuestion where available, with concrete options and a recommendation). Order questions by blast radius: ask first where the answer changes the architecture or data model; skip questions whose answer doesn't change what you'd build. Stop when remaining ambiguities are cheap to reverse — note them as "deferred, will pick the conservative option" instead of asking. Cap the interview (~3–7 questions); an interrogation is its own failure mode.

### `references` — Work from references
*For requirements too rich to describe in prose.*

Ask the user for (or hunt down) existing code, designs, or products that already embody what they want — even in another language or stack ("this Rust rate-limiter has the backoff behavior I want — reimplement the semantics in TypeScript"). Read the reference and extract the *semantics* to carry over, listing explicitly what you're keeping and what you're deliberately not porting. Code beats screenshots beats descriptions.

### `plan` — Decision-ordered implementation plan
*For surfacing pivots before they're expensive.*

Write an implementation plan ordered by **decision risk, not build order**: data models, type interfaces, and user-facing behavior first; mechanical refactoring buried last. Each early item should be phrased so the user can veto it in one line. Explicitly flag the assumptions you made where the spec was silent.

### `notes` — Implementation notes (during implementation)
*For capturing what implementation teaches you.*

While implementing, maintain `implementation-notes.md` (scratch/untracked) logging: edge cases discovered, deviations from the plan and why, and places where you picked the conservative option rather than stopping to ask. Don't interrupt flow for reversible choices — log them and keep going. The notes file feeds `pitch` and `quiz` and is a review artifact in its own right.

### `pitch` — Pitch / explainer (post-implementation)
*For accelerating review.*

Package the journey into one shareable doc: what the unknowns were → what was decided and why → what changed → how to try it. Lead with the demo (GIF/screenshot/commands), fold in the spec, notable implementation notes, and open questions. Reviewers should understand the *decisions*, not just the diff.

### `quiz` — Comprehension quiz (post-implementation)
*For making sure the user actually understands what shipped under their name.*

Produce a short explainer of the changes with context and intuition (not a diff walkthrough), then quiz the user on it: the why behind key decisions, the edge cases, what would break if X changed. Grade honestly; anything missed gets re-explained from a different angle. Frame it as the merge gate: don't sign off until the user passes.

## Rules of engagement

- **Be a thought partner, not an instruction executor.** If the stated task looks like a solution to an unstated problem, ask about the problem.
- **Calibrate to the user's honesty about their own map.** Ask early: "how familiar are you with this area?" — the answer changes which techniques pay off.
- **Every artifact is disposable.** Brainstorms, mocks, plans and notes exist to surface unknowns cheaply; never gold-plate them.
- **Don't run the full flow ritually.** A one-line bugfix needs none of this. Match the ceremony to the cost of being wrong.
- **The opening question for any project is:** *"What don't I know that I need to clarify?"* — and the ROI of 30 minutes answering it beats hours of rework.
