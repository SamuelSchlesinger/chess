import Chess.Game

namespace Chess.Theory

/-- `player` can force `goal` within at most `plies` legal half-moves.

At the player's nodes one legal continuation suffices; at the opponent's nodes
every legal continuation must preserve the force. Requiring an opponent move
prevents a non-goal stalemate or checkmate from satisfying the universal clause
vacuously. -/
def CanForceWithin (player : Color) (goal : Position → Prop) : Nat → Position → Prop
  | 0, position => goal position
  | plies + 1, position =>
      goal position ∨
        if position.turn = player then
          ∃ move, Legal position move ∧
            CanForceWithin player goal plies (applyUnchecked position move)
        else
          HasLegalMove position ∧
            ∀ move, Legal position move →
              CanForceWithin player goal plies (applyUnchecked position move)

/-- An unbounded finite forcing strategy has some uniform finite ply bound. -/
def CanForce (player : Color) (goal : Position → Prop) (position : Position) : Prop :=
  ∃ plies, CanForceWithin player goal plies position

theorem goal_implies_canForceWithin (player : Color) (goal : Position → Prop)
    {position : Position} (achieved : goal position) (plies : Nat) :
    CanForceWithin player goal plies position := by
  cases plies with
  | zero => exact achieved
  | succ plies => exact Or.inl achieved

/-- More time cannot destroy a forcing strategy. -/
theorem canForceWithin_mono_one (player : Color) (goal : Position → Prop)
    {plies : Nat} {position : Position}
    (force : CanForceWithin player goal plies position) :
    CanForceWithin player goal (plies + 1) position := by
  induction plies generalizing position with
  | zero =>
      exact Or.inl force
  | succ plies ih =>
      unfold CanForceWithin at force ⊢
      rcases force with achieved | continuing
      · exact Or.inl achieved
      · exact Or.inr <| by
          by_cases turnEq : position.turn = player
          · simp only [turnEq, ↓reduceIte] at continuing ⊢
            rcases continuing with ⟨move, legal, rest⟩
            exact ⟨move, legal, ih rest⟩
          · simp only [turnEq, ↓reduceIte] at continuing ⊢
            rcases continuing with ⟨hasMove, allMoves⟩
            exact ⟨hasMove, fun move legal => ih (allMoves move legal)⟩

theorem canForceWithin_mono (player : Color) (goal : Position → Prop)
    {smaller larger : Nat} {position : Position} (bound : smaller ≤ larger)
    (force : CanForceWithin player goal smaller position) :
    CanForceWithin player goal larger position := by
  obtain ⟨difference, rfl⟩ := Nat.exists_eq_add_of_le bound
  clear bound
  induction difference with
  | zero => simpa
  | succ difference ih =>
      rw [Nat.add_succ]
      exact canForceWithin_mono_one player goal ih

theorem canForce_of_canForceWithin (player : Color) (goal : Position → Prop)
    {plies : Nat} {position : Position}
    (force : CanForceWithin player goal plies position) : CanForce player goal position :=
  ⟨plies, force⟩

end Chess.Theory
