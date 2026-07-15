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

/-- Positions in the same strongly connected component have the same phase
potential. This is the grading theorem for the legal position graph. -/
theorem phasePotential_eq_of_mutuallyReachable {left right : Position}
    (forward : Position.Reachable left right)
    (backward : Position.Reachable right left) :
    left.phasePotential = right.phasePotential := by
  exact Nat.le_antisymm (reachable_phasePotential_le backward)
    (reachable_phasePotential_le forward)

/-- An edge on a directed cycle preserves phase exactly. -/
theorem successor_on_cycle_phasePotential_eq {position next : Position}
    (successor : Position.Successor position next)
    (returns : Position.Reachable next position) :
    next.phasePotential = position.phasePotential := by
  exact (phasePotential_eq_of_mutuallyReachable
    (.step successor (.refl next)) returns).symm

/-- No strictly irreversible edge can lie on a directed cycle. -/
theorem no_phaseDrop_on_cycle {position next : Position}
    (drop : PhaseDrop position next) :
    ¬Position.Reachable next position := by
  intro returns
  have reverseLe := reachable_phasePotential_le returns
  exact (Nat.not_lt_of_ge reverseLe) drop.2

/-- **Pawn-cycle purity.** A legal pawn move can never belong to a directed
cycle of chess positions. Capturing pawn moves, en passant, double steps, and
promotions are all covered by the same theorem. -/
theorem pawn_move_not_on_cycle (position : Position) (move : Move) (piece : Piece)
    (occupied : position.board.pieceAt move.source = some piece)
    (isPawn : piece.kind = .pawn) (legal : Legal position move) :
    ¬Position.Reachable (applyUnchecked position move) position := by
  intro returns
  have reverseLe := reachable_phasePotential_le returns
  have strict := phasePotential_applyUnchecked_lt_of_pawn
    position move piece occupied isPawn legal
  omega

end Chess.Theory
