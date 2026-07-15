# Review: certified chess knowledge

## Verdict

**REVISE.** The branch is technically serious and unusually honest about the
difference between a verified mechanism and a new chess theorem. Its arithmetic
artifacts reproduce, the en-passant example really does separate FIDE repetition
identity from Polyglot hashing, and the tablebase metric cautions are broadly
accurate. Two claims nevertheless need substantial correction before this can
pass: tablebase **counterexample-and-repair is itself direct prior art**, and the
flagship trainer item is not certified end to end. The current oracle proves a
pair of hashes differ; it does not yet replay a legal game history through the
Rust repetition counters or instantiate the cited Lean theorem for the concrete
example.

The recommended program remains promising after those corrections. Its possible
novelty is a new, precisely scoped chess predicate with a checked proof,
minimality result, and demonstrated human transfer—not the generic workflow of
using tablebase counterexamples to repair human rules.

## Validation performed

- Reran `data/rank_candidates.py --check` and
  `data/state_space_bounds.py --check`. Both checked outputs match exactly. The
  former validates arithmetic over authored ordinal scores, not the evidential
  basis of those scores.
- Reran `data/repetition_ep_counterexample.py --check` with pinned
  `chess==1.11.2`. Its output matches exactly: the candidate en-passant capture
  is pseudo-legal but illegal, the effective FIDE keys are equal, and the
  Polyglot hashes differ.
- Reran `data/check_corpus.py`: seven Markdown documents are reachable, 41 local
  link targets resolve, and 46 inline citations have definitions and full local
  entries.
- Checked FIDE Articles 9.2.3 and 9.6 against the current official Laws. The
  branch correctly uses legal move availability for en-passant repetition
  identity, correctly separates claimable 50-move and automatic 75-move rules,
  and correctly notes checkmate's precedence over the 75-move rule
  [fide23][fide23].
- Checked the pinned `python-chess` Polyglot implementation. It explicitly hashes
  an en-passant file when an adjacent pawn exists and states that legality of the
  potential capture is irrelevant [python-chess112][python-chess112]. This
  matches the imported Rust engine's `ep_hash_contribution` implementation.
- Inspected `engine/src/board.rs`, `engine/src/game.rs`,
  `engine/src/search.rs`, and `engine/tests/legal_movegen.rs`. The prose correctly
  reports that the same Polyglot-compatible key feeds `Game::repetition_count`
  and search-path repetition, while the existing regression test checks only
  that the pinned en-passant capture is absent from legal move generation.
- Constructed a reachable-from-start witness that the current artifact lacks:

  ```text
  1.d4 e5 2.dxe5 Nf6 3.e4 Nxe4 4.Nf3 Nc5 5.Nc3 g6
  6.Bf4 Bg7 7.Qd2 O-O 8.h3 Re8 9.a3 d5
  10.Ng1 Nbd7 11.Nf3 Nb8
  ```

  After `9...d5`, `e5xd6 e.p.` is pseudo-legal but illegal because moving the
  e5-pawn exposes White's king on e1 to the rook on e8. The four-ply knight
  cycle returns to the same FIDE position with no raw en-passant field. Repeating
  that cycle twice gives three FIDE occurrences but only two occurrences of the
  later Polyglot key. This makes the claimed `Game` undercount concrete and
  repairable.
- Checked the current Lichess API documentation and `op1` repository. The API
  exposes nullable DTZ/DTM/DTC fields and the stated result categories; `op1`
  covers the opposing-pawns-on-one-file family and defines DTC while ignoring
  the 50-move rule [lila-tablebase][lila-tablebase] [op1][op1].
- Cross-checked the closest novelty sources. Most importantly, Guid et al. do
  not merely extract generic KBNK rules: their method repeatedly presents
  tablebase counterexamples to an expert, who may accept them or repair rule
  preconditions and goals [guid10][guid10]. Earlier work used a tablebase to
  verify and codify human KNNKP statements [herschberg89][herschberg89], and
  Jansen explicitly assessed human-facing KQKR heuristics [jansen92][jansen92].

## Required fixes

### 1. Reframe counterexample-and-repair as prior art, not a novelty-4 method

- **Location:** `route-comparison.md` → “Recommendation,” “Common rubric,” and
  “Tablebase counterexample and repair”; `tablebase-rule-mining.md` → “Research
  target” and “Publishable candidate”; `sources.md`
- **Severity:** Error
- **Finding:** The branch recognizes generic rule extraction as prior art but
  draws the boundary too narrowly. Guid et al. explicitly describe an iterative
  loop in which tablebase-derived counterexamples are shown to an expert and
  unacceptable examples cause rule preconditions or goals to be added, removed,
  or modified [guid10][guid10]. Herschberg, van den Herik, and Schoo used a
  tablebase to verify and codify Troitzky's KNNKP claims in 1989
  [herschberg89][herschberg89]. Jansen evaluated human-useful KQKR heuristics in
  1992 [jansen92][jansen92]. Therefore “tablebase counterexample and repair” is
  not plausibly a 4/5 novelty category as a methodology, and the current ranking
  gives the recommended route an unsupported advantage.
- **Suggested fix:** Add these direct predecessors and state that the workflow
  is established. Put candidate novelty only in the conjunction of a **new exact
  chess predicate**, canonical/minimal exception characterization, formal proof
  against complete declared semantics, and measured player transfer. Rescore
  the route after that correction or replace the single-point ordinal ranking
  with ranges. At minimum, show score-perturbation sensitivity: the present four
  hand-picked weight profiles cannot establish robustness of the recommendation.

### 2. Turn the en-passant hash pair into an actual end-to-end engine witness

- **Location:** `index.md` → “Planned pilots” and “Result”;
  `training-integration.md` → “Flagship theorem-to-system example”;
  `data/repetition_ep_counterexample.py`
- **Severity:** Gap
- **Finding:** The script proves that two manually supplied FENs have equal
  effective keys and unequal `python-chess` Polyglot hashes. It does not prove
  that both occur in one legal game history, exercise `engine::Game`, or observe
  either `Game::repetition_count` or search behavior. The existing Rust test
  establishes move-generation legality only. Thus “can undercount a real
  repetition in both `Game` and search” is a correct inference from the code,
  but not yet the completed worked example the index claims.
- **Suggested fix:** Replace or supplement the sparse FEN with the legal sequence
  in “Validation performed,” repeat the knight cycle twice, and add a Rust
  regression asserting FIDE count `3` where the current Polyglot counter returns
  `2`. Add a separate search regression because search intentionally treats an
  earlier twofold occurrence as a draw heuristic; do not describe that internal
  heuristic as the FIDE threefold rule. Make the checked output include the
  history, both counts, and the exact engine revision.

### 3. Do not label the concrete trainer item Lean-certified yet

- **Location:** `training-integration.md` → “Knowledge-item contract,”
  “Flagship theorem-to-system example,” and “Promotion and stop rules”;
  `index.md` → “Result”
- **Severity:** Error
- **Finding:** `Chess.RepetitionKey.ofPosition_eq_iff` proves equivalence between
  a generic Lean key and the repository's modeled relation. It is not a theorem
  about the displayed FEN pair, its reachability, the Python script, or the Rust
  counter. Naming that generic theorem beside an external checker gives the JSON
  record a stronger assurance level than it has. Likewise, a tablebase
  provenance record and an engine threshold are useful evidence, but neither is
  a checked theorem. “Certification prevents bad material” also exceeds what
  semantic certification can guarantee about explanation quality or pedagogy.
- **Suggested fix:** Add an explicit assurance field with disjoint values such
  as `lean-theorem-instance`, `checked-certificate`, `pinned-tablebase-oracle`,
  `engine-estimate`, and `human-hypothesis`. A Lean-certified item must name a
  concrete theorem or certificate digest that entails that exact answer under
  the declared semantics. Until the FEN pair has such an instance theorem,
  label it a cross-validated diagnostic. State that certification prevents a
  declared class of semantic errors, not bad curriculum or poor explanations.

### 4. Specify exact repetition equality, not merely a second 64-bit hash

- **Location:** `training-integration.md` → “Flagship theorem-to-system example”
  and “Knowledge-item contract”; `engine/src/game.rs`; `engine/src/search.rs`
- **Severity:** Gap
- **Finding:** Conditioning the en-passant component on a legal capture fixes
  this false negative, but a bare 64-bit key is still not exact identity: a hash
  collision can create a false positive. The document says exported artifacts
  need collision-checked equality, yet its engine repair recommendation only
  calls for a “distinct FIDE repetition identity.” It also groups `Game` and
  search even though their semantics differ: `Game` counts claim/termination
  occurrences, while search treats one matching ancestor as an internal draw
  condition.
- **Suggested fix:** Define a canonical repetition record containing placement,
  side, castling rights, and legally effective en passant; use its hash only for
  indexing and resolve equality structurally. If performance requires retaining
  64-bit path keys, document the residual collision assumption rather than
  calling them exact. Specify and test `Game` and search semantics separately.

### 5. Freeze the discovery domains and kill thresholds before mining

- **Location:** `tablebase-rule-mining.md` → “Counterexample-and-repair loop,”
  “Plumbing control,” and “Publishable candidate”;
  `small-material-classification.md` → “Candidate pilot” and “Falsification
  protocol”; `route-comparison.md` → “Smallest falsifying pilots”
- **Severity:** Gap
- **Finding:** The tactical mutation pilot is genuinely capable of failing, but
  the tablebase and KPKP pilots can currently move their goalposts. The corridor,
  pawn files, legality/reachability domain, starting heuristic, allowed feature
  language, acceptable rule length, and player-transfer threshold are not fixed.
  “Choose the predicate after the control,” “one fixed-file corridor,” and stop
  when the rule is “lookup-sized” permit the family or complexity standard to
  change after counterexamples are seen. A rule eventually fitted to the full
  finite domain has no untouched empirical test even if its enumeration is
  exact.
- **Suggested fix:** Before querying labels, publish the exact FEN-domain
  predicate, reachability assumptions, WDL/clock semantics, symmetry group,
  initial `H`, canonical counterexample order, feature grammar, and a numerical
  description-length or clause budget. Separate discovery strata from untouched
  transfer strata. Predeclare the player sample, primary transfer outcome, and a
  failure threshold. An exact post-hoc theorem can still be valid, but its
  novelty and cognitive simplicity must be evaluated on criteria fixed before
  the search.

### 6. Complete the FIDE forced-mate semantics before claiming checker soundness

- **Location:** `tactical-certificates.md` → “Certificate shape” and “Draw and
  identity semantics”
- **Severity:** Gap
- **Finding:** The proposed tree shape contains chess moves but no explicit draw
  claim action or terminal-priority definition. For FIDE mode, the independently
  defined force relation must account for a claim already available, a legal
  intended move that enables a threefold/50-move claim, automatic fivefold and
  75-move draws, stalemate, and dead position. The repository's exact
  `DeadPosition` is semantic rather than an immediately executable Boolean, so
  saying the current primitives are already sufficient hides a checker-design
  obligation. The history-free “position-tree mate” mode also needs to say
  whether it ignores the halfmove clock as well as repetition history.
- **Suggested fix:** Define the two force relations first, including terminal
  ordering and claim actions, then prove checker soundness against them. For
  dead positions, either derive non-deadness from the checked future mate tree,
  use a separately certified dead-position decision procedure on the supported
  domain, or state a conservative restriction. Add mutations for an at-state
  claim and an intended-move claim, not only a deleted legal defense.

### 7. Add the missing primary source for the Polyglot oracle

- **Location:** `sources.md`; `training-integration.md` → “Flagship
  theorem-to-system example”; `data/repetition_ep_counterexample.py`
- **Severity:** Gap
- **Finding:** The branch cites local Rust code for what that code does, but not
  a primary Polyglot-format or pinned oracle implementation for the claim that
  adjacent-pawn, legality-insensitive en passant is “the Polyglot convention.”
  The executable artifact depends on `chess==1.11.2`, yet that release and its
  `polyglot.py` implementation are absent from the source ledger.
- **Suggested fix:** Cite the pinned `python-chess` release and source line that
  states capture legality is irrelevant [python-chess112][python-chess112], and
  record the package artifact hash or lockfile used by the validation command.
  If the original Polyglot specification/source is relied upon, cite a pinned
  authoritative revision rather than an unversioned summary.

## Optional improvements

1. **Do not infer feasibility from placement arithmetic.** The values
   `2 * P(64,k)` are correct and clearly labeled as crude ceilings. Add one
   actual legal/symmetry-reduced KPKP count with runtime and memory before using
   the table to choose the Lean implementation strategy.
2. **Pin live tablebase evidence.** The source descriptions are current and the
   metric claims check out, but a publication pilot should record the exact
   `lila-tablebase` commit, request URL, response body, timestamp, and relevant
   backing-table provenance. Preserve nullable/rounded `dtz` versus
   `precise_dtz`; do not silently coerce missing metrics.
3. **Temper the AlphaZero transfer sentence.** The cited paper's human study
   had four grandmasters, and its authors explicitly call the result a proof of
   concept with possible priming and difficulty confounds [schut25][schut25].
   Say “a preliminary four-participant study found post-learning improvement”
   instead of an unqualified “transferred concepts to elite grandmasters.”
4. **Validate URLs as well as local citation syntax.** `check_corpus.py` is a
   useful deterministic structure check, but its name can suggest more source
   validation than it performs. Add a separate cached URL/DOI audit, or state in
   its output that external targets were not checked.
5. **Measure the theorem-to-trainer delta.** To support player usefulness, compare
   certified rule/exception cards against time-matched ordinary explanations on
   delayed boundary judgment and unseen transfer. Otherwise success could come
   from retrieval practice or minimal pairs rather than certification.
6. **Keep the good metric boundary.** Preserve the branch's distinction among
   WDL, rounded and precise DTZ, DTM, and DTC. It is one of the strongest parts
   of the corpus and should become machine-readable item metadata.

## Pass condition

Pass after fixes 1–4 correct the novelty and assurance claims, fixes 5–6 turn
the proposed research into genuinely falsifiable experiments under complete
declared semantics, and fix 7 closes the source gap for the flagship oracle.
The quantitative scripts, current FIDE interpretation, tablebase API summary,
and broad recommendation to pursue small exact player-facing predicates are
sound enough to retain.

## Local References

- **[fide23]** International Chess Federation. *FIDE Laws of Chess Taking Effect from 1 January 2023*. Approved 7 August 2022, applied 1 January 2023. https://handbook.fide.com/chapter/E012023
- **[guid10]** Matej Guid, Martin Možina, Aleksander Sadikov, and Ivan Bratko. “Deriving Concepts and Strategies from Chess Tablebases.” *Advances in Computer Games*, LNCS 6048, 195–207, 2010. https://doi.org/10.1007/978-3-642-12993-3_18
- **[herschberg89]** I. S. Herschberg, H. Jaap van den Herik, and P. N. A. Schoo. “Verifying and Codifying Strategies in the KNNKP(h) Endgame.” *ICCA Journal* 12(3), 144–154, 1989. https://doi.org/10.3233/ICG-1989-12304
- **[jansen92]** Peter J. Jansen. “KQKR: Assessing the Utility of Heuristics.” *ICCA Journal* 15(4), 179–191, 1992. https://doi.org/10.3233/ICG-1992-15402
- **[python-chess112]** Niklas Fiekas. `python-chess` v1.11.2 release and `chess/polyglot.py` Polyglot Zobrist implementation, 2025. https://github.com/niklasf/python-chess/releases/tag/v1.11.2
- **[lila-tablebase]** Lichess. `lila-tablebase` README and HTTP API documentation, inspected 14 July 2026. https://github.com/lichess-org/lila-tablebase/blob/main/README.md
- **[op1]** Lichess. `op1`: partial eight-piece tablebase probe and metric documentation, inspected 14 July 2026. https://github.com/lichess-org/op1
- **[schut25]** Lisa Schut, Nenad Tomašev, Thomas McGrath, Demis Hassabis, Ulrich Paquet, and Been Kim. “Bridging the Human–AI Knowledge Gap through Concept Discovery and Transfer in AlphaZero.” *PNAS* 122(13), e2406675122, 2025. https://doi.org/10.1073/pnas.2406675122

[fide23]: https://handbook.fide.com/chapter/E012023
[guid10]: https://doi.org/10.1007/978-3-642-12993-3_18
[herschberg89]: https://doi.org/10.3233/ICG-1989-12304
[jansen92]: https://doi.org/10.3233/ICG-1992-15402
[python-chess112]: https://github.com/niklasf/python-chess/releases/tag/v1.11.2
[lila-tablebase]: https://github.com/lichess-org/lila-tablebase/blob/main/README.md
[op1]: https://github.com/lichess-org/op1
[schut25]: https://doi.org/10.1073/pnas.2406675122
