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

private def kingsideFirst : List Move := [g1f3, g8f6, b1c3, b8c6]
private def queensideFirst : List Move := [b1c3, b8c6, g1f3, g8f6]

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

end OpeningExamples
end Chess.Theory
