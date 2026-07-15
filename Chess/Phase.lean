import Chess.Position

namespace Chess

/-- The irreversible pawn resource remaining at a square. It decreases whenever
the pawn advances toward promotion. -/
def pawnTravel (color : Color) (square : Square) : Nat :=
  match color with
  | .white => 7 - square.rank.val
  | .black => square.rank.val

/-- A local contribution to the irreversible phase potential.

Every piece contributes eight units, so removing a piece is visible. A pawn
contributes eight more units, so promotion is visible, and also contributes its
remaining rank-distance to promotion, so every pawn advance is visible. -/
def piecePhasePotential (square : Square) : Option Piece → Nat
  | none => 0
  | some piece =>
      8 + if piece.kind = .pawn then 8 + pawnTravel piece.color square else 0

@[simp] theorem piecePhasePotential_none (square : Square) :
    piecePhasePotential square none = 0 := rfl

theorem piecePhasePotential_some_pos (square : Square) (piece : Piece) :
    0 < piecePhasePotential square (some piece) := by
  simp [piecePhasePotential]
  omega

namespace Board

/-- Sum a phase contribution over a chosen list of squares. -/
def phasePotentialOn (board : Board) (squares : List Square) : Nat :=
  (squares.map fun square => piecePhasePotential square (board.pieceAt square)).sum

/-- The board part of the irreversible phase potential. -/
def phasePotential (board : Board) : Nat :=
  phasePotentialOn board Square.all

private theorem phasePotentialOn_set_of_not_mem (board : Board) (changed : Square)
    (piece : Option Piece) {squares : List Square} (absent : changed ∉ squares) :
    phasePotentialOn (board.set changed piece) squares = phasePotentialOn board squares := by
  induction squares with
  | nil => rfl
  | cons head tail ih =>
      simp only [List.mem_cons, not_or] at absent
      change piecePhasePotential head ((board.set changed piece).pieceAt head) +
          phasePotentialOn (board.set changed piece) tail =
        piecePhasePotential head (board.pieceAt head) + phasePotentialOn board tail
      rw [set_at_other board (Ne.symm absent.1) piece]
      rw [ih absent.2]

private theorem phasePotentialOn_set_add (board : Board) (changed : Square)
    (piece : Option Piece) {squares : List Square} (present : changed ∈ squares)
    (nodup : squares.Nodup) :
    phasePotentialOn (board.set changed piece) squares +
        piecePhasePotential changed (board.pieceAt changed) =
      phasePotentialOn board squares + piecePhasePotential changed piece := by
  induction squares with
  | nil => simp at present
  | cons head tail ih =>
      rw [List.nodup_cons] at nodup
      simp only [List.mem_cons] at present
      rcases present with headEq | inTail
      · subst head
        have tailUnchanged := phasePotentialOn_set_of_not_mem board changed piece nodup.1
        change piecePhasePotential changed ((board.set changed piece).pieceAt changed) +
              phasePotentialOn (board.set changed piece) tail +
              piecePhasePotential changed (board.pieceAt changed) =
            piecePhasePotential changed (board.pieceAt changed) +
              phasePotentialOn board tail + piecePhasePotential changed piece
        rw [set_at]
        rw [tailUnchanged]
        omega
      · have headNe : head ≠ changed := by
          intro headEq
          subst head
          exact nodup.1 inTail
        have rest := ih inTail nodup.2
        change piecePhasePotential head ((board.set changed piece).pieceAt head) +
              phasePotentialOn (board.set changed piece) tail +
              piecePhasePotential changed (board.pieceAt changed) =
            piecePhasePotential head (board.pieceAt head) + phasePotentialOn board tail +
              piecePhasePotential changed piece
        rw [set_at_other board headNe piece]
        omega

theorem phasePotential_set_add (board : Board) (changed : Square) (piece : Option Piece) :
    (board.set changed piece).phasePotential +
        piecePhasePotential changed (board.pieceAt changed) =
      board.phasePotential + piecePhasePotential changed piece := by
  apply phasePotentialOn_set_add
  · exact Square.mem_all changed
  · native_decide

theorem phasePotential_set_le (board : Board) (changed : Square) (piece : Option Piece)
    (doesNotIncrease : piecePhasePotential changed piece ≤
      piecePhasePotential changed (board.pieceAt changed)) :
    (board.set changed piece).phasePotential ≤ board.phasePotential := by
  have balance := phasePotential_set_add board changed piece
  omega

theorem phasePotential_clear_le (board : Board) (changed : Square) :
    (board.clear changed).phasePotential ≤ board.phasePotential := by
  apply phasePotential_set_le
  simp [piecePhasePotential]

/-- Exact accounting for moving/replacing a source piece at a distinct target.
The target's old contribution is charged on the left and the replacement's new
contribution is credited on the right. -/
theorem phasePotential_clear_set_add (board : Board) {source target : Square}
    (different : source ≠ target) (replacement : Option Piece) :
    ((board.clear source).set target replacement).phasePotential +
        piecePhasePotential source (board.pieceAt source) +
        piecePhasePotential target (board.pieceAt target) =
      board.phasePotential + piecePhasePotential target replacement := by
  have clearBalance := phasePotential_set_add board source none
  have targetUnchanged : (board.clear source).pieceAt target = board.pieceAt target := by
    exact set_at_other board (Ne.symm different) none
  have setBalance := phasePotential_set_add (board.clear source) target replacement
  simp only [piecePhasePotential_none, Nat.add_zero] at clearBalance
  change (board.clear source).phasePotential +
      piecePhasePotential source (board.pieceAt source) = board.phasePotential at clearBalance
  rw [targetUnchanged] at setBalance
  calc
    ((board.clear source).set target replacement).phasePotential +
          piecePhasePotential source (board.pieceAt source) +
          piecePhasePotential target (board.pieceAt target) =
        ((board.clear source).set target replacement).phasePotential +
          piecePhasePotential target (board.pieceAt target) +
          piecePhasePotential source (board.pieceAt source) := by omega
    _ = (board.clear source).phasePotential + piecePhasePotential target replacement +
          piecePhasePotential source (board.pieceAt source) := by rw [setBalance]
    _ = (board.clear source).phasePotential +
          piecePhasePotential source (board.pieceAt source) +
          piecePhasePotential target replacement := by omega
    _ = board.phasePotential + piecePhasePotential target replacement := by rw [clearBalance]

theorem phasePotential_clear_set_eq (board : Board) {source target : Square}
    (different : source ≠ target) (replacement : Option Piece)
    (targetEmpty : board.pieceAt target = none)
    (sameContribution : piecePhasePotential target replacement =
      piecePhasePotential source (board.pieceAt source)) :
    ((board.clear source).set target replacement).phasePotential = board.phasePotential := by
  have balance := phasePotential_clear_set_add board different replacement
  rw [targetEmpty, piecePhasePotential_none, Nat.add_zero, sameContribution] at balance
  omega

end Board

namespace CastlingRights

/-- The number of historical castling rights still available. -/
def count (rights : CastlingRights) : Nat :=
  rights.whiteKingSide.toNat + rights.whiteQueenSide.toNat +
    rights.blackKingSide.toNat + rights.blackQueenSide.toNat

/-- `after` retains only castling rights that were present in `before`.

This is the natural irreversible order on the four historical right bits. -/
def Subset (after before : CastlingRights) : Prop :=
  ∀ color side, after.has color side = true → before.has color side = true

theorem subset_refl (rights : CastlingRights) : rights.Subset rights := by
  intro _ _ present
  exact present

theorem subset_trans {first second third : CastlingRights}
    (firstSecond : first.Subset second) (secondThird : second.Subset third) :
    first.Subset third := by
  intro color side present
  exact secondThird color side (firstSecond color side present)

theorem revoke_subset (rights : CastlingRights) (color : Color) (side : CastleSide) :
    (rights.revoke color side).Subset rights := by
  intro queriedColor queriedSide present
  cases color <;> cases side <;> cases queriedColor <;> cases queriedSide <;>
    simp [revoke, has] at present ⊢ <;> assumption

theorem revokeKing_subset (rights : CastlingRights) (color : Color) :
    (rights.revokeKing color).Subset rights := by
  exact subset_trans (revoke_subset (rights.revoke color .kingSide) color .queenSide)
    (revoke_subset rights color .kingSide)

private theorem bool_toNat_le_of_true_imp (left right : Bool)
    (imp : left = true → right = true) : left.toNat ≤ right.toNat := by
  cases left <;> cases right <;> simp_all

private theorem bool_eq_of_toNat_eq (left right : Bool)
    (same : left.toNat = right.toNat) : left = right := by
  cases left <;> cases right <;> simp_all

theorem count_le_of_subset {after before : CastlingRights}
    (subset : after.Subset before) : after.count ≤ before.count := by
  have whiteKingLe := bool_toNat_le_of_true_imp
    after.whiteKingSide before.whiteKingSide (subset .white .kingSide)
  have whiteQueenLe := bool_toNat_le_of_true_imp
    after.whiteQueenSide before.whiteQueenSide (subset .white .queenSide)
  have blackKingLe := bool_toNat_le_of_true_imp
    after.blackKingSide before.blackKingSide (subset .black .kingSide)
  have blackQueenLe := bool_toNat_le_of_true_imp
    after.blackQueenSide before.blackQueenSide (subset .black .queenSide)
  unfold count
  omega

/-- Inclusion plus equal cardinality identifies a set of castling rights. -/
theorem eq_of_subset_of_count_eq {after before : CastlingRights}
    (subset : after.Subset before) (sameCount : after.count = before.count) :
    after = before := by
  have whiteKingLe := bool_toNat_le_of_true_imp
    after.whiteKingSide before.whiteKingSide (subset .white .kingSide)
  have whiteQueenLe := bool_toNat_le_of_true_imp
    after.whiteQueenSide before.whiteQueenSide (subset .white .queenSide)
  have blackKingLe := bool_toNat_le_of_true_imp
    after.blackKingSide before.blackKingSide (subset .black .kingSide)
  have blackQueenLe := bool_toNat_le_of_true_imp
    after.blackQueenSide before.blackQueenSide (subset .black .queenSide)
  unfold count at sameCount
  have whiteKingEq : after.whiteKingSide = before.whiteKingSide := by
    apply bool_eq_of_toNat_eq
    omega
  have whiteQueenEq : after.whiteQueenSide = before.whiteQueenSide := by
    apply bool_eq_of_toNat_eq
    omega
  have blackKingEq : after.blackKingSide = before.blackKingSide := by
    apply bool_eq_of_toNat_eq
    omega
  have blackQueenEq : after.blackQueenSide = before.blackQueenSide := by
    apply bool_eq_of_toNat_eq
    omega
  cases after
  cases before
  simp_all

theorem count_lt_of_subset_of_ne {after before : CastlingRights}
    (subset : after.Subset before) (different : after ≠ before) :
    after.count < before.count := by
  have countLe := count_le_of_subset subset
  have countNe : after.count ≠ before.count := by
    intro sameCount
    exact different (eq_of_subset_of_count_eq subset sameCount)
  omega

theorem count_revoke_le (rights : CastlingRights) (color : Color) (side : CastleSide) :
    (rights.revoke color side).count ≤ rights.count := by
  cases rights
  cases color <;> cases side <;> simp [revoke, count] <;> omega

theorem count_revokeKing_le (rights : CastlingRights) (color : Color) :
    (rights.revokeKing color).count ≤ rights.count := by
  exact Nat.le_trans (count_revoke_le (rights.revoke color .kingSide) color .queenSide)
    (count_revoke_le rights color .kingSide)

end CastlingRights

namespace Position

/-- A natural-valued grading of the position graph by irreversible resources.

Move clocks, the player to move, and the ephemeral en-passant target are absent:
they do not represent resources that can only be consumed. -/
def phasePotential (position : Position) : Nat :=
  position.board.phasePotential + position.castlingRights.count

end Position

end Chess
