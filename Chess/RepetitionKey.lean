import Chess.Initial
import Chess.Theory.RepetitionGraph

namespace Chess

/-- The board component of an executable repetition key.

Squares occur in `Square.all` order (`a1` through `h8`).  The length proof
prevents malformed keys with missing or extra squares, while the private
constructor keeps the ordering canonical. -/
structure BoardPlacement where
  private mk ::
  squares : List (Option Piece)
  size_eq : squares.length = 64
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

namespace BoardPlacement

/-- Read a board extensionally in the canonical 64-square order. -/
def ofBoard (board : Board) : BoardPlacement where
  squares := Square.all.map board.pieceAt
  size_eq := by simp [Square.all, Square.allCoordinates] <;> decide

/-- No information about piece placement is lost by `ofBoard`. -/
theorem ofBoard_injective : Function.Injective ofBoard := by
  intro left right equal
  apply Board.ext
  intro square
  have squaresEqual :
      Square.all.map left.pieceAt = Square.all.map right.pieceAt :=
    congrArg BoardPlacement.squares equal
  exact (List.map_inj_left.mp squaresEqual) square (Square.mem_all square)

@[simp] theorem ofBoard_eq_iff {left right : Board} :
    ofBoard left = ofBoard right ↔ left = right :=
  ofBoard_injective.eq_iff

end BoardPlacement

/-- A canonical, executable key for the modeled repetition identity.

For positions legally reached from the initial position, the key contains the
four rule-relevant components from FIDE Article 9.2.3: piece placement, side to
move, castling rights, and an en-passant target only when an en-passant capture
is actually legal. Move clocks are deliberately absent. Its derived `BEq`,
`DecidableEq`, and `Hashable` instances make it suitable for exact hash tables
rather than merely heuristic position hashes. -/
structure RepetitionKey where
  private mk ::
  placement : BoardPlacement
  turn : Color
  castlingRights : CastlingRights
  enPassantTarget : Option Square
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

namespace RepetitionKey

@[ext] theorem ext {left right : RepetitionKey}
    (placement : left.placement = right.placement)
    (turn : left.turn = right.turn)
    (castlingRights : left.castlingRights = right.castlingRights)
    (enPassantTarget : left.enPassantTarget = right.enPassantTarget) :
    left = right := by
  cases left
  cases right
  simp_all

/-- Compute the exact executable key for `sameForRepetition`.

Arbitrary `Position` values can encode malformed state such as stale castling
rights, so external FIDE correspondence additionally assumes legal reachability
or an equivalent well-formedness invariant. -/
def ofPosition (position : Position) : RepetitionKey where
  placement := BoardPlacement.ofBoard position.board
  turn := position.turn
  castlingRights := position.castlingRights
  enPassantTarget := effectiveEnPassantTarget position

/-- Equality of executable keys is exactly the existing proved repetition
relation. In particular this is not an implication justified by a
collision-prone engine hash: the key stores all modeled data extensionally. -/
theorem ofPosition_eq_iff {left right : Position} :
    ofPosition left = ofPosition right ↔ sameForRepetition left right := by
  constructor
  · intro equal
    have placementEqual := congrArg RepetitionKey.placement equal
    have boardEqual : left.board = right.board :=
      BoardPlacement.ofBoard_injective placementEqual
    have turnEqual : left.turn = right.turn :=
      congrArg RepetitionKey.turn equal
    have rightsEqual : left.castlingRights = right.castlingRights :=
      congrArg RepetitionKey.castlingRights equal
    have enPassantEqual :
        effectiveEnPassantTarget left = effectiveEnPassantTarget right :=
      congrArg RepetitionKey.enPassantTarget equal
    simp [sameForRepetition, boardEqual, turnEqual, rightsEqual, enPassantEqual]
  · intro same
    simp [sameForRepetition] at same
    have boardEqual : left.board = right.board := Board.eq_of_same same.1.1.1
    have turnEqual : left.turn = right.turn := same.1.1.2
    have rightsEqual : left.castlingRights = right.castlingRights :=
      same.1.2
    have enPassantEqual :
        effectiveEnPassantTarget left = effectiveEnPassantTarget right :=
      same.2
    apply RepetitionKey.ext
    · exact congrArg BoardPlacement.ofBoard boardEqual
    · exact turnEqual
    · exact rightsEqual
    · exact enPassantEqual

/-- The Boolean equality used by hash tables computes the same Boolean as
`sameForRepetition`, not just a propositionally related test. -/
@[simp] theorem beq_ofPosition (left right : Position) :
    (ofPosition left == ofPosition right) = sameForRepetition left right := by
  apply Bool.eq_iff_iff.mpr
  rw [beq_iff_eq]
  exact ofPosition_eq_iff

/-- Repetition-equivalent positions necessarily enter the same hash bucket.
The converse is intentionally not claimed: hash collisions are resolved by the
lawful Boolean equality above. -/
theorem hash_eq_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) :
    hash (ofPosition left) = hash (ofPosition right) :=
  congrArg hash (ofPosition_eq_iff.mpr same)

/-- Changing only the move clocks leaves the executable key unchanged. -/
theorem clocks_irrelevant :
    ofPosition Initial.position =
      ofPosition { Initial.position with halfmoveClock := 73, fullmoveNumber := 42 } := by
  rfl

/-- Historical castling rights remain part of the key even when placement is
unchanged. -/
theorem castling_rights_relevant :
    ofPosition Initial.position ≠
      ofPosition { Initial.position with castlingRights := .none } := by
  intro equal
  have rightsEqual := congrArg RepetitionKey.castlingRights equal
  simp [ofPosition, Initial.position, CastlingRights.initial, CastlingRights.none]
    at rightsEqual

/-- A raw en-passant square with no legal capture is normalized away. -/
theorem ineffective_enPassant_irrelevant :
    ofPosition Initial.position =
      ofPosition { Initial.position with enPassantTarget := some ⟨3, 5⟩ } := by
  rw [ofPosition_eq_iff]
  native_decide

private def legalEnPassantBoard : Board :=
  Board.empty
    |>.set Square.a1 (some ⟨.white, .king⟩)
    |>.set Square.h8 (some ⟨.black, .king⟩)
    |>.set ⟨2, 4⟩ (some ⟨.white, .pawn⟩)
    |>.set ⟨3, 4⟩ (some ⟨.black, .pawn⟩)

private def legalEnPassantPosition : Position where
  board := legalEnPassantBoard
  turn := .white
  castlingRights := .none
  enPassantTarget := some ⟨3, 5⟩
  halfmoveClock := 0
  fullmoveNumber := 1

/-- Conversely, a genuinely legal en-passant capture changes the key. -/
theorem effective_enPassant_relevant :
    ofPosition legalEnPassantPosition ≠
      ofPosition { legalEnPassantPosition with enPassantTarget := none } := by
  intro equal
  exact (by native_decide :
    ¬sameForRepetition legalEnPassantPosition
      { legalEnPassantPosition with enPassantTarget := none })
    (ofPosition_eq_iff.mp equal)

end RepetitionKey

namespace RepetitionNode

/-- Every abstract repetition node has an exact executable key.  The quotient
lift is sound because `RepetitionKey.ofPosition` is constant on precisely the
equivalence relation used to construct `RepetitionNode`. -/
def key (node : RepetitionNode) : RepetitionKey :=
  Quotient.lift RepetitionKey.ofPosition
    (fun _left _right same => RepetitionKey.ofPosition_eq_iff.mpr same) node

@[simp] theorem key_ofPosition (position : Position) :
    key (ofPosition position) = RepetitionKey.ofPosition position := rfl

/-- The executable key loses no quotient information. -/
theorem key_injective : Function.Injective key := by
  intro left right equal
  induction left using Quotient.inductionOn with
  | _ leftPosition =>
      induction right using Quotient.inductionOn with
      | _ rightPosition =>
          apply RepetitionNode.ofPosition_eq_iff.mpr
          exact RepetitionKey.ofPosition_eq_iff.mp equal

@[simp] theorem key_eq_iff {left right : RepetitionNode} :
    key left = key right ↔ left = right :=
  key_injective.eq_iff

end RepetitionNode
end Chess
