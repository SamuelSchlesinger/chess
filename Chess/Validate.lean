import Chess.Replay
import Chess.Theory.Opening

namespace Chess.Validate

private abbrev Failures := List String

private def location (file : String) (line : Nat) (message : String) : String :=
  s!"{file}:{line}: {message}"

private def fieldError (field message : String) : Except String α :=
  .error s!"invalid {field}: {message}"

private def parseNatField (field text : String) : Except String Nat :=
  match text.toNat? with
  | some value => .ok value
  | none => fieldError field s!"expected a natural number, got '{text}'"

private def parseBoolField (field text : String) : Except String Bool :=
  match text with
  | "0" => .ok false
  | "1" => .ok true
  | _ => fieldError field s!"expected 0 or 1, got '{text}'"

private def parseMoves (text : String) : Except String (List String) :=
  if text == "-" then
    .ok []
  else if text.isEmpty then
    fieldError "UCI move list" "use '-' for an empty list"
  else
    let moves := text.splitOn " "
    if moves.any String.isEmpty then
      fieldError "UCI move list" "moves must be separated by one ASCII space"
    else
      .ok moves

private def knownSources : List String :=
  ["stockfish18-standard-suite", "local-stockfish18", "pgn-fen-1994",
    "uci-2006", "lean-game-examples", "lean-opening-theory", "handcrafted-rules"]

private def validateSource (source : String) : Failures :=
  if knownSources.contains source then [] else [s!"unknown source_id '{source}'"]

private def mismatch [BEq α] [ToString α] (field : String) (expected actual : α) : Failures :=
  if actual == expected then []
  else [s!"{field}: expected {expected}, got {actual}"]

private def parseFEN (text : String) : Except String Position :=
  match FEN.parse text with
  | .ok position => .ok position
  | .error message => .error s!"invalid FEN '{text}': {message}"

private def replay (startFEN movesText : String) : Except String GameState := do
  let position ← parseFEN startFEN
  let moves ← parseMoves movesText
  let initial : GameState := { current := position, prior := [] }
  match Replay.replayUCI initial moves with
  | .ok state => .ok state
  | .error failure => .error s!"checked replay failed: {failure}"

private def renderRaw (position : Position) : Except String String :=
  match FEN.renderRaw position with
  | .ok text => .ok text
  | .error message => .error s!"raw FEN rendering failed: {message}"

private def renderEffective (position : Position) : Except String String :=
  match FEN.renderEffective position with
  | .ok text => .ok text
  | .error message => .error s!"effective FEN rendering failed: {message}"

/-- `GameState.prior` is newest first, so every newer phase must be no larger
than its adjacent predecessor. Report every offending ply rather than stopping
at the first one. -/
private def phasePathFailures (state : GameState) : Failures :=
  let rec loop (ply : Nat) : List Position → Failures
    | newer :: older :: rest =>
        let newerPhase := newer.phasePotential
        let olderPhase := older.phasePotential
        let here :=
          if newerPhase ≤ olderPhase then []
          else [s!"phase increased at ply {ply}: {olderPhase} -> {newerPhase}"]
        here ++ loop (ply - 1) (older :: rest)
    | _ => []
  loop state.prior.length (state.current :: state.prior)

private def validatePerftRow : List String → Failures
  | [_, fen, depthText, nodesText, source] =>
      match parseFEN fen, parseNatField "depth" depthText, parseNatField "nodes" nodesText with
      | .error message, _, _ => message :: validateSource source
      | _, .error message, _ => message :: validateSource source
      | _, _, .error message => message :: validateSource source
      | .ok position, .ok depth, .ok expected =>
          mismatch "perft nodes" expected (perft depth position) ++ validateSource source
  | fields => [s!"expected 5 tab-separated fields, got {fields.length}"]

private def validateMoveRow : List String → Failures
  | [_, fen, uci, legalText, source] =>
      match parseFEN fen, parseBoolField "legal" legalText with
      | .error message, _ => message :: validateSource source
      | _, .error message => message :: validateSource source
      | .ok position, .ok expectedLegal =>
          match UCI.parse uci with
          | .error parseError =>
              s!"invalid UCI fixture '{uci}': {parseError}" :: validateSource source
          | .ok move =>
              let initial : GameState := { current := position, prior := [] }
              let replayed := Replay.replayUCI initial [uci]
              let actualLegal := isLegal position move
              let replaySucceeded := match replayed with
                | .ok _ => true
                | .error _ => false
              let diagnostic := match expectedLegal, replayed with
                | true, .error failure =>
                    [s!"expected success, but checked replay reported: {failure}"]
                | _, _ => []
              let phaseFailures := match replayed with
                | .ok state => phasePathFailures state
                | .error _ => []
              mismatch "move legality" expectedLegal actualLegal ++
              mismatch "checked replay consistency" actualLegal replaySucceeded ++
              diagnostic ++ phaseFailures ++ validateSource source
  | fields => [s!"expected 5 tab-separated fields, got {fields.length}"]

private def validateTraceRow : List String → Failures
  | [_, startFEN, movesText, expectedRaw, expectedEffective, repetitionsText,
      threefoldText, fivefoldText, halfmove100Text, halfmove150Text, checkmateText,
      phaseText, source] =>
      match replay startFEN movesText,
          parseNatField "repetitions" repetitionsText,
          parseBoolField "threefold" threefoldText,
          parseBoolField "fivefold" fivefoldText,
          parseBoolField "halfmove_ge_100" halfmove100Text,
          parseBoolField "halfmove_ge_150" halfmove150Text,
          parseBoolField "checkmate" checkmateText,
          parseNatField "phase" phaseText with
      | .error message, _, _, _, _, _, _, _ => message :: validateSource source
      | _, .error message, _, _, _, _, _, _ => message :: validateSource source
      | _, _, .error message, _, _, _, _, _ => message :: validateSource source
      | _, _, _, .error message, _, _, _, _ => message :: validateSource source
      | _, _, _, _, .error message, _, _, _ => message :: validateSource source
      | _, _, _, _, _, .error message, _, _ => message :: validateSource source
      | _, _, _, _, _, _, .error message, _ => message :: validateSource source
      | _, _, _, _, _, _, _, .error message => message :: validateSource source
      | .ok state, .ok expectedRepetitions, .ok expectedThreefold, .ok expectedFivefold,
          .ok expectedHalfmove100, .ok expectedHalfmove150, .ok expectedCheckmate,
          .ok expectedPhase =>
          match renderRaw state.current, renderEffective state.current with
          | .error message, _ => message :: validateSource source
          | _, .error message => message :: validateSource source
          | .ok actualRaw, .ok actualEffective =>
              let actualRepetitions := repetitionCount state
              let actualMate := inCheck state.current.board state.current.turn &&
                (legalMoves state.current).isEmpty
              mismatch "raw FEN" expectedRaw actualRaw ++
              mismatch "effective FEN" expectedEffective actualEffective ++
              mismatch "repetition count" expectedRepetitions actualRepetitions ++
              mismatch "threefold flag" expectedThreefold (3 ≤ actualRepetitions) ++
              mismatch "fivefold flag" expectedFivefold (5 ≤ actualRepetitions) ++
              mismatch "100-halfmove flag" expectedHalfmove100
                (100 ≤ state.current.halfmoveClock) ++
              mismatch "150-halfmove flag" expectedHalfmove150
                (150 ≤ state.current.halfmoveClock) ++
              mismatch "checkmate flag" expectedCheckmate actualMate ++
              mismatch "phase" expectedPhase state.current.phasePotential ++
              phasePathFailures state ++
              validateSource source
  | fields => [s!"expected 13 tab-separated fields, got {fields.length}"]

private def validateOpeningPairRow : List String → Failures
  | [_, startFEN, leftMoves, rightMoves, relation, expectedLeftRaw, expectedRightRaw,
      expectedLeftEffective, expectedRightEffective, leftPhaseText, rightPhaseText, source] =>
      match replay startFEN leftMoves, replay startFEN rightMoves,
          parseNatField "left_phase" leftPhaseText, parseNatField "right_phase" rightPhaseText with
      | .error message, _, _, _ => s!"left line: {message}" :: validateSource source
      | _, .error message, _, _ => s!"right line: {message}" :: validateSource source
      | _, _, .error message, _ => message :: validateSource source
      | _, _, _, .error message => message :: validateSource source
      | .ok left, .ok right, .ok expectedLeftPhase, .ok expectedRightPhase =>
          match renderRaw left.current, renderRaw right.current,
              renderEffective left.current, renderEffective right.current with
          | .error message, _, _, _ => s!"left line: {message}" :: validateSource source
          | _, .error message, _, _ => s!"right line: {message}" :: validateSource source
          | _, _, .error message, _ => s!"left line: {message}" :: validateSource source
          | _, _, _, .error message => s!"right line: {message}" :: validateSource source
          | .ok leftRaw, .ok rightRaw, .ok leftEffective, .ok rightEffective =>
              let sameExact := Theory.sameCompletePosition left.current right.current
              let sameRepetition := sameForRepetition left.current right.current
              let relationFailures := match relation with
                | "exact" => mismatch "exact endpoint relation" true sameExact ++
                    mismatch "repetition endpoint relation" true sameRepetition
                | "repetition" => mismatch "repetition endpoint relation" true sameRepetition ++
                    mismatch "repetition-only endpoint distinction" false sameExact
                | _ => [s!"invalid relation '{relation}': expected exact or repetition"]
              mismatch "left raw FEN" expectedLeftRaw leftRaw ++
              mismatch "right raw FEN" expectedRightRaw rightRaw ++
              mismatch "left effective FEN" expectedLeftEffective leftEffective ++
              mismatch "right effective FEN" expectedRightEffective rightEffective ++
              mismatch "left phase" expectedLeftPhase left.current.phasePotential ++
              mismatch "right phase" expectedRightPhase right.current.phasePotential ++
              relationFailures ++
              (phasePathFailures left).map (s!"left line: " ++ ·) ++
              (phasePathFailures right).map (s!"right line: " ++ ·) ++
              validateSource source
  | fields => [s!"expected 12 tab-separated fields, got {fields.length}"]

private def numberedLines (content : String) : List (Nat × String) :=
  let rec loop (number : Nat) : List String → List (Nat × String)
    | [] => []
    | line :: rest =>
        let line := if line.endsWith "\r" then line.dropEnd 1 |>.toString else line
        (number, line) :: loop (number + 1) rest
  loop 1 (content.splitOn "\n")

private def dataLines (content : String) : List (Nat × String) :=
  (numberedLines content).filter fun entry =>
    let trimmed := entry.2.trimAscii.toString
    !trimmed.isEmpty && !trimmed.startsWith "#"

private def validateRows (file expectedHeader : String)
    (validateRow : List String → Failures) (content : String) : Failures :=
  match dataLines content with
  | [] => [s!"{file}: missing header and data rows"]
  | (headerLine, header) :: rows =>
      let headerFailures :=
        if header == expectedHeader then []
        else [location file headerLine s!"unexpected header; expected '{expectedHeader}'"]
      let rec loop (seen : List String) : List (Nat × String) → Failures
        | [] => []
        | (line, text) :: rest =>
            let fields := text.splitOn "\t"
            let id := fields.head?.getD ""
            let idFailures :=
              if id.isEmpty then ["row id must not be empty"]
              else if seen.contains id then [s!"duplicate row id '{id}'"]
              else []
            let seen := if id.isEmpty then seen else id :: seen
            (idFailures ++ validateRow fields).map (location file line) ++ loop seen rest
      headerFailures ++ loop [] rows

private def validateFile (file header : String)
    (validateRow : List String → Failures) : IO Failures := do
  try
    let content ← IO.FS.readFile file
    pure (validateRows file header validateRow content)
  catch error =>
    pure [s!"{file}: could not read file: {error}"]

private def perftHeader := "id\tfen\tdepth\tnodes\tsource_id"
private def movesHeader := "id\tfen\tuci\tlegal\tsource_id"
private def tracesHeader :=
  "id\tstart_fen\tuci_moves\texpected_raw_fen\texpected_effective_fen\trepetitions\tthreefold\tfivefold\thalfmove_ge_100\thalfmove_ge_150\tcheckmate\tphase\tsource_id"
private def openingPairsHeader :=
  "id\tstart_fen\tleft_moves\tright_moves\trelation\tleft_raw_fen\tright_raw_fen\tleft_effective_fen\tright_effective_fen\tleft_phase\tright_phase\tsource_id"

def main : IO Unit := do
  let perftFailures ← validateFile "data/perft.tsv" perftHeader validatePerftRow
  let moveFailures ← validateFile "data/moves.tsv" movesHeader validateMoveRow
  let traceFailures ← validateFile "data/traces.tsv" tracesHeader validateTraceRow
  let openingFailures ←
    validateFile "data/opening_pairs.tsv" openingPairsHeader validateOpeningPairRow
  let failures := perftFailures ++ moveFailures ++ traceFailures ++ openingFailures
  if failures.isEmpty then
    IO.println "validated data/perft.tsv, data/moves.tsv, data/traces.tsv, and data/opening_pairs.tsv"
  else
    for failure in failures do
      IO.eprintln s!"FAIL: {failure}"
    IO.eprintln s!"validation failed with {failures.length} error(s)"
    IO.Process.exit 1

end Chess.Validate

def main : IO Unit := Chess.Validate.main
