import Chess.Initial
import Chess.Replay

namespace Chess.InteropExamples

private def sq (file rank : Coordinate) : Square := Square.ofCoords file rank

private def e2e4 : Move := ⟨sq 4 1, sq 4 3, none⟩
private def a7a8q : Move := ⟨sq 0 6, sq 0 7, some .queen⟩

private def uciParsesAs (text : String) (expected : Move) : Bool :=
  match UCI.parse text with
  | .ok move => move == expected
  | .error _ => false

private def uciFailsAs (text : String) (expected : UCI.ParseError) : Bool :=
  match UCI.parse text with
  | .ok _ => false
  | .error failure => failure == expected

/-! ## UCI interchange -/

/-- Canonical UCI text parses to the expected quiet move. -/
theorem uci_parses_quiet_move : uciParsesAs "e2e4" e2e4 := by
  native_decide

/-- The optional fifth UCI character records the promotion choice. -/
theorem uci_parses_promotion : uciParsesAs "a7a8q" a7a8q := by
  native_decide

/-- Rendering is lowercase and includes the promotion suffix exactly once. -/
theorem uci_renders_promotion : UCI.render a7a8q = "a7a8q" := by
  native_decide

/-- Parse-after-render is executable over the complete finite space of raw
orthodox move values, including choices that are illegal in any given
position. Legality is deliberately a separate replay concern. -/
theorem uci_parse_render_all_raw_moves :
    Move.all.all (fun move => uciParsesAs (UCI.render move) move) := by
  native_decide

/-- UCI errors distinguish malformed length, source, target, and promotion. -/
theorem malformed_uci_errors_are_structured :
    uciFailsAs "e2e" (.wrongLength 3) ∧
    uciFailsAs "i2e4" (.invalidSource 'i' '2') ∧
    uciFailsAs "e2e9" (.invalidTarget 'e' '9') ∧
    uciFailsAs "e7e8Q" (.invalidPromotion 'Q') := by
  native_decide

/-- Protocol null moves are not silently accepted as chess moves. -/
theorem uci_null_move_is_rejected :
    uciFailsAs "0000" (.invalidSource '0' '0') := by
  native_decide

/-! ## Checked FEN interchange -/

private def initialFEN :=
  "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"

private def kiwipeteFEN :=
  "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"

private def legalEnPassantFEN :=
  "8/8/8/K1Pp4/8/8/8/7k w - d6 0 1"

private def pinnedEnPassantFEN :=
  "8/8/8/K1Pp3r/8/8/8/7k w - d6 0 1"

private def renderParsedRaw (fen : String) : Except String String := do
  let position ← FEN.parse fen
  FEN.renderRaw position

private def renderParsedEffective (fen : String) : Except String String := do
  let position ← FEN.parse fen
  FEN.renderEffective position

private def rendersAs (result : Except String String) (expected : String) : Bool :=
  match result with
  | .ok rendered => rendered == expected
  | .error _ => false

private def renderFailsAs (result : Except String String) (expected : String) : Bool :=
  match result with
  | .ok _ => false
  | .error failure => failure == expected

/-- The standard initial position is a checked, canonical FEN round trip. -/
theorem initial_fen_roundtrip : rendersAs (renderParsedRaw initialFEN) initialFEN := by
  native_decide

/-- Kiwipete exercises dense placement and all four castling rights. -/
theorem kiwipete_fen_roundtrip : rendersAs (renderParsedRaw kiwipeteFEN) kiwipeteFEN := by
  native_decide

/-- A legal en-passant opportunity is retained by effective rendering. -/
theorem legal_en_passant_is_effective :
    rendersAs (renderParsedEffective legalEnPassantFEN) legalEnPassantFEN := by
  native_decide

/-- A nominal en-passant square is preserved in raw FEN even when the only
capturing pawn is absolutely pinned. -/
theorem pinned_en_passant_raw_fen_preserves_target :
    rendersAs (renderParsedRaw pinnedEnPassantFEN) pinnedEnPassantFEN := by
  native_decide

/-- Effective FEN removes the pinned en-passant target because no legal
en-passant capture exists. -/
theorem pinned_en_passant_effective_fen_normalizes_target :
    rendersAs (renderParsedEffective pinnedEnPassantFEN)
      "8/8/8/K1Pp3r/8/8/8/7k w - - 0 1" := by
  native_decide

/-- Checked rendering refuses the unconstrained `Position` value that uses
FEN's reserved fullmove number zero. -/
theorem fen_renderer_rejects_zero_fullmove_number :
    renderFailsAs (FEN.renderRaw { Initial.position with fullmoveNumber := 0 })
      "FEN fullmove number must be positive" := by
  native_decide

/-- Checked rendering also rejects raw en-passant targets outside ranks three
and six, even if effective normalization would otherwise erase the target. -/
theorem fen_renderer_rejects_invalid_en_passant_rank :
    renderFailsAs
      (FEN.renderEffective { Initial.position with enPassantTarget := some (sq 4 3) })
      "FEN en-passant target must be on rank 3 or rank 6" := by
  native_decide

private def fenRejected (fen : String) : Bool :=
  match FEN.parse fen with
  | .error _ => true
  | .ok _ => false

/-- Adjacent empty-square digits are rejected rather than accepted as a
noncanonical spelling of a rank. -/
theorem fen_parser_rejects_adjacent_empty_runs :
    fenRejected "44/8/8/8/8/8/8/8 w - - 0 1" := by
  native_decide

/-! ## Checked game replay -/

private def openingLine : List String := ["e2e4", "e7e5", "g1f3"]

private def replayRawFEN (moves : List String) : Option String :=
  match Replay.replayUCI Initial.game moves with
  | .error _ => none
  | .ok state => (FEN.renderRaw state.current).toOption

private def replayEffectiveFEN (moves : List String) : Option String :=
  match Replay.replayUCI Initial.game moves with
  | .error _ => none
  | .ok state => (FEN.renderEffective state.current).toOption

private def replayHistoryFENs (moves : List String) : Option (List String) :=
  match Replay.replayUCI Initial.game moves with
  | .error _ => none
  | .ok state => some (state.prior.map fun position => FEN.renderUnchecked position .raw)

private def replayError (moves : List String) : Option Replay.Error :=
  match Replay.replayUCI Initial.game moves with
  | .error failure => some failure
  | .ok _ => none

/-- A checked three-ply replay reaches the expected complete raw FEN. -/
theorem replay_opening_final_fen :
    replayRawFEN openingLine = some
      "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2" := by
  native_decide

/-- A double pawn step remains in raw FEN but disappears from effective FEN
when no legal en-passant capture exists. -/
theorem replay_raw_and_effective_en_passant_differ :
    replayRawFEN ["e2e4"] = some
      "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1" ∧
    replayEffectiveFEN ["e2e4"] = some
      "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1" := by
  native_decide

/-- Successful replay retains every preceding position newest first. -/
theorem replay_preserves_history_newest_first :
    replayHistoryFENs openingLine = some
      ["rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2",
       "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
       initialFEN] := by
  native_decide

/-- An illegal move reports the first ply and the position in which it failed. -/
theorem replay_reports_illegal_move :
    replayError ["e2e5"] = some
      { ply := 1
        moveText := "e2e5"
        positionText := initialFEN
        reason := .illegalMove } := by
  native_decide

/-- A malformed move after one legal ply reports the second-ply position and
preserves the structured UCI parser error. -/
theorem replay_reports_malformed_move_at_exact_ply :
    replayError ["e2e4", "oops"] = some
      { ply := 2
        moveText := "oops"
        positionText :=
          "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
        reason := .invalidUCI (.invalidSource 'o' 'o') } := by
  native_decide

/-- The replay soundness theorem can be consumed directly by clients: every
successful parse-and-legality check supplies a path in the legal move graph. -/
theorem opening_replay_success_implies_reachability {final : GameState}
    (success : Replay.replayUCI Initial.game openingLine = .ok final) :
    Position.Reachable Initial.position final.current :=
  Replay.reachable_of_replayUCI_eq_ok success

private def knightCycles : List String :=
  ["g1f3", "g8f6", "f3g1", "f6g8",
   "g1f3", "g8f6", "f3g1", "f6g8"]

private def replayRepetitionCount (moves : List String) : Option Nat :=
  match Replay.replayUCI Initial.game moves with
  | .error _ => none
  | .ok state => some (repetitionCount state)

private def replayEndpointRepeatsInitial (moves : List String) : Bool :=
  match Replay.replayUCI Initial.game moves with
  | .error _ => false
  | .ok state => sameForRepetition state.current Initial.position

/-- The legal four-ply knight shuffle returns to the initial repetition class. -/
theorem replayed_knight_cycle_returns_to_initial_class :
    replayEndpointRepeatsInitial (knightCycles.take 4) := by
  native_decide

/-- Two checked knight cycles record the initial position three times: the
initial occurrence plus one endpoint for each cycle. -/
theorem replayed_two_knight_cycles_have_three_occurrences :
    replayRepetitionCount knightCycles = some 3 := by
  native_decide

end Chess.InteropExamples
