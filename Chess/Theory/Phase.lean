import Chess.Theory.RepetitionGraph

namespace Chess.Theory

/-- A legal edge that strictly consumes irreversible phase potential. -/
def PhaseDrop (position next : Position) : Prop :=
  Position.Successor position next ∧ next.phasePotential < position.phasePotential

/-- Every edge of the legal position graph weakly descends the phase grading. -/
theorem successor_phasePotential_le {position next : Position}
    (successor : Position.Successor position next) :
    next.phasePotential ≤ position.phasePotential := by
  rcases successor with ⟨move, legal, rfl⟩
  exact phasePotential_applyUnchecked_le position move legal

/-- Phase potential is monotone along every finite legal continuation. -/
theorem reachable_phasePotential_le {position future : Position}
    (reachable : Position.Reachable position future) :
    future.phasePotential ≤ position.phasePotential := by
  induction reachable with
  | refl => exact Nat.le_refl _
  | step successor rest ih =>
      exact Nat.le_trans ih (successor_phasePotential_le successor)

/-- FIDE repetition-equivalent positions have the same phase potential. The
clocks omitted from repetition identity are also omitted from the grade. -/
theorem phasePotential_eq_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) :
    left.phasePotential = right.phasePotential := by
  simp [sameForRepetition] at same
  have boardEq := Board.eq_of_same same.1.1.1
  have rightsEq := same.1.2
  simp [Position.phasePotential, boardEq, rightsEq]

/-- Concrete reachability projected only at its target: some future position
belongs to the target's FIDE repetition-equivalence class. -/
def RepetitionReachable (start target : Position) : Prop :=
  ∃ future, Position.Reachable start future ∧ sameForRepetition future target

/-- Every concrete path projects to a path in the FIDE repetition quotient. -/
theorem repetitionNodeReachable_of_repetitionReachable {start target : Position}
    (reachable : RepetitionReachable start target) :
    RepetitionNode.Reachable
      (RepetitionNode.ofPosition start) (RepetitionNode.ofPosition target) := by
  rcases reachable with ⟨future, path, same⟩
  have nodePath := RepetitionNode.reachable_of_position path
  have targetEq : RepetitionNode.ofPosition future =
      RepetitionNode.ofPosition target :=
    RepetitionNode.ofPosition_eq_iff.mpr same
  rwa [targetEq] at nodePath

/-- Conversely, operational congruence lifts every quotient path from any
chosen concrete representative. -/
theorem repetitionReachable_of_repetitionNodeReachable {start target : Position}
    (reachable : RepetitionNode.Reachable
      (RepetitionNode.ofPosition start) (RepetitionNode.ofPosition target)) :
    RepetitionReachable start target := by
  rcases RepetitionNode.reachable_lift start reachable with
    ⟨future, path, targetEq⟩
  exact ⟨future, path, RepetitionNode.ofPosition_eq_iff.mp targetEq⟩

theorem repetitionReachable_iff_repetitionNodeReachable (start target : Position) :
    RepetitionReachable start target ↔
      RepetitionNode.Reachable
        (RepetitionNode.ofPosition start) (RepetitionNode.ofPosition target) :=
  ⟨repetitionNodeReachable_of_repetitionReachable,
    repetitionReachable_of_repetitionNodeReachable⟩

theorem repetitionReachable_phasePotential_le {start target : Position}
    (reachable : RepetitionReachable start target) :
    target.phasePotential ≤ start.phasePotential := by
  rcases reachable with ⟨future, path, same⟩
  rw [← phasePotential_eq_of_sameForRepetition same]
  exact reachable_phasePotential_le path

/-- Positions with concrete continuations into each other's FIDE repetition
classes have the same phase potential. -/
theorem phasePotential_eq_of_mutuallyRepetitionReachable {left right : Position}
    (forward : RepetitionReachable left right)
    (backward : RepetitionReachable right left) :
    left.phasePotential = right.phasePotential := by
  exact Nat.le_antisymm (repetitionReachable_phasePotential_le backward)
    (repetitionReachable_phasePotential_le forward)

/-- An edge whose successor can return to the source's repetition class
preserves phase. -/
theorem successor_on_repetition_cycle_phasePotential_eq {position next : Position}
    (successor : Position.Successor position next)
    (returns : RepetitionReachable next position) :
    next.phasePotential = position.phasePotential := by
  exact Nat.le_antisymm (successor_phasePotential_le successor)
    (repetitionReachable_phasePotential_le returns)

/-- No strictly irreversible edge admits a continuation back to its source's
repetition class. -/
theorem no_phaseDrop_on_repetition_cycle {position next : Position}
    (drop : PhaseDrop position next) :
    ¬RepetitionReachable next position := by
  intro returns
  have reverseLe := repetitionReachable_phasePotential_le returns
  exact (Nat.not_lt_of_ge reverseLe) drop.2

/-- **Pawn-cycle purity.** After a legal pawn move, no continuation can return
to the source's FIDE repetition class. Captures, en passant, double steps, and
promotions are all covered by the same theorem. -/
theorem pawn_move_not_on_cycle (position : Position) (move : Move) (piece : Piece)
    (occupied : position.board.pieceAt move.source = some piece)
    (isPawn : piece.kind = .pawn) (legal : Legal position move) :
    ¬RepetitionReachable (applyUnchecked position move) position := by
  intro returns
  have reverseLe := repetitionReachable_phasePotential_le returns
  have strict := phasePotential_applyUnchecked_lt_of_pawn
    position move piece occupied isPawn legal
  omega

/-- **Quiet-kernel theorem.** Every legal edge whose successor can return to
the source's FIDE repetition class is a quiet non-pawn move that preserves
castling rights. Consequently the concrete halfmove clock advances across
every such edge.

The occupied-target clause rules out ordinary captures; en passant is ruled
out by the non-pawn clause. Rights preservation also rules out castling and any
first king or rook move that consumes a surviving right. -/
theorem move_on_repetition_cycle_is_quiet (position : Position) (move : Move)
    (legal : Legal position move)
    (returns : RepetitionReachable (applyUnchecked position move) position) :
    ∃ piece,
      position.board.pieceAt move.source = some piece ∧
      piece.kind ≠ .pawn ∧
      position.board.pieceAt move.target = none ∧
      (applyUnchecked position move).castlingRights = position.castlingRights ∧
      (applyUnchecked position move).halfmoveClock = position.halfmoveClock + 1 := by
  have pseudo := ((legal_iff position move).mp legal).1
  have phaseEq := successor_on_repetition_cycle_phasePotential_eq
    (position := position) (next := applyUnchecked position move)
    ⟨move, legal, rfl⟩ returns
  unfold PseudoLegal at pseudo
  cases occupied : position.board.pieceAt move.source with
  | none => simp [isPseudoLegal, occupied] at pseudo
  | some piece =>
      have notPawn : piece.kind ≠ .pawn := by
        intro isPawn
        have strict := phasePotential_applyUnchecked_lt_of_pawn
          position move piece occupied isPawn legal
        omega
      have targetEmpty : position.board.pieceAt move.target = none := by
        cases targetOccupied : position.board.pieceAt move.target with
        | none => rfl
        | some captured =>
            have strict := phasePotential_applyUnchecked_lt_of_occupied_target
              position move captured targetOccupied legal
            omega
      have rightsEq : (applyUnchecked position move).castlingRights =
          position.castlingRights := by
        by_cases same : (applyUnchecked position move).castlingRights =
            position.castlingRights
        · exact same
        · have strict := phasePotential_applyUnchecked_lt_of_castlingRights_ne
            position move legal same
          omega
      have clockEq : (applyUnchecked position move).halfmoveClock =
          position.halfmoveClock + 1 := by
        simp [applyUnchecked, occupied, notPawn, targetEmpty]
      exact ⟨piece, rfl, notPawn, targetEmpty, rightsEq, clockEq⟩

end Chess.Theory

namespace Chess.RepetitionNode

/-- Irreversible phase potential descends to a numerical grading of the FIDE
repetition quotient. -/
def phasePotential (node : RepetitionNode) : Nat :=
  Quotient.lift Position.phasePotential
    (fun _left _right same => Theory.phasePotential_eq_of_sameForRepetition same) node

@[simp] theorem phasePotential_ofPosition (position : Position) :
    phasePotential (ofPosition position) = position.phasePotential := rfl

/-- Every quotient edge weakly descends the phase grading. -/
theorem successor_phasePotential_le {node next : RepetitionNode}
    (successor : Successor node next) :
    next.phasePotential ≤ node.phasePotential := by
  induction node using Quotient.inductionOn with
  | _ position =>
      rcases successor with ⟨move, legal, rfl⟩
      exact Chess.phasePotential_applyUnchecked_le position move legal

theorem reachable_phasePotential_le {node future : RepetitionNode}
    (reachable : Reachable node future) :
    future.phasePotential ≤ node.phasePotential := by
  induction reachable with
  | refl => exact Nat.le_refl _
  | step successor _ ih =>
      exact Nat.le_trans ih (successor_phasePotential_le successor)

/-- Every strongly connected component of the formal FIDE-repetition quotient
has constant irreversible phase potential. -/
theorem phasePotential_eq_of_mutuallyReachable {left right : RepetitionNode}
    (forward : Reachable left right) (backward : Reachable right left) :
    left.phasePotential = right.phasePotential := by
  exact Nat.le_antisymm (reachable_phasePotential_le backward)
    (reachable_phasePotential_le forward)

/-- A strictly phase-decreasing quotient edge cannot lie on a directed cycle. -/
theorem no_strict_phase_edge_on_cycle {node next : RepetitionNode}
    (_successor : Successor node next)
    (strict : next.phasePotential < node.phasePotential) :
    ¬Reachable next node := by
  intro returns
  have reverseLe := reachable_phasePotential_le returns
  exact (Nat.not_lt_of_ge reverseLe) strict

end Chess.RepetitionNode
