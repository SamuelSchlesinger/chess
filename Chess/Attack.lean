import Chess.Board

namespace Chess

namespace Square

/-- Squares visible along a ray, including the first occupied square and
stopping immediately afterward. The fuel bound is semantic: no ray on an
8-by-8 board can contain more than seven destination squares. -/
def rayAttacksFrom (board : Board) (direction : Direction) : Nat → Square → List Square
  | 0, _ => []
  | fuel + 1, source =>
      match source.offset direction with
      | none => []
      | some target =>
          target :: match board.pieceAt target with
            | some _ => []
            | none => rayAttacksFrom board direction fuel target

/-- The attacked squares in one sliding direction. -/
def rayAttacks (board : Board) (source : Square) (direction : Direction) : List Square :=
  rayAttacksFrom board direction 7 source

theorem rayAttacksFrom_length_le (board : Board) (direction : Direction)
    (fuel : Nat) (source : Square) :
    (rayAttacksFrom board direction fuel source).length ≤ fuel := by
  induction fuel generalizing source with
  | zero => simp [rayAttacksFrom]
  | succ fuel ih =>
    simp only [rayAttacksFrom]
    split
    · simp
    · rename_i target htarget
      split
      · simp
      · simp only [List.length_cons]
        exact Nat.succ_le_succ (ih target)

/-- An adjacent blocker is attacked, but every square behind it is not. -/
theorem rayAttacks_of_adjacent_blocker (board : Board) (source target : Square)
    (direction : Direction) (step : source.offset direction = some target)
    (blocker : board.pieceAt target ≠ none) :
    rayAttacks board source direction = [target] := by
  simp only [rayAttacks, rayAttacksFrom, step]
  cases occupied : board.pieceAt target with
  | none => exact (blocker occupied).elim
  | some piece => rfl

end Square

private def leapTargets (source : Square) (directions : List Direction) : List Square :=
  directions.filterMap source.offset

private def slidingTargets (board : Board) (source : Square) (directions : List Direction) :
    List Square :=
  directions.flatMap (source.rayAttacks board)

private def whitePawnDirections : List Direction := [⟨-1, 1⟩, ⟨1, 1⟩]
private def blackPawnDirections : List Direction := [⟨-1, -1⟩, ⟨1, -1⟩]
private def kingDirections : List Direction := Direction.queen
private def knightDirections : List Direction :=
  [⟨1, 2⟩, ⟨2, 1⟩, ⟨2, -1⟩, ⟨1, -2⟩,
   ⟨-1, -2⟩, ⟨-2, -1⟩, ⟨-2, 1⟩, ⟨-1, 2⟩]

/-- Squares attacked by a piece on `source`, according to FIDE's attack notion.

In particular, this ignores whether moving the piece would expose its own king.
Sliding pieces include the first occupied square on each ray. Pawns include only
their diagonal attack squares, never their forward movement squares. -/
def attacksFrom (board : Board) (source : Square) (piece : Piece) : List Square :=
  match piece.kind with
  | .pawn => leapTargets source <| match piece.color with
      | .white => whitePawnDirections
      | .black => blackPawnDirections
  | .knight => leapTargets source knightDirections
  | .bishop => slidingTargets board source Direction.diagonal
  | .rook => slidingTargets board source Direction.orthogonal
  | .queen => slidingTargets board source Direction.queen
  | .king => leapTargets source kingDirections

theorem whiteKing_e1_not_attack_g1 (board : Board) :
    Square.g1 ∉ attacksFrom board Square.e1 ⟨.white, .king⟩ := by
  change Square.g1 ∉ leapTargets Square.e1 kingDirections
  native_decide

theorem whiteKing_e1_not_attack_c1 (board : Board) :
    Square.c1 ∉ attacksFrom board Square.e1 ⟨.white, .king⟩ := by
  change Square.c1 ∉ leapTargets Square.e1 kingDirections
  native_decide

theorem blackKing_e8_not_attack_g8 (board : Board) :
    Square.g8 ∉ attacksFrom board Square.e8 ⟨.black, .king⟩ := by
  change Square.g8 ∉ leapTargets Square.e8 kingDirections
  native_decide

theorem blackKing_e8_not_attack_c8 (board : Board) :
    Square.c8 ∉ attacksFrom board Square.e8 ⟨.black, .king⟩ := by
  change Square.c8 ∉ leapTargets Square.e8 kingDirections
  native_decide

/-- A structural, proof-facing statement that a particular piece attacks a square. -/
def PieceAttacks (board : Board) (source : Square) (piece : Piece) (target : Square) : Prop :=
  target ∈ attacksFrom board source piece

instance (board : Board) (source : Square) (piece : Piece) (target : Square) :
    Decidable (PieceAttacks board source piece target) := by
  unfold PieceAttacks
  infer_instance

/-- Some piece of `color` attacks `target`. Pinned pieces still count. -/
def AttackedBy (board : Board) (color : Color) (target : Square) : Prop :=
  ∃ source piece, board.pieceAt source = some piece ∧ piece.color = color ∧
    PieceAttacks board source piece target

/-- Executable attack detection, with completeness justified by `Square.mem_all`. -/
def attackedBy (board : Board) (color : Color) (target : Square) : Bool :=
  Square.all.any fun source =>
    match board.pieceAt source with
    | some piece => piece.color == color && (attacksFrom board source piece).contains target
    | none => false

/-- Executable attack detection and the proof-facing attack relation coincide. -/
theorem attackedBy_iff (board : Board) (color : Color) (target : Square) :
    attackedBy board color target ↔ AttackedBy board color target := by
  simp only [attackedBy, AttackedBy, PieceAttacks, List.any_eq_true,
    Square.mem_all, true_and]
  constructor
  · rintro ⟨source, h⟩
    cases occupied : board.pieceAt source with
    | none => simp [occupied] at h
    | some piece =>
      have facts : piece.color = color ∧ target ∈ attacksFrom board source piece := by
        simpa [occupied, List.contains_eq_mem] using h
      exact ⟨source, piece, occupied, facts.1, facts.2⟩
  · rintro ⟨source, piece, occupied, colorEq, attacks⟩
    refine ⟨source, ?_⟩
    simp [occupied, colorEq, List.contains_eq_mem, attacks]

end Chess
