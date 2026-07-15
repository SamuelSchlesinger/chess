import Chess.Geometry
import Chess.Piece

namespace Chess

/-- The only pieces to which a pawn may promote. -/
inductive PromotionPiece where
  | queen
  | rook
  | bishop
  | knight
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

namespace PromotionPiece

def pieceKind : PromotionPiece → PieceKind
  | queen => .queen
  | rook => .rook
  | bishop => .bishop
  | knight => .knight

end PromotionPiece

/-- A player's choice of source, destination, and (when required) promotion.

Castling and en-passant are inferred from the position. This matches UCI's
compact representation and prevents contradictory move tags. -/
structure Move where
  source : Square
  target : Square
  promotion : Option PromotionPiece := none
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

end Chess
