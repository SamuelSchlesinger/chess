//! TEMP probe.
use chess::{Board, Engine, Limits};

#[test]
fn minimize_board314() {
    let fen = "7k/8/8/8/8/8/6PP/Q6K w - - 0 1";
    let board = Board::from_fen(fen).unwrap();
    eprintln!("legal_moves at root: {}", board.legal_moves().len());
    eprintln!("in_check: {}", board.in_check());
    for &mv in board.legal_moves().iter() {
        eprintln!(
            "  {} cap={} promo={} ep={}",
            board.san(mv),
            mv.is_capture(),
            mv.is_promotion(),
            mv.is_en_passant()
        );
    }
    let mut engine = Engine::new();
    let a = engine.analyze(&board, &Limits::depth(1));
    eprintln!("bestmove {} score {}", board.san(a.best_move), a.score);
}

#[test]
fn deep_check_ladder_no_overflow() {
    // Positions chosen to maximize long non-repeating checking lines with
    // occasional irreversible (pawn/capture) moves that reset the 50-move
    // counter, per the CLAIM's mechanism. Search very deep / many nodes.
    // If ply reaches 128 the fixed [MAX_PLY] arrays panic at search.rs:340.
    let positions: &[&str] = &[
        // Exposed kings, lots of pawns -> long forcing lines.
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        // Endgame king hunt with pawns to push.
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        // Sharp open position.
        "r1b1k2r/pppp1ppp/2n2n2/2bNp1q1/2B1P3/3P4/PPP2PPP/RNBQK2R w KQkq - 0 1",
        // Two-rook + exposed kings.
        "6k1/5ppp/8/8/8/8/5PPP/4R1K1 w - - 0 1",
    ];
    for fen in positions {
        let board = Board::from_fen(fen).unwrap();
        let mut limits = Limits::depth(120);
        limits.nodes = Some(40_000_000);
        let mut engine = Engine::new();
        let a = engine.analyze(&board, &limits);
        eprintln!("seldepth={} nodes={} for {}", a.seldepth, a.nodes, fen);
        assert!(a.seldepth < 128, "seldepth reached {} for {}", a.seldepth, fen);
    }
}
