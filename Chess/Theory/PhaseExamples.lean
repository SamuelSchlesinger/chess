import Chess.Initial
import Chess.Theory.Opening
import Chess.Theory.Phase

namespace Chess.Theory.PhaseExamples

private def e2e4 : Move := ⟨⟨4, 1⟩, ⟨4, 3⟩, none⟩
private def g1f3 : Move := ⟨⟨6, 0⟩, ⟨5, 2⟩, none⟩
private def g8f6 : Move := ⟨⟨6, 7⟩, ⟨5, 5⟩, none⟩
private def f3g1 : Move := ⟨⟨5, 2⟩, ⟨6, 0⟩, none⟩
private def f6g8 : Move := ⟨⟨5, 5⟩, ⟨6, 7⟩, none⟩

/-- The initial double pawn step consumes exactly two units of pawn travel. -/
theorem e2e4_phase_drop_exact :
    (applyUnchecked Initial.position e2e4).phasePotential + 2 =
      Initial.position.phasePotential := by
  native_decide

/-- The structural theorem, instantiated on the opening move `1. e4`. -/
theorem e2e4_can_never_lie_on_a_cycle :
    ¬RepetitionReachable (applyUnchecked Initial.position e2e4) Initial.position := by
  apply pawn_move_not_on_cycle Initial.position e2e4 ⟨.white, .pawn⟩
  · native_decide
  · rfl
  · native_decide

/-- A knight development preserves the phase grade. This says that the move is
structurally reversible, not that the resulting chess position is equivalent. -/
theorem g1f3_preserves_phase :
    (applyUnchecked Initial.position g1f3).phasePotential =
      Initial.position.phasePotential := by
  native_decide

private def afterNf3 : Position := applyUnchecked Initial.position g1f3
private def knightReturn : List Move := [g8f6, f3g1, f6g8]

/-- The ordinary knight shuffle genuinely returns to the initial FIDE
repetition class, despite its different move clocks. -/
theorem nf3_returns_to_initial_repetition_node :
    RepetitionReachable afterNf3 Initial.position := by
  refine ⟨playMoves afterNf3 knightReturn,
    reachable_playMoves_of_lineIsLegal afterNf3 knightReturn (by native_decide), ?_⟩
  native_decide

/-- The quiet-kernel theorem is non-vacuous: its hypotheses hold for `1. Nf3`
inside the four-knight shuffle. -/
theorem nf3_is_a_quiet_kernel_edge :
    ∃ piece,
      Initial.position.board.pieceAt g1f3.source = some piece ∧
      piece.kind ≠ .pawn ∧
      Initial.position.board.pieceAt g1f3.target = none ∧
      afterNf3.castlingRights = Initial.position.castlingRights ∧
      afterNf3.halfmoveClock = Initial.position.halfmoveClock + 1 := by
  exact move_on_repetition_cycle_is_quiet Initial.position g1f3
    (by native_decide) nf3_returns_to_initial_repetition_node

end Chess.Theory.PhaseExamples
