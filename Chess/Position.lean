import Chess.Board

namespace Chess

/-- The two castling wings, named by the king's destination side. -/
inductive CastleSide where
  | kingSide
  | queenSide
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

/-- Castling rights are historical rights, not facts recoverable from piece placement. -/
structure CastlingRights where
  whiteKingSide : Bool
  whiteQueenSide : Bool
  blackKingSide : Bool
  blackQueenSide : Bool
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

namespace CastlingRights

def none : CastlingRights := ⟨false, false, false, false⟩
def initial : CastlingRights := ⟨true, true, true, true⟩

/-- Whether the named player still has the historical right to castle on a side.

This does not assert that castling is currently legal: intervening pieces,
attacked king squares, and check are handled by move legality. -/
def has (rights : CastlingRights) (color : Color) (side : CastleSide) : Bool :=
  match color, side with
  | .white, .kingSide => rights.whiteKingSide
  | .white, .queenSide => rights.whiteQueenSide
  | .black, .kingSide => rights.blackKingSide
  | .black, .queenSide => rights.blackQueenSide

/-- Revoke one castling right. Castling rights can never be regained. -/
def revoke (rights : CastlingRights) (color : Color) (side : CastleSide) : CastlingRights :=
  match color, side with
  | .white, .kingSide => { rights with whiteKingSide := false }
  | .white, .queenSide => { rights with whiteQueenSide := false }
  | .black, .kingSide => { rights with blackKingSide := false }
  | .black, .queenSide => { rights with blackQueenSide := false }

/-- Revoke both castling rights after a king move. -/
def revokeKing (rights : CastlingRights) (color : Color) : CastlingRights :=
  (rights.revoke color .kingSide).revoke color .queenSide

@[simp] theorem initial_has (color : Color) (side : CastleSide) : initial.has color side := by
  cases color <;> cases side <;> decide

@[simp] theorem none_has_not (color : Color) (side : CastleSide) : ¬none.has color side := by
  cases color <;> cases side <;> decide

@[simp] theorem revoke_has_not (rights : CastlingRights) (color : Color) (side : CastleSide) :
    ¬(rights.revoke color side).has color side := by
  cases color <;> cases side <;> simp [revoke, has]

@[simp] theorem revokeKing_has_not (rights : CastlingRights) (color : Color)
    (side : CastleSide) : ¬(rights.revokeKing color).has color side := by
  cases color <;> cases side <;> simp [revokeKing, revoke, has]

end CastlingRights

/-- The complete instantaneous state needed to interpret orthodox chess moves.

`enPassantTarget` records the square over which the last double-stepping pawn
passed, when applicable. It is intentionally raw here: structural consistency
and reachability are predicates on positions rather than construction barriers.
-/
structure Position where
  board : Board
  turn : Color
  castlingRights : CastlingRights
  enPassantTarget : Option Square
  halfmoveClock : Nat
  fullmoveNumber : Nat

/-- A game state retains earlier positions because repetition is not a property
of the current `Position` alone. `prior` is stored newest first. -/
structure GameState where
  current : Position
  prior : List Position

end Chess
