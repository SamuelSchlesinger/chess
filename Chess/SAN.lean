import Chess.Game
import Chess.UCI

namespace Chess.SAN

/-!
Standard Algebraic Notation is position dependent: the same text can denote
different raw moves in different positions, and disambiguation is determined
by the other *legal* moves in the position.  This module therefore separates
syntax (`parse`) from checked resolution (`resolveLegal`).

Only the orthodox PGN spellings `O-O` and `O-O-O` are accepted for castling.
The visually similar zero spellings `0-0` and `0-0-0` are rejected rather than
silently normalized.
-/

/-- The semantic check marker carried by a SAN token. -/
inductive CheckSuffix where
  | none
  | check
  | mate
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq

/-- The non-castling constraints extracted from a SAN token. -/
structure OrdinaryConstraints where
  piece : PieceKind
  target : Square
  capture : Bool
  sourceFile : Option Coordinate
  sourceRank : Option Coordinate
  promotion : Option PromotionPiece
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq

/-- Position-independent SAN syntax, before legal-move resolution. -/
inductive MoveConstraints where
  | castle (side : CastleSide)
  | ordinary (constraints : OrdinaryConstraints)
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq

structure Constraints where
  move : MoveConstraints
  suffix : CheckSuffix
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq

inductive ParseError where
  | emptyToken
  | zeroCastlingNotAccepted
  | repeatedCheckSuffix
  | invalidDestination (file rank : Char)
  | invalidPromotion (symbol : Char)
  | promotionOnPieceMove
  | malformed (token : String)
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq

instance : ToString ParseError where
  toString
    | .emptyToken => "SAN token is empty"
    | .zeroCastlingNotAccepted =>
        "SAN castling uses the letter O: write O-O or O-O-O"
    | .repeatedCheckSuffix => "SAN has more than one check or mate suffix"
    | .invalidDestination file rank =>
        s!"invalid SAN destination square: {file}{rank}"
    | .invalidPromotion symbol =>
        s!"invalid SAN promotion piece: {symbol}"
    | .promotionOnPieceMove => "only a pawn move may contain a SAN promotion"
    | .malformed token => s!"malformed SAN token: {token}"

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

private def pieceKind? : Char → Option PieceKind
  | 'K' => some .king
  | 'Q' => some .queen
  | 'R' => some .rook
  | 'B' => some .bishop
  | 'N' => some .knight
  | _ => none

private def promotion? : Char → Option PromotionPiece
  | 'Q' => some .queen
  | 'R' => some .rook
  | 'B' => some .bishop
  | 'N' => some .knight
  | _ => none

private def hasCheckSymbol (symbols : List Char) : Bool :=
  symbols.any fun symbol => symbol == '+' || symbol == '#'

private def stripCheckSuffix (symbols : List Char) :
    Except ParseError (List Char × CheckSuffix) :=
  match symbols.reverse with
  | [] => .error .emptyToken
  | '+' :: rest =>
      if hasCheckSymbol rest then .error .repeatedCheckSuffix
      else .ok (rest.reverse, .check)
  | '#' :: rest =>
      if hasCheckSymbol rest then .error .repeatedCheckSuffix
      else .ok (rest.reverse, .mate)
  | _ =>
      if hasCheckSymbol symbols then .error .repeatedCheckSuffix
      else .ok (symbols, .none)

private def stripPromotion (token : String) (symbols : List Char) :
    Except ParseError (List Char × Option PromotionPiece) :=
  match symbols.reverse with
  | symbol :: '=' :: rest =>
      match promotion? symbol with
      | some promotion =>
          if rest.contains '=' then .error (.malformed token)
          else .ok (rest.reverse, some promotion)
      | none => .error (.invalidPromotion symbol)
  | _ =>
      if symbols.contains '=' then .error (.malformed token)
      else .ok (symbols, none)

private def splitDestination (token : String) (symbols : List Char) :
    Except ParseError (List Char × Square) :=
  match symbols.reverse with
  | rankSymbol :: fileSymbol :: rest =>
      match file? fileSymbol, rank? rankSymbol with
      | some file, some rank => .ok (rest.reverse, ⟨file, rank⟩)
      | _, _ => .error (.invalidDestination fileSymbol rankSymbol)
  | _ => .error (.malformed token)

private def parsePieceHint (token : String) : List Char →
    Except ParseError (Option Coordinate × Option Coordinate)
  | [] => .ok (none, none)
  | [symbol] =>
      match file? symbol, rank? symbol with
      | some file, _ => .ok (some file, none)
      | _, some rank => .ok (none, some rank)
      | _, _ => .error (.malformed token)
  | [fileSymbol, rankSymbol] =>
      match file? fileSymbol, rank? rankSymbol with
      | some file, some rank => .ok (some file, some rank)
      | _, _ => .error (.malformed token)
  | _ => .error (.malformed token)

private def parseOrdinary (token : String) (symbols : List Char)
    (suffix : CheckSuffix) : Except ParseError Constraints := do
  let (withoutPromotion, promotion) ← stripPromotion token symbols
  let (leading, target) ← splitDestination token withoutPromotion
  match leading with
  | [] =>
      pure {
        move := .ordinary {
          piece := .pawn
          target
          capture := false
          sourceFile := none
          sourceRank := none
          promotion
        }
        suffix
      }
  | first :: rest =>
      match pieceKind? first with
      | some piece =>
          if promotion.isSome then
            .error .promotionOnPieceMove
          else
            let (capture, hint) := match rest.reverse with
              | 'x' :: reversedHint => (true, reversedHint.reverse)
              | _ => (false, rest)
            let (sourceFile, sourceRank) ← parsePieceHint token hint
            pure {
              move := .ordinary {
                piece
                target
                capture
                sourceFile
                sourceRank
                promotion := none
              }
              suffix
            }
      | none =>
          match leading with
          | [sourceFileSymbol, 'x'] =>
              match file? sourceFileSymbol with
              | none => .error (.malformed token)
              | some sourceFile =>
                  pure {
                    move := .ordinary {
                      piece := .pawn
                      target
                      capture := true
                      sourceFile := some sourceFile
                      sourceRank := none
                      promotion
                    }
                    suffix
                  }
          | _ => .error (.malformed token)

/-- Parse standard orthodox SAN into position-independent constraints.

This is deliberately stricter than many interactive chess programs: it does
not accept zero castling, long algebraic notation, annotations such as `!?`,
or a trailing `e.p.` marker. -/
def parse (token : String) : Except ParseError Constraints := do
  let (core, suffix) ← stripCheckSuffix token.toList
  if core == ['0', '-', '0'] || core == ['0', '-', '0', '-', '0'] then
    .error .zeroCastlingNotAccepted
  else if core == ['O', '-', 'O'] then
    .ok { move := .castle .kingSide, suffix }
  else if core == ['O', '-', 'O', '-', 'O'] then
    .ok { move := .castle .queenSide, suffix }
  else
    parseOrdinary token core suffix

private def squareText (square : Square) : String :=
  String.ofList [fileChar square.file, rankChar square.rank]

private def pieceChar : PieceKind → Char
  | .king => 'K'
  | .queen => 'Q'
  | .rook => 'R'
  | .bishop => 'B'
  | .knight => 'N'
  | .pawn => 'P'

private def promotionChar : PromotionPiece → Char
  | .queen => 'Q'
  | .rook => 'R'
  | .bishop => 'B'
  | .knight => 'N'

/-- Captures include en passant, whose target square is empty. -/
def isCapture (position : Position) (move : Move) : Bool :=
  (position.board.pieceAt move.target).isSome ||
    isEnPassantCapture position move

private def sourceKindIs (position : Position) (source : Square)
    (kind : PieceKind) : Bool :=
  match position.board.pieceAt source with
  | some piece => piece.color == position.turn && piece.kind == kind
  | none => false

private def castleSide? (position : Position) (move : Move) : Option CastleSide :=
  if !sourceKindIs position move.source .king then none
  else
    match position.turn with
    | .white =>
        if move.source == Square.e1 && move.target == Square.g1 then some .kingSide
        else if move.source == Square.e1 && move.target == Square.c1 then some .queenSide
        else none
    | .black =>
        if move.source == Square.e8 && move.target == Square.g8 then some .kingSide
        else if move.source == Square.e8 && move.target == Square.c8 then some .queenSide
        else none

/-- Legal alternatives of the same piece kind that reach the same target.
Only the 64 possible sources are inspected; the 20,480-element raw move space
is intentionally not scanned. -/
private def competingSources (position : Position) (move : Move)
    (kind : PieceKind) : List Square :=
  Square.all.filter fun source =>
    source != move.source &&
      sourceKindIs position source kind &&
      isLegal position { source, target := move.target, promotion := move.promotion }

private def disambiguation (position : Position) (move : Move)
    (kind : PieceKind) : String :=
  let competitors := competingSources position move kind
  if competitors.isEmpty then
    ""
  else if competitors.all fun source => source.file != move.source.file then
    String.singleton (fileChar move.source.file)
  else if competitors.all fun source => source.rank != move.source.rank then
    String.singleton (rankChar move.source.rank)
  else
    squareText move.source

private def promotionTarget (piece : Piece) (target : Square) : Bool :=
  piece.kind == .pawn &&
    match piece.color with
    | .white => target.rank == 7
    | .black => target.rank == 0

private def promotionChoices (piece : Piece) (target : Square) :
    List (Option PromotionPiece) :=
  if promotionTarget piece target then
    [some .queen, some .rook, some .bishop, some .knight]
  else
    [none]

/-- An exhaustive, source-pruned legal-move existence test.  Occupied sources
and promotion-compatible choices reduce the usual raw enumeration from 20,480
moves to at most 4,096 ordinary source-target pairs (plus promotion choices). -/
def hasLegalMove (position : Position) : Bool :=
  Square.all.any fun source =>
    match position.board.pieceAt source with
    | none => false
    | some piece =>
        piece.color == position.turn &&
          Square.all.any fun target =>
            (promotionChoices piece target).any fun promotion =>
              isLegal position { source, target, promotion }

/-- The check state produced by a move.  This function is intended for legal
moves; it remains total so rendering can be executable. -/
def suffixAfter (position : Position) (move : Move) : CheckSuffix :=
  let next := applyUnchecked position move
  if inCheck next.board next.turn then
    if hasLegalMove next then .check else .mate
  else
    .none

private def suffixText : CheckSuffix → String
  | .none => ""
  | .check => "+"
  | .mate => "#"

private def renderLegal (position : Position) (move : Move) : String :=
  let suffix := suffixText (suffixAfter position move)
  match castleSide? position move with
  | some .kingSide => "O-O" ++ suffix
  | some .queenSide => "O-O-O" ++ suffix
  | none =>
      match position.board.pieceAt move.source with
      | none => "" -- unreachable for a legal move
      | some piece =>
          let capture := isCapture position move
          let leading := if piece.kind == .pawn then
              if capture then String.singleton (fileChar move.source.file) else ""
            else
              String.singleton (pieceChar piece.kind) ++
                disambiguation position move piece.kind
          let captureText := if capture then "x" else ""
          let promotionText := match move.promotion with
            | none => ""
            | some promotion => "=" ++ String.singleton (promotionChar promotion)
          leading ++ captureText ++ squareText move.target ++ promotionText ++ suffix

inductive RenderError where
  | illegalMove (move : Move)
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq

instance : ToString RenderError where
  toString
    | .illegalMove move => s!"cannot render illegal move as SAN: {repr move}"

/-- Canonical SAN for a legal move, including minimal legal-move
disambiguation and a semantically computed `+` or `#` suffix. -/
def render (position : Position) (move : Move) : Except RenderError String :=
  if isLegal position move then .ok (renderLegal position move)
  else .error (.illegalMove move)

private def matchesOrdinary (position : Position)
    (constraints : OrdinaryConstraints) (move : Move) : Bool :=
  sourceKindIs position move.source constraints.piece &&
    move.target == constraints.target &&
    move.promotion == constraints.promotion &&
    isCapture position move == constraints.capture &&
    (match constraints.sourceFile with
      | none => true
      | some file => move.source.file == file) &&
    (match constraints.sourceRank with
      | none => true
      | some rank => move.source.rank == rank)

/-- A legal move packaged with the legality fact used by replay proofs. -/
abbrev LegalMove (position : Position) := { move : Move // Legal position move }

private def checkedCandidate (position : Position) (move : Move) :
    Option (LegalMove position) :=
  if legal : Legal position move then some ⟨move, legal⟩ else none

private def castleMove (position : Position) (side : CastleSide) : Move :=
  match position.turn, side with
  | .white, .kingSide => ⟨Square.e1, Square.g1, none⟩
  | .white, .queenSide => ⟨Square.e1, Square.c1, none⟩
  | .black, .kingSide => ⟨Square.e8, Square.g8, none⟩
  | .black, .queenSide => ⟨Square.e8, Square.c8, none⟩

/-- Candidate-restricted legal resolution: a non-castling SAN move fixes its
target and promotion, so only its 64 possible source squares are considered. -/
private def candidateMoves (position : Position) :
    MoveConstraints → List (LegalMove position)
  | .castle side =>
      let move := castleMove position side
      if sourceKindIs position move.source .king then
        match checkedCandidate position move with
        | none => []
        | some move => [move]
      else
        []
  | .ordinary constraints =>
      Square.all.filterMap fun source =>
        let move : Move := {
          source := source
          target := constraints.target
          promotion := constraints.promotion
        }
        if matchesOrdinary position constraints move then
          checkedCandidate position move
        else
          none

inductive ResolveError where
  | syntax (error : ParseError)
  | noMatchingLegalMove
  | ambiguousLegalMoves (candidates : List Move)
  | suffixMismatch (expected actual : CheckSuffix)
  | nonCanonical (canonical : String)
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq

instance : ToString ResolveError where
  toString
    | .syntax error => toString error
    | .noMatchingLegalMove => "SAN does not match a legal move"
    | .ambiguousLegalMoves candidates =>
        s!"SAN is ambiguous between {candidates.length} legal moves"
    | .suffixMismatch expected actual =>
        s!"SAN check suffix is {repr expected}, but the move produces {repr actual}"
    | .nonCanonical canonical => s!"non-canonical SAN; canonical spelling is {canonical}"

private def values {position : Position} (moves : List (LegalMove position)) : List Move :=
  moves.map Subtype.val

/-- Parse and resolve a canonical SAN token to a uniquely determined legal
move.  Structural candidates are restricted before legality checks; canonical
rendering is the final authority for minimal disambiguation and suffixes. -/
def resolveLegal (position : Position) (token : String) :
    Except ResolveError (LegalMove position) := do
  let constraints ← match parse token with
    | .ok constraints => .ok constraints
    | .error error => .error (.syntax error)
  let candidates := candidateMoves position constraints.move
  let exact := candidates.filter fun move => renderLegal position move.val == token
  match exact with
  | [move] => .ok move
  | _ :: _ :: _ => .error (.ambiguousLegalMoves (values exact))
  | [] =>
      match candidates with
      | [] => .error .noMatchingLegalMove
      | [move] =>
          let actual := suffixAfter position move.val
          if constraints.suffix != actual then
            .error (.suffixMismatch constraints.suffix actual)
          else
            .error (.nonCanonical (renderLegal position move.val))
      | _ => .error (.ambiguousLegalMoves (values candidates))

/-- Resolve SAN while erasing the bundled proof for callers that only need the
raw move value. -/
def resolve (position : Position) (token : String) : Except ResolveError Move :=
  match resolveLegal position token with
  | .error error => .error error
  | .ok move => .ok move.val

/-- Successful SAN resolution can never introduce an unchecked game-graph
edge. -/
theorem legal_of_resolve_eq_ok {position : Position} {token : String} {move : Move}
    (success : resolve position token = .ok move) :
    Legal position move := by
  unfold resolve at success
  cases resolved : resolveLegal position token with
  | error error => simp [resolved] at success
  | ok legalMove =>
      simp [resolved] at success
      subst move
      exact legalMove.property

structure ReplayError where
  ply : Nat
  token : String
  reason : ResolveError
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq

instance : ToString ReplayError where
  toString error := s!"ply {error.ply}, SAN {error.token}: {error.reason}"

private def replayFrom : GameState → Nat → List String →
    Except ReplayError GameState
  | state, _, [] => .ok state
  | state, ply, token :: rest =>
      match resolveLegal state.current token with
      | .error reason => .error { ply, token, reason }
      | .ok move => replayFrom (state.afterMove move.val) (ply + 1) rest

/-- Replay canonical SAN tokens, retaining complete `GameState` history and
reporting the first failing one-based ply. -/
def replay (state : GameState) (tokens : List String) : Except ReplayError GameState :=
  replayFrom state 1 tokens

/-- Split a whitespace-delimited SAN line and replay it.  Move numbers,
comments, variations, and result markers are PGN movetext rather than SAN and
are intentionally outside this helper. -/
def replayLine (state : GameState) (line : String) : Except ReplayError GameState :=
  replay state ((line.splitToList Char.isWhitespace).filter fun token => !token.isEmpty)

private theorem reachable_of_replayFrom_eq_ok {state final : GameState}
    {ply : Nat} {tokens : List String}
    (success : replayFrom state ply tokens = .ok final) :
    Position.Reachable state.current final.current := by
  induction tokens generalizing state ply with
  | nil =>
      simp [replayFrom] at success
      subst final
      exact .refl _
  | cons token rest ih =>
      cases resolved : resolveLegal state.current token with
      | error error => simp [replayFrom, resolved] at success
      | ok move =>
          simp only [replayFrom, resolved] at success
          exact .step ⟨move.val, move.property, rfl⟩ (ih success)

/-- Every successful SAN replay is a finite path of orthodox legal moves. -/
theorem reachable_of_replay_eq_ok {state final : GameState} {tokens : List String}
    (success : replay state tokens = .ok final) :
    Position.Reachable state.current final.current :=
  reachable_of_replayFrom_eq_ok success

/-- The same reachability guarantee for the whitespace-delimited helper. -/
theorem reachable_of_replayLine_eq_ok {state final : GameState} {line : String}
    (success : replayLine state line = .ok final) :
    Position.Reachable state.current final.current :=
  reachable_of_replay_eq_ok success

/-! A coordinate-shaped `e1g1` move is not castling unless the source is the
side-to-move king.  Arbitrary analysis positions make this guard observable. -/

private def nonKingCastleShapePosition : Position where
  board := (((Board.empty.set Square.a1 (some ⟨.white, .king⟩)).set
    Square.e1 (some ⟨.white, .rook⟩)).set Square.e8 (some ⟨.black, .king⟩))
  turn := .white
  castlingRights := .none
  enPassantTarget := none
  halfmoveClock := 0
  fullmoveNumber := 1

private def nonKingE1G1 : Move := ⟨Square.e1, Square.g1, none⟩

example : Legal nonKingCastleShapePosition nonKingE1G1 := by native_decide

example :
    (match render nonKingCastleShapePosition nonKingE1G1 with
    | .ok text => (text == "Rg1" : Bool)
    | .error _ => false) = true := by
  native_decide

example :
    (match resolve nonKingCastleShapePosition "O-O" with
    | .error .noMatchingLegalMove => true
    | _ => false) = true := by
  native_decide

end Chess.SAN
