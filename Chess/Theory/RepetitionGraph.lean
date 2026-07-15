import Chess.Theory.Repetition

namespace Chess

/-- The proved FIDE repetition equivalence, packaged for Lean's quotient
construction. -/
def repetitionSetoid : Setoid Position where
  r := fun left right => sameForRepetition left right
  iseqv := sameForRepetition_equivalence

/-- A FIDE repetition class of `Position` values, including analysis positions
that need not be reachable from the initial setup. -/
def RepetitionNode := Quotient repetitionSetoid

namespace RepetitionNode

def ofPosition (position : Position) : RepetitionNode :=
  Quotient.mk repetitionSetoid position

theorem ofPosition_eq_iff {left right : Position} :
    ofPosition left = ofPosition right ↔ sameForRepetition left right :=
  ⟨fun equal => Quotient.exact equal,
    fun same => Quotient.sound (s := repetitionSetoid) same⟩

/-- Legality is well-defined on repetition classes. -/
def isLegal (node : RepetitionNode) (move : Move) : Bool :=
  Quotient.lift (fun position => Chess.isLegal position move)
    (fun _left _right same => isLegal_eq_of_sameForRepetition same move) node

/-- Applying a raw move is a well-defined total transformation of repetition
classes. Legal graph edges restrict this transformation with `isLegal`. -/
def after (node : RepetitionNode) (move : Move) : RepetitionNode :=
  Quotient.lift (fun position => ofPosition (applyUnchecked position move))
    (fun _left _right same =>
      Quotient.sound (sameForRepetition_applyUnchecked same move)) node

@[simp] theorem isLegal_ofPosition (position : Position) (move : Move) :
    isLegal (ofPosition position) move = Chess.isLegal position move := rfl

@[simp] theorem after_ofPosition (position : Position) (move : Move) :
    after (ofPosition position) move = ofPosition (applyUnchecked position move) := rfl

def Legal (node : RepetitionNode) (move : Move) : Prop := node.isLegal move

/-- The complete legal move set is therefore independent of the representative
chosen for a repetition node. -/
def legalMoves (node : RepetitionNode) : List Move :=
  Move.all.filter fun move => node.isLegal move

@[simp] theorem legalMoves_ofPosition (position : Position) :
    legalMoves (ofPosition position) = Chess.legalMoves position := rfl

/-- A labelled edge in the repetition quotient, restricted by the legal-move
predicate even when the source is an arbitrary analysis position. -/
def Successor (node next : RepetitionNode) : Prop :=
  ∃ move, node.Legal move ∧ next = node.after move

/-- Finite reachability in the quotient move graph. -/
inductive Reachable : RepetitionNode → RepetitionNode → Prop where
  | refl (node : RepetitionNode) : Reachable node node
  | step {start middle finish : RepetitionNode} :
      Successor start middle → Reachable middle finish → Reachable start finish

theorem successor_of_position {position next : Position}
    (successor : Position.Successor position next) :
    Successor (ofPosition position) (ofPosition next) := by
  rcases successor with ⟨move, legal, rfl⟩
  exact ⟨move, legal, rfl⟩

theorem reachable_of_position {position future : Position}
    (reachable : Position.Reachable position future) :
    Reachable (ofPosition position) (ofPosition future) := by
  induction reachable with
  | refl => exact .refl _
  | step successor _ ih => exact .step (successor_of_position successor) ih

/-- Operational congruence gives path lifting: a quotient path can start from
any concrete representative of its source node. -/
theorem reachable_lift_aux {source target : RepetitionNode}
    (reachable : Reachable source target) :
    ∀ position, ofPosition position = source →
      ∃ future, Position.Reachable position future ∧ ofPosition future = target := by
  induction reachable with
  | refl node =>
      intro position sourceEq
      exact ⟨position, .refl position, sourceEq⟩
  | @step start middle finish successor rest ih =>
      intro position sourceEq
      rw [← sourceEq] at successor
      rcases successor with ⟨move, legal, middleEq⟩
      let next := applyUnchecked position move
      have nextNodeEq : ofPosition next = middle := middleEq.symm
      rcases ih next nextNodeEq with ⟨future, path, targetEq⟩
      exact ⟨future, .step ⟨move, legal, rfl⟩ path, targetEq⟩

theorem reachable_lift (position : Position) {target : RepetitionNode}
    (reachable : Reachable (ofPosition position) target) :
    ∃ future, Position.Reachable position future ∧ ofPosition future = target :=
  reachable_lift_aux reachable position rfl

end RepetitionNode

/-- Dead-position status is behavioral and therefore invariant under FIDE
repetition identity. The proof lifts every quotient continuation to the other
representative and uses checkmate invariance at its endpoint. -/
theorem deadPosition_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) (deadLeft : DeadPosition left) :
    DeadPosition right := by
  intro future rightPath mate
  have quotientPath := RepetitionNode.reachable_of_position rightPath
  have sourceEq : RepetitionNode.ofPosition left = RepetitionNode.ofPosition right :=
    RepetitionNode.ofPosition_eq_iff.mpr same
  rw [← sourceEq] at quotientPath
  rcases RepetitionNode.reachable_lift left quotientPath with
    ⟨leftFuture, leftPath, targetEq⟩
  have futureSame : sameForRepetition leftFuture future :=
    RepetitionNode.ofPosition_eq_iff.mp targetEq
  have leftMate : Checkmate leftFuture :=
    (checkmate_iff_of_sameForRepetition futureSame).mpr mate
  exact deadLeft leftFuture leftPath leftMate

theorem deadPosition_iff_of_sameForRepetition {left right : Position}
    (same : sameForRepetition left right) :
    DeadPosition left ↔ DeadPosition right :=
  ⟨deadPosition_of_sameForRepetition same,
    deadPosition_of_sameForRepetition (sameForRepetition_symm same)⟩

end Chess
