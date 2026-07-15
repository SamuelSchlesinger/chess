import Chess.Geometry

namespace Chess.Theory

/-- Absolute difference on natural coordinates. -/
def absDiff (left right : Nat) : Nat :=
  if left ≤ right then right - left else left - right

@[simp] theorem absDiff_self (value : Nat) : absDiff value value = 0 := by
  simp [absDiff]

theorem absDiff_comm (left right : Nat) : absDiff left right = absDiff right left := by
  simp only [absDiff]
  split <;> split <;> omega

theorem absDiff_triangle (a b c : Nat) :
    absDiff a c ≤ absDiff a b + absDiff b c := by
  simp only [absDiff]
  split <;> split <;> split <;> omega

theorem absDiff_eq_zero {left right : Nat} : absDiff left right = 0 ↔ left = right := by
  simp only [absDiff]
  split <;> omega

/-- The number of unobstructed king moves between two squares: the Chebyshev
metric on file/rank coordinates. -/
def kingDistance (source target : Square) : Nat :=
  max (absDiff source.file.val target.file.val) (absDiff source.rank.val target.rank.val)

theorem kingDistance_le_iff (source target : Square) (budget : Nat) :
    kingDistance source target ≤ budget ↔
      absDiff source.file.val target.file.val ≤ budget ∧
      absDiff source.rank.val target.rank.val ≤ budget := by
  exact Nat.max_le

@[simp] theorem kingDistance_self (square : Square) : kingDistance square square = 0 := by
  simp [kingDistance]

theorem kingDistance_comm (source target : Square) :
    kingDistance source target = kingDistance target source := by
  simp [kingDistance, absDiff_comm]

/-- Two distinct squares connected by one geometrically possible king step.
This deliberately ignores occupancy and attacks. -/
def KingAdjacent (source target : Square) : Prop :=
  source ≠ target ∧
    absDiff source.file.val target.file.val ≤ 1 ∧
    absDiff source.rank.val target.rank.val ≤ 1

instance (source target : Square) : Decidable (KingAdjacent source target) := by
  unfold KingAdjacent
  infer_instance

theorem kingDistance_le_add (a b c : Square) :
    kingDistance a c ≤ kingDistance a b + kingDistance b c := by
  have fileTriangle := absDiff_triangle a.file.val b.file.val c.file.val
  have rankTriangle := absDiff_triangle a.rank.val b.rank.val c.rank.val
  have fileAB : absDiff a.file.val b.file.val ≤ kingDistance a b :=
    Nat.le_max_left _ _
  have rankAB : absDiff a.rank.val b.rank.val ≤ kingDistance a b :=
    Nat.le_max_right _ _
  have fileBC : absDiff b.file.val c.file.val ≤ kingDistance b c :=
    Nat.le_max_left _ _
  have rankBC : absDiff b.rank.val c.rank.val ≤ kingDistance b c :=
    Nat.le_max_right _ _
  change max (absDiff a.file.val c.file.val) (absDiff a.rank.val c.rank.val) ≤ _
  exact Nat.max_le.mpr ⟨by omega, by omega⟩

theorem kingDistance_of_adjacent {source target : Square} (h : KingAdjacent source target) :
    kingDistance source target = 1 := by
  rcases h with ⟨different, fileClose, rankClose⟩
  unfold kingDistance
  have atMost : max (absDiff source.file.val target.file.val)
      (absDiff source.rank.val target.rank.val) ≤ 1 := Nat.max_le.mpr ⟨fileClose, rankClose⟩
  have nonzero : max (absDiff source.file.val target.file.val)
      (absDiff source.rank.val target.rank.val) ≠ 0 := by
    intro zero
    have fileZero : absDiff source.file.val target.file.val = 0 := by
      have := Nat.le_max_left (absDiff source.file.val target.file.val)
        (absDiff source.rank.val target.rank.val)
      omega
    have rankZero : absDiff source.rank.val target.rank.val = 0 := by
      have := Nat.le_max_right (absDiff source.file.val target.file.val)
        (absDiff source.rank.val target.rank.val)
      omega
    apply different
    cases source with
    | mk sf sr =>
      cases target with
      | mk tf tr =>
        have hf : sf = tf := Fin.ext (absDiff_eq_zero.mp fileZero)
        have hr : sr = tr := Fin.ext (absDiff_eq_zero.mp rankZero)
        simp [hf, hr]
  omega

/-- A length-indexed geometric king walk. -/
inductive KingWalk : Nat → Square → Square → Prop where
  | nil (square : Square) : KingWalk 0 square square
  | step {length : Nat} {source next target : Square} :
      KingAdjacent source next → KingWalk length next target →
      KingWalk (length + 1) source target

/-- No king walk can beat Chebyshev distance. -/
theorem kingDistance_le_of_walk {length : Nat} {source target : Square}
    (walk : KingWalk length source target) : kingDistance source target ≤ length := by
  induction walk with
  | nil => simp
  | @step length source next target adjacent walk ih =>
      have triangle := kingDistance_le_add source next target
      have first := kingDistance_of_adjacent adjacent
      omega

namespace Coordinate

/-- Move one coordinate one step toward another coordinate. -/
def toward (source target : Coordinate) : Coordinate :=
  if lower : source.val < target.val then
    ⟨source.val + 1, by omega⟩
  else if higher : target.val < source.val then
    ⟨source.val - 1, by omega⟩
  else
    source

theorem toward_close (source target : Coordinate) :
    absDiff source.val (toward source target).val ≤ 1 := by
  unfold toward
  split
  · change absDiff source.val (source.val + 1) ≤ 1
    unfold absDiff
    split <;> omega
  · split
    · change absDiff source.val (source.val - 1) ≤ 1
      unfold absDiff
      split <;> omega
    · simp [absDiff]

theorem toward_distance (source target : Coordinate) :
    absDiff (toward source target).val target.val + (if source = target then 0 else 1) =
      absDiff source.val target.val := by
  unfold toward
  split
  · rename_i lower
    have different : source ≠ target := by
      intro same
      cases same
      omega
    simp only [different, ↓reduceIte]
    change absDiff (source.val + 1) target.val + 1 = absDiff source.val target.val
    unfold absDiff
    split <;> split <;> omega
  · rename_i notLower
    split
    · rename_i higher
      have different : source ≠ target := by
        intro same
        cases same
        omega
      simp only [different, ↓reduceIte]
      change absDiff (source.val - 1) target.val + 1 = absDiff source.val target.val
      unfold absDiff
      split <;> split <;> omega
    · rename_i notHigher
      have same : source = target := by apply Fin.ext; omega
      simp [same, absDiff]

theorem eq_of_toward_eq_source {source target : Coordinate}
    (same : toward source target = source) : source = target := by
  unfold toward at same
  split at same
  · have values := congrArg Fin.val same
    change source.val + 1 = source.val at values
    omega
  · split at same
    · have values := congrArg Fin.val same
      change source.val - 1 = source.val at values
      omega
    · apply Fin.ext
      omega

@[simp] theorem toward_self (coordinate : Coordinate) : toward coordinate coordinate = coordinate := by
  simp [toward]

end Coordinate

/-- The diagonal-or-straight king step that greedily approaches a target. -/
def toward (source target : Square) : Square :=
  ⟨Coordinate.toward source.file target.file, Coordinate.toward source.rank target.rank⟩

theorem adjacent_toward {source target : Square} (different : source ≠ target) :
    KingAdjacent source (toward source target) := by
  refine ⟨?_, Coordinate.toward_close _ _, Coordinate.toward_close _ _⟩
  intro same
  apply different
  have fileToward : Coordinate.toward source.file target.file = source.file := by
    exact (congrArg Square.file same).symm
  have rankToward : Coordinate.toward source.rank target.rank = source.rank := by
    exact (congrArg Square.rank same).symm
  have fileSame := Coordinate.eq_of_toward_eq_source fileToward
  have rankSame := Coordinate.eq_of_toward_eq_source rankToward
  cases source with
  | mk sf sr =>
    cases target with
    | mk tf tr =>
      cases fileSame
      cases rankSame
      rfl

private theorem max_step_both {a b a' b' : Nat} (ha : a' + 1 = a) (hb : b' + 1 = b) :
    max a' b' + 1 = max a b := by
  simp only [Nat.max_def]
  split <;> split <;> omega

theorem kingDistance_toward {source target : Square} (different : source ≠ target) :
    kingDistance (toward source target) target + 1 = kingDistance source target := by
  have fileStep := Coordinate.toward_distance source.file target.file
  have rankStep := Coordinate.toward_distance source.rank target.rank
  by_cases fileSame : source.file = target.file
  · have fileToward : Coordinate.toward source.file target.file = target.file := by
      simp [fileSame, Coordinate.toward]
    have rankDifferent : source.rank ≠ target.rank := by
      intro rankSame
      apply different
      cases source
      cases target
      simp_all
    have rankDecrease : absDiff (Coordinate.toward source.rank target.rank).val target.rank.val + 1 =
        absDiff source.rank.val target.rank.val := by simpa [rankDifferent] using rankStep
    simpa [kingDistance, toward, fileSame, absDiff_self] using rankDecrease
  · have fileDecrease : absDiff (Coordinate.toward source.file target.file).val target.file.val + 1 =
        absDiff source.file.val target.file.val := by simpa [fileSame] using fileStep
    by_cases rankSame : source.rank = target.rank
    · have rankToward : Coordinate.toward source.rank target.rank = target.rank := by
        simp [rankSame, Coordinate.toward]
      simpa [kingDistance, toward, rankSame, absDiff_self] using fileDecrease
    · have rankDecrease : absDiff (Coordinate.toward source.rank target.rank).val target.rank.val + 1 =
          absDiff source.rank.val target.rank.val := by simpa [rankSame] using rankStep
      unfold kingDistance toward
      exact max_step_both fileDecrease rankDecrease

/-- Chebyshev distance is achievable: a king has a route of exactly that many
moves between any two squares on an otherwise empty board. -/
theorem exists_walk_at_kingDistance (source target : Square) :
    KingWalk (kingDistance source target) source target := by
  generalize distanceEq : kingDistance source target = distance
  induction distance generalizing source with
  | zero =>
      have fileZero : absDiff source.file.val target.file.val = 0 := by
        unfold kingDistance at distanceEq
        omega
      have rankZero : absDiff source.rank.val target.rank.val = 0 := by
        unfold kingDistance at distanceEq
        omega
      have sourceEq : source = target := by
        cases source with
        | mk sf sr =>
          cases target with
          | mk tf tr =>
            have hf : sf = tf := by
              apply Fin.ext
              simp only [absDiff] at fileZero
              split at fileZero <;> omega
            have hr : sr = tr := by
              apply Fin.ext
              simp only [absDiff] at rankZero
              split at rankZero <;> omega
            simp [hf, hr]
      rw [sourceEq]
      exact KingWalk.nil target
  | succ distance ih =>
      have different : source ≠ target := by
        intro same
        subst source
        simp at distanceEq
      have decreases := kingDistance_toward different
      have nextDistance : kingDistance (toward source target) target = distance := by omega
      have rest := ih (toward source target) nextDistance
      exact KingWalk.step (adjacent_toward different) rest

/-- Exactness theorem: a geometric king walk of length `n` exists exactly when
Chebyshev distance is at most `n` (extra moves can be treated separately when
waiting moves matter). The minimal possible length is therefore exactly
`kingDistance`. -/
theorem kingDistance_is_minimum (source target : Square) :
    KingWalk (kingDistance source target) source target ∧
      ∀ {length}, KingWalk length source target → kingDistance source target ≤ length :=
  ⟨exists_walk_at_kingDistance source target, fun walk => kingDistance_le_of_walk walk⟩

end Chess.Theory
