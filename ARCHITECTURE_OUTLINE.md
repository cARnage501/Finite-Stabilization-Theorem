# Architecture Outline: Deterministic Abductive Fixed-Point Reasoning System

## 1. Purpose and scope

This document specifies an implementable architecture for a deterministic, abductive, fixed-point reasoning engine.

The target system repeatedly:

1. proposes candidate hypotheses,
2. filters hypotheses against observations and consistency constraints,
3. computes bounded consequences for the viable hypotheses,
4. aggregates skeptical commitments,
5. stops at stabilization.

The architecture is designed so that each transition is auditable, testable, deterministic, and independently verifiable.

---

## 2. Design goals

- **Determinism**: the same canonical input state and specification must produce the same next state.
- **Finite stabilization**: under finite assumptions, the trajectory reaches a fixed point in finitely many steps.
- **Abductive narrowing**: the system maintains and shrinks a hypothesis set based on consistency with observations and commitments.
- **Skeptical acceptance**: conclusions are accepted only when they survive across all viable hypotheses.
- **Verification separation**: generation and checking remain separable.
- **Replayability**: independent re-execution of the step relation must reproduce the same outputs and hashes.

---

## 3. Core formal state model

### 3.1 State and specification

At iteration `n`, the dynamic state is:

- `State_n = (Gamma_n, Xi_n, Meta_n)`
  - `Gamma_n`: accepted commitments at iteration `n`
  - `Xi_n`: surviving hypotheses at iteration `n`
  - `Meta_n`: diagnostics and telemetry (`n`, hashes, timings, proof artifacts, conflict sets)

The static problem specification is:

- `Gamma0`: initial commitments
- `Xi0`: initial hypothesis pool
- `Theta(H)`: deterministic hypothesis-to-theory expansion
- `Obs`: observations
- `Q`: finite bounded query vocabulary
- `Rules`: deterministic Horn or Horn-like closure rules
- `IC`: integrity constraints
- `Profile`: semantic profile identifier and execution flags

All closure outputs are subsets of `Q` after canonicalization.

### 3.2 Normative transition function

The transition function is total and functional.

#### Step 1: filter viable hypotheses

- `Xi_{n+1} = { H in Xi_n | Consistent(Gamma_n ∪ Theta(H), Obs, IC) }`

Here `Consistent(T, Obs, IC)` is a deterministic meta-level predicate. Integrity constraints are checked as constraints, not silently treated as ordinary closure facts unless the profile says otherwise.

#### Step 2: compute bounded closures for viable hypotheses only

For each `H in Xi_{n+1}`, define:

- `C_n(H) = Closure_Q(Gamma_n ∪ Theta(H) ∪ Rules)`

with `C_n(H) ⊆ Q`.

#### Step 3: aggregate skeptical commitments

- if `Xi_{n+1} != ∅`, then `Skeptical_n = ⋂_{H in Xi_{n+1}} C_n(H)`
- if `Xi_{n+1} = ∅`, then `Skeptical_n = ∅`

and:

- `Gamma_{n+1} = Gamma0 ∪ Skeptical_n`

This empty-set convention is normative. No host-language default for empty intersections is permitted.

### 3.3 Termination condition

Stop when:

- `(Gamma_{n+1}, Xi_{n+1}) == (Gamma_n, Xi_n)`

where equality is structural equality over canonicalized sets.

### 3.4 Fixed schedule rule

This architecture fixes exactly one update schedule:

1. compute `Xi_{n+1}` by filtering `Xi_n`,
2. compute `Gamma_{n+1}` from closures indexed by `Xi_{n+1}`.

No runtime choice between `Xi_n` and `Xi_{n+1}` in the `Gamma` update is compliant.

---

## 4. Component architecture

### 4.1 Input and normalization layer

**Responsibility**
- parse and normalize `ProblemSpec`
- enforce the supported finiteness profile
- canonicalize literals, rules, and hypotheses for deterministic hashing

**Outputs**
- `NormalizedSpec`
- deterministic IDs for hypotheses, rules, literals, and constraints

### 4.2 Hypothesis manager

**Responsibility**
- store `Xi_n` and hypothesis metadata
- provide deterministic iteration order
- track elimination reasons and certificates

**Outputs**
- `Xi_n`
- elimination report and certificates

### 4.3 Closure engine

**Responsibility**
- compute `Closure_Q(S)` using deterministic forward chaining
- restrict emitted conclusions to `Q`
- emit derivation traces

**Outputs**
- closure set per viable hypothesis
- derivation trace artifacts

### 4.4 Consistency and integrity checker

**Responsibility**
- decide `Consistent(T, Obs, IC)` deterministically
- produce an inconsistency witness when applicable

**Outputs**
- boolean decision
- optional conflict certificate

### 4.5 Skeptical aggregator

**Responsibility**
- compute the skeptical aggregate over closures for `Xi_{n+1}`
- apply the empty-viable-set rule deterministically
- merge with `Gamma0`

**Outputs**
- `Gamma_{n+1}`
- change set versus `Gamma_n`

### 4.6 Stabilization controller

**Responsibility**
- run the iteration loop
- detect fixed points
- enforce hard limits such as `max_iterations`

**Outputs**
- final state `(Gamma*, Xi*)`
- trajectory and convergence report

### 4.7 Independent verifier

**Responsibility**
- re-check each recorded transition against the normative step semantics
- validate eliminations, closures, and accepted skeptical conclusions
- verify the run hash-chain when enabled

**Outputs**
- pass/fail verdict
- machine-checkable verification report

---

## 5. Execution flow

1. Load and normalize `ProblemSpec`.
2. Initialize `Gamma_0 := Gamma0` and `Xi_0 := Xi0`.
3. For each iteration `n`:
   - for each `H in Xi_n`, evaluate `Consistent(Gamma_n ∪ Theta(H), Obs, IC)`
   - form `Xi_{n+1}` from the surviving hypotheses
   - for each `H in Xi_{n+1}`, compute `C_n(H) = Closure_Q(Gamma_n ∪ Theta(H) ∪ Rules)`
   - compute the skeptical core over `Xi_{n+1}`
   - set `Gamma_{n+1} = Gamma0 ∪ Skeptical_n`
   - emit an iteration artifact bundle
   - stop on fixed point
4. Run the independent verifier over the artifact bundle.
5. Publish final outputs and metrics.

---

## 6. Data contracts

Minimal JSON-like contracts:

- `hypothesis`
  - `id`, `assumptions[]`, `signature[]`, `status`
- `iteration_record`
  - `n`, `gamma_in_hash`, `xi_in_hash`, `kept[]`, `eliminated[]`, `gamma_out_hash`, `xi_out_hash`, `timings`
- `certificate`
  - `type` (`consistency`, `inconsistency`, `derivation`, `intersection`)
  - `payload`
  - `checker_version`
  - `digest`
- `run_report`
  - `n_star`, `termination_mode`, `|Gamma*|`, `|Xi*|`, trajectory stats, verifier verdict

---

## 7. Invariants and theorem hooks

The following are normative runtime assertions and verifier targets:

- `Xi_{n+1} ⊆ Xi_n`
- `Step(State_n, Spec)` is functional
- all emitted commitments belong to `Q`
- every accepted non-base commitment has derivation support across all viable closures
- stabilization is tested by structural equality over canonicalized sets
- if `Xi_{n+1} = ∅`, then `Gamma_{n+1} = Gamma0`

---

## 8. Complexity and scaling model

Per iteration, roughly:

- consistency checks: `O(|Xi_n| * C_consistency)`
- closures: `O(|Xi_{n+1}| * C_closure(|Q|, |Rules|))`
- skeptical intersection: `O(total closure size over viable hypotheses)`

Total runtime:

- `T_total = Σ_{n=0}^{n*-1} T_step(n)`

Practical levers:

- cache closures by safe keys such as `(gamma_hash, hypothesis_id, rules_hash)`
- prune before full closure
- partition hypotheses by signatures
- parallelize local checks while preserving deterministic reduction order

---

## 9. Reference MVP design

**MVP constraints**
- function-free Horn rules
- finite `Q`
- finite `Xi0`
- deterministic contradiction testing
- JSON artifact emission per iteration

**MVP modules**
- `spec_parser`
- `closure_engine_horn`
- `consistency_checker`
- `loop_controller`
- `artifact_writer`
- `independent_checker`

**MVP acceptance criteria**
- reproduces the same `(Gamma*, Xi*, n*)` across repeated runs
- emits full elimination reasons
- handles `Xi_{n+1} = ∅` exactly as specified
- uses the fixed post-filter aggregation schedule
- allows the verifier to confirm every transition independently

---

## 10. Roadmap

### Phase 1 — Formal / executable core
- implement the deterministic loop and runtime invariants
- establish fixed-point detection and trajectory logging
- map theorem claims to executable assertions

### Phase 2 — Verification hardening
- add certificate schemas and checker CLI
- produce a tamper-evident hash-chain across iterations
- add replay mode for third-party verification

### Phase 3 — Richer semantics
- support well-founded or alternating fixed-point variants
- add abductive minimality criteria
- add alternative semantics only as separately versioned profiles, not runtime ambiguities

### Phase 4 — Robustness and sensitivity
- perturb `Obs`
- measure `Δn*`, `Δ|Xi*|`, and `Δ|Gamma*|`
- characterize stability envelopes

---

## 11. Risks and mitigations

- **State explosion in hypothesis space**  
  Mitigate with signature-based pruning, conflict-directed elimination, and bounded case exploration.

- **Semantic drift between generator and verifier**  
  Mitigate with one normative transition definition, strict certificate schemas, and checker-first contract tests.

- **Non-termination outside the finite fragment**  
  Mitigate with explicit profile flags and rejection of unsupported specifications in deterministic mode.

- **Underspecified semantics**  
  Mitigate by fixing the update schedule and the empty-family behavior in the specification itself.

---

## 12. Suggested repository alignment

- Keep theory documents as rationale.
- Treat this document as the implementation blueprint.
- Next add:
  - `/spec/problem-schema.md`
  - `/spec/certificate-schema.md`
  - `/engine/pseudocode.md`
  - `/benchmarks/` with finite stabilization test cases

This gives a direct bridge from theorem statements to an executable, verifier-friendly system.
