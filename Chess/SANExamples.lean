import Chess.FEN
import Chess.Initial
import Chess.SAN

namespace Chess.SANExamples

private def sq (file rank : Coordinate) : Square := Square.ofCoords file rank

private def e2e4 : Move := ⟨sq 4 1, sq 4 3, none⟩
private def g1f3 : Move := ⟨sq 6 0, sq 5 2, none⟩

private def resolvesAs (position : Position) (token : String) (expected : Move) : Bool :=
  match SAN.resolve position token with
  | .ok move => move == expected
  | .error _ => false

private def rendersAs (position : Position) (move : Move) (expected : String) : Bool :=
  match SAN.render position move with
  | .ok token => token == expected
  | .error _ => false

private def atFEN (fen : String) (test : Position → Bool) : Bool :=
  match FEN.parse fen with
  | .ok position => test position
  | .error _ => false

/-! ## Ordinary moves and local round trips -/

/-- SAN is resolved relative to a position, not parsed as a context-free move. -/
theorem initial_san_resolves_moves :
    resolvesAs Initial.position "e4" e2e4 ∧
    resolvesAs Initial.position "Nf3" g1f3 := by
  native_decide

/-- Every legal move in the initial position survives canonical
render-then-resolve.  The corpus validator exercises the same property over a
much larger family of positions. -/
theorem initial_legal_moves_render_resolve :
    (legalMoves Initial.position).all fun move =>
      match SAN.render Initial.position move with
      | .ok token => resolvesAs Initial.position token move
      | .error _ => false := by
  native_decide

/-! ## Castling -/

private def castlingFEN :=
  "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1"

theorem both_white_castles_use_letter_O :
    atFEN castlingFEN fun position =>
      resolvesAs position "O-O" ⟨Square.e1, Square.g1, none⟩ &&
      resolvesAs position "O-O-O" ⟨Square.e1, Square.c1, none⟩ := by
  native_decide

/-- Zeroes are not silently normalized to the PGN letter-O spelling. -/
theorem zero_castling_is_rejected :
    atFEN castlingFEN fun position =>
      match SAN.resolve position "0-0" with
      | .error (.syntax .zeroCastlingNotAccepted) => true
      | _ => false := by
  native_decide

/-! ## Minimal legal-move disambiguation -/

private def fileDisambiguationFEN :=
  "7k/8/8/8/8/2N1N3/8/K7 w - - 0 1"

/-- When two knights can reach d5, the source file is sufficient. -/
theorem file_disambiguation_is_canonical :
    atFEN fileDisambiguationFEN fun position =>
      resolvesAs position "Ncd5" ⟨sq 2 2, sq 3 4, none⟩ &&
      rendersAs position ⟨sq 2 2, sq 3 4, none⟩ "Ncd5" := by
  native_decide

/-- Omitting a necessary hint is ambiguous, while spelling more than the
minimal hint is rejected as noncanonical. -/
theorem disambiguation_errors_are_structured :
    atFEN fileDisambiguationFEN fun position =>
      (match SAN.resolve position "Nd5" with
       | .error (.ambiguousLegalMoves _) => true
       | _ => false) &&
      (match SAN.resolve position "Nc3d5" with
       | .error (.nonCanonical "Ncd5") => true
       | _ => false) := by
  native_decide

private def rankDisambiguationFEN :=
  "7k/8/8/8/8/R7/8/R6K w - - 0 1"

/-- Same-file rooks require the source rank. -/
theorem rank_disambiguation_is_canonical :
    atFEN rankDisambiguationFEN fun position =>
      resolvesAs position "R1a2" ⟨sq 0 0, sq 0 1, none⟩ &&
      resolvesAs position "R3a2" ⟨sq 0 2, sq 0 1, none⟩ := by
  native_decide

private def fullSquareDisambiguationFEN :=
  "7k/8/8/8/N7/8/N3N3/7K w - - 0 1"

/-- With competitors sharing the source file and source rank, SAN needs the
whole source square. -/
theorem full_square_disambiguation_is_canonical :
    atFEN fullSquareDisambiguationFEN fun position =>
      resolvesAs position "Na2c3" ⟨sq 0 1, sq 2 2, none⟩ &&
      rendersAs position ⟨sq 0 1, sq 2 2, none⟩ "Na2c3" := by
  native_decide

/-! ## En passant, promotion, check, and mate -/

private def enPassantFEN :=
  "8/8/8/K1Pp4/8/8/8/7k w - d6 0 1"

/-- En passant is rendered as a capture even though its target is empty. -/
theorem en_passant_renders_as_capture :
    atFEN enPassantFEN fun position =>
      resolvesAs position "cxd6" ⟨sq 2 4, sq 3 5, none⟩ &&
      rendersAs position ⟨sq 2 4, sq 3 5, none⟩ "cxd6" := by
  native_decide

private def promotionFEN :=
  "7k/P7/8/8/8/8/8/7K w - - 0 1"

/-- Promotion notation includes the chosen piece and a semantic check suffix. -/
theorem promotion_and_check_suffix_are_canonical :
    atFEN promotionFEN fun position =>
      resolvesAs position "a8=Q+" ⟨sq 0 6, sq 0 7, some .queen⟩ &&
      resolvesAs position "a8=N" ⟨sq 0 6, sq 0 7, some .knight⟩ := by
  native_decide

private def foolsMatePrefix : List String := ["f3", "e5", "g4"]

private def foolsMateSucceeds : Bool :=
  match SAN.replay Initial.game (foolsMatePrefix ++ ["Qh4#"]) with
  | .error _ => false
  | .ok state =>
      inCheck state.current.board state.current.turn &&
        (legalMoves state.current).isEmpty

/-- The mate marker is computed from the resulting position. -/
theorem fools_mate_has_semantic_mate_suffix : foolsMateSucceeds := by
  native_decide

/-- A check marker cannot be substituted for mate merely because both denote
check: canonical SAN distinguishes the two resulting states. -/
private def foolsMateRejectsPlainCheckSuffix : Bool :=
  match SAN.replay Initial.game foolsMatePrefix with
  | .error _ => false
  | .ok state =>
      match SAN.resolve state.current "Qh4+" with
      | .error (.suffixMismatch .check .mate) => true
      | _ => false

theorem fools_mate_rejects_plain_check_suffix :
    foolsMateRejectsPlainCheckSuffix := by
  native_decide

end Chess.SANExamples
