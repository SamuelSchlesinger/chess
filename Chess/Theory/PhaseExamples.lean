import Chess.Initial
import Chess.Theory.Phase

namespace Chess.Theory.PhaseExamples

private def e2e4 : Move := ⟨⟨4, 1⟩, ⟨4, 3⟩, none⟩
private def g1f3 : Move := ⟨⟨6, 0⟩, ⟨5, 2⟩, none⟩

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

end Chess.Theory.PhaseExamples
