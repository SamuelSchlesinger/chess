// chess-web frontend. The server is stateless: every request carries
// (startFen, moves, at); this file owns the game state and the rendering.
'use strict';

const START_FEN = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1';
const GLYPH = { p: '♟', n: '♞', b: '♝', r: '♜', q: '♛', k: '♚' };
const FILES = 'abcdefgh';

const S = {
  startFen: 'startpos',
  moves: [],        // UCI strings, the full line
  sans: [],         // SAN of each move (from server)
  at: 0,            // view index: number of moves applied, 0..moves.length
  fen: START_FEN,   // position at `at`
  side: 'w',
  check: false,
  lastMove: null,
  outcome: { status: 'ongoing' },
  legal: [],        // [{uci, san}] at `at`
  flipped: false,
  selected: null,
  engineOn: true,
  engineId: null,   // which engine analyzes (from /api/engines)
  multipv: 3,
  es: null,         // analysis EventSource
  lines: [],        // engine lines by multipv index
  anaEs: null,      // game-analysis EventSource
  evals: null,      // per-ply eval series (game analysis)
  judgments: null,  // per-move {g, cls} or null
  tok: 0,
  lastGood: null,
  // play-against-engine mode
  playMode: false,
  playerColor: 'w',   // 'w' or 'b': which side the human plays
  playMovetime: 1000, // ms per engine move
  playEngineId: null, // engine used for opponent moves
  engineThinking: false,
};

const $ = id => document.getElementById(id);
const boardEl = $('board');
const qs = o => Object.entries(o)
  .filter(([, v]) => v !== '' && v != null)
  .map(([k, v]) => k + '=' + encodeURIComponent(v))
  .join('&');

// --- server state ---

async function refresh() {
  const tok = ++S.tok;
  let d;
  try {
    const r = await fetch('/api/state?' + qs({ fen: S.startFen, moves: S.moves.join(' '), at: S.at }));
    d = await r.json();
  } catch {
    $('status').textContent = 'server unreachable';
    return;
  }
  if (tok !== S.tok) return;
  if (d.error) {
    if (S.lastGood) Object.assign(S, S.lastGood);
    $('status').textContent = d.error;
    return;
  }
  Object.assign(S, {
    startFen: d.startFen, moves: d.moves, sans: d.sans, at: d.at, fen: d.fen,
    side: d.side, check: d.check, lastMove: d.lastMove, outcome: d.outcome, legal: d.legal,
  });
  S.lastGood = { startFen: d.startFen, moves: d.moves, at: d.at };
  S.selected = null;
  renderAll();
  if (S.playMode && S.outcome.status === 'ongoing' && S.side !== S.playerColor && S.at === S.moves.length) {
    engineMove();
  } else {
    restartEngine();
  }
}

function setAt(i) {
  const j = Math.max(0, Math.min(S.moves.length, i));
  if (j !== S.at) { S.at = j; refresh(); }
}

function play(uci) {
  if (S.at < S.moves.length && S.moves[S.at] === uci) {
    S.at++;                       // following the existing line
  } else {
    S.moves = S.moves.slice(0, S.at).concat([uci]);  // new continuation
    S.at = S.moves.length;
    invalidateGameAnalysis();
  }
  refresh();
}

function loadPosition(fen, moves, at) {
  S.startFen = fen;
  S.moves = moves;
  S.at = at;
  invalidateGameAnalysis();
  refresh();
}

function invalidateGameAnalysis() {
  if (S.anaEs) { S.anaEs.close(); S.anaEs = null; }
  S.evals = null;
  S.judgments = null;
  $('graph').hidden = true;
  $('ana-progress').hidden = true;
  $('ana-summary').textContent = '';
  $('btn-analyze-game').textContent = 'Analyze game';
}

// --- board ---

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

function squareName(row, col) {  // row/col are display coordinates
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
        c.className = 'coord rank';
        c.textContent = sq[1];
        el.appendChild(c);
      }
      if (row === 7) {
        const c = document.createElement('span');
        c.className = 'coord file';
        c.textContent = sq[0];
        el.appendChild(c);
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

function canMoveFrom(sq) {
  if (S.playMode && (S.engineThinking || S.side !== S.playerColor)) return false;
  return S.legal.some(m => m.uci.slice(0, 2) === sq);
}

function tryMove(from, to) {
  const cands = S.legal.filter(m => m.uci.slice(0, 2) === from && m.uci.slice(2, 4) === to);
  if (!cands.length) return false;
  if (cands.length === 1 && cands[0].uci.length === 4) {
    play(cands[0].uci);
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
    b.onclick = e => { e.stopPropagation(); overlay.remove(); play(from + to + p); };
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

  if (S.selected && S.selected !== sq && tryMove(S.selected, sq)) {
    S.selected = null;
    return;
  }
  if (canMoveFrom(sq)) {
    S.selected = sq;
    renderBoard();
    const pieceEl = boardEl.querySelector(`[data-sq="${sq}"] .piece`);
    if (pieceEl) {
      e.preventDefault();
      boardEl.setPointerCapture(e.pointerId);
      drag = { from: sq, pieceEl, ghost: null };
    }
  } else {
    S.selected = null;
    renderBoard();
  }
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
    if (to && to !== from && tryMove(from, to)) {
      S.selected = null;
      return;
    }
    renderBoard();
  }
  // No drag: leave the piece selected for click-click.
});

boardEl.addEventListener('pointercancel', () => {
  if (drag && drag.ghost) {
    drag.ghost.remove();
    drag.pieceEl.classList.remove('dragging');
  }
  drag = null;
});

// --- engine panel ---

function restartEngine() {
  if (S.es) { S.es.close(); S.es = null; }
  S.lines = [];
  renderLines();
  $('engine-stats').textContent = '';
  if (S.outcome.status !== 'ongoing') {
    setEvalBarOutcome(S.outcome);
    return;
  }
  if (!S.engineOn) {
    setEvalBar(null);
    return;
  }
  const url = '/api/analyze?' + qs({
    fen: S.startFen, moves: S.moves.join(' '), at: S.at, multipv: S.multipv,
    engine: S.engineId,
  });
  const es = new EventSource(url);
  S.es = es;
  es.addEventListener('info', ev => {
    const d = JSON.parse(ev.data);
    S.lines[d.multipv - 1] = d;
    renderLines();
    if (d.multipv === 1) {
      setEvalBar(d);
      $('engine-stats').textContent =
        `depth ${d.depth}/${d.seldepth} · ${fmtNodes(d.nodes)} · ${Math.round(d.nps / 1000)} knps`;
    }
  });
  es.addEventListener('done', () => es.close());
  es.onerror = () => es.close();
}

function fmtNodes(n) {
  if (n >= 1e9) return (n / 1e9).toFixed(1) + ' Gn';
  if (n >= 1e6) return (n / 1e6).toFixed(1) + ' Mn';
  return Math.round(n / 1e3) + ' kn';
}

function fmtScore(d) {
  if (d.mate != null) return (d.mate < 0 ? '#-' : '#') + Math.abs(d.mate);
  const v = d.cp / 100;
  return (v > 0 ? '+' : '') + v.toFixed(2);
}

function isNegScore(d) {
  return d.mate != null ? d.mate < 0 : d.cp < 0;
}

// "1. e4 e5 2. Nf3" numbering for a SAN sequence starting at `fen`.
function numberedSans(fen, sans, decorate) {
  const parts = fen.split(' ');
  let side = parts[1] || 'w';
  let num = parseInt(parts[5], 10) || 1;
  const out = [];
  sans.forEach((san, i) => {
    if (side === 'w') out.push(num + '.');
    else if (i === 0) out.push(num + '...');
    out.push(decorate ? decorate(san, i) : san);
    if (side === 'b') num++;
    side = side === 'w' ? 'b' : 'w';
  });
  return out;
}

function renderLines() {
  const box = $('lines');
  box.textContent = '';
  if (S.playMode && S.engineThinking) {
    box.innerHTML = '<div class="placeholder">engine thinking…</div>';
    return;
  }
  if (!S.engineOn) {
    box.innerHTML = '<div class="placeholder">engine off</div>';
    return;
  }
  if (S.outcome.status !== 'ongoing') {
    box.innerHTML = '<div class="placeholder">game over</div>';
    return;
  }
  for (let k = 0; k < S.multipv; k++) {
    const d = S.lines[k];
    const row = document.createElement('div');
    row.className = 'line';
    if (!d) {
      row.innerHTML = '<span class="score">…</span><span class="pv"></span>';
      box.appendChild(row);
      continue;
    }
    const score = document.createElement('span');
    score.className = 'score' + (isNegScore(d) ? ' neg' : '');
    score.textContent = fmtScore(d);
    const depth = document.createElement('span');
    depth.className = 'depth';
    depth.textContent = 'd' + d.depth;
    const pv = document.createElement('span');
    pv.className = 'pv';
    pv.textContent = numberedSans(S.fen, d.sanPv).join(' ');
    pv.title = 'Play ' + (d.sanPv[0] || '');
    pv.onclick = () => { if (d.pv[0]) play(d.pv[0]); };
    row.append(score, depth, pv);
    box.appendChild(row);
  }
}

function setEvalBar(d) {
  const fill = $('evalbar-white');
  const num = $('evalbar-num');
  if (!d) {
    fill.style.height = '50%';
    num.textContent = '–';
    num.classList.remove('black');
    return;
  }
  let pct;
  if (d.mate != null) pct = d.mate > 0 ? 100 : 0;
  else {
    pct = 50 + 50 * (2 / (1 + Math.exp(-0.00368208 * d.cp)) - 1);
    pct = Math.max(2, Math.min(98, pct));
  }
  fill.style.height = pct + '%';
  num.textContent = d.mate != null ? '#' + Math.abs(d.mate) : Math.abs(d.cp / 100).toFixed(1);
  num.classList.toggle('black', pct < 12);
}

function setEvalBarOutcome(o) {
  const fill = $('evalbar-white');
  const num = $('evalbar-num');
  const pct = o.status === 'checkmate' ? (o.winner === 'w' ? 100 : 0) : 50;
  fill.style.height = pct + '%';
  num.textContent = o.status === 'checkmate' ? (o.winner === 'w' ? '1-0' : '0-1') : '½';
  num.classList.toggle('black', pct < 12);
}

// --- move list / status ---

function renderMoves() {
  const box = $('movelist');
  box.textContent = '';
  if (!S.sans.length) {
    box.innerHTML = '<span class="empty">No moves yet — drag a piece or paste a PGN.</span>';
    return;
  }
  const tokens = numberedSans(S.startFen, S.sans, (san, i) => {
    const span = document.createElement('span');
    span.className = 'mv' + (i + 1 === S.at ? ' cur' : '');
    span.textContent = san;
    const j = S.judgments && S.judgments[i];
    if (j) {
      const g = document.createElement('span');
      g.className = 'glyph ' + j.cls;
      g.textContent = j.g;
      span.appendChild(g);
      span.title = { inacc: 'Inaccuracy', mistake: 'Mistake', blunder: 'Blunder' }[j.cls];
    }
    span.onclick = () => setAt(i + 1);
    return span;
  });
  for (const t of tokens) {
    if (typeof t === 'string') {
      const n = document.createElement('span');
      n.className = 'num';
      n.textContent = t;
      box.appendChild(n);
    } else {
      box.appendChild(t);
    }
  }
  const cur = box.querySelector('.cur');
  if (cur) cur.scrollIntoView({ block: 'nearest' });
}

function renderStatus() {
  const o = S.outcome;
  let txt;
  if (o.status === 'checkmate') {
    txt = `Checkmate — ${o.winner === 'w' ? 'White' : 'Black'} wins`;
    if (S.playMode) txt += o.winner === S.playerColor ? ' · You win!' : ' · Engine wins';
  } else if (o.status === 'draw') {
    txt = `Draw — ${o.reason}`;
  } else if (S.playMode) {
    if (S.engineThinking) {
      txt = 'Engine thinking…';
    } else if (S.side === S.playerColor) {
      txt = `Your turn · ${S.side === 'w' ? 'White' : 'Black'}${S.check ? ' · check' : ''}`;
    } else {
      txt = 'Engine to move';
    }
    if (S.at < S.moves.length) txt += ` · viewing ply ${S.at}/${S.moves.length}`;
  } else {
    txt = `${S.side === 'w' ? 'White' : 'Black'} to move${S.check ? ' · check' : ''}`;
    if (S.at < S.moves.length) txt += ` · viewing ply ${S.at}/${S.moves.length}`;
  }
  $('status').textContent = txt;
}

function renderAll() {
  renderBoard();
  renderMoves();
  renderStatus();
  drawGraph();
}

// --- game analysis (eval series + graph + judgments) ---

$('btn-analyze-game').onclick = () => {
  if (S.anaEs) {            // acts as cancel
    invalidateGameAnalysis();
    return;
  }
  if (!S.moves.length) return;
  S.evals = new Array(S.moves.length + 1).fill(null);
  S.judgments = null;
  const prog = $('ana-progress');
  prog.hidden = false;
  prog.value = 0;
  $('btn-analyze-game').textContent = 'Cancel';
  $('ana-summary').textContent = '';

  const es = new EventSource('/api/evalseries?' + qs({
    fen: S.startFen, moves: S.moves.join(' '), movetime: 150,
    engine: S.engineId,
  }));
  S.anaEs = es;
  es.addEventListener('eval', ev => {
    const d = JSON.parse(ev.data);
    if (S.evals && d.idx < S.evals.length) {
      S.evals[d.idx] = d;
      prog.value = (d.idx + 1) / S.evals.length;
    }
  });
  es.addEventListener('done', () => {
    es.close();
    S.anaEs = null;
    prog.hidden = true;
    $('btn-analyze-game').textContent = 'Analyze game';
    finishGameAnalysis();
  });
  es.onerror = () => {
    es.close();
    S.anaEs = null;
    prog.hidden = true;
    $('btn-analyze-game').textContent = 'Analyze game';
  };
};

function cappedEval(e) {
  if (!e) return null;
  if (e.terminal) return e.terminal.status === 'checkmate' ? (e.terminal.winner === 'w' ? 1000 : -1000) : 0;
  if (e.mate != null) return e.mate > 0 ? 1000 : -1000;
  return Math.max(-1000, Math.min(1000, e.cp));
}

function finishGameAnalysis() {
  if (!S.evals) return;
  S.judgments = S.moves.map((_, i) => {
    const before = cappedEval(S.evals[i]);
    const after = cappedEval(S.evals[i + 1]);
    if (before == null || after == null) return null;
    const loss = S.evals[i].side === 'w' ? before - after : after - before;
    if (loss >= 300) return { g: '??', cls: 'blunder' };
    if (loss >= 150) return { g: '?', cls: 'mistake' };
    if (loss >= 75) return { g: '?!', cls: 'inacc' };
    return null;
  });
  const count = (side, cls) =>
    S.judgments.filter((j, i) => j && j.cls === cls && S.evals[i].side === side).length;
  const sum = side =>
    `${count(side, 'inacc')} inaccuracies, ${count(side, 'mistake')} mistakes, ${count(side, 'blunder')} blunders`;
  $('ana-summary').textContent = `White: ${sum('w')} · Black: ${sum('b')}`;
  $('graph').hidden = false;
  renderMoves();
  drawGraph();
}

function drawGraph() {
  const canvas = $('graph');
  if (!S.evals || canvas.hidden) return;
  const ctx = canvas.getContext('2d');
  const w = canvas.width, h = canvas.height, pad = 6;
  const n = S.evals.length;
  const x = i => pad + i * (w - 2 * pad) / Math.max(1, n - 1);
  const y = v => h / 2 - (v / 1000) * (h / 2 - pad);

  ctx.clearRect(0, 0, w, h);
  // midline
  ctx.strokeStyle = '#3a3d43';
  ctx.beginPath();
  ctx.moveTo(0, h / 2);
  ctx.lineTo(w, h / 2);
  ctx.stroke();
  // eval area (White's advantage above the midline)
  ctx.beginPath();
  ctx.moveTo(x(0), h);
  for (let i = 0; i < n; i++) {
    const v = cappedEval(S.evals[i]);
    ctx.lineTo(x(i), v == null ? h / 2 : y(v));
  }
  ctx.lineTo(x(n - 1), h);
  ctx.closePath();
  ctx.fillStyle = '#d9d9d9';
  ctx.fill();
  // judgment markers
  if (S.judgments) {
    const colors = { inacc: '#d9b84a', mistake: '#e8923a', blunder: '#e06c5f' };
    S.judgments.forEach((j, i) => {
      if (!j) return;
      const v = cappedEval(S.evals[i + 1]);
      ctx.fillStyle = colors[j.cls];
      ctx.beginPath();
      ctx.arc(x(i + 1), v == null ? h / 2 : y(v), 4, 0, 2 * Math.PI);
      ctx.fill();
    });
  }
  // current position marker
  ctx.strokeStyle = '#6ea8fe';
  ctx.beginPath();
  ctx.moveTo(x(S.at), 0);
  ctx.lineTo(x(S.at), h);
  ctx.stroke();
}

$('graph').addEventListener('click', e => {
  if (!S.evals) return;
  const rect = e.target.getBoundingClientRect();
  const fx = (e.clientX - rect.left) / rect.width;
  setAt(Math.round(fx * (S.evals.length - 1)));
});

// --- play-against-engine mode ---

async function engineMove() {
  if (S.engineThinking) return;
  S.engineThinking = true;
  if (S.es) { S.es.close(); S.es = null; }
  S.lines = [];
  renderLines();
  renderStatus();
  let d;
  try {
    const r = await fetch('/api/bestmove?' + qs({
      fen: S.startFen, moves: S.moves.join(' '), at: S.at,
      engine: S.playEngineId, movetime: S.playMovetime,
    }));
    d = await r.json();
  } catch {
    S.engineThinking = false;
    renderStatus();
    return;
  }
  S.engineThinking = false;
  if (d.error || !d.uci) { renderStatus(); return; }
  play(d.uci);
}

function enterPlayMode(color, movetime, engineId) {
  const pc = color === 'r' ? (Math.random() < 0.5 ? 'w' : 'b') : color;
  S.playMode = true;
  S.playerColor = pc;
  S.playMovetime = movetime;
  S.playEngineId = engineId;
  S.engineThinking = false;
  $('btn-play').textContent = 'Stop';
  $('btn-play').classList.add('on');
  loadPosition('startpos', [], 0);
}

function exitPlayMode() {
  S.playMode = false;
  S.engineThinking = false;
  $('btn-play').textContent = 'Play';
  $('btn-play').classList.remove('on');
  restartEngine();
  renderStatus();
}

// --- toolbar / dialogs / keys ---

$('btn-new').onclick = () => {
  if (S.playMode) exitPlayMode();
  loadPosition('startpos', [], 0);
};
$('btn-flip').onclick = () => { S.flipped = !S.flipped; renderBoard(); };

$('btn-play').onclick = () => {
  if (S.playMode) { exitPlayMode(); } else { $('dlg-play').showModal(); }
};
$('play-close').onclick = () => $('dlg-play').close();
$('play-start').onclick = () => {
  const color = $('play-color').value;
  const movetime = +$('play-movetime').value;
  const engineId = $('play-engine').value;
  $('dlg-play').close();
  enterPlayMode(color, movetime, engineId);
};

$('btn-engine').onclick = () => {
  S.engineOn = !S.engineOn;
  const b = $('btn-engine');
  b.textContent = S.engineOn ? 'On' : 'Off';
  b.classList.toggle('on', S.engineOn);
  restartEngine();
};

$('sel-multipv').onchange = e => {
  S.multipv = +e.target.value;
  restartEngine();
};

$('sel-engine').onchange = e => {
  S.engineId = e.target.value;
  try { localStorage.setItem('chessWebEngine', S.engineId); } catch {}
  restartEngine();
};

async function loadEngines() {
  let d;
  try {
    d = await (await fetch('/api/engines')).json();
  } catch {
    return; // selector stays empty; server default applies
  }
  const sel = $('sel-engine');
  const playSel = $('play-engine');
  sel.textContent = '';
  playSel.textContent = '';
  for (const e of d.engines) {
    const opt = document.createElement('option');
    opt.value = e.id;
    opt.textContent = e.label;
    sel.appendChild(opt);
    playSel.appendChild(opt.cloneNode(true));
  }
  let saved = null;
  try { saved = localStorage.getItem('chessWebEngine'); } catch {}
  S.engineId = d.engines.some(e => e.id === saved) ? saved : d.default;
  sel.value = S.engineId;
  S.playEngineId = S.engineId;
  playSel.value = S.playEngineId;
}

$('nav-start').onclick = () => setAt(0);
$('nav-back').onclick = () => setAt(S.at - 1);
$('nav-fwd').onclick = () => setAt(S.at + 1);
$('nav-end').onclick = () => setAt(S.moves.length);

$('btn-fen').onclick = () => {
  $('fen-input').value = S.fen;
  $('dlg-fen').showModal();
};
$('fen-close').onclick = () => $('dlg-fen').close();
$('fen-copy').onclick = () => navigator.clipboard.writeText(S.fen);
$('fen-load').onclick = () => {
  const v = $('fen-input').value.trim();
  if (v) {
    $('dlg-fen').close();
    loadPosition(v, [], 0);
  }
};

$('btn-pgn').onclick = () => {
  $('pgn-input').value = exportPgn();
  $('dlg-pgn').showModal();
};
$('pgn-close').onclick = () => $('dlg-pgn').close();
$('pgn-copy').onclick = () => navigator.clipboard.writeText(exportPgn());
$('pgn-load').onclick = async () => {
  const text = $('pgn-input').value.trim();
  if (!text) return;
  let d;
  try {
    const r = await fetch('/api/pgn', { method: 'POST', body: text });
    d = await r.json();
  } catch {
    return;
  }
  if (d.error) {
    $('status').textContent = 'PGN: ' + d.error;
    return;
  }
  $('dlg-pgn').close();
  loadPosition(d.fen, d.moves, 0);
};

function exportPgn() {
  // The result reflects the end of the line, regardless of the view index.
  // A final '#' SAN identifies checkmate; the mating side follows from the
  // start side and the move count.
  let result = '*';
  const o = S.outcome;
  if (S.at === S.moves.length && o.status === 'draw') {
    result = '1/2-1/2';
  } else if (S.sans.length && S.sans[S.sans.length - 1].endsWith('#')) {
    const startSide = (S.startFen === 'startpos' ? 'w' : S.startFen.split(' ')[1]) || 'w';
    const lastMover = (S.sans.length % 2 === 1) === (startSide === 'w') ? 'w' : 'b';
    result = lastMover === 'w' ? '1-0' : '0-1';
  }
  const date = new Date().toISOString().slice(0, 10).replaceAll('-', '.');
  let head = `[Event "chess-web analysis"]\n[Site "local"]\n[Date "${date}"]\n[Result "${result}"]\n`;
  const custom = S.startFen !== 'startpos' && S.startFen !== START_FEN;
  if (custom) head += `[SetUp "1"]\n[FEN "${S.startFen}"]\n`;
  const body = numberedSans(custom ? S.startFen : START_FEN, S.sans).join(' ');
  return head + '\n' + (body ? body + ' ' : '') + result + '\n';
}

document.addEventListener('keydown', e => {
  const t = document.activeElement;
  if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')) return;
  if ($('dlg-fen').open || $('dlg-pgn').open || $('dlg-play').open) return;
  switch (e.key) {
    case 'ArrowLeft': setAt(S.at - 1); e.preventDefault(); break;
    case 'ArrowRight': setAt(S.at + 1); e.preventDefault(); break;
    case 'Home': setAt(0); e.preventDefault(); break;
    case 'End': setAt(S.moves.length); e.preventDefault(); break;
    case 'f': $('btn-flip').click(); break;
  }
});

function fitBoard() {
  const size = Math.max(320, Math.min(640, window.innerHeight - 140, window.innerWidth - 560));
  document.documentElement.style.setProperty('--board-size', size + 'px');
}
window.addEventListener('resize', fitBoard);

fitBoard();
loadEngines().then(refresh);
