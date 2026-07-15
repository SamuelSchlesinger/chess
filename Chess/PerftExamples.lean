import Chess.FEN
import Chess.Rules

namespace Chess.PerftExamples

private def perftOfFEN (depth : Nat) (fen : String) : Option Nat :=
  match FEN.parse fen with
  | .ok position => some (perft depth position)
  | .error _ => none

private def kiwipete :=
  "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"

/-- Kiwipete stresses castling, pins, and sliding-piece obstructions. -/
theorem kiwipete_perft_one : perftOfFEN 1 kiwipete = some 48 := by native_decide

/-- The second Kiwipete level covers 2,039 legal move sequences. -/
theorem kiwipete_perft_two : perftOfFEN 2 kiwipete = some 2039 := by native_decide

private def perftPositionThree :=
  "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"

/-- A standard suite position stressing en-passant and discovered checks. -/
theorem position_three_perft_two :
    perftOfFEN 2 perftPositionThree = some 191 := by native_decide

private def perftPositionFour :=
  "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1"

/-- A standard suite position stressing promotion, checks, and castling rights. -/
theorem position_four_perft_two :
    perftOfFEN 2 perftPositionFour = some 264 := by native_decide

end Chess.PerftExamples
