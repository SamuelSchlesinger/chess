import Chess.Attack

namespace Chess.AttackExamples

private def d4 : Square := Square.ofCoords 3 3
private def d5 : Square := Square.ofCoords 3 4
private def d6 : Square := Square.ofCoords 3 5
private def d7 : Square := Square.ofCoords 3 6
private def e5 : Square := Square.ofCoords 4 4
private def e4 : Square := Square.ofCoords 4 3

private def whiteRook : Piece := ⟨.white, .rook⟩
private def whitePawn : Piece := ⟨.white, .pawn⟩
private def blackPawn : Piece := ⟨.black, .pawn⟩

private def blockedRookBoard : Board :=
  (Board.empty.set d4 (some whiteRook)).set d6 (some blackPawn)

/-- A rook attacks the first blocker. -/
example : PieceAttacks blockedRookBoard d4 whiteRook d6 := by decide

/-- That blocker screens the next square on the same ray. -/
example : ¬PieceAttacks blockedRookBoard d4 whiteRook d7 := by decide

/-- Pawns attack diagonally even when the target is empty. -/
example : PieceAttacks Board.empty d4 whitePawn e5 := by decide

/-- A pawn's forward movement square is not a pawn attack. -/
example : ¬PieceAttacks Board.empty d4 whitePawn d5 := by decide

/-- The side-adjacent square is not a pawn attack either. -/
example : ¬PieceAttacks Board.empty d4 whitePawn e4 := by decide

end Chess.AttackExamples
