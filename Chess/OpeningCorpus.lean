import Std.Data.HashMap
import Chess.Initial
import Chess.Replay
import Chess.SAN

namespace Chess.OpeningCorpus

/-!
# Validation of the pinned Lichess opening corpus

The corpus supplies five textual fields.  UCI is parsed and legally replayed
from `Initial.game`.  PGN is first checked for its canonical move-number
skeleton, then every SAN token is resolved in the evolving position and
required to denote exactly the corresponding UCI move.
-/

/-- Repository-relative location of the pinned Lichess opening table. -/
def path : String := "data/lichess-openings/all.tsv"

/-- The only accepted schema, including field order and spelling. -/
def header : String := "eco\tname\tpgn\tuci\tepd"

/-- Row count of the pinned upstream snapshot, excluding its header. -/
def expectedRows : Nat := 3803

/-- Aggregate number of half-moves in the pinned snapshot. -/
def expectedPlies : Nat := 36840

/-- Successful-work counters and uniqueness cardinalities. -/
structure Summary where
  rowCount : Nat
  schemaRows : Nat
  replayedRows : Nat
  checkedPlies : Nat
  sanMatchedRows : Nat
  sanMatchedPlies : Nat
  uniquePGN : Nat
  uniqueUCI : Nat
  uniqueEPD : Nat
  deriving Repr, DecidableEq

/-- A validation run returns every discovered failure rather than stopping at
the first malformed or illegal row. -/
structure Report where
  source : String
  summary : Summary
  failures : List String
  deriving Repr

def Report.isValid (report : Report) : Bool := report.failures.isEmpty

def Summary.describe (summary : Summary) : String :=
  s!"{summary.rowCount} rows, {summary.replayedRows} legally replayed, " ++
  s!"{summary.checkedPlies} checked plies, " ++
  s!"{summary.sanMatchedRows} SAN/UCI-matched rows, " ++
  s!"{summary.sanMatchedPlies} SAN/UCI-matched plies, " ++
  s!"{summary.uniquePGN} unique PGN, {summary.uniqueUCI} unique UCI, " ++
  s!"{summary.uniqueEPD} unique EPD"

private abbrev Seen := Std.HashMap String Nat

private structure State where
  schemaRows : Nat := 0
  replayedRows : Nat := 0
  checkedPlies : Nat := 0
  sanMatchedRows : Nat := 0
  sanMatchedPlies : Nat := 0
  seenPGN : Seen
  seenUCI : Seen
  seenEPD : Seen
  failures : Array String := #[]

private def initialState : State where
  seenPGN := Std.HashMap.emptyWithCapacity expectedRows
  seenUCI := Std.HashMap.emptyWithCapacity expectedRows
  seenEPD := Std.HashMap.emptyWithCapacity expectedRows

private def location (source : String) (line : Nat) (message : String) : String :=
  s!"{source}:{line}: {message}"

private def addFailure (source : String) (line : Nat) (state : State)
    (message : String) : State :=
  { state with failures := state.failures.push (location source line message) }

private def addFileFailure (source : String) (state : State) (message : String) : State :=
  { state with failures := state.failures.push s!"{source}: {message}" }

/-- Record a value's first row and diagnose every subsequent occurrence
against that row.  Keeping the first row makes three-way duplicates useful to
debug rather than producing a moving chain of references. -/
private def recordUnique (source field value : String) (line : Nat)
    (seen : Seen) (state : State) : Seen × State :=
  match seen.get? value with
  | none => (seen.insert value line, state)
  | some firstLine =>
      (seen, addFailure source line state
        s!"duplicate {field}; first occurrence is row {firstLine}")

private def validECO : String → Bool
  | text =>
      match text.toList with
      | [letter, tens, ones] =>
          let validLetter :=
            letter == 'A' || letter == 'B' || letter == 'C' ||
            letter == 'D' || letter == 'E'
          let digit (symbol : Char) : Bool :=
            symbol == '0' || symbol == '1' || symbol == '2' ||
            symbol == '3' || symbol == '4' || symbol == '5' ||
            symbol == '6' || symbol == '7' || symbol == '8' || symbol == '9'
          validLetter && digit tens && digit ones
      | _ => false

/-- Split a nonempty UCI line, requiring exactly one ASCII space between
tokens.  Individual token syntax and chess legality are checked by replay. -/
private def parseUCITokens (text : String) : Except String (List String) :=
  if text.isEmpty then
    .error "UCI field must not be empty"
  else
    let tokens := text.splitOn " "
    if tokens.any String.isEmpty then
      .error "UCI moves must be separated by one ASCII space"
    else
      .ok tokens

private def opaquePGNMove (token : String) : Except String String :=
  if token.isEmpty then
    .error "PGN move token must not be empty"
  else if token.endsWith "." then
    .error s!"expected an opaque move token, got move-number-like token '{token}'"
  else
    .ok token

/-- Parse only the canonical move-number skeleton used by the corpus:
`1. white black 2. white black ...`.  This structural pass leaves move tokens
opaque; `crossValidateSAN` subsequently gives each token its checked SAN
meaning in the position where it occurs. -/
private def parsePGNStructure (text : String) : Except String (List String) := do
  if text.isEmpty then
    throw "PGN field must not be empty"
  let tokens := text.splitOn " "
  if tokens.any String.isEmpty then
    throw "PGN tokens must be separated by one ASCII space"
  let rec loop (moveNumber : Nat) (whiteToRead : Bool)
      (remaining : List String) (movesRev : List String) : Except String (List String) := do
    if whiteToRead then
      match remaining with
      | [] => pure movesRev.reverse
      | [_] => throw s!"move {moveNumber} has a number but no White move"
      | numberToken :: moveToken :: rest =>
          let expected := s!"{moveNumber}."
          if numberToken != expected then
            throw s!"expected move number '{expected}', got '{numberToken}'"
          let move ← opaquePGNMove moveToken
          loop moveNumber false rest (move :: movesRev)
    else
      match remaining with
      | [] => pure movesRev.reverse
      | moveToken :: rest =>
          let move ← opaquePGNMove moveToken
          loop (moveNumber + 1) true rest (move :: movesRev)
  loop 1 true tokens []

/-- Resolve SAN and UCI in lockstep.  Equality is checked at every ply, which
is stronger than merely comparing endpoints after a possible transposition. -/
private def crossValidateSAN : GameState → Nat → List String → List String →
    Except String GameState
  | state, _, [], [] => .ok state
  | _, ply, [], _ :: _ =>
      .error s!"ply {ply}: PGN ended before UCI"
  | _, ply, _ :: _, [] =>
      .error s!"ply {ply}: UCI ended before PGN"
  | state, ply, sanToken :: sanRest, uciToken :: uciRest => do
      let uciMove ← match UCI.parse uciToken with
        | .ok move => .ok move
        | .error error => .error s!"ply {ply}, UCI '{uciToken}': {error}"
      let sanMove ← match SAN.resolve state.current sanToken with
        | .ok move => .ok move
        | .error error => .error s!"ply {ply}, SAN '{sanToken}': {error}"
      if sanMove != uciMove then
        throw (s!"ply {ply}: SAN '{sanToken}' resolves to {UCI.render sanMove}, " ++
          s!"but UCI field contains {uciToken}")
      crossValidateSAN (state.afterMove uciMove) (ply + 1) sanRest uciRest

private def effectiveEPD (position : Position) : Except String String := do
  let fen ← FEN.renderEffective position
  match fen.splitOn " " with
  | [placement, turn, castling, enPassant, _, _] =>
      pure (String.intercalate " " [placement, turn, castling, enPassant])
  | fields =>
      throw s!"internal effective FEN renderer produced {fields.length} fields"

private def validateNonemptyFields (source : String) (line : Nat)
    (fields : List (String × String)) (state : State) : State :=
  fields.foldl (fun state entry =>
    if entry.2.isEmpty then
      addFailure source line state s!"{entry.1} field must not be empty"
    else state) state

private def validateRow (source : String) (line : Nat) (text : String)
    (state : State) : State :=
  match text.splitOn "\t" with
  | [eco, name, pgn, uci, epd] =>
      let state := { state with schemaRows := state.schemaRows + 1 }
      let state := validateNonemptyFields source line
        [("ECO", eco), ("name", name), ("PGN", pgn), ("UCI", uci), ("EPD", epd)] state
      let state :=
        if !eco.isEmpty && !validECO eco then
          addFailure source line state s!"invalid ECO '{eco}'; expected A00 through E99"
        else state
      let (seenPGN, state) :=
        if pgn.isEmpty then (state.seenPGN, state)
        else recordUnique source "PGN" pgn line state.seenPGN state
      let state := { state with seenPGN }
      let (seenUCI, state) :=
        if uci.isEmpty then (state.seenUCI, state)
        else recordUnique source "UCI" uci line state.seenUCI state
      let state := { state with seenUCI }
      let (seenEPD, state) :=
        if epd.isEmpty then (state.seenEPD, state)
        else recordUnique source "EPD" epd line state.seenEPD state
      let state := { state with seenEPD }
      let parsedPGN := parsePGNStructure pgn
      let parsedUCI := parseUCITokens uci
      let state := match parsedPGN with
        | .error message => addFailure source line state s!"invalid PGN structure: {message}"
        | .ok _ => state
      let state := match parsedUCI with
        | .error message => addFailure source line state s!"invalid UCI move list: {message}"
        | .ok _ => state
      let state := match parsedPGN, parsedUCI with
        | .ok pgnMoves, .ok uciMoves =>
            if pgnMoves.length == uciMoves.length then state
            else addFailure source line state
              s!"PGN/UCI ply-count mismatch: PGN has {pgnMoves.length}, UCI has {uciMoves.length}"
        | _, _ => state
      let sanResult := match parsedPGN, parsedUCI with
        | .ok pgnMoves, .ok uciMoves =>
            crossValidateSAN Initial.game 1 pgnMoves uciMoves
        | .error message, _ => .error s!"PGN structure unavailable: {message}"
        | _, .error message => .error s!"UCI move list unavailable: {message}"
      let state := match sanResult with
        | .ok _ =>
            match parsedUCI with
            | .ok moves => { state with
                sanMatchedRows := state.sanMatchedRows + 1
                sanMatchedPlies := state.sanMatchedPlies + moves.length }
            | .error _ => state
        | .error message =>
            match parsedPGN, parsedUCI with
            | .ok _, .ok _ =>
                addFailure source line state s!"SAN/UCI cross-validation failed: {message}"
            | _, _ => state
      match parsedUCI with
      | .error _ => state
      | .ok moves =>
          match Replay.replayUCI Initial.game moves with
          | .error failure =>
              addFailure source line state s!"checked UCI replay failed: {failure}"
          | .ok final =>
              match effectiveEPD final.current with
              | .error message =>
                  addFailure source line state s!"effective EPD rendering failed: {message}"
              | .ok actualEPD =>
                  let state := { state with
                    replayedRows := state.replayedRows + 1
                    checkedPlies := state.checkedPlies + moves.length }
                  if actualEPD == epd then state
                  else addFailure source line state
                    s!"EPD mismatch: expected '{epd}', got '{actualEPD}'"
  | fields =>
      addFailure source line state
        s!"expected exactly 5 tab-separated fields, got {fields.length}"

private def contentLines (content : String) : List String :=
  if content.isEmpty then
    []
  else
    let lines := content.splitOn "\n"
    if content.endsWith "\n" then lines.dropLast else lines

private def validateDataRows (source : String) : Nat → List String → State → State
  | _, [], state => state
  | line, row :: rest, state =>
      validateDataRows source (line + 1) rest (validateRow source line row state)

private def requireAggregate (source label : String) (expected actual : Nat)
    (state : State) : State :=
  if actual == expected then state
  else addFileFailure source state s!"expected {expected} {label}, got {actual}"

/-- Validate already-loaded corpus text.  This pure entry point is useful for
unit tests and for callers that obtain the pinned bytes by another mechanism. -/
def validateContent (source content : String) : Report :=
  let lines := contentLines content
  let (actualHeader, rows) := match lines with
    | [] => (none, [])
    | first :: rest => (some first, rest)
  let state := initialState
  let state := match actualHeader with
    | none => addFileFailure source state "missing header"
    | some actual =>
        if actual == header then state
        else addFailure source 1 state s!"unexpected header; expected '{header}'"
  let state :=
    if rows.length == expectedRows then state
    else addFileFailure source state
      s!"expected exactly {expectedRows} data rows, got {rows.length}"
  let state := validateDataRows source 2 rows state
  let state := requireAggregate source "five-field rows"
    expectedRows state.schemaRows state
  let state := requireAggregate source "legally replayed rows"
    expectedRows state.replayedRows state
  let state := requireAggregate source "checked UCI plies"
    expectedPlies state.checkedPlies state
  let state := requireAggregate source "SAN/UCI-matched rows"
    expectedRows state.sanMatchedRows state
  let state := requireAggregate source "SAN/UCI-matched plies"
    expectedPlies state.sanMatchedPlies state
  let state := requireAggregate source "unique PGN values"
    expectedRows state.seenPGN.size state
  let state := requireAggregate source "unique UCI values"
    expectedRows state.seenUCI.size state
  let state := requireAggregate source "unique EPD values"
    expectedRows state.seenEPD.size state
  { source
    summary :=
      { rowCount := rows.length
        schemaRows := state.schemaRows
        replayedRows := state.replayedRows
        checkedPlies := state.checkedPlies
        sanMatchedRows := state.sanMatchedRows
        sanMatchedPlies := state.sanMatchedPlies
        uniquePGN := state.seenPGN.size
        uniqueUCI := state.seenUCI.size
        uniqueEPD := state.seenEPD.size }
    failures := state.failures.toList }

/-- Read and validate a corpus file, preserving I/O errors in the same
accumulated-report interface as content failures. -/
def validateFile (source : String := path) : IO Report := do
  try
    let content ← IO.FS.readFile source
    pure (validateContent source content)
  catch error =>
    pure
      { source
        summary := ⟨0, 0, 0, 0, 0, 0, 0, 0, 0⟩
        failures := [s!"{source}: could not read file: {error}"] }

/-- Validate the repository's pinned corpus. -/
def validate : IO Report := validateFile path

end Chess.OpeningCorpus
