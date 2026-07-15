import Chess.Theory.Reti

namespace Chess.Theory.RetiExample

private def h8 : Square := Square.ofCoords 7 7
private def g7 : Square := Square.ofCoords 6 6
private def f6 : Square := Square.ofCoords 5 5
private def h6 : Square := Square.ofCoords 7 5
private def d6 : Square := Square.ofCoords 3 5
private def f4 : Square := Square.ofCoords 5 3

/-- The geometric heart of Réti's 1921 study. After `Kh8-g7-f6`, the king on
`f6` is still exactly two moves from both `d6` (supporting the c-pawn) and `f4`
(catching the h-pawn after Black spends tempi taking on c6). -/
theorem famous_study_f6_is_dual_pivot : RetiPivot h8 d6 f4 f6 2 4 4 := by
  refine ⟨?_, ?_, ?_⟩
  · apply (kingCanReachWithin_iff 2 h8 f6).mpr
    decide
  · apply (kingCanReachWithin_iff 2 f6 d6).mpr
    decide
  · apply (kingCanReachWithin_iff 2 f6 f4).mpr
    decide

/-- Committing straight down the h-file for two moves loses the dual purpose:
from h6, the king cannot still reach d6 by the same four-move deadline. -/
theorem famous_study_h6_is_not_dual_pivot : ¬RetiPivot h8 d6 f4 h6 2 4 4 := by
  intro supposedPivot
  have supportReach := supposedPivot.2.1
  have impossible := (kingCanReachWithin_iff 2 h6 d6).mp supportReach
  change 4 ≤ 2 at impossible
  omega

/-- The closed interval theorem discovers that some dual-purpose pivot exists;
the preceding theorem identifies the celebrated f6 pivot constructively. -/
theorem famous_study_has_dual_pivot : ∃ pivot, RetiPivot h8 d6 f4 pivot 2 4 4 :=
  ⟨f6, famous_study_f6_is_dual_pivot⟩

/-- The dual-purpose square after two king moves is not merely a good choice:
the deadline geometry forces it to be f6. -/
theorem famous_study_f6_is_unique_dual_pivot (pivot : Square) :
    RetiPivot h8 d6 f4 pivot 2 4 4 ↔ pivot = f6 := by
  constructor
  · rintro ⟨startReach, supportReach, catchReach⟩
    have startDistance := (kingCanReachWithin_iff 2 h8 pivot).mp startReach
    have supportDistance := (kingCanReachWithin_iff 2 pivot d6).mp supportReach
    have catchDistance := (kingCanReachWithin_iff 2 pivot f4).mp catchReach
    have startParts := (kingDistance_le_iff h8 pivot 2).mp startDistance
    have supportParts := (kingDistance_le_iff pivot d6 2).mp supportDistance
    have catchParts := (kingDistance_le_iff pivot f4 2).mp catchDistance
    have pivotFileAtLeastFive : 5 ≤ pivot.file.val := by
      have interval := (coordinateWithin_iff_interval pivot.file.val h8.file.val 2).mp <| by
        simpa [CoordinateWithin, absDiff_comm] using startParts.1
      change 5 ≤ pivot.file.val ∧ pivot.file.val ≤ 9 at interval
      exact interval.1
    have pivotFileAtMostFive : pivot.file.val ≤ 5 := by
      have interval := (coordinateWithin_iff_interval pivot.file.val d6.file.val 2).mp <| by
        simpa [CoordinateWithin] using supportParts.1
      change 1 ≤ pivot.file.val ∧ pivot.file.val ≤ 5 at interval
      exact interval.2
    have pivotRankAtLeastFive : 5 ≤ pivot.rank.val := by
      have interval := (coordinateWithin_iff_interval pivot.rank.val h8.rank.val 2).mp <| by
        simpa [CoordinateWithin, absDiff_comm] using startParts.2
      change 5 ≤ pivot.rank.val ∧ pivot.rank.val ≤ 9 at interval
      exact interval.1
    have pivotRankAtMostFive : pivot.rank.val ≤ 5 := by
      have interval := (coordinateWithin_iff_interval pivot.rank.val f4.rank.val 2).mp <| by
        simpa [CoordinateWithin] using catchParts.2
      change 1 ≤ pivot.rank.val ∧ pivot.rank.val ≤ 5 at interval
      exact interval.2
    cases pivot with
    | mk file rank =>
      change 5 ≤ file.val at pivotFileAtLeastFive
      change file.val ≤ 5 at pivotFileAtMostFive
      change 5 ≤ rank.val at pivotRankAtLeastFive
      change rank.val ≤ 5 at pivotRankAtMostFive
      simp only [f6, Square.ofCoords]
      congr <;> apply Fin.ext <;> omega
  · intro pivotEq
    rw [pivotEq]
    exact famous_study_f6_is_dual_pivot

/-- There is exactly one intermediate square on a two-move king route from h8
to f6: g7. Thus the celebrated `1.Kg7!` is derived from the geometry rather
than supplied as a move from a solution table. -/
theorem famous_study_g7_is_unique_first_step (middle : Square) :
    (KingAdjacent h8 middle ∧ KingAdjacent middle f6) ↔ middle = g7 := by
  constructor
  · rintro ⟨firstStep, secondStep⟩
    have firstDistance := kingDistance_of_adjacent firstStep
    have secondDistance := kingDistance_of_adjacent secondStep
    have firstParts := (kingDistance_le_iff h8 middle 1).mp (by omega)
    have secondParts := (kingDistance_le_iff middle f6 1).mp (by omega)
    have fileAtLeastSix : 6 ≤ middle.file.val := by
      have interval := (coordinateWithin_iff_interval middle.file.val h8.file.val 1).mp <| by
        simpa [CoordinateWithin, absDiff_comm] using firstParts.1
      change 6 ≤ middle.file.val ∧ middle.file.val ≤ 8 at interval
      exact interval.1
    have fileAtMostSix : middle.file.val ≤ 6 := by
      have interval := (coordinateWithin_iff_interval middle.file.val f6.file.val 1).mp <| by
        simpa [CoordinateWithin] using secondParts.1
      change 4 ≤ middle.file.val ∧ middle.file.val ≤ 6 at interval
      exact interval.2
    have rankAtLeastSix : 6 ≤ middle.rank.val := by
      have interval := (coordinateWithin_iff_interval middle.rank.val h8.rank.val 1).mp <| by
        simpa [CoordinateWithin, absDiff_comm] using firstParts.2
      change 6 ≤ middle.rank.val ∧ middle.rank.val ≤ 8 at interval
      exact interval.1
    have rankAtMostSix : middle.rank.val ≤ 6 := by
      have interval := (coordinateWithin_iff_interval middle.rank.val f6.rank.val 1).mp <| by
        simpa [CoordinateWithin] using secondParts.2
      change 4 ≤ middle.rank.val ∧ middle.rank.val ≤ 6 at interval
      exact interval.2
    cases middle with
    | mk file rank =>
      change 6 ≤ file.val at fileAtLeastSix
      change file.val ≤ 6 at fileAtMostSix
      change 6 ≤ rank.val at rankAtLeastSix
      change rank.val ≤ 6 at rankAtMostSix
      simp only [g7, Square.ofCoords]
      congr <;> apply Fin.ext <;> omega
  · intro middleEq
    rw [middleEq]
    constructor <;> decide

end Chess.Theory.RetiExample
