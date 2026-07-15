# Review and revision disposition

The three investigation branches were reviewed independently after phase-one
authoring. Each reviewer returned **REVISE**. The original reports are retained
unchanged as an audit trail; this file records the subsequent corpus-level
disposition rather than replacing those historical verdicts.

| Investigation | Review | Required findings | Revision disposition |
|---|---|---:|---|
| Transposition algebra | [report](transposition-algebra.md) | 9 | Addressed in the revised branch and executable certificate |
| Opening decisions | [report](opening-decisions.md) | 9 | Addressed in the revised definitions, pilot, and experiment design |
| Certified chess knowledge | [report](certified-chess-knowledge.md) | 7 | Addressed in the revised novelty boundary, protocols, and engine witness |

## Transposition algebra

The revision scopes the 205-equation result to root-originating paths in one
fixed edge-retaining graph; publishes every equation and endpoint in
`rooted_path_basis.json`; distinguishes 87 local braid relations from 296
contextual applications; and stops treating syntactic signatures as legal
derivations. It corrects the curriculum arithmetic, separates 798 duplicate
prefix occurrences from side-specific decision-node differentials, adds prior
position-keyed trainers to the novelty boundary, downgrades the 30-position
trial to feasibility work, and reserves “partial-order reduction” for a future
preservation theorem.

## Opening decisions

The revision replaces “controllable pairs” and deviation-set inclusion with the
quantities actually computed: projection-matched pairs and summed branch
incidences. It defines weak dominance and its strict part over typed common
scenarios, separates node deduplication from cognitive savings, freezes
training/validation/test splits and risk estimands, and distinguishes an N-of-1
feasibility check from a powered component study. Named openings are now
engineering fixtures rather than prescriptions, and scheduler constants are
explicitly calibration heuristics.

## Certified chess knowledge

The revision recognizes tablebase counterexample-and-repair as established
prior art and lowers its novelty score. It adds sensitivity analysis, a full
reachable-from-start en-passant history, Rust regressions for exact game counts
and the separate search heuristic, structural equality after a hash prefilter,
and disjoint assurance labels. The KPK/KPKP mining domains and kill rules are
frozen, forced-claim semantics are specified before checker soundness is
claimed, and the pinned `python-chess` oracle and artifact hash are recorded.

## Convergence checks

The root synthesis accepts a revision only when its prose agrees with its
artifacts and all monorepo checks pass together. The convergence command is:

```sh
scripts/check_all.sh
```

It verifies shared data hashes, the Lean build and corpus validator, Rust tests,
research-document structure, and the three pinned Python pilots. The root
[synthesis](../index.md) reports only claims that survive this disposition and
the full check.
