"""Reference transition module for the repository's normative update equations.

Implements a deterministic single-step transition
    (Γ_n, Ξ_n) -> (Γ_{n+1}, Ξ_{n+1})
and fixed-point detection for
    n*, Γ*, Ξ*.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, Iterable, Mapping, Sequence, Tuple

Literal = str
Hypothesis = str


def neg(lit: Literal) -> Literal:
    """Return the negated form under the repo convention (prefix '¬')."""
    return lit[1:] if lit.startswith("¬") else f"¬{lit}"


def _canon(values: Iterable[str]) -> Tuple[str, ...]:
    """Deterministic set representation for reproducible traces."""
    return tuple(sorted(set(values)))


@dataclass(frozen=True)
class Gamma:
    """Accepted conclusions Γ."""

    literals: Tuple[Literal, ...]

    @staticmethod
    def from_items(items: Iterable[Literal]) -> "Gamma":
        return Gamma(_canon(items))

    def as_set(self) -> set[Literal]:
        return set(self.literals)


@dataclass(frozen=True)
class Xi:
    """Surviving hypotheses Ξ."""

    hypotheses: Tuple[Hypothesis, ...]

    @staticmethod
    def from_items(items: Iterable[Hypothesis]) -> "Xi":
        return Xi(_canon(items))

    def as_set(self) -> set[Hypothesis]:
        return set(self.hypotheses)


@dataclass(frozen=True)
class Obs:
    """Observation literals Obs."""

    literals: Tuple[Literal, ...]

    @staticmethod
    def from_items(items: Iterable[Literal]) -> "Obs":
        return Obs(_canon(items))

    def as_set(self) -> set[Literal]:
        return set(self.literals)


@dataclass(frozen=True)
class Q:
    """Query vocabulary Q (atoms, not negated literals)."""

    atoms: Tuple[str, ...]

    @staticmethod
    def from_items(items: Iterable[str]) -> "Q":
        return Q(_canon(items))

    def literal_universe(self) -> set[Literal]:
        atoms = set(self.atoms)
        return atoms | {neg(a) for a in atoms}


@dataclass(frozen=True)
class HornRule:
    """Horn clause antecedents -> consequent."""

    antecedents: Tuple[Literal, ...]
    consequent: Literal

    @staticmethod
    def of(antecedents: Iterable[Literal], consequent: Literal) -> "HornRule":
        return HornRule(tuple(antecedents), consequent)


Theta = Mapping[Hypothesis, Tuple[Literal, ...]]


@dataclass(frozen=True)
class StepResult:
    gamma_next: Gamma
    xi_next: Xi


@dataclass(frozen=True)
class FixedPointResult:
    n_star: int
    gamma_star: Gamma
    xi_star: Xi
    trajectory: Tuple[Tuple[int, Gamma, Xi], ...]


def closure_Q(S: Iterable[Literal], rules: Sequence[HornRule], q: Q) -> Gamma:
    """Forward Horn closure restricted to Q-literals."""
    allowed = q.literal_universe()
    closed = set(S) & allowed

    changed = True
    while changed:
        changed = False
        for rule in rules:
            if rule.consequent not in allowed:
                continue
            if set(rule.antecedents).issubset(closed) and rule.consequent not in closed:
                closed.add(rule.consequent)
                changed = True

    return Gamma.from_items(closed)


def entails_bottom_Q(S: Iterable[Literal], q: Q) -> bool:
    """S ⊢_Q ⊥ iff there exists u in Q with {u, ¬u} ⊆ S."""
    facts = set(S)
    for u in q.atoms:
        if u in facts and neg(u) in facts:
            return True
    return False


def transition_step(
    gamma_n: Gamma,
    xi_n: Xi,
    gamma0: Gamma,
    theta: Theta,
    obs: Obs,
    q: Q,
    rules: Sequence[HornRule],
) -> StepResult:
    """Execute exactly one deterministic transition step.

    Normative schedule:
      * Ξ-update uses Γ_n and Obs.
      * Γ-update uses Ξ_{n+1}, not Ξ_n.
      * If Ξ_{n+1} is empty, the skeptical core is empty.
    """
    gamma_n_set = gamma_n.as_set()
    obs_set = obs.as_set()

    # Ξ_{n+1} = {H in Ξ_n | Γ_n ∪ Θ(H) ∪ Obs ⊬_Q ⊥}
    xi_next_items = []
    for h in xi_n.hypotheses:
        support = set(theta.get(h, ()))
        if not entails_bottom_Q(gamma_n_set | support | obs_set, q):
            xi_next_items.append(h)
    xi_next = Xi.from_items(xi_next_items)

    # Γ_{n+1} = Γ_0 ∪ ⋂_{H in Ξ_{n+1}} Cn^Q(Γ_n ∪ Θ(H)), with empty core = ∅.
    closures = []
    for h in xi_next.hypotheses:
        support = set(theta.get(h, ()))
        closures.append(closure_Q(gamma_n_set | support, rules, q).as_set())

    skeptical_core = set.intersection(*closures) if closures else set()
    gamma_next = Gamma.from_items(gamma0.as_set() | skeptical_core)
    return StepResult(gamma_next=gamma_next, xi_next=xi_next)


def run_to_fixed_point(
    gamma0: Gamma,
    xi0: Xi,
    theta: Theta,
    obs: Obs,
    q: Q,
    rules: Sequence[HornRule],
    max_steps: int = 10_000,
) -> FixedPointResult:
    """Iterate transitions until X_{n+1} == X_n and return n*, Γ*, Ξ*."""
    gamma_n = gamma0
    xi_n = xi0
    trajectory: list[Tuple[int, Gamma, Xi]] = [(0, gamma_n, xi_n)]

    for n in range(max_steps):
        step = transition_step(gamma_n, xi_n, gamma0, theta, obs, q, rules)
        gamma_next, xi_next = step.gamma_next, step.xi_next
        trajectory.append((n + 1, gamma_next, xi_next))

        if gamma_next == gamma_n and xi_next == xi_n:
            return FixedPointResult(
                n_star=n,
                gamma_star=gamma_n,
                xi_star=xi_n,
                trajectory=tuple(trajectory),
            )

        gamma_n, xi_n = gamma_next, xi_next

    raise RuntimeError(f"No fixed point found within {max_steps} steps")


if __name__ == "__main__":
    # Tiny deterministic sanity example.
    q = Q.from_items(["a", "b"])
    gamma0 = Gamma.from_items(["a"])
    xi0 = Xi.from_items(["h1", "h2"])
    theta: Dict[Hypothesis, Tuple[Literal, ...]] = {
        "h1": ("a",),
        "h2": ("a", "¬b"),
    }
    obs = Obs.from_items(["b"])
    rules = [HornRule.of(["a"], "b")]

    fp = run_to_fixed_point(gamma0, xi0, theta, obs, q, rules)
    print(fp)
