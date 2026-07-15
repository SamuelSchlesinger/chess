import Std.Data.String.ToNat
import Chess.Game
import Chess.UCI

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

private def parseRank (rank : Coordinate) : List Char → Nat → Bool → Board → Except String Board
  | [], file, _, board =>
      if file == 8 then .ok board else .error "FEN rank does not contain eight squares"
  | symbol :: rest, file, previousWasEmpty, board =>
      match emptyCount? symbol with
      | some count =>
          if previousWasEmpty then
            .error "adjacent FEN empty-square counts are not canonical"
          else if file + count ≤ 8 then
            parseRank rank rest (file + count) true board
          else
            .error "FEN rank extends beyond the board"
      | none =>
          match piece? symbol with
          | none => .error s!"invalid FEN piece symbol: {symbol}"
          | some piece =>
              if onBoard : file < 8 then
                let square : Square := ⟨⟨file, onBoard⟩, rank⟩
                parseRank rank rest (file + 1) false (board.set square (some piece))
              else
                .error "FEN rank extends beyond the board"

private def parseRanks : List String → List Coordinate → Board → Except String Board
  | [], [], board => .ok board
  | rankText :: restText, rank :: restRanks, board => do
      let board ← parseRank rank rankText.toList 0 false board
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

private def pieceChar : Piece → Char
  | ⟨.white, .king⟩ => 'K'
  | ⟨.white, .queen⟩ => 'Q'
  | ⟨.white, .rook⟩ => 'R'
  | ⟨.white, .bishop⟩ => 'B'
  | ⟨.white, .knight⟩ => 'N'
  | ⟨.white, .pawn⟩ => 'P'
  | ⟨.black, .king⟩ => 'k'
  | ⟨.black, .queen⟩ => 'q'
  | ⟨.black, .rook⟩ => 'r'
  | ⟨.black, .bishop⟩ => 'b'
  | ⟨.black, .knight⟩ => 'n'
  | ⟨.black, .pawn⟩ => 'p'

private def emptyRun : Nat → List Char
  | 0 => []
  | 1 => ['1']
  | 2 => ['2']
  | 3 => ['3']
  | 4 => ['4']
  | 5 => ['5']
  | 6 => ['6']
  | 7 => ['7']
  | _ => ['8']

private def renderRank (board : Board) (rank : Coordinate) : String :=
  let result := Square.allCoordinates.foldl
    (fun (state : Nat × List Char) file =>
      match board.pieceAt ⟨file, rank⟩ with
      | none => (state.1 + 1, state.2)
      | some piece => (0, state.2 ++ emptyRun state.1 ++ [pieceChar piece]))
    (0, [])
  String.ofList (result.2 ++ emptyRun result.1)

private def renderBoard (board : Board) : String :=
  String.intercalate "/"
    (([7, 6, 5, 4, 3, 2, 1, 0] : List Coordinate).map (renderRank board))

private def renderTurn : Color → String
  | .white => "w"
  | .black => "b"

private def renderCastling (rights : CastlingRights) : String :=
  let symbols :=
    (if rights.whiteKingSide then ['K'] else []) ++
    (if rights.whiteQueenSide then ['Q'] else []) ++
    (if rights.blackKingSide then ['k'] else []) ++
    (if rights.blackQueenSide then ['q'] else [])
  if symbols.isEmpty then "-" else String.ofList symbols

private def renderEnPassant : Option Square → String
  | none => "-"
  | some target => UCI.renderSquare target

/-- Which en-passant convention to use when rendering FEN. Standard raw FEN
records the square passed over by every double pawn move. Some engines instead
emit that square only when an en-passant capture is actually legal; this is the
same normalization used by FIDE repetition identity. -/
inductive EnPassantMode where
  | raw
  | effective
  deriving DecidableEq, Repr

/-- Render all six fields without checking whether the unconstrained Lean
`Position` is representable by standard FEN. This is intended for diagnostics;
use `render`, `renderRaw`, or `renderEffective` for interchange. -/
def renderUnchecked (position : Position) (mode : EnPassantMode := .raw) : String :=
  let enPassantTarget := match mode with
    | .raw => position.enPassantTarget
    | .effective => effectiveEnPassantTarget position
  String.intercalate " "
    [renderBoard position.board,
      renderTurn position.turn,
      renderCastling position.castlingRights,
      renderEnPassant enPassantTarget,
      toString position.halfmoveClock,
      toString position.fullmoveNumber]

private def validateForRendering (position : Position) : Except String Unit := do
  if position.fullmoveNumber == 0 then
    .error "FEN fullmove number must be positive"
  match position.enPassantTarget with
  | none => pure ()
  | some target =>
      if target.rank.val == 2 || target.rank.val == 5 then
        pure ()
      else
        .error "FEN en-passant target must be on rank 3 or rank 6"

/-- Render a standard six-field FEN after checking the representation-level
constraints not enforced by the deliberately permissive `Position` structure.
The default preserves the raw en-passant field. Effective rendering normalizes
that field, but is not itself a repetition key because FEN retains both clocks. -/
def render (position : Position) (mode : EnPassantMode := .raw) : Except String String := do
  validateForRendering position
  pure (renderUnchecked position mode)

def renderRaw (position : Position) : Except String String := render position .raw
def renderEffective (position : Position) : Except String String := render position .effective

end Chess.FEN
