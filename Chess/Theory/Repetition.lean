import Chess.Game

namespace Chess

/-- The complete raw move enumeration contains every move value. -/
theorem Move.mem_all (move : Move) : move ∈ Move.all := by
  rcases move with ⟨source, target, promotion⟩
  simp [Move.all, Square.mem_all]
  cases promotion with
  | none => simp
  | some promotion => cases promotion <;> simp

private theorem mem_legalMoves_of_legal {position : Position} {move : Move}
    (legal : Legal position move) :
    move ∈ legalMoves position := by
  unfold Legal at legal
  simp only [legalMoves, List.mem_filter]
  exact ⟨Move.mem_all move, legal⟩

/-- A legal move recognized as en passant makes its target effective for FIDE
repetition identity. -/
private theorem effectiveEnPassantTarget_eq_of_legal_capture
    {position : Position} {move : Move}
    (legal : Legal position move)
    (capture : isEnPassantCapture position move) :
    effectiveEnPassantTarget position = some move.target := by
  have rawTarget : position.enPassantTarget = some move.target := by
    by_cases target : position.enPassantTarget = some move.target
    · exact target
    · simp [isEnPassantCapture, target] at capture
  have anyCapture :
      (legalMoves position).any (isEnPassantCapture position) := by
    rw [List.any_eq_true]
    exact ⟨move, mem_legalMoves_of_legal legal, capture⟩
  simp [effectiveEnPassantTarget, rawTarget, anyCapture]

/-- An effective en-passant target must be the raw target recorded in the
position. -/
private theorem enPassantTarget_eq_of_effective_eq_some
    {position : Position} {target : Square}
    (effective : effectiveEnPassantTarget position = some target) :
    position.enPassantTarget = some target := by
  unfold effectiveEnPassantTarget at effective
  cases raw : position.enPassantTarget with
  | none => simp [raw] at effective
  | some rawTarget =>
      simp [raw] at effective
      rw [effective.2]

/-- FIDE repetition-equivalent positions have the same legal raw moves. -/
private theorem legal_of_sameForRepetition {left right : Position} {move : Move}
    (same : sameForRepetition left right)
    (legalLeft : Legal left move) :
    Legal right move := by
  have fields := same
  simp [sameForRepetition] at fields
  have boardEq : left.board = right.board := Board.eq_of_same fields.1.1.1
  have turnEq : left.turn = right.turn := fields.1.1.2
  have rightsEq : left.castlingRights = right.castlingRights := fields.1.2
  by_cases legalRight : Legal right move
  · exact legalRight
  · have captureLeft : isEnPassantCapture left move :=
      isEnPassantCapture_of_legal_of_not_legal move boardEq turnEq rightsEq
        legalLeft legalRight
    have leftEffective := effectiveEnPassantTarget_eq_of_legal_capture
      legalLeft captureLeft
    have rightEffective : effectiveEnPassantTarget right = some move.target := by
      rw [← fields.2]
      exact leftEffective
    have leftRaw := enPassantTarget_eq_of_effective_eq_some leftEffective
    have rightRaw := enPassantTarget_eq_of_effective_eq_some rightEffective
    have rawEq : left.enPassantTarget = right.enPassantTarget :=
      leftRaw.trans rightRaw.symm
    exact (legal_iff_of_rule_fields_eq move boardEq turnEq rightsEq rawEq).mp legalLeft

theorem legal_iff_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) (move : Move) :
    Legal left move ↔ Legal right move := by
  constructor
  · exact legal_of_sameForRepetition same
  · exact legal_of_sameForRepetition (sameForRepetition_symm same)

theorem hasLegalMove_iff_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) :
    HasLegalMove left ↔ HasLegalMove right := by
  constructor
  · rintro ⟨move, legal⟩
    exact ⟨move, (legal_iff_of_sameForRepetition same move).mp legal⟩
  · rintro ⟨move, legal⟩
    exact ⟨move, (legal_iff_of_sameForRepetition same move).mpr legal⟩

theorem inCheck_eq_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) :
    inCheck left.board left.turn = inCheck right.board right.turn := by
  simp [sameForRepetition] at same
  have boardEq := Board.eq_of_same same.1.1.1
  rw [boardEq, same.1.1.2]

theorem checkmate_iff_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) :
    Checkmate left ↔ Checkmate right := by
  unfold Checkmate
  rw [inCheck_eq_of_sameForRepetition same,
    hasLegalMove_iff_of_sameForRepetition same]

theorem stalemate_iff_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) :
    Stalemate left ↔ Stalemate right := by
  unfold Stalemate
  rw [inCheck_eq_of_sameForRepetition same,
    hasLegalMove_iff_of_sameForRepetition same]

/-- Executable legality is constant on FIDE repetition classes. -/
theorem isLegal_eq_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) (move : Move) :
    isLegal left move = isLegal right move := by
  have legalIff := legal_iff_of_sameForRepetition same move
  cases leftLegal : isLegal left move <;>
    cases rightLegal : isLegal right move <;>
    simp [Legal, leftLegal, rightLegal] at legalIff ⊢

theorem legalMoves_eq_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) :
    legalMoves left = legalMoves right := by
  have predicatesEq : isLegal left = isLegal right := by
    funext move
    exact isLegal_eq_of_sameForRepetition same move
  simp [legalMoves, predicatesEq]

private theorem isLegal_eq_of_rule_fields_eq {left right : Position} (move : Move)
    (boardEq : left.board = right.board)
    (turnEq : left.turn = right.turn)
    (rightsEq : left.castlingRights = right.castlingRights)
    (enPassantEq : left.enPassantTarget = right.enPassantTarget) :
    isLegal left move = isLegal right move := by
  have legalIff := legal_iff_of_rule_fields_eq move boardEq turnEq rightsEq enPassantEq
  cases leftLegal : isLegal left move <;>
    cases rightLegal : isLegal right move <;>
    simp [Legal, leftLegal, rightLegal] at legalIff ⊢

private theorem legalMoves_eq_of_rule_fields_eq {left right : Position}
    (boardEq : left.board = right.board)
    (turnEq : left.turn = right.turn)
    (rightsEq : left.castlingRights = right.castlingRights)
    (enPassantEq : left.enPassantTarget = right.enPassantTarget) :
    legalMoves left = legalMoves right := by
  have predicatesEq : isLegal left = isLegal right := by
    funext move
    exact isLegal_eq_of_rule_fields_eq move boardEq turnEq rightsEq enPassantEq
  simp [legalMoves, predicatesEq]

private theorem isEnPassantCapture_eq_of_rule_fields_eq {left right : Position}
    (move : Move)
    (boardEq : left.board = right.board)
    (turnEq : left.turn = right.turn)
    (enPassantEq : left.enPassantTarget = right.enPassantTarget) :
    isEnPassantCapture left move = isEnPassantCapture right move := by
  cases left with
  | mk leftBoard leftTurn leftRights leftEnPassant leftHalfmove leftFullmove =>
      cases right with
      | mk rightBoard rightTurn rightRights rightEnPassant rightHalfmove rightFullmove =>
          simp only at boardEq turnEq enPassantEq
          subst rightBoard
          subst rightTurn
          subst rightEnPassant
          rfl

private theorem effectiveEnPassantTarget_eq_of_rule_fields_eq {left right : Position}
    (boardEq : left.board = right.board)
    (turnEq : left.turn = right.turn)
    (rightsEq : left.castlingRights = right.castlingRights)
    (enPassantEq : left.enPassantTarget = right.enPassantTarget) :
    effectiveEnPassantTarget left = effectiveEnPassantTarget right := by
  have movesEq := legalMoves_eq_of_rule_fields_eq boardEq turnEq rightsEq enPassantEq
  have capturesEq : isEnPassantCapture left = isEnPassantCapture right := by
    funext move
    exact isEnPassantCapture_eq_of_rule_fields_eq move boardEq turnEq enPassantEq
  unfold effectiveEnPassantTarget
  rw [enPassantEq, movesEq, capturesEq]

/-- Agreement on all four rule-relevant fields implies FIDE repetition
identity, irrespective of the move clocks. -/
private theorem sameForRepetition_of_rule_fields_eq {left right : Position}
    (boardEq : left.board = right.board)
    (turnEq : left.turn = right.turn)
    (rightsEq : left.castlingRights = right.castlingRights)
    (enPassantEq : left.enPassantTarget = right.enPassantTarget) :
    sameForRepetition left right := by
  have effectiveEq := effectiveEnPassantTarget_eq_of_rule_fields_eq
    boardEq turnEq rightsEq enPassantEq
  simp [sameForRepetition, boardEq, turnEq, rightsEq, effectiveEq]

/-- Unchecked move application respects FIDE repetition identity for every raw
move. Legality is not needed: an empty source leaves both repetition classes
unchanged, while an occupied source replaces every possibly unequal raw field. -/
theorem sameForRepetition_applyUnchecked {left right : Position}
    (same : sameForRepetition left right) (move : Move) :
    sameForRepetition (applyUnchecked left move) (applyUnchecked right move) := by
  have fields := same
  simp [sameForRepetition] at fields
  have boardEq : left.board = right.board := Board.eq_of_same fields.1.1.1
  have turnEq : left.turn = right.turn := fields.1.1.2
  have rightsEq : left.castlingRights = right.castlingRights := fields.1.2
  cases sourcePiece : left.board.pieceAt move.source with
  | none =>
      have rightSource : right.board.pieceAt move.source = none := by
        rw [← boardEq]
        exact sourcePiece
      simpa [applyUnchecked, sourcePiece, rightSource] using same
  | some piece =>
      have resultFields := applyUnchecked_rule_fields_eq_of_occupied
        move piece boardEq turnEq rightsEq sourcePiece
      exact sameForRepetition_of_rule_fields_eq
        resultFields.1 resultFields.2.1 resultFields.2.2.1 resultFields.2.2.2

/-- Repetition-equivalent positions have identical finite-depth move trees as
measured by the standard `perft` leaf count. -/
theorem perft_eq_of_sameForRepetition (depth : Nat) {left right : Position}
    (same : sameForRepetition left right) :
    perft depth left = perft depth right := by
  induction depth generalizing left right with
  | zero => rfl
  | succ depth ih =>
      simp only [perft]
      rw [legalMoves_eq_of_sameForRepetition same]
      have foldEq (moves : List Move) (total : Nat) :
          moves.foldl
              (fun count move => count + perft depth (applyUnchecked left move)) total =
            moves.foldl
              (fun count move => count + perft depth (applyUnchecked right move)) total := by
        induction moves generalizing total with
        | nil => rfl
        | cons move rest restIH =>
            simp only [List.foldl_cons]
            rw [ih (sameForRepetition_applyUnchecked same move)]
            exact restIH _
      exact foldEq (legalMoves right) 0

end Chess
