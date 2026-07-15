import Chess.FEN
import Chess.UCI

namespace Chess.Replay

/-! Checked replay validates each newly supplied move against the current
position and retains the resulting history. It intentionally does not certify
that a caller-provided starting history is internally consistent, enforce
automatic game termination, or justify a recorded conclusion; those are
separate record-level validity obligations. -/

inductive ErrorReason where
  | invalidUCI (error : UCI.ParseError)
  | illegalMove
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq

/-- A replay failure with enough stable, chess-facing context to diagnose a
corpus row. `ply` is one-based and relative to the supplied move list.
`positionText` is unchecked FEN-shaped diagnostic text because an arbitrary
analysis-state input need not itself be representable by standard FEN. -/
structure Error where
  ply : Nat
  moveText : String
  positionText : String
  reason : ErrorReason
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq

instance : ToString ErrorReason where
  toString
    | .invalidUCI error => s!"invalid UCI: {error}"
    | .illegalMove => "illegal move"

instance : ToString Error where
  toString failure :=
    s!"ply {failure.ply}, move {failure.moveText}: {failure.reason}; position {failure.positionText}"

private def failure (state : GameState) (ply : Nat) (moveText : String)
    (reason : ErrorReason) : Error :=
  { ply, moveText, positionText := FEN.renderUnchecked state.current .raw, reason }

private def replayMovesFrom : GameState → Nat → List Move → Except Error GameState
  | state, _, [] => .ok state
  | state, ply, move :: rest =>
      if isLegal state.current move then
        replayMovesFrom (state.afterMove move) (ply + 1) rest
      else
        .error (failure state ply (UCI.render move) .illegalMove)

/-- Replay already-parsed moves, rejecting the first illegal ply while
preserving the full `GameState` history on success. -/
def replayMoves (state : GameState) (moves : List Move) : Except Error GameState :=
  replayMovesFrom state 1 moves

private theorem reachable_of_replayMovesFrom_eq_ok {state final : GameState}
    {ply : Nat} {moves : List Move}
    (success : replayMovesFrom state ply moves = .ok final) :
    Position.Reachable state.current final.current := by
  induction moves generalizing state ply with
  | nil =>
      simp [replayMovesFrom] at success
      subst final
      exact .refl _
  | cons move rest ih =>
      by_cases legal : isLegal state.current move
      · simp only [replayMovesFrom, legal, if_true] at success
        exact .step ⟨move, legal, rfl⟩ (ih success)
      · simp [replayMovesFrom, legal] at success

/-- Every successfully checked move replay is a path in the legal position
graph, regardless of whether the supplied starting state itself came from the
standard initial position. -/
theorem reachable_of_replayMoves_eq_ok {state final : GameState} {moves : List Move}
    (success : replayMoves state moves = .ok final) :
    Position.Reachable state.current final.current :=
  reachable_of_replayMovesFrom_eq_ok success

private def replayUCIFrom : GameState → Nat → List String → Except Error GameState
  | state, _, [] => .ok state
  | state, ply, moveText :: rest =>
      match UCI.parse moveText with
      | .error message => .error (failure state ply moveText (.invalidUCI message))
      | .ok move =>
          if isLegal state.current move then
            replayUCIFrom (state.afterMove move) (ply + 1) rest
          else
            .error (failure state ply moveText .illegalMove)

/-- Parse and replay UCI move text, reporting the failing ply, move text,
unchecked FEN-shaped position text, and whether parsing or legality failed. -/
def replayUCI (state : GameState) (moves : List String) : Except Error GameState :=
  replayUCIFrom state 1 moves

private theorem reachable_of_replayUCIFrom_eq_ok {state final : GameState}
    {ply : Nat} {moves : List String}
    (success : replayUCIFrom state ply moves = .ok final) :
    Position.Reachable state.current final.current := by
  induction moves generalizing state ply with
  | nil =>
      simp [replayUCIFrom] at success
      subst final
      exact .refl _
  | cons moveText rest ih =>
      cases parsed : UCI.parse moveText with
      | error parseError =>
          simp [replayUCIFrom, parsed] at success
      | ok move =>
          by_cases legal : isLegal state.current move
          · simp only [replayUCIFrom, parsed, legal, if_true] at success
            exact .step ⟨move, legal, rfl⟩ (ih success)
          · simp [replayUCIFrom, parsed, legal] at success

/-- Parsing UCI text adds no unchecked edges: successful text replay also
produces a path in the legal position graph. -/
theorem reachable_of_replayUCI_eq_ok {state final : GameState} {moves : List String}
    (success : replayUCI state moves = .ok final) :
    Position.Reachable state.current final.current :=
  reachable_of_replayUCIFrom_eq_ok success

end Chess.Replay
