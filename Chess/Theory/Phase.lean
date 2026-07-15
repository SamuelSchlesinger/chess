import Chess.Game

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

/-- Reachability in the clock-erased FIDE position graph: some concrete future
belongs to the target's repetition-equivalence class. -/
def RepetitionReachable (start target : Position) : Prop :=
  ∃ future, Position.Reachable start future ∧ sameForRepetition future target

theorem repetitionReachable_phasePotential_le {start target : Position}
    (reachable : RepetitionReachable start target) :
    target.phasePotential ≤ start.phasePotential := by
  rcases reachable with ⟨future, path, same⟩
  rw [← phasePotential_eq_of_sameForRepetition same]
  exact reachable_phasePotential_le path

/-- Positions in the same strongly connected component of the FIDE repetition
quotient have the same phase potential. -/
theorem phasePotential_eq_of_mutuallyRepetitionReachable {left right : Position}
    (forward : RepetitionReachable left right)
    (backward : RepetitionReachable right left) :
    left.phasePotential = right.phasePotential := by
  exact Nat.le_antisymm (repetitionReachable_phasePotential_le backward)
    (repetitionReachable_phasePotential_le forward)

/-- An edge on a directed cycle of the repetition quotient preserves phase. -/
theorem successor_on_repetition_cycle_phasePotential_eq {position next : Position}
    (successor : Position.Successor position next)
    (returns : RepetitionReachable next position) :
    next.phasePotential = position.phasePotential := by
  exact Nat.le_antisymm (successor_phasePotential_le successor)
    (repetitionReachable_phasePotential_le returns)

/-- No strictly irreversible edge can lie on a directed cycle of the
repetition quotient. -/
theorem no_phaseDrop_on_repetition_cycle {position next : Position}
    (drop : PhaseDrop position next) :
    ¬RepetitionReachable next position := by
  intro returns
  have reverseLe := repetitionReachable_phasePotential_le returns
  exact (Nat.not_lt_of_ge reverseLe) drop.2

/-- **Pawn-cycle purity.** A legal pawn move can never belong to a directed
cycle of the FIDE repetition quotient. Capturing pawn moves, en passant, double
steps, and promotions are all covered by the same theorem. -/
theorem pawn_move_not_on_cycle (position : Position) (move : Move) (piece : Piece)
    (occupied : position.board.pieceAt move.source = some piece)
    (isPawn : piece.kind = .pawn) (legal : Legal position move) :
    ¬RepetitionReachable (applyUnchecked position move) position := by
  intro returns
  have reverseLe := repetitionReachable_phasePotential_le returns
  have strict := phasePotential_applyUnchecked_lt_of_pawn
    position move piece occupied isPawn legal
  omega

end Chess.Theory
