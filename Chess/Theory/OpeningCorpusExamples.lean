import Chess.RepetitionKey
import Chess.Theory.Opening

namespace Chess.Theory.OpeningCorpusExamples

private def move (sourceFile sourceRank targetFile targetRank : Coordinate) : Move :=
  ⟨⟨sourceFile, sourceRank⟩, ⟨targetFile, targetRank⟩, none⟩

private def e2e4 := move 4 1 4 3
private def e7e5 := move 4 6 4 4
private def c7c6 := move 2 6 2 5
private def c2c4 := move 2 1 2 3
private def d2d4 := move 3 1 3 3
private def d7d5 := move 3 6 3 4
private def b1c3 := move 1 0 2 2
private def b1d2 := move 1 0 3 1
private def d5e4 := move 3 4 4 3
private def c3e4 := move 2 2 4 3
private def d2e4 := move 3 1 4 3
private def b8d7 := move 1 7 3 6
private def e2e3 := move 4 1 4 2
private def f2f4 := move 5 1 5 3
private def e5f4 := move 4 4 5 3

/-! ## A transposition that is not a permutation of independent moves -/

private def caroKannNc3 : List Move :=
  [e2e4, c7c6, d2d4, d7d5, b1c3, d5e4, c3e4, b8d7]

private def caroKannNd2 : List Move :=
  [e2e4, c7c6, d2d4, d7d5, b1d2, d5e4, d2e4, b8d7]

theorem caroKannNc3_legal : lineIsLegal Initial.position caroKannNc3 := by
  native_decide

theorem caroKannNd2_legal : lineIsLegal Initial.position caroKannNd2 := by
  native_decide

/-- The Caro-Kann routes `3.Nc3 dxe4 4.Nxe4` and `3.Nd2 dxe4 4.Nxe4`
coalesce at the same `...Nd7` Karpov Variation position. -/
theorem caroKann_knight_routes_same_complete_position :
    sameCompletePosition
      (playMoves Initial.position caroKannNc3)
      (playMoves Initial.position caroKannNd2) := by
  native_decide

theorem caroKann_knight_routes_transpose :
    LinesTransposeAt Initial.position caroKannNc3 caroKannNd2 := by
  exact ⟨caroKannNc3_legal, caroKannNd2_legal,
    eq_of_sameCompletePosition caroKann_knight_routes_same_complete_position⟩

/-- These paths do not merely permute the same raw moves: the knight travels
through different squares.  A transposition theory generated only by swapping
independent moves would therefore miss this ordinary opening phenomenon. -/
theorem caroKann_knight_routes_not_move_permutations :
    ¬caroKannNc3.Perm caroKannNd2 := by
  native_decide

/-! ## A corpus transposition with unequal path lengths -/

private def catalanDirect : List Move :=
  [move 3 1 3 3, move 6 7 5 5, move 2 1 2 3, move 4 6 4 5,
   move 6 0 5 2, move 3 6 3 4, move 6 1 6 2, move 5 7 4 6,
   move 5 0 6 1, move 4 7 6 7, move 4 0 6 0, move 1 7 3 6,
   move 3 0 2 1, move 2 6 2 5, move 2 0 5 3]

private def catalanBogoDetour : List Move :=
  [move 3 1 3 3, move 6 7 5 5, move 2 1 2 3, move 4 6 4 5,
   move 6 0 5 2, move 3 6 3 4, move 6 1 6 2, move 5 7 1 3,
   move 2 0 3 1, move 1 3 4 6, move 5 0 6 1, move 4 7 6 7,
   move 4 0 6 0, move 2 6 2 5, move 3 0 2 1, move 1 7 3 6,
   move 3 1 5 3]

theorem catalan_unequal_lines_legal :
    lineIsLegal Initial.position catalanDirect ∧
      lineIsLegal Initial.position catalanBogoDetour := by
  native_decide

/-- The Bogo-style `...Bb4+ Bd2 ...Be7` detour disappears from the eventual
position.  This is an observed opening transposition with paths of different
lengths, not a same-ply move-order diamond. -/
theorem catalan_bogo_detour_transposes :
    LinesRepetitionTransposeAt
      Initial.position catalanDirect catalanBogoDetour := by
  exact ⟨catalan_unequal_lines_legal.1, catalan_unequal_lines_legal.2,
    by native_decide⟩

theorem catalan_bogo_detour_lengths :
    catalanDirect.length = 15 ∧ catalanBogoDetour.length = 17 := by
  native_decide

theorem catalan_bogo_detour_keys_equal :
    RepetitionKey.ofPosition (playMoves Initial.position catalanDirect) =
      RepetitionKey.ofPosition (playMoves Initial.position catalanBogoDetour) :=
  RepetitionKey.ofPosition_eq_iff.mpr catalan_bogo_detour_transposes.sameNode

/-! ## Why repetition keys normalize en passant, but cannot erase it blindly -/

private def c4ThenE3 : List Move := [c2c4, e7e5, e2e3]
private def e3ThenC4 : List Move := [e2e3, e7e5, c2c4]

theorem ineffective_raw_ep_lines_legal :
    lineIsLegal Initial.position c4ThenE3 ∧
      lineIsLegal Initial.position e3ThenC4 := by
  native_decide

/-- The last double step in the second move order records raw `c3`, but no
black pawn can capture there.  The exact FIDE repetition key merges the two
positions. -/
theorem ineffective_raw_ep_keys_equal :
    RepetitionKey.ofPosition (playMoves Initial.position c4ThenE3) =
      RepetitionKey.ofPosition (playMoves Initial.position e3ThenC4) := by
  rw [RepetitionKey.ofPosition_eq_iff]
  native_decide

theorem ineffective_raw_ep_fields_differ :
    (playMoves Initial.position c4ThenE3).enPassantTarget = none ∧
      (playMoves Initial.position e3ThenC4).enPassantTarget = some ⟨2, 2⟩ := by
  native_decide

private def legalEpAvailable : List Move :=
  [b1c3, e7e5, f2f4, e5f4, e2e4]

private def noEpAvailable : List Move :=
  [e2e4, e7e5, f2f4, e5f4, b1c3]

theorem effective_ep_lines_legal :
    lineIsLegal Initial.position legalEpAvailable ∧
      lineIsLegal Initial.position noEpAvailable := by
  native_decide

/-- These endpoints agree on board, turn, and castling rights. -/
theorem effective_ep_base_fields_equal :
    (playMoves Initial.position legalEpAvailable).board.same
        (playMoves Initial.position noEpAvailable).board ∧
      (playMoves Initial.position legalEpAvailable).turn =
        (playMoves Initial.position noEpAvailable).turn ∧
      (playMoves Initial.position legalEpAvailable).castlingRights =
        (playMoves Initial.position noEpAvailable).castlingRights := by
  native_decide

/-- Nevertheless only the first position permits `...fxe3 e.p.`.  Erasing en
passant unconditionally would merge positions with different legal futures. -/
theorem effective_ep_targets_differ :
    effectiveEnPassantTarget (playMoves Initial.position legalEpAvailable) =
        some ⟨4, 2⟩ ∧
      effectiveEnPassantTarget (playMoves Initial.position noEpAvailable) = none := by
  native_decide

theorem effective_ep_keys_differ :
    RepetitionKey.ofPosition (playMoves Initial.position legalEpAvailable) ≠
      RepetitionKey.ofPosition (playMoves Initial.position noEpAvailable) := by
  intro equal
  have same := RepetitionKey.ofPosition_eq_iff.mp equal
  exact (by native_decide :
    ¬sameForRepetition
      (playMoves Initial.position legalEpAvailable)
      (playMoves Initial.position noEpAvailable)) same

end Chess.Theory.OpeningCorpusExamples
