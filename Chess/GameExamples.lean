import Chess.FEN
import Chess.Game
import Chess.Initial

namespace Chess.GameExamples

private def sq (file rank : Coordinate) : Square := Square.ofCoords file rank

private def g1f3 : Move := ⟨sq 6 0, sq 5 2, none⟩
private def g8f6 : Move := ⟨sq 6 7, sq 5 5, none⟩
private def f3g1 : Move := ⟨sq 5 2, sq 6 0, none⟩
private def f6g8 : Move := ⟨sq 5 5, sq 6 7, none⟩

private def oneKnightCycle (state : GameState) : GameState :=
  state.afterMove g1f3 |>.afterMove g8f6 |>.afterMove f3g1 |>.afterMove f6g8

private def twoKnightCycles : GameState := oneKnightCycle (oneKnightCycle Initial.game)

/-- A returned position has the same repetition identity despite changed clocks. -/
theorem knight_cycle_returns_repetition_position :
    sameForRepetition (oneKnightCycle Initial.game).current Initial.position := by native_decide

/-- The initial occurrence plus two knight cycles produces the claimable third occurrence. -/
theorem two_knight_cycles_are_threefold : ThreefoldRepetition twoKnightCycles := by native_decide

/-- Move clocks are deliberately excluded from repetition identity. -/
theorem clocks_do_not_affect_repetition_identity :
    sameForRepetition Initial.position { Initial.position with halfmoveClock := 73 } := by
  native_decide

/-- Castling rights remain significant even when every piece occupies the same square. -/
theorem castling_rights_affect_repetition_identity :
    ¬sameForRepetition Initial.position { Initial.position with castlingRights := .none } := by
  native_decide

private def pinnedEnPassant? : Option Position :=
  (FEN.parse "8/8/8/K1Pp3r/8/8/8/7k w - d6 0 1").toOption

private def pinnedEnPassantWithoutRight? : Option Position :=
  (FEN.parse "8/8/8/K1Pp3r/8/8/8/7k w - - 0 1").toOption

private def pinnedEnPassant : Position :=
  pinnedEnPassant?.get (by native_decide)

private def pinnedEnPassantWithoutRight : Position :=
  pinnedEnPassantWithoutRight?.get (by native_decide)

/-- A nominal en-passant target does not distinguish repetitions when the only
capturing pawn is absolutely pinned and therefore cannot legally capture. -/
theorem illegal_en_passant_does_not_affect_repetition :
    sameForRepetition pinnedEnPassant pinnedEnPassantWithoutRight := by native_decide

private def legalEnPassant? : Option Position :=
  (FEN.parse "8/8/8/K1Pp4/8/8/8/7k w - d6 0 1").toOption

private def legalEnPassantWithoutRight? : Option Position :=
  (FEN.parse "8/8/8/K1Pp4/8/8/8/7k w - - 0 1").toOption

private def legalEnPassant : Position :=
  legalEnPassant?.get (by native_decide)

private def legalEnPassantWithoutRight : Position :=
  legalEnPassantWithoutRight?.get (by native_decide)

/-- A genuinely legal en-passant capture does distinguish repetition identity. -/
theorem legal_en_passant_affects_repetition :
    ¬sameForRepetition legalEnPassant legalEnPassantWithoutRight := by native_decide

private def fourPreviousOccurrences : GameState where
  current := Initial.position
  prior := [Initial.position, Initial.position, Initial.position, Initial.position]

theorem five_occurrences_trigger_automatic_threshold :
    FivefoldRepetition fourPreviousOccurrences := by native_decide

private def ninetyNineHalfmoves : GameState where
  current := { Initial.position with halfmoveClock := 99 }
  prior := []

/-- A player may indicate a legal quiet move that will complete 100 halfmoves. -/
theorem fifty_move_claim_can_be_announced_before_move :
    DrawClaimAvailableAfter ninetyNineHalfmoves g1f3 := by native_decide

end Chess.GameExamples
