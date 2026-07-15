import Chess.Geometry
import Chess.Piece

namespace Chess

/-- A proof-facing chessboard: each square is either empty or contains a piece.

This extensional representation is intentionally independent of an eventual
bitboard implementation. -/
structure Board where
  pieceAt : Square → Option Piece

namespace Board

/-- The empty board, useful for constructing and analyzing composed positions. -/
def empty : Board := ⟨fun _ => none⟩

/-- Replace the contents of one square. -/
def set (board : Board) (square : Square) (piece : Option Piece) : Board :=
  ⟨fun target => if target = square then piece else board.pieceAt target⟩

/-- Empty one square. -/
def clear (board : Board) (square : Square) : Board := board.set square none

/-- Move a piece between squares without checking chess legality. -/
def move (board : Board) (source target : Square) : Board :=
  (board.clear source).set target (board.pieceAt source)

/-- Executable extensional equality over all 64 squares. -/
def same (left right : Board) : Bool :=
  Square.all.all fun square => left.pieceAt square == right.pieceAt square

@[ext] theorem ext {left right : Board} (h : ∀ square, left.pieceAt square = right.pieceAt square) :
    left = right := by
  cases left with
  | mk leftAt =>
    cases right with
    | mk rightAt =>
      congr
      funext square
      exact h square

@[simp] theorem empty_pieceAt (square : Square) : empty.pieceAt square = none := rfl

@[simp] theorem set_at (board : Board) (square : Square) (piece : Option Piece) :
    (board.set square piece).pieceAt square = piece := by
  simp [set]

theorem set_at_other (board : Board) {changed target : Square} (h : target ≠ changed)
    (piece : Option Piece) :
    (board.set changed piece).pieceAt target = board.pieceAt target := by
  simp [set, h]

@[simp] theorem clear_at (board : Board) (square : Square) :
    (board.clear square).pieceAt square = none := by
  simp [clear]

@[simp] theorem move_at_target (board : Board) (source target : Square) :
    (board.move source target).pieceAt target = board.pieceAt source := by
  simp [move]

@[simp] theorem move_at_source (board : Board) {source target : Square} (h : source ≠ target) :
    (board.move source target).pieceAt source = none := by
  unfold move
  rw [set_at_other (board.clear source) h]
  exact clear_at board source

@[simp] theorem same_self (board : Board) : board.same board := by
  simp [same]

end Board
end Chess
