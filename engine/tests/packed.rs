//! The 34-byte canonical form must round-trip every position losslessly (up to
//! the intentionally-dropped full-move number and the 7-bit-clamped half-move
//! clock), and its random-access API must agree with a full board.

use chess::{Board, Square};
use std::fs;

fn assert_round_trip(fen: &str) {
    let board = Board::from_fen(fen).unwrap();
    let packed = board.pack();
    let back = packed.unpack();

    // Zobrist captures placement + side + castling + ep — everything the packed
    // form is required to preserve.
    assert_eq!(board.hash(), back.hash(), "hash changed for {fen}");

    // Every square agrees.
    for i in 0..64u8 {
        let sq = Square(i);
        assert_eq!(board.piece_at(sq), back.piece_at(sq), "square {sq} for {fen}");
        // Random access on the packed form (no unpack) also agrees.
        assert_eq!(packed.piece_at(sq), board.piece_at(sq), "packed sq {sq} for {fen}");
    }
    assert_eq!(packed.side_to_move(), board.side_to_move(), "{fen}");
    assert_eq!(packed.castling_rights(), board.castling_rights(), "{fen}");
    assert_eq!(packed.en_passant_square(), board.en_passant_square(), "{fen}");
    assert_eq!(
        packed.halfmove_clock(),
        board.halfmove_clock().min(127),
        "{fen}"
    );
}

#[test]
fn packed_is_34_bytes() {
    assert_eq!(core::mem::size_of::<chess::Packed>(), 34);
}

#[test]
fn packed_round_trip_landmarks() {
    for fen in [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3", // ep set
        "8/8/8/4k3/8/4K3/8/5N2 w - - 0 1",
    ] {
        assert_round_trip(fen);
    }
}

#[test]
fn packed_round_trip_suite() {
    let data = fs::read_to_string("tests/data/perft_vajolet.txt").unwrap();
    let mut n = 0;
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fen = line.split(',').next().unwrap().trim();
        assert_round_trip(fen);
        n += 1;
    }
    assert!(n > 1000, "expected the full suite, saw {n}");
}
