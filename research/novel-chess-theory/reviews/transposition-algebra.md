# Review: transposition algebra

## Verdict

**REVISE.** The core rooted-graph calculation is sound and unusually
reproducible: the pinned corpus really projects to 7,848 vertices and 8,052
directed edges, the selected arborescence has 7,847 edges, and the resulting
205 chords attain the conditional cycle-rank lower bound. The Lean witnesses
also build. The branch nevertheless needs revision before its strongest claims
are publication-ready. It calls a presentation of root-originating paths a
category presentation, counts 296 contextual braid applications as though they
were local relations, does not emit the claimed 205-relation certificate,
turns prefix-node counts into player-card savings, contains one incorrect
curriculum partition, and omits substantial prior art for position-keyed
transposition-aware trainers.

## Validation performed

- Reran `data/classify_transpositions.py` with its pinned `chess==1.11.2`
  dependency and pinned input SHA-256. Every embedded aggregate check passed,
  and the emitted counts match `data/classify_transpositions.output.txt`:
  8,646 histories, 7,848 repetition keys, 8,052 projected edges, 798 history
  excess, 193 merge vertices, and 205 chords.
- Independently reconstructed the projected graph and its shortest-shortlex
  arborescence. It has 7,847 tree edges and 205 non-tree edges; the merge
  indegrees are exactly `181 x 2` and `12 x 3`; the root has no incoming edge;
  and `8052 - 7848 + 1 = 205`. Every reconstructed canonical path replayed
  legally and reached its claimed exact key (zero replay or endpoint failures).
- Recomputed the selected basis classification: 201 equations preserve length
  and UCI multiset, three preserve length but not the multiset, one changes
  length, and 84 are direct alternating braids. The `86` and `138` short-tail
  totals and 2,112-token total also reproduce.
- Audited `direct_catalog_braids` at the local redex rather than at the complete
  history. The reported 296 edges contain only **87 distinct local prefix braid
  pairs**. The other 209 are right-context/suffix propagation of those pairs;
  one local pair occurs in as many as 16 complete-history contexts.
- Cross-tabulated the proposed exercise buckets. The 205 chords divide into 84
  direct braids that are also at most three plies per side, two additional
  non-braids at most three plies per side, 52 further relations with four or
  five plies per side, and 67 longer relations.
- Ran `lake build`; all 41 jobs completed successfully, including the
  Caro-Kann route-substitution and Catalan/Bogo unequal-length witnesses.
- Checked the authoritative rules/source boundary. FIDE Article 9.2.3 supports
  the four repetition-key fields, and Fishburn does describe an opening book as
  a position DAG. The Flanagan--Godefroid DOI used here is a valid SIGPLAN
  Notices record of the POPL paper, although the proceedings record has a
  different parent DOI.
- Checked the current trainer implementation. It contains 21 embedded SAN lines
  and uses earliest exact-prefix matching as described. The sibling
  `opening-decisions` pilot also supplies the more relevant decision-node
  counts: White `3091 -> 2741` and Black `3102 -> 2736`.

## Required fixes

### 1. Scope the 205 theorem to the rooted path language, not the whole path category

- **Location:** `index.md:3-10,34-39`; `formalism.md:17-21,113-162`;
  `pilot.md:69-112,184-198`
- **Severity:** Error
- **Finding:** The path-induction argument is correct for paths whose source is
  the distinguished root. It does not present the full thin reachability
  category: an equation `tau(u);e ~= tau(v)` has source at the root, and
  category-congruence closure does not in general let one cancel `tau(x)` to
  identify arbitrary parallel paths `p,q : x -> y`. The lower bound is likewise
  conditional on retaining every concrete move arrow, generating every
  root-path endpoint equality, and counting each concrete parallel-path
  equation as one generator. It is not a lower bound for simply deduplicating
  the 798 observed prefixes, for a macro/schema language, or for a presentation
  of all hom-sets. The detailed pilot states most of these conditions, but the
  headline “finite-presentation theorem,” “typed path presentation,” and
  “exactly 205 independent ... relations” still invite the stronger reading.
- **Suggested fix:** Name the result the **rooted-path normalization theorem**
  and state its presentation model in the headline. Say that 205 is minimal for
  concrete endpoint-sound equations on all root-originating paths of this fixed
  edge-retaining graph. Reserve “presentation of the path category” or “thin
  reachability category” for a theorem that handles arbitrary sources and
  recompute the required cardinality if that stronger object is pursued.

### 2. Publish the 205 equations and make the graph certificate executable

- **Location:** `index.md:101-107`; `pilot.md:79-92`;
  `data/classify_transpositions.py:224-345`;
  `data/classify_transpositions.output.txt`
- **Severity:** Gap
- **Finding:** The script constructs canonical paths and chords but discards
  them, preserving only aggregate counts. It also does not explicitly replay
  every recombined canonical path or every chord side and assert the advertised
  endpoint. Those checks pass in an independent reconstruction, but a count-only
  output is not a machine-checkable “205-relation basis” and cannot support
  audit of independence, classification, or human explanations. The statement
  that the script regression-checks “every quantitative claim” is broader than
  what the output proves.
- **Suggested fix:** Emit a deterministic JSON/TSV certificate containing, for
  each chord, stable source/target keys, edge label, canonical left and right
  UCI paths, divergent tails, trace signatures, and current classification.
  Replay both sides, assert legality and exact endpoint equality, assert all
  vertices are reached, and verify that the chord boundaries form the declared
  fundamental-cycle basis. Pin the certificate hash and either connect it to
  the proposed Lean theorem or label the theorem as prose-checked pending that
  connection. Narrow “every quantitative claim” to the aggregates actually
  checked.

### 3. Separate local braid generators from contextual braid applications

- **Location:** `pilot.md:140-155`;
  `data/classify_transpositions.py:131-155,210-223,322-325`;
  `data/classify_transpositions.output.txt:13-16`
- **Severity:** Error
- **Finding:** `direct_catalog_braids` stores pairs of complete histories. If
  the same local `abc <-> cba` prefix is followed by one, two, or many shared
  suffixes, each suffix context becomes a new edge. Consequently, 296 is a
  correct count of catalog-visible **contextual history-pair applications**, but
  not of distinct local relations or algebraic generators. There are only 87
  distinct local prefix pairs in this run, with 209 additional suffix-propagated
  instances. Calling all 296 “Direct braid relations” obscures exactly the
  congruence compression the branch is meant to study.
- **Suggested fix:** Report both numbers. Deduplicate at the sound local prefix
  equation for the generator count, retain 296 as the contextual-application
  count, and explain that their union-find closure collapses 296 observed
  history occurrences. Use the 87 local relations—not 296 contextual copies—as
  the input to schema discovery and explanation clustering.

### 4. Do not promote signature tests into a braid/substitution/detour decomposition

- **Location:** `index.md:34-38,41-55`; `pilot.md:9-13,114-138`;
  `data/classify_transpositions.py:286-305,334-336`
- **Severity:** Gap
- **Finding:** The classifier establishes only two invariants: equality of
  `(length, UCI multiset)` and equality of length. Thus 201 chords are
  **signature-compatible**, not shown to be braid-generated or even generated
  by arbitrary legal reordering. “Same length but different UCI multiset” is a
  useful syntactic flag, not by itself a chess explanation of route
  substitution; unequal length is not by itself proof that a particular
  removable detour is the mechanism. The actual four exceptional chord pairs
  are inspectable and consistent with the prose, but they are absent from the
  output, and the two Lean examples do not certify all four selected chords.
  Moreover, `201/3/1` is a property of this chosen shortest-shortlex
  arborescence, not an invariant decomposition of the graph.
- **Suggested fix:** Call the 201 pairs “signature-compatible, derivation not
  yet established,” and call the other four “syntactic non-permutation
  candidates” until classified. Export all four witnesses, give their SAN/UCI
  paths and chess explanation, and identify exactly which exported chord each
  Lean theorem certifies. State prominently that changing the arborescence can
  change these class counts. Use “measured braid/substitution/detour
  decomposition” only after the proposed legal-braid search and explicit
  four-witness review have run.

### 5. Fix the exercise-bucket arithmetic and ordering

- **Location:** `player-training.md:76-78`
- **Severity:** Error
- **Finding:** “84 direct braids, then the 54 additional relations ... beyond
  the 86 very short relations” mixes overlapping sets. The direct braids are a
  subset of the 86 very-short relations. Also, `138 - 86 = 52`, not 54. The
  exact disjoint cross-tab is: 84 direct-and-very-short, two other very-short,
  52 additional four-to-five-ply-per-side relations, and 67 longer relations.
- **Suggested fix:** Use those four disjoint buckets, and make the classifier
  emit their cross-tab so the curriculum cannot drift independently of the
  analysis.

### 6. Replace “798 cards saved” with the decision-node quantity actually measured

- **Location:** `player-training.md:3-16,42-58,183-200`
- **Severity:** Gap
- **Finding:** The 8,646 histories include the root, both players' turns,
  terminal catalog prefixes, and prefixes at which a personal repertoire would
  never schedule a learner decision. They are not 8,646 instantiated cards.
  Therefore `8646 -> 7848` is a valid quotient of all catalog-prefix nodes, but
  “removes 798 ... history cards” and “the corpus establishes state
  deduplication” overstate its player meaning. The sibling pilot already
  computes the appropriate corpus-relative decision-node reductions: White
  `3091 -> 2741` (350, 11.32%) and Black `3102 -> 2736` (366, 11.80%), still
  before adding deviation/relation cards. Exact repetition identity also proves
  equal legal continuations, but “ordinary position evaluation agree” is true
  only for evaluations that factor through that key; clocks, repetition
  history, route frequency, and pre-merge risks do not.
- **Suggested fix:** Report 798 as **duplicate prefix occurrences**. For player
  savings, use side-specific decision nodes and then instantiate the complete
  `NodeReview`/`RelationReview`/`DeviationReview` schema to count actual records
  and review cost. Replace the evaluation sentence with a factorization rule:
  share only annotations proved to depend on the repetition position, while
  keeping clock/history-sensitive engine values and route-specific evidence on
  their richer keys.

### 7. Add existing transposition-aware training systems to the novelty boundary

- **Location:** `index.md:24-39,61-73`; `player-training.md:38-40,152-181`;
  `sources.md`
- **Severity:** Gap
- **Finding:** The literature ledger covers opening DAGs but omits direct
  practitioner prior art for the branch's highest-ranked player feature.
  [Chess Position Trainer](https://www.chesspositiontrainer.com/index.php/en/features)
  has long advertised a position database, a scheduler, flash-card training,
  and cross-opening transposition detection; its 2012/2014 manuals predate this
  work. Current systems such as
  [GambitLab](https://gambit-lab.com/) explicitly key scheduled cards by board
  position so transpositions do not duplicate work. Thus position-keyed node
  cards and visible transposition deduplication are not the new point. The
  plausible novelty is narrower: an explicit minimum rooted relation basis,
  route/deviation exercises derived from it, and controlled evidence of
  transfer beyond a strong position-keyed baseline.
- **Suggested fix:** Add these systems and any comparable ChessTempo/Bookup/
  Chessable behavior to `sources.md`, with versions, dates, and exact feature
  boundaries. Rewrite the novelty claim around the relation certificate and
  exercise intervention. Treat a mature position-keyed trainer as the baseline
  rather than comparing only against exact-prefix line drilling. Preserve the
  cautious “scoped search” wording and publish the databases, queries, dates,
  and inclusion decisions before making any priority claim.

### 8. The 30-position trial is a feasibility pilot, not yet a decisive falsifier

- **Location:** `index.md:63-67`; `player-training.md:160-181`
- **Severity:** Gap
- **Finding:** Thirty positions divided among three conditions leaves roughly
  ten items per condition in what appears to be an N-of-1 study. No participant
  count, item-cluster randomization, condition-order counterbalancing, power or
  smallest-effect target, primary endpoint, analysis model, or contamination
  control is specified. Learning a relation in one condition can directly teach
  a supposedly held-out transposition in another. Subsequent game quality is
  especially uninterpretable at this scale. The design can expose usability
  failures, but absence of an effect would not reliably falsify the training
  hypothesis.
- **Suggested fix:** Either label this an N-of-1 feasibility pilot and limit its
  conclusions accordingly, or power an inferential repeated-measures study on
  one preregistered primary outcome. Assign disjoint transposition components,
  prevent cross-condition endpoint overlap, counterbalance opening families,
  preserve a genuinely unseen-route test, define the analysis and missing-data
  plan, and compare relation prompts against the best position-keyed baseline.
  Report review time and route errors alongside recall so apparent card
  compression cannot hide extra cognitive cost.

### 9. Do not call the opening-search proposal POR without a preservation theorem

- **Location:** `formalism.md:29-53`; `index.md:61-68`;
  `sources.md:81-89`
- **Severity:** Gap
- **Finding:** Flanagan and Godefroid study interleavings of concurrently
  enabled transitions under an independence relation. This branch's own
  alternation obstruction says that adjacent single-ply independence is empty.
  An alternating braid instead relates different choices made across both
  players' turns; pruning one route can remove adversarial deviations and alter
  strategy or value. The DPOR citation therefore supplies an analogy, not
  evidence that “certified partial-order reduction” is technically available
  for this game graph.
- **Suggested fix:** Define the explored object (histories, policies, or game
  states), the observational equivalence, and the property to preserve
  (reachability, deviation language, minimax value, or repertoire coverage).
  Prove a reduction theorem that respects alternating choice before using POR
  terminology. Until then call the idea **relation-guided graph exploration**
  or ordinary transposition memoization plus route analysis, and rank its
  credibility as unestablished.

## Optional improvements

1. **Canonical-backbone optimization:** `player-training.md:101-117` gives a
   useful cost sketch, but choosing incoming edges independently need not
   minimize total relation-tail or review cost. Define the global objective,
   constraints, and algorithm, and report sensitivity to the arborescence.
2. **Threshold provenance:** The 70%/four-braid kill rule in `index.md:66` and
   `pilot.md:177-182` is a reasonable pilot default, not an evidence-derived
   threshold. Label and preregister it as such, and show conclusions across a
   small threshold grid.
3. **Exact-key language:** Replace “clean castling rights” in
   `player-training.md:33` with “historical castling rights represented
   exactly.” State explicitly that a hash is only an index and equality must
   compare the complete semantic key, as the Lean implementation already does.
4. **Bibliographic consistency:** The Flanagan--Godefroid link uses the
   `10.1145/1047659.1040315` SIGPLAN Notices record while the prose cites POPL;
   either label that venue or use the POPL proceedings DOI
   `10.1145/1040305.1040315`. Generate or mechanically check duplicated local
   reference blocks against `sources.md`.
5. **Formalization priority:** Formalizing the generic rooted theorem and
   checking the exported graph certificate would materially strengthen the
   result. Formalizing “alternation obstruction” alone would add much less
   assurance because that argument is elementary and not where the current
   evidentiary risk lies.

## Pass condition

Pass after fixes 1--5 make the theorem, certificate, and braid taxonomy denote
exactly what the code computes; fixes 6--8 separate structural compression from
player benefit and place the intervention against real prior art; and fix 9
either supplies a game-appropriate preservation statement or downgrades the POR
claim. The 205 rooted-basis result, exact-state handling, reproducibility, and
Lean witness examples are strong enough to retain.
