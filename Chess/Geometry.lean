import Std.Tactic

namespace Chess

/-- A file or rank index, represented internally from `0` through `7`. -/
abbrev Coordinate := Fin 8

/-- A square on an orthodox 8-by-8 chessboard.

Files and ranks are zero-based internally: `(0, 0)` is `a1` and `(7, 7)` is
`h8`. Named notation and FEN parsing will provide the usual chess-facing view.
-/
structure Square where
  file : Coordinate
  rank : Coordinate
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

namespace Square

/-- Construct a square from its zero-based file and rank. -/
def ofCoords (file rank : Coordinate) : Square := ⟨file, rank⟩

def a1 : Square := ofCoords 0 0
def b1 : Square := ofCoords 1 0
def c1 : Square := ofCoords 2 0
def d1 : Square := ofCoords 3 0
def e1 : Square := ofCoords 4 0
def f1 : Square := ofCoords 5 0
def g1 : Square := ofCoords 6 0
def h1 : Square := ofCoords 7 0

def a8 : Square := ofCoords 0 7
def b8 : Square := ofCoords 1 7
def c8 : Square := ofCoords 2 7
def d8 : Square := ofCoords 3 7
def e8 : Square := ofCoords 4 7
def f8 : Square := ofCoords 5 7
def g8 : Square := ofCoords 6 7
def h8 : Square := ofCoords 7 7

theorem ofCoords_injective : Function.Injective (fun p : Coordinate × Coordinate =>
    ofCoords p.1 p.2) := by
  intro x y h
  cases x with
  | mk xf xr =>
    cases y with
    | mk yf yr =>
      simp only [ofCoords] at h
      cases h
      rfl

end Square

/-- A signed displacement on the board. Positive rank displacement points
towards Black's back rank, i.e. in White's pawn direction. -/
structure Direction where
  fileDelta : Int
  rankDelta : Int
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

namespace Direction

def north : Direction := ⟨0, 1⟩
def south : Direction := ⟨0, -1⟩
def east : Direction := ⟨1, 0⟩
def west : Direction := ⟨-1, 0⟩
def northEast : Direction := ⟨1, 1⟩
def northWest : Direction := ⟨-1, 1⟩
def southEast : Direction := ⟨1, -1⟩
def southWest : Direction := ⟨-1, -1⟩

def orthogonal : List Direction := [north, south, east, west]
def diagonal : List Direction := [northEast, northWest, southEast, southWest]
def queen : List Direction := orthogonal ++ diagonal

end Direction

namespace Coordinate

/-- Translate a coordinate, returning `none` exactly when it leaves the board. -/
def offset (coordinate : Coordinate) (delta : Int) : Option Coordinate :=
  let result := (coordinate.val : Int) + delta
  if lower : 0 ≤ result then
    if upper : result < 8 then
      some ⟨result.toNat, by omega⟩
    else
      none
  else
    none

@[simp] theorem offset_zero (coordinate : Coordinate) : coordinate.offset 0 = some coordinate := by
  have lower : (0 : Int) ≤ (coordinate.val : Int) := by omega
  have upper : (coordinate.val : Int) < 8 := by omega
  simp only [offset, Int.add_zero, dif_pos lower, dif_pos upper]
  congr

end Coordinate

namespace Square

/-- Translate a square by a signed direction, if the result remains on-board. -/
def offset (square : Square) (direction : Direction) : Option Square := do
  let file ← square.file.offset direction.fileDelta
  let rank ← square.rank.offset direction.rankDelta
  pure ⟨file, rank⟩

/-- All coordinates, in increasing internal order. -/
def allCoordinates : List Coordinate := List.finRange 8

/-- All 64 squares, in rank-major order from `a1` through `h8`. -/
def all : List Square := allCoordinates.flatMap fun rank =>
  allCoordinates.map fun file => ofCoords file rank

@[simp] theorem offset_zero (square : Square) : square.offset ⟨0, 0⟩ = some square := by
  simp [offset, Coordinate.offset_zero]

theorem mem_all (square : Square) : square ∈ all := by
  rcases square with ⟨file, rank⟩
  apply List.mem_flatMap.mpr
  refine ⟨rank, List.mem_finRange rank, ?_⟩
  apply List.mem_map.mpr
  exact ⟨file, List.mem_finRange file, rfl⟩

theorem rank_succ_of_offset {source target : Square} {direction : Direction}
    (rankStep : direction.rankDelta = 1)
    (step : source.offset direction = some target) :
    target.rank.val = source.rank.val + 1 := by
  rcases source with ⟨sourceFile, sourceRank⟩
  rcases target with ⟨targetFile, targetRank⟩
  rcases direction with ⟨fileDelta, rankDelta⟩
  simp only at rankStep
  subst rankDelta
  simp [Square.offset, Coordinate.offset] at step
  split at step <;> simp_all
  split at step <;> simp_all
  have rankLower : (0 : Int) ≤ (sourceRank.val : Int) + 1 := by omega
  simp [rankLower] at step
  split at step
  · simp at step
    have rankEq := congrArg Fin.val step.2
    simp at rankEq
    omega
  · simp_all

theorem rank_pred_of_offset {source target : Square} {direction : Direction}
    (rankStep : direction.rankDelta = -1)
    (step : source.offset direction = some target) :
    target.rank.val + 1 = source.rank.val := by
  rcases source with ⟨sourceFile, sourceRank⟩
  rcases target with ⟨targetFile, targetRank⟩
  rcases direction with ⟨fileDelta, rankDelta⟩
  simp only at rankStep
  subst rankDelta
  simp [Square.offset, Coordinate.offset] at step
  split at step <;> simp_all
  split at step <;> simp_all
  have rankUpper : (sourceRank.val : Int) + (-1) < 8 := by omega
  simp [rankUpper] at step
  split at step
  · simp at step
    have rankEq := congrArg Fin.val step.2
    simp at rankEq
    omega
  · simp_all

end Square
end Chess
