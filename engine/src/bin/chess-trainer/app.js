// Chess Trainer frontend. Free play is stateless (positions are passed as a
// UCI move list); diagnostic review uses private, server-owned cards and a
// durable scheduler without sending game histories to the browser.
'use strict';

const START_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1';
const GLYPH = { p: '♟', n: '♞', b: '♝', r: '♜', q: '♛', k: '♚' };
const FILES = 'abcdefgh';
const REVIEW_CHOICE_COLOR = '#6ea8fe';

const GRADE_COLOR = {
  brilliant: '#21d0c3', best: '#f1c33f', excellent: '#6abf69', great: '#8bc34a',
  good: '#9aa7b2', inaccuracy: '#e0c14a', mistake: '#e8923a', blunder: '#e0544a',
};
const GRADE_LABEL = {
  brilliant: 'BRILLIANT', best: 'BEST MOVE', excellent: 'EXCELLENT', great: 'GREAT',
  good: 'GOOD', inaccuracy: 'INACCURACY', mistake: 'MISTAKE', blunder: 'BLUNDER',
};
const GRADE_GLYPH = {
  brilliant: '!!', best: '★', excellent: '!', great: '', good: '',
  inaccuracy: '?!', mistake: '?', blunder: '??',
};
const STREAK_GRADES = new Set(['brilliant', 'best', 'excellent', 'great', 'good']);
const BASE_PTS = {
  brilliant: 150, best: 100, excellent: 70, great: 45, good: 25,
  inaccuracy: 5, mistake: 0, blunder: 0,
};
const RANKS = [
  { name: 'Wood Pusher', min: 0 },
  { name: 'Coffee-house Player', min: 160 },
  { name: 'Club Player', min: 420 },
  { name: 'Tournament Player', min: 850 },
  { name: 'Expert', min: 1500 },
  { name: 'Candidate Master', min: 2400 },
  { name: 'Master', min: 3600 },
  { name: 'Grandmaster', min: 5200 },
  { name: 'Engine Whisperer', min: 7400 },
  { name: 'Stockfish', min: 10000 },
];

const S = {
  moves: [],          // current line, UCI (one game, or one rep)
  sans: [],           // SAN per ply (yours + opponent's)
  judg: [],           // per-ply verdict for YOUR moves THIS line: {grade,matched,book,cpLoss,hintUsed}
  history: [],        // every graded move this SESSION (across reps) — drives stats
  playerColor: 'w',
  depth: 15,          // Stockfish search depth (the judge / opponent strength)
  mode: 'game',       // 'game' (one full game) | 'reps' (short, auto-advancing)
  repTarget: 6,       // your moves per rep
  repIndex: 0,        // reps completed this session
  repMoves: 0,        // your graded moves in the current rep
  phase: 'menu',      // menu | user | judging | opponent | over
  flipped: false,
  sound: true,
  gen: 0,             // bumped on New drill to cancel stale async
  // viewed position (authoritative, from the server)
  viewAt: 0, fen: START_FEN, side: 'w', check: false, legal: [],
  lastMove: null, outcome: { status: 'ongoing' },
  selected: null,
  hintUsed: false,
  evalCp: null, evalMate: null,
  inBook: true, opening: null,
  // stats (derived; see recomputeStats)
  xp: 0, rankIdx: 0, streak: 0, bestStreak: 0,
  graded: 0, matched: 0, cpLossSum: 0,
  dist: zeroDist(),
};

const R = {
  configured: false,
  mode: false,
  phase: 'idle',       // idle | loading | answering | revealing | grading | saving | empty
  queue: { due: 0, new: 0, active: 0, reviewed24h: 0 },
  card: null,
  staged: null,
  answer: null,
  reviewedSession: 0,
  gen: 0,
};

function zeroDist() {
  return { brilliant: 0, best: 0, excellent: 0, great: 0, good: 0, inaccuracy: 0, mistake: 0, blunder: 0 };
}

const $ = id => document.getElementById(id);
const boardEl = $('board');
const qs = o => Object.entries(o)
  .filter(([, v]) => v !== '' && v != null)
  .map(([k, v]) => k + '=' + encodeURIComponent(v))
  .join('&');
const getJSON = async url => (await fetch(url)).json();
const sleep = ms => new Promise(r => setTimeout(r, ms));
const REVIEW_TOKEN = document.querySelector('meta[name="review-token"]')?.content || '';

async function postReview(path, body) {
  const response = await fetch(path, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-Chess-Review-Token': REVIEW_TOKEN,
    },
    body: JSON.stringify(body),
  });
  let data;
  try { data = await response.json(); }
  catch { data = { error: `request failed (${response.status})` }; }
  if (!response.ok || data.error) throw new Error(data.error || `request failed (${response.status})`);
  return data;
}

// --- private diagnostic review ---

function normalizedQueue(value) {
  const q = value?.queue || value || {};
  const count = key => Math.max(0, Number(q[key]) || 0);
  return {
    due: count('due'),
    new: count('new'),
    active: count('active'),
    reviewed24h: count('reviewed24h'),
  };
}

function renderReviewProgress() {
  const q = R.queue;
  $('review-due').textContent = q.due;
  $('review-new').textContent = q.new;
  $('review-active').textContent = q.active;
  $('review-done').textContent = q.reviewed24h;
  const remaining = q.due + q.new;
  const windowTotal = q.reviewed24h + remaining;
  const pct = windowTotal ? 100 * q.reviewed24h / windowTotal : 100;
  $('review-progress-fill').style.width = `${Math.max(0, Math.min(100, pct))}%`;
  $('review-progress-label').textContent = remaining
    ? `${remaining} remaining`
    : 'queue clear';
  const label = remaining ? `Review ${remaining}` : 'Review';
  $('btn-review').textContent = label;
}

function setReviewTags(tags) {
  const box = $('review-tags');
  box.textContent = '';
  for (const tag of Array.isArray(tags) ? tags : []) {
    const badge = document.createElement('span');
    badge.className = 'badge';
    badge.textContent = String(tag).replaceAll('-', ' ');
    box.appendChild(badge);
  }
}

function setReviewMode(on) {
  R.mode = on;
  document.body.classList.toggle('review-mode', on);
  $('btn-review').classList.toggle('primary', on);
  $('btn-new').classList.toggle('primary', !on);
  $('btn-new').textContent = on ? 'Free play' : 'New drill';
  $('tagline').textContent = on
    ? 'Retrieve decisions from your own games'
    : 'Play like Stockfish — earn the moves it would make';
  updateButtons();
  fitBoard();
}

function resetFreeMenu() {
  S.gen++;
  S.phase = 'menu';
  S.moves = []; S.sans = []; S.judg = [];
  S.viewAt = 0; S.fen = START_FEN; S.side = 'w'; S.check = false; S.legal = [];
  S.lastMove = null; S.selected = null; S.flipped = false;
  clearArrows(); clearBanner(); showJudging(false);
  renderBoard(); renderMoves(); renderStatus(); updateButtons();
}

function leaveReviewMode() {
  if (!R.mode) return;
  R.gen++;
  R.phase = 'idle';
  R.card = null; R.staged = null; R.answer = null;
  setReviewMode(false);
  resetFreeMenu();
}

async function enterReviewMode() {
  if (!R.configured) return;
  S.gen++;
  R.gen++;
  R.reviewedSession = 0;
  setReviewMode(true);
  showJudging(false);
  if ($('dlg-new').open) $('dlg-new').close();
  if ($('dlg-summary').open) $('dlg-summary').close();
  await loadNextReview();
}

function resetReviewPanels() {
  $('review-prompt-card').hidden = false;
  $('review-answer-card').hidden = true;
  $('review-empty').hidden = true;
  $('review-lines').open = false;
  $('review-move-entry').value = '';
  $('review-move-entry').disabled = false;
  $('review-reason').value = '';
  $('review-reason').disabled = false;
  $('review-staged').textContent = 'No move selected.';
  $('review-staged').classList.remove('ready');
  $('review-reveal').disabled = true;
  $('review-give-up').disabled = false;
  for (const button of document.querySelectorAll('[data-review-grade]')) button.disabled = false;
}

async function loadNextReview() {
  if (!R.mode) return;
  const gen = ++R.gen;
  R.phase = 'loading';
  R.card = null; R.staged = null; R.answer = null;
  resetReviewPanels();
  clearArrows(); clearBanner();
  $('review-prompt').textContent = 'Loading your review queue…';
  $('review-instruction').textContent = 'Your game histories remain on the local server.';
  $('status').textContent = 'Loading the next private review…';
  S.legal = []; S.selected = null; S.lastMove = null;
  renderBoard();

  let data;
  try {
    data = await postReview('/api/review/next', {});
  } catch {
    if (gen !== R.gen || !R.mode) return;
    showReviewLoadError('Review service is unreachable.');
    return;
  }
  if (gen !== R.gen || !R.mode) return;
  if (data.error) {
    showReviewLoadError(data.error);
    return;
  }
  if (data.configured === false) {
    R.configured = false;
    showReviewLoadError('No private review deck is configured.');
    return;
  }
  if (data.queue) R.queue = normalizedQueue(data.queue);
  renderReviewProgress();
  if (!data.card) {
    R.phase = 'empty';
    $('review-prompt-card').hidden = true;
    $('review-empty').hidden = false;
    $('review-empty-title').textContent = 'Queue clear';
    $('review-empty-detail').textContent = R.reviewedSession
      ? `You reviewed ${R.reviewedSession} position${R.reviewedSession === 1 ? '' : 's'} this session.`
      : 'No curated cards are due right now.';
    $('status').textContent = 'Review complete — come back when the scheduler has something due.';
    S.fen = START_FEN; S.side = 'w'; S.check = false; S.legal = []; S.lastMove = null;
    renderBoard(); updateButtons();
    return;
  }

  const card = data.card;
  R.card = card;
  R.phase = 'answering';
  S.phase = 'review';
  S.moves = []; S.sans = []; S.judg = []; S.viewAt = 0;
  S.fen = card.fen || START_FEN;
  S.side = S.fen.split(/\s+/)[1] || (card.orientation === 'black' ? 'b' : 'w');
  S.check = Boolean(card.check);
  S.legal = Array.isArray(card.legal) ? card.legal : [];
  S.lastMove = null; S.selected = null;
  S.flipped = card.orientation === 'black' || card.orientation === 'b';
  setReviewTags(card.tags);
  $('review-prompt').textContent = typeof card.prompt === 'string'
    ? card.prompt
    : (card.prompt?.text || 'What would you play?');
  $('review-instruction').textContent = 'Choose a move, then write the idea you calculated before revealing.';
  renderBoard();
  renderStatus();
  updateButtons();
  if (data.recoveredAnswer) {
    R.phase = 'grading';
    renderReviewAnswer(data.recoveredAnswer);
    updateButtons();
    return;
  }
  $('review-prompt').focus({ preventScroll: true });
}

function showReviewLoadError(message) {
  R.phase = 'empty';
  $('review-prompt-card').hidden = true;
  $('review-answer-card').hidden = true;
  $('review-empty').hidden = false;
  $('review-empty-title').textContent = 'Review unavailable';
  $('review-empty-detail').textContent = message;
  $('status').textContent = message;
  updateButtons();
}

function stageReviewMove(uci, focusReason = false) {
  if (!R.mode || R.phase !== 'answering') return;
  const legal = S.legal.find(move => move.uci === uci);
  if (!legal) return;
  R.staged = { uci: legal.uci, san: legal.san || legal.uci };
  S.selected = null;
  S.lastMove = legal.uci;
  renderBoard();
  clearArrows();
  drawArrow(legal.uci, REVIEW_CHOICE_COLOR);
  $('review-move-entry').value = R.staged.san;
  $('review-staged').textContent = `Selected: ${R.staged.san}. Choose another move to change it.`;
  $('review-staged').classList.add('ready');
  $('status').textContent = `Selected ${R.staged.san} — write the reason before revealing.`;
  updateReviewRevealButton();
  if (focusReason) $('review-reason').focus();
}

function normalizedReviewMoveText(value) {
  return String(value).trim().replaceAll('0', 'O').toLocaleLowerCase();
}

function clearStagedReviewMove() {
  if (!R.staged) return;
  R.staged = null;
  S.lastMove = null;
  S.selected = null;
  $('review-staged').textContent = 'No move selected.';
  $('review-staged').classList.remove('ready');
  clearArrows();
  renderBoard();
  updateReviewRevealButton();
}

function stagedMoveMatchesEntry() {
  if (!R.staged) return false;
  const typed = normalizedReviewMoveText($('review-move-entry').value);
  return typed === normalizedReviewMoveText(R.staged.uci)
    || typed === normalizedReviewMoveText(R.staged.san);
}

function stageTypedReviewMove() {
  if (!R.mode || R.phase !== 'answering') return;
  const typed = normalizedReviewMoveText($('review-move-entry').value);
  if (!typed) {
    clearStagedReviewMove();
    return;
  }
  const move = S.legal.find(candidate =>
    candidate.uci.toLocaleLowerCase() === typed
    || normalizedReviewMoveText(candidate.san) === typed);
  if (!move) {
    clearStagedReviewMove();
    $('status').textContent = 'That is not a legal SAN or UCI move in this position.';
    $('review-move-entry').select();
    return;
  }
  stageReviewMove(move.uci, true);
}

function updateReviewRevealButton() {
  const hasReason = Boolean($('review-reason').value.trim());
  $('review-reveal').disabled = R.phase !== 'answering' || !R.staged || !hasReason;
}

function lineText(line) {
  if (Array.isArray(line)) return line.join(' ');
  return line == null || line === '' ? '—' : String(line);
}

function formatEvidence(evidence) {
  if (!evidence) return '';
  const bits = [];
  if (evidence.assurance) bits.push(String(evidence.assurance).replaceAll('-', ' '));
  if (Number.isFinite(Number(evidence.nodes))) {
    bits.push(`${Number(evidence.nodes).toLocaleString()} confirmation nodes`);
  }
  if (evidence.evidenceVersion) {
    const digest = String(evidence.evidenceVersion).replace(/^sha256:/, '');
    bits.push(`evidence ${digest.slice(0, 12)}…`);
  }
  if (evidence.analysisConfigVersion) {
    const digest = String(evidence.analysisConfigVersion).replace(/^sha256:/, '');
    bits.push(`analysis ${digest.slice(0, 12)}…`);
  }
  return bits.join(' · ');
}

async function revealReview(gaveUp) {
  if (!R.mode || R.phase !== 'answering' || !R.card) return;
  const gen = R.gen;
  const reasonPresent = Boolean($('review-reason').value.trim());
  if (!gaveUp && (!stagedMoveMatchesEntry() || !reasonPresent)) {
    clearStagedReviewMove();
    $('status').textContent = 'Choose a legal move again before revealing.';
    return;
  }
  R.phase = 'revealing';
  $('review-reveal').disabled = true;
  $('review-give-up').disabled = true;
  $('review-move-entry').disabled = true;
  $('review-reason').disabled = true;
  $('judging-label').textContent = 'Revealing the stored reference…';
  showJudging(true);
  let data;
  try {
    data = await postReview('/api/review/reveal', {
      attemptId: R.card.attemptId,
      moveUci: gaveUp ? null : R.staged.uci,
      reasonPresent,
      gaveUp: Boolean(gaveUp),
    });
  } catch (error) {
    if (gen !== R.gen || !R.mode) return;
    showJudging(false);
    R.phase = 'answering';
    $('review-give-up').disabled = false;
    $('review-move-entry').disabled = false;
    $('review-reason').disabled = false;
    updateReviewRevealButton();
    $('status').textContent = `Could not reveal: ${error.message}`;
    return;
  }
  if (gen !== R.gen || !R.mode) return;
  showJudging(false);
  if (!data.legal) {
    R.phase = 'answering';
    $('review-give-up').disabled = false;
    $('review-move-entry').disabled = false;
    $('review-reason').disabled = false;
    R.staged = null; S.lastMove = null;
    clearArrows(); renderBoard(); updateReviewRevealButton();
    $('status').textContent = 'That staged move is no longer legal; choose another move.';
    return;
  }
  R.phase = 'grading';
  renderReviewAnswer(data);
  updateButtons();
}

function renderReviewAnswer(data) {
  R.answer = data;
  const gaveUp = data.choice == null && data.referenceMatch == null;
  $('review-prompt-card').hidden = true;
  $('review-answer-card').hidden = false;
  const result = $('review-result');
  result.classList.remove('match', 'different');
  if (gaveUp) {
    result.textContent = 'Reference revealed';
  } else if (data.referenceMatch) {
    result.textContent = 'Matched the bounded reference';
    result.classList.add('match');
  } else {
    result.textContent = 'Different from the bounded reference';
    result.classList.add('different');
  }
  const committedChoice = data.choice?.san || data.choice?.uci || R.staged?.san || R.staged?.uci;
  $('review-choice').textContent = gaveUp ? 'No move submitted' : (committedChoice || '—');
  $('review-reference').textContent = data.reference?.san || data.reference?.uci || '—';
  $('review-reference-line').textContent = lineText(data.reference?.lineSan);
  $('review-original-line').textContent = lineText(data.original?.lineSan);

  const originalMove = data.original?.san || data.original?.uci || 'the recorded move';
  const loss = data.original?.lossCp;
  const bucket = data.original?.lossBucket ? ` (${data.original.lossBucket} loss bucket)` : '';
  $('review-original-note').textContent = loss == null
    ? `In the original game, ${originalMove} was the move selected for review${bucket}.`
    : `In the original game, ${originalMove} was estimated ${loss} centipawn-equivalent worse than the reference${bucket}. This estimate does not grade a different move you chose today.`;

  const explanation = $('review-explanation');
  explanation.textContent = data.explanation || '';
  explanation.hidden = !data.explanation;
  const criterion = $('review-criterion');
  criterion.textContent = data.successCriterion ? `Success criterion: ${data.successCriterion}` : '';
  criterion.hidden = !data.successCriterion;
  $('review-evidence').textContent = formatEvidence(data.evidence);
  clearArrows();
  if (data.reference?.uci) drawArrow(data.reference.uci, GRADE_COLOR.best);
  $('status').textContent = 'Grade the tactical idea honestly; exact reference agreement is not required.';
  $('review-result').focus({ preventScroll: true });
}

function dueText(dueAtMs, intervalMs) {
  const interval = Number(intervalMs);
  if (Number.isFinite(interval) && interval > 0) {
    if (interval < 60 * 60 * 1000) return 'later today';
    if (interval < 36 * 60 * 60 * 1000) return 'tomorrow';
    const days = Math.round(interval / (24 * 60 * 60 * 1000));
    return `in ${days} days`;
  }
  const due = Number(dueAtMs);
  if (!Number.isFinite(due)) return 'on its new schedule';
  return new Date(due).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });
}

async function gradeReview(outcome) {
  if (!R.mode || R.phase !== 'grading' || !R.card) return;
  const gen = R.gen;
  R.phase = 'saving';
  for (const button of document.querySelectorAll('[data-review-grade]')) button.disabled = true;
  let data;
  try {
    data = await postReview('/api/review/grade', { attemptId: R.card.attemptId, outcome });
  } catch (error) {
    if (gen !== R.gen || !R.mode) return;
    R.phase = 'grading';
    for (const button of document.querySelectorAll('[data-review-grade]')) button.disabled = false;
    $('status').textContent = `Could not save the grade: ${error.message}`;
    return;
  }
  if (gen !== R.gen || !R.mode) return;
  if (data.progress) R.queue = normalizedQueue(data.progress);
  renderReviewProgress();
  R.reviewedSession++;
  const applied = data.appliedOutcome || outcome;
  const adjusted = applied !== outcome ? ` Recorded as ${applied} because the reference was revealed.` : '';
  $('status').textContent = `Saved — this position is due ${dueText(data.dueAtMs, data.intervalMs)}.${adjusted}`;
  if (S.sound) playGradeSound(applied === 'pass' ? 'excellent' : applied === 'partial' ? 'inaccuracy' : 'mistake');
  R.phase = 'loading';
  const scheduledGen = R.gen;
  setTimeout(() => {
    if (R.mode && R.gen === scheduledGen) loadNextReview();
  }, 600);
}

async function initializeReview() {
  let data;
  try { data = await getJSON('/api/review/progress'); }
  catch { data = null; }
  if (data && !data.error && data.configured) {
    R.configured = true;
    R.queue = normalizedQueue(data.queue || data.progress);
    $('btn-review').hidden = false;
    $('privacy-badge').hidden = false;
    renderReviewProgress();
    await enterReviewMode();
    return;
  }
  $('dlg-new').showModal();
}

// --- server state ---

async function refreshView(at) {
  let d;
  try {
    d = await getJSON('/api/state?' + qs({ moves: S.moves.slice(0, at).join(' ') }));
  } catch {
    $('status').textContent = 'server unreachable';
    return null;
  }
  if (d.error) return null;
  S.viewAt = at;
  S.fen = d.fen; S.side = d.side; S.check = d.check;
  S.legal = d.legal; S.lastMove = d.lastMove; S.outcome = d.outcome;
  if (at === S.moves.length) { S.inBook = d.inBook; S.opening = d.opening; }
  renderBoard();
  renderMoves();
  return d;
}

// --- the drill loop ---

function newSession(color, depth, mode) {
  if (R.mode) leaveReviewMode();
  S.playerColor = color === 'r' ? (Math.random() < 0.5 ? 'w' : 'b') : color;
  S.depth = depth;
  S.mode = mode;
  S.history = [];          // session stats start fresh
  S.repIndex = 0;
  S.bestStreak = 0;
  S.flipped = S.playerColor === 'b';
  $('btn-finish').disabled = false;
  startLine();
}

// Begin a fresh line — a new game, or the next rep. Session stats (S.history)
// carry over; only the on-board line resets.
function startLine() {
  S.gen++;
  S.moves = []; S.sans = []; S.judg = [];
  S.repMoves = 0;
  S.hintUsed = false; S.evalCp = null; S.evalMate = null;
  S.inBook = true; S.opening = null;
  S.outcome = { status: 'ongoing' };
  recomputeStats();
  clearArrows(); clearBanner();
  renderVerdictPlaceholder();
  setEvalBar();
  renderRepIndicator();
  advanceFrom(0);
}

function nextRep() {
  S.repIndex++;
  const m = S.graded ? Math.round(100 * S.matched / S.graded) : 0;
  flashBanner(`REP ${S.repIndex} ✓`, `${S.repIndex} done · ${m}% engine-match so far`, GRADE_COLOR.best);
  if (S.sound) playGradeSound('excellent');
  const gen = S.gen;
  setTimeout(() => { if (!R.mode && gen === S.gen) startLine(); }, 250);
}

function renderRepIndicator() {
  const el = $('rep-indicator');
  if (S.mode === 'reps') { el.hidden = false; el.textContent = `Rep ${S.repIndex + 1}`; }
  else el.hidden = true;
}

async function advanceFrom(at) {
  const g = S.gen;
  const d = await refreshView(at);
  if (g !== S.gen || !d) return;
  advance();
}

function advance() {
  if (S.outcome.status !== 'ongoing') { finishOver(); return; }
  if (S.side === S.playerColor) {
    S.phase = 'user';
    updateButtons();
    renderStatus();
  } else {
    opponentMove();
  }
}

async function onUserMove(uci) {
  if (S.phase !== 'user' || S.viewAt !== S.moves.length) return;
  const g = S.gen;
  const movesBefore = S.moves.slice();
  const ply = movesBefore.length;

  S.phase = 'judging';
  S.selected = null;
  clearArrows();
  updateButtons();
  // Show the move immediately, then let Stockfish judge it.
  S.moves.push(uci);
  await refreshView(S.moves.length);
  if (g !== S.gen) return;
  showJudging(true);
  renderStatus();

  let d;
  try {
    d = await getJSON('/api/grade?' + qs({
      moves: movesBefore.join(' '), move: uci, depth: S.depth, multipv: 3,
    }));
  } catch { d = { error: 'network' }; }
  if (g !== S.gen) return;
  showJudging(false);

  if (d.error) { // roll back the move and let them try again
    S.moves = movesBefore;
    await refreshView(S.moves.length);
    S.phase = 'user'; updateButtons(); renderStatus();
    return;
  }

  S.sans[ply] = d.playedSan;
  S.repMoves++;
  applyGrade(d, ply);
  S.evalCp = d.evalCp; S.evalMate = d.evalMate; setEvalBar();
  // panel book/opening reflect the live line after the move
  await refreshView(S.moves.length);
  if (g !== S.gen) return;
  renderOpening();

  if (d.outcome.status !== 'ongoing') {                  // your move ended the game
    if (S.mode === 'reps') { S.phase = 'opponent'; await sleep(1050); if (g !== S.gen) return; nextRep(); return; }
    S.phase = 'over'; await sleep(900); finishOver(); return;
  }
  if (S.mode === 'reps' && S.repMoves >= S.repTarget) {  // rep complete → next rep
    S.phase = 'opponent';                                // lock input during the beat
    $('status').textContent = 'Rep complete — next one coming up…';
    await sleep(1050);
    if (g !== S.gen) return;
    nextRep();
    return;
  }
  await sleep(720);                  // let the reward animation breathe
  if (g !== S.gen) return;
  opponentMove();
}

async function opponentMove() {
  const g = S.gen;
  S.phase = 'opponent';
  updateButtons();
  renderStatus();
  let r;
  try {
    r = await getJSON('/api/reply?' + qs({ moves: S.moves.join(' '), depth: S.depth }));
  } catch { r = { error: 'network' }; }
  if (g !== S.gen) return;
  if (r.error) { S.phase = 'user'; updateButtons(); renderStatus(); return; }
  if (r.over) {
    await refreshView(S.moves.length);
    if (S.mode === 'reps') { nextRep(); return; }
    finishOver();
    return;
  }

  S.moves.push(r.uci);
  S.sans[S.moves.length - 1] = r.san;
  if (r.opening) S.opening = r.opening;
  clearArrows();        // the "you missed this" arrow belonged to the prior position
  await refreshView(S.moves.length);
  if (g !== S.gen) return;
  renderOpening();
  advance();
}

function finishOver() {
  S.phase = 'over';
  updateButtons();
  renderStatus();
  showSummary();
}

// --- grading rewards ---

function applyGrade(d, ply) {
  const rec = { grade: d.grade, matched: d.matched, book: d.book, cpLoss: d.cpLoss, hintUsed: S.hintUsed };
  S.judg[ply] = rec;       // for this line's move list / retry
  S.history.push(rec);     // for session-wide stats (spans reps)
  const prevRank = S.rankIdx;
  recomputeStats();

  showBanner(d);
  boardFlash(d.grade);
  if (S.sound) playGradeSound(d.grade);
  if ((d.grade === 'best' || d.grade === 'brilliant') && !S.hintUsed) {
    confettiBurst(d.grade === 'brilliant' ? 'brilliant' : 'best');
  }
  // The engine's preferred move is intentionally never drawn — you learn from
  // the consequence (d.reason), not the answer.
  renderVerdict(d);
  renderStats();
  renderRank();
  if (S.rankIdx > prevRank) celebrateRankUp();
  S.hintUsed = false;
}

function recomputeStats() {
  let xp = 0, graded = 0, matched = 0, sum = 0, streak = 0, best = 0;
  const dist = zeroDist();
  for (const j of S.history) {
    if (!j) continue;
    graded++;
    if (!j.hintUsed && j.matched) matched++;
    sum += j.cpLoss;
    dist[j.grade] = (dist[j.grade] || 0) + 1;
    if (STREAK_GRADES.has(j.grade)) { streak++; best = Math.max(best, streak); }
    else streak = 0;
    let pts = BASE_PTS[j.grade] || 0;
    if (j.hintUsed) pts = Math.round(pts * 0.4);
    pts = Math.round(pts * (1 + Math.min(streak, 10) * 0.1));
    xp += pts;
  }
  S.xp = xp; S.graded = graded; S.matched = matched; S.cpLossSum = sum;
  S.dist = dist; S.streak = streak; S.bestStreak = Math.max(S.bestStreak, best);
  S.rankIdx = RANKS.reduce((acc, r, i) => (xp >= r.min ? i : acc), 0);
}

function flashBanner(label, sub, color) {
  const banner = $('banner');
  banner.innerHTML =
    `<div class="b-label" style="color:${color};box-shadow:0 6px 24px ${color}55,0 0 0 2px ${color}aa inset">${label}</div>` +
    (sub ? `<div class="b-sub">${sub}</div>` : '');
  banner.classList.remove('show');
  void banner.offsetWidth;          // restart the animation
  banner.classList.add('show');
}

function showBanner(d) {
  const color = GRADE_COLOR[d.grade] || '#ccc';
  const label = GRADE_LABEL[d.grade] + (GRADE_GLYPH[d.grade] ? ' ' + GRADE_GLYPH[d.grade] : '');
  let sub;
  if (d.outcome && d.outcome.status === 'checkmate') sub = d.matched ? 'Checkmate — clinical!' : 'Checkmate!';
  else if (d.matched) sub = 'You found Stockfish’s move';
  else if (d.reason) sub = d.reason;                 // the "why", not the move
  else sub = `Solid — lost only ${(d.cpLoss / 100).toFixed(2)}`;
  if (d.book) sub = '📖 ' + sub;
  if (S.hintUsed) sub += ' · hint used';
  flashBanner(label, sub, color);
}

function clearBanner() {
  const b = $('banner');
  b.classList.remove('show');
  b.innerHTML = '';
}

function boardFlash(grade) {
  const cls = {
    brilliant: 'flash-brill', best: 'flash-best',
    excellent: 'flash-good', great: 'flash-good', good: 'flash-good',
    inaccuracy: 'flash-mid', mistake: 'flash-bad', blunder: 'flash-bad',
  }[grade];
  const all = ['flash-good', 'flash-best', 'flash-brill', 'flash-mid', 'flash-bad', 'shake'];
  boardEl.classList.remove(...all);
  void boardEl.offsetWidth;
  if (cls) boardEl.classList.add(cls);
  if (grade === 'blunder' || grade === 'mistake') boardEl.classList.add('shake');
  setTimeout(() => boardEl.classList.remove(...all), 700);
}

function celebrateRankUp() {
  const el = $('rank-name');
  el.classList.remove('up'); void el.offsetWidth; el.classList.add('up');
  confettiBurst('rankup');
  if (S.sound) playRankUp();
}

// --- stats / rank rendering ---

function bump(el) { el.classList.remove('bump'); void el.offsetWidth; el.classList.add('bump'); }

function renderStats() {
  // Per-move accuracy, averaged — a steep curve so a real error actually bites
  // (a hung piece ≈ 0%, a 50cp slip ≈ 70%).
  const acc = S.history.length
    ? Math.round(S.history.reduce((a, j) => a + 100 * Math.exp(-j.cpLoss / 140), 0) / S.history.length)
    : null;
  const match = S.graded ? Math.round(100 * S.matched / S.graded) : null;
  setStat('stat-streak', S.streak);
  setStat('stat-match', match == null ? '—' : match + '%');
  setStat('stat-acc', acc == null ? '—' : acc + '%');
  setStat('stat-moves', S.graded);
  const fire = S.streak >= 9 ? '🔥🔥🔥' : S.streak >= 6 ? '🔥🔥' : S.streak >= 3 ? '🔥' : '';
  $('streak-fire').textContent = fire;
}

function setStat(id, val) {
  const el = $(id);
  if (el.textContent !== String(val)) { el.textContent = val; bump(el); }
}

function renderRank() {
  const r = RANKS[S.rankIdx];
  const next = RANKS[S.rankIdx + 1];
  $('rank-name').textContent = r.name;
  $('rank-pts').textContent = S.xp + ' XP';
  if (next) {
    const span = next.min - r.min;
    const pct = Math.max(0, Math.min(100, 100 * (S.xp - r.min) / span));
    $('rank-fill').style.width = pct + '%';
    $('rank-next').textContent = `${next.min - S.xp} XP to ${next.name}`;
  } else {
    $('rank-fill').style.width = '100%';
    $('rank-next').textContent = 'Top rank reached — you are the engine.';
  }
}

function renderOpening() {
  const name = S.opening
    ? S.opening
    : (!S.moves.length ? '—' : (S.inBook ? 'Opening theory' : 'Off-book line'));
  $('opening-name').textContent = name;
  const badge = $('book-badge');
  if (S.inBook && S.moves.length) {
    badge.hidden = false; badge.textContent = 'in book'; badge.classList.remove('off');
  } else if (S.moves.length) {
    badge.hidden = false; badge.textContent = 'off book'; badge.classList.add('off');
  } else {
    badge.hidden = true;
  }
}

function renderVerdictPlaceholder() {
  $('verdict').hidden = true;
  $('verdict-placeholder').hidden = false;
}

function renderVerdict(d) {
  $('verdict-placeholder').hidden = true;
  $('verdict').hidden = false;
  const grade = $('verdict-grade');
  grade.textContent = GRADE_LABEL[d.grade] + (GRADE_GLYPH[d.grade] ? ' ' + GRADE_GLYPH[d.grade] : '');
  grade.style.color = GRADE_COLOR[d.grade];
  $('verdict-eval').textContent = fmtEvalWhite(d.evalCp, d.evalMate);
  const loss = (d.cpLoss / 100).toFixed(2);
  let html;
  if (d.matched) {
    html = `<b>${d.playedSan}</b> is the engine’s top choice` + (d.book ? ' — straight theory.' : '.');
  } else if (d.reason) {
    html = `<b>${d.playedSan}</b> — lost ${loss}. ${d.reason}`;
  } else {
    html = `<b>${d.playedSan}</b> — solid, lost only ${loss}. Not the engine’s pick, but close.`;
  }
  $('verdict-detail').innerHTML = html;
}

// --- eval bar ---

function setEvalBar() {
  const fill = $('evalbar-white');
  const num = $('evalbar-num');
  if (S.evalCp == null && S.evalMate == null) {
    fill.style.height = '50%'; num.textContent = '·'; num.classList.remove('black');
    return;
  }
  let pct;
  if (S.evalMate != null) pct = S.evalMate > 0 ? 100 : 0;
  else {
    pct = 50 + 50 * (2 / (1 + Math.exp(-0.00368208 * S.evalCp)) - 1);
    pct = Math.max(2, Math.min(98, pct));
  }
  fill.style.height = pct + '%';
  num.textContent = S.evalMate != null ? '#' + Math.abs(S.evalMate) : Math.abs(S.evalCp / 100).toFixed(1);
  num.classList.toggle('black', pct < 12);
}

function fmtEvalWhite(cp, mate) {
  if (mate != null) return (mate < 0 ? '#-' : '#') + Math.abs(mate);
  if (cp == null) return '—';
  const v = cp / 100;
  return (v > 0 ? '+' : '') + v.toFixed(2);
}

// --- board rendering (adapted from chess-web) ---

function fenPieces(fen) {
  const out = {};
  const rows = fen.split(' ')[0].split('/');
  for (let r = 0; r < 8; r++) {
    let f = 0;
    for (const c of rows[r]) {
      if (c >= '1' && c <= '8') { f += +c; continue; }
      out[FILES[f] + (8 - r)] = c;
      f++;
    }
  }
  return out;
}

function squareName(row, col) {
  const f = S.flipped ? 7 - col : col;
  const r = S.flipped ? row : 7 - row;
  return FILES[f] + (r + 1);
}

function renderBoard() {
  const pieces = fenPieces(S.fen);
  const kingSq = Object.keys(pieces).find(sq => pieces[sq] === (S.side === 'w' ? 'K' : 'k'));
  const lastFrom = S.lastMove && S.lastMove.slice(0, 2);
  const lastTo = S.lastMove && S.lastMove.slice(2, 4);
  const targets = S.selected
    ? new Set(S.legal.filter(m => m.uci.slice(0, 2) === S.selected).map(m => m.uci.slice(2, 4)))
    : new Set();

  boardEl.textContent = '';
  for (let row = 0; row < 8; row++) {
    for (let col = 0; col < 8; col++) {
      const sq = squareName(row, col);
      const el = document.createElement('div');
      const f = FILES.indexOf(sq[0]), r = +sq[1] - 1;
      el.className = 'sq ' + ((f + r) % 2 ? 'light' : 'dark');
      el.dataset.sq = sq;
      if (sq === lastFrom || sq === lastTo) el.classList.add('last');
      if (sq === S.selected) el.classList.add('sel');
      if (S.check && sq === kingSq) el.classList.add('check');

      if (col === 0) {
        const c = document.createElement('span');
        c.className = 'coord rank'; c.textContent = sq[1]; el.appendChild(c);
      }
      if (row === 7) {
        const c = document.createElement('span');
        c.className = 'coord file'; c.textContent = sq[0]; el.appendChild(c);
      }

      const p = pieces[sq];
      if (p) {
        const span = document.createElement('span');
        const white = p === p.toUpperCase();
        span.className = 'piece ' + (white ? 'w' : 'b');
        span.textContent = GLYPH[p.toLowerCase()];
        el.appendChild(span);
      }
      if (targets.has(sq)) {
        const dot = document.createElement('div');
        dot.className = 'dot';
        if (p) el.classList.add('cap');
        el.appendChild(dot);
      }
      boardEl.appendChild(el);
    }
  }
}

function canPlay() {
  if (R.mode) return R.phase === 'answering';
  return S.phase === 'user' && S.viewAt === S.moves.length;
}
function canMoveFrom(sq) { return canPlay() && S.legal.some(m => m.uci.slice(0, 2) === sq); }

function submitBoardMove(uci) {
  if (R.mode) stageReviewMove(uci);
  else onUserMove(uci);
}

function tryMove(from, to) {
  const cands = S.legal.filter(m => m.uci.slice(0, 2) === from && m.uci.slice(2, 4) === to);
  if (!cands.length) return false;
  if (cands.length === 1 && cands[0].uci.length === 4) {
    submitBoardMove(cands[0].uci);
  } else {
    showPromotion(from, to);
  }
  return true;
}

function showPromotion(from, to) {
  const overlay = document.createElement('div');
  overlay.id = 'promo';
  for (const p of ['q', 'r', 'b', 'n']) {
    const b = document.createElement('button');
    b.textContent = GLYPH[p];
    b.style.color = S.side === 'w' ? '#fff' : '#222';
    b.style.background = S.side === 'w' ? '#666' : '#ddd';
    b.onclick = e => { e.stopPropagation(); overlay.remove(); submitBoardMove(from + to + p); };
    overlay.appendChild(b);
  }
  overlay.onclick = () => overlay.remove();
  boardEl.appendChild(overlay);
}

// pointer input: click-click and drag
let drag = null;

function squareAt(x, y) {
  const rect = boardEl.getBoundingClientRect();
  const col = Math.floor((x - rect.left) / (rect.width / 8));
  const row = Math.floor((y - rect.top) / (rect.height / 8));
  if (col < 0 || col > 7 || row < 0 || row > 7) return null;
  return squareName(row, col);
}

boardEl.addEventListener('pointerdown', e => {
  if (e.button !== 0) return;
  const sqEl = e.target.closest('.sq');
  if (!sqEl) return;
  const sq = sqEl.dataset.sq;
  if (S.selected && S.selected !== sq && tryMove(S.selected, sq)) { S.selected = null; return; }
  if (canMoveFrom(sq)) {
    S.selected = sq;
    renderBoard();
    const pieceEl = boardEl.querySelector(`[data-sq="${sq}"] .piece`);
    if (pieceEl) {
      e.preventDefault();
      boardEl.setPointerCapture(e.pointerId);
      drag = { from: sq, pieceEl, ghost: null };
    }
  } else { S.selected = null; renderBoard(); }
});

boardEl.addEventListener('pointermove', e => {
  if (!drag) return;
  if (!drag.ghost) {
    const g = document.createElement('div');
    g.id = 'ghost';
    g.textContent = drag.pieceEl.textContent;
    g.className = drag.pieceEl.className.replace('piece', '');
    g.style.color = getComputedStyle(drag.pieceEl).color;
    g.style.textShadow = getComputedStyle(drag.pieceEl).textShadow;
    document.body.appendChild(g);
    drag.ghost = g;
    drag.pieceEl.classList.add('dragging');
  }
  drag.ghost.style.left = e.clientX + 'px';
  drag.ghost.style.top = e.clientY + 'px';
});

boardEl.addEventListener('pointerup', e => {
  if (!drag) return;
  const { from, ghost, pieceEl } = drag;
  drag = null;
  if (ghost) {
    ghost.remove();
    pieceEl.classList.remove('dragging');
    const to = squareAt(e.clientX, e.clientY);
    if (to && to !== from && tryMove(from, to)) { S.selected = null; return; }
    renderBoard();
  }
});

boardEl.addEventListener('pointercancel', () => {
  if (drag && drag.ghost) { drag.ghost.remove(); drag.pieceEl.classList.remove('dragging'); }
  drag = null;
});

// --- best-move / hint arrows ---

function sqCenter(sq) {
  const f = FILES.indexOf(sq[0]), r = +sq[1] - 1;
  const col = S.flipped ? 7 - f : f;
  const row = S.flipped ? r : 7 - r;
  return { x: col + 0.5, y: row + 0.5 };
}

function clearArrows() { $('arrows').innerHTML = ''; }

function drawArrow(uci, color) {
  if (!uci || uci.length < 4) return;
  const a = sqCenter(uci.slice(0, 2)), b = sqCenter(uci.slice(2, 4));
  const ang = Math.atan2(b.y - a.y, b.x - a.x);
  const head = 0.34, hw = 0.17;
  const bx = b.x - Math.cos(ang) * head, by = b.y - Math.sin(ang) * head;
  const tipx = b.x - Math.cos(ang) * 0.05, tipy = b.y - Math.sin(ang) * 0.05;
  const lx = bx - Math.cos(ang - Math.PI / 2) * hw, ly = by - Math.sin(ang - Math.PI / 2) * hw;
  const rx = bx - Math.cos(ang + Math.PI / 2) * hw, ry = by - Math.sin(ang + Math.PI / 2) * hw;
  $('arrows').innerHTML =
    `<line x1="${a.x}" y1="${a.y}" x2="${bx}" y2="${by}" stroke="${color}" ` +
    `stroke-width="0.15" stroke-linecap="round" opacity="0.9"/>` +
    `<polygon points="${tipx},${tipy} ${lx},${ly} ${rx},${ry}" fill="${color}" opacity="0.9"/>`;
}

// --- move list / nav ---

function renderMoves() {
  const box = $('movelist');
  box.textContent = '';
  if (!S.sans.length) { box.innerHTML = '<span class="empty">No moves yet.</span>'; return; }
  let num = 1;
  for (let i = 0; i < S.sans.length; i++) {
    if (i % 2 === 0) {
      const n = document.createElement('span');
      n.className = 'num'; n.textContent = num + '.'; box.appendChild(n);
    }
    const span = document.createElement('span');
    span.className = 'mv' + (i + 1 === S.viewAt ? ' cur' : '');
    span.textContent = S.sans[i] || '…';
    const j = S.judg[i];
    if (j && GRADE_GLYPH[j.grade]) {
      const g = document.createElement('span');
      g.className = 'glyph g-' + j.grade;
      g.textContent = GRADE_GLYPH[j.grade];
      span.appendChild(g);
      span.title = GRADE_LABEL[j.grade];
    }
    span.onclick = () => navTo(i + 1);
    box.appendChild(span);
    if (i % 2 === 1) num++;
  }
  const cur = box.querySelector('.cur');
  if (cur) cur.scrollIntoView({ block: 'nearest' });
}

async function navTo(at) {
  if (!(S.phase === 'user' || S.phase === 'over')) return;
  at = Math.max(0, Math.min(S.moves.length, at));
  await refreshView(at);
  renderStatus();
}

// --- status ---

function gameOverText() {
  const o = S.outcome;
  if (o.status === 'checkmate') {
    const youWon = o.winner === S.playerColor;
    return `Checkmate — ${o.winner === 'w' ? 'White' : 'Black'} wins. ${youWon ? 'You delivered it!' : ''}`;
  }
  if (o.status === 'draw') return `Draw — ${o.reason}.`;
  return 'Game over.';
}

function renderStatus() {
  if (R.mode) {
    const text = {
      loading: 'Loading the next private review…',
      answering: `Your move — ${S.side === 'w' ? 'White' : 'Black'}. Calculate checks, captures, and threats before committing.`,
      revealing: 'Revealing the stored reference…',
      grading: 'Grade the retrieval honestly: the move and the reason both matter.',
      saving: 'Saving this review locally…',
      empty: 'Review complete — come back when the scheduler has something due.',
    }[R.phase] || 'Private diagnostic review.';
    $('status').textContent = text;
    return;
  }
  let t;
  if (S.phase === 'menu') t = 'Press <b>New drill</b> to begin.';
  else if (S.phase === 'over') t = gameOverText() + ' Press <b>New drill</b> to go again.';
  else if (S.viewAt < S.moves.length) t = `Reviewing ${S.viewAt}/${S.moves.length} — press <b>Live</b> to continue.`;
  else if (S.phase === 'judging') t = 'Stockfish is judging your move…';
  else if (S.phase === 'opponent') t = 'Opponent is playing theory…';
  else {
    const rep = S.mode === 'reps' ? `Rep ${S.repIndex + 1} · ` : '';
    t = `${rep}Your move — ${S.side === 'w' ? 'White' : 'Black'}${S.check ? ' · check!' : ''}. Play like Stockfish.`;
  }
  $('status').innerHTML = t;
}

function showJudging(on) {
  if (on && !R.mode) $('judging-label').textContent = 'Stockfish is judging…';
  $('judging').classList.toggle('show', on);
  $('board-stack').setAttribute('aria-busy', String(Boolean(on)));
}

function updateButtons() {
  if (R.mode) {
    $('btn-hint').disabled = true;
    $('btn-retry').disabled = true;
    return;
  }
  const live = canPlay();
  $('btn-hint').disabled = !live;
  // retry needs at least one graded move
  $('btn-retry').disabled = !(S.judg.some(Boolean) && (S.phase === 'user' || S.phase === 'over'));
}

// --- hint / retry ---

async function hint() {
  if (!canPlay()) return;
  const g = S.gen;
  $('status').textContent = 'Consulting the engine…';
  let d;
  try { d = await getJSON('/api/eval?' + qs({ moves: S.moves.join(' '), depth: S.depth })); }
  catch { return; }
  if (g !== S.gen || !d || d.error || d.over || !d.best || !d.best.uci) { renderStatus(); return; }
  clearArrows();
  drawArrow(d.best.uci, GRADE_COLOR.best);
  S.hintUsed = true;
  S.evalCp = d.cp; S.evalMate = d.mate; setEvalBar();
  $('status').innerHTML = 'Hint shown — this move won’t count toward your engine match.';
}

async function retry() {
  if (!(S.judg.some(Boolean) && (S.phase === 'user' || S.phase === 'over'))) return;
  S.gen++;                            // cancel any in-flight loop step
  let lp = -1;
  for (let i = 0; i < S.judg.length; i++) if (S.judg[i]) lp = i;
  if (lp < 0) return;
  S.moves.length = lp; S.sans.length = lp; S.judg.length = lp;
  S.history.pop();                              // undo this move's session stats
  S.repMoves = Math.max(0, S.repMoves - 1);
  S.outcome = { status: 'ongoing' };
  S.hintUsed = false;
  S.evalCp = null; S.evalMate = null;
  recomputeStats();
  clearArrows(); clearBanner();
  renderVerdictPlaceholder();
  await refreshView(S.moves.length);
  setEvalBar();
  renderStats(); renderRank(); renderOpening();
  S.phase = 'user'; updateButtons(); renderStatus();
}

// --- session summary ---

function showSummary() {
  const acc = S.history.length
    ? Math.round(S.history.reduce((a, j) => a + 100 * Math.exp(-j.cpLoss / 140), 0) / S.history.length)
    : 0;
  const match = S.graded ? Math.round(100 * S.matched / S.graded) : 0;
  const order = ['brilliant', 'best', 'excellent', 'great', 'good', 'inaccuracy', 'mistake', 'blunder'];
  const maxC = Math.max(1, ...order.map(k => S.dist[k]));
  const dist = order.filter(k => S.dist[k] > 0).map(k => {
    const w = Math.round(160 * S.dist[k] / maxC);
    return `<div class="bar-row"><span class="lbl">${GRADE_LABEL[k].toLowerCase()}</span>` +
      `<span class="bar" style="width:${w}px;background:${GRADE_COLOR[k]}"></span>` +
      `<span class="cnt">${S.dist[k]}</span></div>`;
  }).join('');

  $('summary-body').innerHTML =
    `<div class="big-rank">${RANKS[S.rankIdx].name}</div>` +
    `<div class="muted" style="margin-bottom:10px">${S.xp} XP earned this session</div>` +
    (S.mode === 'reps' ? row('Reps completed', S.repIndex) : '') +
    row('Moves graded', S.graded) +
    row('Matched the engine', `${match}%`) +
    row('Accuracy', `${acc}%`) +
    row('Best streak', S.bestStreak) +
    (S.opening ? row('Opening reached', S.opening) : '') +
    (dist ? `<div class="dist">${dist}</div>` : '');
  $('dlg-summary').showModal();

  function row(k, v) { return `<div class="srow"><span class="k">${k}</span><span class="v">${v}</span></div>`; }
}

// --- confetti ---

const confettiCanvas = $('confetti');
const cctx = confettiCanvas.getContext('2d');
let confettiParticles = [];
let confettiRunning = false;

function resizeConfetti() {
  confettiCanvas.width = window.innerWidth;
  confettiCanvas.height = window.innerHeight;
}

function boardCenter() {
  const r = boardEl.getBoundingClientRect();
  return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
}

const CONFETTI_COLORS = ['#f1c33f', '#21d0c3', '#6ea8fe', '#6abf69', '#e8923a', '#e0544a', '#ffffff'];

function confettiBurst(kind) {
  const c = boardCenter();
  let n, cx, cy, power;
  if (kind === 'rankup') { n = 160; cx = window.innerWidth / 2; cy = window.innerHeight * 0.28; power = 13; }
  else if (kind === 'brilliant') { n = 130; cx = c.x; cy = c.y; power = 12; }
  else { n = 80; cx = c.x; cy = c.y - 40; power = 10; }
  for (let i = 0; i < n; i++) {
    const ang = Math.random() * Math.PI * 2;
    const sp = power * (0.4 + Math.random() * 0.9);
    confettiParticles.push({
      x: cx, y: cy,
      vx: Math.cos(ang) * sp,
      vy: Math.sin(ang) * sp - 4,
      size: 4 + Math.random() * 6,
      color: CONFETTI_COLORS[(Math.random() * CONFETTI_COLORS.length) | 0],
      rot: Math.random() * Math.PI,
      vrot: (Math.random() - 0.5) * 0.4,
      life: 1,
    });
  }
  if (!confettiRunning) { confettiRunning = true; requestAnimationFrame(stepConfetti); }
}

function stepConfetti() {
  cctx.clearRect(0, 0, confettiCanvas.width, confettiCanvas.height);
  confettiParticles = confettiParticles.filter(p => p.life > 0 && p.y < confettiCanvas.height + 40);
  for (const p of confettiParticles) {
    p.vy += 0.28;             // gravity
    p.vx *= 0.99;
    p.x += p.vx; p.y += p.vy;
    p.rot += p.vrot;
    p.life -= 0.008;
    cctx.save();
    cctx.translate(p.x, p.y);
    cctx.rotate(p.rot);
    cctx.globalAlpha = Math.max(0, Math.min(1, p.life));
    cctx.fillStyle = p.color;
    cctx.fillRect(-p.size / 2, -p.size / 2, p.size, p.size * 0.6);
    cctx.restore();
  }
  if (confettiParticles.length) requestAnimationFrame(stepConfetti);
  else { confettiRunning = false; cctx.clearRect(0, 0, confettiCanvas.width, confettiCanvas.height); }
}

// --- audio (WebAudio, generated tones, no assets) ---

let audioCtx = null;
function ac() {
  if (!audioCtx) { try { audioCtx = new (window.AudioContext || window.webkitAudioContext)(); } catch { } }
  return audioCtx;
}
function tone(freq, start, dur, gain = 0.12, type = 'sine') {
  const ctx = ac(); if (!ctx) return;
  const t0 = ctx.currentTime + start;
  const osc = ctx.createOscillator();
  const g = ctx.createGain();
  osc.type = type; osc.frequency.value = freq;
  g.gain.setValueAtTime(0.0001, t0);
  g.gain.exponentialRampToValueAtTime(gain, t0 + 0.012);
  g.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
  osc.connect(g).connect(ctx.destination);
  osc.start(t0); osc.stop(t0 + dur + 0.02);
}
function chord(freqs, dur, gain) { freqs.forEach(f => tone(f, 0, dur, gain)); }
function arp(freqs, step, dur, gain, type) { freqs.forEach((f, i) => tone(f, i * step, dur, gain, type)); }

function playGradeSound(grade) {
  switch (grade) {
    case 'brilliant': arp([523, 659, 784, 1047, 1319], 0.07, 0.5, 0.12, 'triangle'); break;
    case 'best': arp([523, 659, 784], 0.06, 0.42, 0.12, 'triangle'); break;
    case 'excellent': arp([523, 784], 0.05, 0.34, 0.1, 'triangle'); break;
    case 'great': tone(659, 0, 0.3, 0.1, 'triangle'); break;
    case 'good': tone(523, 0, 0.28, 0.08, 'sine'); break;
    case 'inaccuracy': tone(392, 0, 0.26, 0.08, 'sine'); break;
    case 'mistake': tone(262, 0, 0.3, 0.1, 'sawtooth'); break;
    case 'blunder': tone(196, 0, 0.18, 0.12, 'sawtooth'); tone(155, 0.16, 0.34, 0.12, 'sawtooth'); break;
  }
}
function playRankUp() { arp([523, 659, 784, 1047, 1319, 1568], 0.08, 0.6, 0.13, 'triangle'); }

// --- toolbar / dialogs / keys ---

$('btn-new').onclick = () => {
  if (R.mode) leaveReviewMode();
  $('dlg-new').showModal();
};
$('btn-review').onclick = () => { if (!R.mode) enterReviewMode(); };
$('new-close').onclick = () => $('dlg-new').close();
$('new-start').onclick = () => {
  const color = $('opt-color').value;
  const depth = +$('opt-strength').value;
  const mode = $('opt-mode').value;
  $('dlg-new').close();
  newSession(color, depth, mode);
};

$('btn-flip').onclick = () => {
  S.flipped = !S.flipped;
  renderBoard();
  clearArrows();
  if (R.mode && R.phase === 'answering' && R.staged) drawArrow(R.staged.uci, REVIEW_CHOICE_COLOR);
  if (R.mode && (R.phase === 'grading' || R.phase === 'saving') && R.answer?.reference?.uci) {
    drawArrow(R.answer.reference.uci, GRADE_COLOR.best);
  }
};
$('btn-hint').onclick = hint;
$('btn-retry').onclick = retry;
$('btn-finish').onclick = () => { if (S.graded) showSummary(); };
$('btn-sound').onclick = () => {
  S.sound = !S.sound;
  const b = $('btn-sound');
  b.classList.toggle('on', S.sound);
  b.textContent = S.sound ? '🔊' : '🔇';
  if (S.sound) { ac(); tone(659, 0, 0.18, 0.1, 'triangle'); }
};

$('summary-close').onclick = () => $('dlg-summary').close();
$('summary-again').onclick = () => { $('dlg-summary').close(); $('dlg-new').showModal(); };

$('nav-start').onclick = () => navTo(0);
$('nav-back').onclick = () => navTo(S.viewAt - 1);
$('nav-fwd').onclick = () => navTo(S.viewAt + 1);
$('nav-live').onclick = () => navTo(S.moves.length);

$('review-reason').addEventListener('input', updateReviewRevealButton);
$('review-move-entry').addEventListener('input', () => {
  if (R.phase === 'answering' && R.staged && !stagedMoveMatchesEntry()) {
    clearStagedReviewMove();
  }
});
$('review-move-entry').addEventListener('change', stageTypedReviewMove);
$('review-move-entry').addEventListener('keydown', e => {
  if (e.key === 'Enter') {
    e.preventDefault();
    stageTypedReviewMove();
  }
});
$('review-reason').addEventListener('keydown', e => {
  if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !$('review-reveal').disabled) {
    e.preventDefault();
    revealReview(false);
  }
});
$('review-reveal').onclick = () => revealReview(false);
$('review-give-up').onclick = () => revealReview(true);
$('review-refresh').onclick = loadNextReview;
for (const button of document.querySelectorAll('[data-review-grade]')) {
  button.onclick = () => gradeReview(button.dataset.reviewGrade);
}

document.addEventListener('keydown', e => {
  const t = document.activeElement;
  if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT')) return;
  if ($('dlg-new').open || $('dlg-summary').open) return;
  if (R.mode) {
    if (e.key === 'f') $('btn-flip').click();
    else if (R.phase === 'grading' && e.key === '1') gradeReview('miss');
    else if (R.phase === 'grading' && e.key === '2') gradeReview('partial');
    else if (R.phase === 'grading' && e.key === '3') gradeReview('pass');
    return;
  }
  switch (e.key) {
    case 'ArrowLeft': navTo(S.viewAt - 1); e.preventDefault(); break;
    case 'ArrowRight': navTo(S.viewAt + 1); e.preventDefault(); break;
    case 'Home': navTo(0); e.preventDefault(); break;
    case 'End': navTo(S.moves.length); e.preventDefault(); break;
    case 'f': $('btn-flip').click(); break;
    case 'h': if (!$('btn-hint').disabled) hint(); break;
    case 'r': if (!$('btn-retry').disabled) retry(); break;
  }
});

function fitBoard() {
  const narrow = window.innerWidth <= 780;
  const widthAllowance = narrow
    ? window.innerWidth - (R.mode ? 24 : 58)
    : window.innerWidth - 460;
  const heightAllowance = window.innerHeight - (narrow ? 230 : 150);
  const size = Math.max(240, Math.min(640, heightAllowance, widthAllowance));
  document.documentElement.style.setProperty('--board-size', size + 'px');
}
window.addEventListener('resize', () => { fitBoard(); resizeConfetti(); });

// --- boot ---
fitBoard();
resizeConfetti();
renderBoard();
renderStats();
renderRank();
renderStatus();
updateButtons();
initializeReview();
