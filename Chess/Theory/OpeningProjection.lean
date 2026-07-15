import Init.GrindInstances.Ring.Nat
import Chess.RepetitionKey
import Chess.Theory.OpeningDatabase

namespace Chess.Theory.OpeningProjection

open OpeningDatabase
open Lean.Grind
open Lean.Grind.AddCommMonoid

/-!
# Projecting finite opening corpora

An opening corpus records **occurrences of histories**.  Its rows may carry a
name, a result count, a frequency, or a probability mass.  Transposition
merging does not turn any of those row fields into an intrinsic property of a
position.  Instead, the mathematically honest operation is a finite
pushforward: project every occurrence to an exact modeled repetition key and add
the weights in each fibre.

This file separates those two ideas:

* `WeightedObservation` and `WeightedCorpus` retain occurrence metadata;
* `fibreWeight` adds the weights of all occurrences mapped to one node;
* `endpointKey` is the executable, information-complete projection of a legal
  line;
* `RecordedContinuation` certifies that every projected corpus edge is a legal
  edge of the repetition graph.

Consequently, an opening name attached to a row remains a name of that
occurrence or move order.  `labelsInFibre` can return several (even repeated)
names for one key.  This is deliberately different from the
endpoint-invariant labels characterized in `OpeningDatabase`.
-/

universe u v w x y

/-- One weighted occurrence in a finite dataset.  `history` is the thing that
will be projected; `label` is occurrence metadata, not an assertion that the
label factors through the projected node. -/
structure WeightedObservation (History : Type u) (Label : Type v)
    (Weight : Type w) where
  history : History
  label : Label
  weight : Weight
  deriving Repr

namespace WeightedObservation

/-- Change the space in which an occurrence is indexed, retaining its label
and weight. -/
def project {History : Type u} {Target : Type x} {Label : Type v}
    {Weight : Type w} (projection : History → Target)
    (observation : WeightedObservation History Label Weight) :
    WeightedObservation Target Label Weight where
  history := projection observation.history
  label := observation.label
  weight := observation.weight

/-- Rename occurrence metadata without changing the history or weight. -/
def relabel {History : Type u} {Label : Type v} {NewLabel : Type x}
    {Weight : Type w} (rename : Label → NewLabel)
    (observation : WeightedObservation History Label Weight) :
    WeightedObservation History NewLabel Weight where
  history := observation.history
  label := rename observation.label
  weight := observation.weight

@[simp] theorem project_id {History : Type u} {Label : Type v}
    {Weight : Type w} (observation : WeightedObservation History Label Weight) :
    observation.project id = observation := by
  cases observation
  rfl

@[simp] theorem project_comp {History : Type u} {Middle : Type v}
    {Target : Type x} {Label : Type w} {Weight : Type y}
    (first : History → Middle) (second : Middle → Target)
    (observation : WeightedObservation History Label Weight) :
    (observation.project first).project second =
      observation.project (second ∘ first) := by
  cases observation
  rfl

@[simp] theorem relabel_id {History : Type u} {Label : Type v}
    {Weight : Type w} (observation : WeightedObservation History Label Weight) :
    observation.relabel id = observation := by
  cases observation
  rfl

@[simp] theorem relabel_comp {History : Type u} {Label : Type v}
    {MiddleLabel : Type w} {NewLabel : Type x} {Weight : Type y}
    (first : Label → MiddleLabel) (second : MiddleLabel → NewLabel)
    (observation : WeightedObservation History Label Weight) :
    (observation.relabel first).relabel second =
      observation.relabel (second ∘ first) := by
  cases observation
  rfl

/-- Projection of histories and renaming of occurrence labels are independent
operations. -/
theorem project_relabel {History : Type u} {Target : Type v}
    {Label : Type w} {NewLabel : Type x} {Weight : Type y}
    (projection : History → Target) (rename : Label → NewLabel)
    (observation : WeightedObservation History Label Weight) :
    (observation.project projection).relabel rename =
      (observation.relabel rename).project projection := by
  cases observation
  rfl

end WeightedObservation

/-- A finite weighted corpus is a list of occurrences.  Repeated rows remain
meaningful: they contribute their weights repeatedly. -/
abbrev WeightedCorpus (History : Type u) (Label : Type v) (Weight : Type w) :=
  List (WeightedObservation History Label Weight)

namespace WeightedCorpus

/-- Project every occurrence in a corpus. -/
def project {History : Type u} {Target : Type x} {Label : Type v}
    {Weight : Type w} (projection : History → Target)
    (corpus : WeightedCorpus History Label Weight) :
    WeightedCorpus Target Label Weight :=
  corpus.map (WeightedObservation.project projection)

/-- Rename every occurrence label in a corpus. -/
def relabel {History : Type u} {Label : Type v} {NewLabel : Type x}
    {Weight : Type w} (rename : Label → NewLabel)
    (corpus : WeightedCorpus History Label Weight) :
    WeightedCorpus History NewLabel Weight :=
  corpus.map (WeightedObservation.relabel rename)

@[simp] theorem project_id {History : Type u} {Label : Type v}
    {Weight : Type w} (corpus : WeightedCorpus History Label Weight) :
    corpus.project id = corpus := by
  induction corpus with
  | nil => rfl
  | cons observation rest ih =>
      change WeightedObservation.project id observation ::
          List.map (WeightedObservation.project id) rest = observation :: rest
      rw [WeightedObservation.project_id]
      congr

@[simp] theorem project_comp {History : Type u} {Middle : Type v}
    {Target : Type x} {Label : Type w} {Weight : Type y}
    (first : History → Middle) (second : Middle → Target)
    (corpus : WeightedCorpus History Label Weight) :
    (corpus.project first).project second =
      corpus.project (second ∘ first) := by
  simp [project]

@[simp] theorem project_append {History : Type u} {Target : Type x}
    {Label : Type v} {Weight : Type w} (projection : History → Target)
    (left right : WeightedCorpus History Label Weight) :
    (left ++ right).project projection =
      left.project projection ++ right.project projection := by
  simp [project]

@[simp] theorem relabel_id {History : Type u} {Label : Type v}
    {Weight : Type w} (corpus : WeightedCorpus History Label Weight) :
    corpus.relabel id = corpus := by
  induction corpus with
  | nil => rfl
  | cons observation rest ih =>
      change WeightedObservation.relabel id observation ::
          List.map (WeightedObservation.relabel id) rest = observation :: rest
      rw [WeightedObservation.relabel_id]
      congr

@[simp] theorem relabel_comp {History : Type u} {Label : Type v}
    {MiddleLabel : Type w} {NewLabel : Type x} {Weight : Type y}
    (first : Label → MiddleLabel) (second : MiddleLabel → NewLabel)
    (corpus : WeightedCorpus History Label Weight) :
    (corpus.relabel first).relabel second =
      corpus.relabel (second ∘ first) := by
  simp [relabel]

@[simp] theorem relabel_append {History : Type u} {Label : Type v}
    {NewLabel : Type x} {Weight : Type w} (rename : Label → NewLabel)
    (left right : WeightedCorpus History Label Weight) :
    (left ++ right).relabel rename =
      left.relabel rename ++ right.relabel rename := by
  simp [relabel]

theorem project_relabel {History : Type u} {Target : Type v}
    {Label : Type w} {NewLabel : Type x} {Weight : Type y}
    (projection : History → Target) (rename : Label → NewLabel)
    (corpus : WeightedCorpus History Label Weight) :
    (corpus.project projection).relabel rename =
      (corpus.relabel rename).project projection := by
  simp [project, relabel, WeightedObservation.project_relabel]

/-- Add all occurrence weights in the fibre over `target`.  This is the finite
pushforward of the corpus's weight measure along `projection`. -/
def fibreWeight {History : Type u} {Node : Type v} {Label : Type w}
    {Weight : Type x} [DecidableEq Node] [AddCommMonoid Weight]
    (projection : History → Node) (target : Node) :
    WeightedCorpus History Label Weight → Weight
  | [] => 0
  | observation :: rest =>
      if projection observation.history = target then
        observation.weight + fibreWeight projection target rest
      else
        fibreWeight projection target rest

/-- Retain every occurrence label in a fibre, in corpus order and with
duplicates.  In particular this operation intentionally does not choose a
single canonical opening name for a repetition node. -/
def labelsInFibre {History : Type u} {Node : Type v} {Label : Type w}
    {Weight : Type x} [DecidableEq Node]
    (projection : History → Node) (target : Node) :
    WeightedCorpus History Label Weight → List Label
  | [] => []
  | observation :: rest =>
      if projection observation.history = target then
        observation.label :: labelsInFibre projection target rest
      else
        labelsInFibre projection target rest

@[simp] theorem fibreWeight_nil {History : Type u} {Node : Type v}
    {Label : Type w} {Weight : Type x} [DecidableEq Node]
    [AddCommMonoid Weight] (projection : History → Node) (target : Node) :
    fibreWeight (Label := Label) (Weight := Weight) projection target [] = 0 :=
  rfl

@[simp] theorem fibreWeight_append {History : Type u} {Node : Type v}
    {Label : Type w} {Weight : Type x} [DecidableEq Node]
    [AddCommMonoid Weight] (projection : History → Node) (target : Node)
    (left right : WeightedCorpus History Label Weight) :
    fibreWeight projection target (left ++ right) =
      fibreWeight projection target left + fibreWeight projection target right := by
  induction left with
  | nil =>
      change fibreWeight projection target right =
        0 + fibreWeight projection target right
      exact (zero_add _).symm
  | cons observation rest ih =>
      by_cases equal : projection observation.history = target
      · simp [fibreWeight, equal, ih, add_assoc]
      · simp [fibreWeight, equal, ih]

/-- Since weights form a commutative monoid, reordering two corpus chunks does
not change their pushforward weight.  Corpus order still remains observable in
`labelsInFibre`, where it can encode provenance. -/
theorem fibreWeight_append_comm {History : Type u} {Node : Type v}
    {Label : Type w} {Weight : Type x} [DecidableEq Node]
    [AddCommMonoid Weight] (projection : History → Node) (target : Node)
    (left right : WeightedCorpus History Label Weight) :
    fibreWeight projection target (left ++ right) =
      fibreWeight projection target (right ++ left) := by
  rw [fibreWeight_append, fibreWeight_append, add_comm]

/-- Aggregate weight depends on the multiset of observations, not their row
order.  This is the formal reason database ingestion may sort or shard rows
before merging counts. -/
theorem fibreWeight_eq_of_perm {History : Type u} {Node : Type v}
    {Label : Type w} {Weight : Type x} [DecidableEq Node]
    [AddCommMonoid Weight] (projection : History → Node) (target : Node)
    {left right : WeightedCorpus History Label Weight}
    (permutation : left.Perm right) :
    fibreWeight projection target left =
      fibreWeight projection target right := by
  induction permutation with
  | nil => rfl
  | cons observation _ ih =>
      by_cases equal : projection observation.history = target
      · simp [fibreWeight, equal, ih]
      · simp [fibreWeight, equal, ih]
  | swap first second rest =>
      by_cases firstEqual : projection first.history = target
      · by_cases secondEqual : projection second.history = target
        · simp [fibreWeight, firstEqual, secondEqual, add_left_comm]
        · simp [fibreWeight, firstEqual, secondEqual]
      · by_cases secondEqual : projection second.history = target
        · simp [fibreWeight, firstEqual, secondEqual]
        · simp [fibreWeight, firstEqual, secondEqual]
  | trans _ _ leftRight rightFinal =>
      exact leftRight.trans rightFinal

@[simp] theorem labelsInFibre_append {History : Type u} {Node : Type v}
    {Label : Type w} {Weight : Type x} [DecidableEq Node]
    (projection : History → Node) (target : Node)
    (left right : WeightedCorpus History Label Weight) :
    labelsInFibre projection target (left ++ right) =
      labelsInFibre projection target left ++
        labelsInFibre projection target right := by
  induction left with
  | nil => rfl
  | cons observation rest ih =>
      by_cases equal : projection observation.history = target
      · simp [labelsInFibre, equal, ih]
      · simp [labelsInFibre, equal, ih]

/-- Projecting occurrences first and then aggregating is the same pushforward
as aggregating along the composite map. -/
theorem fibreWeight_project {History : Type u} {Middle : Type v}
    {Node : Type w} {Label : Type x} {Weight : Type y}
    [DecidableEq Node] [AddCommMonoid Weight]
    (first : History → Middle) (second : Middle → Node) (target : Node)
    (corpus : WeightedCorpus History Label Weight) :
    fibreWeight second target (corpus.project first) =
      fibreWeight (second ∘ first) target corpus := by
  induction corpus with
  | nil => rfl
  | cons observation rest ih =>
      by_cases equal : second (first observation.history) = target
      · change fibreWeight second target
            (WeightedObservation.project first observation ::
              List.map (WeightedObservation.project first) rest) = _
        change fibreWeight second target
            (List.map (WeightedObservation.project first) rest) =
          fibreWeight (second ∘ first) target rest at ih
        simp [WeightedObservation.project, fibreWeight, equal, ih]
      · change fibreWeight second target
            (WeightedObservation.project first observation ::
              List.map (WeightedObservation.project first) rest) = _
        change fibreWeight second target
            (List.map (WeightedObservation.project first) rest) =
          fibreWeight (second ∘ first) target rest at ih
        simp [WeightedObservation.project, fibreWeight, equal, ih]

/-- Relabelling occurrence metadata cannot change aggregate weight. -/
theorem fibreWeight_relabel {History : Type u} {Node : Type v}
    {Label : Type w} {NewLabel : Type x} {Weight : Type y}
    [DecidableEq Node] [AddCommMonoid Weight]
    (projection : History → Node) (target : Node) (rename : Label → NewLabel)
    (corpus : WeightedCorpus History Label Weight) :
    fibreWeight projection target (corpus.relabel rename) =
      fibreWeight projection target corpus := by
  induction corpus with
  | nil => rfl
  | cons observation rest ih =>
      by_cases equal : projection observation.history = target
      · change fibreWeight projection target
            (WeightedObservation.relabel rename observation ::
              List.map (WeightedObservation.relabel rename) rest) = _
        change fibreWeight projection target
            (List.map (WeightedObservation.relabel rename) rest) =
          fibreWeight projection target rest at ih
        simp [WeightedObservation.relabel, fibreWeight, equal, ih]
      · change fibreWeight projection target
            (WeightedObservation.relabel rename observation ::
              List.map (WeightedObservation.relabel rename) rest) = _
        change fibreWeight projection target
            (List.map (WeightedObservation.relabel rename) rest) =
          fibreWeight projection target rest at ih
        simp [WeightedObservation.relabel, fibreWeight, equal, ih]

/-- Relabelling maps the list of labels in each fibre pointwise. -/
theorem labelsInFibre_relabel {History : Type u} {Node : Type v}
    {Label : Type w} {NewLabel : Type x} {Weight : Type y}
    [DecidableEq Node] (projection : History → Node) (target : Node)
    (rename : Label → NewLabel)
    (corpus : WeightedCorpus History Label Weight) :
    labelsInFibre projection target (corpus.relabel rename) =
      (labelsInFibre projection target corpus).map rename := by
  induction corpus with
  | nil => rfl
  | cons observation rest ih =>
      by_cases equal : projection observation.history = target
      · change labelsInFibre projection target
            (WeightedObservation.relabel rename observation ::
              List.map (WeightedObservation.relabel rename) rest) = _
        change labelsInFibre projection target
            (List.map (WeightedObservation.relabel rename) rest) =
          List.map rename (labelsInFibre projection target rest) at ih
        simp [WeightedObservation.relabel, labelsInFibre, equal, ih]
      · change labelsInFibre projection target
            (WeightedObservation.relabel rename observation ::
              List.map (WeightedObservation.relabel rename) rest) = _
        change labelsInFibre projection target
            (List.map (WeightedObservation.relabel rename) rest) =
          List.map rename (labelsInFibre projection target rest) at ih
        simp [WeightedObservation.relabel, labelsInFibre, equal, ih]

end WeightedCorpus

/-! ## The exact opening projection -/

/-- A weighted occurrence whose history is a certified legal line from
`start`. -/
abbrev OpeningOccurrence (start : Position) (Label : Type u) (Weight : Type v) :=
  WeightedObservation (LegalLine start) Label Weight

/-- The exact, executable repetition key at the end of a legal line. -/
def endpointKey {start : Position} (line : LegalLine start) : RepetitionKey :=
  RepetitionKey.ofPosition line.endpointPosition

/-- Exact executable keys merge precisely the histories merged by the proved
repetition quotient.  Hash collisions are irrelevant because equality of
`RepetitionKey` values stores and compares every rule-relevant component. -/
theorem endpointKey_eq_iff_endpointNode_eq {start : Position}
    (left right : LegalLine start) :
    endpointKey left = endpointKey right ↔
      left.endpointNode = right.endpointNode := by
  rw [endpointKey, endpointKey, RepetitionKey.ofPosition_eq_iff]
  exact RepetitionNode.ofPosition_eq_iff.symm

/-- Equivalently, exact key equality is equality in the reachable opening
quotient. -/
theorem endpointKey_eq_iff_openingNode_eq {start : Position}
    (left right : LegalLine start) :
    endpointKey left = endpointKey right ↔
      left.openingNode = right.openingNode := by
  rw [endpointKey_eq_iff_endpointNode_eq]
  constructor
  · intro equal
    apply OpeningNode.endpoint_injective
    simpa using equal
  · intro equal
    have endpointEqual := congrArg OpeningNode.endpoint equal
    simpa using endpointEqual

/-- Push an opening corpus to its exact repetition keys while preserving each
occurrence's label and weight. -/
def projectToKeys {start : Position} {Label : Type u} {Weight : Type v}
    (corpus : WeightedCorpus (LegalLine start) Label Weight) :
    WeightedCorpus RepetitionKey Label Weight :=
  corpus.project endpointKey

/-- The total weight of every recorded history ending at one exact repetition
key.  This, rather than the weight of an arbitrary representative, is the
natural node statistic induced by a finite corpus. -/
def weightAtKey {start : Position} {Label : Type u} {Weight : Type v}
    [AddCommMonoid Weight]
    (key : RepetitionKey)
    (corpus : WeightedCorpus (LegalLine start) Label Weight) : Weight :=
  WeightedCorpus.fibreWeight endpointKey key corpus

/-- All occurrence labels attached to histories in one exact-key fibre.
Duplicates and competing names are retained. -/
def labelsAtKey {start : Position} {Label : Type u} {Weight : Type v}
    (key : RepetitionKey)
    (corpus : WeightedCorpus (LegalLine start) Label Weight) : List Label :=
  WeightedCorpus.labelsInFibre endpointKey key corpus

@[simp] theorem weightAtKey_append {start : Position} {Label : Type u}
    {Weight : Type v} [AddCommMonoid Weight] (key : RepetitionKey)
    (left right : WeightedCorpus (LegalLine start) Label Weight) :
    weightAtKey key (left ++ right) =
      weightAtKey key left + weightAtKey key right := by
  exact WeightedCorpus.fibreWeight_append endpointKey key left right

@[simp] theorem labelsAtKey_append {start : Position} {Label : Type u}
    {Weight : Type v} (key : RepetitionKey)
    (left right : WeightedCorpus (LegalLine start) Label Weight) :
    labelsAtKey key (left ++ right) =
      labelsAtKey key left ++ labelsAtKey key right := by
  exact WeightedCorpus.labelsInFibre_append endpointKey key left right

/-- Two arbitrary labels can occur at the very same exact endpoint.  This
small theorem states the modelling boundary directly: key equality alone
places no constraint on occurrence names. -/
theorem same_key_with_distinct_occurrence_labels {start : Position}
    {Label : Type u} {Weight : Type v} (line : LegalLine start)
    {leftLabel rightLabel : Label} (different : leftLabel ≠ rightLabel)
    (leftWeight rightWeight : Weight) :
    let left : OpeningOccurrence start Label Weight :=
      ⟨line, leftLabel, leftWeight⟩
    let right : OpeningOccurrence start Label Weight :=
      ⟨line, rightLabel, rightWeight⟩
    endpointKey left.history = endpointKey right.history ∧
      left.label ≠ right.label := by
  exact ⟨rfl, different⟩

/-- A two-row fibre sums both weights; it does not select one row as the
position's intrinsic count. -/
@[simp] theorem weightAtKey_pair_same_line {start : Position}
    {Label : Type u} {Weight : Type v} [AddCommMonoid Weight]
    (line : LegalLine start) (leftLabel rightLabel : Label)
    (leftWeight rightWeight : Weight) :
    weightAtKey (endpointKey line)
      [⟨line, leftLabel, leftWeight⟩, ⟨line, rightLabel, rightWeight⟩] =
        leftWeight + rightWeight := by
  simp [weightAtKey, WeightedCorpus.fibreWeight]
  rw [add_zero]

/-- The corresponding label fibre keeps both occurrence names, even when they
name the same move order. -/
@[simp] theorem labelsAtKey_pair_same_line {start : Position}
    {Label : Type u} {Weight : Type v} (line : LegalLine start)
    (leftLabel rightLabel : Label) (leftWeight rightWeight : Weight) :
    labelsAtKey (endpointKey line)
      [⟨line, leftLabel, leftWeight⟩, ⟨line, rightLabel, rightWeight⟩] =
        [leftLabel, rightLabel] := by
  simp [labelsAtKey, WeightedCorpus.labelsInFibre]

/-! ## Certified continuation observations -/

/-- One continuation recorded after a certified legal prefix.  Legality is
stored at the concrete prefix endpoint, so this structure can be populated by
an importer only after checking the corpus move.  Labels and weights belong to
the surrounding `WeightedObservation`, just as they do for endpoint rows. -/
structure RecordedContinuation (start : Position) where
  sourceLine : LegalLine start
  move : Move
  legal : isLegal sourceLine.endpointPosition move = true

namespace RecordedContinuation

/-- The certified trie child obtained by appending the recorded move. -/
def targetLine {start : Position}
    (continuation : RecordedContinuation start) :
    LegalLine start where
  moves := continuation.sourceLine.moves ++ [continuation.move]
  legal := by
    rw [lineIsLegal_append]
    refine ⟨continuation.sourceLine.legal, ?_⟩
    have moveLegal := continuation.legal
    change isLegal (playMoves start continuation.sourceLine.moves)
      continuation.move = true at moveLegal
    simpa only [lineIsLegal, Bool.and_true] using moveLegal

/-- Executable source key for edge aggregation. -/
def sourceKey {start : Position}
    (continuation : RecordedContinuation start) : RepetitionKey :=
  endpointKey continuation.sourceLine

/-- Executable target key for edge aggregation. -/
def targetKey {start : Position}
    (continuation : RecordedContinuation start) : RepetitionKey :=
  endpointKey continuation.targetLine

/-- Source node of the recorded edge in the FIDE repetition quotient. -/
def sourceNode {start : Position}
    (continuation : RecordedContinuation start) : RepetitionNode :=
  continuation.sourceLine.endpointNode

/-- Target node of the recorded edge in the FIDE repetition quotient. -/
def targetNode {start : Position}
    (continuation : RecordedContinuation start) : RepetitionNode :=
  continuation.targetLine.endpointNode

/-- Projection of a checked corpus continuation never invents an edge: its
source and target repetition nodes are joined by the recorded legal move. -/
theorem legal_edge {start : Position}
    (continuation : RecordedContinuation start) :
    RepetitionNode.Successor continuation.sourceNode continuation.targetNode := by
  change RepetitionNode.Successor continuation.sourceLine.endpointNode
    continuation.targetLine.endpointNode
  apply LegalLine.endpointNode_successor
  exact ⟨continuation.move, rfl⟩

/-- The legal edge theorem exposes the recorded move itself as a witness, not
merely the existence of some quotient edge. -/
theorem legal_edge_witness {start : Position}
    (continuation : RecordedContinuation start) :
    continuation.sourceNode.Legal continuation.move ∧
      continuation.targetNode =
        continuation.sourceNode.after continuation.move := by
  change isLegal continuation.sourceLine.endpointPosition continuation.move = true ∧
    RepetitionNode.ofPosition continuation.targetLine.endpointPosition =
      RepetitionNode.ofPosition
        (applyUnchecked continuation.sourceLine.endpointPosition continuation.move)
  refine ⟨continuation.legal, ?_⟩
  simp [targetLine, LegalLine.endpointPosition, playMoves_append, playMoves]

/-- A move-labelled edge is deterministic after transposition merging: the
same move from the same repetition node reaches the same target node, even
when the two source histories are different. -/
theorem targetNode_eq_of_sourceNode_eq_move_eq {start : Position}
    (left right : RecordedContinuation start)
    (sameSource : left.sourceNode = right.sourceNode)
    (sameMove : left.move = right.move) :
    left.targetNode = right.targetNode := by
  rw [left.legal_edge_witness.2, right.legal_edge_witness.2,
    sameSource, sameMove]

/-- The executable exact keys inherit the same deterministic-edge law.  Thus
`target` is a useful validation checksum in `ExactContinuationKey`, but not an
independent choice once a checked source key and move are fixed. -/
theorem targetKey_eq_of_sourceKey_eq_move_eq {start : Position}
    (left right : RecordedContinuation start)
    (sameSource : left.sourceKey = right.sourceKey)
    (sameMove : left.move = right.move) :
    left.targetKey = right.targetKey := by
  apply (endpointKey_eq_iff_endpointNode_eq left.targetLine right.targetLine).mpr
  apply targetNode_eq_of_sourceNode_eq_move_eq left right
  · exact (endpointKey_eq_iff_endpointNode_eq
      left.sourceLine right.sourceLine).mp sameSource
  · exact sameMove

end RecordedContinuation

/-- A weighted occurrence whose history is one checked continuation edge. -/
abbrev ContinuationOccurrence (start : Position) (Label : Type u)
    (Weight : Type v) :=
  WeightedObservation (RecordedContinuation start) Label Weight

/-- An executable key for aggregating repeated observations of the same
continuation after transposition merging.  Occurrence labels deliberately do
not form part of this key. -/
structure ExactContinuationKey where
  source : RepetitionKey
  move : Move
  target : RepetitionKey
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

/-- Project a checked continuation observation to its exact source, move, and
target triple. -/
def continuationKey {start : Position}
    (continuation : RecordedContinuation start) :
    ExactContinuationKey where
  source := continuation.sourceKey
  move := continuation.move
  target := continuation.targetKey

/-- Aggregate any additive weight (games, wins, probability mass, and so on)
over exactly equal checked continuation keys. -/
def continuationWeightAtKey {start : Position} {Label : Type u}
    {Weight : Type v} [AddCommMonoid Weight] (key : ExactContinuationKey)
    (corpus : List (ContinuationOccurrence start Label Weight)) : Weight :=
  WeightedCorpus.fibreWeight continuationKey key corpus

@[simp] theorem continuationWeightAtKey_append {start : Position}
    {Label : Type u} {Weight : Type v} [AddCommMonoid Weight]
    (key : ExactContinuationKey)
    (left right : List (ContinuationOccurrence start Label Weight)) :
    continuationWeightAtKey key (left ++ right) =
      continuationWeightAtKey key left + continuationWeightAtKey key right := by
  exact WeightedCorpus.fibreWeight_append continuationKey key left right

end Chess.Theory.OpeningProjection
