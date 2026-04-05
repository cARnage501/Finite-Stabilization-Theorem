# Architecture Outline: Deterministic Abductive Fixed-Point Reasoning System

## 1) Purpose and scope

This document translates the repository research notes into an implementable architecture for a **deterministic, abductive, fixed-point reasoning engine**.

The target system repeatedly:
1. proposes candidate hypotheses,
2. computes bounded consequences,
3. filters hypotheses against observations and consistency constraints,
4. aggregates skeptical commitments,
5. stops at stabilization.

The architecture is designed so that each step is auditable, testable, and independently verifiable.

---

## 2) Design goals (derived from current docs)

- **Determinism:** same input state must produce the same next state.
- **Finite stabilization:** under finite assumptions, the trajectory reaches a fixed point in finite steps.
- **Abductive narrowing:** maintain and shrink a hypothesis set based on consistency with observations and commitments.
- **Skeptical acceptance:** accept conclusions only when they survive across viable hypotheses.
- **Verification separation:** keep proposal/generation logic separate from trusted checking.
- **Case-space closure controls:** support finite enumeration or bounded exploration with explicit stopping criteria.

---

## 3) Core formal state model

Use the state tuple:

- `State_n = (Gamma_n, Xi_n, Meta_n)`
  - `Gamma_n`: accepted commitments at iteration `n`
  - `Xi_n`: surviving hypotheses at iteration `n`
  - `Meta_n`: diagnostics/telemetry (`n`, hashes, timings, proof artifacts, conflict sets)

And a static `ProblemSpec`:

- `Gamma0`: initial commitments/theory
- `Xi0`: initial hypothesis pool
- `Theta(H)`: hypothesis-to-theory expansion
- `Obs`: observations
- `Q`: bounded query vocabulary
- `Rules`: Horn or Horn-like closure rules (deterministic)
- `IC`: integrity constraints

Primary update scheme (matching current theorem notes):

- `Xi_{n+1} = {H in Xi_n | Consistent(Gamma_n ∪ Theta(H) ∪ Obs ∪ IC)}`
- `Gamma_{n+1} = Gamma0 ∪ Intersect_{H in Xi_n}(Closure_Q(Gamma_n ∪ Theta(H) ∪ Rules))`

Stop when `(Gamma_{n+1}, Xi_{n+1}) == (Gamma_n, Xi_n)`.

---

## 4) Component architecture

## 4.1 Input & normalization layer

**Responsibility**
- Parse and normalize `ProblemSpec`.
- Enforce finiteness profile (`Q`, bounded symbols, function-free mode for MVP).
- Canonicalize literals/rules for deterministic hashing.

**Outputs**
- `NormalizedSpec`
- deterministic IDs for hypotheses/rules/literals.

## 4.2 Hypothesis manager

**Responsibility**
- Store `Xi_n`, hypothesis metadata, and signatures.
- Provide deterministic iteration order.
- Track elimination reasons (`conflict with Obs`, `violates IC`, `inconsistent with Gamma_n`).

**Outputs**
- `Xi_n` set/list and elimination report.

## 4.3 Closure engine (bounded consequence writer)

**Responsibility**
- Compute `Closure_Q(S)` using deterministic forward chaining.
- Restrict conclusions to vocabulary `Q`.
- Emit derivation traces (minimal proof DAG or rule-fire log).

**Outputs**
- closure set per hypothesis
- derivation trace artifacts.

## 4.4 Consistency & integrity checker

**Responsibility**
- Decide `Consistent(S)` against contradictions and integrity constraints.
- Produce minimal conflict set if inconsistent.

**Outputs**
- boolean decision + optional conflict certificate.

## 4.5 Skeptical aggregator

**Responsibility**
- Compute intersection across viable hypothesis closures.
- Merge with `Gamma0`.
- Enforce canonical representation and monotonicity policy.

**Outputs**
- `Gamma_{n+1}` + change set vs `Gamma_n`.

## 4.6 Stabilization controller

**Responsibility**
- Run iteration loop.
- Detect fixed point and compute `n*`.
- Guard with hard limits (`max_iterations`, `max_case_checks`) and classify termination mode.

**Outputs**
- final state `(Gamma*, Xi*)`
- trajectory and convergence report.

## 4.7 Independent verifier (trusted kernel)

**Responsibility**
- Re-check candidate step transitions from produced artifacts.
- Validate each elimination and each accepted conclusion.
- Optionally verify full run hash-chain.

**Outputs**
- pass/fail verdict + machine-checkable report.

---

## 5) Execution flow

1. Load `ProblemSpec`; normalize and validate constraints.
2. Initialize `Gamma_0 := Gamma0`, `Xi_0 := Xi0`.
3. For each iteration `n`:
   - For each `H in Xi_n`:
     - evaluate consistency of `Gamma_n ∪ Theta(H) ∪ Obs ∪ IC`;
     - keep or remove `H` with explicit reason/certificate.
   - Compute closure for each `H in Xi_n` (per schedule choice: current set or next set, must be fixed globally).
   - Aggregate skeptically to form `Gamma_{n+1}`.
   - Emit iteration artifact bundle (inputs, outputs, traces, hashes).
   - Stop on fixed point.
4. Run independent verifier over the artifact bundle.
5. Publish final outputs and metrics.

---

## 6) Data contracts

Minimal JSON-like contracts:

- `hypothesis`:
  - `id`, `assumptions[]`, `signature[]`, `status`
- `iteration_record`:
  - `n`, `gamma_in_hash`, `xi_in_hash`, `kept[]`, `eliminated[]`, `gamma_out_hash`, `xi_out_hash`, `timings`
- `certificate`:
  - `type` (`consistency`, `inconsistency`, `derivation`, `intersection`)
  - `payload`
  - `checker_version`
  - `digest`
- `run_report`:
  - `n_star`, `termination_mode`, `|Gamma*|`, `|Xi*|`, trajectory stats, verifier verdict.

---

## 7) Invariants and theorem hooks

Track these invariants at runtime to match theorem program goals:

- `Xi_{n+1} subseteq Xi_n` (hypothesis elimination monotonicity).
- Determinism: `Step(State_n, Spec)` is functional.
- Boundedness: all emitted commitments belong to `Q`.
- Soundness hook: each `phi in Gamma_n` has derivation support in closure traces.
- Stabilization check: equality is structural over canonicalized sets.

These invariants become executable assertions, not just paper claims.

---

## 8) Complexity and scaling model

Per iteration (rough order):

- consistency checks: `O(|Xi_n| * C_consistency)`
- closures: `O(|Xi_n| * C_closure(|Q|, |Rules|))`
- skeptical intersection: `O(total_closure_size)`

Total:

- `T_total = Sum_{n=0..n*-1} T_step(n)`

Practical levers:

- cache closure by `(Gamma_n_hash, hypothesis_id)` when safe,
- early prune before full closure,
- partition hypotheses by signatures,
- parallelize closure/checking while preserving deterministic reduction order.

---

## 9) Reference MVP design

**MVP constraints**
- function-free Horn rules,
- finite `Q`, finite `Xi0`,
- deterministic contradiction test (`u` and `not_u` style encoding),
- JSON artifact emission per iteration.

**MVP modules**
- `spec_parser`
- `closure_engine_horn`
- `consistency_checker`
- `loop_controller`
- `artifact_writer`
- `independent_checker`

**MVP acceptance criteria**
- reproduces same `(Gamma*, Xi*, n*)` across repeated runs,
- emits full elimination reasons,
- verifier independently confirms every transition,
- includes benchmark cases with known fixed points.

---

## 10) Roadmap (phased)

### Phase 1 — Formal/Executable core
- implement deterministic loop and invariants,
- establish fixed-point detection and trajectory logging,
- deliver theorem-to-assertion mapping.

### Phase 2 — Verification hardening
- add certificate format and checker CLI,
- produce tamper-evident hash-chain across iterations,
- add replay mode for third-party verification.

### Phase 3 — Richer semantics
- support well-founded/alternating fixed-point variants,
- plug in abductive minimality criteria,
- add schedule comparison mode (`Xi_n` vs `Xi_{n+1}` in Gamma update).

### Phase 4 — Robustness & sensitivity
- perturbation experiments for `Obs`,
- measure `delta n*`, `delta |Xi*|`, `delta |Gamma*|`,
- identify phase-transition regimes and stability envelopes.

---

## 11) Risks and mitigations

- **State explosion in hypothesis space**
  - mitigate with signature-based pruning, conflict-directed elimination, bounded case exploration.
- **Semantic drift between generator and verifier**
  - mitigate with strict certificate schema and checker-first contract tests.
- **Non-termination outside finite fragment**
  - mitigate with explicit profile flags and rejection of unsupported specs for deterministic mode.
- **Ambiguous schedule choices**
  - mitigate by pinning schedule in spec version and recording it in artifacts.

---

## 12) Suggested repository alignment

- Keep theory docs (`proposal-arch.md`, literature notes) as rationale.
- Treat this document as implementation blueprint.
- Next add:
  - `/spec/problem-schema.md`
  - `/spec/certificate-schema.md`
  - `/engine/pseudocode.md`
  - `/benchmarks/` for finite stabilization test cases.

This gives a clean bridge from theorem statements to a buildable system.
