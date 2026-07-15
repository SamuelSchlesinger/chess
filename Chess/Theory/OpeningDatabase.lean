import Chess.Theory.Opening

namespace Chess.Theory.OpeningDatabase

/-!
# Opening databases: histories versus positions

An opening explorer has two related, but different, shapes.

* Its **prefix trie** distinguishes move orders: a node is a legal move word.
* Its **position graph** identifies legal words that transpose to the same FIDE
  repetition state.

The distinction matters in practice.  A static evaluation deliberately defined
only from repetition state, the set of legal continuations, and any other
endpoint-invariant quantity belong on that graph.  Clock- or history-aware
evaluations do not.  Game counts, move-order labels, and the ply at which a
position was reached belong on trie nodes (or must be aggregated over a whole
corpus fibre).

This file makes that boundary precise.  `LegalLine.endpointNode` is the
quotient map from the legal prefix trie to the repetition graph;
`OpeningNode` is its exact quotient on lines reachable from a fixed start.
-/

/-- A node of the legal move-word prefix trie rooted at `start`.  The proof
records that every edge in the word is legal where it is played. -/
structure LegalLine (start : Position) where
  moves : List Move
  legal : lineIsLegal start moves

namespace LegalLine

/-- The concrete endpoint of a legal opening line. -/
def endpointPosition {start : Position} (line : LegalLine start) : Position :=
  playMoves start line.moves

/-- The FIDE repetition node reached by a legal opening line.  This deliberately
forgets the move clocks and any ineffective raw en-passant target. -/
def endpointNode {start : Position} (line : LegalLine start) : RepetitionNode :=
  RepetitionNode.ofPosition line.endpointPosition

/-- The empty line is the root of the prefix trie. -/
def root (start : Position) : LegalLine start :=
  ⟨[], by simp [lineIsLegal]⟩

@[simp] theorem root_moves (start : Position) : (root start).moves = [] := rfl

@[simp] theorem endpointPosition_root (start : Position) :
    (root start).endpointPosition = start := rfl

@[simp] theorem endpointNode_root (start : Position) :
    (root start).endpointNode = RepetitionNode.ofPosition start := rfl

/-- One trie node is an immediate child of another when its move word appends
one move.  Legality is already carried by both nodes. -/
def Successor {start : Position} (line next : LegalLine start) : Prop :=
  ∃ move, next.moves = line.moves ++ [move]

/-- The endpoint map sends every prefix-trie edge to a legal edge of the
repetition graph.  Thus merging transpositions does not invent chess moves. -/
theorem endpointNode_successor {start : Position} {line next : LegalLine start}
    (successor : Successor line next) :
    RepetitionNode.Successor line.endpointNode next.endpointNode := by
  rcases successor with ⟨move, nextMoves⟩
  have nextLegal : lineIsLegal start (line.moves ++ [move]) := by
    rw [← nextMoves]
    exact next.legal
  have legalParts := (lineIsLegal_append start line.moves [move]).mp nextLegal
  refine ⟨move, ?_, ?_⟩
  · change isLegal (playMoves start line.moves) move = true
    simpa [lineIsLegal] using legalParts.2
  · simp [endpointNode, endpointPosition, nextMoves, playMoves_append, playMoves]

/-- Every legal trie node maps to a node reachable in the repetition graph. -/
theorem endpointNode_reachable {start : Position} (line : LegalLine start) :
    RepetitionNode.Reachable (RepetitionNode.ofPosition start) line.endpointNode := by
  exact RepetitionNode.reachable_of_position
    (reachable_playMoves_of_lineIsLegal start line.moves line.legal)

/-- Every concrete legal path has a move-word representative in the prefix
trie. -/
theorem exists_of_position_reachable {start finish : Position}
    (reachable : Position.Reachable start finish) :
    ∃ line : LegalLine start, line.endpointPosition = finish := by
  induction reachable with
  | refl position =>
      exact ⟨root position, rfl⟩
  | @step start middle finish successor _ ih =>
      rcases successor with ⟨move, moveLegal, rfl⟩
      rcases ih with ⟨tail, tailEndpoint⟩
      change isLegal start move = true at moveLegal
      let line : LegalLine start :=
        ⟨move :: tail.moves, by
          simp [lineIsLegal, moveLegal, tail.legal]⟩
      refine ⟨line, ?_⟩
      simpa [line, endpointPosition, playMoves] using tailEndpoint

/-- The image of the trie-to-graph map is exactly the part of the repetition
graph reachable from the chosen root. -/
theorem endpointNode_range_iff_reachable (start : Position) (node : RepetitionNode) :
    (∃ line : LegalLine start, line.endpointNode = node) ↔
      RepetitionNode.Reachable (RepetitionNode.ofPosition start) node := by
  constructor
  · rintro ⟨line, rfl⟩
    exact line.endpointNode_reachable
  · intro reachable
    rcases RepetitionNode.reachable_lift start reachable with
      ⟨finish, positionPath, finishNode⟩
    rcases exists_of_position_reachable positionPath with ⟨line, lineEndpoint⟩
    refine ⟨line, ?_⟩
    change RepetitionNode.ofPosition line.endpointPosition = node
    rw [lineEndpoint]
    exact finishNode

end LegalLine

/-- Two legal line histories represent the same opening node precisely when
their endpoints are the same FIDE repetition node. -/
def lineEndpointSetoid (start : Position) : Setoid (LegalLine start) where
  r := fun left right => left.endpointNode = right.endpointNode
  iseqv := ⟨fun _ => rfl, fun equal => equal.symm,
    fun leftMiddle middleRight => leftMiddle.trans middleRight⟩

/-- The quotient of the legal prefix trie obtained by merging transpositions.
It contains exactly the repetition nodes reachable by legal lines from the
chosen start, with legal lines as its quotient representatives.  This does not
provide an executable operation that extracts a representative. -/
def OpeningNode (start : Position) := Quotient (lineEndpointSetoid start)

namespace OpeningNode

/-- Insert a legal trie node into the transposition quotient. -/
def ofLine {start : Position} (line : LegalLine start) : OpeningNode start :=
  Quotient.mk (lineEndpointSetoid start) line

/-- The quotient embeds into the ambient FIDE repetition graph. -/
def endpoint {start : Position} (node : OpeningNode start) : RepetitionNode :=
  Quotient.lift LegalLine.endpointNode (fun _ _ equal => equal) node

@[simp] theorem endpoint_ofLine {start : Position} (line : LegalLine start) :
    endpoint (ofLine line) = line.endpointNode := rfl

/-- No additional identifications are introduced after quotienting legal
lines: the opening quotient embeds faithfully in the repetition graph. -/
theorem endpoint_injective {start : Position} :
    Function.Injective (@endpoint start) := by
  intro left right
  induction left using Quotient.inductionOn with
  | _ leftLine =>
      induction right using Quotient.inductionOn with
      | _ rightLine =>
          intro equal
          exact Quotient.sound equal

/-- Equality in the opening quotient is exactly endpoint equality in the
repetition graph. -/
theorem eq_iff_endpoint_eq {start : Position} (left right : OpeningNode start) :
    left = right ↔ endpoint left = endpoint right := by
  constructor
  · intro equal
    exact congrArg endpoint equal
  · intro equal
    exact (@endpoint_injective start) equal

end OpeningNode

namespace LegalLine

/-- The quotient node represented by a legal line. -/
def openingNode {start : Position} (line : LegalLine start) : OpeningNode start :=
  OpeningNode.ofLine line

@[simp] theorem openingNode_endpoint {start : Position} (line : LegalLine start) :
    line.openingNode.endpoint = line.endpointNode := rfl

/-- The exact relationship between opening transpositions and the trie-to-graph
quotient: two legal lines are merged iff they form a repetition
transposition. -/
theorem openingNode_eq_iff_transposes {start : Position}
    (left right : LegalLine start) :
    left.openingNode = right.openingNode ↔
      LinesRepetitionTransposeAt start left.moves right.moves := by
  rw [OpeningNode.eq_iff_endpoint_eq]
  constructor
  · intro equal
    exact ⟨left.legal, right.legal,
      RepetitionNode.ofPosition_eq_iff.mp equal⟩
  · intro transpose
    exact RepetitionNode.ofPosition_eq_iff.mpr transpose.sameNode

end LegalLine

/-- A line observable is endpoint-invariant when it assigns equal values to
all histories in the same transposition fibre. -/
def EndpointInvariant {start : Position} {Value : Sort _}
    (observable : LegalLine start → Value) : Prop :=
  ∀ left right, left.endpointNode = right.endpointNode →
    observable left = observable right

/-- An observable factors through the opening quotient when it is the pullback
of a label on transposition classes. -/
def FactorsThroughOpeningNode {start : Position} {Value : Sort _}
    (observable : LegalLine start → Value) : Prop :=
  ∃ nodeObservable : OpeningNode start → Value,
    ∀ line, observable line = nodeObservable line.openingNode

/-- Factoring through the ambient repetition graph.  Unlike a factor through
`OpeningNode`, such a factor is not unique: its values away from nodes
reachable from `start` are unconstrained by the opening database. -/
def FactorsThroughRepetitionNode {start : Position} {Value : Sort _}
    (observable : LegalLine start → Value) : Prop :=
  ∃ nodeObservable : RepetitionNode → Value,
    ∀ line, observable line = nodeObservable line.endpointNode

/-- Every endpoint-invariant line observable descends to the transposition
quotient. -/
def factor {start : Position} {Value : Sort _}
    (observable : LegalLine start → Value)
    (invariant : EndpointInvariant observable) : OpeningNode start → Value :=
  fun node => Quotient.lift observable
    (fun left right equal => invariant left right equal) node

@[simp] theorem factor_ofLine {start : Position} {Value : Sort _}
    (observable : LegalLine start → Value)
    (invariant : EndpointInvariant observable) (line : LegalLine start) :
    factor observable invariant line.openingNode = observable line := rfl

/-- Universal property of the opening quotient: a history statistic can be
stored unambiguously on transposition nodes iff it is endpoint-invariant. -/
theorem factorsThroughOpeningNode_iff_endpointInvariant
    {start : Position} {Value : Sort _}
    (observable : LegalLine start → Value) :
    FactorsThroughOpeningNode observable ↔ EndpointInvariant observable := by
  constructor
  · rintro ⟨nodeObservable, realizes⟩ left right sameEndpoint
    rw [realizes left, realizes right]
    apply congrArg nodeObservable
    apply OpeningNode.endpoint_injective
    simpa using sameEndpoint
  · intro invariant
    exact ⟨factor observable invariant, fun line => (factor_ofLine _ _ line).symm⟩

/-- Endpoint invariance is also exactly the condition for a line observable to
come from some label on the whole repetition graph.  The reverse direction
extends the unique reachable-node factor with an arbitrary root value on
unreachable nodes. -/
theorem factorsThroughRepetitionNode_iff_endpointInvariant
    {start : Position} {Value : Sort _}
    (observable : LegalLine start → Value) :
    FactorsThroughRepetitionNode observable ↔ EndpointInvariant observable := by
  constructor
  · rintro ⟨nodeObservable, realizes⟩ left right sameEndpoint
    rw [realizes left, realizes right, sameEndpoint]
  · intro invariant
    classical
    let nodeObservable : RepetitionNode → Value := fun node =>
      if represented : ∃ line : LegalLine start, line.endpointNode = node then
        observable (Classical.choose represented)
      else
        observable (LegalLine.root start)
    refine ⟨nodeObservable, ?_⟩
    intro line
    have represented : ∃ candidate : LegalLine start,
        candidate.endpointNode = line.endpointNode := ⟨line, rfl⟩
    rw [show nodeObservable line.endpointNode =
        observable (Classical.choose represented) by
      simp only [nodeObservable, dif_pos represented]]
    exact invariant line (Classical.choose represented)
      (Classical.choose_spec represented).symm

/-- The factor through the quotient is unique. -/
theorem factor_unique {start : Position} {Value : Sort _}
    (observable : LegalLine start → Value)
    (invariant : EndpointInvariant observable)
    (candidate : OpeningNode start → Value)
    (realizes : ∀ line, candidate line.openingNode = observable line) :
    candidate = factor observable invariant := by
  funext node
  induction node using Quotient.inductionOn with
  | _ line => exact realizes line

/-- Pull an ordinary repetition-node label back to every legal opening line
that reaches it.  Examples include evaluations, tactical predicates, and
tablebase values whenever those are functions of the FIDE repetition state. -/
def evaluateAtEndpoint {start : Position} {Value : Sort _}
    (observable : RepetitionNode → Value) (line : LegalLine start) : Value :=
  observable line.endpointNode

theorem evaluateAtEndpoint_invariant {start : Position} {Value : Sort _}
    (observable : RepetitionNode → Value) :
    EndpointInvariant (@evaluateAtEndpoint start Value observable) := by
  intro left right equal
  exact congrArg observable equal

/-- Any node-valued evaluation gives the same answer after transposed legal
move orders. -/
theorem evaluateAtEndpoint_eq_of_transpose
    {start : Position} {Value : Sort _}
    (observable : RepetitionNode → Value)
    {left right : List Move}
    (transpose : LinesRepetitionTransposeAt start left right) :
    evaluateAtEndpoint observable ⟨left, transpose.leftLegal⟩ =
      evaluateAtEndpoint observable ⟨right, transpose.rightLegal⟩ := by
  exact congrArg observable
    (RepetitionNode.ofPosition_eq_iff.mpr transpose.sameNode)

/-- The residual legal language rooted at a repetition node. -/
def residualLanguage (node : RepetitionNode) : List Move → Bool :=
  fun continuation => node.lineIsLegal continuation

/-- Transposed opening lines have extensionally identical residual languages,
so every legality-based opening query is safe to memoize by repetition node. -/
theorem residualLanguage_eq_of_transpose {start : Position}
    {left right : List Move}
    (transpose : LinesRepetitionTransposeAt start left right) :
    residualLanguage (RepetitionNode.ofPosition (playMoves start left)) =
      residualLanguage (RepetitionNode.ofPosition (playMoves start right)) := by
  have nodeEq := RepetitionNode.ofPosition_eq_iff.mpr transpose.sameNode
  exact congrArg residualLanguage nodeEq

/-- More generally, every endpoint-invariant history label agrees on
transposed lines. -/
theorem endpointInvariant_eq_of_transpose
    {start : Position} {Value : Sort _}
    {observable : LegalLine start → Value}
    (invariant : EndpointInvariant observable)
    {left right : List Move}
    (transpose : LinesRepetitionTransposeAt start left right) :
    observable ⟨left, transpose.leftLegal⟩ =
      observable ⟨right, transpose.rightLegal⟩ := by
  apply invariant
  exact RepetitionNode.ofPosition_eq_iff.mpr transpose.sameNode

/-- A generic obstruction: if a legal nonempty line returns to its starting
repetition node, then ply count cannot be a label on endpoint nodes. -/
theorem plyCount_not_invariant_of_nonempty_cycle
    {start : Position} {cycle : List Move}
    (legal : lineIsLegal start cycle)
    (nonempty : cycle ≠ [])
    (returns : sameForRepetition (playMoves start cycle) start) :
    ¬ EndpointInvariant (fun line : LegalLine start => line.moves.length) := by
  intro invariant
  let root := LegalLine.root start
  let loop : LegalLine start := ⟨cycle, legal⟩
  have sameNode : root.endpointNode = loop.endpointNode := by
    apply RepetitionNode.ofPosition_eq_iff.mpr
    exact sameForRepetition_symm returns
  have sameCount := invariant root loop sameNode
  have zeroLength : cycle.length = 0 := by
    simpa [root, loop] using sameCount.symm
  exact nonempty (List.eq_nil_of_length_eq_zero zeroLength)

namespace Examples

private def g1f3 : Move := ⟨⟨6, 0⟩, ⟨5, 2⟩, none⟩
private def g8f6 : Move := ⟨⟨6, 7⟩, ⟨5, 5⟩, none⟩
private def f3g1 : Move := ⟨⟨5, 2⟩, ⟨6, 0⟩, none⟩
private def f6g8 : Move := ⟨⟨5, 5⟩, ⟨6, 7⟩, none⟩

/-- A four-ply legal cycle returning to the initial repetition node. -/
private def knightCycle : List Move := [g1f3, g8f6, f3g1, f6g8]

theorem knightCycle_legal : lineIsLegal Initial.position knightCycle := by
  native_decide

theorem knightCycle_returns :
    sameForRepetition
      (playMoves Initial.position knightCycle) Initial.position := by
  native_decide

/-- A concrete warning for opening-database design: the root and the position
after `1.Nf3 Nf6 2.Ng1 Ng8` are one repetition node but occur at different
plies.  Therefore ply number (and similarly a move-order name or occurrence
record) cannot be inferred from the endpoint alone. -/
theorem initial_plyCount_does_not_factor :
    ¬ FactorsThroughOpeningNode
      (fun line : LegalLine Initial.position => line.moves.length) := by
  rw [factorsThroughOpeningNode_iff_endpointInvariant]
  exact plyCount_not_invariant_of_nonempty_cycle
    knightCycle_legal (by native_decide) knightCycle_returns

end Examples

end Chess.Theory.OpeningDatabase
