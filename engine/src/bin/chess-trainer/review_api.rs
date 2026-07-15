//! Loopback-only HTTP-facing review service.
//!
//! The catalog owns private game histories and answers.  This layer emits one
//! sanitized prompt at a time, keeps server-timed pending attempts, reveals
//! evidence only after commitment, and records the eventual self-grade.

use crate::cards::{CardFeedback, CardKey, Catalog, TrainingCard};
use crate::review::{
    RevealInput, ReviewGrade, ReviewInput, ReviewKey, ReviewStore, SCHEDULER_VERSION,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::Read;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DAY_MS: i64 = 24 * 60 * 60 * 1_000;
const MAX_PENDING_ATTEMPTS: usize = 256;
const PENDING_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Debug)]
struct PendingAttempt {
    key: CardKey,
    expected_last_event_sequence: Option<u64>,
    shown_at: Instant,
    shown_at_unix_ms: i64,
    revealed: Option<RevealedAttempt>,
    recorded_at_unix_ms: Option<i64>,
    recorded_grade: Option<ReviewGrade>,
}

#[derive(Clone, Debug)]
struct RevealedAttempt {
    feedback: CardFeedback,
    response_uci: Option<String>,
    hint_used: bool,
    latency_ms: u64,
    revealed_at_unix_ms: i64,
    assurance: String,
    confirmation_nodes_per_position: u64,
    analysis_config_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NextRequest {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevealRequest {
    pub attempt_id: String,
    pub move_uci: Option<String>,
    pub reason_present: bool,
    pub gave_up: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GradeRequest {
    pub attempt_id: String,
    pub outcome: ReviewGrade,
}

pub struct ReviewApi {
    catalog: Catalog,
    store: ReviewStore,
    pending: HashMap<String, PendingAttempt>,
}

impl ReviewApi {
    pub fn new(catalog: Catalog, store: ReviewStore) -> Result<Self, String> {
        let durable_reveals: Vec<_> = store.pending_reveals().cloned().collect();
        let mut pending = HashMap::new();
        for reveal in durable_reveals {
            let key = CardKey {
                card_id: reveal.card_id.clone(),
                content_version: reveal.content_version.clone(),
            };
            if catalog.get(&key.card_id, &key.content_version).is_none() {
                continue;
            }
            let current_sequence = store
                .state(&review_key(&key))
                .map(|state| state.last_event_sequence);
            if current_sequence != reveal.prior_review_sequence {
                return Err(format!(
                    "unfinished answer release '{}' is stale for active card '{}'; repair the private review log",
                    reveal.event_id, reveal.card_id
                ));
            }
            pending.insert(
                reveal.event_id.clone(),
                PendingAttempt {
                    key,
                    expected_last_event_sequence: reveal.prior_review_sequence,
                    shown_at: Instant::now(),
                    shown_at_unix_ms: reveal.shown_at_unix_ms,
                    revealed: Some(RevealedAttempt {
                        feedback: reveal.feedback,
                        response_uci: reveal.response_uci,
                        hint_used: reveal.hint_used,
                        latency_ms: reveal.latency_ms,
                        revealed_at_unix_ms: reveal.revealed_at_unix_ms,
                        assurance: reveal.assurance,
                        confirmation_nodes_per_position: reveal.confirmation_nodes_per_position,
                        analysis_config_version: reveal.analysis_config_version,
                    }),
                    recorded_at_unix_ms: None,
                    recorded_grade: None,
                },
            );
            if pending.len() > MAX_PENDING_ATTEMPTS {
                return Err(format!(
                    "active deck has more than {MAX_PENDING_ATTEMPTS} unfinished answer releases"
                ));
            }
        }
        Ok(Self {
            catalog,
            store,
            pending,
        })
    }

    pub fn deck_title(&self) -> &str {
        &self.catalog.title
    }

    pub fn progress_json(&mut self) -> Result<String, String> {
        self.prune_pending();
        serde_json::to_string(&self.progress_value(now_unix_ms()?))
            .map_err(|error| format!("cannot serialize review progress: {error}"))
    }

    pub fn next_json(&mut self, _request: NextRequest) -> Result<String, String> {
        let now = now_unix_ms()?;
        self.prune_pending();
        let queue = self.queue_value(now);
        let reusable = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.recorded_grade.is_none())
            .min_by_key(|(_, pending)| pending.shown_at_unix_ms)
            .map(|(attempt_id, pending)| {
                (
                    attempt_id.clone(),
                    pending.key.clone(),
                    pending.revealed.clone(),
                )
            });
        if let Some((attempt_id, key, revealed)) = reusable {
            let card = self
                .catalog
                .get(&key.card_id, &key.content_version)
                .ok_or_else(|| "pending review card is no longer active".to_string())?;
            let recovered_answer = revealed.map(|revealed| feedback_value(&revealed));
            return serialize_next(queue, Some((attempt_id, card)), recovered_answer);
        }
        let card = self.select_next(now).cloned();
        match card {
            None => serialize_next(queue, None, None),
            Some(card) => {
                let key = card.key();
                let expected_last_event_sequence = self
                    .store
                    .state(&review_key(&key))
                    .map(|state| state.last_event_sequence);
                let attempt_id = random_token(24)?;
                self.pending.insert(
                    attempt_id.clone(),
                    PendingAttempt {
                        key,
                        expected_last_event_sequence,
                        shown_at: Instant::now(),
                        shown_at_unix_ms: now,
                        revealed: None,
                        recorded_at_unix_ms: None,
                        recorded_grade: None,
                    },
                );
                serialize_next(queue, Some((attempt_id, &card)), None)
            }
        }
    }

    pub fn reveal_json(&mut self, request: RevealRequest) -> Result<String, String> {
        self.prune_pending();
        let pending_snapshot = self
            .pending
            .get(&request.attempt_id)
            .cloned()
            .ok_or_else(|| "unknown or expired review attempt".to_string())?;
        if pending_snapshot.recorded_grade.is_some() {
            return Err("review attempt was already graded".to_string());
        }
        if self
            .store
            .state(&review_key(&pending_snapshot.key))
            .map(|state| state.last_event_sequence)
            != pending_snapshot.expected_last_event_sequence
        {
            return Err(
                "review card changed after this attempt was issued; load a fresh card".to_string(),
            );
        }
        if let Some(revealed) = pending_snapshot.revealed.as_ref() {
            let same_commitment = if revealed.hint_used {
                request.gave_up && request.move_uci.is_none()
            } else {
                !request.gave_up
                    && request.reason_present
                    && request.move_uci.as_deref() == revealed.response_uci.as_deref()
            };
            if !same_commitment {
                return Err(
                    "reveal request conflicts with the move already committed for this attempt"
                        .to_string(),
                );
            }
            return feedback_json(revealed);
        }

        let card = self
            .catalog
            .get(
                &pending_snapshot.key.card_id,
                &pending_snapshot.key.content_version,
            )
            .ok_or_else(|| "review card is no longer active".to_string())?;
        let (feedback, response_uci, hint_used) = if request.gave_up {
            if request.move_uci.is_some() {
                return Err("give-up reveal must not include a move".to_string());
            }
            (card.give_up_feedback(), None, true)
        } else {
            if !request.reason_present {
                return Err("commit a reason before revealing".to_string());
            }
            let response = request
                .move_uci
                .as_deref()
                .ok_or_else(|| "commit a legal move before revealing".to_string())?;
            (
                card.feedback(response).map_err(|error| error.to_string())?,
                Some(response.to_string()),
                false,
            )
        };
        let latency_ms = pending_snapshot
            .shown_at
            .elapsed()
            .as_millis()
            .min(24 * 60 * 60 * 1_000) as u64;
        let revealed_at_unix_ms = pending_snapshot
            .shown_at_unix_ms
            .saturating_add(latency_ms as i64);
        if pending_snapshot.expected_last_event_sequence.is_none()
            && self.new_revealed_24h(revealed_at_unix_ms) >= self.catalog.new_per_day
        {
            return Err("the new-card limit was reached; load a fresh queue".to_string());
        }
        let receipt = self
            .store
            .record_reveal(RevealInput {
                event_id: request.attempt_id.clone(),
                key: review_key(&pending_snapshot.key),
                evidence_version: feedback.evidence_version.clone(),
                shown_at_unix_ms: pending_snapshot.shown_at_unix_ms,
                revealed_at_unix_ms,
                hint_used,
                response_uci: response_uci.clone(),
                reference_match: feedback.reference_match,
                latency_ms,
                feedback: feedback.clone(),
                assurance: self.catalog.assurance.clone(),
                confirmation_nodes_per_position: self.catalog.confirmation_nodes_per_position,
                analysis_config_version: self.catalog.analysis_config_version.clone(),
            })
            .map_err(|error| error.to_string())?;
        if receipt.event.prior_review_sequence != pending_snapshot.expected_last_event_sequence {
            return Err("answer release was recorded from a stale card state".to_string());
        }
        let revealed = RevealedAttempt {
            feedback,
            response_uci,
            hint_used,
            latency_ms,
            revealed_at_unix_ms,
            assurance: self.catalog.assurance.clone(),
            confirmation_nodes_per_position: self.catalog.confirmation_nodes_per_position,
            analysis_config_version: self.catalog.analysis_config_version.clone(),
        };
        self.pending
            .get_mut(&request.attempt_id)
            .expect("pending attempt was checked above")
            .revealed = Some(revealed.clone());
        feedback_json(&revealed)
    }

    pub fn grade_json(&mut self, request: GradeRequest) -> Result<String, String> {
        self.prune_pending();
        let now = now_unix_ms()?;
        if !self.pending.contains_key(&request.attempt_id) {
            let event = self
                .store
                .event(&request.attempt_id)
                .cloned()
                .ok_or_else(|| "unknown or expired review attempt".to_string())?;
            if event.submitted_grade != request.outcome {
                return Err("review attempt was already graded differently".to_string());
            }
            let value = json!({
                "dueAtMs": event.next_due_at_unix_ms,
                "intervalMs": event.next_interval_ms,
                "appliedOutcome": event.applied_grade,
                "inserted": false,
                "progress": self.progress_value(now),
            });
            return serde_json::to_string(&value)
                .map_err(|error| format!("cannot serialize review receipt: {error}"));
        }
        let (key, expected_sequence, revealed, recorded_at, prior_grade) = {
            let pending = self
                .pending
                .get(&request.attempt_id)
                .ok_or_else(|| "unknown or expired review attempt".to_string())?;
            let revealed = pending
                .revealed
                .clone()
                .ok_or_else(|| "reveal the card before grading it".to_string())?;
            let recorded_at = pending.recorded_at_unix_ms.unwrap_or_else(|| {
                let monotonic_now = pending.shown_at_unix_ms.saturating_add(
                    pending.shown_at.elapsed().as_millis().min(i64::MAX as u128) as i64,
                );
                now.max(monotonic_now).max(revealed.revealed_at_unix_ms)
            });
            (
                pending.key.clone(),
                pending.expected_last_event_sequence,
                revealed,
                recorded_at,
                pending.recorded_grade,
            )
        };
        if let Some(prior_grade) = prior_grade {
            if prior_grade != request.outcome {
                return Err("review attempt was already graded differently".to_string());
            }
        } else {
            let current_sequence = self
                .store
                .state(&review_key(&key))
                .map(|state| state.last_event_sequence);
            if current_sequence != expected_sequence {
                return Err(
                    "review card changed after this attempt was issued; load a fresh card"
                        .to_string(),
                );
            }
        }
        let input = ReviewInput {
            event_id: request.attempt_id.clone(),
            key: review_key(&key),
            evidence_version: revealed.feedback.evidence_version.clone(),
            shown_at_unix_ms: self
                .pending
                .get(&request.attempt_id)
                .expect("pending attempt was checked above")
                .shown_at_unix_ms,
            reviewed_at_unix_ms: recorded_at,
            grade: request.outcome,
            hint_used: revealed.hint_used,
            response_uci: revealed.response_uci.clone(),
            reference_match: revealed.feedback.reference_match,
            latency_ms: revealed.latency_ms,
        };
        let receipt = self
            .store
            .record(input)
            .map_err(|error| error.to_string())?;
        let pending = self
            .pending
            .get_mut(&request.attempt_id)
            .expect("pending attempt was checked above");
        pending.recorded_at_unix_ms = Some(recorded_at);
        pending.recorded_grade = Some(request.outcome);
        pending.expected_last_event_sequence = Some(receipt.event.sequence);
        let value = json!({
            "dueAtMs": receipt.event.next_due_at_unix_ms,
            "intervalMs": receipt.event.next_interval_ms,
            "appliedOutcome": receipt.event.applied_grade,
            "inserted": receipt.inserted,
            "progress": self.progress_value(recorded_at),
        });
        serde_json::to_string(&value)
            .map_err(|error| format!("cannot serialize review receipt: {error}"))
    }

    fn active_review_keys(&self) -> Vec<ReviewKey> {
        self.catalog
            .cards()
            .iter()
            .map(|card| ReviewKey {
                card_id: card.card_id.clone(),
                content_version: card.content_version.clone(),
            })
            .collect()
    }

    fn progress_value(&self, now: i64) -> Value {
        let keys = self.active_review_keys();
        let summary = self.store.summary(&keys, now);
        let queue = self.queue_value(now);
        let active: BTreeSet<_> = keys.into_iter().collect();
        let delayed_accuracy = if summary.delayed_attempts == 0 {
            Value::Null
        } else {
            json!(summary.delayed_passes as f64 / summary.delayed_attempts as f64)
        };
        let no_hint_events: Vec<_> = self
            .store
            .events()
            .iter()
            .filter(|event| {
                active.contains(&ReviewKey {
                    card_id: event.card_id.clone(),
                    content_version: event.content_version.clone(),
                })
            })
            .filter(|event| !event.hint_used)
            .collect();
        let no_hint_passes = no_hint_events
            .iter()
            .filter(|event| event.applied_grade == ReviewGrade::Pass)
            .count();
        let median_latency_ms = median_latency(
            self.store
                .events()
                .iter()
                .filter(|event| {
                    active.contains(&ReviewKey {
                        card_id: event.card_id.clone(),
                        content_version: event.content_version.clone(),
                    })
                })
                .map(|event| event.latency_ms)
                .collect(),
        );
        json!({
            "configured": true,
            "deckTitle": self.catalog.title,
            "deckId": self.catalog.deck_id,
            "schedulerVersion": SCHEDULER_VERSION,
            "queue": queue,
            "reviewedCards": summary.reviewed_cards,
            "matureCards": summary.mature_cards,
            "totalReviews": summary.attempts,
            "hints": summary.hints,
            "lapses": summary.lapses,
            "referenceMatches": summary.reference_matches,
            "delayedAttempts": summary.delayed_attempts,
            "delayedPasses": summary.delayed_passes,
            "delayedAccuracy": delayed_accuracy,
            "noHintAttempts": no_hint_events.len(),
            "noHintPasses": no_hint_passes,
            "medianLatencyMs": median_latency_ms,
            "nextDueAtMs": summary.next_due_at_unix_ms,
        })
    }

    fn queue_value(&self, now: i64) -> Value {
        let active_keys = self.active_review_keys();
        let summary = self.store.summary(&active_keys, now);
        let window_start = now.saturating_sub(DAY_MS);
        let active: BTreeSet<_> = active_keys.iter().cloned().collect();
        let reviewed_24h: BTreeSet<_> = self
            .store
            .events()
            .iter()
            .filter(|event| event.reviewed_at_unix_ms >= window_start)
            .map(|event| ReviewKey {
                card_id: event.card_id.clone(),
                content_version: event.content_version.clone(),
            })
            .filter(|key| active.contains(key))
            .collect();
        let new_revealed_24h = self.new_revealed_24h(now);
        let new_remaining = self
            .catalog
            .new_per_day
            .saturating_sub(new_revealed_24h)
            .min(summary.new_cards);
        let scheduled_due = summary.due_cards.saturating_sub(summary.new_cards);
        json!({
            "due": scheduled_due,
            "new": new_remaining,
            "active": summary.active_cards,
            "reviewed24h": reviewed_24h.len(),
        })
    }

    fn new_revealed_24h(&self, now: i64) -> usize {
        let window_start = now.saturating_sub(DAY_MS);
        let active: BTreeSet<_> = self.active_review_keys().into_iter().collect();
        let mut event_ids = BTreeSet::new();
        for reveal in self.store.reveals().filter(|reveal| {
            reveal.revealed_at_unix_ms >= window_start
                && reveal.prior_review_sequence.is_none()
                && active.contains(&ReviewKey {
                    card_id: reveal.card_id.clone(),
                    content_version: reveal.content_version.clone(),
                })
        }) {
            event_ids.insert(reveal.event_id.clone());
        }
        for event in self.store.events().iter().filter(|event| {
            !self.store.has_reveal(&event.event_id)
                && event.shown_at_unix_ms >= window_start
                && event.prior_due_at_unix_ms.is_none()
                && active.contains(&ReviewKey {
                    card_id: event.card_id.clone(),
                    content_version: event.content_version.clone(),
                })
        }) {
            event_ids.insert(event.event_id.clone());
        }
        event_ids.len()
    }

    fn select_next(&self, now: i64) -> Option<&TrainingCard> {
        let mut due: Vec<_> = self
            .catalog
            .cards()
            .iter()
            .filter_map(|card| {
                if self.card_is_reserved(card) {
                    return None;
                }
                let key = ReviewKey {
                    card_id: card.card_id.clone(),
                    content_version: card.content_version.clone(),
                };
                self.store
                    .state(&key)
                    .filter(|state| state.due_at_unix_ms <= now)
                    .map(|state| (state.due_at_unix_ms, card))
            })
            .collect();
        due.sort_by_key(|(due_at, card)| (*due_at, card.card_id.as_str()));
        if let Some((_, card)) = due.first() {
            return Some(*card);
        }

        let queue = self.queue_value(now);
        let new_allowed = queue["new"].as_u64().unwrap_or(0) as usize;
        if new_allowed == 0 {
            return None;
        }
        self.catalog.cards().iter().find(|card| {
            !self.card_is_reserved(card)
                && self
                    .store
                    .state(&ReviewKey {
                        card_id: card.card_id.clone(),
                        content_version: card.content_version.clone(),
                    })
                    .is_none()
        })
    }

    fn card_is_reserved(&self, card: &TrainingCard) -> bool {
        self.pending.values().any(|pending| {
            pending.recorded_grade.is_none()
                && pending.key.card_id == card.card_id
                && pending.key.content_version == card.content_version
        })
    }

    fn prune_pending(&mut self) {
        self.pending.retain(|_, pending| {
            (pending.revealed.is_some() && pending.recorded_grade.is_none())
                || pending.shown_at.elapsed() <= PENDING_TTL
        });
        if self.pending.len() >= MAX_PENDING_ATTEMPTS {
            let mut oldest: Vec<_> = self
                .pending
                .iter()
                .filter(|(_, pending)| {
                    pending.revealed.is_none() || pending.recorded_grade.is_some()
                })
                .map(|(id, pending)| (pending.shown_at, id.clone()))
                .collect();
            oldest.sort_by_key(|(shown, _)| *shown);
            let remove = self.pending.len() + 1 - MAX_PENDING_ATTEMPTS;
            for (_, id) in oldest.into_iter().take(remove) {
                self.pending.remove(&id);
            }
        }
    }
}

fn serialize_next(
    queue: Value,
    selected: Option<(String, &TrainingCard)>,
    recovered_answer: Option<Value>,
) -> Result<String, String> {
    let value = match selected {
        None => json!({
            "configured": true,
            "queue": queue,
            "card": null,
            "recoveredAnswer": recovered_answer,
        }),
        Some((attempt_id, card)) => {
            let prompt = card.prompt_view();
            json!({
                "configured": true,
                "queue": queue,
                "card": {
                    "attemptId": attempt_id,
                    "prompt": prompt.prompt,
                    "orientation": prompt.orientation,
                    "check": prompt.check,
                    "fen": prompt.fen,
                    "positionId": prompt.position_id,
                    "tags": prompt.tags,
                    "legal": prompt.legal_moves,
                },
                "recoveredAnswer": recovered_answer,
            })
        }
    };
    serde_json::to_string(&value).map_err(|error| format!("cannot serialize next review: {error}"))
}

fn review_key(key: &CardKey) -> ReviewKey {
    ReviewKey {
        card_id: key.card_id.clone(),
        content_version: key.content_version.clone(),
    }
}

fn feedback_json(revealed: &RevealedAttempt) -> Result<String, String> {
    serde_json::to_string(&feedback_value(revealed))
        .map_err(|error| format!("cannot serialize review feedback: {error}"))
}

fn feedback_value(revealed: &RevealedAttempt) -> Value {
    let feedback = &revealed.feedback;
    json!({
        "legal": true,
        "referenceMatch": feedback.reference_match,
        "latencyMs": revealed.latency_ms,
        "choice": feedback.played_move,
        "reference": {
            "uci": feedback.reference_move.uci,
            "san": feedback.reference_move.san,
            "lineSan": feedback.reference_line.iter().map(|item| item.san.clone()).collect::<Vec<_>>(),
        },
        "original": {
            "uci": feedback.source_game_move.uci,
            "san": feedback.source_game_move.san,
            "lineSan": feedback.source_game_line.iter().map(|item| item.san.clone()).collect::<Vec<_>>(),
            "lossCp": feedback.loss_cp_equivalent,
            "lossBucket": feedback.loss_bucket,
            "mateEvent": feedback.mate_event,
        },
        "explanation": feedback.explanation,
        "successCriterion": feedback.success_criterion,
        "evidence": {
            "assurance": revealed.assurance,
            "nodes": revealed.confirmation_nodes_per_position,
            "analysisConfigVersion": revealed.analysis_config_version,
            "evidenceVersion": feedback.evidence_version,
        },
    })
}

fn median_latency(mut values: Vec<u64>) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        Some(values[middle])
    } else {
        Some(values[middle - 1].saturating_add(values[middle]) / 2)
    }
}

pub fn now_unix_ms() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock precedes the Unix epoch".to_string())?;
    i64::try_from(duration.as_millis()).map_err(|_| "system clock is out of range".to_string())
}

pub fn random_token(bytes: usize) -> Result<String, String> {
    let mut random = vec![0u8; bytes];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut random))
        .map_err(|error| format!("cannot read operating-system randomness: {error}"))?;
    let mut token = String::with_capacity(bytes * 2);
    use std::fmt::Write as _;
    for byte in random {
        write!(&mut token, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::Catalog;
    use crate::review::{ReviewInput, ReviewKey};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_path(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        std::env::temp_dir().join(format!(
            "chess-review-api-{label}-{}-{}.jsonl",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn api(card_count: usize, new_per_day: usize, label: &str) -> (ReviewApi, PathBuf) {
        let path = temp_path(label);
        let store = ReviewStore::open(&path).unwrap();
        (
            ReviewApi::new(Catalog::fixture_for_tests(card_count, new_per_day), store).unwrap(),
            path,
        )
    }

    fn next(api: &mut ReviewApi) -> Value {
        serde_json::from_str(&api.next_json(NextRequest {}).unwrap()).unwrap()
    }

    fn reveal(api: &mut ReviewApi, attempt_id: &str) {
        api.reveal_json(RevealRequest {
            attempt_id: attempt_id.to_string(),
            move_uci: Some("e2e4".to_string()),
            reason_present: true,
            gave_up: false,
        })
        .unwrap();
    }

    fn input(event_id: &str, content_version: &str, shown: i64, reviewed: i64) -> ReviewInput {
        ReviewInput {
            event_id: event_id.to_string(),
            key: ReviewKey {
                card_id: "fixture-card-0".to_string(),
                content_version: content_version.to_string(),
            },
            evidence_version: "fixture-evidence-v1".to_string(),
            shown_at_unix_ms: shown,
            reviewed_at_unix_ms: reviewed,
            grade: ReviewGrade::Pass,
            hint_used: false,
            response_uci: Some("e2e4".to_string()),
            reference_match: Some(true),
            latency_ms: 1_000,
        }
    }

    #[test]
    fn next_reuses_one_answer_safe_pending_attempt() {
        let (mut api, path) = api(2, 1, "reuse");
        let first = next(&mut api);
        assert_eq!(first["recoveredAnswer"], Value::Null);
        let card = first["card"].as_object().unwrap();
        assert_eq!(
            card.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "attemptId",
                "check",
                "fen",
                "legal",
                "orientation",
                "positionId",
                "prompt",
                "tags",
            ])
        );
        let second = next(&mut api);
        assert_eq!(first["card"]["attemptId"], second["card"]["attemptId"]);
        assert_eq!(first["queue"]["new"], json!(1));
        assert_eq!(second["queue"]["new"], json!(1));
        let progress: Value = serde_json::from_str(&api.progress_json().unwrap()).unwrap();
        assert_eq!(progress["queue"]["new"], json!(1));
        assert_eq!(api.pending.len(), 1);
        drop(api);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn reveal_retry_must_match_and_next_recovers_the_answer() {
        let (mut api, path) = api(1, 1, "reveal-recovery");
        let first = next(&mut api);
        let attempt_id = first["card"]["attemptId"].as_str().unwrap().to_string();
        let request = || RevealRequest {
            attempt_id: attempt_id.clone(),
            move_uci: Some("e2e4".to_string()),
            reason_present: true,
            gave_up: false,
        };
        let initial = api.reveal_json(request()).unwrap();
        assert_eq!(api.reveal_json(request()).unwrap(), initial);

        let conflicting_move = api
            .reveal_json(RevealRequest {
                attempt_id: attempt_id.clone(),
                move_uci: Some("d2d4".to_string()),
                reason_present: true,
                gave_up: false,
            })
            .unwrap_err();
        assert!(conflicting_move.contains("conflicts with the move already committed"));
        let conflicting_give_up = api
            .reveal_json(RevealRequest {
                attempt_id: attempt_id.clone(),
                move_uci: None,
                reason_present: false,
                gave_up: true,
            })
            .unwrap_err();
        assert!(conflicting_give_up.contains("conflicts with the move already committed"));

        let recovered = next(&mut api);
        assert_eq!(recovered["card"]["attemptId"], json!(attempt_id));
        assert_eq!(recovered["recoveredAnswer"]["choice"]["uci"], json!("e2e4"));
        assert_eq!(recovered["queue"]["new"], json!(0));
        drop(api);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn give_up_survives_restart_expiry_and_grade_retry_as_a_miss() {
        let path = temp_path("durable-give-up");
        let catalog = Catalog::fixture_for_tests(1, 1);
        let attempt_id;
        {
            let store = ReviewStore::open(&path).unwrap();
            let mut api = ReviewApi::new(catalog.clone(), store).unwrap();
            let first = next(&mut api);
            attempt_id = first["card"]["attemptId"].as_str().unwrap().to_string();
            let answer: Value = serde_json::from_str(
                &api.reveal_json(RevealRequest {
                    attempt_id: attempt_id.clone(),
                    move_uci: None,
                    reason_present: false,
                    gave_up: true,
                })
                .unwrap(),
            )
            .unwrap();
            assert_eq!(answer["choice"], Value::Null);
            assert_eq!(api.store.reveals().count(), 1);
            assert!(api.store.events().is_empty());
        }

        {
            let store = ReviewStore::open(&path).unwrap();
            let mut api = ReviewApi::new(catalog.clone(), store).unwrap();
            api.pending.get_mut(&attempt_id).unwrap().shown_at =
                Instant::now() - PENDING_TTL - Duration::from_secs(1);
            let recovered = next(&mut api);
            assert_eq!(recovered["card"]["attemptId"], json!(attempt_id.clone()));
            assert_eq!(recovered["recoveredAnswer"]["choice"], Value::Null);
            assert_eq!(recovered["queue"]["new"], json!(0));

            let receipt: Value = serde_json::from_str(
                &api.grade_json(GradeRequest {
                    attempt_id: attempt_id.clone(),
                    outcome: ReviewGrade::Pass,
                })
                .unwrap(),
            )
            .unwrap();
            assert_eq!(receipt["appliedOutcome"], json!("miss"));
            assert_eq!(receipt["inserted"], json!(true));
            assert!(api.store.events()[0].hint_used);
        }

        {
            let store = ReviewStore::open(&path).unwrap();
            let mut api = ReviewApi::new(catalog, store).unwrap();
            let retry: Value = serde_json::from_str(
                &api.grade_json(GradeRequest {
                    attempt_id: attempt_id.clone(),
                    outcome: ReviewGrade::Pass,
                })
                .unwrap(),
            )
            .unwrap();
            assert_eq!(retry["inserted"], json!(false));
            let conflict = api
                .grade_json(GradeRequest {
                    attempt_id,
                    outcome: ReviewGrade::Partial,
                })
                .unwrap_err();
            assert_eq!(conflict, "review attempt was already graded differently");
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn stale_duplicate_cannot_advance_a_card_twice() {
        let (mut api, path) = api(1, 1, "stale");
        let first = next(&mut api);
        let first_id = first["card"]["attemptId"].as_str().unwrap().to_string();
        let duplicate_id = "duplicate-attempt".to_string();
        reveal(&mut api, &first_id);
        let duplicate = api.pending.get(&first_id).unwrap().clone();
        api.pending.insert(duplicate_id.clone(), duplicate);

        let inserted: Value = serde_json::from_str(
            &api.grade_json(GradeRequest {
                attempt_id: first_id.clone(),
                outcome: ReviewGrade::Pass,
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(inserted["inserted"], json!(true));
        let stale = api
            .grade_json(GradeRequest {
                attempt_id: duplicate_id,
                outcome: ReviewGrade::Pass,
            })
            .unwrap_err();
        assert!(stale.contains("changed after this attempt was issued"));

        let retry: Value = serde_json::from_str(
            &api.grade_json(GradeRequest {
                attempt_id: first_id,
                outcome: ReviewGrade::Pass,
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(retry["inserted"], json!(false));
        assert_eq!(api.store.events().len(), 1);
        drop(api);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn expired_attempt_cannot_reveal_or_record() {
        let (mut api, path) = api(1, 1, "expiry");
        let response = next(&mut api);
        let attempt_id = response["card"]["attemptId"].as_str().unwrap().to_string();
        api.pending.get_mut(&attempt_id).unwrap().shown_at =
            Instant::now() - PENDING_TTL - Duration::from_secs(1);
        let error = api
            .reveal_json(RevealRequest {
                attempt_id,
                move_uci: Some("e2e4".to_string()),
                reason_present: true,
                gave_up: false,
            })
            .unwrap_err();
        assert_eq!(error, "unknown or expired review attempt");
        assert!(api.store.events().is_empty());
        drop(api);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn progress_ignores_retired_content_versions() {
        let path = temp_path("active-metrics");
        let mut store = ReviewStore::open(&path).unwrap();
        store
            .record(input("old", "retired-content", 1_000, 2_000))
            .unwrap();
        let mut api = ReviewApi::new(Catalog::fixture_for_tests(1, 1), store).unwrap();
        let progress: Value = serde_json::from_str(&api.progress_json().unwrap()).unwrap();
        assert_eq!(progress["totalReviews"], json!(0));
        assert_eq!(progress["noHintAttempts"], json!(0));
        assert_eq!(progress["medianLatencyMs"], Value::Null);
        drop(api);
        fs::remove_file(path).unwrap();
    }

    fn available_new_after_shown(shown_at: i64, now: i64, label: &str) -> u64 {
        let path = temp_path(label);
        let mut store = ReviewStore::open(&path).unwrap();
        store
            .record(input(label, "fixture-content-v1", shown_at, now))
            .unwrap();
        let api = ReviewApi::new(Catalog::fixture_for_tests(2, 1), store).unwrap();
        let available = api.queue_value(now)["new"].as_u64().unwrap();
        drop(api);
        fs::remove_file(path).unwrap();
        available
    }

    #[test]
    fn new_card_limit_keeps_legacy_reviews_in_the_rolling_window() {
        let now = 10 * DAY_MS;
        assert_eq!(
            available_new_after_shown(now - DAY_MS + 1, now, "inside"),
            0
        );
        assert_eq!(
            available_new_after_shown(now - DAY_MS - 1, now, "outside"),
            1
        );
    }

    #[test]
    fn median_latency_handles_even_and_odd_samples() {
        assert_eq!(median_latency(vec![]), None);
        assert_eq!(median_latency(vec![30, 10, 20]), Some(20));
        assert_eq!(median_latency(vec![40, 10, 30, 20]), Some(25));
    }
}
