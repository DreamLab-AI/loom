# agentbox ADR-051 — Loom client & deferred distillation (de-vendored stub)

> **This is a cross-link stub, not the canonical document.** The full ADR-051 lives in its
> owning repo, **`DreamLab-AI/agentbox`** (now archived at `docs/archive/adr/ADR-051-loom-client-and-deferred-distillation.md`;
> current operative decision: ADR-2023).
> A full copy used to be vendored here; it was removed (2026-08-17) because a second copy is a
> drift surface — exactly the "no fourth copy / no re-vendoring" rule the doc set enforces
> (DOC-REENGINEERING-PLAN P4). Read the canonical in agentbox; this stub records only what a
> Loom reader needs to know it exists and why it matters to this repo.

**What it is.** ADR-051 owns the **harness (agentbox) side** of the Ontology Loom capstone:
agentbox as a *client* of the Loom façade, the consumer-side deferred-distillation MCP tools
(submit → await → fetch), and the beads-adapter changes that make a distillation job a durable,
fenced, content-addressed work item. Status there: *Proposed* (direct-to-target dev/test build).

**Why it matters to this repo.** ADR-051 is the **client-side contract this Loom binary must
keep stable.** The façade wire shapes it consumes — `/v1/chat/completions`, `/loom/scaffold`,
`/loom/generation`, and the deferred `/loom/distill` envelope — are specified on the *Loom* side
by VisionClaw **ADR-135** (façade + lifecycle) and **PRD-025**, and re-platformed onto the Rust
substrate by **ADR-137 / PRD-027** with the contract preserved. ADR-051 consumes those shapes
and does not redefine them; its own `review_trigger` fires if the Loom façade changes its
generation/index shape. The Rust re-platform is deliberately contract-identical so ADR-051's
client binds unchanged (PRD-027 AC-2).

**Historical consumer note:** the original stub named the email gateway as the
only live consumer. Current agentbox source also has Loom retrieval/scaffold
consumer paths; see the [estate consumer trace](../../../VisionFlow/docs/estate-review/agent-grounding-and-governance.md).
That source evidence does not certify deployment or complete deferred distillation.

- **Canonical:** `DreamLab-AI/agentbox` → ADR-051.
- **Loom-side contract it consumes:** [`ADR-135`](./ADR-135-ontology-loom-node.md) D1,
  [`ADR-137`](./ADR-137-loom-rust-replatform.md), [`PRD-025`](./PRD-025-ontology-loom-and-connector-platform.md),
  [`PRD-027`](./PRD-027-rust-loom-reengineering.md) §4 (consumers).

## Lineage review — 2026-09-04

This remains a support stub, not an independent decision. Read the [historical ADR-051](../../../project/agentbox/docs/archive/adr/ADR-051-loom-client-and-deferred-distillation.md) and [operative ADR-2023](../../../project/agentbox/docs/adr/ADR-2023-loom-facade.md) in the owning repository. Deferred-job acceptance remains separate from implemented retrieval.
