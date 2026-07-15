//! Durable, local review scheduling.
//!
//! The append-only JSONL log is the source of truth.  [`ReviewState`] and
//! [`ReviewSummary`] are projections rebuilt by replay, so scheduler changes
//! remain auditable and a process restart cannot silently change progress.

use crate::cards::CardFeedback;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

const EVENT_SCHEMA: &str = "chess-trainer-review-event";
const REVEAL_SCHEMA: &str = "chess-trainer-reveal-event";
const EVENT_SCHEMA_VERSION: u32 = 1;
pub const SCHEDULER_VERSION: &str = "fixed-2-4-7-v1";
const SECOND_MS: i64 = 1_000;
const MINUTE_MS: i64 = 60 * SECOND_MS;
const DAY_MS: i64 = 24 * 60 * MINUTE_MS;
const MISS_INTERVAL_MS: i64 = 10 * MINUTE_MS;
const PARTIAL_INTERVAL_MS: i64 = DAY_MS;
const PASS_INTERVAL_DAYS: [i64; 6] = [2, 4, 7, 14, 30, 60];
const MATURE_INTERVAL_MS: i64 = 30 * DAY_MS;
const MAX_EVENT_ID_BYTES: usize = 128;
const MAX_RESPONSE_BYTES: usize = 32;
const MAX_LATENCY_MS: u64 = 24 * 60 * 60 * 1_000;

#[derive(Deserialize)]
struct RecordHeader {
    schema: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewKey {
    pub card_id: String,
    pub content_version: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewGrade {
    Pass,
    Partial,
    Miss,
}

/// The observation supplied by the trusted local HTTP handler.  The handler,
/// rather than the browser, supplies `reviewed_at_unix_ms`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewInput {
    pub event_id: String,
    pub key: ReviewKey,
    pub evidence_version: String,
    pub shown_at_unix_ms: i64,
    pub reviewed_at_unix_ms: i64,
    pub grade: ReviewGrade,
    pub hint_used: bool,
    pub response_uci: Option<String>,
    pub reference_match: Option<bool>,
    pub latency_ms: u64,
}

/// A server-timed answer release written durably before feedback is returned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevealInput {
    pub event_id: String,
    pub key: ReviewKey,
    pub evidence_version: String,
    pub shown_at_unix_ms: i64,
    pub revealed_at_unix_ms: i64,
    pub hint_used: bool,
    pub response_uci: Option<String>,
    pub reference_match: Option<bool>,
    pub latency_ms: u64,
    pub feedback: CardFeedback,
    pub assurance: String,
    pub confirmation_nodes_per_position: u64,
    pub analysis_config_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealEvent {
    pub schema: String,
    pub schema_version: u32,
    pub sequence: u64,
    pub event_id: String,
    pub card_id: String,
    pub content_version: String,
    pub evidence_version: String,
    pub shown_at_unix_ms: i64,
    pub revealed_at_unix_ms: i64,
    pub hint_used: bool,
    pub response_uci: Option<String>,
    pub reference_match: Option<bool>,
    pub latency_ms: u64,
    pub feedback: CardFeedback,
    pub assurance: String,
    pub confirmation_nodes_per_position: u64,
    pub analysis_config_version: String,
    pub prior_review_sequence: Option<u64>,
}

impl RevealEvent {
    fn input(&self) -> RevealInput {
        RevealInput {
            event_id: self.event_id.clone(),
            key: ReviewKey {
                card_id: self.card_id.clone(),
                content_version: self.content_version.clone(),
            },
            evidence_version: self.evidence_version.clone(),
            shown_at_unix_ms: self.shown_at_unix_ms,
            revealed_at_unix_ms: self.revealed_at_unix_ms,
            hint_used: self.hint_used,
            response_uci: self.response_uci.clone(),
            reference_match: self.reference_match,
            latency_ms: self.latency_ms,
            feedback: self.feedback.clone(),
            assurance: self.assurance.clone(),
            confirmation_nodes_per_position: self.confirmation_nodes_per_position,
            analysis_config_version: self.analysis_config_version.clone(),
        }
    }
}

/// One immutable persisted observation plus the scheduler decision it caused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewEvent {
    pub schema: String,
    pub schema_version: u32,
    pub scheduler_version: String,
    pub sequence: u64,
    pub event_id: String,
    pub card_id: String,
    pub content_version: String,
    pub evidence_version: String,
    pub shown_at_unix_ms: i64,
    pub reviewed_at_unix_ms: i64,
    pub submitted_grade: ReviewGrade,
    pub applied_grade: ReviewGrade,
    pub hint_used: bool,
    pub response_uci: Option<String>,
    pub reference_match: Option<bool>,
    pub latency_ms: u64,
    pub prior_due_at_unix_ms: Option<i64>,
    pub prior_interval_ms: i64,
    pub was_due: bool,
    pub delayed_eligible: bool,
    pub next_due_at_unix_ms: i64,
    pub next_interval_ms: i64,
    pub next_success_rung: usize,
    pub next_lapses: u64,
}

impl ReviewEvent {
    fn input(&self) -> ReviewInput {
        ReviewInput {
            event_id: self.event_id.clone(),
            key: ReviewKey {
                card_id: self.card_id.clone(),
                content_version: self.content_version.clone(),
            },
            evidence_version: self.evidence_version.clone(),
            shown_at_unix_ms: self.shown_at_unix_ms,
            reviewed_at_unix_ms: self.reviewed_at_unix_ms,
            grade: self.submitted_grade,
            hint_used: self.hint_used,
            response_uci: self.response_uci.clone(),
            reference_match: self.reference_match,
            latency_ms: self.latency_ms,
        }
    }

    fn matches_input(&self, input: &ReviewInput) -> bool {
        self.input() == *input
    }
}

/// Current projection for one semantic card version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewState {
    pub attempts: u64,
    pub passes: u64,
    pub partials: u64,
    pub misses: u64,
    pub hints: u64,
    pub lapses: u64,
    pub reference_matches: u64,
    pub delayed_attempts: u64,
    pub delayed_passes: u64,
    pub success_rung: usize,
    pub interval_ms: i64,
    pub due_at_unix_ms: i64,
    pub last_reviewed_at_unix_ms: i64,
    pub last_grade: ReviewGrade,
    pub last_evidence_version: String,
    pub last_event_sequence: u64,
}

/// Aggregate over the active deck.  “Mature” means only that the current
/// interval is at least 30 days; it is deliberately not called mastery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSummary {
    pub active_cards: usize,
    pub new_cards: usize,
    pub reviewed_cards: usize,
    pub due_cards: usize,
    pub mature_cards: usize,
    pub attempts: u64,
    pub hints: u64,
    pub lapses: u64,
    pub reference_matches: u64,
    pub delayed_attempts: u64,
    pub delayed_passes: u64,
    pub next_due_at_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordReceipt {
    pub event: ReviewEvent,
    /// False means an identical event ID was already durable.
    pub inserted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevealReceipt {
    pub event: RevealEvent,
    pub inserted: bool,
}

pub struct ReviewStore {
    path: PathBuf,
    file: File,
    events: Vec<ReviewEvent>,
    event_ids: BTreeMap<String, usize>,
    reveals: BTreeMap<String, RevealEvent>,
    states: BTreeMap<ReviewKey, ReviewState>,
    next_sequence: u64,
    poisoned: bool,
}

impl ReviewStore {
    /// Open or create a mode-0600 log.  A single unterminated final record is
    /// treated as an interrupted append and truncated; corruption in any
    /// newline-terminated record is fatal.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ReviewError> {
        let path = path.as_ref().to_path_buf();
        let mut file = open_private_log(&path)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => {
                return Err(ReviewError::Locked(path));
            }
            Err(fs::TryLockError::Error(source)) => {
                return Err(ReviewError::Io { path, source });
            }
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|source| ReviewError::Io {
                path: path.clone(),
                source,
            })?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|source| ReviewError::Io {
                path: path.clone(),
                source,
            })?;

        let complete_len = if bytes.is_empty() || bytes.ends_with(b"\n") {
            bytes.len()
        } else {
            bytes
                .iter()
                .rposition(|&byte| byte == b'\n')
                .map_or(0, |index| index + 1)
        };

        let mut events = Vec::new();
        let mut event_ids = BTreeMap::new();
        let mut reveals = BTreeMap::new();
        let mut states = BTreeMap::new();
        let mut next_sequence = 1u64;
        let complete = &bytes[..complete_len];
        for (line_index, line) in complete.split(|&byte| byte == b'\n').enumerate() {
            if line.is_empty() {
                if line_index + 1 == complete.split(|&byte| byte == b'\n').count() {
                    continue;
                }
                return Err(ReviewError::Corrupt {
                    path: path.clone(),
                    line: line_index + 1,
                    message: "blank records are not allowed".to_string(),
                });
            }
            let header: RecordHeader =
                serde_json::from_slice(line).map_err(|error| ReviewError::Corrupt {
                    path: path.clone(),
                    line: line_index + 1,
                    message: format!("invalid JSON at column {}", error.column()),
                })?;
            match header.schema.as_str() {
                EVENT_SCHEMA => {
                    let event: ReviewEvent =
                        serde_json::from_slice(line).map_err(|error| ReviewError::Corrupt {
                            path: path.clone(),
                            line: line_index + 1,
                            message: format!("invalid review JSON at column {}", error.column()),
                        })?;
                    validate_loaded_event(&event, next_sequence, &states).map_err(|message| {
                        ReviewError::Corrupt {
                            path: path.clone(),
                            line: line_index + 1,
                            message,
                        }
                    })?;
                    if let Some(reveal) = reveals.get(&event.event_id) {
                        validate_review_matches_reveal(&event.input(), reveal).map_err(
                            |message| ReviewError::Corrupt {
                                path: path.clone(),
                                line: line_index + 1,
                                message,
                            },
                        )?;
                        let current_sequence = states
                            .get(&ReviewKey {
                                card_id: reveal.card_id.clone(),
                                content_version: reveal.content_version.clone(),
                            })
                            .map(|state| state.last_event_sequence);
                        if current_sequence != reveal.prior_review_sequence {
                            return Err(ReviewError::Corrupt {
                                path: path.clone(),
                                line: line_index + 1,
                                message: "review was recorded from a stale answer release"
                                    .to_string(),
                            });
                        }
                    }
                    if event_ids
                        .insert(event.event_id.clone(), events.len())
                        .is_some()
                    {
                        return Err(ReviewError::Corrupt {
                            path: path.clone(),
                            line: line_index + 1,
                            message: "duplicate review event id".to_string(),
                        });
                    }
                    apply_event(&mut states, &event);
                    events.push(event);
                }
                REVEAL_SCHEMA => {
                    let reveal: RevealEvent =
                        serde_json::from_slice(line).map_err(|error| ReviewError::Corrupt {
                            path: path.clone(),
                            line: line_index + 1,
                            message: format!("invalid reveal JSON at column {}", error.column()),
                        })?;
                    validate_loaded_reveal(&reveal, next_sequence, &states).map_err(|message| {
                        ReviewError::Corrupt {
                            path: path.clone(),
                            line: line_index + 1,
                            message,
                        }
                    })?;
                    if event_ids.contains_key(&reveal.event_id) {
                        return Err(ReviewError::Corrupt {
                            path: path.clone(),
                            line: line_index + 1,
                            message: "answer release appears after its review event".to_string(),
                        });
                    }
                    if reveals.insert(reveal.event_id.clone(), reveal).is_some() {
                        return Err(ReviewError::Corrupt {
                            path: path.clone(),
                            line: line_index + 1,
                            message: "duplicate reveal event id".to_string(),
                        });
                    }
                }
                other => {
                    return Err(ReviewError::Corrupt {
                        path: path.clone(),
                        line: line_index + 1,
                        message: format!("unknown log record schema '{other}'"),
                    });
                }
            }
            next_sequence += 1;
        }

        if complete_len != bytes.len() {
            file.set_len(complete_len as u64)
                .and_then(|()| file.sync_data())
                .map_err(|source| ReviewError::Io {
                    path: path.clone(),
                    source,
                })?;
        }
        file.seek(SeekFrom::End(0))
            .map_err(|source| ReviewError::Io {
                path: path.clone(),
                source,
            })?;

        Ok(ReviewStore {
            path,
            file,
            next_sequence,
            events,
            event_ids,
            reveals,
            states,
            poisoned: false,
        })
    }

    pub fn events(&self) -> &[ReviewEvent] {
        &self.events
    }

    #[cfg(test)]
    pub fn states(&self) -> &BTreeMap<ReviewKey, ReviewState> {
        &self.states
    }

    pub fn state(&self, key: &ReviewKey) -> Option<&ReviewState> {
        self.states.get(key)
    }

    pub fn reveals(&self) -> impl Iterator<Item = &RevealEvent> {
        self.reveals.values()
    }

    pub fn pending_reveals(&self) -> impl Iterator<Item = &RevealEvent> {
        self.reveals
            .values()
            .filter(|reveal| !self.event_ids.contains_key(&reveal.event_id))
    }

    pub fn event(&self, event_id: &str) -> Option<&ReviewEvent> {
        self.event_ids
            .get(event_id)
            .map(|&index| &self.events[index])
    }

    pub fn has_reveal(&self, event_id: &str) -> bool {
        self.reveals.contains_key(event_id)
    }

    pub fn record_reveal(&mut self, input: RevealInput) -> Result<RevealReceipt, ReviewError> {
        validate_reveal_input(&input).map_err(ReviewError::InvalidInput)?;
        if self.poisoned {
            return Err(ReviewError::Poisoned(self.path.clone()));
        }
        if let Some(event) = self.reveals.get(&input.event_id) {
            if event.input() == input {
                return Ok(RevealReceipt {
                    event: event.clone(),
                    inserted: false,
                });
            }
            return Err(ReviewError::EventIdConflict(input.event_id));
        }
        if self.event_ids.contains_key(&input.event_id) {
            return Err(ReviewError::EventIdConflict(input.event_id));
        }

        let event = derive_reveal(self.next_sequence, &self.states, input);
        let mut encoded = serde_json::to_vec(&event).map_err(ReviewError::Serialize)?;
        encoded.push(b'\n');
        if let Err(source) = self
            .file
            .write_all(&encoded)
            .and_then(|()| self.file.flush())
            .and_then(|()| self.file.sync_data())
        {
            self.poisoned = true;
            return Err(ReviewError::Io {
                path: self.path.clone(),
                source,
            });
        }
        self.reveals.insert(event.event_id.clone(), event.clone());
        self.next_sequence += 1;
        Ok(RevealReceipt {
            event,
            inserted: true,
        })
    }

    pub fn record(&mut self, input: ReviewInput) -> Result<RecordReceipt, ReviewError> {
        validate_input(&input).map_err(ReviewError::InvalidInput)?;
        if self.poisoned {
            return Err(ReviewError::Poisoned(self.path.clone()));
        }
        if let Some(&index) = self.event_ids.get(&input.event_id) {
            let event = &self.events[index];
            if event.matches_input(&input) {
                return Ok(RecordReceipt {
                    event: event.clone(),
                    inserted: false,
                });
            }
            return Err(ReviewError::EventIdConflict(input.event_id));
        }
        if let Some(reveal) = self.reveals.get(&input.event_id) {
            validate_review_matches_reveal(&input, reveal).map_err(ReviewError::InvalidInput)?;
            let current_sequence = self
                .states
                .get(&input.key)
                .map(|state| state.last_event_sequence);
            if current_sequence != reveal.prior_review_sequence {
                return Err(ReviewError::InvalidInput(
                    "review was submitted from a stale answer release".to_string(),
                ));
            }
        }

        let event = derive_event(self.next_sequence, &self.states, input);
        let mut encoded = serde_json::to_vec(&event).map_err(ReviewError::Serialize)?;
        encoded.push(b'\n');
        if let Err(source) = self
            .file
            .write_all(&encoded)
            .and_then(|()| self.file.flush())
            .and_then(|()| self.file.sync_data())
        {
            self.poisoned = true;
            return Err(ReviewError::Io {
                path: self.path.clone(),
                source,
            });
        }

        let index = self.events.len();
        self.event_ids.insert(event.event_id.clone(), index);
        apply_event(&mut self.states, &event);
        self.events.push(event.clone());
        self.next_sequence += 1;
        Ok(RecordReceipt {
            event,
            inserted: true,
        })
    }

    pub fn summary(&self, active_keys: &[ReviewKey], now_unix_ms: i64) -> ReviewSummary {
        let unique: BTreeSet<_> = active_keys.iter().collect();
        let mut summary = ReviewSummary {
            active_cards: unique.len(),
            new_cards: 0,
            reviewed_cards: 0,
            due_cards: 0,
            mature_cards: 0,
            attempts: 0,
            hints: 0,
            lapses: 0,
            reference_matches: 0,
            delayed_attempts: 0,
            delayed_passes: 0,
            next_due_at_unix_ms: None,
        };
        for key in unique {
            match self.states.get(key) {
                None => {
                    summary.new_cards += 1;
                    summary.due_cards += 1;
                }
                Some(state) => {
                    summary.reviewed_cards += 1;
                    summary.attempts += state.attempts;
                    summary.hints += state.hints;
                    summary.lapses += state.lapses;
                    summary.reference_matches += state.reference_matches;
                    summary.delayed_attempts += state.delayed_attempts;
                    summary.delayed_passes += state.delayed_passes;
                    if state.interval_ms >= MATURE_INTERVAL_MS {
                        summary.mature_cards += 1;
                    }
                    if state.due_at_unix_ms <= now_unix_ms {
                        summary.due_cards += 1;
                    } else {
                        summary.next_due_at_unix_ms = Some(
                            summary
                                .next_due_at_unix_ms
                                .map_or(state.due_at_unix_ms, |due| due.min(state.due_at_unix_ms)),
                        );
                    }
                }
            }
        }
        summary
    }
}

#[derive(Debug)]
pub enum ReviewError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Corrupt {
        path: PathBuf,
        line: usize,
        message: String,
    },
    InvalidInput(String),
    EventIdConflict(String),
    Locked(PathBuf),
    Serialize(serde_json::Error),
    Poisoned(PathBuf),
    UnsafePath(String),
}

impl fmt::Display for ReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewError::Io { path, source } => {
                write!(f, "review log '{}': {source}", path.display())
            }
            ReviewError::Corrupt {
                path,
                line,
                message,
            } => {
                write!(f, "review log '{}' line {line}: {message}", path.display())
            }
            ReviewError::InvalidInput(message) => write!(f, "invalid review: {message}"),
            ReviewError::EventIdConflict(event_id) => {
                write!(
                    f,
                    "event id '{event_id}' was already used for a different review"
                )
            }
            ReviewError::Locked(path) => write!(
                f,
                "review log '{}' is already open by another trainer process",
                path.display()
            ),
            ReviewError::Serialize(source) => write!(f, "cannot serialize review event: {source}"),
            ReviewError::Poisoned(path) => write!(
                f,
                "review log '{}' had an interrupted write; reopen it before recording more reviews",
                path.display()
            ),
            ReviewError::UnsafePath(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ReviewError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReviewError::Io { source, .. } => Some(source),
            ReviewError::Serialize(source) => Some(source),
            _ => None,
        }
    }
}

fn validate_input(input: &ReviewInput) -> Result<(), String> {
    if input.event_id.is_empty()
        || input.event_id.len() > MAX_EVENT_ID_BYTES
        || input.event_id.chars().any(char::is_control)
    {
        return Err("event_id is empty, too long, or contains a control character".to_string());
    }
    if input.key.card_id.trim().is_empty() || input.key.content_version.trim().is_empty() {
        return Err("card identity must not be empty".to_string());
    }
    if input.evidence_version.trim().is_empty() {
        return Err("evidence_version must not be empty".to_string());
    }
    if input.shown_at_unix_ms < 0 || input.reviewed_at_unix_ms < input.shown_at_unix_ms {
        return Err("review timestamps are invalid or out of order".to_string());
    }
    if input.latency_ms > MAX_LATENCY_MS {
        return Err("latency exceeds 24 hours".to_string());
    }
    if let Some(response) = &input.response_uci
        && (response.is_empty()
            || response.len() > MAX_RESPONSE_BYTES
            || response.chars().any(char::is_control))
    {
        return Err("response_uci is empty, too long, or contains a control character".to_string());
    }
    Ok(())
}

fn validate_reveal_input(input: &RevealInput) -> Result<(), String> {
    if input.event_id.is_empty()
        || input.event_id.len() > MAX_EVENT_ID_BYTES
        || input.event_id.chars().any(char::is_control)
    {
        return Err("event_id is empty, too long, or contains a control character".to_string());
    }
    if input.key.card_id.trim().is_empty() || input.key.content_version.trim().is_empty() {
        return Err("card identity must not be empty".to_string());
    }
    if input.evidence_version.trim().is_empty() {
        return Err("evidence_version must not be empty".to_string());
    }
    if input.shown_at_unix_ms < 0 || input.revealed_at_unix_ms < input.shown_at_unix_ms {
        return Err("reveal timestamps are invalid or out of order".to_string());
    }
    if input.latency_ms > MAX_LATENCY_MS {
        return Err("latency exceeds 24 hours".to_string());
    }
    if let Some(response) = &input.response_uci
        && (response.is_empty()
            || response.len() > MAX_RESPONSE_BYTES
            || response.chars().any(char::is_control))
    {
        return Err("response_uci is empty, too long, or contains a control character".to_string());
    }
    if input.hint_used {
        if input.response_uci.is_some() || input.reference_match.is_some() {
            return Err(
                "a give-up reveal must not claim a response or reference match".to_string(),
            );
        }
        if input.feedback.played_move.is_some() || input.feedback.reference_match.is_some() {
            return Err("give-up feedback must not claim a played move or match".to_string());
        }
    } else if input.response_uci.is_none() || input.reference_match.is_none() {
        return Err("a committed reveal must record its response and reference match".to_string());
    } else if input
        .feedback
        .played_move
        .as_ref()
        .map(|choice| choice.uci.as_str())
        != input.response_uci.as_deref()
        || input.feedback.reference_match != input.reference_match
    {
        return Err("committed feedback does not match the recorded response".to_string());
    }
    if input.feedback.evidence_version != input.evidence_version {
        return Err("feedback evidence version does not match the reveal".to_string());
    }
    if input.assurance.trim().is_empty() || input.analysis_config_version.trim().is_empty() {
        return Err("answer-release provenance must not be empty".to_string());
    }
    if input.confirmation_nodes_per_position == 0 {
        return Err("answer-release confirmation nodes must be positive".to_string());
    }
    Ok(())
}

fn derive_reveal(
    sequence: u64,
    states: &BTreeMap<ReviewKey, ReviewState>,
    input: RevealInput,
) -> RevealEvent {
    let prior_review_sequence = states
        .get(&input.key)
        .map(|state| state.last_event_sequence);
    RevealEvent {
        schema: REVEAL_SCHEMA.to_string(),
        schema_version: EVENT_SCHEMA_VERSION,
        sequence,
        event_id: input.event_id,
        card_id: input.key.card_id,
        content_version: input.key.content_version,
        evidence_version: input.evidence_version,
        shown_at_unix_ms: input.shown_at_unix_ms,
        revealed_at_unix_ms: input.revealed_at_unix_ms,
        hint_used: input.hint_used,
        response_uci: input.response_uci,
        reference_match: input.reference_match,
        latency_ms: input.latency_ms,
        feedback: input.feedback,
        assurance: input.assurance,
        confirmation_nodes_per_position: input.confirmation_nodes_per_position,
        analysis_config_version: input.analysis_config_version,
        prior_review_sequence,
    }
}

fn validate_loaded_reveal(
    event: &RevealEvent,
    expected_sequence: u64,
    states: &BTreeMap<ReviewKey, ReviewState>,
) -> Result<(), String> {
    if event.schema != REVEAL_SCHEMA || event.schema_version != EVENT_SCHEMA_VERSION {
        return Err(format!(
            "expected {REVEAL_SCHEMA} schema version {EVENT_SCHEMA_VERSION}"
        ));
    }
    if event.sequence != expected_sequence {
        return Err(format!(
            "expected sequence {expected_sequence}, found {}",
            event.sequence
        ));
    }
    let input = event.input();
    validate_reveal_input(&input)?;
    if *event != derive_reveal(expected_sequence, states, input) {
        return Err("stored reveal does not match deterministic replay".to_string());
    }
    Ok(())
}

fn validate_review_matches_reveal(input: &ReviewInput, reveal: &RevealEvent) -> Result<(), String> {
    if input.event_id != reveal.event_id
        || input.key.card_id != reveal.card_id
        || input.key.content_version != reveal.content_version
        || input.evidence_version != reveal.evidence_version
        || input.shown_at_unix_ms != reveal.shown_at_unix_ms
        || input.hint_used != reveal.hint_used
        || input.response_uci != reveal.response_uci
        || input.reference_match != reveal.reference_match
        || input.latency_ms != reveal.latency_ms
        || input.reviewed_at_unix_ms < reveal.revealed_at_unix_ms
    {
        return Err("review does not match its durable answer release".to_string());
    }
    Ok(())
}

fn derive_event(
    sequence: u64,
    states: &BTreeMap<ReviewKey, ReviewState>,
    input: ReviewInput,
) -> ReviewEvent {
    let prior = states.get(&input.key);
    let prior_due_at_unix_ms = prior.map(|state| state.due_at_unix_ms);
    let prior_interval_ms = prior.map_or(0, |state| state.interval_ms);
    let prior_rung = prior.map_or(0, |state| state.success_rung);
    let prior_lapses = prior.map_or(0, |state| state.lapses);
    let was_due = prior_due_at_unix_ms.is_none_or(|due| input.reviewed_at_unix_ms >= due);
    let delayed_eligible = prior_interval_ms >= DAY_MS && was_due;
    let applied_grade = if input.hint_used {
        ReviewGrade::Miss
    } else {
        input.grade
    };

    let (next_interval_ms, next_success_rung, next_lapses) = match applied_grade {
        ReviewGrade::Pass => {
            let interval_index = prior_rung.min(PASS_INTERVAL_DAYS.len() - 1);
            let next_rung = (prior_rung + 1).min(PASS_INTERVAL_DAYS.len());
            (
                PASS_INTERVAL_DAYS[interval_index] * DAY_MS,
                next_rung,
                prior_lapses,
            )
        }
        ReviewGrade::Partial => (PARTIAL_INTERVAL_MS, 0, prior_lapses),
        ReviewGrade::Miss => (MISS_INTERVAL_MS, 0, prior_lapses.saturating_add(1)),
    };
    let next_due_at_unix_ms = input.reviewed_at_unix_ms.saturating_add(next_interval_ms);

    ReviewEvent {
        schema: EVENT_SCHEMA.to_string(),
        schema_version: EVENT_SCHEMA_VERSION,
        scheduler_version: SCHEDULER_VERSION.to_string(),
        sequence,
        event_id: input.event_id,
        card_id: input.key.card_id,
        content_version: input.key.content_version,
        evidence_version: input.evidence_version,
        shown_at_unix_ms: input.shown_at_unix_ms,
        reviewed_at_unix_ms: input.reviewed_at_unix_ms,
        submitted_grade: input.grade,
        applied_grade,
        hint_used: input.hint_used,
        response_uci: input.response_uci,
        reference_match: input.reference_match,
        latency_ms: input.latency_ms,
        prior_due_at_unix_ms,
        prior_interval_ms,
        was_due,
        delayed_eligible,
        next_due_at_unix_ms,
        next_interval_ms,
        next_success_rung,
        next_lapses,
    }
}

fn validate_loaded_event(
    event: &ReviewEvent,
    expected_sequence: u64,
    states: &BTreeMap<ReviewKey, ReviewState>,
) -> Result<(), String> {
    if event.schema != EVENT_SCHEMA || event.schema_version != EVENT_SCHEMA_VERSION {
        return Err(format!(
            "expected {EVENT_SCHEMA} schema version {EVENT_SCHEMA_VERSION}"
        ));
    }
    if event.scheduler_version != SCHEDULER_VERSION {
        return Err(format!(
            "unsupported scheduler version '{}'",
            event.scheduler_version
        ));
    }
    if event.sequence != expected_sequence {
        return Err(format!(
            "expected sequence {expected_sequence}, found {}",
            event.sequence
        ));
    }
    let input = event.input();
    validate_input(&input)?;
    let expected = derive_event(expected_sequence, states, input);
    if *event != expected {
        return Err("stored scheduler decision does not match deterministic replay".to_string());
    }
    Ok(())
}

fn apply_event(states: &mut BTreeMap<ReviewKey, ReviewState>, event: &ReviewEvent) {
    let key = ReviewKey {
        card_id: event.card_id.clone(),
        content_version: event.content_version.clone(),
    };
    let state = states.entry(key).or_insert_with(|| ReviewState {
        attempts: 0,
        passes: 0,
        partials: 0,
        misses: 0,
        hints: 0,
        lapses: 0,
        reference_matches: 0,
        delayed_attempts: 0,
        delayed_passes: 0,
        success_rung: 0,
        interval_ms: 0,
        due_at_unix_ms: event.reviewed_at_unix_ms,
        last_reviewed_at_unix_ms: event.reviewed_at_unix_ms,
        last_grade: event.applied_grade,
        last_evidence_version: event.evidence_version.clone(),
        last_event_sequence: event.sequence,
    });
    state.attempts += 1;
    match event.applied_grade {
        ReviewGrade::Pass => state.passes += 1,
        ReviewGrade::Partial => state.partials += 1,
        ReviewGrade::Miss => state.misses += 1,
    }
    if event.hint_used {
        state.hints += 1;
    }
    if event.reference_match == Some(true) {
        state.reference_matches += 1;
    }
    if event.delayed_eligible {
        state.delayed_attempts += 1;
        if event.applied_grade == ReviewGrade::Pass {
            state.delayed_passes += 1;
        }
    }
    state.lapses = event.next_lapses;
    state.success_rung = event.next_success_rung;
    state.interval_ms = event.next_interval_ms;
    state.due_at_unix_ms = event.next_due_at_unix_ms;
    state.last_reviewed_at_unix_ms = event.reviewed_at_unix_ms;
    state.last_grade = event.applied_grade;
    state.last_evidence_version = event.evidence_version.clone();
    state.last_event_sequence = event.sequence;
}

fn open_private_log(path: &Path) -> Result<File, ReviewError> {
    let existing_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(ReviewError::UnsafePath(format!(
                    "review log '{}' must be a regular, non-symlink file",
                    path.display()
                )));
            }
            #[cfg(unix)]
            if metadata.nlink() != 1 {
                return Err(ReviewError::UnsafePath(format!(
                    "review log '{}' must not have multiple hard links",
                    path.display()
                )));
            }
            Some(metadata)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(ReviewError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let exists = existing_metadata.is_some();

    let mut options = OpenOptions::new();
    options.read(true).append(true);
    if exists {
        options.create(false);
    } else {
        options.create_new(true);
    }
    #[cfg(unix)]
    options.mode(0o600);
    let file = options.open(path).map_err(|source| ReviewError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let metadata = file.metadata().map_err(|source| ReviewError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(ReviewError::UnsafePath(format!(
            "review log '{}' is not a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if let Some(path_metadata) = existing_metadata
            && (path_metadata.dev() != metadata.dev() || path_metadata.ino() != metadata.ino())
        {
            return Err(ReviewError::UnsafePath(format!(
                "review log '{}' changed while it was opened",
                path.display()
            )));
        }
        if metadata.nlink() != 1 {
            return Err(ReviewError::UnsafePath(format!(
                "review log '{}' must not have multiple hard links",
                path.display()
            )));
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|source| ReviewError::Io {
                path: path.to_path_buf(),
                source,
            })?;
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn key(version: &str) -> ReviewKey {
        ReviewKey {
            card_id: "fixture-card".to_string(),
            content_version: version.to_string(),
        }
    }

    fn input(id: &str, at: i64, grade: ReviewGrade) -> ReviewInput {
        ReviewInput {
            event_id: id.to_string(),
            key: key("content-v1"),
            evidence_version: "evidence-v1".to_string(),
            shown_at_unix_ms: at.saturating_sub(1_500).max(0),
            reviewed_at_unix_ms: at,
            grade,
            hint_used: false,
            response_uci: Some("g1f3".to_string()),
            reference_match: Some(grade == ReviewGrade::Pass),
            latency_ms: 1_500,
        }
    }

    fn apply_input(
        states: &mut BTreeMap<ReviewKey, ReviewState>,
        sequence: u64,
        input: ReviewInput,
    ) -> ReviewEvent {
        let event = derive_event(sequence, states, input);
        apply_event(states, &event);
        event
    }

    #[test]
    fn fixed_schedule_handles_pass_partial_miss_and_hint() {
        let mut states = BTreeMap::new();
        let first = apply_input(&mut states, 1, input("one", 0, ReviewGrade::Pass));
        assert_eq!(first.next_interval_ms, 2 * DAY_MS);
        assert!(!first.delayed_eligible);

        let second = apply_input(
            &mut states,
            2,
            input("two", first.next_due_at_unix_ms, ReviewGrade::Pass),
        );
        assert_eq!(second.next_interval_ms, 4 * DAY_MS);
        assert!(second.was_due);
        assert!(second.delayed_eligible);

        let partial = apply_input(
            &mut states,
            3,
            input("three", second.next_due_at_unix_ms, ReviewGrade::Partial),
        );
        assert_eq!(partial.next_interval_ms, DAY_MS);
        assert_eq!(partial.next_success_rung, 0);
        assert_eq!(partial.next_lapses, 0);

        let miss = apply_input(
            &mut states,
            4,
            input("four", partial.next_due_at_unix_ms, ReviewGrade::Miss),
        );
        assert_eq!(miss.next_interval_ms, 10 * MINUTE_MS);
        assert_eq!(miss.next_lapses, 1);

        let mut hinted = input("five", miss.next_due_at_unix_ms, ReviewGrade::Pass);
        hinted.hint_used = true;
        let hinted = apply_input(&mut states, 5, hinted);
        assert_eq!(hinted.applied_grade, ReviewGrade::Miss);
        assert_eq!(hinted.next_interval_ms, 10 * MINUTE_MS);
        assert_eq!(hinted.next_lapses, 2);
    }

    #[test]
    fn delayed_measure_requires_a_day_interval_and_due_review() {
        let mut states = BTreeMap::new();
        let first = apply_input(&mut states, 1, input("one", 0, ReviewGrade::Pass));
        let early = apply_input(
            &mut states,
            2,
            input("early", first.next_due_at_unix_ms - 1, ReviewGrade::Pass),
        );
        assert!(!early.delayed_eligible);
        let due = apply_input(
            &mut states,
            3,
            input("due", early.next_due_at_unix_ms, ReviewGrade::Pass),
        );
        assert!(due.delayed_eligible);
    }

    fn temp_path(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "chess-trainer-review-{label}-{}-{}.jsonl",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn durable_reload_is_idempotent_and_discards_only_partial_tail() {
        let path = temp_path("reload");
        let first_input = input("stable-event", 10_000, ReviewGrade::Pass);
        let expected_state;
        let valid_len;
        {
            let mut store = ReviewStore::open(&path).unwrap();
            let receipt = store.record(first_input.clone()).unwrap();
            assert!(receipt.inserted);
            expected_state = store.state(&first_input.key).unwrap().clone();
            valid_len = fs::metadata(&path).unwrap().len();
            #[cfg(unix)]
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        {
            let mut append = OpenOptions::new().append(true).open(&path).unwrap();
            append.write_all(b"{\"schema\":\"interrupted").unwrap();
            append.flush().unwrap();
        }
        {
            let mut store = ReviewStore::open(&path).unwrap();
            assert_eq!(fs::metadata(&path).unwrap().len(), valid_len);
            assert_eq!(store.state(&first_input.key), Some(&expected_state));
            let duplicate = store.record(first_input.clone()).unwrap();
            assert!(!duplicate.inserted);
            assert_eq!(store.events().len(), 1);

            let mut conflict = first_input.clone();
            conflict.latency_ms += 1;
            assert!(matches!(
                store.record(conflict),
                Err(ReviewError::EventIdConflict(_))
            ));
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn evidence_changes_preserve_state_but_content_changes_do_not() {
        let path = temp_path("versions");
        let mut store = ReviewStore::open(&path).unwrap();
        let first = input("one", 0, ReviewGrade::Pass);
        let first_due = store
            .record(first.clone())
            .unwrap()
            .event
            .next_due_at_unix_ms;
        let mut evidence_change = input("two", first_due, ReviewGrade::Pass);
        evidence_change.evidence_version = "evidence-v2".to_string();
        let second = store.record(evidence_change).unwrap();
        assert_eq!(second.event.next_interval_ms, 4 * DAY_MS);

        let mut content_change = input("three", first_due, ReviewGrade::Pass);
        content_change.key = key("content-v2");
        let fresh = store.record(content_change.clone()).unwrap();
        assert_eq!(fresh.event.next_interval_ms, 2 * DAY_MS);
        assert_eq!(store.states().len(), 2);

        let active = vec![first.key, content_change.key];
        let summary = store.summary(&active, first_due);
        assert_eq!(summary.active_cards, 2);
        assert_eq!(summary.reviewed_cards, 2);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn newline_terminated_corruption_is_rejected() {
        let path = temp_path("corrupt");
        fs::write(&path, b"{not-json}\n").unwrap();
        assert!(matches!(
            ReviewStore::open(&path),
            Err(ReviewError::Corrupt { line: 1, .. })
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn refuses_a_second_writer_until_the_first_store_closes() {
        let path = temp_path("exclusive-lock");
        let first = ReviewStore::open(&path).unwrap();
        assert!(matches!(
            ReviewStore::open(&path),
            Err(ReviewError::Locked(locked)) if locked == path
        ));
        drop(first);
        ReviewStore::open(&path).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_and_multiple_hard_links() {
        use std::os::unix::fs::symlink;

        let target = temp_path("target");
        let alias = temp_path("alias");
        fs::write(&target, b"").unwrap();
        fs::hard_link(&target, &alias).unwrap();
        assert!(matches!(
            ReviewStore::open(&target),
            Err(ReviewError::UnsafePath(_))
        ));
        fs::remove_file(&alias).unwrap();

        let link = temp_path("link");
        symlink(&target, &link).unwrap();
        assert!(matches!(
            ReviewStore::open(&link),
            Err(ReviewError::UnsafePath(_))
        ));
        fs::remove_file(link).unwrap();
        fs::remove_file(target).unwrap();
    }
}
