use chess::{Board, Game, Square};

#[test]
fn pinned_en_passant_is_ignored_by_fide_identity() {
    let raw = Board::from_fen("8/8/8/K1Pp3r/8/8/8/7k w - d6 0 1").unwrap();
    let absent = Board::from_fen("8/8/8/K1Pp3r/8/8/8/7k w - - 17 9").unwrap();

    // Polyglot sees the adjacent pawn and includes the file.  FIDE asks
    // whether the capture is legal; c5xd6 e.p. would expose the king on a5.
    assert_ne!(raw.hash(), absent.hash());
    assert_eq!(raw.effective_en_passant_square(), None);
    assert_eq!(raw.repetition_key(), absent.repetition_key());
    assert_eq!(raw.position_id(), "8/8/8/K1Pp3r/8/8/8/7k w - -");
    assert_eq!(raw.position_id(), absent.position_id());
}

#[test]
fn legal_en_passant_changes_fide_identity() {
    let raw = Board::from_fen("8/8/8/K1Pp4/8/8/8/7k w - d6 0 1").unwrap();
    let absent = Board::from_fen("8/8/8/K1Pp4/8/8/8/7k w - - 0 1").unwrap();

    assert_eq!(
        raw.effective_en_passant_square(),
        Square::from_algebraic("d6")
    );
    assert_ne!(raw.repetition_key(), absent.repetition_key());
    assert_eq!(raw.position_id(), "8/8/8/K1Pp4/8/8/8/7k w - d6");
    assert_eq!(absent.position_id(), "8/8/8/K1Pp4/8/8/8/7k w - -");
}

#[test]
fn clocks_are_irrelevant_but_castling_rights_are_not() {
    let initial = Board::startpos();
    let changed_clocks =
        Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 73 42").unwrap();
    let lost_rights =
        Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1").unwrap();

    assert_eq!(initial.repetition_key(), changed_clocks.repetition_key());
    assert_eq!(initial.position_id(), changed_clocks.position_id());
    assert_ne!(initial.repetition_key(), lost_rights.repetition_key());
    assert_ne!(initial.position_id(), lost_rights.position_id());
}

#[test]
fn a_reachable_pinned_ep_position_repeats_after_the_target_expires() {
    let mut game = Game::from_fen("7k/3p4/8/K1P4r/8/8/8/8 b - - 0 1").unwrap();

    game.push_uci("d7d5").expect("legal double push");
    let polyglot_with_raw_ep = game.board().hash();
    assert_eq!(game.board().effective_en_passant_square(), None);

    for mv in ["a5a4", "h8h7", "a4a5", "h7h8"] {
        game.push_uci(mv)
            .unwrap_or_else(|| panic!("expected legal move {mv}"));
    }

    assert_eq!(game.repetition_count(), 2);
    assert_eq!(game.position_keys()[1], game.position_keys()[5]);
    assert_ne!(polyglot_with_raw_ep, game.board().hash());
}

#[test]
fn standard_start_threefold_is_not_undercounted_by_polyglot_ep_semantics() {
    let mut game = Game::new();
    let mut polyglot_history = vec![game.board().hash()];
    let setup = [
        "d2d4", "e7e5", "d4e5", "g8f6", "e2e4", "f6e4", "g1f3", "e4c5", "b1c3", "g7g6", "c1f4",
        "f8g7", "d1d2", "e8g8", "h2h3", "f8e8", "a2a3", "d7d5",
    ];
    let knight_cycle = ["f3g1", "b8d7", "g1f3", "d7b8"];

    for mv in setup.into_iter().chain(knight_cycle).chain(knight_cycle) {
        game.push_uci(mv)
            .unwrap_or_else(|| panic!("expected legal move {mv}"));
        polyglot_history.push(game.board().hash());
    }

    // The first occurrence immediately follows ...d7-d5.  e5xd6 e.p. is
    // pseudo-legal but would expose White's king on e1 to the rook on e8, so
    // FIDE identity ignores the target.  The two knight cycles create the
    // second and third occurrences after the raw target has expired.
    assert_eq!(game.repetition_count(), 3);
    let current_polyglot = game.board().hash();
    let legacy_polyglot_count = polyglot_history
        .iter()
        .filter(|&&key| key == current_polyglot)
        .count();
    assert_eq!(legacy_polyglot_count, 2);
}

#[test]
fn shared_position_id_fixture_matches_rust() {
    let fixture = include_str!("../../data/position_ids.tsv");
    let mut rows = 0;
    for (index, line) in fixture.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') || line.starts_with("id\t") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 4, "fixture line {}", index + 1);
        let board =
            Board::from_fen(fields[1]).unwrap_or_else(|error| panic!("{}: {error}", fields[0]));
        assert_eq!(board.position_id(), fields[2], "{}", fields[0]);
        rows += 1;
    }
    assert_eq!(rows, 9);
}
