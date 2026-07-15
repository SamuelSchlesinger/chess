namespace Chess

/-- The two players in orthodox chess. -/
inductive Color where
  | white
  | black
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

namespace Color

/-- The opponent of a player. -/
def other : Color → Color
  | white => black
  | black => white

@[simp] theorem other_other (color : Color) : color.other.other = color := by
  cases color <;> rfl

@[simp] theorem other_ne (color : Color) : color.other ≠ color := by
  cases color <;> decide

end Color

/-- The role of a chess piece, independent of its color. -/
inductive PieceKind where
  | king
  | queen
  | rook
  | bishop
  | knight
  | pawn
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

/-- A colored chess piece. -/
structure Piece where
  color : Color
  kind : PieceKind
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

end Chess
