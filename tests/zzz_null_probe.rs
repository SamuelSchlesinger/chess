use chess::Board;

#[test]
fn double_null_same_hash_when_no_ep() {
    // A quiet middlegame position with NO en-passant square.
    let mut b = Board::from_fen("r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/2N2N2/PPPP1PPP/R1BQK2R w KQkq - 0 1").unwrap();
    let h0 = b.hash();
    let u1 = b.make_null_move();
    let h1 = b.hash();
    let u2 = b.make_null_move();
    let h2 = b.hash();
    eprintln!("h0={:#x} h1={:#x} h2={:#x}", h0, h1, h2);
    assert_ne!(h0, h1, "single null must differ");
    assert_eq!(h0, h2, "double null must equal original when no EP square");
    b.unmake_null_move(u2);
    b.unmake_null_move(u1);
    assert_eq!(b.hash(), h0);
}

#[test]
fn halfmove_increments_across_null() {
    let mut b = Board::from_fen("r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/2N2N2/PPPP1PPP/R1BQK2R w KQkq - 5 1").unwrap();
    let hm0 = b.halfmove_clock();
    let u1 = b.make_null_move();
    assert_eq!(b.halfmove_clock(), hm0 + 1);
    let u2 = b.make_null_move();
    assert_eq!(b.halfmove_clock(), hm0 + 2);
    b.unmake_null_move(u2);
    b.unmake_null_move(u1);
    assert_eq!(b.halfmove_clock(), hm0);
}
