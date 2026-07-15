import Chess.Theory.KingGeometry

namespace Chess.Theory

/-- The lower king and upper king are in direct vertical opposition: same file
with exactly one square between them. -/
def LowerVerticalDirectOpposition (lower upper : Square) : Prop :=
  lower.file = upper.file ∧ lower.rank.val + 2 = upper.rank.val

instance (lower upper : Square) : Decidable (LowerVerticalDirectOpposition lower upper) := by
  unfold LowerVerticalDirectOpposition
  infer_instance

theorem lowerVerticalDirect_distance {lower upper : Square}
    (opposition : LowerVerticalDirectOpposition lower upper) :
    kingDistance lower upper = 2 := by
  rcases opposition with ⟨sameFile, twoRanks⟩
  unfold kingDistance
  have fileZero : absDiff lower.file.val upper.file.val = 0 := by simp [sameFile]
  have rankTwo : absDiff lower.rank.val upper.rank.val = 2 := by
    unfold absDiff
    split <;> omega
  omega

/-- **Opposition response theorem.** Suppose two kings are in direct vertical
opposition. If the lower king makes any geometric king step sideways or
backward, the upper king has a geometric king step that restores direct
opposition.

The response is constructed by translating the moved king two ranks upward.
This covers both ordinary mirroring and the closer-following response to a
retreat, with board-edge safety proved from the original opposition. -/
theorem lowerVerticalDirect_has_restoring_response
    {lower upper moved : Square}
    (opposition : LowerVerticalDirectOpposition lower upper)
    (move : KingAdjacent lower moved)
    (notForward : moved.rank.val ≤ lower.rank.val) :
    ∃ response,
      KingAdjacent upper response ∧
      LowerVerticalDirectOpposition moved response := by
  rcases opposition with ⟨sameFile, twoRanks⟩
  rcases move with ⟨movedDifferent, fileClose, rankClose⟩
  have responseOnBoard : moved.rank.val + 2 < 8 := by
    have upperOnBoard := upper.rank.isLt
    omega
  let response : Square :=
    ⟨moved.file, ⟨moved.rank.val + 2, responseOnBoard⟩⟩
  refine ⟨response, ?_, ?_⟩
  · refine ⟨?_, ?_, ?_⟩
    · intro sameResponse
      apply movedDifferent
      have responseFile := congrArg Square.file sameResponse
      have responseRank := congrArg (fun square : Square => square.rank.val) sameResponse
      change upper.file = moved.file at responseFile
      change upper.rank.val = moved.rank.val + 2 at responseRank
      have fileValues : lower.file = moved.file := sameFile.trans responseFile
      have rankValues : lower.rank.val = moved.rank.val := by omega
      cases lower with
      | mk lowerFile lowerRank =>
        cases upper with
        | mk upperFile upperRank =>
          cases moved with
          | mk movedFile movedRank =>
            have fileEq : lowerFile = movedFile := fileValues
            have rankEq : lowerRank = movedRank := by
              apply Fin.ext
              exact rankValues
            simp [fileEq, rankEq]
    · simpa [sameFile] using fileClose
    · change absDiff upper.rank.val (moved.rank.val + 2) ≤ 1
      unfold absDiff at rankClose ⊢
      split at rankClose <;> split <;> omega
  · exact ⟨rfl, rfl⟩

/-- A natural number is even, stated without importing an external arithmetic library. -/
def NatEven (value : Nat) : Prop := ∃ half, value = 2 * half

/-- A natural number is odd. -/
def NatOdd (value : Nat) : Prop := ∃ half, value = 2 * half + 1

/-- Direct opposition is the first even-distance case of distant opposition. -/
def LowerVerticalOpposition (lower upper : Square) : Prop :=
  lower.file = upper.file ∧ lower.rank.val < upper.rank.val ∧
    NatEven (kingDistance lower upper)

theorem direct_is_vertical_opposition {lower upper : Square}
    (direct : LowerVerticalDirectOpposition lower upper) :
    LowerVerticalOpposition lower upper := by
  have directCopy := direct
  rcases direct with ⟨sameFile, twoRanks⟩
  refine ⟨sameFile, by omega, ?_⟩
  unfold NatEven
  rw [lowerVerticalDirect_distance directCopy]
  exact ⟨1, rfl⟩

/-- With aligned kings, even king distance is equivalent to an odd number of
intervening squares—the player's usual distant-opposition counting rule. -/
theorem vertical_opposition_odd_gap {lower upper : Square}
    (sameFile : lower.file = upper.file) (ordered : lower.rank.val < upper.rank.val) :
    LowerVerticalOpposition lower upper ↔ NatOdd (upper.rank.val - lower.rank.val - 1) := by
  have distanceEq : kingDistance lower upper = upper.rank.val - lower.rank.val := by
    unfold kingDistance
    have fileZero : absDiff lower.file.val upper.file.val = 0 := by simp [sameFile]
    have rankDistance : absDiff lower.rank.val upper.rank.val =
        upper.rank.val - lower.rank.val := by
      unfold absDiff
      split <;> omega
    omega
  unfold LowerVerticalOpposition NatEven NatOdd
  rw [distanceEq]
  simp only [sameFile, ordered, true_and]
  constructor
  · rintro ⟨half, evenDistance⟩
    refine ⟨half - 1, ?_⟩
    omega
  · rintro ⟨half, oddGap⟩
    refine ⟨half + 1, ?_⟩
    omega

end Chess.Theory
