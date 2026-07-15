import Chess.Initial
import Chess.Game
import Chess.Theory.RepetitionGraph

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

/-- Repetition-equivalent positions admit exactly the same legal move words. -/
theorem lineIsLegal_eq_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) (moves : List Move) :
    lineIsLegal left moves = lineIsLegal right moves := by
  induction moves generalizing left right with
  | nil => rfl
  | cons move rest ih =>
      simp only [lineIsLegal]
      rw [isLegal_eq_of_sameForRepetition same move]
      exact congrArg (fun tail => isLegal right move && tail)
        (ih (sameForRepetition_applyUnchecked same move))

/-- Unchecked execution of any move word respects FIDE repetition identity. -/
theorem playMoves_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) (moves : List Move) :
    sameForRepetition (playMoves left moves) (playMoves right moves) := by
  induction moves generalizing left right with
  | nil => exact same
  | cons move rest ih =>
      exact ih (sameForRepetition_applyUnchecked same move)

end Chess.Theory

namespace Chess.RepetitionNode

/-- The free monoid of raw move words acts on repetition nodes. -/
def playMoves (node : RepetitionNode) (moves : List Move) : RepetitionNode :=
  Quotient.lift
    (fun position => ofPosition (Theory.playMoves position moves))
    (fun _left _right same =>
      Quotient.sound (Theory.playMoves_sameForRepetition same moves)) node

/-- Word legality is well-defined on a repetition node. -/
def lineIsLegal (node : RepetitionNode) (moves : List Move) : Bool :=
  Quotient.lift (fun position => Theory.lineIsLegal position moves)
    (fun _left _right same => Theory.lineIsLegal_eq_of_sameForRepetition same moves) node

@[simp] theorem playMoves_ofPosition (position : Position) (moves : List Move) :
    playMoves (ofPosition position) moves =
      ofPosition (Theory.playMoves position moves) := rfl

@[simp] theorem lineIsLegal_ofPosition (position : Position) (moves : List Move) :
    lineIsLegal (ofPosition position) moves =
      Theory.lineIsLegal position moves := rfl

theorem playMoves_append (node : RepetitionNode) (initialMoves suffix : List Move) :
    playMoves node (initialMoves ++ suffix) =
      playMoves (playMoves node initialMoves) suffix := by
  induction node using Quotient.inductionOn with
  | _ position =>
      exact congrArg ofPosition (Theory.playMoves_append position initialMoves suffix)

end Chess.RepetitionNode

namespace Chess.Theory

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

/-- Repetition transposition is preserved when both lines follow a common
legal prefix. -/
theorem linesRepetitionTransposeAt_prepend {position : Position}
    {left right initialMoves : List Move}
    (initialLegal : lineIsLegal position initialMoves)
    (transpose : LinesRepetitionTransposeAt
      (playMoves position initialMoves) left right) :
    LinesRepetitionTransposeAt position
      (initialMoves ++ left) (initialMoves ++ right) := by
  refine ⟨(lineIsLegal_append position initialMoves left).mpr
      ⟨initialLegal, transpose.leftLegal⟩,
    (lineIsLegal_append position initialMoves right).mpr
      ⟨initialLegal, transpose.rightLegal⟩, ?_⟩
  simpa [playMoves_append] using transpose.sameNode

/-- Transposed prefixes have identical residual languages of legal move
words. -/
theorem linesRepetitionTransposeAt_continuation_legal_iff
    {position : Position} {left right : List Move}
    (transpose : LinesRepetitionTransposeAt position left right)
    (suffix : List Move) :
    lineIsLegal (playMoves position left) suffix ↔
      lineIsLegal (playMoves position right) suffix := by
  rw [← Bool.eq_iff_iff]
  exact lineIsLegal_eq_of_sameForRepetition transpose.sameNode suffix

/-- Equivalently, every common extension is legal on both sides or neither. -/
theorem linesRepetitionTransposeAt_extension_legal_iff
    {position : Position} {left right : List Move}
    (transpose : LinesRepetitionTransposeAt position left right)
    (suffix : List Move) :
    lineIsLegal position (left ++ suffix) ↔
      lineIsLegal position (right ++ suffix) := by
  rw [lineIsLegal_append, lineIsLegal_append]
  constructor
  · rintro ⟨_, suffixLegal⟩
    exact ⟨transpose.rightLegal,
      (linesRepetitionTransposeAt_continuation_legal_iff transpose suffix).mp suffixLegal⟩
  · rintro ⟨_, suffixLegal⟩
    exact ⟨transpose.leftLegal,
      (linesRepetitionTransposeAt_continuation_legal_iff transpose suffix).mpr suffixLegal⟩

/-- Right whiskering: every legal common continuation preserves a repetition
transposition. -/
theorem linesRepetitionTransposeAt_append {position : Position}
    {left right suffix : List Move}
    (transpose : LinesRepetitionTransposeAt position left right)
    (suffixLegal : lineIsLegal (playMoves position left) suffix) :
    LinesRepetitionTransposeAt position
      (left ++ suffix) (right ++ suffix) := by
  have suffixLegalRight : lineIsLegal (playMoves position right) suffix :=
    (linesRepetitionTransposeAt_continuation_legal_iff transpose suffix).mp suffixLegal
  refine ⟨(lineIsLegal_append position left suffix).mpr
      ⟨transpose.leftLegal, suffixLegal⟩,
    (lineIsLegal_append position right suffix).mpr
      ⟨transpose.rightLegal, suffixLegalRight⟩, ?_⟩
  rw [playMoves_append, playMoves_append]
  exact playMoves_sameForRepetition transpose.sameNode suffix

/-- Composition of move-order diamonds: quotient-equivalent prefixes may be
continued by quotient-equivalent suffix lines. -/
theorem linesRepetitionTransposeAt_compose {position : Position}
    {left right leftSuffix rightSuffix : List Move}
    (prefixTranspose : LinesRepetitionTransposeAt position left right)
    (suffixTranspose : LinesRepetitionTransposeAt
      (playMoves position left) leftSuffix rightSuffix) :
    LinesRepetitionTransposeAt position
      (left ++ leftSuffix) (right ++ rightSuffix) := by
  have rightSuffixLegal : lineIsLegal (playMoves position right) rightSuffix :=
    (linesRepetitionTransposeAt_continuation_legal_iff
      prefixTranspose rightSuffix).mp suffixTranspose.rightLegal
  refine ⟨(lineIsLegal_append position left leftSuffix).mpr
      ⟨prefixTranspose.leftLegal, suffixTranspose.leftLegal⟩,
    (lineIsLegal_append position right rightSuffix).mpr
      ⟨prefixTranspose.rightLegal, rightSuffixLegal⟩, ?_⟩
  rw [playMoves_append, playMoves_append]
  exact sameForRepetition_trans suffixTranspose.sameNode
    (playMoves_sameForRepetition prefixTranspose.sameNode rightSuffix)

/-- A labelled legal path whose target is specified only up to FIDE
repetition identity. -/
def RepetitionTrace (start : Position) (moves : List Move) (finish : Position) : Prop :=
  lineIsLegal start moves ∧ sameForRepetition (playMoves start moves) finish

theorem repetitionTrace_nil_iff (start finish : Position) :
    RepetitionTrace start [] finish ↔ sameForRepetition start finish := by
  simp [RepetitionTrace, lineIsLegal, playMoves]

/-- Trace concatenation factors through an intermediate repetition class. -/
theorem repetitionTrace_append_iff (start finish : Position)
    (initialMoves suffix : List Move) :
    RepetitionTrace start (initialMoves ++ suffix) finish ↔
      ∃ middle, RepetitionTrace start initialMoves middle ∧
        RepetitionTrace middle suffix finish := by
  constructor
  · rintro ⟨legal, same⟩
    have legalParts := (lineIsLegal_append start initialMoves suffix).mp legal
    refine ⟨playMoves start initialMoves,
      ⟨legalParts.1, sameForRepetition_self _⟩,
      ⟨legalParts.2, ?_⟩⟩
    simpa [playMoves_append] using same
  · rintro ⟨middle, ⟨initialLegal, initialSame⟩, ⟨suffixLegal, suffixSame⟩⟩
    have suffixLegalAtEndpoint :
        lineIsLegal (playMoves start initialMoves) suffix := by
      rw [lineIsLegal_eq_of_sameForRepetition initialSame suffix]
      exact suffixLegal
    refine ⟨(lineIsLegal_append start initialMoves suffix).mpr
      ⟨initialLegal, suffixLegalAtEndpoint⟩, ?_⟩
    rw [playMoves_append]
    exact sameForRepetition_trans
      (playMoves_sameForRepetition initialSame suffix) suffixSame

theorem repetitionTrace_target_unique {start first second : Position}
    {moves : List Move}
    (firstTrace : RepetitionTrace start moves first)
    (secondTrace : RepetitionTrace start moves second) :
    sameForRepetition first second :=
  sameForRepetition_trans (sameForRepetition_symm firstTrace.2) secondTrace.2

theorem linesRepetitionTransposeAt_iff_exists_commonTarget
    (position : Position) (left right : List Move) :
    LinesRepetitionTransposeAt position left right ↔
      ∃ finish, RepetitionTrace position left finish ∧
        RepetitionTrace position right finish := by
  constructor
  · intro transpose
    exact ⟨playMoves position left,
      ⟨transpose.leftLegal, sameForRepetition_self _⟩,
      ⟨transpose.rightLegal, sameForRepetition_symm transpose.sameNode⟩⟩
  · rintro ⟨finish, leftTrace, rightTrace⟩
    exact ⟨leftTrace.1, rightTrace.1,
      sameForRepetition_trans leftTrace.2 (sameForRepetition_symm rightTrace.2)⟩

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

/-- Despite their different clocks and raw en-passant fields, the two move
orders admit exactly the same legal continuations. -/
theorem knight_d4_same_residual_language (continuation : List Move) :
    lineIsLegal (playMoves Initial.position knightThenD4) continuation ↔
      lineIsLegal (playMoves Initial.position d4ThenKnight) continuation :=
  linesRepetitionTransposeAt_continuation_legal_iff
    knight_d4_repetition_transposition continuation

/-- Every finite-depth exhaustive search also sees the same number of leaves
from the two endpoints. -/
theorem knight_d4_same_perft (depth : Nat) :
    perft depth (playMoves Initial.position knightThenD4) =
      perft depth (playMoves Initial.position d4ThenKnight) :=
  perft_eq_of_sameForRepetition depth knight_d4_repetition_transposition.sameNode

/-- For example, appending `...Nf6` preserves the transposition. -/
theorem knight_d4_then_nf6_repetition_transposition :
    LinesRepetitionTransposeAt Initial.position
      (knightThenD4 ++ [g8f6]) (d4ThenKnight ++ [g8f6]) := by
  apply linesRepetitionTransposeAt_append knight_d4_repetition_transposition
  native_decide

end OpeningExamples
end Chess.Theory
