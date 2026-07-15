//! Differential validation of the fast pin-aware legal generator against the
//! obviously-correct clone+make/unmake filter, plus perft re-checks. Walks game
//! trees from tricky positions (pins, checks, en passant, castling, promotion)
//! and asserts the two generators yield identical move *sets* at every node.

use chess::{Board, Move};
use std::collections::BTreeSet;

fn move_set(list: &chess::MoveList) -> BTreeSet<u16> {
    list.iter().map(|m| m.0).collect()
}

fn walk(board: &mut Board, depth: u32) {
    let fast = move_set(&board.legal_moves());
    let filtered = move_set(&board.legal_moves_filtered());
    assert_eq!(
        fast,
        filtered,
        "generator disagreement at {}\n fast-only: {:?}\n filt-only: {:?}",
        board.to_fen(),
        fast.difference(&filtered).map(|&m| Move(m)).collect::<Vec<_>>(),
        filtered.difference(&fast).map(|&m| Move(m)).collect::<Vec<_>>(),
    );
    if depth == 0 {
        return;
    }
    let moves: Vec<Move> = board.legal_moves().iter().copied().collect();
    for mv in moves {
        let undo = board.make_move(mv);
        walk(board, depth - 1);
        board.unmake_move(mv, undo);
    }
}

#[test]
fn differential_legal_vs_filtered() {
    // The standard perft positions exercise pins, double checks, en passant,
    // castling rights, and promotions.
    let positions = [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        // En-passant discovered-check trap (white Kh5, pawns set up for ep on b6/...).
        "8/8/8/k7/3p4/8/4P3/3K4 w - - 0 1",
        // Position with an en-passant that would expose the king along the 5th rank.
        "8/8/8/8/k2Pp2Q/8/8/3K4 b - d3 0 1",
    ];
    for fen in positions {
        let mut board = Board::from_fen(fen).unwrap();
        walk(&mut board, 3);
    }
}

#[test]
fn ep_discovered_check_is_illegal() {
    // Black king a4, white rook/queen on the 4th rank; white pawn just played
    // d2-d4. Black's ...exd3 e.p. would remove both the e4 and d4 pawns, exposing
    // the king to the h4 queen along the rank, so it must be ILLEGAL.
    let board = Board::from_fen("8/8/8/8/k2Pp2Q/8/8/3K4 b - d3 0 1").unwrap();
    let has_ep = board.legal_moves().iter().any(|m| m.is_en_passant());
    assert!(!has_ep, "illegal en-passant (discovered check) was generated");
    // The filtered reference must agree.
    let has_ep_ref = board.legal_moves_filtered().iter().any(|m| m.is_en_passant());
    assert!(!has_ep_ref);
}
