import Chess.Initial
import Chess.Rules

namespace Chess.RulesExamples

private def sq (file rank : Coordinate) : Square := Square.ofCoords file rank

private def e2e4 : Move := ⟨sq 4 1, sq 4 3, none⟩
private def e2e5 : Move := ⟨sq 4 1, sq 4 4, none⟩
private def g1f3 : Move := ⟨sq 6 0, sq 5 2, none⟩
private def f1b5 : Move := ⟨sq 5 0, sq 1 4, none⟩

/-- The initial legal-move count is the first standard perft value. -/
theorem initial_perft_one : (legalMoves Initial.position).length = 20 := by native_decide

/-- The second standard perft value checks all legal replies to every first move. -/
theorem initial_perft_two : perft 2 Initial.position = 400 := by native_decide

/-- Depth three exercises 8,902 distinct move sequences from the initial state. -/
theorem initial_perft_three : perft 3 Initial.position = 8902 := by native_decide

example : Legal Initial.position e2e4 := by decide
example : ¬Legal Initial.position e2e5 := by decide
example : Legal Initial.position g1f3 := by decide
example : ¬Legal Initial.position f1b5 := by decide

private def afterE4 : Position := applyUnchecked Initial.position e2e4

example : afterE4.board.pieceAt (sq 4 3) = some ⟨.white, .pawn⟩ := by decide
example : afterE4.board.pieceAt (sq 4 1) = none := by decide
example : afterE4.turn = .black := by decide
example : afterE4.enPassantTarget = some (sq 4 2) := by decide
example : afterE4.halfmoveClock = 0 := by decide
example : afterE4.fullmoveNumber = 1 := by decide

private def analysisPosition (board : Board) (turn : Color) : Position where
  board := board
  turn := turn
  castlingRights := .none
  enPassantTarget := none
  halfmoveClock := 0
  fullmoveNumber := 1

private def pinnedRookBoard : Board :=
  Board.empty
    |>.set (sq 4 0) (some ⟨.white, .king⟩)
    |>.set (sq 4 1) (some ⟨.white, .rook⟩)
    |>.set (sq 4 7) (some ⟨.black, .rook⟩)
    |>.set (sq 0 7) (some ⟨.black, .king⟩)

private def pinnedRookPosition : Position := analysisPosition pinnedRookBoard .white
private def pinnedRookMove : Move := ⟨sq 4 1, sq 0 1, none⟩

/-- FIDE attack semantics: the absolutely pinned rook still attacks horizontally. -/
theorem pinned_piece_still_attacks :
    AttackedBy pinnedRookBoard .white (sq 0 1) := by
  exact attackedBy_iff pinnedRookBoard .white (sq 0 1) |>.mp (by decide)

/-- But moving that rook away is illegal because it exposes its own king. -/
theorem pinned_piece_cannot_expose_king : ¬Legal pinnedRookPosition pinnedRookMove := by decide

private def castleBoard : Board :=
  Board.empty
    |>.set Square.e1 (some ⟨.white, .king⟩)
    |>.set Square.h1 (some ⟨.white, .rook⟩)
    |>.set Square.e8 (some ⟨.black, .king⟩)

private def castlePosition : Position where
  board := castleBoard
  turn := .white
  castlingRights := ⟨true, false, false, false⟩
  enPassantTarget := none
  halfmoveClock := 0
  fullmoveNumber := 1

private def whiteKingSideCastle : Move := ⟨Square.e1, Square.g1, none⟩

example : Legal castlePosition whiteKingSideCastle := by decide
example :
    (applyUnchecked castlePosition whiteKingSideCastle).board.pieceAt Square.g1 =
      some ⟨.white, .king⟩ := by decide
example :
    (applyUnchecked castlePosition whiteKingSideCastle).board.pieceAt Square.f1 =
      some ⟨.white, .rook⟩ := by decide

private def castleThroughCheckBoard : Board :=
  castleBoard.set Square.f8 (some ⟨.black, .rook⟩)

private def castleThroughCheckPosition : Position :=
  { castlePosition with board := castleThroughCheckBoard }

/-- Castling through an attacked transit square is illegal. -/
theorem cannot_castle_through_check :
    ¬Legal castleThroughCheckPosition whiteKingSideCastle := by decide

end Chess.RulesExamples
