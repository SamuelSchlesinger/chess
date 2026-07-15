import Chess.Move

namespace Chess.UCI

inductive ParseError where
  | wrongLength (actual : Nat)
  | invalidSource (file rank : Char)
  | invalidTarget (file rank : Char)
  | invalidPromotion (symbol : Char)
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq

instance : ToString ParseError where
  toString
    | .wrongLength actual =>
        s!"UCI move must contain four characters, plus an optional promotion piece; got {actual}"
    | .invalidSource file rank => s!"invalid UCI source square: {file}{rank}"
    | .invalidTarget file rank => s!"invalid UCI target square: {file}{rank}"
    | .invalidPromotion symbol => s!"invalid UCI promotion piece: {symbol}"

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

private def rank? : Char → Option Coordinate
  | '1' => some 0
  | '2' => some 1
  | '3' => some 2
  | '4' => some 3
  | '5' => some 4
  | '6' => some 5
  | '7' => some 6
  | '8' => some 7
  | _ => none

private def fileChar (file : Coordinate) : Char :=
  match file.val with
  | 0 => 'a'
  | 1 => 'b'
  | 2 => 'c'
  | 3 => 'd'
  | 4 => 'e'
  | 5 => 'f'
  | 6 => 'g'
  | _ => 'h'

private def rankChar (rank : Coordinate) : Char :=
  match rank.val with
  | 0 => '1'
  | 1 => '2'
  | 2 => '3'
  | 3 => '4'
  | 4 => '5'
  | 5 => '6'
  | 6 => '7'
  | _ => '8'

private theorem coordinate_cases (coordinate : Coordinate) :
    coordinate = 0 ∨ coordinate = 1 ∨ coordinate = 2 ∨ coordinate = 3 ∨
      coordinate = 4 ∨ coordinate = 5 ∨ coordinate = 6 ∨ coordinate = 7 := by
  rcases coordinate with ⟨value, bound⟩
  have cases : value = 0 ∨ value = 1 ∨ value = 2 ∨ value = 3 ∨
      value = 4 ∨ value = 5 ∨ value = 6 ∨ value = 7 := by
    omega
  rcases cases with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> simp

@[simp] private theorem file?_fileChar (file : Coordinate) :
    file? (fileChar file) = some file := by
  rcases coordinate_cases file with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> rfl

@[simp] private theorem rank?_rankChar (rank : Coordinate) :
    rank? (rankChar rank) = some rank := by
  rcases coordinate_cases rank with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> rfl

private def squareOfChars (fileSymbol rankSymbol : Char) : Except String Square :=
  match file? fileSymbol, rank? rankSymbol with
  | some file, some rank => .ok ⟨file, rank⟩
  | _, _ => .error s!"invalid UCI square: {fileSymbol}{rankSymbol}"

/-- Parse an ASCII square such as `e4`. -/
def parseSquare (text : String) : Except String Square :=
  match text.toList with
  | [fileSymbol, rankSymbol] => squareOfChars fileSymbol rankSymbol
  | _ => .error "UCI square must contain exactly two ASCII characters"

/-- Render a square in lowercase UCI coordinate form. -/
def renderSquare (square : Square) : String :=
  String.ofList [fileChar square.file, rankChar square.rank]

private def promotion? : Char → Option PromotionPiece
  | 'q' => some .queen
  | 'r' => some .rook
  | 'b' => some .bishop
  | 'n' => some .knight
  | _ => none

private def promotionChar : PromotionPiece → Char
  | .queen => 'q'
  | .rook => 'r'
  | .bishop => 'b'
  | .knight => 'n'

/-- Parse the UCI coordinate encoding of a chess move. Protocol-level null
moves such as `0000` are deliberately excluded because they are not chess
moves. Legality in a particular position is checked separately. -/
def parse (text : String) : Except ParseError Move :=
  let symbols := text.toList
  match symbols with
  | [sourceFile, sourceRank, targetFile, targetRank] => do
      let source ← match file? sourceFile, rank? sourceRank with
        | some file, some rank => .ok ⟨file, rank⟩
        | _, _ => .error (.invalidSource sourceFile sourceRank)
      let target ← match file? targetFile, rank? targetRank with
        | some file, some rank => .ok ⟨file, rank⟩
        | _, _ => .error (.invalidTarget targetFile targetRank)
      pure ⟨source, target, none⟩
  | [sourceFile, sourceRank, targetFile, targetRank, promotionSymbol] => do
      let source ← match file? sourceFile, rank? sourceRank with
        | some file, some rank => .ok ⟨file, rank⟩
        | _, _ => .error (.invalidSource sourceFile sourceRank)
      let target ← match file? targetFile, rank? targetRank with
        | some file, some rank => .ok ⟨file, rank⟩
        | _, _ => .error (.invalidTarget targetFile targetRank)
      match promotion? promotionSymbol with
      | some promotion => pure ⟨source, target, some promotion⟩
      | none => .error (.invalidPromotion promotionSymbol)
  | _ => .error (.wrongLength symbols.length)

/-- Canonical lowercase UCI rendering. -/
def render (move : Move) : String :=
  renderSquare move.source ++ renderSquare move.target ++
    match move.promotion with
    | none => ""
    | some promotion => String.singleton (promotionChar promotion)

/-- Canonical rendering is a left inverse of syntactic UCI parsing for every
raw orthodox move value. Position-dependent legality remains separate. -/
theorem parse_render (move : Move) : parse (render move) = .ok move := by
  rcases move with ⟨⟨sourceFile, sourceRank⟩, ⟨targetFile, targetRank⟩, promotion⟩
  cases promotion with
  | none =>
      simp [render, renderSquare, parse] <;> rfl
  | some promotion =>
      cases promotion <;>
        simp [render, renderSquare, parse, promotionChar, promotion?] <;> rfl

/-- Distinct raw moves have distinct canonical UCI encodings. -/
theorem render_injective : Function.Injective render := by
  intro left right equal
  have parsed := congrArg parse equal
  simpa only [parse_render, Except.ok.injEq] using parsed

end Chess.UCI
