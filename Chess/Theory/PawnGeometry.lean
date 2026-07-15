import Chess.Theory.KingGeometry
import Chess.Piece

namespace Chess.Theory

/-- A king can reach a square within a move budget when some geometric king walk
uses no more than that many moves. Occupancy and enemy attacks are intentionally
absent: this is the geometry used in pawn-race calculations. -/
def KingCanReachWithin (budget : Nat) (source target : Square) : Prop :=
  ∃ length, length ≤ budget ∧ KingWalk length source target

/-- Exact deadline theorem for a king. This is a reusable structural result,
not a search through the 64 squares. -/
theorem kingCanReachWithin_iff (budget : Nat) (source target : Square) :
    KingCanReachWithin budget source target ↔ kingDistance source target ≤ budget := by
  constructor
  · rintro ⟨length, withinBudget, walk⟩
    exact Nat.le_trans (kingDistance_le_of_walk walk) withinBudget
  · intro withinBudget
    exact ⟨kingDistance source target, withinBudget,
      exists_walk_at_kingDistance source target⟩

/-- The promotion square of a pawn, preserving its file. -/
def promotionSquare (color : Color) (pawn : Square) : Square :=
  match color with
  | .white => ⟨pawn.file, 7⟩
  | .black => ⟨pawn.file, 0⟩

/-- Number of unobstructed pawn moves to promotion, including the initial
double-step when the pawn is still on its home rank. The value is zero only for
an analysis input already on its promotion rank. -/
def pawnMovesToPromote (color : Color) (pawn : Square) : Nat :=
  match color with
  | .white => if pawn.rank = 1 then 5 else 7 - pawn.rank.val
  | .black => if pawn.rank = 6 then 5 else pawn.rank.val

/-- Number of king moves available before the pawn promotes when both sides
move greedily. If the pawn moves first, the king receives one fewer tempo. -/
def kingMoveBudget (pawnMovesFirst : Bool) (color : Color) (pawn : Square) : Nat :=
  pawnMovesToPromote color pawn - if pawnMovesFirst then 1 else 0

/-- The precise metric form of the pawn's square, adjusted for whose move it is
and for the pawn's initial double-step. -/
def InsidePawnSquare (pawnMovesFirst : Bool) (color : Color)
    (pawn king : Square) : Prop :=
  kingDistance king (promotionSquare color pawn) ≤ kingMoveBudget pawnMovesFirst color pawn

/-- The visual square criterion: membership means that both the file distance
and rank distance to the promotion square fit inside the same tempo budget. -/
theorem insidePawnSquare_iff_components (pawnMovesFirst : Bool) (color : Color)
    (pawn king : Square) :
    InsidePawnSquare pawnMovesFirst color pawn king ↔
      absDiff king.file.val pawn.file.val ≤ kingMoveBudget pawnMovesFirst color pawn ∧
      absDiff king.rank.val (promotionSquare color pawn).rank.val ≤
        kingMoveBudget pawnMovesFirst color pawn := by
  unfold InsidePawnSquare kingDistance promotionSquare
  cases color <;> simp only
  · exact Nat.max_le
  · exact Nat.max_le

/-- **Geometric rule of the square.** A king can reach the pawn's promotion
square by the promotion deadline exactly when it begins inside the correctly
tempo-adjusted square.

This theorem isolates the exact content of the mnemonic. A later KPK theorem
adds occupied squares, king opposition, protection by the attacking king, and
rook-pawn exceptions rather than silently attributing those complications to
the square rule itself. -/
theorem ruleOfSquare (pawnMovesFirst : Bool) (color : Color) (pawn king : Square) :
    KingCanReachWithin (kingMoveBudget pawnMovesFirst color pawn)
        king (promotionSquare color pawn) ↔
      InsidePawnSquare pawnMovesFirst color pawn king := by
  exact kingCanReachWithin_iff _ _ _

/-- Moving first changes the geometric deadline by exactly one tempo whenever
the pawn still needs at least one move to promote. -/
theorem defender_first_extra_tempo (color : Color) (pawn : Square)
    (hasMove : 0 < pawnMovesToPromote color pawn) :
    kingMoveBudget false color pawn = kingMoveBudget true color pawn + 1 := by
  simp [kingMoveBudget]
  omega

end Chess.Theory
