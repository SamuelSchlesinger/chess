//! Cross-language conformance over the root fixtures also executed by Lean.

use chess::{Board, Game};

fn rows(content: &str) -> impl Iterator<Item = (usize, Vec<&str>)> {
    content.lines().enumerate().filter_map(|(index, line)| {
        if line.is_empty() || line.starts_with('#') || line.starts_with("id\t") {
            None
        } else {
            Some((index + 1, line.split('\t').collect()))
        }
    })
}

fn bool_field(value: &str) -> bool {
    match value {
        "0" => false,
        "1" => true,
        _ => panic!("expected Boolean fixture field, got {value}"),
    }
}

fn position_id_field(fen: &str) -> String {
    fen.split_whitespace().take(4).collect::<Vec<_>>().join(" ")
}

fn replay(id: &str, start_fen: &str, moves: &str) -> Game {
    let mut game = Game::from_fen(start_fen).unwrap_or_else(|error| panic!("{id}: {error}"));
    if moves != "-" {
        for mv in moves.split_whitespace() {
            game.push_uci(mv)
                .unwrap_or_else(|| panic!("{id}: illegal fixture move {mv}"));
        }
    }
    game
}

#[test]
fn root_perft_fixture_matches_rust() {
    for (line, fields) in rows(include_str!("../../data/perft.tsv")) {
        assert_eq!(fields.len(), 5, "data/perft.tsv:{line}");
        let id = fields[0];
        let mut board = Board::from_fen(fields[1]).unwrap_or_else(|error| panic!("{id}: {error}"));
        let depth: u32 = fields[2].parse().unwrap();
        let expected: u64 = fields[3].parse().unwrap();
        assert_eq!(board.perft(depth), expected, "{id}");
    }
}

#[test]
fn root_move_legality_fixture_matches_rust() {
    for (line, fields) in rows(include_str!("../../data/moves.tsv")) {
        assert_eq!(fields.len(), 5, "data/moves.tsv:{line}");
        let board =
            Board::from_fen(fields[1]).unwrap_or_else(|error| panic!("{}: {error}", fields[0]));
        assert_eq!(
            board.parse_uci(fields[2]).is_some(),
            bool_field(fields[3]),
            "{}",
            fields[0]
        );
    }
}

#[test]
fn root_trace_fixture_matches_rust() {
    for (line, fields) in rows(include_str!("../../data/traces.tsv")) {
        assert_eq!(fields.len(), 13, "data/traces.tsv:{line}");
        let id = fields[0];
        let game = replay(id, fields[1], fields[2]);
        let board = game.board();

        assert_eq!(board.to_fen(), fields[3], "{id}: raw FEN");
        assert_eq!(
            board.position_id(),
            position_id_field(fields[4]),
            "{id}: effective position ID"
        );

        let repetitions: usize = fields[5].parse().unwrap();
        assert_eq!(game.repetition_count(), repetitions, "{id}: repetitions");
        assert_eq!(repetitions >= 3, bool_field(fields[6]), "{id}: threefold");
        assert_eq!(repetitions >= 5, bool_field(fields[7]), "{id}: fivefold");
        assert_eq!(
            board.halfmove_clock() >= 100,
            bool_field(fields[8]),
            "{id}: 50-move threshold"
        );
        assert_eq!(
            board.halfmove_clock() >= 150,
            bool_field(fields[9]),
            "{id}: 75-move threshold"
        );
        let checkmate = board.in_check() && board.legal_moves().is_empty();
        assert_eq!(checkmate, bool_field(fields[10]), "{id}: checkmate");
    }
}

#[test]
fn root_opening_pair_fixture_matches_rust() {
    for (line, fields) in rows(include_str!("../../data/opening_pairs.tsv")) {
        assert_eq!(fields.len(), 12, "data/opening_pairs.tsv:{line}");
        let id = fields[0];
        let left = replay(id, fields[1], fields[2]);
        let right = replay(id, fields[1], fields[3]);

        assert_eq!(left.board().to_fen(), fields[5], "{id}: left raw FEN");
        assert_eq!(right.board().to_fen(), fields[6], "{id}: right raw FEN");
        assert_eq!(
            left.board().position_id(),
            position_id_field(fields[7]),
            "{id}: left effective position ID"
        );
        assert_eq!(
            right.board().position_id(),
            position_id_field(fields[8]),
            "{id}: right effective position ID"
        );

        match fields[4] {
            "exact" => {
                assert_eq!(left.board(), right.board(), "{id}: exact equality");
                assert_eq!(
                    left.board().repetition_key(),
                    right.board().repetition_key(),
                    "{id}: repetition equality"
                );
            }
            "repetition" => {
                assert_ne!(left.board(), right.board(), "{id}: exact distinction");
                assert_eq!(
                    left.board().repetition_key(),
                    right.board().repetition_key(),
                    "{id}: repetition equality"
                );
            }
            relation => panic!("{id}: unknown relation {relation}"),
        }
    }
}
