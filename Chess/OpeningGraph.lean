import Std.Data.HashMap
import Chess.RepetitionKey
import Chess.Replay
import Chess.OpeningCorpus

namespace Chess.OpeningGraph

/-!
# Exact finite graph analysis of the pinned opening corpus

The opening table is a list of named move words, not a sample of played games.
This module therefore counts its prefix trie and the exact image of that trie
under `RepetitionKey.ofPosition`; none of the counts should be read as move
frequency or practical opening strength.

Histories are deduplicated by the canonical UCI rendering of every parsed
move.  The repetition-node grouping consequently uses two independent exact
representations: canonical move words for the domain and the extensional,
lawful hash key from `Chess.RepetitionKey` for the codomain.
-/

/-- Repository-relative corpus path, shared with the row-level validator. -/
def path : String := OpeningCorpus.path

/-- Number of prefix occurrences when every named row contributes its own
root occurrence. -/
def expectedPrefixOccurrences : Nat := 40643

/-- Number of distinct canonical UCI move words, including the empty word. -/
def expectedHistories : Nat := 8646

/-- Number of exact modeled repetition classes reached by those histories. -/
def expectedRepetitionNodes : Nat := 7848

/-- Number of repetition fibres containing at least two move words. -/
def expectedNonSingletonFibres : Nat := 570

/-- Sum of `fibreSize - 1` over all non-singleton fibres. -/
def expectedTranspositionExcess : Nat := 798

def expectedMaximumFibre : Nat := 8

/-- Repetition nodes reached at more than one ply depth. -/
def expectedDepthVaryingNodes : Nat := 3

/-- Distinct four-field position keys before ineffective en-passant targets
are normalized away.  Clocks are deliberately absent from this diagnostic
key, just as they are from `RepetitionKey`. -/
def expectedRawEnPassantKeys : Nat := 7921

def expectedMultiplicityDistribution : List (Nat × Nat) :=
  [(2, 445), (3, 69), (4, 31), (5, 15), (6, 1), (7, 6), (8, 3)]

/-- The same components as `RepetitionKey`, except that the raw en-passant
field is retained even when no legal en-passant capture exists. -/
private structure RawEnPassantKey where
  placement : BoardPlacement
  turn : Color
  castlingRights : CastlingRights
  enPassantTarget : Option Square
  deriving DecidableEq, Repr, BEq, ReflBEq, LawfulBEq, Hashable

private def RawEnPassantKey.ofPosition (position : Position) : RawEnPassantKey where
  placement := BoardPlacement.ofBoard position.board
  turn := position.turn
  castlingRights := position.castlingRights
  enPassantTarget := position.enPassantTarget

/-- Sufficient statistics for one repetition fibre.  Minimum and maximum
depth detect precisely whether ply count varies inside the fibre. -/
private structure Fibre where
  historyCount : Nat
  minimumDepth : Nat
  maximumDepth : Nat
  deriving Repr

private def Fibre.singleton (depth : Nat) : Fibre :=
  { historyCount := 1, minimumDepth := depth, maximumDepth := depth }

private def Fibre.insertDepth (fibre : Fibre) (depth : Nat) : Fibre :=
  { historyCount := fibre.historyCount + 1
    minimumDepth := min fibre.minimumDepth depth
    maximumDepth := max fibre.maximumDepth depth }

private abbrev Histories := Std.HashMap String Nat
private abbrev Fibres := Std.HashMap RepetitionKey Fibre
private abbrev RawKeys := Std.HashMap RawEnPassantKey Unit

private structure ScanState where
  rowCount : Nat
  prefixOccurrences : Nat
  checkedPlies : Nat
  histories : Histories
  fibres : Fibres
  rawKeys : RawKeys
  failures : Array String

private def initialState : ScanState :=
  let position := Initial.position
  let repetitionKey := RepetitionKey.ofPosition position
  let rawKey := RawEnPassantKey.ofPosition position
  { rowCount := 0
    prefixOccurrences := 0
    checkedPlies := 0
    histories :=
      (Std.HashMap.emptyWithCapacity expectedHistories).insert "" 0
    fibres :=
      (Std.HashMap.emptyWithCapacity expectedRepetitionNodes).insert
        repetitionKey (Fibre.singleton 0)
    rawKeys :=
      (Std.HashMap.emptyWithCapacity expectedRawEnPassantKeys).insert rawKey ()
    failures := #[] }

private def location (source : String) (line : Nat) (message : String) : String :=
  s!"{source}:{line}: {message}"

private def addRowFailure (source : String) (line : Nat)
    (state : ScanState) (message : String) : ScanState :=
  { state with failures := state.failures.push (location source line message) }

private def addFileFailure (source : String) (state : ScanState)
    (message : String) : ScanState :=
  { state with failures := state.failures.push s!"{source}: {message}" }

/-- Insert a newly observed canonical move word.  Repeated row prefixes do not
increase any graph cardinality, but do still contribute to
`prefixOccurrences`. -/
private def insertHistory (history : String) (depth : Nat) (position : Position)
    (state : ScanState) : ScanState :=
  match state.histories.get? history with
  | some recordedDepth =>
      if recordedDepth == depth then state
      else
        let message :=
          s!"internal canonical-history depth mismatch for '{history}': " ++
            s!"first saw {recordedDepth}, then {depth}"
        { state with failures := state.failures.push message }
  | none =>
      let repetitionKey := RepetitionKey.ofPosition position
      let fibre := match state.fibres.get? repetitionKey with
        | none => Fibre.singleton depth
        | some existing => existing.insertDepth depth
      let rawKey := RawEnPassantKey.ofPosition position
      { state with
        histories := state.histories.insert history depth
        fibres := state.fibres.insert repetitionKey fibre
        rawKeys := state.rawKeys.insert rawKey () }

private def canonicalExtend (historyPrefix token : String) : String :=
  if historyPrefix.isEmpty then token else historyPrefix ++ " " ++ token

/-- Parse, legally replay, canonicalize, and record every prefix of one UCI
word.  A bad ply stops only its own row; later rows are still analyzed. -/
private def scanMoves (source : String) (line : Nat) :
    Position → String → Nat → List String → ScanState → ScanState
  | _, _, _, [], state => state
  | position, historyPrefix, depth, token :: rest, state =>
      match UCI.parse token with
      | .error error =>
          addRowFailure source line state
            s!"ply {depth + 1}, UCI '{token}': {error}"
      | .ok move =>
          if isLegal position move then
            let next := applyUnchecked position move
            let canonical := UCI.render move
            let history := canonicalExtend historyPrefix canonical
            let nextDepth := depth + 1
            let state := { state with
              prefixOccurrences := state.prefixOccurrences + 1
              checkedPlies := state.checkedPlies + 1 }
            let state := insertHistory history nextDepth next state
            scanMoves source line next history nextDepth rest state
          else
            addRowFailure source line state
              (s!"ply {depth + 1}, UCI '{token}' is illegal in " ++
                FEN.renderUnchecked position .raw)

private def scanRow (source : String) (line : Nat) (text : String)
    (state : ScanState) : ScanState :=
  let state := { state with
    rowCount := state.rowCount + 1
    prefixOccurrences := state.prefixOccurrences + 1 }
  match text.splitOn "\t" with
  | [_, _, _, uci, _] =>
      if uci.isEmpty then
        addRowFailure source line state "UCI field must not be empty"
      else
        let tokens := uci.splitOn " "
        if tokens.any String.isEmpty then
          addRowFailure source line state
            "UCI moves must be separated by one ASCII space"
        else
          scanMoves source line Initial.position "" 0 tokens state
  | fields =>
      addRowFailure source line state
        s!"expected exactly 5 tab-separated fields, got {fields.length}"

private def scanRows (source : String) : Nat → List String → ScanState → ScanState
  | _, [], state => state
  | line, row :: rest, state =>
      scanRows source (line + 1) rest (scanRow source line row state)

private def contentLines (content : String) : List String :=
  if content.isEmpty then
    []
  else
    let lines := content.splitOn "\n"
    if content.endsWith "\n" then lines.dropLast else lines

private def incrementMultiplicity (multiplicity : Nat) :
    List (Nat × Nat) → List (Nat × Nat)
  | [] => [(multiplicity, 1)]
  | (current, count) :: rest =>
      if multiplicity == current then
        (current, count + 1) :: rest
      else if multiplicity < current then
        (multiplicity, 1) :: (current, count) :: rest
      else
        (current, count) :: incrementMultiplicity multiplicity rest

private structure FibreAggregate where
  nonSingletonFibres : Nat := 0
  transpositionExcess : Nat := 0
  maximumFibre : Nat := 0
  depthVaryingNodes : Nat := 0
  multiplicityDistribution : List (Nat × Nat) := []

private def aggregateFibre (aggregate : FibreAggregate)
    (_ : RepetitionKey) (fibre : Fibre) : FibreAggregate :=
  let aggregate :=
    { aggregate with
      maximumFibre := max aggregate.maximumFibre fibre.historyCount
      depthVaryingNodes := aggregate.depthVaryingNodes +
        if fibre.minimumDepth == fibre.maximumDepth then 0 else 1 }
  if fibre.historyCount ≤ 1 then
    aggregate
  else
    { aggregate with
      nonSingletonFibres := aggregate.nonSingletonFibres + 1
      transpositionExcess := aggregate.transpositionExcess + fibre.historyCount - 1
      multiplicityDistribution :=
        incrementMultiplicity fibre.historyCount aggregate.multiplicityDistribution }

/-- Exact finite-graph statistics returned by the analyzer. -/
structure Summary where
  rowCount : Nat
  prefixOccurrences : Nat
  checkedPlies : Nat
  uniqueHistories : Nat
  repetitionNodes : Nat
  rawEnPassantKeys : Nat
  nonSingletonFibres : Nat
  transpositionExcess : Nat
  maximumFibre : Nat
  multiplicityDistribution : List (Nat × Nat)
  depthVaryingNodes : Nat
  deriving Repr, DecidableEq

def Summary.describe (summary : Summary) : String :=
  s!"{summary.prefixOccurrences} prefix occurrences, " ++
  s!"{summary.uniqueHistories} canonical histories, " ++
  s!"{summary.repetitionNodes} repetition nodes, " ++
  s!"{summary.nonSingletonFibres} non-singleton fibres, " ++
  s!"excess {summary.transpositionExcess}, maximum {summary.maximumFibre}, " ++
  s!"{summary.depthVaryingNodes} depth-varying nodes, " ++
  s!"{summary.rawEnPassantKeys} raw-EP keys"

/-- The report accumulates row-level replay failures and every aggregate
mismatch instead of failing fast. -/
structure Report where
  source : String
  summary : Summary
  failures : List String
  deriving Repr

def Report.isValid (report : Report) : Bool := report.failures.isEmpty

private def requireNat (source label : String) (expected actual : Nat)
    (failures : Array String) : Array String :=
  if actual == expected then failures
  else failures.push s!"{source}: expected {expected} {label}, got {actual}"

private def requireDistribution (source : String)
    (actual : List (Nat × Nat)) (failures : Array String) : Array String :=
  if actual == expectedMultiplicityDistribution then failures
  else failures.push
    (s!"{source}: expected fibre multiplicities " ++
      reprStr expectedMultiplicityDistribution ++ ", got " ++ reprStr actual)

/-- Analyze already-loaded bytes.  This pure entry point also makes malformed
fixture tests possible without touching the pinned corpus. -/
def analyzeContent (source content : String) : Report :=
  let lines := contentLines content
  let (actualHeader, rows) := match lines with
    | [] => (none, [])
    | first :: rest => (some first, rest)
  let state := match actualHeader with
    | none => addFileFailure source initialState "missing header"
    | some actual =>
        if actual == OpeningCorpus.header then initialState
        else addRowFailure source 1 initialState
          s!"unexpected header; expected '{OpeningCorpus.header}'"
  let state := scanRows source 2 rows state
  let aggregate := state.fibres.fold aggregateFibre {}
  let summary : Summary :=
    { rowCount := state.rowCount
      prefixOccurrences := state.prefixOccurrences
      checkedPlies := state.checkedPlies
      uniqueHistories := state.histories.size
      repetitionNodes := state.fibres.size
      rawEnPassantKeys := state.rawKeys.size
      nonSingletonFibres := aggregate.nonSingletonFibres
      transpositionExcess := aggregate.transpositionExcess
      maximumFibre := aggregate.maximumFibre
      multiplicityDistribution := aggregate.multiplicityDistribution
      depthVaryingNodes := aggregate.depthVaryingNodes }
  let failures := requireNat source "rows"
    OpeningCorpus.expectedRows summary.rowCount state.failures
  let failures := requireNat source "checked plies"
    OpeningCorpus.expectedPlies summary.checkedPlies failures
  let failures := requireNat source "prefix occurrences"
    expectedPrefixOccurrences summary.prefixOccurrences failures
  let failures := requireNat source "canonical histories"
    expectedHistories summary.uniqueHistories failures
  let failures := requireNat source "repetition nodes"
    expectedRepetitionNodes summary.repetitionNodes failures
  let failures := requireNat source "raw-en-passant keys"
    expectedRawEnPassantKeys summary.rawEnPassantKeys failures
  let failures := requireNat source "non-singleton fibres"
    expectedNonSingletonFibres summary.nonSingletonFibres failures
  let failures := requireNat source "transposition excess"
    expectedTranspositionExcess summary.transpositionExcess failures
  let failures := requireNat source "maximum fibre size"
    expectedMaximumFibre summary.maximumFibre failures
  let failures := requireNat source "depth-varying repetition nodes"
    expectedDepthVaryingNodes summary.depthVaryingNodes failures
  let failures := requireDistribution source summary.multiplicityDistribution failures
  { source, summary, failures := failures.toList }

/-- Read and analyze any table with the pinned schema. -/
def analyzeFile (source : String := path) : IO Report := do
  try
    let content ← IO.FS.readFile source
    pure (analyzeContent source content)
  catch error =>
    pure
      { source
        summary := ⟨0, 0, 0, 0, 0, 0, 0, 0, 0, [], 0⟩
        failures := [s!"{source}: could not read file: {error}"] }

/-- Analyze the repository's pinned Lichess opening table. -/
def analyze : IO Report := analyzeFile path

/-- Standalone driver suitable for a Lake executable root or a small runner. -/
def main : IO Unit := do
  let report ← analyze
  if report.isValid then
    IO.println s!"analyzed {report.source}: {report.summary.describe}"
  else
    for failure in report.failures do
      IO.eprintln s!"FAIL: {failure}"
    IO.eprintln s!"opening-graph analysis failed with {report.failures.length} error(s)"
    IO.Process.exit 1

end Chess.OpeningGraph
