import Chess.Game
import Chess.Initial

namespace Chess

/-- FIDE Article 3.10.3: a position is legal exactly when some finite sequence
of legal moves from the standard initial position reaches it. -/
def LegallyReachable (position : Position) : Prop :=
  Position.Reachable Initial.position position

theorem initial_legallyReachable : LegallyReachable Initial.position :=
  .refl Initial.position

end Chess
