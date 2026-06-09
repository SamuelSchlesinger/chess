//! Probe a policy+value net: print value (side-to-move) and top policy moves for
//! a few diagnostic FENs. Confirms the value head learned material/positional sense.
use chess::{Board, PolicyValueNet};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "nets/az_warm.azn".into());
    let net = PolicyValueNet::load(&path).unwrap_or_else(|e| panic!("load {path}: {e}"));
    let cases = [
        ("startpos (~0)", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        ("white up a queen (stm=W, ~ +)", "rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        ("black up a queen (stm=B, ~ +)", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR b KQkq - 0 1"),
        ("white up a rook (stm=W, ~ +)", "rnbqkbn1/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
        ("white down a queen (stm=W, ~ -)", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1"),
    ];
    for (label, fen) in cases {
        let board = Board::from_fen(fen).expect("fen");
        let moves: Vec<_> = board.legal_moves().iter().copied().collect();
        let (v, priors) = net.evaluate(&board, &moves);
        let mut idx: Vec<usize> = (0..moves.len()).collect();
        idx.sort_by(|&a, &b| priors[b].partial_cmp(&priors[a]).unwrap());
        let top: Vec<String> = idx
            .iter()
            .take(3)
            .map(|&i| format!("{}={:.2}", moves[i].to_uci(), priors[i]))
            .collect();
        println!("{label:36}  value={v:+.3}  top: {}", top.join(" "));
    }
}
