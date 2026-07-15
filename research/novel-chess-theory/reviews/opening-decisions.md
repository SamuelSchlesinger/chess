# Review: opening decisions

## Verdict

**REVISE.** This is a strong research-program draft: it separates formal,
empirical, and cognitive claims unusually well; its pilot is reproducible; and
its novelty language is appropriately conditional. The reported aggregates are
accurate. The central remaining problems are semantic rather than arithmetic:
the pilot's projection-matched histories are called “controllable,” an
across-ply union of child positions is allowed to resemble deviation-language
inclusion, a strict Pareto relation is called a preorder, and provisional
trainer/repertoire choices are more specific than the evidence supports.

## Validation performed

- Reran `data/pilot.py` with its pinned `chess==1.11.2` dependency and pinned
  input hash. Its stdout is byte-for-byte identical to `data/output.txt`, and
  all embedded aggregate checks pass.
- Recomputed the displayed arithmetic: `350 / 3091 = 11.32%`, `366 / 3102 =
  11.80%`, `270 + 265 = 535`, and `247 + 220 = 467` after rounding exactly as
  the script does.
- Checked the route-pair and hub implementations against their prose. The
  reported `270/265`, `247/220`, `32/98`, `41/46`, and top-hub `577/2/577`
  values are faithful to the code's current definitions. The statement about
  six-to-eight-route hubs with dozens of descendants is also supported by the
  output.
- Checked the current trainer source: it contains exactly 21 embedded lines and
  selects the earliest prefix-compatible continuation, as stated.
- Spot-checked the primary literature underlying the important quantitative
  prose. Chassy and Gobet do estimate about 100,000 memorized opening moves;
  Chowdhary, Iacopini, and Battiston analyze more than 120 million games; and
  the descriptions of Bilalic et al., De Marzo and Servedio, Walczak, Hyatt,
  Levene and Bar-Ilan, Munshi, and Chvatal match the cited papers. No citation
  correction is required from this review.
- The remaining numbers—20 plies, the rating bands, 90% target retention, five
  selected route pairs, and 40–80 cards—are proposed design parameters, not
  validated findings. They need to be labeled and preregistered as such.

## Required fixes

### 1. Do not call the pilot pairs “controllable”

- **Location:** `data/pilot.py:236-281`; `experiments.md:16-26`;
  `index.md:65-70`
- **Severity:** Gap
- **Finding:** The code proves that two equal-length histories have the same
  endpoint and the same raw UCI projection for the opponent while the selected
  side's projection differs. It does not construct the conditional player
  policy required by `experiments.md:78-85`, nor does it show that a fixed
  state-dependent opponent policy induces both routes. “Controllable” therefore
  reads more strongly than the test. At most these are paired recorded histories
  under an identical open-loop opponent move script.
- **Suggested fix:** Rename them **same-endpoint, opponent-projection-matched
  history pairs** throughout the pilot and Gate 1. Reserve “controllable route
  pair” for Gate 2 candidates for which an explicit branching policy and common
  opponent-scenario semantics have been constructed.

### 2. The `41/46` inclusion statistic is not deviation-language inclusion

- **Location:** `data/pilot.py:94-134,242-281`; `formal-model.md:67-84`;
  `index.md:68-78`
- **Severity:** Error
- **Finding:** `catalog_outcomes` unions child position keys from every opponent
  decision on a route into one untyped set. This discards the decision index,
  pre-deviation state, move, and prefix, and it deduplicates outcomes that
  transpose. A proper subset can therefore be caused by cross-ply coincidences
  and cannot be read as inclusion between opponent words, first-deviation
  events, or preparation failures. Likewise, `catalog_count` and `legal_count`
  are sums of per-node branch incidences, not counts of unique opponent
  alternatives. The prose does say “coarse,” but placing this beside the formal
  language program still invites the stronger inference.
- **Suggested fix:** Either remove the inclusion count from the evidentiary
  headline, or represent an event at least as `(opponent decision index,
  pre-deviation key, move, child key)` and compare events in an explicitly
  aligned scenario universe. Call the count statistics “summed per-decision
  alternative incidences.” State explicitly that the present `41/46` result is
  not evidence of dominance; the full bounded-language experiment must supply
  that evidence.

### 3. Strict dominance is not a preorder

- **Location:** `formal-model.md:86-104,123-140`; `index.md:162-176`
- **Severity:** Error
- **Finding:** Componentwise `<=` with “at least one strict relation” is the
  strict Pareto relation. It is irreflexive, so it is not a preorder. The same
  issue appears in practical dominance. Calling adversarial move-order
  dominance a “preorder” is therefore false under the stated definition, even
  if the intended transitivity theorem is true.
- **Suggested fix:** Define weak dominance using only the non-strict component
  relations and prove that it is a preorder. Define strict dominance as weak
  dominance plus failure of the converse (or one strict component under a true
  product order), and characterize it as a strict partial order after quotienting
  any component preorders as needed.

### 4. Corridors and opponent words need common comparison semantics

- **Location:** `formal-model.md:60-84,86-104,259-274`
- **Severity:** Gap
- **Finding:** A prepared corridor is defined as following `pi`, so policies
  with different own moves generally induce different corridors, yet dominance
  and its proposed theorem refer to an unindexed or “fixed” corridor. The legal
  opponent-word languages also depend on the policy: the same UCI word can be
  illegal on one route or denote moves from different states. Avoiding an
  opponent option by making it illegal may be a genuine move-order benefit, but
  the model must say whether that is an absent scenario, a successfully blocked
  scenario, or an object compared through a common global opponent policy.
- **Suggested fix:** Index corridors (`C1`, `C2`) and define a common typed
  scenario space before stating inclusion. One clean option is to use global
  opponent policies as the common scenarios and project their induced plays to
  route-specific failure events. If raw opponent words are retained, define the
  alphabet, alignment, legality convention, and treatment of words legal on
  only one route. Update the Lean theorem statement before implementation.

### 5. The `11.32%/11.80%` result is node deduplication, not card or memory compression

- **Location:** `data/pilot.py:217-233`; `index.md:58-87`;
  `formal-model.md:242-257`
- **Severity:** Gap
- **Finding:** The numerator deduplicates corpus prefixes with an outgoing edge
  by repetition key. It excludes terminal prefixes, does not include the
  learner's cards for opponent deviations, and assigns unit cost regardless of
  accepted moves, explanations, plans, route exceptions, or conflicting
  continuations. This is a valid database-node differential, but “save 350
  cards” suggests an instantiated trainer and cognitive cost model that do not
  yet exist. The later caveat about delayed recall does not repair the earlier
  unit mismatch.
- **Suggested fix:** Report **history decision nodes versus unique repetition
  keys**, or “potential duplicate decision records,” and reserve “cards saved”
  for a conversion through the proposed mixed card schema. In Gate 2, report
  position decisions, deviation cards, transfer cards, plans, and route
  exceptions separately, with a sensitivity analysis over their costs.

### 6. Preserve a genuinely untouched test set and define the risk estimands

- **Location:** `experiments.md:54-58,76-89,107-134,171-180`
- **Severity:** Gap
- **Finding:** Candidate generation currently requires held-out support, while
  the “top five” selection rule and ranking data are unspecified. That risks
  using the evaluation window for selection. “Expected engine regret from
  supported deviations” also lacks a precise player-perspective baseline,
  reach-probability definition, unit, and rule preventing multiple counting
  after a first exit. Finally, “a practically chosen minimum number of cards”
  is not a falsifier until the minimum is declared in advance.
- **Suggested fix:** Use train data for move probabilities, a validation period
  for route/threshold selection, and an untouched chronological test period for
  the published comparison. Preregister support, danger, soundness, stability,
  and minimum-saving thresholds. Define regret's sign, baseline, horizon, score
  normalization, first-exit handling, uncertainty interval, and aggregation
  before running the engine. Select the five pairs without inspecting test
  outcomes.

### 7. Gate 3 is not yet a publication-grade randomized study

- **Location:** `experiments.md:136-179`
- **Severity:** Gap
- **Finding:** The design gives card counts and test dates but no participant
  count, power calculation, primary endpoint, analysis model, or multiplicity
  plan. A within-player crossover is especially vulnerable to carryover here:
  learning a graph card can directly teach the same transposition later shown
  in the line condition. Opening families can also share downstream positions,
  violating the intended separation.
- **Suggested fix:** Declare whether this is an N-of-1 product pilot or an
  inferential human study. For the latter, power the study on one preregistered
  primary outcome, cluster assignments by disjoint transposition components,
  counterbalance condition order, prevent cross-condition position overlap,
  specify the repeated-measures model and missing-data treatment, and report
  confidence intervals plus all prespecified secondary outcomes. For an N-of-1
  pilot, make no population-level novelty claim.

### 8. The provisional repertoire does not follow from the pilot

- **Location:** `index.md:137-160`; `trainer-design.md:112-135`
- **Severity:** Gap
- **Finding:** The taxonomy supports using the `d4/c4/Nf3` region as a
  catalog-dense structural test bed. It does not establish encounter frequency,
  soundness, recall cost, stylistic fit, or superiority. In particular, the
  Caro-Kann choice receives no support from the reported pilot, and “it is
  sound, structurally coherent” is an uncited chess judgment. Calling the list a
  hypothesis is good calibration, but saying the product can start with this
  backbone still turns an editorial choice into a recommendation.
- **Suggested fix:** Present these openings only as deliberately chosen test
  fixtures, with provenance and selection criteria, until Gate 2 and personal
  game data exist. Remove the Caro-Kann recommendation or identify it explicitly
  as an author/player preference unrelated to the pilot. Promote a repertoire
  only after reporting cohort mass, engine evidence, unique-card/exception cost,
  player preference, and held-out stability.

### 9. Scheduler constants and the multiplicative priority are unevidenced heuristics

- **Location:** `trainer-design.md:79-99`
- **Severity:** Gap
- **Finding:** The cited scheduling paper supports modeling review scheduling as
  an optimization problem; it does not validate 90% retention for chess cards or
  this five-factor product. The factors have unspecified scales and zero
  behavior, so changing units or setting any factor to zero can arbitrarily
  change the ranking. The “small” rare-severe exploration quota is also
  undefined.
- **Suggested fix:** Label 90%, the product, and the quota as tunable pilot
  defaults rather than evidence-backed policy. Define normalization, floors,
  missing values, and the exploration rate; log every component; and preregister
  an ablation or calibration rule before claiming scheduling benefit.

## Optional improvements

1. **Novelty audit:** `literature-and-novelty.md:3-7,44-55` is commendably
   cautious. Preserve the exact database names, query strings, result counts,
   inclusion/exclusion decisions, and search date in a machine-readable log.
   Before a priority claim, add practitioner systems, patents, theses, ICGA
   archive coverage, and non-English move-order literature as the document
   already anticipates.
2. **Hub terminology:** `data/pilot.py:306-333` counts distinct terminal
   histories below all representatives, not quotient-level reusable knowledge;
   it is heavily shaped by taxonomy granularity. Rename it “taxonomy hub score”
   and always display multiplicity and terminal-history count separately, as the
   output already does.
3. **State the scope of the exact key:** The repetition key is appropriate for
   static reusable annotations, but the trainer schema should make explicit
   which cards require complete FIDE history/clock state and which provably
   factor through the key.
4. **Copy edit:** `trainer-design.md:3-4` currently says “for the existing the
   monorepo's,” which should be “for the monorepo's.”
5. **Source maintenance:** Keep `sources.md` as the canonical ledger and
   generate or mechanically check duplicated local reference blocks so titles,
   years, and URLs cannot drift.

## Pass condition

Pass after fixes 1–5 make the mathematical and pilot claims denote exactly what
the code computes, fixes 6–7 make the empirical claims identifiable, and fixes
8–9 clearly separate experimental defaults from chess recommendations. The
underlying research direction, reproducibility discipline, citations, and
proof/evidence boundary are already strong enough to retain.
