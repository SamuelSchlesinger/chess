import Chess.Rules

namespace Chess

def HasLegalMove (position : Position) : Prop := ∃ move, Legal position move

def Checkmate (position : Position) : Prop :=
  inCheck position.board position.turn ∧ ¬HasLegalMove position

def Stalemate (position : Position) : Prop :=
  inCheck position.board position.turn = false ∧ ¬HasLegalMove position

/-- Reachability by a finite sequence of legal chess moves. -/
inductive Position.Reachable : Position → Position → Prop where
  | refl (position : Position) : Position.Reachable position position
  | step {start middle finish : Position} :
      Position.Successor start middle → Position.Reachable middle finish →
      Position.Reachable start finish

/-- The exact FIDE notion of a dead position: no continuation by any legal move
sequence can ever end in checkmate. This semantic definition covers more than
the familiar insufficient-material shortcuts. -/
def DeadPosition (position : Position) : Prop :=
  ∀ future, Position.Reachable position future → ¬Checkmate future

theorem deadPosition_not_checkmate {position : Position} (dead : DeadPosition position) :
    ¬Checkmate position := dead position (.refl position)

theorem checkmate_not_stalemate (position : Position) :
    Checkmate position → ¬Stalemate position := by
  rintro ⟨checked, _⟩ ⟨safe, _⟩
  simp_all

theorem stalemate_not_checkmate (position : Position) :
    Stalemate position → ¬Checkmate position := by
  intro stale mate
  exact checkmate_not_stalemate position mate stale

/-- A raw move is an en-passant capture in a position. Legality remains a
separate condition because a pinned capturing pawn may not actually move. -/
def isEnPassantCapture (position : Position) (move : Move) : Bool :=
  position.enPassantTarget == some move.target &&
    (position.board.pieceAt move.target).isNone &&
    move.source.file != move.target.file &&
    match position.board.pieceAt move.source with
    | some piece => piece.color == position.turn && piece.kind == .pawn
    | none => false

/-- En-passant state matters for repetition only if an en-passant capture is
actually legal. This handles the pinned-pawn subtlety in FIDE Article 9.2.3. -/
def effectiveEnPassantTarget (position : Position) : Option Square :=
  match position.enPassantTarget with
  | none => none
  | some target =>
      if (legalMoves position).any (isEnPassantCapture position) then some target else none

/-- FIDE repetition identity. Move clocks and the fullmove number are excluded;
piece placement, player to move, castling rights, and effective en-passant
possibility are included. -/
def sameForRepetition (left right : Position) : Bool :=
  left.board.same right.board &&
  left.turn == right.turn &&
  left.castlingRights == right.castlingRights &&
  effectiveEnPassantTarget left == effectiveEnPassantTarget right

@[simp] theorem sameForRepetition_self (position : Position) :
    sameForRepetition position position := by
  simp [sameForRepetition]

theorem sameForRepetition_symm {left right : Position}
    (same : sameForRepetition left right) : sameForRepetition right left := by
  simp [sameForRepetition] at same ⊢
  have boardEq := Board.eq_of_same same.1.1.1
  rw [boardEq]
  simp_all

theorem sameForRepetition_trans {first second third : Position}
    (firstSecond : sameForRepetition first second)
    (secondThird : sameForRepetition second third) :
    sameForRepetition first third := by
  simp [sameForRepetition] at firstSecond secondThird ⊢
  have firstBoardEq := Board.eq_of_same firstSecond.1.1.1
  have secondBoardEq := Board.eq_of_same secondThird.1.1.1
  rw [firstBoardEq, secondBoardEq]
  simp_all

theorem sameForRepetition_equivalence :
    Equivalence (fun left right : Position => sameForRepetition left right) :=
  ⟨sameForRepetition_self, sameForRepetition_symm, sameForRepetition_trans⟩

/-- Number of occurrences of the current repetition-equivalence class in the
recorded game, including the current position. -/
def repetitionCount (state : GameState) : Nat :=
  (state.current :: state.prior).foldl
    (fun count position => if sameForRepetition state.current position then count + 1 else count) 0

def ThreefoldRepetition (state : GameState) : Prop := 3 ≤ repetitionCount state
def FivefoldRepetition (state : GameState) : Prop := 5 ≤ repetitionCount state
def FiftyMoveClaimAvailable (state : GameState) : Prop := 100 ≤ state.current.halfmoveClock
def SeventyFiveMoveLimit (state : GameState) : Prop := 150 ≤ state.current.halfmoveClock

instance (state : GameState) : Decidable (ThreefoldRepetition state) := by
  unfold ThreefoldRepetition
  infer_instance

instance (state : GameState) : Decidable (FivefoldRepetition state) := by
  unfold FivefoldRepetition
  infer_instance

instance (state : GameState) : Decidable (FiftyMoveClaimAvailable state) := by
  unfold FiftyMoveClaimAvailable
  infer_instance

instance (state : GameState) : Decidable (SeventyFiveMoveLimit state) := by
  unfold SeventyFiveMoveLimit
  infer_instance

/-- Player-claimable draw conditions in the current state. -/
def DrawClaimAvailable (state : GameState) : Prop :=
  ThreefoldRepetition state ∨ FiftyMoveClaimAvailable state

instance (state : GameState) : Decidable (DrawClaimAvailable state) := by
  unfold DrawClaimAvailable
  infer_instance

/-- Append a legal move to the history, newest previous position first. -/
def GameState.afterMove (state : GameState) (move : Move) : GameState where
  current := applyUnchecked state.current move
  prior := state.current :: state.prior

/-- A claim may also be made by indicating a legal move that will produce the
third repetition or the 50-move threshold. -/
def DrawClaimAvailableAfter (state : GameState) (move : Move) : Prop :=
  Legal state.current move ∧ DrawClaimAvailable (state.afterMove move)

instance (state : GameState) (move : Move) : Decidable (DrawClaimAvailableAfter state move) := by
  unfold DrawClaimAvailableAfter
  infer_instance

/-- Automatic FIDE draws. Checkmate explicitly takes precedence over the
75-move limit; fivefold repetition and dead position terminate immediately. -/
def AutomaticDraw (state : GameState) : Prop :=
  Stalemate state.current ∨
  DeadPosition state.current ∨
  FivefoldRepetition state ∨
  (SeventyFiveMoveLimit state ∧ ¬Checkmate state.current)

theorem checkmate_takes_precedence_over_seventyFive
    {state : GameState} (mate : Checkmate state.current) :
    ¬(SeventyFiveMoveLimit state ∧ ¬Checkmate state.current) := by
  rintro ⟨_, notMate⟩
  exact notMate mate

inductive WinReason where
  | checkmate
  | resignation
  | timeForfeit
  deriving DecidableEq, Repr

inductive DrawReason where
  | stalemate
  | deadPosition
  | agreement
  | threefoldClaim
  | fiftyMoveClaim
  | fivefoldRepetition
  | seventyFiveMoveLimit
  | noMatingPossibilityOnResignation
  | noMatingPossibilityOnTime
  deriving DecidableEq, Repr

inductive GameConclusion where
  | win (winner : Color) (reason : WinReason)
  | draw (reason : DrawReason)
  deriving DecidableEq, Repr

/-- A complete abstract FIDE game: legal history plus an optional terminal
result. Physical-board procedure and arbiter penalties can be layered as events
without changing chess-theory positions. -/
structure Game where
  state : GameState
  conclusion : Option GameConclusion := none

/-- A recorded checkmate conclusion is semantically justified by the current
position and awards the win to the opponent of the mated player. -/
def CheckmateConclusionValid (game : Game) : Prop :=
  game.conclusion = some (.win game.state.current.turn.other .checkmate) ∧
    Checkmate game.state.current

/-- A recorded automatic-draw conclusion must be backed by an automatic FIDE
draw condition. -/
def AutomaticDrawConclusionValid (game : Game) : Prop :=
  (∃ reason, game.conclusion = some (.draw reason)) ∧ AutomaticDraw game.state

end Chess
