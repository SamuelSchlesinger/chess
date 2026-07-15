import Chess.Position

namespace Chess

namespace Initial

private def backRankKind (file : Coordinate) : PieceKind :=
  match file.val with
  | 0 => .rook
  | 1 => .knight
  | 2 => .bishop
  | 3 => .queen
  | 4 => .king
  | 5 => .bishop
  | 6 => .knight
  | _ => .rook

/-- Piece placement in the standard initial position. -/
def board : Board := ⟨fun square =>
  if square.rank = 0 then
    some ⟨.white, backRankKind square.file⟩
  else if square.rank = 1 then
    some ⟨.white, .pawn⟩
  else if square.rank = 6 then
    some ⟨.black, .pawn⟩
  else if square.rank = 7 then
    some ⟨.black, backRankKind square.file⟩
  else
    none⟩

/-- The standard initial orthodox-chess position. -/
def position : Position where
  board := board
  turn := .white
  castlingRights := .initial
  enPassantTarget := none
  halfmoveClock := 0
  fullmoveNumber := 1

/-- A new game, before White's first move. -/
def game : GameState where
  current := position
  prior := []

@[simp] theorem white_king : board.pieceAt Square.e1 = some ⟨.white, .king⟩ := by decide
@[simp] theorem black_king : board.pieceAt Square.e8 = some ⟨.black, .king⟩ := by decide
@[simp] theorem a1_rook : board.pieceAt Square.a1 = some ⟨.white, .rook⟩ := by decide
@[simp] theorem h8_rook : board.pieceAt Square.h8 = some ⟨.black, .rook⟩ := by decide
@[simp] theorem white_to_move : position.turn = .white := rfl
@[simp] theorem no_en_passant : position.enPassantTarget = none := rfl

end Initial
end Chess
