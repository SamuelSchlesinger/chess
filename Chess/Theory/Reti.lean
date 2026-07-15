import Chess.Theory.PawnGeometry

namespace Chess.Theory

/-- A coordinate lies in the one-dimensional radius around a center. -/
def CoordinateWithin (value center : Nat) (radius : Nat) : Prop :=
  absDiff value center ≤ radius

/-- Absolute-distance balls on a line are closed intervals. -/
theorem coordinateWithin_iff_interval (value center radius : Nat) :
    CoordinateWithin value center radius ↔ center - radius ≤ value ∧ value ≤ center + radius := by
  unfold CoordinateWithin absDiff
  split <;> omega

/-- Lower endpoint of three intersected king-distance intervals. -/
def tripleLower (center₀ radius₀ center₁ radius₁ center₂ radius₂ : Nat) : Nat :=
  max (center₀ - radius₀) (max (center₁ - radius₁) (center₂ - radius₂))

/-- Upper endpoint of three intersected king-distance intervals, clipped to an
orthodox-board coordinate. -/
def tripleUpper (center₀ radius₀ center₁ radius₁ center₂ radius₂ : Nat) : Nat :=
  min 7 (min (center₀ + radius₀) (min (center₁ + radius₁) (center₂ + radius₂)))

def CoordinateTripleFeasible (value : Coordinate)
    (center₀ : Coordinate) (radius₀ : Nat)
    (center₁ : Coordinate) (radius₁ : Nat)
    (center₂ : Coordinate) (radius₂ : Nat) : Prop :=
  CoordinateWithin value.val center₀.val radius₀ ∧
  CoordinateWithin value.val center₁.val radius₁ ∧
  CoordinateWithin value.val center₂.val radius₂

/-- Three one-dimensional king-deadline intervals share an on-board coordinate
exactly when their greatest lower endpoint does not exceed their least upper
endpoint. The reverse direction constructs the coordinate at the lower bound. -/
theorem coordinateTripleFeasible_exists_iff
    (center₀ : Coordinate) (radius₀ : Nat)
    (center₁ : Coordinate) (radius₁ : Nat)
    (center₂ : Coordinate) (radius₂ : Nat) :
    (∃ value, CoordinateTripleFeasible value center₀ radius₀ center₁ radius₁ center₂ radius₂) ↔
      tripleLower center₀.val radius₀ center₁.val radius₁ center₂.val radius₂ ≤
        tripleUpper center₀.val radius₀ center₁.val radius₁ center₂.val radius₂ := by
  constructor
  · rintro ⟨value, feasible₀, feasible₁, feasible₂⟩
    have interval₀ := (coordinateWithin_iff_interval value.val center₀.val radius₀).mp feasible₀
    have interval₁ := (coordinateWithin_iff_interval value.val center₁.val radius₁).mp feasible₁
    have interval₂ := (coordinateWithin_iff_interval value.val center₂.val radius₂).mp feasible₂
    have lowerBound :
        tripleLower center₀.val radius₀ center₁.val radius₁ center₂.val radius₂ ≤ value.val := by
      apply Nat.max_le.mpr
      exact ⟨interval₀.1, Nat.max_le.mpr ⟨interval₁.1, interval₂.1⟩⟩
    have upperBound :
        value.val ≤ tripleUpper center₀.val radius₀ center₁.val radius₁ center₂.val radius₂ := by
      apply Nat.le_min.mpr
      refine ⟨by omega, ?_⟩
      apply Nat.le_min.mpr
      exact ⟨interval₀.2, Nat.le_min.mpr ⟨interval₁.2, interval₂.2⟩⟩
    exact Nat.le_trans lowerBound upperBound
  · intro intersects
    let lower := tripleLower center₀.val radius₀ center₁.val radius₁ center₂.val radius₂
    let upper := tripleUpper center₀.val radius₀ center₁.val radius₁ center₂.val radius₂
    have upperOnBoard : upper ≤ 7 := by
      exact Nat.min_le_left _ _
    have lowerOnBoard : lower < 8 := by omega
    let value : Coordinate := ⟨lower, lowerOnBoard⟩
    have lower₀ : center₀.val - radius₀ ≤ lower := by
      exact Nat.le_max_left _ _
    have lower₁ : center₁.val - radius₁ ≤ lower := by
      exact Nat.le_trans (Nat.le_max_left _ _ ) (Nat.le_max_right _ _)
    have lower₂ : center₂.val - radius₂ ≤ lower := by
      exact Nat.le_trans (Nat.le_max_right _ _) (Nat.le_max_right _ _)
    have upper₀ : upper ≤ center₀.val + radius₀ := by
      exact Nat.le_trans (Nat.min_le_right _ _ ) (Nat.min_le_left _ _)
    have upper₁ : upper ≤ center₁.val + radius₁ := by
      exact Nat.le_trans (Nat.le_trans (Nat.min_le_right _ _) (Nat.min_le_right _ _))
        (Nat.min_le_left _ _)
    have upper₂ : upper ≤ center₂.val + radius₂ := by
      exact Nat.le_trans (Nat.le_trans (Nat.min_le_right _ _) (Nat.min_le_right _ _))
        (Nat.min_le_right _ _)
    refine ⟨value, ?_, ?_, ?_⟩
    · apply (coordinateWithin_iff_interval _ _ _).mpr
      exact ⟨lower₀, Nat.le_trans intersects upper₀⟩
    · apply (coordinateWithin_iff_interval _ _ _).mpr
      exact ⟨lower₁, Nat.le_trans intersects upper₁⟩
    · apply (coordinateWithin_iff_interval _ _ _).mpr
      exact ⟨lower₂, Nat.le_trans intersects upper₂⟩

/-- A Réti pivot reachable by `elapsed` king moves keeps two target deadlines
alive: from the pivot, either target remains independently reachable within the
remaining conservative budget. -/
def RetiPivot (start first second pivot : Square)
    (elapsed firstDeadline secondDeadline : Nat) : Prop :=
  KingCanReachWithin elapsed start pivot ∧
  KingCanReachWithin (firstDeadline - elapsed) pivot first ∧
  KingCanReachWithin (secondDeadline - elapsed) pivot second

private def pivotLowerFile (start first second : Square)
    (elapsed firstDeadline secondDeadline : Nat) : Nat :=
  tripleLower start.file.val elapsed
    first.file.val (firstDeadline - elapsed)
    second.file.val (secondDeadline - elapsed)

private def pivotUpperFile (start first second : Square)
    (elapsed firstDeadline secondDeadline : Nat) : Nat :=
  tripleUpper start.file.val elapsed
    first.file.val (firstDeadline - elapsed)
    second.file.val (secondDeadline - elapsed)

private def pivotLowerRank (start first second : Square)
    (elapsed firstDeadline secondDeadline : Nat) : Nat :=
  tripleLower start.rank.val elapsed
    first.rank.val (firstDeadline - elapsed)
    second.rank.val (secondDeadline - elapsed)

private def pivotUpperRank (start first second : Square)
    (elapsed firstDeadline secondDeadline : Nat) : Nat :=
  tripleUpper start.rank.val elapsed
    first.rank.val (firstDeadline - elapsed)
    second.rank.val (secondDeadline - elapsed)

/-- **Réti pivot theorem.** A king has a bounded prefix route that simultaneously keeps
two deadline objectives possible exactly when the corresponding file intervals
intersect and the corresponding rank intervals intersect.

This reduces a seemingly branching chess calculation to two closed inequalities
and constructs the shared pivot square from their lower endpoints. It is the
multi-objective generalization of the ordinary rule of the square. -/
theorem retiPivot_exists_iff (start first second : Square)
    (elapsed firstDeadline secondDeadline : Nat) :
    (∃ pivot, RetiPivot start first second pivot elapsed firstDeadline secondDeadline) ↔
      pivotLowerFile start first second elapsed firstDeadline secondDeadline ≤
        pivotUpperFile start first second elapsed firstDeadline secondDeadline ∧
      pivotLowerRank start first second elapsed firstDeadline secondDeadline ≤
        pivotUpperRank start first second elapsed firstDeadline secondDeadline := by
  let firstRemaining := firstDeadline - elapsed
  let secondRemaining := secondDeadline - elapsed
  constructor
  · rintro ⟨pivot, startReach, firstReach, secondReach⟩
    have startDistance := (kingCanReachWithin_iff elapsed start pivot).mp startReach
    have firstDistance := (kingCanReachWithin_iff firstRemaining pivot first).mp firstReach
    have secondDistance := (kingCanReachWithin_iff secondRemaining pivot second).mp secondReach
    have startParts := (kingDistance_le_iff start pivot elapsed).mp startDistance
    have firstParts := (kingDistance_le_iff pivot first firstRemaining).mp firstDistance
    have secondParts := (kingDistance_le_iff pivot second secondRemaining).mp secondDistance
    constructor
    · apply (coordinateTripleFeasible_exists_iff start.file elapsed first.file firstRemaining
        second.file secondRemaining).mp
      exact ⟨pivot.file, by simpa [CoordinateWithin, absDiff_comm] using startParts.1,
        firstParts.1, secondParts.1⟩
    · apply (coordinateTripleFeasible_exists_iff start.rank elapsed first.rank firstRemaining
        second.rank secondRemaining).mp
      exact ⟨pivot.rank, by simpa [CoordinateWithin, absDiff_comm] using startParts.2,
        firstParts.2, secondParts.2⟩
  · rintro ⟨fileIntersects, rankIntersects⟩
    obtain ⟨file, startFile, firstFile, secondFile⟩ :=
      (coordinateTripleFeasible_exists_iff start.file elapsed first.file firstRemaining
        second.file secondRemaining).mpr fileIntersects
    obtain ⟨rank, startRank, firstRank, secondRank⟩ :=
      (coordinateTripleFeasible_exists_iff start.rank elapsed first.rank firstRemaining
        second.rank secondRemaining).mpr rankIntersects
    let pivot : Square := ⟨file, rank⟩
    refine ⟨pivot, ?_, ?_, ?_⟩
    · apply (kingCanReachWithin_iff elapsed start pivot).mpr
      apply (kingDistance_le_iff start pivot elapsed).mpr
      exact ⟨by simpa [pivot, CoordinateWithin, absDiff_comm] using startFile,
        by simpa [pivot, CoordinateWithin, absDiff_comm] using startRank⟩
    · apply (kingCanReachWithin_iff firstRemaining pivot first).mpr
      apply (kingDistance_le_iff pivot first firstRemaining).mpr
      exact ⟨firstFile, firstRank⟩
    · apply (kingCanReachWithin_iff secondRemaining pivot second).mpr
      apply (kingDistance_le_iff pivot second secondRemaining).mpr
      exact ⟨secondFile, secondRank⟩

end Chess.Theory
