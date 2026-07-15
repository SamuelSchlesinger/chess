import Chess.Attack
import Chess.Move
import Chess.Phase

namespace Chess

namespace Board

def occupiedBy (board : Board) (square : Square) (color : Color) : Bool :=
  match board.pieceAt square with
  | some piece => piece.color == color
  | none => false

def occupiedByOpponent (board : Board) (square : Square) (color : Color) : Bool :=
  match board.pieceAt square with
  | some piece => piece.color == color.other
  | none => false

/-- A normal destination cannot contain a friendly piece or either king.
Kings are checkmated, never captured. -/
def capturableBy (board : Board) (square : Square) (color : Color) : Bool :=
  match board.pieceAt square with
  | none => true
  | some piece => piece.color != color && piece.kind != .king

/-- Find the king of a color, if present. -/
def kingSquare? (board : Board) (color : Color) : Option Square :=
  Square.all.find? fun square => board.pieceAt square == some ⟨color, .king⟩

end Board

/-- Missing kings make an arbitrary analysis position unsafe rather than
silently legal. On reachable game states both kings will be proved present. -/
def inCheck (board : Board) (color : Color) : Bool :=
  match board.kingSquare? color with
  | some king => attackedBy board color.other king
  | none => true

private def pawnDirection : Color → Direction
  | .white => Direction.north
  | .black => Direction.south

private def pawnHomeRank : Color → Coordinate
  | .white => 1
  | .black => 6

private def promotionRank : Color → Coordinate
  | .white => 7
  | .black => 0

private def promotionMatches (move : Move) (color : Color) : Bool :=
  (move.target.rank == promotionRank color) == move.promotion.isSome

private def pawnSingleAdvance (position : Position) (move : Move) (color : Color) : Bool :=
  move.source.offset (pawnDirection color) == some move.target &&
    (position.board.pieceAt move.target).isNone

private def pawnDoubleAdvance (position : Position) (move : Move) (color : Color) : Bool :=
  if move.source.rank != pawnHomeRank color then
    false
  else
    match move.source.offset (pawnDirection color) with
    | none => false
    | some middle =>
        middle.offset (pawnDirection color) == some move.target &&
          (position.board.pieceAt middle).isNone &&
          (position.board.pieceAt move.target).isNone

private def pawnCapture (position : Position) (move : Move) (color : Color) : Bool :=
  let diagonals := match color with
    | .white => [Direction.northWest, Direction.northEast]
    | .black => [Direction.southWest, Direction.southEast]
  let capturedEnPassantSquare : Square := ⟨move.target.file, move.source.rank⟩
  let validEnPassant := position.enPassantTarget == some move.target &&
    position.board.pieceAt capturedEnPassantSquare == some ⟨color.other, .pawn⟩
  (diagonals.filterMap move.source.offset).contains move.target &&
    (position.board.occupiedByOpponent move.target color ||
      validEnPassant)

private def ordinaryPseudoLegal (position : Position) (move : Move) (piece : Piece) : Bool :=
  position.board.capturableBy move.target piece.color &&
    match piece.kind with
    | .pawn =>
        promotionMatches move piece.color &&
          (pawnSingleAdvance position move piece.color ||
            pawnDoubleAdvance position move piece.color ||
            pawnCapture position move piece.color)
    | _ => move.promotion.isNone &&
        (attacksFrom position.board move.source piece).contains move.target

private structure CastleData where
  side : CastleSide
  kingSource : Square
  kingTransit : Square
  kingTarget : Square
  rookSource : Square
  rookTarget : Square
  mustBeEmpty : List Square

private def castleData : Color → CastleSide → CastleData
  | .white, .kingSide =>
      ⟨.kingSide, .e1, .f1, .g1, .h1, .f1, [.f1, .g1]⟩
  | .white, .queenSide =>
      ⟨.queenSide, .e1, .d1, .c1, .a1, .d1, [.b1, .c1, .d1]⟩
  | .black, .kingSide =>
      ⟨.kingSide, .e8, .f8, .g8, .h8, .f8, [.f8, .g8]⟩
  | .black, .queenSide =>
      ⟨.queenSide, .e8, .d8, .c8, .a8, .d8, [.b8, .c8, .d8]⟩

private def castleSide? (color : Color) (move : Move) : Option CastleSide :=
  if move.source != (castleData color .kingSide).kingSource then none
  else if move.target == (castleData color .kingSide).kingTarget then some .kingSide
  else if move.target == (castleData color .queenSide).kingTarget then some .queenSide
  else none

private def castlePseudoLegal (position : Position) (move : Move) (color : Color) : Bool :=
  match castleSide? color move with
  | none => false
  | some side =>
      let data := castleData color side
      move.promotion.isNone &&
      position.castlingRights.has color side &&
      position.board.pieceAt data.rookSource == some ⟨color, .rook⟩ &&
      (data.mustBeEmpty.all fun square => (position.board.pieceAt square).isNone) &&
      !attackedBy position.board color.other data.kingSource &&
      !attackedBy position.board color.other data.kingTransit &&
      !attackedBy position.board color.other data.kingTarget

/-- All piece-movement and special-move conditions except the final test that
the moving player's king remains safe. -/
def isPseudoLegal (position : Position) (move : Move) : Bool :=
  match position.board.pieceAt move.source with
  | none => false
  | some piece =>
      piece.color == position.turn &&
        (ordinaryPseudoLegal position move piece ||
          (piece.kind == .king && castlePseudoLegal position move piece.color))

private def revokeRookSquare (rights : CastlingRights) (square : Square) : CastlingRights :=
  if square == Square.a1 then rights.revoke .white .queenSide
  else if square == Square.h1 then rights.revoke .white .kingSide
  else if square == Square.a8 then rights.revoke .black .queenSide
  else if square == Square.h8 then rights.revoke .black .kingSide
  else rights

private def rightsAfter (position : Position) (move : Move) (piece : Piece) : CastlingRights :=
  let afterMover := match piece.kind with
    | .king => position.castlingRights.revokeKing piece.color
    | .rook => revokeRookSquare position.castlingRights move.source
    | _ => position.castlingRights
  revokeRookSquare afterMover move.target

private def promotedPiece (piece : Piece) (move : Move) : Piece :=
  match move.promotion with
  | some promotion => ⟨piece.color, promotion.pieceKind⟩
  | none => piece

private def boardAfterOrdinary (position : Position) (move : Move) (piece : Piece) : Board :=
  let moved := (position.board.clear move.source).set move.target (some (promotedPiece piece move))
  if piece.kind == .pawn && (position.board.pieceAt move.target).isNone &&
      move.source.file != move.target.file then
    -- A pseudo-legal diagonal pawn move to an empty square is en passant.
    moved.clear ⟨move.target.file, move.source.rank⟩
  else
    moved

private def boardAfter (position : Position) (move : Move) (piece : Piece) : Board :=
  match piece.kind, castleSide? piece.color move with
  | .king, some side =>
      let data := castleData piece.color side
      let kingMoved := (position.board.clear data.kingSource).set data.kingTarget (some piece)
      (kingMoved.clear data.rookSource).set data.rookTarget (some ⟨piece.color, .rook⟩)
  | _, _ => boardAfterOrdinary position move piece

private def nextEnPassantTarget (move : Move) (piece : Piece) : Option Square :=
  if piece.kind != .pawn || move.source.file != move.target.file then none
  else
    let rankDistance := if move.source.rank.val ≤ move.target.rank.val then
      move.target.rank.val - move.source.rank.val
    else
      move.source.rank.val - move.target.rank.val
    if rankDistance != 2 then none
    else move.source.offset (pawnDirection piece.color)

/-- Apply a move whose pseudo-legality is established. This function remains
total so that legal-move checking can compute the successor efficiently. -/
def applyUnchecked (position : Position) (move : Move) : Position :=
  match position.board.pieceAt move.source with
  | none => position
  | some piece =>
      let capture := !(position.board.pieceAt move.target).isNone ||
        (piece.kind == .pawn && move.source.file != move.target.file)
      {
        board := boardAfter position move piece
        turn := position.turn.other
        castlingRights := rightsAfter position move piece
        enPassantTarget := nextEnPassantTarget move piece
        halfmoveClock := if piece.kind == .pawn || capture then 0 else position.halfmoveClock + 1
        fullmoveNumber := if position.turn == .black then position.fullmoveNumber + 1
          else position.fullmoveNumber
      }

/-- Full orthodox move legality: pseudo-legality plus preservation of the
moving player's king. -/
def isLegal (position : Position) (move : Move) : Bool :=
  isPseudoLegal position move && !inCheck (applyUnchecked position move).board position.turn

def PseudoLegal (position : Position) (move : Move) : Prop := isPseudoLegal position move
def Legal (position : Position) (move : Move) : Prop := isLegal position move

/-- A raw move is an en-passant capture in a position. Legality remains a
separate condition because a pinned capturing pawn may not actually move. -/
def isEnPassantCapture (position : Position) (move : Move) : Bool :=
  position.enPassantTarget == some move.target &&
    (position.board.pieceAt move.target).isNone &&
    move.source.file != move.target.file &&
    match position.board.pieceAt move.source with
    | some piece => piece.color == position.turn && piece.kind == .pawn
    | none => false

namespace Move

/-- A complete, deliberately simple enumeration of raw orthodox move choices.
Optimized generators can later be proved equivalent to filtering this list. -/
def all : List Move :=
  Square.all.flatMap fun source =>
    Square.all.flatMap fun target =>
      [none, some .queen, some .rook, some .bishop, some .knight].map fun promotion =>
        { source, target, promotion }

end Move

/-- All legal moves, specified by filtering the complete raw move space. -/
def legalMoves (position : Position) : List Move := Move.all.filter (isLegal position)

/-- The standard move-generation validation function: the number of leaf nodes
at an exact legal-move depth. -/
def perft : Nat → Position → Nat
  | 0, _ => 1
  | depth + 1, position =>
      (legalMoves position).foldl
        (fun total move => total + perft depth (applyUnchecked position move)) 0

instance (position : Position) (move : Move) : Decidable (PseudoLegal position move) :=
  inferInstanceAs (Decidable (isPseudoLegal position move = true))

instance (position : Position) (move : Move) : Decidable (Legal position move) :=
  inferInstanceAs (Decidable (isLegal position move = true))

theorem legal_iff (position : Position) (move : Move) :
    Legal position move ↔
      PseudoLegal position move ∧ inCheck (applyUnchecked position move).board position.turn = false := by
  simp [Legal, PseudoLegal, isLegal]

/-- Pseudo-legality depends on the rule-relevant position fields, not on either
move clock. -/
private theorem pseudoLegal_iff_of_rule_fields_eq {left right : Position} (move : Move)
    (boardEq : left.board = right.board)
    (turnEq : left.turn = right.turn)
    (rightsEq : left.castlingRights = right.castlingRights)
    (enPassantEq : left.enPassantTarget = right.enPassantTarget) :
    PseudoLegal left move ↔ PseudoLegal right move := by
  cases left with
  | mk leftBoard leftTurn leftRights leftEnPassant leftHalfmove leftFullmove =>
      cases right with
      | mk rightBoard rightTurn rightRights rightEnPassant rightHalfmove rightFullmove =>
          simp only at boardEq turnEq rightsEq enPassantEq
          subst rightBoard
          subst rightTurn
          subst rightRights
          subst rightEnPassant
          simp [PseudoLegal, isPseudoLegal, ordinaryPseudoLegal,
            pawnSingleAdvance, pawnDoubleAdvance, pawnCapture,
            castlePseudoLegal]

/-- The board produced by unchecked move application depends only on the
starting board and the raw move. -/
private theorem applyUnchecked_board_eq_of_board_eq {left right : Position} (move : Move)
    (boardEq : left.board = right.board) :
    (applyUnchecked left move).board = (applyUnchecked right move).board := by
  cases left with
  | mk leftBoard leftTurn leftRights leftEnPassant leftHalfmove leftFullmove =>
      cases right with
      | mk rightBoard rightTurn rightRights rightEnPassant rightHalfmove rightFullmove =>
          simp only at boardEq
          subst rightBoard
          cases sourcePiece : leftBoard.pieceAt move.source <;>
            simp [applyUnchecked, sourcePiece, boardAfter, boardAfterOrdinary]

/-- From an occupied source, unchecked move application preserves equality of
all rule-relevant fields whenever the inputs agree on board, turn, and
castling rights. The raw input en-passant fields and clocks are irrelevant
because move application replaces them. -/
theorem applyUnchecked_rule_fields_eq_of_occupied
    {left right : Position} (move : Move) (piece : Piece)
    (boardEq : left.board = right.board)
    (turnEq : left.turn = right.turn)
    (rightsEq : left.castlingRights = right.castlingRights)
    (occupied : left.board.pieceAt move.source = some piece) :
    (applyUnchecked left move).board = (applyUnchecked right move).board ∧
    (applyUnchecked left move).turn = (applyUnchecked right move).turn ∧
    (applyUnchecked left move).castlingRights =
      (applyUnchecked right move).castlingRights ∧
    (applyUnchecked left move).enPassantTarget =
      (applyUnchecked right move).enPassantTarget := by
  cases left with
  | mk leftBoard leftTurn leftRights leftEnPassant leftHalfmove leftFullmove =>
      cases right with
      | mk rightBoard rightTurn rightRights rightEnPassant rightHalfmove rightFullmove =>
          simp only at boardEq turnEq rightsEq occupied
          subst rightBoard
          subst rightTurn
          subst rightRights
          simp [applyUnchecked, occupied, rightsAfter, revokeRookSquare,
            boardAfter, boardAfterOrdinary]

/-- Move legality depends on the rule-relevant position fields, not on either
move clock. -/
theorem legal_iff_of_rule_fields_eq {left right : Position} (move : Move)
    (boardEq : left.board = right.board)
    (turnEq : left.turn = right.turn)
    (rightsEq : left.castlingRights = right.castlingRights)
    (enPassantEq : left.enPassantTarget = right.enPassantTarget) :
    Legal left move ↔ Legal right move := by
  rw [legal_iff, legal_iff]
  rw [pseudoLegal_iff_of_rule_fields_eq move boardEq turnEq rightsEq enPassantEq]
  rw [applyUnchecked_board_eq_of_board_eq move boardEq, turnEq]

private theorem pawnCapture_mono_of_enPassantTarget
    {left right : Position} (move : Move) (color : Color)
    (boardEq : left.board = right.board)
    (enPassantMono :
      left.enPassantTarget == some move.target →
        right.enPassantTarget == some move.target) :
    pawnCapture left move color → pawnCapture right move color := by
  cases left with
  | mk leftBoard leftTurn leftRights leftEnPassant leftHalfmove leftFullmove =>
      cases right with
      | mk rightBoard rightTurn rightRights rightEnPassant rightHalfmove rightFullmove =>
          simp only at boardEq
          subst rightBoard
          simp only [pawnCapture, Bool.and_eq_true, Bool.or_eq_true] at enPassantMono ⊢
          rintro ⟨diagonal, occupied | ⟨enPassant, capturedPawn⟩⟩
          · exact ⟨diagonal, Or.inl occupied⟩
          · exact ⟨diagonal, Or.inr ⟨enPassantMono enPassant, capturedPawn⟩⟩

private theorem coordinate_offset_unit_ne_self (coordinate : Coordinate) (delta : Int)
    (unit : delta = 1 ∨ delta = -1)
    (step : coordinate.offset delta = some coordinate) : False := by
  rcases unit with rfl | rfl <;> simp [Coordinate.offset] at step
  · rcases step with ⟨_, _, equality⟩
    have valueEq := congrArg Fin.val equality
    simp at valueEq
  · rcases step with ⟨lower, _, equality⟩
    have valueEq := congrArg Fin.val equality
    simp at valueEq
    omega

private theorem source_file_ne_of_offset_unit {source target : Square}
    {direction : Direction}
    (unit : direction.fileDelta = 1 ∨ direction.fileDelta = -1)
    (step : source.offset direction = some target) :
    source.file ≠ target.file := by
  intro sameFile
  cases fileStep : source.file.offset direction.fileDelta with
  | none => simp [Square.offset, fileStep] at step
  | some nextFile =>
      cases rankStep : source.rank.offset direction.rankDelta with
      | none => simp [Square.offset, fileStep, rankStep] at step
      | some nextRank =>
          simp [Square.offset, fileStep, rankStep] at step
          have nextFileEq : nextFile = target.file := congrArg Square.file step
          subst nextFile
          rw [← sameFile] at fileStep
          exact coordinate_offset_unit_ne_self source.file direction.fileDelta unit fileStep

private theorem source_file_ne_of_pawnCapture {position : Position}
    {move : Move} {color : Color} (capture : pawnCapture position move color) :
    move.source.file ≠ move.target.file := by
  have parts :
      (let diagonals := match color with
        | .white => [Direction.northWest, Direction.northEast]
        | .black => [Direction.southWest, Direction.southEast]
       (diagonals.filterMap move.source.offset).contains move.target) ∧
      (position.board.occupiedByOpponent move.target color ||
        (position.enPassantTarget == some move.target &&
          position.board.pieceAt ⟨move.target.file, move.source.rank⟩ ==
            some ⟨color.other, .pawn⟩)) := by
    simpa only [pawnCapture, Bool.and_eq_true] using capture
  have diagonal := parts.1
  cases color with
  | white =>
      simp at diagonal
      rcases diagonal with northWest | northEast
      · exact source_file_ne_of_offset_unit (Or.inr rfl) northWest
      · exact source_file_ne_of_offset_unit (Or.inl rfl) northEast
  | black =>
      simp at diagonal
      rcases diagonal with southWest | southEast
      · exact source_file_ne_of_offset_unit (Or.inr rfl) southWest
      · exact source_file_ne_of_offset_unit (Or.inl rfl) southEast

private theorem ordinaryPseudoLegal_mono_of_enPassantTarget
    {left right : Position} (move : Move) (piece : Piece)
    (boardEq : left.board = right.board)
    (enPassantMono :
      left.enPassantTarget == some move.target →
        right.enPassantTarget == some move.target) :
    ordinaryPseudoLegal left move piece → ordinaryPseudoLegal right move piece := by
  have captureMono := pawnCapture_mono_of_enPassantTarget
    move piece.color boardEq enPassantMono
  cases left with
  | mk leftBoard leftTurn leftRights leftEnPassant leftHalfmove leftFullmove =>
      cases right with
      | mk rightBoard rightTurn rightRights rightEnPassant rightHalfmove rightFullmove =>
          simp only at boardEq
          subst rightBoard
          cases piece with
          | mk color kind =>
              cases kind with
              | pawn =>
                  have singleEq :
                      pawnSingleAdvance
                        { board := leftBoard, turn := leftTurn,
                          castlingRights := leftRights, enPassantTarget := leftEnPassant,
                          halfmoveClock := leftHalfmove, fullmoveNumber := leftFullmove }
                        move color =
                      pawnSingleAdvance
                        { board := leftBoard, turn := rightTurn,
                          castlingRights := rightRights, enPassantTarget := rightEnPassant,
                          halfmoveClock := rightHalfmove, fullmoveNumber := rightFullmove }
                        move color := by
                    simp [pawnSingleAdvance]
                  have doubleEq :
                      pawnDoubleAdvance
                        { board := leftBoard, turn := leftTurn,
                          castlingRights := leftRights, enPassantTarget := leftEnPassant,
                          halfmoveClock := leftHalfmove, fullmoveNumber := leftFullmove }
                        move color =
                      pawnDoubleAdvance
                        { board := leftBoard, turn := rightTurn,
                          castlingRights := rightRights, enPassantTarget := rightEnPassant,
                          halfmoveClock := rightHalfmove, fullmoveNumber := rightFullmove }
                        move color := by
                    simp [pawnDoubleAdvance]
                  simp only [ordinaryPseudoLegal, Bool.and_eq_true, Bool.or_eq_true,
                    singleEq, doubleEq]
                  rintro ⟨capturable, promotion, (single | double) | capture⟩
                  · exact ⟨capturable, promotion, Or.inl (Or.inl single)⟩
                  · exact ⟨capturable, promotion, Or.inl (Or.inr double)⟩
                  · exact ⟨capturable, promotion, Or.inr (captureMono capture)⟩
              | king =>
                  intro ordinary
                  simpa [ordinaryPseudoLegal] using ordinary
              | queen =>
                  intro ordinary
                  simpa [ordinaryPseudoLegal] using ordinary
              | rook =>
                  intro ordinary
                  simpa [ordinaryPseudoLegal] using ordinary
              | bishop =>
                  intro ordinary
                  simpa [ordinaryPseudoLegal] using ordinary
              | knight =>
                  intro ordinary
                  simpa [ordinaryPseudoLegal] using ordinary

/-- Pseudo-legality is monotone in the availability of the one raw
en-passant target inspected by a move. -/
private theorem pseudoLegal_mono_of_rule_fields {left right : Position} (move : Move)
    (boardEq : left.board = right.board)
    (turnEq : left.turn = right.turn)
    (rightsEq : left.castlingRights = right.castlingRights)
    (enPassantMono :
      left.enPassantTarget == some move.target →
        right.enPassantTarget == some move.target) :
    PseudoLegal left move → PseudoLegal right move := by
  have ordinaryMono (piece : Piece) := ordinaryPseudoLegal_mono_of_enPassantTarget
    move piece boardEq enPassantMono
  cases left with
  | mk leftBoard leftTurn leftRights leftEnPassant leftHalfmove leftFullmove =>
      cases right with
      | mk rightBoard rightTurn rightRights rightEnPassant rightHalfmove rightFullmove =>
          simp only at boardEq turnEq rightsEq
          subst rightBoard
          subst rightTurn
          subst rightRights
          cases sourcePiece : leftBoard.pieceAt move.source with
          | none => simp [PseudoLegal, isPseudoLegal, sourcePiece]
          | some piece =>
              have castleEq :
                  castlePseudoLegal
                    { board := leftBoard, turn := leftTurn,
                      castlingRights := leftRights, enPassantTarget := leftEnPassant,
                      halfmoveClock := leftHalfmove, fullmoveNumber := leftFullmove }
                    move piece.color =
                  castlePseudoLegal
                    { board := leftBoard, turn := leftTurn,
                      castlingRights := leftRights, enPassantTarget := rightEnPassant,
                      halfmoveClock := rightHalfmove, fullmoveNumber := rightFullmove }
                    move piece.color := by
                simp [castlePseudoLegal]
              simp only [PseudoLegal, isPseudoLegal, sourcePiece, Bool.and_eq_true,
                Bool.or_eq_true, castleEq]
              rintro ⟨colorEq, ordinary | castle⟩
              · exact ⟨colorEq, Or.inl (ordinaryMono piece ordinary)⟩
              · exact ⟨colorEq, Or.inr castle⟩

/-- Full legality is monotone under the same rule-field comparison: enlarging
raw en-passant availability cannot invalidate an already legal move. -/
private theorem legal_mono_of_rule_fields {left right : Position} (move : Move)
    (boardEq : left.board = right.board)
    (turnEq : left.turn = right.turn)
    (rightsEq : left.castlingRights = right.castlingRights)
    (enPassantMono :
      left.enPassantTarget == some move.target →
        right.enPassantTarget == some move.target) :
    Legal left move → Legal right move := by
  intro legalLeft
  have leftParts := (legal_iff left move).mp legalLeft
  refine (legal_iff right move).mpr ⟨?_, ?_⟩
  · exact pseudoLegal_mono_of_rule_fields move boardEq turnEq rightsEq
      enPassantMono leftParts.1
  · rw [← applyUnchecked_board_eq_of_board_eq move boardEq, ← turnEq]
    exact leftParts.2

private theorem pieceAt_eq_none_of_capturable_of_not_occupiedByOpponent
    (board : Board) (square : Square) (color : Color)
    (capturable : board.capturableBy square color)
    (notOccupied : ¬board.occupiedByOpponent square color) :
    board.pieceAt square = none := by
  cases target : board.pieceAt square with
  | none => rfl
  | some piece =>
      cases piece with
      | mk targetColor targetKind =>
          exfalso
          cases color <;> cases targetColor <;>
            simp [Board.capturableBy, Board.occupiedByOpponent, Color.other, target] at capturable notOccupied

/-- If changing only the raw en-passant field destroys a legal move, that move
was precisely an en-passant capture in the first position. -/
theorem isEnPassantCapture_of_legal_of_not_legal
    {left right : Position} (move : Move)
    (boardEq : left.board = right.board)
    (turnEq : left.turn = right.turn)
    (rightsEq : left.castlingRights = right.castlingRights)
    (legalLeft : Legal left move)
    (notLegalRight : ¬Legal right move) :
    isEnPassantCapture left move := by
  have leftParts := (legal_iff left move).mp legalLeft
  have nextBoardEq := applyUnchecked_board_eq_of_board_eq move boardEq
  have rightSafe :
      inCheck (applyUnchecked right move).board right.turn = false := by
    rw [← nextBoardEq, ← turnEq]
    exact leftParts.2
  have notPseudoRight : ¬PseudoLegal right move := by
    intro pseudoRight
    exact notLegalRight ((legal_iff right move).mpr ⟨pseudoRight, rightSafe⟩)
  cases left with
  | mk leftBoard leftTurn leftRights leftEnPassant leftHalfmove leftFullmove =>
      cases right with
      | mk rightBoard rightTurn rightRights rightEnPassant rightHalfmove rightFullmove =>
          simp only at boardEq turnEq rightsEq
          subst rightBoard
          subst rightTurn
          subst rightRights
          cases sourcePiece : leftBoard.pieceAt move.source with
          | none =>
              simp [PseudoLegal, isPseudoLegal, sourcePiece] at leftParts
          | some piece =>
              cases piece with
              | mk color kind =>
                  cases kind with
                  | king =>
                      exfalso
                      apply notPseudoRight
                      simpa [PseudoLegal, isPseudoLegal, sourcePiece,
                        ordinaryPseudoLegal, castlePseudoLegal] using leftParts.1
                  | queen =>
                      exfalso
                      apply notPseudoRight
                      simpa [PseudoLegal, isPseudoLegal, sourcePiece,
                        ordinaryPseudoLegal, castlePseudoLegal] using leftParts.1
                  | rook =>
                      exfalso
                      apply notPseudoRight
                      simpa [PseudoLegal, isPseudoLegal, sourcePiece,
                        ordinaryPseudoLegal, castlePseudoLegal] using leftParts.1
                  | bishop =>
                      exfalso
                      apply notPseudoRight
                      simpa [PseudoLegal, isPseudoLegal, sourcePiece,
                        ordinaryPseudoLegal, castlePseudoLegal] using leftParts.1
                  | knight =>
                      exfalso
                      apply notPseudoRight
                      simpa [PseudoLegal, isPseudoLegal, sourcePiece,
                        ordinaryPseudoLegal, castlePseudoLegal] using leftParts.1
                  | pawn =>
                      have pseudoLeft := leftParts.1
                      simp only [PseudoLegal, isPseudoLegal, sourcePiece,
                        Bool.and_eq_true, Bool.or_eq_true] at pseudoLeft
                      rcases pseudoLeft with ⟨colorEq, ordinary | impossibleCastle⟩
                      · simp only [ordinaryPseudoLegal, Bool.and_eq_true,
                          Bool.or_eq_true] at ordinary
                        rcases ordinary with
                          ⟨capturable, promotion, (single | double) | capture⟩
                        · exfalso
                          apply notPseudoRight
                          simp only [PseudoLegal, isPseudoLegal, sourcePiece,
                            Bool.and_eq_true, Bool.or_eq_true]
                          refine ⟨colorEq, Or.inl ?_⟩
                          simp only [ordinaryPseudoLegal, Bool.and_eq_true,
                            Bool.or_eq_true]
                          exact ⟨capturable, promotion, Or.inl (Or.inl single)⟩
                        · exfalso
                          apply notPseudoRight
                          simp only [PseudoLegal, isPseudoLegal, sourcePiece,
                            Bool.and_eq_true, Bool.or_eq_true]
                          refine ⟨colorEq, Or.inl ?_⟩
                          simp only [ordinaryPseudoLegal, Bool.and_eq_true,
                            Bool.or_eq_true]
                          exact ⟨capturable, promotion, Or.inl (Or.inr double)⟩
                        · have notCaptureRight :
                              ¬pawnCapture
                                { board := leftBoard, turn := leftTurn,
                                  castlingRights := leftRights,
                                  enPassantTarget := rightEnPassant,
                                  halfmoveClock := rightHalfmove,
                                  fullmoveNumber := rightFullmove }
                                move color := by
                            intro captureRight
                            apply notPseudoRight
                            simp only [PseudoLegal, isPseudoLegal, sourcePiece,
                              Bool.and_eq_true, Bool.or_eq_true]
                            refine ⟨colorEq, Or.inl ?_⟩
                            simp only [ordinaryPseudoLegal, Bool.and_eq_true,
                              Bool.or_eq_true]
                            exact ⟨capturable, promotion, Or.inr captureRight⟩
                          have captureParts :
                              (let diagonals := match color with
                                | .white => [Direction.northWest, Direction.northEast]
                                | .black => [Direction.southWest, Direction.southEast]
                               (diagonals.filterMap move.source.offset).contains move.target = true) ∧
                              (leftBoard.occupiedByOpponent move.target color = true ∨
                                (leftEnPassant == some move.target) = true ∧
                                  (leftBoard.pieceAt
                                      ⟨move.target.file, move.source.rank⟩ ==
                                    some ⟨color.other, .pawn⟩) = true) := by
                            simpa only [pawnCapture, Bool.and_eq_true,
                              Bool.or_eq_true] using capture
                          rcases captureParts with ⟨diagonal, occupied | enPassant⟩
                          · exfalso
                            apply notCaptureRight
                            simp only [pawnCapture, Bool.and_eq_true,
                              Bool.or_eq_true]
                            exact ⟨diagonal, Or.inl occupied⟩
                          · have notOccupied :
                                ¬leftBoard.occupiedByOpponent move.target color := by
                              intro occupied
                              apply notCaptureRight
                              simp only [pawnCapture, Bool.and_eq_true,
                                Bool.or_eq_true]
                              exact ⟨diagonal, Or.inl occupied⟩
                            have targetEmpty : leftBoard.pieceAt move.target = none := by
                              exact pieceAt_eq_none_of_capturable_of_not_occupiedByOpponent
                                leftBoard move.target color capturable notOccupied
                            have fileNe := source_file_ne_of_pawnCapture capture
                            simp [isEnPassantCapture, enPassant.1, targetEmpty,
                              fileNe, sourcePiece, colorEq]
                      · have : False := by simpa using impossibleCastle.1
                        exact this.elim

private theorem pawnMove_phase_decreases (position : Position) (move : Move) (piece : Piece)
    (isPawn : piece.kind = .pawn)
    (ordinary : ordinaryPseudoLegal position move piece) :
    piecePhasePotential move.target (some (promotedPiece piece move)) <
      piecePhasePotential move.source (some piece) := by
  rcases move with ⟨source, target, promotion⟩
  rcases piece with ⟨color, kind⟩
  simp only at isPawn
  subst kind
  cases color with
  | white =>
      simp [ordinaryPseudoLegal, pawnSingleAdvance, pawnDoubleAdvance, pawnCapture,
        pawnDirection, promotionMatches] at ordinary
      have rankProgress :
          target.rank.val = source.rank.val + 1 ∨
          target.rank.val = source.rank.val + 2 := by
        rcases ordinary.2.2 with (single | double) | capture
        · exact Or.inl (Square.rank_succ_of_offset rfl single.1)
        · rcases double with ⟨_, double⟩
          cases first : source.offset Direction.north with
          | none => simp [first] at double
          | some middle =>
              simp [first] at double
              have firstRank := Square.rank_succ_of_offset rfl first
              have secondRank := Square.rank_succ_of_offset rfl double.1.1
              omega
        · rcases capture.1 with northWest | northEast
          · exact Or.inl (Square.rank_succ_of_offset rfl northWest)
          · exact Or.inl (Square.rank_succ_of_offset rfl northEast)
      cases promotion with
      | none =>
          simp [promotedPiece, piecePhasePotential, pawnTravel]
          omega
      | some promotion =>
          cases promotion <;> simp [promotedPiece, piecePhasePotential,
            PromotionPiece.pieceKind] <;> omega

  | black =>
      simp [ordinaryPseudoLegal, pawnSingleAdvance, pawnDoubleAdvance, pawnCapture,
        pawnDirection, promotionMatches] at ordinary
      have rankProgress :
          target.rank.val + 1 = source.rank.val ∨
          target.rank.val + 2 = source.rank.val := by
        rcases ordinary.2.2 with (single | double) | capture
        · exact Or.inl (Square.rank_pred_of_offset rfl single.1)
        · rcases double with ⟨_, double⟩
          cases first : source.offset Direction.south with
          | none => simp [first] at double
          | some middle =>
              simp [first] at double
              have firstRank := Square.rank_pred_of_offset rfl first
              have secondRank := Square.rank_pred_of_offset rfl double.1.1
              omega
        · rcases capture.1 with southWest | southEast
          · exact Or.inl (Square.rank_pred_of_offset rfl southWest)
          · exact Or.inl (Square.rank_pred_of_offset rfl southEast)
      cases promotion with
      | none =>
          simp [promotedPiece, piecePhasePotential, pawnTravel]
          omega
      | some promotion =>
          cases promotion <;> simp [promotedPiece, piecePhasePotential,
            PromotionPiece.pieceKind] <;> omega

private theorem ordinary_source_ne_target (position : Position) (move : Move) (piece : Piece)
    (occupied : position.board.pieceAt move.source = some piece)
    (ordinary : ordinaryPseudoLegal position move piece) :
    move.source ≠ move.target := by
  unfold ordinaryPseudoLegal at ordinary
  simp at ordinary
  have capturable := ordinary.1
  intro same
  rw [← same] at capturable
  simp [Board.capturableBy, occupied] at capturable

private theorem promotedPiece_phase_le (position : Position) (move : Move) (piece : Piece)
    (ordinary : ordinaryPseudoLegal position move piece) :
    piecePhasePotential move.target (some (promotedPiece piece move)) ≤
      piecePhasePotential move.source (some piece) := by
  have ordinaryCopy := ordinary
  unfold ordinaryPseudoLegal at ordinary
  simp at ordinary
  have movement := ordinary.2
  rcases piece with ⟨color, kind⟩
  cases kind with
  | pawn =>
      exact Nat.le_of_lt (pawnMove_phase_decreases position move ⟨color, .pawn⟩ rfl ordinaryCopy)
  | king | queen | rook | bishop | knight =>
      have noPromotion : move.promotion = none := by
        cases promotionEq : move.promotion <;> simp [promotionEq] at movement ⊢
      simp [promotedPiece, noPromotion, piecePhasePotential]

private theorem boardAfterOrdinary_phase_le (position : Position) (move : Move) (piece : Piece)
    (occupied : position.board.pieceAt move.source = some piece)
    (ordinary : ordinaryPseudoLegal position move piece) :
    (boardAfterOrdinary position move piece).phasePotential ≤ position.board.phasePotential := by
  let moved := (position.board.clear move.source).set move.target
    (some (promotedPiece piece move))
  have balance := Board.phasePotential_clear_set_add position.board
    (ordinary_source_ne_target position move piece occupied ordinary)
    (some (promotedPiece piece move))
  rw [occupied] at balance
  have replacementLe := promotedPiece_phase_le position move piece ordinary
  have movedLe : moved.phasePotential ≤ position.board.phasePotential := by
    change ((position.board.clear move.source).set move.target
      (some (promotedPiece piece move))).phasePotential ≤ position.board.phasePotential
    omega
  unfold boardAfterOrdinary
  change (if piece.kind == .pawn && (position.board.pieceAt move.target).isNone &&
      move.source.file != move.target.file then
        (moved.clear { file := move.target.file, rank := move.source.rank })
      else moved).phasePotential ≤ position.board.phasePotential
  split
  · exact Nat.le_trans (Board.phasePotential_clear_le moved _) movedLe
  · exact movedLe

private theorem boardAfterOrdinary_phase_lt_of_occupied_target (position : Position)
    (move : Move) (piece captured : Piece)
    (occupied : position.board.pieceAt move.source = some piece)
    (targetOccupied : position.board.pieceAt move.target = some captured)
    (ordinary : ordinaryPseudoLegal position move piece) :
    (boardAfterOrdinary position move piece).phasePotential <
      position.board.phasePotential := by
  let moved := (position.board.clear move.source).set move.target
    (some (promotedPiece piece move))
  have balance := Board.phasePotential_clear_set_add position.board
    (ordinary_source_ne_target position move piece occupied ordinary)
    (some (promotedPiece piece move))
  rw [occupied, targetOccupied] at balance
  have replacementLe := promotedPiece_phase_le position move piece ordinary
  have capturedPos := piecePhasePotential_some_pos move.target captured
  have movedLt : moved.phasePotential < position.board.phasePotential := by
    change ((position.board.clear move.source).set move.target
      (some (promotedPiece piece move))).phasePotential < position.board.phasePotential
    omega
  simpa [boardAfterOrdinary, moved, targetOccupied] using movedLt

private theorem boardAfterOrdinary_phase_lt_of_pawn (position : Position) (move : Move)
    (piece : Piece) (occupied : position.board.pieceAt move.source = some piece)
    (isPawn : piece.kind = .pawn)
    (ordinary : ordinaryPseudoLegal position move piece) :
    (boardAfterOrdinary position move piece).phasePotential < position.board.phasePotential := by
  let moved := (position.board.clear move.source).set move.target
    (some (promotedPiece piece move))
  have balance := Board.phasePotential_clear_set_add position.board
    (ordinary_source_ne_target position move piece occupied ordinary)
    (some (promotedPiece piece move))
  rw [occupied] at balance
  have replacementLt := pawnMove_phase_decreases position move piece isPawn ordinary
  have movedLt : moved.phasePotential < position.board.phasePotential := by
    change ((position.board.clear move.source).set move.target
      (some (promotedPiece piece move))).phasePotential < position.board.phasePotential
    omega
  unfold boardAfterOrdinary
  change (if piece.kind == .pawn && (position.board.pieceAt move.target).isNone &&
      move.source.file != move.target.file then
        (moved.clear { file := move.target.file, rank := move.source.rank })
      else moved).phasePotential < position.board.phasePotential
  split
  · exact Nat.lt_of_le_of_lt (Board.phasePotential_clear_le moved _) movedLt
  · exact movedLt

private theorem castlePseudoLegal_target_empty (position : Position) (move : Move)
    (color : Color) (castle : castlePseudoLegal position move color) :
    position.board.pieceAt move.target = none := by
  cases sideEq : castleSide? color move with
  | none => simp [castlePseudoLegal, sideEq] at castle
  | some side =>
      simp [castlePseudoLegal, sideEq] at castle
      have targetEq : move.target = (castleData color side).kingTarget := by
        cases color <;> cases side
        · simp [castleSide?, castleData] at sideEq ⊢
          exact sideEq.2
        · simp [castleSide?, castleData] at sideEq ⊢
          by_cases kingSideTarget : move.target = Square.g1
          · simp [kingSideTarget] at sideEq
          · simp [kingSideTarget] at sideEq
            exact sideEq.2
        · simp [castleSide?, castleData] at sideEq ⊢
          exact sideEq.2
        · simp [castleSide?, castleData] at sideEq ⊢
          by_cases kingSideTarget : move.target = Square.g8
          · simp [kingSideTarget] at sideEq
          · simp [kingSideTarget] at sideEq
            exact sideEq.2
      rw [targetEq]
      apply castle.1.1.1.2
      cases color <;> cases side <;> decide

private theorem boardAfterCastle_phase_eq (position : Position) (move : Move) (color : Color)
    (occupied : position.board.pieceAt move.source = some ⟨color, .king⟩)
    (castle : castlePseudoLegal position move color) :
    (boardAfter position move ⟨color, .king⟩).phasePotential = position.board.phasePotential := by
  cases sideEq : castleSide? color move with
  | none => simp [castlePseudoLegal, sideEq] at castle
  | some side =>
      simp [castlePseudoLegal, sideEq] at castle
      let data := castleData color side
      have sourceEq : move.source = data.kingSource := by
        cases color <;> cases side <;>
          simp [data, castleSide?, castleData] at sideEq ⊢ <;> simp_all
      have kingAt : position.board.pieceAt data.kingSource = some ⟨color, .king⟩ := by
        rw [← sourceEq]
        exact occupied
      have rookAt : position.board.pieceAt data.rookSource = some ⟨color, .rook⟩ :=
        castle.1.1.1.1.2
      have emptySquares : ∀ square ∈ data.mustBeEmpty,
          position.board.pieceAt square = none := castle.1.1.1.2
      have kingTargetEmpty : position.board.pieceAt data.kingTarget = none := by
        apply emptySquares
        cases color <;> cases side <;> decide
      have rookTargetEmpty : position.board.pieceAt data.rookTarget = none := by
        apply emptySquares
        cases color <;> cases side <;> decide
      let kingMoved := (position.board.clear data.kingSource).set data.kingTarget
        (some ⟨color, .king⟩)
      have kingMoveEq : kingMoved.phasePotential = position.board.phasePotential := by
        apply Board.phasePotential_clear_set_eq
        · cases color <;> cases side <;> decide
        · exact kingTargetEmpty
        · simp [kingAt, piecePhasePotential]
      have rookAtAfter : kingMoved.pieceAt data.rookSource = some ⟨color, .rook⟩ := by
        dsimp [kingMoved]
        rw [Board.set_at_other (position.board.clear data.kingSource)
          (changed := data.kingTarget) (target := data.rookSource)
          (by cases color <;> cases side <;> decide)]
        change (position.board.set data.kingSource none).pieceAt data.rookSource = _
        rw [Board.set_at_other position.board (changed := data.kingSource)
          (target := data.rookSource) (by cases color <;> cases side <;> decide)]
        exact rookAt
      have rookTargetEmptyAfter : kingMoved.pieceAt data.rookTarget = none := by
        dsimp [kingMoved]
        rw [Board.set_at_other (position.board.clear data.kingSource)
          (changed := data.kingTarget) (target := data.rookTarget)
          (by cases color <;> cases side <;> decide)]
        change (position.board.set data.kingSource none).pieceAt data.rookTarget = _
        rw [Board.set_at_other position.board (changed := data.kingSource)
          (target := data.rookTarget) (by cases color <;> cases side <;> decide)]
        exact rookTargetEmpty
      have rookMoveEq :
          ((kingMoved.clear data.rookSource).set data.rookTarget
            (some ⟨color, .rook⟩)).phasePotential = kingMoved.phasePotential := by
        apply Board.phasePotential_clear_set_eq
        · cases color <;> cases side <;> decide
        · exact rookTargetEmptyAfter
        · simp [rookAtAfter, piecePhasePotential]
      unfold boardAfter
      simp only [sideEq]
      change ((kingMoved.clear data.rookSource).set data.rookTarget
        (some ⟨color, .rook⟩)).phasePotential = position.board.phasePotential
      rw [rookMoveEq, kingMoveEq]

private theorem revokeRookSquare_count_le (rights : CastlingRights) (square : Square) :
    (revokeRookSquare rights square).count ≤ rights.count := by
  unfold revokeRookSquare
  split
  · exact CastlingRights.count_revoke_le rights .white .queenSide
  · split
    · exact CastlingRights.count_revoke_le rights .white .kingSide
    · split
      · exact CastlingRights.count_revoke_le rights .black .queenSide
      · split
        · exact CastlingRights.count_revoke_le rights .black .kingSide
        · exact Nat.le_refl _

private theorem revokeRookSquare_subset (rights : CastlingRights) (square : Square) :
    (revokeRookSquare rights square).Subset rights := by
  unfold revokeRookSquare
  split
  · exact CastlingRights.revoke_subset rights .white .queenSide
  · split
    · exact CastlingRights.revoke_subset rights .white .kingSide
    · split
      · exact CastlingRights.revoke_subset rights .black .queenSide
      · split
        · exact CastlingRights.revoke_subset rights .black .kingSide
        · exact CastlingRights.subset_refl rights

private theorem rightsAfter_count_le (position : Position) (move : Move) (piece : Piece) :
    (rightsAfter position move piece).count ≤ position.castlingRights.count := by
  rcases piece with ⟨color, kind⟩
  cases kind with
  | king =>
      exact Nat.le_trans
        (revokeRookSquare_count_le (position.castlingRights.revokeKing color) move.target)
        (CastlingRights.count_revokeKing_le position.castlingRights color)
  | rook =>
      exact Nat.le_trans
        (revokeRookSquare_count_le
          (revokeRookSquare position.castlingRights move.source) move.target)
        (revokeRookSquare_count_le position.castlingRights move.source)
  | pawn | knight | bishop | queen =>
      exact revokeRookSquare_count_le position.castlingRights move.target

private theorem rightsAfter_subset (position : Position) (move : Move) (piece : Piece) :
    (rightsAfter position move piece).Subset position.castlingRights := by
  rcases piece with ⟨color, kind⟩
  cases kind with
  | king =>
      exact CastlingRights.subset_trans
        (revokeRookSquare_subset (position.castlingRights.revokeKing color) move.target)
        (CastlingRights.revokeKing_subset position.castlingRights color)
  | rook =>
      exact CastlingRights.subset_trans
        (revokeRookSquare_subset
          (revokeRookSquare position.castlingRights move.source) move.target)
        (revokeRookSquare_subset position.castlingRights move.source)
  | pawn | knight | bishop | queen =>
      exact revokeRookSquare_subset position.castlingRights move.target

private theorem ordinaryKing_castleSide_none (position : Position) (move : Move) (color : Color)
    (ordinary : ordinaryPseudoLegal position move ⟨color, .king⟩) :
    castleSide? color move = none := by
  rcases move with ⟨source, target, promotion⟩
  cases sideEq : castleSide? color (⟨source, target, promotion⟩ : Move) with
  | none => rfl
  | some side =>
      have sourceEq : source = (castleData color side).kingSource := by
        cases color <;> cases side <;>
          simp [castleSide?, castleData] at sideEq ⊢ <;> simp_all
      have targetEq : target = (castleData color side).kingTarget := by
        cases color <;> cases side
        · simp [castleSide?, castleData] at sideEq ⊢
          exact sideEq.2
        · simp [castleSide?, castleData] at sideEq ⊢
          by_cases kingSideTarget : target = Square.g1
          · simp [kingSideTarget] at sideEq
          · simp [kingSideTarget] at sideEq
            exact sideEq.2
        · simp [castleSide?, castleData] at sideEq ⊢
          exact sideEq.2
        · simp [castleSide?, castleData] at sideEq ⊢
          by_cases kingSideTarget : target = Square.g8
          · simp [kingSideTarget] at sideEq
          · simp [kingSideTarget] at sideEq
            exact sideEq.2
      subst source
      subst target
      have movement := ordinary
      simp [ordinaryPseudoLegal] at movement
      cases color <;> cases side
      · have notAttack : Square.g1 ∉
            attacksFrom position.board Square.e1 ⟨.white, .king⟩ :=
          whiteKing_e1_not_attack_g1 position.board
        exact (notAttack movement.2.2).elim
      · have notAttack : Square.c1 ∉
            attacksFrom position.board Square.e1 ⟨.white, .king⟩ :=
          whiteKing_e1_not_attack_c1 position.board
        exact (notAttack movement.2.2).elim
      · have notAttack : Square.g8 ∉
            attacksFrom position.board Square.e8 ⟨.black, .king⟩ :=
          blackKing_e8_not_attack_g8 position.board
        exact (notAttack movement.2.2).elim
      · have notAttack : Square.c8 ∉
            attacksFrom position.board Square.e8 ⟨.black, .king⟩ :=
          blackKing_e8_not_attack_c8 position.board
        exact (notAttack movement.2.2).elim

set_option maxHeartbeats 500000

private theorem boardAfter_phase_le (position : Position) (move : Move) (piece : Piece)
    (occupied : position.board.pieceAt move.source = some piece)
    (moves : ordinaryPseudoLegal position move piece ∨
      (piece.kind = .king ∧ castlePseudoLegal position move piece.color)) :
    (boardAfter position move piece).phasePotential ≤ position.board.phasePotential := by
  rcases moves with ordinary | castleMove
  · rcases piece with ⟨color, kind⟩
    cases kind with
    | king =>
        have noCastle := ordinaryKing_castleSide_none position move color ordinary
        simpa [boardAfter, noCastle] using
          boardAfterOrdinary_phase_le position move ⟨color, .king⟩ occupied ordinary
    | pawn | knight | bishop | rook | queen =>
        simpa [boardAfter] using
          boardAfterOrdinary_phase_le position move ⟨color, _⟩ occupied ordinary
  · have isKing : piece.kind = .king := castleMove.1
    rcases piece with ⟨color, kind⟩
    simp only at isKing
    subst kind
    exact Nat.le_of_eq
      (boardAfterCastle_phase_eq position move color occupied castleMove.2)

private theorem boardAfter_phase_lt_of_occupied_target (position : Position) (move : Move)
    (piece captured : Piece)
    (occupied : position.board.pieceAt move.source = some piece)
    (targetOccupied : position.board.pieceAt move.target = some captured)
    (moves : ordinaryPseudoLegal position move piece ∨
      (piece.kind = .king ∧ castlePseudoLegal position move piece.color)) :
    (boardAfter position move piece).phasePotential < position.board.phasePotential := by
  rcases moves with ordinary | castleMove
  · rcases piece with ⟨color, kind⟩
    cases kind with
    | king =>
        have noCastle := ordinaryKing_castleSide_none position move color ordinary
        simpa [boardAfter, noCastle] using
          boardAfterOrdinary_phase_lt_of_occupied_target position move
            ⟨color, .king⟩ captured occupied targetOccupied ordinary
    | pawn | knight | bishop | rook | queen =>
        simpa [boardAfter] using
          boardAfterOrdinary_phase_lt_of_occupied_target position move
            ⟨color, _⟩ captured occupied targetOccupied ordinary
  · have targetEmpty := castlePseudoLegal_target_empty position move piece.color castleMove.2
    rw [targetOccupied] at targetEmpty
    contradiction

private theorem applyUnchecked_board_of_occupied (position : Position) (move : Move)
    (piece : Piece) (occupied : position.board.pieceAt move.source = some piece) :
    (applyUnchecked position move).board = boardAfter position move piece := by
  simp [applyUnchecked, occupied]

private theorem applyUnchecked_rights_of_occupied (position : Position) (move : Move)
    (piece : Piece) (occupied : position.board.pieceAt move.source = some piece) :
    (applyUnchecked position move).castlingRights = rightsAfter position move piece := by
  simp [applyUnchecked, occupied]

/-- Every pseudo-legal move weakly descends the irreversible phase potential.
Thus the result also holds for every fully legal move. -/
theorem phasePotential_applyUnchecked_le_of_pseudoLegal (position : Position) (move : Move)
    (pseudo : PseudoLegal position move) :
    (applyUnchecked position move).phasePotential ≤ position.phasePotential := by
  unfold PseudoLegal at pseudo
  cases occupied : position.board.pieceAt move.source with
  | none => simp [isPseudoLegal, occupied] at pseudo
  | some piece =>
      unfold isPseudoLegal at pseudo
      rw [occupied] at pseudo
      have moves : ordinaryPseudoLegal position move piece ∨
          (piece.kind = .king ∧ castlePseudoLegal position move piece.color) := by
        cases colorEq : piece.color == position.turn
        · simp [colorEq] at pseudo
        · simpa [colorEq] using pseudo
      have boardLe : (boardAfter position move piece).phasePotential ≤
          position.board.phasePotential := by
        exact boardAfter_phase_le position move piece occupied moves
      have rightsLe := rightsAfter_count_le position move piece
      rw [Position.phasePotential, Position.phasePotential,
        applyUnchecked_board_of_occupied position move piece occupied,
        applyUnchecked_rights_of_occupied position move piece occupied]
      omega

set_option maxHeartbeats 200000

theorem phasePotential_applyUnchecked_le (position : Position) (move : Move)
    (legal : Legal position move) :
    (applyUnchecked position move).phasePotential ≤ position.phasePotential := by
  exact phasePotential_applyUnchecked_le_of_pseudoLegal position move
    ((legal_iff position move).mp legal).1

set_option maxHeartbeats 500000

/-- Every legal move to an occupied target strictly consumes phase potential.
Together with pawn strictness, this covers every form of capture, including en
passant. -/
theorem phasePotential_applyUnchecked_lt_of_occupied_target (position : Position)
    (move : Move) (captured : Piece)
    (targetOccupied : position.board.pieceAt move.target = some captured)
    (legal : Legal position move) :
    (applyUnchecked position move).phasePotential < position.phasePotential := by
  have pseudo := ((legal_iff position move).mp legal).1
  unfold PseudoLegal at pseudo
  cases occupied : position.board.pieceAt move.source with
  | none => simp [isPseudoLegal, occupied] at pseudo
  | some piece =>
      unfold isPseudoLegal at pseudo
      rw [occupied] at pseudo
      have moves : ordinaryPseudoLegal position move piece ∨
          (piece.kind = .king ∧ castlePseudoLegal position move piece.color) := by
        cases colorEq : piece.color == position.turn
        · simp [colorEq] at pseudo
        · simpa [colorEq] using pseudo
      have boardLt := boardAfter_phase_lt_of_occupied_target position move piece captured
        occupied targetOccupied moves
      have rightsLe := rightsAfter_count_le position move piece
      rw [Position.phasePotential, Position.phasePotential,
        applyUnchecked_board_of_occupied position move piece occupied,
        applyUnchecked_rights_of_occupied position move piece occupied]
      omega

/-- A legal move that actually changes the historical castling rights strictly
consumes phase potential. This includes castling, first king or rook moves, and
captures of still-entitled home rooks. -/
theorem phasePotential_applyUnchecked_lt_of_castlingRights_ne (position : Position)
    (move : Move) (legal : Legal position move)
    (changed : (applyUnchecked position move).castlingRights ≠ position.castlingRights) :
    (applyUnchecked position move).phasePotential < position.phasePotential := by
  have pseudo := ((legal_iff position move).mp legal).1
  unfold PseudoLegal at pseudo
  cases occupied : position.board.pieceAt move.source with
  | none => simp [isPseudoLegal, occupied] at pseudo
  | some piece =>
      unfold isPseudoLegal at pseudo
      rw [occupied] at pseudo
      have moves : ordinaryPseudoLegal position move piece ∨
          (piece.kind = .king ∧ castlePseudoLegal position move piece.color) := by
        cases colorEq : piece.color == position.turn
        · simp [colorEq] at pseudo
        · simpa [colorEq] using pseudo
      have boardLe := boardAfter_phase_le position move piece occupied moves
      have rightsDifferent : rightsAfter position move piece ≠ position.castlingRights := by
        intro same
        apply changed
        rw [applyUnchecked_rights_of_occupied position move piece occupied]
        exact same
      have rightsLt := CastlingRights.count_lt_of_subset_of_ne
        (rightsAfter_subset position move piece) rightsDifferent
      rw [Position.phasePotential, Position.phasePotential,
        applyUnchecked_board_of_occupied position move piece occupied,
        applyUnchecked_rights_of_occupied position move piece occupied]
      omega

/-- Every legal pawn move strictly consumes irreversible phase potential. This
includes captures, double steps, en passant, and promotions. -/
theorem phasePotential_applyUnchecked_lt_of_pawn (position : Position) (move : Move)
    (piece : Piece) (occupied : position.board.pieceAt move.source = some piece)
    (isPawn : piece.kind = .pawn) (legal : Legal position move) :
    (applyUnchecked position move).phasePotential < position.phasePotential := by
  have pseudo := ((legal_iff position move).mp legal).1
  have ordinary : ordinaryPseudoLegal position move piece := by
    unfold PseudoLegal isPseudoLegal at pseudo
    rw [occupied] at pseudo
    simp [isPawn] at pseudo
    exact pseudo.2
  have boardLt := boardAfterOrdinary_phase_lt_of_pawn position move piece occupied isPawn ordinary
  have rightsLe := rightsAfter_count_le position move piece
  rw [Position.phasePotential, Position.phasePotential,
    applyUnchecked_board_of_occupied position move piece occupied,
    applyUnchecked_rights_of_occupied position move piece occupied]
  rcases piece with ⟨color, kind⟩
  simp only at isPawn
  subst kind
  simp only [boardAfter]
  omega

set_option maxHeartbeats 200000

/-- An explicit legal transition in the chess game graph. -/
def Position.Successor (position next : Position) : Prop :=
  ∃ move, Legal position move ∧ next = applyUnchecked position move

end Chess
