import Chess.Initial
import Chess.Game

namespace Chess.Theory

/-- Apply a recorded line of moves. Legality is checked separately so that the
fold remains executable on imported game data. -/
def playMoves : Position → List Move → Position
  | position, [] => position
  | position, move :: rest => playMoves (applyUnchecked position move) rest

/-- Executable certification that every move in a line is legal at the point
where it is played. -/
def lineIsLegal : Position → List Move → Bool
  | _, [] => true
  | position, move :: rest =>
      isLegal position move && lineIsLegal (applyUnchecked position move) rest

/-- Executable extensional equality for complete positions, including fields
that FIDE repetition identity deliberately ignores. -/
def sameCompletePosition (left right : Position) : Bool :=
  left.board.same right.board &&
  left.turn == right.turn &&
  left.castlingRights == right.castlingRights &&
  left.enPassantTarget == right.enPassantTarget &&
  left.halfmoveClock == right.halfmoveClock &&
  left.fullmoveNumber == right.fullmoveNumber

@[simp] theorem sameCompletePosition_self (position : Position) :
    sameCompletePosition position position := by
  simp [sameCompletePosition]

/-- The executable comparison recognizes actual equality of complete
positions, not merely repetition equivalence. -/
theorem eq_of_sameCompletePosition {left right : Position}
    (same : sameCompletePosition left right) : left = right := by
  simp [sameCompletePosition] at same
  have boardEq := Board.eq_of_same same.1.1.1.1.1
  rcases left with ⟨leftBoard, leftTurn, leftRights, leftEp, leftHalfmove, leftFullmove⟩
  rcases right with ⟨rightBoard, rightTurn, rightRights, rightEp, rightHalfmove, rightFullmove⟩
  simp only at boardEq same ⊢
  simp_all

theorem sameCompletePosition_iff_eq (left right : Position) :
    sameCompletePosition left right ↔ left = right := by
  constructor
  · exact eq_of_sameCompletePosition
  · rintro rfl
    exact sameCompletePosition_self left

/-- Executing concatenated lines is composition of their state transformers. -/
theorem playMoves_append (position : Position) (initialMoves suffix : List Move) :
    playMoves position (initialMoves ++ suffix) =
      playMoves (playMoves position initialMoves) suffix := by
  induction initialMoves generalizing position with
  | nil => rfl
  | cons move rest ih =>
      simp only [List.cons_append, playMoves]
      exact ih (applyUnchecked position move)

/-- A concatenated line is legal exactly when its prefix is legal and its
suffix is legal at the prefix endpoint. -/
theorem lineIsLegal_append (position : Position) (initialMoves suffix : List Move) :
    lineIsLegal position (initialMoves ++ suffix) ↔
      lineIsLegal position initialMoves ∧
        lineIsLegal (playMoves position initialMoves) suffix := by
  induction initialMoves generalizing position with
  | nil => simp [lineIsLegal, playMoves]
  | cons move rest ih =>
      simp only [List.cons_append, lineIsLegal, playMoves]
      simp [ih, and_assoc]

/-- Two certified lines transpose at a position when they reach the same
complete instantaneous endpoint. Their `GameState` histories may still differ. -/
structure LinesTransposeAt (position : Position) (left right : List Move) : Prop where
  leftLegal : lineIsLegal position left
  rightLegal : lineIsLegal position right
  endpointEq : playMoves position left = playMoves position right

/-- The opening-theory notion of transposition: both legal lines reach the same
node of the FIDE repetition graph. Clocks and ineffective raw en-passant targets
may differ. -/
structure LinesRepetitionTransposeAt (position : Position) (left right : List Move) : Prop where
  leftLegal : lineIsLegal position left
  rightLegal : lineIsLegal position right
  sameNode : sameForRepetition (playMoves position left) (playMoves position right)

theorem linesTransposeAt_repetition {position : Position} {left right : List Move}
    (exact : LinesTransposeAt position left right) :
    LinesRepetitionTransposeAt position left right := by
  refine ⟨exact.leftLegal, exact.rightLegal, ?_⟩
  rw [exact.endpointEq]
  exact sameForRepetition_self _

theorem linesRepetitionTransposeAt_refl (position : Position) {line : List Move}
    (legal : lineIsLegal position line) : LinesRepetitionTransposeAt position line line :=
  ⟨legal, legal, sameForRepetition_self _⟩

theorem linesRepetitionTransposeAt_symm {position : Position} {left right : List Move}
    (transpose : LinesRepetitionTransposeAt position left right) :
    LinesRepetitionTransposeAt position right left :=
  ⟨transpose.rightLegal, transpose.leftLegal,
    sameForRepetition_symm transpose.sameNode⟩

theorem linesRepetitionTransposeAt_trans {position : Position}
    {first second third : List Move}
    (firstSecond : LinesRepetitionTransposeAt position first second)
    (secondThird : LinesRepetitionTransposeAt position second third) :
    LinesRepetitionTransposeAt position first third :=
  ⟨firstSecond.leftLegal, secondThird.rightLegal,
    sameForRepetition_trans firstSecond.sameNode secondThird.sameNode⟩

theorem linesTransposeAt_refl (position : Position) {line : List Move}
    (legal : lineIsLegal position line) : LinesTransposeAt position line line :=
  ⟨legal, legal, rfl⟩

theorem linesTransposeAt_symm {position : Position} {left right : List Move}
    (transpose : LinesTransposeAt position left right) :
    LinesTransposeAt position right left :=
  ⟨transpose.rightLegal, transpose.leftLegal, transpose.endpointEq.symm⟩

theorem linesTransposeAt_trans {position : Position} {first second third : List Move}
    (firstSecond : LinesTransposeAt position first second)
    (secondThird : LinesTransposeAt position second third) :
    LinesTransposeAt position first third :=
  ⟨firstSecond.leftLegal, secondThird.rightLegal,
    firstSecond.endpointEq.trans secondThird.endpointEq⟩

/-- Transposition is preserved when both lines are placed after the same legal
prefix. -/
theorem linesTransposeAt_prepend {position : Position} {left right initialMoves : List Move}
    (initialLegal : lineIsLegal position initialMoves)
    (transpose : LinesTransposeAt (playMoves position initialMoves) left right) :
    LinesTransposeAt position (initialMoves ++ left) (initialMoves ++ right) := by
  refine ⟨(lineIsLegal_append position initialMoves left).mpr
      ⟨initialLegal, transpose.leftLegal⟩,
    (lineIsLegal_append position initialMoves right).mpr
      ⟨initialLegal, transpose.rightLegal⟩, ?_⟩
  rw [playMoves_append, playMoves_append, transpose.endpointEq]

/-- Transposition is preserved by any common continuation that is legal from
the shared endpoint. -/
theorem linesTransposeAt_append {position : Position} {left right suffix : List Move}
    (transpose : LinesTransposeAt position left right)
    (suffixLegal : lineIsLegal (playMoves position left) suffix) :
    LinesTransposeAt position (left ++ suffix) (right ++ suffix) := by
  have suffixLegalRight : lineIsLegal (playMoves position right) suffix := by
    rw [← transpose.endpointEq]
    exact suffixLegal
  refine ⟨(lineIsLegal_append position left suffix).mpr
      ⟨transpose.leftLegal, suffixLegal⟩,
    (lineIsLegal_append position right suffix).mpr
      ⟨transpose.rightLegal, suffixLegalRight⟩, ?_⟩
  rw [playMoves_append, playMoves_append, transpose.endpointEq]

/-- A complete move/reply pair, used as the atomic plan block when comparing
opening move orders. -/
structure ReplyPlan where
  move : Move
  reply : Move

namespace ReplyPlan

def moves (plan : ReplyPlan) : List Move := [plan.move, plan.reply]

def LegalAt (plan : ReplyPlan) (position : Position) : Prop :=
  lineIsLegal position plan.moves

def after (plan : ReplyPlan) (position : Position) : Position :=
  playMoves position plan.moves

end ReplyPlan

/-- Two move/reply plans commute when both alternating four-ply orders are
legal and close to the same complete endpoint. -/
def ReplyPlansCommuteAt (position : Position) (first second : ReplyPlan) : Prop :=
  LinesTransposeAt position (first.moves ++ second.moves) (second.moves ++ first.moves)

/-- Semantic independence of two move/reply plans. Each plan is enabled before
and after the other, and their state transformations commute. Future tactical
criteria can establish these five obligations from disjoint influence regions. -/
structure ReplyPlansIndependentAt (position : Position) (first second : ReplyPlan) : Prop where
  firstLegal : first.LegalAt position
  secondAfterFirstLegal : second.LegalAt (first.after position)
  secondLegal : second.LegalAt position
  firstAfterSecondLegal : first.LegalAt (second.after position)
  effectsCommute : second.after (first.after position) = first.after (second.after position)

/-- Independent move/reply plans form a legal move-order diamond. -/
theorem replyPlansCommute_of_independent {position : Position} {first second : ReplyPlan}
    (independent : ReplyPlansIndependentAt position first second) :
    ReplyPlansCommuteAt position first second := by
  refine ⟨(lineIsLegal_append position first.moves second.moves).mpr
      ⟨independent.firstLegal, independent.secondAfterFirstLegal⟩,
    (lineIsLegal_append position second.moves first.moves).mpr
      ⟨independent.secondLegal, independent.firstAfterSecondLegal⟩, ?_⟩
  rw [playMoves_append, playMoves_append]
  exact independent.effectsCommute

theorem replyPlansIndependent_of_commute {position : Position} {first second : ReplyPlan}
    (commute : ReplyPlansCommuteAt position first second) :
    ReplyPlansIndependentAt position first second := by
  have firstOrder :=
    (lineIsLegal_append position first.moves second.moves).mp commute.leftLegal
  have secondOrder :=
    (lineIsLegal_append position second.moves first.moves).mp commute.rightLegal
  refine ⟨firstOrder.1, firstOrder.2, secondOrder.1, secondOrder.2, ?_⟩
  simpa [ReplyPlan.after, playMoves_append] using commute.endpointEq

theorem replyPlansCommute_iff_independent (position : Position)
    (first second : ReplyPlan) :
    ReplyPlansCommuteAt position first second ↔
      ReplyPlansIndependentAt position first second :=
  ⟨replyPlansIndependent_of_commute, replyPlansCommute_of_independent⟩

/-- Every certified opening line denotes a path through the legal position
graph. -/
theorem reachable_playMoves_of_lineIsLegal (position : Position) (moves : List Move)
    (legal : lineIsLegal position moves) :
    Position.Reachable position (playMoves position moves) := by
  induction moves generalizing position with
  | nil => exact .refl position
  | cons move rest ih =>
      simp [lineIsLegal] at legal
      exact .step ⟨move, legal.1, rfl⟩ (ih _ legal.2)

namespace OpeningExamples

private def g1f3 : Move := ⟨⟨6, 0⟩, ⟨5, 2⟩, none⟩
private def g8f6 : Move := ⟨⟨6, 7⟩, ⟨5, 5⟩, none⟩
private def b1c3 : Move := ⟨⟨1, 0⟩, ⟨2, 2⟩, none⟩
private def b8c6 : Move := ⟨⟨1, 7⟩, ⟨2, 5⟩, none⟩
private def d2d4 : Move := ⟨⟨3, 1⟩, ⟨3, 3⟩, none⟩
private def d7d5 : Move := ⟨⟨3, 6⟩, ⟨3, 4⟩, none⟩

private def kingsidePlan : ReplyPlan := ⟨g1f3, g8f6⟩
private def queensidePlan : ReplyPlan := ⟨b1c3, b8c6⟩

private def kingsideFirst : List Move := kingsidePlan.moves ++ queensidePlan.moves
private def queensideFirst : List Move := queensidePlan.moves ++ kingsidePlan.moves

theorem kingsideFirst_legal : lineIsLegal Initial.position kingsideFirst := by
  native_decide

theorem queensideFirst_legal : lineIsLegal Initial.position queensideFirst := by
  native_decide

/-- A genuine opening transposition: the two legal move orders reach
extensionally identical complete positions, including turn, castling rights,
en-passant state, and move clocks—not merely the same piece placement. -/
theorem independent_knight_development_transposes :
    sameCompletePosition
      (playMoves Initial.position kingsideFirst)
      (playMoves Initial.position queensideFirst) := by
  native_decide

theorem independent_knight_development_endpoint_eq :
    playMoves Initial.position kingsideFirst =
      playMoves Initial.position queensideFirst :=
  eq_of_sameCompletePosition independent_knight_development_transposes

/-- The two plans satisfy the semantic independence interface: either can be
played first, the other remains legal, and their effects commute. -/
theorem independent_knight_plans_independent :
    ReplyPlansIndependentAt Initial.position kingsidePlan queensidePlan := by
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · change lineIsLegal Initial.position kingsidePlan.moves = true
    native_decide
  · change lineIsLegal (kingsidePlan.after Initial.position) queensidePlan.moves = true
    native_decide
  · change lineIsLegal Initial.position queensidePlan.moves = true
    native_decide
  · change lineIsLegal (queensidePlan.after Initial.position) kingsidePlan.moves = true
    native_decide
  · simpa [ReplyPlan.after, kingsideFirst, queensideFirst, playMoves_append] using
      independent_knight_development_endpoint_eq

/-- The computed example inhabits the general alternating move-order diamond:
the two move/reply plans commute at the initial position. -/
theorem independent_knight_plans_commute :
    ReplyPlansCommuteAt Initial.position kingsidePlan queensidePlan := by
  exact replyPlansCommute_of_independent independent_knight_plans_independent

private def knightThenD4 : List Move := [g1f3, d7d5, d2d4]
private def d4ThenKnight : List Move := [d2d4, d7d5, g1f3]

theorem knightThenD4_legal : lineIsLegal Initial.position knightThenD4 := by
  native_decide

theorem d4ThenKnight_legal : lineIsLegal Initial.position d4ThenKnight := by
  native_decide

/-- A typical opening transposition belongs to the FIDE repetition quotient
even though its complete states differ. -/
theorem knight_d4_repetition_transposition :
    LinesRepetitionTransposeAt Initial.position knightThenD4 d4ThenKnight := by
  exact ⟨knightThenD4_legal, d4ThenKnight_legal, by native_decide⟩

/-- The move orders leave different halfmove clocks and raw en-passant fields,
so exact complete-position equality would reject this genuine opening
transposition. -/
theorem knight_d4_not_exact_transposition :
    ¬sameCompletePosition
      (playMoves Initial.position knightThenD4)
      (playMoves Initial.position d4ThenKnight) := by
  native_decide

theorem knight_d4_clock_difference :
    (playMoves Initial.position knightThenD4).halfmoveClock = 0 ∧
    (playMoves Initial.position d4ThenKnight).halfmoveClock = 1 := by
  native_decide

theorem knight_d4_raw_enPassant_difference :
    (playMoves Initial.position knightThenD4).enPassantTarget = some ⟨3, 2⟩ ∧
    (playMoves Initial.position d4ThenKnight).enPassantTarget = none := by
  native_decide

end OpeningExamples
end Chess.Theory
