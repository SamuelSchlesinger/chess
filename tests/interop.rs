//! Interop correctness against downloaded oracles:
//!  * Polyglot Zobrist keys (`tests/data/zobrist_polyglot.txt`)
//!  * FEN round-tripping over the whole perft suite
//!  * Incremental-vs-recomputed hash consistency under make/unmake
//!  * UCI / SAN round-trips

use chess::{Board, Game};
use std::fs;

#[test]
fn polyglot_zobrist_reference_keys() {
    let data = fs::read_to_string("tests/data/zobrist_polyglot.txt").unwrap();
    let mut checked = 0;
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (fen, key_hex) = line.split_once(';').expect("FEN;key");
        let expected = u64::from_str_radix(key_hex.trim().trim_start_matches("0x"), 16).unwrap();
        let board = Board::from_fen(fen.trim()).unwrap();
        assert_eq!(
            board.hash(),
            expected,
            "incremental hash mismatch for {fen}: got {:#018x} want {expected:#018x}",
            board.hash()
        );
        assert_eq!(
            board.recompute_hash(),
            expected,
            "recomputed hash mismatch for {fen}"
        );
        checked += 1;
    }
    assert_eq!(checked, 9, "expected 9 reference positions");
}

#[test]
fn fen_round_trip_suite() {
    // Parse -> serialize -> parse, and assert the boards are identical. (The
    // suite FENs omit clock fields, so string equality is checked only for the
    // canonical 6-field re-serialization round-trip.)
    let data = fs::read_to_string("tests/data/perft_vajolet.txt").unwrap();
    let mut checked = 0;
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fen = line.split(',').next().unwrap().trim();
        let board = Board::from_fen(fen).unwrap();
        let serialized = board.to_fen();
        let reparsed = Board::from_fen(&serialized).unwrap();
        assert_eq!(board, reparsed, "round-trip changed board for {fen}");
        assert_eq!(
            serialized,
            reparsed.to_fen(),
            "serialization not idempotent for {fen}"
        );
        checked += 1;
    }
    assert!(checked > 1000);
}

#[test]
fn incremental_hash_matches_recompute_under_play() {
    // Walk a moderate tree, asserting the incremental hash equals a full
    // recompute at every node, and that unmake restores it exactly.
    fn walk(board: &mut Board, depth: u32) {
        assert_eq!(
            board.hash(),
            board.recompute_hash(),
            "hash desync at {}",
            board.to_fen()
        );
        if depth == 0 {
            return;
        }
        let before = board.hash();
        for &mv in board.legal_moves().iter() {
            let undo = board.make_move(mv);
            walk(board, depth - 1);
            board.unmake_move(mv, undo);
            assert_eq!(board.hash(), before, "hash not restored after unmake");
        }
    }
    for fen in [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    ] {
        let mut board = Board::from_fen(fen).unwrap();
        walk(&mut board, 3);
    }
}

#[test]
fn uci_round_trip() {
    // For every legal move in a set of positions, UCI render then parse must
    // recover the same move.
    for fen in [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        // promotion-rich position
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    ] {
        let board = Board::from_fen(fen).unwrap();
        for &mv in board.legal_moves().iter() {
            let uci = mv.to_uci();
            let parsed = board.parse_uci(&uci).expect("parse own uci");
            assert_eq!(parsed, mv, "uci round-trip failed: {uci}");
        }
    }
}

#[test]
fn san_round_trip() {
    for fen in [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    ] {
        let board = Board::from_fen(fen).unwrap();
        for &mv in board.legal_moves().iter() {
            let san = board.san(mv);
            let parsed = board.parse_san(&san).unwrap_or_else(|| panic!("parse san {san}"));
            assert_eq!(parsed, mv, "san round-trip failed: {san}");
        }
    }
}

#[test]
fn scholars_mate_is_checkmate() {
    // 1.e4 e5 2.Bc4 Nc6 3.Qh5 Nf6?? 4.Qxf7#
    let mut game = Game::new();
    for mv in ["e4", "e5", "Bc4", "Nc6", "Qh5", "Nf6", "Qxf7"] {
        assert!(game.push_san(mv).is_some(), "failed to play {mv}");
    }
    assert!(game.is_checkmate(), "expected checkmate, got {:?}", game.outcome());
    match game.outcome() {
        chess::Outcome::Checkmate { winner } => assert_eq!(winner, chess::Color::White),
        other => panic!("expected white checkmate, got {other:?}"),
    }
}
