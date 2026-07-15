# Performance: alternatives compared, optimum chosen

This documents how the board-operation design was chosen — every claim below is
backed by `cargo bench` (criterion, release, `lto=fat`, `codegen-units=1`) on the
development machine (Apple Silicon). Reproduce with `cargo bench`; the fast
iteration harness is `cargo run --release --example nps`.

> **Two optimization rounds.** Round 1 (below) took the move generator from a
> classical-rays + make/unmake-filter baseline to magic bitboards + a pin-aware
> legal generator (~73 → ~460 Mnps). Round 2 (["Locality & micro-arch pass"](#round-2-locality--micro-architecture-pass))
> pushed a further **+13%** with locality and micro-architectural work — and,
> just as importantly, *rejected* three plausible ideas that measured as noise.

## Headline

| Metric | Baseline (classical rays + make/unmake filter) | Round 1 (magic + pin-aware legal gen) | Final (after round 2) | Speedup |
|--------|---------------------------|---------------------------|---------------------------|---------|
| perft startpos d5 | 66.5 ms (73 Mnps) | 10.6 ms (457 Mnps) | **9.2 ms (530 Mnps)** | **7.2×** |
| perft kiwipete d4 | 53.2 ms (73 Mnps) | 8.1 ms (502 Mnps) | **7.0 ms (580 Mnps)** | **7.5×** |
| perft midgame d4 | 50.0 ms (78 Mnps) | 6.1 ms (643 Mnps) | **5.2 ms (748 Mnps)** | **9.6×** |
| legal movegen (kiwipete) | 615 ns | 84 ns | **68 ns** | 9.0× |

The two changes that mattered most, in order of impact:

1. **Pin-aware legal generation** (the dominant win), and
2. **Magic bitboards** for sliding attacks.

A research pass (4 parallel deep-dives + synthesis) compared the full design
space first; the empirical numbers below confirmed the picks.

## 1. Sliding-piece attacks

Benchmark: XOR-accumulate the attack set for all 64 squares over a realistic
midgame occupancy (so blockers are typical, not empty/full).

| Method | bishop | rook | queen | Notes |
|--------|--------|------|-------|-------|
| Classical ray scan | 1.29 ns | 1.24 ns | 2.18 ns | baseline; 4 ray lookups + bitscans per piece |
| **Magic (flattened)** | **0.76 ns** | **0.69 ns** | ~1.7 ns | single `(occ&mask)*magic>>shift` table read |

**Chosen: magic bitboards.** ~1.7–1.9× faster on the kernel, portable (one 64-bit
multiply; identical on x86-64 and ARM). The magics are *found at startup* by a
deterministic fixed-seed search — no hand-embedded constants to get wrong — and
the tables (rook ≈ 800 KiB, bishop ≈ 41 KiB) are global, so per-board size is
unaffected. The classical scan bootstraps and cross-checks the magic tables and
is retained behind `*_classical` for benchmarking.

A key refinement: the first magic implementation chased `Vec<Magic> → Vec<u64>`
behind a `LazyLock`. Flattening to inline `[Magic; 64]` + a single `Box<[u64]>`
and using unchecked indexing cut another **20–27%** off each lookup (rook 60 → 44 ns
over 64 squares).

### Considered and rejected as the default

- **BMI2 PEXT** — as fast as magic on Intel Haswell+/AMD Zen 3+, but **absent on
  Apple Silicon** (the target) and *microcoded/catastrophic* on AMD pre-Zen 3.
  Correct only as an opt-in `cfg(target_feature="bmi2")` path, never the default.
- **Hyperbola quintessence / Kogge-Stone** — tiny tables, fully portable, but
  slower than magic per call; worth it only if memory is the hard constraint (it
  isn't — density lives in the 34-byte `Packed` form).
- **Fancy/black-magic** — ~140 KiB smaller tables, same hot path; the packing
  logic adds correctness risk for a marginal memory win.

## 2. Legal move generation

Sliding attacks were only ~5% of perft — the real cost was the legality filter:
the original generator produced pseudo-legal moves, then **cloned the board and
did make / king-safety / unmake per move** (pseudo gen was ~5× faster than legal,
all the difference being the filter).

| Approach | legal movegen (startpos) | (kiwipete) |
|----------|--------------------------|------------|
| pseudo-legal only (no legality) | 51 ns | 80 ns |
| clone + make/unmake filter | 254 ns | 615 ns |
| **pin-aware legal generator** | **59 ns** | **84 ns** |

**Chosen: a pin-aware legal generator** — nearly free over pseudo-legal, ~5–7×
faster than the filter. It computes:

- a **king-danger map**: enemy attacks with our king removed from the occupancy
  (so sliders see *through* the king), which legal king moves must avoid;
- a **check mask**: in single check, non-king moves must block or capture the
  checker; in double check, only the king moves;
- **pin rays**: a pinned piece is restricted to the king–pinner line;
- **exact en-passant legality**: the horizontal discovered-check case (both
  pawns leaving the rank simultaneously) is invisible to pin/check masks, so each
  ep capture is verified against the post-capture occupancy directly.

This unlocked perft **bulk counting** (at depth 1 the legal move count *is* the
leaf count — no make/unmake), worth another large constant factor.

Correctness is guaranteed by a **differential test**: the fast generator must
emit identical move *sets* to the obvious clone+filter reference at every node of
trees rooted at pin/check/ep/castle/promotion positions, plus the full perft
suite.

## 3. Working representation

**Kept as-is**: 6 piece-type bitboards + 2 color bitboards + a `[u8; 64]` mailbox
= 144 B. The mailbox earns its keep on the make/unmake critical path (`piece_at`,
capture target, moved-piece type are a single load + nibble decode, beating a
1-of-6 bitboard scan), and move generation never touches it so it stays cold
during attack work. 12-bitboard, 4-bitboard, and 0x88 layouts all regress the hot
path; density belongs in the separate 34-byte `Packed` form, not here.

## Micro-optimizations applied

- `MoveList` backed by `MaybeUninit<[Move; 256]>` — no 512-byte zero-init per
  node (a `MoveList` is allocated at every perft node).
- Unchecked indexing in the magic lookup and move-list push (bounds are invariants).
- `#[inline]` on the bitboard/attack/make-unmake helpers; `occupied()` hoisted in
  hot loops.

## Reproducing

```sh
cargo bench -- attacks        # magic vs classical, per slider
cargo bench -- 'movegen/legal'
cargo bench -- perft
```

## Round 2: locality & micro-architecture pass

Measured with the `nps` example (aggregate over startpos d6, kiwipete d5,
position3 d6, midgame d5; best of 4 runs). A code-level investigation (4
parallel agents reading the committed source) proposed and ranked these; each
was implemented and measured individually, gated by the 6838-position perft
suite and the differential generator test.

| Step | Change | Aggregate Mnps | Δ |
|------|--------|---------------:|---|
| — | round-1 result (session baseline) | 578.6 | — |
| 1 | **Set-wise pawn generation** — unpinned pawns move via bitboard shifts; pinned pawns and en passant still exact per-pawn | 609.1 | +5.3% |
| 2 | **Compile-time magic tables** — embed the searched magics; build masks/shifts/offsets and the ~108k-entry attack tables as `const`/`static` in `.rodata`. No `LazyLock` atomic, no heap `Box`, no runtime search on the lookup path | 619.2 | +1.7% |
| 3 | **Fused checker + pin detection** — one king-ray sniper pass produces both `checkers` and `pinned`, replacing three separate king-slider computations | 631.9 | +2.0% |
| 4 | **Bounds-check elision** — `Square/File/Rank::index()` mask with `&63`/`&7` so the compiler proves every square-keyed table lookup is in-bounds | 641.5 | +1.5% |
| 5 | **Slider/make-move micro-opts** — visit each slider type once (queens via `queen_attacks`); skip the castling-rights table loads once no rights remain | 653.5 | +1.9% |
| 6 | **Single-pass hash** in FEN/unpack (`finalize_hash`) — no perft effect; speeds `from_fen` and the batch `Packed::unpack` path | 653.5 | — |

**Round-2 total: 578.6 → 653.5 Mnps (+12.9%).** Correctness unchanged: full
perft suite to depth 5, the 6 CPW landmarks to depth 6, and the differential
generator test all still pass exactly.

### Memory layout / batch processing

- The working `Board` stays 144 B: `pieces[6]` + `colors[2]` bitboards are 64 B
  (one cache line, the move-gen working set) and the `[u8; 64]` mailbox — only
  touched by make/unmake — is the next line. The investigation confirmed this is
  already cache-optimal for the perft hot path (move generation never reads the
  mailbox); density lives in the separate 34-B `Packed` form.
- Batch path (`batch` bench): a dense `Vec<Packed>` streams at **~23M boards/s**
  unpacked and **~12M boards/s** unpacked + legal-move-generated. 34 B/position
  keeps a million positions in ~34 MB and cache-friendly under sequential scan.

### Measured and rejected (no free lunch)

These were predicted to help but measured as noise, so they were **not** adopted
— recorded here so the negative results aren't silently lost:

| Idea | Predicted | Measured | Verdict |
|------|-----------|----------|---------|
| `RUSTFLAGS=-C target-cpu=native` | some | 578.6 → 571.6 (−1%, noise) | No PEXT on ARM; the magic multiply is already native. Rejected. |
| `#[repr(align(64))]` on `Board` | better line packing | 619.2 → 613.8 (noise) | The hot bitboards already pack into one line; alignment only bloats `Board` 144 → 192 B. Reverted. |
| Profile-guided optimization (PGO) | 5–12% | 653.5 → 657.3 (+0.6%) | `lto=fat` + already-elided branches + memory-latency-bound table reads leave little for PGO. Not worth the build complexity. |
| Cached incremental `occupied` bitboard | — | (investigation: net ~0/negative) | `occupied()` is computed once per `generate_legal` and threaded; caching adds make/unmake work. Not pursued. |
