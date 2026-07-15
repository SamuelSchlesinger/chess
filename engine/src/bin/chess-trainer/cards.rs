//! Private diagnostic-card import and curated training-deck overlay.
//!
//! A diagnostic bundle is evidence, not a curriculum.  The deck chooses and
//! orders a small subset, supplies human-facing pedagogy, and is the only list
//! exposed to the trainer.  Import validates the complete game history before
//! retaining a deliberately sanitized, history-free [`TrainingCard`].

use chess::{Board, Color, Game};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

const BUNDLE_SCHEMA: &str = "chess-diagnostic-cards";
const DECK_SCHEMA: &str = "chess-training-deck";
const SCHEMA_VERSION: u32 = 1;
const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;

/// Stable identity for mutable review state.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CardKey {
    pub card_id: String,
    pub content_version: String,
}

/// One legal choice rendered without any answer fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalChoice {
    pub uci: String,
    pub san: String,
}

/// The answer-free record that may be serialized to the browser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainingPrompt {
    pub prompt: String,
    pub fen: String,
    pub position_id: String,
    pub orientation: String,
    pub check: bool,
    pub legal_moves: Vec<LegalChoice>,
    pub tags: Vec<String>,
}

/// One move in a revealed, legality-checked engine line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealedLineMove {
    pub uci: String,
    pub san: String,
}

/// Feedback available only after the server has accepted a legal response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardFeedback {
    pub played_move: Option<LegalChoice>,
    pub reference_match: Option<bool>,
    pub reference_move: LegalChoice,
    pub source_game_move: LegalChoice,
    pub reference_line: Vec<RevealedLineMove>,
    pub source_game_line: Vec<RevealedLineMove>,
    pub explanation: String,
    pub success_criterion: String,
    pub loss_cp_equivalent: i64,
    pub loss_bucket: String,
    pub mate_event: Option<String>,
    pub evidence_version: String,
}

#[derive(Clone, Debug)]
struct HiddenAnswer {
    reference_move: LegalChoice,
    source_game_move: LegalChoice,
    reference_line: Vec<RevealedLineMove>,
    source_game_line: Vec<RevealedLineMove>,
    loss_cp_equivalent: i64,
    loss_bucket: String,
    mate_event: Option<String>,
}

/// A curated card.  Its answer and explanation are private so the prompt view
/// cannot accidentally serialize them before retrieval.
#[derive(Clone, Debug)]
pub struct TrainingCard {
    pub card_id: String,
    pub content_version: String,
    pub evidence_version: String,
    pub prompt: String,
    pub success_criterion: String,
    pub fen: String,
    pub position_id: String,
    pub orientation: String,
    pub check: bool,
    pub legal_moves: Vec<LegalChoice>,
    pub tags: Vec<String>,
    explanation: String,
    answer: HiddenAnswer,
}

impl TrainingCard {
    pub fn key(&self) -> CardKey {
        CardKey {
            card_id: self.card_id.clone(),
            content_version: self.content_version.clone(),
        }
    }

    /// An owned browser-safe view.  Neither the explanation nor engine answer
    /// appears in this value.
    pub fn prompt_view(&self) -> TrainingPrompt {
        TrainingPrompt {
            prompt: self.prompt.clone(),
            fen: self.fen.clone(),
            position_id: self.position_id.clone(),
            orientation: self.orientation.clone(),
            check: self.check,
            legal_moves: self.legal_moves.clone(),
            tags: self.tags.clone(),
        }
    }

    /// Validate a submitted move and reveal the bounded-engine evidence.
    pub fn feedback(&self, move_uci: &str) -> Result<CardFeedback, CardsError> {
        let played_move = self
            .legal_moves
            .iter()
            .find(|choice| choice.uci == move_uci)
            .cloned()
            .ok_or_else(|| CardsError::InvalidResponse {
                card_id: self.card_id.clone(),
                message: "submitted move is not legal in the card position".to_string(),
            })?;
        Ok(CardFeedback {
            reference_match: Some(played_move.uci == self.answer.reference_move.uci),
            played_move: Some(played_move),
            reference_move: self.answer.reference_move.clone(),
            source_game_move: self.answer.source_game_move.clone(),
            reference_line: self.answer.reference_line.clone(),
            source_game_line: self.answer.source_game_line.clone(),
            explanation: self.explanation.clone(),
            success_criterion: self.success_criterion.clone(),
            loss_cp_equivalent: self.answer.loss_cp_equivalent,
            loss_bucket: self.answer.loss_bucket.clone(),
            mate_event: self.answer.mate_event.clone(),
            evidence_version: self.evidence_version.clone(),
        })
    }

    /// Reveal the answer after an explicit give-up without inventing either a
    /// played move or a reference-match observation.
    pub fn give_up_feedback(&self) -> CardFeedback {
        CardFeedback {
            played_move: None,
            reference_match: None,
            reference_move: self.answer.reference_move.clone(),
            source_game_move: self.answer.source_game_move.clone(),
            reference_line: self.answer.reference_line.clone(),
            source_game_line: self.answer.source_game_line.clone(),
            explanation: self.explanation.clone(),
            success_criterion: self.success_criterion.clone(),
            loss_cp_equivalent: self.answer.loss_cp_equivalent,
            loss_bucket: self.answer.loss_bucket.clone(),
            mate_event: self.answer.mate_event.clone(),
            evidence_version: self.evidence_version.clone(),
        }
    }
}

/// Immutable curated deck in declared order.
#[derive(Clone, Debug)]
pub struct Catalog {
    pub deck_id: String,
    pub title: String,
    pub new_per_day: usize,
    pub analysis_config_version: String,
    pub assurance: String,
    pub confirmation_nodes_per_position: u64,
    cards: Vec<TrainingCard>,
    by_key: BTreeMap<CardKey, usize>,
}

impl Catalog {
    pub fn load(
        bundle_path: impl AsRef<Path>,
        deck_path: impl AsRef<Path>,
    ) -> Result<Self, CardsError> {
        let bundle_path = bundle_path.as_ref();
        let deck_path = deck_path.as_ref();
        let bundle: DiagnosticBundle = read_json(bundle_path)?;
        let deck: DeckOverlay = read_json(deck_path)?;
        Self::from_inputs(bundle, deck)
    }

    pub fn cards(&self) -> &[TrainingCard] {
        &self.cards
    }

    pub fn get(&self, card_id: &str, content_version: &str) -> Option<&TrainingCard> {
        let key = CardKey {
            card_id: card_id.to_string(),
            content_version: content_version.to_string(),
        };
        self.by_key.get(&key).map(|&index| &self.cards[index])
    }

    #[cfg(test)]
    pub(crate) fn fixture_for_tests(card_count: usize, new_per_day: usize) -> Self {
        let board = Board::startpos();
        let legal_moves: Vec<_> = board
            .legal_moves()
            .iter()
            .map(|&mv| LegalChoice {
                uci: mv.to_uci(),
                san: board.san(mv),
            })
            .collect();
        let reference_move = choice(&board, "e2e4").unwrap();
        let source_game_move = choice(&board, "d2d4").unwrap();
        let mut cards = Vec::new();
        let mut by_key = BTreeMap::new();
        for index in 0..card_count {
            let key = CardKey {
                card_id: format!("fixture-card-{index}"),
                content_version: "fixture-content-v1".to_string(),
            };
            by_key.insert(key.clone(), cards.len());
            cards.push(TrainingCard {
                card_id: key.card_id,
                content_version: key.content_version,
                evidence_version: "fixture-evidence-v1".to_string(),
                prompt: format!("Fixture prompt {index}"),
                success_criterion: "Handle the fixture idea.".to_string(),
                fen: board.to_fen(),
                position_id: board.position_id(),
                orientation: "white".to_string(),
                check: board.in_check(),
                legal_moves: legal_moves.clone(),
                tags: vec!["fixture".to_string()],
                explanation: "Fixture explanation.".to_string(),
                answer: HiddenAnswer {
                    reference_move: reference_move.clone(),
                    source_game_move: source_game_move.clone(),
                    reference_line: vec![RevealedLineMove {
                        uci: reference_move.uci.clone(),
                        san: reference_move.san.clone(),
                    }],
                    source_game_line: vec![RevealedLineMove {
                        uci: source_game_move.uci.clone(),
                        san: source_game_move.san.clone(),
                    }],
                    loss_cp_equivalent: 100,
                    loss_bucket: "fixture".to_string(),
                    mate_event: None,
                },
            });
        }
        Catalog {
            deck_id: "fixture-deck".to_string(),
            title: "Fixture deck".to_string(),
            new_per_day,
            analysis_config_version: format!("sha256:{}", "0".repeat(64)),
            assurance: "fixture".to_string(),
            confirmation_nodes_per_position: 1,
            cards,
            by_key,
        }
    }

    fn from_inputs(bundle: DiagnosticBundle, deck: DeckOverlay) -> Result<Self, CardsError> {
        if bundle.schema != BUNDLE_SCHEMA || bundle.schema_version != SCHEMA_VERSION {
            return Err(CardsError::Schema(format!(
                "expected {BUNDLE_SCHEMA} schema version {SCHEMA_VERSION}"
            )));
        }
        if deck.schema != DECK_SCHEMA || deck.schema_version != SCHEMA_VERSION {
            return Err(CardsError::Schema(format!(
                "expected {DECK_SCHEMA} schema version {SCHEMA_VERSION}"
            )));
        }
        validate_nonempty("deck_id", &deck.deck_id)?;
        validate_nonempty("deck title", &deck.title)?;
        if deck.new_per_day == 0 || deck.new_per_day > 100 {
            return Err(CardsError::Schema(
                "new_per_day must be between 1 and 100".to_string(),
            ));
        }
        if deck.cards.is_empty() {
            return Err(CardsError::Schema("training deck has no cards".to_string()));
        }
        validate_digest("analysis_config_version", &bundle.analysis_config_version)?;
        validate_nonempty("analysis assurance", &bundle.analysis.assurance)?;
        if bundle.analysis.confirmation_nodes_per_position == 0 {
            return Err(CardsError::Schema(
                "confirmation_nodes_per_position must be positive".to_string(),
            ));
        }

        let mut source = BTreeMap::new();
        let mut seen_ids = BTreeSet::new();
        for card in bundle.cards {
            validate_digest("content_version", &card.content_version)?;
            validate_digest("evidence_version", &card.evidence_version)?;
            let key = CardKey {
                card_id: card.card_id.clone(),
                content_version: card.content_version.clone(),
            };
            if !seen_ids.insert(card.card_id.clone()) {
                return Err(CardsError::Duplicate(format!(
                    "bundle repeats card id '{}'",
                    card.card_id
                )));
            }
            if source
                .insert(
                    key.clone(),
                    validate_source_card(card, &bundle.analysis_config_version)?,
                )
                .is_some()
            {
                return Err(CardsError::Duplicate(format!(
                    "bundle repeats card key '{}@{}'",
                    key.card_id, key.content_version
                )));
            }
        }

        let mut cards = Vec::with_capacity(deck.cards.len());
        let mut by_key = BTreeMap::new();
        for overlay in deck.cards {
            let key = CardKey {
                card_id: overlay.card_id.clone(),
                content_version: overlay.content_version.clone(),
            };
            if by_key.contains_key(&key) {
                return Err(CardsError::Duplicate(format!(
                    "deck repeats card key '{}@{}'",
                    key.card_id, key.content_version
                )));
            }
            validate_nonempty("deck prompt", &overlay.prompt)?;
            validate_nonempty("deck explanation", &overlay.explanation)?;
            validate_nonempty("success criterion", &overlay.success_criterion)?;
            let raw = source.get(&key).ok_or_else(|| {
                CardsError::DeckReference(format!(
                    "deck card '{}@{}' is absent from the diagnostic bundle",
                    key.card_id, key.content_version
                ))
            })?;

            let mut tags = raw.tags.clone();
            for tag in overlay.tags {
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
            let index = cards.len();
            cards.push(TrainingCard {
                card_id: key.card_id.clone(),
                content_version: key.content_version.clone(),
                evidence_version: raw.evidence_version.clone(),
                prompt: overlay.prompt,
                success_criterion: overlay.success_criterion,
                fen: raw.current_fen.clone(),
                position_id: raw.position_id.clone(),
                orientation: raw.orientation.clone(),
                check: raw.check,
                legal_moves: raw.legal_moves.clone(),
                tags,
                explanation: overlay.explanation,
                answer: raw.answer.clone(),
            });
            by_key.insert(key, index);
        }

        Ok(Catalog {
            deck_id: deck.deck_id,
            title: deck.title,
            new_per_day: deck.new_per_day,
            analysis_config_version: bundle.analysis_config_version,
            assurance: bundle.analysis.assurance,
            confirmation_nodes_per_position: bundle.analysis.confirmation_nodes_per_position,
            cards,
            by_key,
        })
    }
}

#[derive(Debug)]
pub enum CardsError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Schema(String),
    Duplicate(String),
    DeckReference(String),
    InvalidCard {
        card_id: String,
        message: String,
    },
    InvalidResponse {
        card_id: String,
        message: String,
    },
}

impl fmt::Display for CardsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CardsError::Io { path, source } => {
                write!(f, "cannot read '{}': {source}", path.display())
            }
            CardsError::Json { path, source } => {
                write!(
                    f,
                    "invalid JSON in '{}' at line {}, column {}",
                    path.display(),
                    source.line(),
                    source.column()
                )
            }
            CardsError::Schema(message)
            | CardsError::Duplicate(message)
            | CardsError::DeckReference(message) => f.write_str(message),
            CardsError::InvalidCard { card_id, message }
            | CardsError::InvalidResponse { card_id, message } => {
                write!(f, "card '{card_id}': {message}")
            }
        }
    }
}

impl std::error::Error for CardsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CardsError::Io { source, .. } => Some(source),
            CardsError::Json { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, CardsError> {
    let path_metadata = fs::symlink_metadata(path).map_err(|source| CardsError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(CardsError::Schema(format!(
            "private input '{}' must be a regular, non-symlink file",
            path.display()
        )));
    }
    if path_metadata.len() > MAX_INPUT_BYTES {
        return Err(CardsError::Schema(format!(
            "private input '{}' exceeds the {} byte limit",
            path.display(),
            MAX_INPUT_BYTES
        )));
    }
    let mut file = File::open(path).map_err(|source| CardsError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let opened_metadata = file.metadata().map_err(|source| CardsError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !opened_metadata.is_file() {
        return Err(CardsError::Schema(format!(
            "private input '{}' changed while it was opened",
            path.display()
        )));
    }
    #[cfg(unix)]
    if path_metadata.dev() != opened_metadata.dev()
        || path_metadata.ino() != opened_metadata.ino()
        || opened_metadata.nlink() != 1
    {
        return Err(CardsError::Schema(format!(
            "private input '{}' changed or has multiple hard links",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| CardsError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(CardsError::Schema(format!(
            "private input '{}' exceeds the {} byte limit",
            path.display(),
            MAX_INPUT_BYTES
        )));
    }
    serde_json::from_slice(&bytes).map_err(|source| CardsError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_nonempty(field: &str, value: &str) -> Result<(), CardsError> {
    if value.trim().is_empty() {
        Err(CardsError::Schema(format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}

fn validate_digest(field: &str, value: &str) -> Result<(), CardsError> {
    let valid = value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    if valid {
        Ok(())
    } else {
        Err(CardsError::Schema(format!(
            "{field} must be a lowercase sha256 digest"
        )))
    }
}

#[derive(Clone, Debug)]
struct ValidatedSourceCard {
    evidence_version: String,
    current_fen: String,
    position_id: String,
    orientation: String,
    check: bool,
    legal_moves: Vec<LegalChoice>,
    tags: Vec<String>,
    answer: HiddenAnswer,
}

fn validate_source_card(
    card: DiagnosticCard,
    analysis_config_version: &str,
) -> Result<ValidatedSourceCard, CardsError> {
    let invalid = |message: &str| CardsError::InvalidCard {
        card_id: card.card_id.clone(),
        message: message.to_string(),
    };
    if card.kind != "engine-diagnostic-move" {
        return Err(invalid("unsupported diagnostic card kind"));
    }
    if card.prompt.task != "select-move" {
        return Err(invalid("unsupported prompt task"));
    }
    if card.occurrence.decision_ply == 0
        || card.occurrence.history_uci.len() != card.occurrence.decision_ply - 1
    {
        return Err(invalid("decision ply does not match occurrence history"));
    }

    let computed_version = semantic_content_version(&card)?;
    if computed_version != card.content_version {
        return Err(invalid(
            "semantic content digest does not match content_version",
        ));
    }
    let computed_evidence_version = evidence_content_version(analysis_config_version, &card.answer);
    if computed_evidence_version != card.evidence_version {
        return Err(invalid(
            "answer evidence digest does not match evidence_version",
        ));
    }

    let mut game = Game::from_fen(&card.occurrence.initial_fen)
        .map_err(|_| invalid("initial FEN is invalid"))?;
    for uci in &card.occurrence.history_uci {
        if game.push_uci(uci).is_none() {
            return Err(invalid("occurrence history contains an illegal move"));
        }
    }
    let board = game.board();
    if board.to_fen() != card.position.current_fen {
        return Err(invalid("occurrence history does not reproduce the raw FEN"));
    }
    if board.position_id() != card.position.position_id {
        return Err(invalid(
            "occurrence history does not reproduce the PositionId",
        ));
    }
    let expected_orientation = match board.side_to_move() {
        Color::White => "white",
        Color::Black => "black",
    };
    if card.prompt.orientation != expected_orientation {
        return Err(invalid(
            "prompt orientation does not match the side to move",
        ));
    }

    let reference_move = choice(board, &card.answer.reference_move_uci)
        .ok_or_else(|| invalid("reference root is not legal"))?;
    let source_game_move = choice(board, &card.answer.played_move_uci)
        .ok_or_else(|| invalid("source-game root is not legal"))?;
    let reference_line = validate_line(
        board,
        &card.answer.reference_line_uci,
        &card.answer.reference_move_uci,
        "reference",
        &card.card_id,
    )?;
    let source_game_line = validate_line(
        board,
        &card.answer.played_line_uci,
        &card.answer.played_move_uci,
        "source-game",
        &card.card_id,
    )?;

    let legal_moves = board
        .legal_moves()
        .iter()
        .map(|&mv| LegalChoice {
            uci: mv.to_uci(),
            san: board.san(mv),
        })
        .collect();

    Ok(ValidatedSourceCard {
        evidence_version: card.evidence_version,
        current_fen: card.position.current_fen,
        position_id: card.position.position_id,
        orientation: card.prompt.orientation,
        check: board.in_check(),
        legal_moves,
        tags: card.tags,
        answer: HiddenAnswer {
            reference_move,
            source_game_move,
            reference_line,
            source_game_line,
            loss_cp_equivalent: card.answer.loss_cp_equivalent,
            loss_bucket: card.answer.loss_bucket,
            mate_event: card.answer.mate_event,
        },
    })
}

fn choice(board: &Board, uci: &str) -> Option<LegalChoice> {
    board.parse_uci(uci).map(|mv| LegalChoice {
        uci: mv.to_uci(),
        san: board.san(mv),
    })
}

fn validate_line(
    board: &Board,
    ucis: &[String],
    expected_root: &str,
    label: &str,
    card_id: &str,
) -> Result<Vec<RevealedLineMove>, CardsError> {
    if ucis.first().map(String::as_str) != Some(expected_root) {
        return Err(CardsError::InvalidCard {
            card_id: card_id.to_string(),
            message: format!("{label} line does not begin with its declared root"),
        });
    }
    let mut current = board.clone();
    let mut rendered = Vec::with_capacity(ucis.len());
    for uci in ucis {
        let mv = current
            .parse_uci(uci)
            .ok_or_else(|| CardsError::InvalidCard {
                card_id: card_id.to_string(),
                message: format!("{label} line contains an illegal move"),
            })?;
        rendered.push(RevealedLineMove {
            uci: mv.to_uci(),
            san: current.san(mv),
        });
        current.make_move(mv);
    }
    Ok(rendered)
}

/// Reproduce `json.dumps(..., ensure_ascii=False, separators=(",", ":"),
/// sort_keys=True)` for the JSON value types in the card contract.
fn canonical_json(value: &Value, output: &mut String) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => output.push_str(
            &serde_json::to_string(value).expect("serializing a JSON string cannot fail"),
        ),
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                canonical_json(value, output);
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                output.push_str(
                    &serde_json::to_string(key).expect("serializing a JSON key cannot fail"),
                );
                output.push(':');
                canonical_json(&values[key], output);
            }
            output.push('}');
        }
    }
}

fn semantic_content_version(card: &DiagnosticCard) -> Result<String, CardsError> {
    let value = json!({
        "kind": card.kind,
        "occurrence": card.occurrence,
        "position": card.position,
        "task": card.prompt.task,
        "orientation": card.prompt.orientation,
        "reference_move_uci": card.answer.reference_move_uci,
    });
    let mut canonical = String::new();
    canonical_json(&value, &mut canonical);
    let digest = Sha256::digest(canonical.as_bytes());
    let mut hexadecimal = String::with_capacity(64);
    use std::fmt::Write as _;
    for byte in digest {
        write!(&mut hexadecimal, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(format!("sha256:{hexadecimal}"))
}

/// Reproduce the generator's evidence binding exactly:
/// `sha256(canonical_json({analysis_config_version, answer}))`.
fn evidence_content_version(analysis_config_version: &str, answer: &DiagnosticAnswer) -> String {
    let value = json!({
        "analysis_config_version": analysis_config_version,
        "answer": answer,
    });
    let mut canonical = String::new();
    canonical_json(&value, &mut canonical);
    let digest = Sha256::digest(canonical.as_bytes());
    let mut hexadecimal = String::with_capacity(64);
    use std::fmt::Write as _;
    for byte in digest {
        write!(&mut hexadecimal, "{byte:02x}").expect("writing to a String cannot fail");
    }
    format!("sha256:{hexadecimal}")
}

#[derive(Debug, Deserialize)]
struct DiagnosticBundle {
    schema: String,
    schema_version: u32,
    analysis_config_version: String,
    analysis: BundleAnalysis,
    cards: Vec<DiagnosticCard>,
}

#[derive(Debug, Deserialize)]
struct BundleAnalysis {
    assurance: String,
    confirmation_nodes_per_position: u64,
}

#[derive(Debug, Deserialize)]
struct DiagnosticCard {
    card_id: String,
    content_version: String,
    evidence_version: String,
    kind: String,
    occurrence: Occurrence,
    position: Position,
    prompt: DiagnosticPrompt,
    answer: DiagnosticAnswer,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Occurrence {
    game_id: String,
    decision_ply: usize,
    initial_fen: String,
    history_uci: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Position {
    current_fen: String,
    position_id: String,
}

#[derive(Debug, Deserialize)]
struct DiagnosticPrompt {
    orientation: String,
    task: String,
    #[allow(dead_code)]
    text: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticAnswer {
    reference_move_uci: String,
    played_move_uci: String,
    reference_line_uci: Vec<String>,
    played_line_uci: Vec<String>,
    #[allow(dead_code)]
    reference_score_cp_equivalent: i64,
    #[allow(dead_code)]
    played_score_cp_equivalent: i64,
    loss_cp_equivalent: i64,
    loss_bucket: String,
    mate_event: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeckOverlay {
    schema: String,
    schema_version: u32,
    deck_id: String,
    title: String,
    new_per_day: usize,
    cards: Vec<DeckCard>,
}

#[derive(Debug, Deserialize)]
struct DeckCard {
    card_id: String,
    content_version: String,
    prompt: String,
    explanation: String,
    success_criterion: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    const CONTENT_VERSION: &str =
        "sha256:d9db83b89274a1b4bf1acb02c6f94ce2efe4c315be76822324158b6966ebff60";
    const ANALYSIS_CONFIG_VERSION: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const EVIDENCE_VERSION: &str =
        "sha256:f56f672546c56c6266ba77790c8dec7eeaf3f356b92411b1307ccbd06b47bfe1";

    fn fixture_card() -> Value {
        json!({
            "card_id": "engine-diagnostic/fixture-game/3",
            "content_version": CONTENT_VERSION,
            "evidence_version": EVIDENCE_VERSION,
            "kind": "engine-diagnostic-move",
            "occurrence": {
                "game_id": "fixture-game",
                "decision_ply": 3,
                "initial_fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "history_uci": ["e2e4", "e7e5"]
            },
            "position": {
                "current_fen": "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2",
                "position_id": "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq -"
            },
            "prompt": {"orientation": "white", "task": "select-move", "text": "What would you play?"},
            "answer": {
                "reference_move_uci": "g1f3",
                "played_move_uci": "f1c4",
                "reference_line_uci": ["g1f3", "b8c6"],
                "played_line_uci": ["f1c4", "g8f6"],
                "reference_score_cp_equivalent": 30,
                "played_score_cp_equivalent": 0,
                "loss_cp_equivalent": 30,
                "loss_bucket": "small",
                "mate_event": null
            },
            "tags": ["opening", "personal-game"]
        })
    }

    fn fixture_bundle(card: Value) -> DiagnosticBundle {
        serde_json::from_value(json!({
            "schema": BUNDLE_SCHEMA,
            "schema_version": 1,
            "analysis_config_version": ANALYSIS_CONFIG_VERSION,
            "analysis": {
                "assurance": "bounded-engine-estimate",
                "confirmation_nodes_per_position": 100000
            },
            "cards": [card]
        }))
        .unwrap()
    }

    fn fixture_deck() -> DeckOverlay {
        serde_json::from_value(json!({
            "schema": DECK_SCHEMA,
            "schema_version": 1,
            "deck_id": "fixture-six/v1",
            "title": "Fixture deck",
            "new_per_day": 1,
            "cards": [{
                "card_id": "engine-diagnostic/fixture-game/3",
                "content_version": CONTENT_VERSION,
                "prompt": "Develop while controlling the center.",
                "explanation": "The pinned reference develops the knight.",
                "success_criterion": "Commit to a legal move and explain its purpose.",
                "tags": ["development"]
            }]
        }))
        .unwrap()
    }

    #[test]
    fn semantic_digest_matches_python_canonical_json() {
        let card: DiagnosticCard = serde_json::from_value(fixture_card()).unwrap();
        assert_eq!(semantic_content_version(&card).unwrap(), CONTENT_VERSION);
    }

    #[test]
    fn evidence_digest_matches_python_canonical_json() {
        let card: DiagnosticCard = serde_json::from_value(fixture_card()).unwrap();
        assert_eq!(
            evidence_content_version(ANALYSIS_CONFIG_VERSION, &card.answer),
            EVIDENCE_VERSION
        );
    }

    #[test]
    fn catalog_replays_and_exposes_only_curated_prompt() {
        let catalog = Catalog::from_inputs(fixture_bundle(fixture_card()), fixture_deck()).unwrap();
        assert_eq!(catalog.deck_id, "fixture-six/v1");
        assert_eq!(catalog.cards().len(), 1);
        let card = &catalog.cards()[0];
        let prompt_json = serde_json::to_string(&card.prompt_view()).unwrap();
        assert!(!prompt_json.contains("referenceMove"));
        assert!(!prompt_json.contains("pinned reference"));
        assert!(!prompt_json.contains("contentVersion"));
        assert!(!prompt_json.contains("evidenceVersion"));
        assert!(!prompt_json.contains("cardId"));
        assert!(prompt_json.contains("Develop while controlling"));
        assert!(card.tags.contains(&"development".to_string()));

        let feedback = card.feedback("g1f3").unwrap();
        assert_eq!(feedback.reference_match, Some(true));
        assert_eq!(feedback.played_move.as_ref().unwrap().uci, "g1f3");
        assert_eq!(feedback.reference_move.san, "Nf3");
        assert_eq!(feedback.reference_line[1].san, "Nc6");
        assert!(card.feedback("a1a8").is_err());
        let gave_up = card.give_up_feedback();
        assert_eq!(gave_up.played_move, None);
        assert_eq!(gave_up.reference_match, None);
        assert_eq!(gave_up.reference_move.uci, "g1f3");
    }

    #[test]
    fn rejects_replay_and_deck_reference_mismatches() {
        let mut bad_fen = fixture_card();
        bad_fen["position"]["current_fen"] = Value::String(
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 2".to_string(),
        );
        assert!(matches!(
            Catalog::from_inputs(fixture_bundle(bad_fen), fixture_deck()),
            Err(CardsError::InvalidCard { .. })
        ));

        let mut deck = fixture_deck();
        deck.cards[0].content_version =
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
        assert!(matches!(
            Catalog::from_inputs(fixture_bundle(fixture_card()), deck),
            Err(CardsError::DeckReference(_))
        ));
    }

    #[test]
    fn rejects_answer_mutations_with_stale_evidence_digest() {
        let mutations: Vec<Box<dyn Fn(&mut Value)>> = vec![
            Box::new(|card| {
                card["answer"]["reference_score_cp_equivalent"] = json!(31);
            }),
            Box::new(|card| {
                card["answer"]["loss_cp_equivalent"] = json!(31);
            }),
            Box::new(|card| {
                card["answer"]["reference_line_uci"] = json!(["g1f3", "g8f6"]);
            }),
        ];
        for mutate in mutations {
            let mut card = fixture_card();
            mutate(&mut card);
            let error = Catalog::from_inputs(fixture_bundle(card), fixture_deck()).unwrap_err();
            assert!(
                matches!(
                    error,
                    CardsError::InvalidCard { ref message, .. }
                        if message == "answer evidence digest does not match evidence_version"
                ),
                "unexpected import error: {error}"
            );
        }
    }

    #[test]
    fn evidence_version_tracks_config_without_resetting_semantic_content() {
        const NEW_CONFIG: &str =
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        const NEW_EVIDENCE: &str =
            "sha256:ed87dfdb6f7b95687cab7c02b4811a6d3fc00c7bd4d90665942712d3a53e006f";

        let mut stale_bundle = fixture_bundle(fixture_card());
        stale_bundle.analysis_config_version = NEW_CONFIG.to_string();
        assert!(matches!(
            Catalog::from_inputs(stale_bundle, fixture_deck()),
            Err(CardsError::InvalidCard { ref message, .. })
                if message == "answer evidence digest does not match evidence_version"
        ));

        let mut refreshed = fixture_card();
        refreshed["evidence_version"] = json!(NEW_EVIDENCE);
        let mut refreshed_bundle = fixture_bundle(refreshed);
        refreshed_bundle.analysis_config_version = NEW_CONFIG.to_string();
        let catalog = Catalog::from_inputs(refreshed_bundle, fixture_deck()).unwrap();
        assert_eq!(catalog.cards()[0].content_version, CONTENT_VERSION);
        assert_eq!(catalog.cards()[0].evidence_version, NEW_EVIDENCE);
    }

    #[test]
    fn load_preserves_deck_order() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "chess-trainer-cards-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let bundle_path = dir.join("bundle.json");
        let deck_path = dir.join("deck.json");
        fs::write(
            &bundle_path,
            serde_json::to_vec(&json!({
                "schema": BUNDLE_SCHEMA,
                "schema_version": 1,
                "analysis_config_version": ANALYSIS_CONFIG_VERSION,
                "analysis": {
                    "assurance": "bounded-engine-estimate",
                    "confirmation_nodes_per_position": 100000
                },
                "cards": [fixture_card()]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            &deck_path,
            serde_json::to_vec(&json!({
                "schema": DECK_SCHEMA,
                "schema_version": 1,
                "deck_id": "fixture-six/v1",
                "title": "Fixture deck",
                "new_per_day": 1,
                "cards": [{
                    "card_id": "engine-diagnostic/fixture-game/3",
                    "content_version": CONTENT_VERSION,
                    "prompt": "Prompt",
                    "explanation": "Explanation",
                    "success_criterion": "Criterion",
                    "tags": []
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let catalog = Catalog::load(&bundle_path, &deck_path).unwrap();
        assert_eq!(
            catalog.cards()[0].card_id,
            "engine-diagnostic/fixture-game/3"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
