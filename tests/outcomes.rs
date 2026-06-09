//! Game-outcome and draw-rule validation: stalemate, threefold repetition,
//! 50/75-move rules, and insufficient material.

use chess::{Board, Color, DrawReason, Game, Outcome};

#[test]
fn stalemate_detected() {
    // Black king h8, white queen f7, white king g6 — black is not in check but
    // has no legal move.
    let board = Board::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
    assert!(board.legal_moves().is_empty());
    assert!(!board.in_check());
    assert_eq!(Game::from_board(board).outcome(), Outcome::Stalemate);
}

#[test]
fn back_rank_checkmate() {
    // Black king g8 mated by a rook on e8 with its own pawns blocking.
    let board = Board::from_fen("4R1k1/5ppp/8/8/8/8/8/6K1 b - - 0 1").unwrap();
    assert!(board.legal_moves().is_empty());
    assert!(board.in_check());
    assert_eq!(
        Game::from_board(board).outcome(),
        Outcome::Checkmate { winner: Color::White }
    );
}

#[test]
fn threefold_repetition_by_shuffling() {
    // Knights out and back twice returns to the start position three times.
    let mut g = Game::new();
    for mv in ["Nf3", "Nf6", "Ng1", "Ng8", "Nf3", "Nf6", "Ng1", "Ng8"] {
        assert!(g.push_san(mv).is_some(), "failed {mv}");
    }
    assert_eq!(g.repetition_count(), 3);
    assert!(g.can_claim_draw());
    assert_eq!(g.outcome(), Outcome::Draw(DrawReason::ThreefoldRepetition));
}

#[test]
fn fifty_and_seventy_five_move_rules() {
    // KR vs K (sufficient material) with the half-move clock at the thresholds.
    let fifty = Board::from_fen("8/8/8/4k3/8/4K3/8/R7 w - - 100 1").unwrap();
    assert_eq!(
        Game::from_board(fifty).outcome(),
        Outcome::Draw(DrawReason::FiftyMove)
    );

    let seventy_five = Board::from_fen("8/8/8/4k3/8/4K3/8/R7 w - - 150 1").unwrap();
    assert_eq!(
        Game::from_board(seventy_five).outcome(),
        Outcome::Draw(DrawReason::SeventyFiveMove)
    );

    // Just below the 50-move threshold: still ongoing.
    let ongoing = Board::from_fen("8/8/8/4k3/8/4K3/8/R7 w - - 99 1").unwrap();
    assert_eq!(Game::from_board(ongoing).outcome(), Outcome::Ongoing);
}

#[test]
fn insufficient_material_cases() {
    let insufficient = [
        "8/8/8/4k3/8/4K3/8/8 w - - 0 1",       // K vs K
        "8/8/8/4k3/8/4K3/8/5N2 w - - 0 1",     // K+N vs K
        "8/8/8/4k3/8/4K3/8/5B2 w - - 0 1",     // K+B vs K
        "8/8/4b3/4k3/8/4K3/4B3/8 w - - 0 1",   // K+B vs K+B, both light squares
    ];
    for fen in insufficient {
        let b = Board::from_fen(fen).unwrap();
        assert!(b.is_insufficient_material(), "should be insufficient: {fen}");
        assert_eq!(
            Game::from_board(b).outcome(),
            Outcome::Draw(DrawReason::InsufficientMaterial),
            "{fen}"
        );
    }

    let sufficient = [
        "8/8/8/4k3/8/4K3/8/R7 w - - 0 1",       // K+R vs K
        "8/8/8/4k3/8/4K3/8/5NN1 w - - 0 1",     // K+N+N vs K (not auto-draw)
        "8/8/3b4/4k3/8/4K3/4B3/8 w - - 0 1",    // opposite-colored bishops
        "8/8/8/4k3/8/4K3/4P3/8 w - - 0 1",      // a pawn is always sufficient
    ];
    for fen in sufficient {
        let b = Board::from_fen(fen).unwrap();
        assert!(!b.is_insufficient_material(), "should be sufficient: {fen}");
    }
}

#[test]
fn fools_mate_is_fastest_checkmate() {
    // 1.f3 e5 2.g4 Qh4#
    let mut g = Game::new();
    for mv in ["f3", "e5", "g4", "Qh4"] {
        assert!(g.push_san(mv).is_some(), "failed {mv}");
    }
    assert_eq!(
        g.outcome(),
        Outcome::Checkmate { winner: Color::Black }
    );
}
