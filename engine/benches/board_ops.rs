//! Criterion benchmarks for the core board operations.
//!
//! Run all:        `cargo bench`
//! Run one group:  `cargo bench -- perft`
//!
//! The `perft` and `movegen` groups report throughput in nodes/moves per
//! second, which is the headline number for move-generation work. The
//! `attacks` group isolates the sliding-attack kernel — the operation most
//! affected by the algorithm choice we compare in the research task.

use chess::attacks;
use chess::bitboard::Bitboard;
use chess::types::Square;
use chess::{Board, Move};
use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
const MIDGAME: &str = "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10";

fn bench_perft(c: &mut Criterion) {
    // (name, fen, depth, expected node count for throughput).
    let cases = [
        ("startpos/d5", START, 5u32, 4_865_609u64),
        ("kiwipete/d4", KIWIPETE, 4, 4_085_603),
        ("midgame/d4", MIDGAME, 4, 3_894_594),
    ];
    let mut group = c.benchmark_group("perft");
    group.sample_size(10);
    for (name, fen, depth, nodes) in cases {
        group.throughput(Throughput::Elements(nodes));
        group.bench_function(name, |b| {
            b.iter_batched_ref(
                || Board::from_fen(fen).unwrap(),
                |board| board.perft(black_box(depth)),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_movegen(c: &mut Criterion) {
    let mut group = c.benchmark_group("movegen");
    for (name, fen) in [("startpos", START), ("kiwipete", KIWIPETE), ("midgame", MIDGAME)] {
        let board = Board::from_fen(fen).unwrap();
        let n = board.legal_moves().len() as u64;
        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::new("legal", name), &board, |b, board| {
            b.iter(|| black_box(board).legal_moves());
        });
        group.bench_with_input(BenchmarkId::new("pseudo", name), &board, |b, board| {
            b.iter(|| black_box(board).pseudo_legal_moves());
        });
    }
    group.finish();
}

fn bench_make_unmake(c: &mut Criterion) {
    let mut group = c.benchmark_group("make_unmake");
    for (name, fen) in [("startpos", START), ("kiwipete", KIWIPETE)] {
        let board = Board::from_fen(fen).unwrap();
        let moves: Vec<Move> = board.legal_moves().iter().copied().collect();
        group.throughput(Throughput::Elements(moves.len() as u64));
        group.bench_with_input(BenchmarkId::new("all_moves", name), &board, |b, board| {
            b.iter_batched_ref(
                || board.clone(),
                |bd| {
                    for &mv in &moves {
                        let u = bd.make_move(mv);
                        black_box(bd.hash());
                        bd.unmake_move(mv, u);
                    }
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_attacks(c: &mut Criterion) {
    // Sweep a fixed set of squares over a representative occupancy so the
    // measurement reflects the sliding-attack kernel under realistic blockers.
    // Each slider is benched both ways (magic vs classical ray scan) so the
    // algorithm comparison is empirical and reproducible.
    chess::magic::init();
    let occ = Board::from_fen(MIDGAME).unwrap().occupied();
    let squares: Vec<Square> = (0..64).map(Square).collect();

    let mut group = c.benchmark_group("attacks");
    group.throughput(Throughput::Elements(squares.len() as u64));

    macro_rules! sweep {
        ($name:expr, $f:expr) => {
            group.bench_function($name, |b| {
                b.iter(|| {
                    let mut acc = Bitboard::EMPTY;
                    for &sq in &squares {
                        acc ^= $f(sq, black_box(occ));
                    }
                    black_box(acc)
                });
            });
        };
    }

    sweep!("bishop/magic", attacks::bishop_attacks);
    sweep!("bishop/classical", attacks::bishop_attacks_classical);
    sweep!("rook/magic", attacks::rook_attacks);
    sweep!("rook/classical", attacks::rook_attacks_classical);
    sweep!("queen/magic", attacks::queen_attacks);
    sweep!("queen/classical", |sq, occ| attacks::bishop_attacks_classical(sq, occ)
        | attacks::rook_attacks_classical(sq, occ));

    group.bench_function("knight", |b| {
        b.iter(|| {
            let mut acc = Bitboard::EMPTY;
            for &sq in &squares {
                acc ^= attacks::knight_attacks(sq);
            }
            black_box(acc)
        });
    });
    group.finish();
}

fn bench_pack(c: &mut Criterion) {
    let board = Board::from_fen(KIWIPETE).unwrap();
    let packed = board.pack();
    let mut group = c.benchmark_group("packing");
    group.bench_function("pack", |b| b.iter(|| black_box(&board).pack()));
    group.bench_function("unpack", |b| b.iter(|| black_box(&packed).unpack()));
    group.bench_function("roundtrip", |b| {
        b.iter(|| black_box(&board).pack().unpack())
    });
    group.finish();
}

fn bench_hash(c: &mut Criterion) {
    let board = Board::from_fen(KIWIPETE).unwrap();
    c.benchmark_group("zobrist")
        .bench_function("recompute", |b| b.iter(|| black_box(&board).recompute_hash()));
}

fn bench_batch(c: &mut Criterion) {
    // The "process many compact boards at once" path: a dense Vec<Packed>
    // (34 B each) streamed through unpack + work. Reports positions/sec.
    use chess::Packed;
    let fens = [START, KIWIPETE, MIDGAME, "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"];
    let packed: Vec<Packed> = (0..4096)
        .map(|i| Board::from_fen(fens[i % fens.len()]).unwrap().pack())
        .collect();

    let mut group = c.benchmark_group("batch");
    group.throughput(Throughput::Elements(packed.len() as u64));
    group.bench_function("unpack", |b| {
        b.iter(|| {
            let mut acc = 0u64;
            for p in black_box(&packed) {
                acc ^= p.unpack().hash();
            }
            black_box(acc)
        });
    });
    group.bench_function("unpack+legal_count", |b| {
        b.iter(|| {
            let mut acc = 0usize;
            for p in black_box(&packed) {
                acc += p.unpack().legal_moves().len();
            }
            black_box(acc)
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_perft,
    bench_movegen,
    bench_make_unmake,
    bench_attacks,
    bench_pack,
    bench_hash,
    bench_batch
);
criterion_main!(benches);
