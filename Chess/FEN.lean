import Std.Data.String.ToNat
import Chess.Position

namespace Chess.FEN

private def piece? : Char → Option Piece
  | 'K' => some ⟨.white, .king⟩
  | 'Q' => some ⟨.white, .queen⟩
  | 'R' => some ⟨.white, .rook⟩
  | 'B' => some ⟨.white, .bishop⟩
  | 'N' => some ⟨.white, .knight⟩
  | 'P' => some ⟨.white, .pawn⟩
  | 'k' => some ⟨.black, .king⟩
  | 'q' => some ⟨.black, .queen⟩
  | 'r' => some ⟨.black, .rook⟩
  | 'b' => some ⟨.black, .bishop⟩
  | 'n' => some ⟨.black, .knight⟩
  | 'p' => some ⟨.black, .pawn⟩
  | _ => none

private def emptyCount? : Char → Option Nat
  | '1' => some 1
  | '2' => some 2
  | '3' => some 3
  | '4' => some 4
  | '5' => some 5
  | '6' => some 6
  | '7' => some 7
  | '8' => some 8
  | _ => none

private def parseRank (rank : Coordinate) : List Char → Nat → Board → Except String Board
  | [], file, board =>
      if file == 8 then .ok board else .error "FEN rank does not contain eight squares"
  | symbol :: rest, file, board =>
      match emptyCount? symbol with
      | some count =>
          if file + count ≤ 8 then parseRank rank rest (file + count) board
          else .error "FEN rank extends beyond the board"
      | none =>
          match piece? symbol with
          | none => .error s!"invalid FEN piece symbol: {symbol}"
          | some piece =>
              if onBoard : file < 8 then
                let square : Square := ⟨⟨file, onBoard⟩, rank⟩
                parseRank rank rest (file + 1) (board.set square (some piece))
              else
                .error "FEN rank extends beyond the board"

private def parseRanks : List String → List Coordinate → Board → Except String Board
  | [], [], board => .ok board
  | rankText :: restText, rank :: restRanks, board => do
      let board ← parseRank rank rankText.toList 0 board
      parseRanks restText restRanks board
  | _, _, _ => .error "FEN placement must contain exactly eight ranks"

private def parseBoard (text : String) : Except String Board :=
  parseRanks (text.splitOn "/") [7, 6, 5, 4, 3, 2, 1, 0] Board.empty

private def parseTurn : String → Except String Color
  | "w" => .ok .white
  | "b" => .ok .black
  | _ => .error "FEN active color must be w or b"

private def addCastlingRight (rights : CastlingRights) : Char → Except String CastlingRights
  | 'K' => if rights.whiteKingSide then .error "duplicate FEN castling right K"
      else .ok { rights with whiteKingSide := true }
  | 'Q' => if rights.whiteQueenSide then .error "duplicate FEN castling right Q"
      else .ok { rights with whiteQueenSide := true }
  | 'k' => if rights.blackKingSide then .error "duplicate FEN castling right k"
      else .ok { rights with blackKingSide := true }
  | 'q' => if rights.blackQueenSide then .error "duplicate FEN castling right q"
      else .ok { rights with blackQueenSide := true }
  | _ => .error "invalid FEN castling rights"

private def parseCastling (text : String) : Except String CastlingRights :=
  if text == "-" then .ok .none
  else if text.isEmpty then .error "empty FEN castling rights"
  else text.toList.foldlM addCastlingRight .none

private def file? : Char → Option Coordinate
  | 'a' => some 0
  | 'b' => some 1
  | 'c' => some 2
  | 'd' => some 3
  | 'e' => some 4
  | 'f' => some 5
  | 'g' => some 6
  | 'h' => some 7
  | _ => none

private def enPassantRank? : Char → Option Coordinate
  | '3' => some 2
  | '6' => some 5
  | _ => none

private def parseEnPassant (text : String) : Except String (Option Square) :=
  if text == "-" then .ok none
  else match text.toList with
    | [fileSymbol, rankSymbol] =>
        match file? fileSymbol, enPassantRank? rankSymbol with
        | some file, some rank => .ok (some ⟨file, rank⟩)
        | _, _ => .error "invalid FEN en-passant target"
    | _ => .error "invalid FEN en-passant target"

private def parseNat (fieldName text : String) : Except String Nat :=
  match text.toNat? with
  | some value => .ok value
  | none => .error s!"invalid FEN {fieldName}"

/-- Parse the six fields of standard Forsyth-Edwards Notation. This establishes
syntactic validity; chess reachability is a separate semantic predicate. -/
def parse (text : String) : Except String Position := do
  match text.splitOn " " with
  | [placement, activeColor, castling, enPassant, halfmove, fullmove] =>
      let board ← parseBoard placement
      let turn ← parseTurn activeColor
      let castlingRights ← parseCastling castling
      let enPassantTarget ← parseEnPassant enPassant
      let halfmoveClock ← parseNat "halfmove clock" halfmove
      let fullmoveNumber ← parseNat "fullmove number" fullmove
      if fullmoveNumber == 0 then
        .error "FEN fullmove number must be positive"
      else
        .ok { board, turn, castlingRights, enPassantTarget, halfmoveClock, fullmoveNumber }
  | _ => .error "FEN must contain exactly six space-separated fields"

end Chess.FEN
