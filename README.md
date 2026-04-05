# Finite Stabilization Theorem

A research repository for a deterministic abductive fixed-point reasoning architecture in which iterative updates over commitments and hypotheses converge to a stable state.

## Core idea

Let `X_n = (Γ_n, Ξ_n)` be the epistemic state at stage `n`, where:

- `Γ_n` is the accepted commitment set,
- `Ξ_n` is the surviving hypothesis set.

The engine alternates between:

1. filtering hypotheses by consistency with the current commitments and observations,
2. computing bounded closures for the surviving hypotheses,
3. intersecting those closures skeptically,
4. stopping when the state no longer changes.

The repository now fixes one normative schedule:

- first compute `Ξ_{n+1}` from `Ξ_n`,
- then compute `Γ_{n+1}` from closures indexed by `Ξ_{n+1}`.

It also fixes the empty-survivor convention:

- if `Ξ_{n+1} = ∅`, then the skeptical core is `∅`, so `Γ_{n+1} = Γ_0`.

These conventions are part of the semantics rather than implementation choices.

## Repository map

- [`proposal-arch.md`](proposal-arch.md)  
  Theorem-oriented statement of the update system, conventions, and companion result agenda.

- [`ARCHITECTURE_OUTLINE.md`](ARCHITECTURE_OUTLINE.md)  
  Implementation-oriented blueprint with normative transition semantics, invariants, component boundaries, and verifier hooks.

- [`transition_engine.py`](transition_engine.py)  
  Reference deterministic transition module implementing the same schedule and empty-set convention as the documents.

- [`KR-mine-search.md`](KR-mine-search.md)  
  Literature synthesis on abductive, fixed-point, and verification-adjacent KR sources.

- [`Deeper.md`](Deeper.md)  
  Extended source-to-architecture mapping notes.

## Current status

This repository is still theory-first, but it now has a single repo-wide transition semantics rather than a split between document and prototype behavior.

## Why this framing

The point is to make convergence a semantic object that is reproducible and independently checkable:

- deterministic transitions,
- explicit stabilization criteria,
- bounded consequence computation,
- auditable iteration traces,
- independent verification of each step.
