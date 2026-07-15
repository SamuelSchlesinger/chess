//! Probe a policy+value net: print value (side-to-move) and top policy moves for
//! a few diagnostic FENs. Confirms the value head learned material/positional sense.
//!
//! With `--eval-server <socket>` the evaluations go through a remote batched
//! eval server (neural/eval_server.py) instead of the local net — run both ways
//! and diff to verify the server path end to end.
use chess::{Board, Guide, PolicyValueNet, RemoteGuide};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut path = "nets/az_warm.azn".to_string();
    let mut server: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--eval-server" && i + 1 < args.len() {
            server = Some(args[i + 1].clone());
            i += 2;
        } else {
            path = args[i].clone();
            i += 1;
        }
    }
    let mut local = None;
    let mut remote = None;
    match &server {
        Some(s) => {
            remote = Some(RemoteGuide::connect(s).unwrap_or_else(|e| panic!("connect {s}: {e}")))
        }
        None => {
            local = Some(PolicyValueNet::load(&path).unwrap_or_else(|e| panic!("load {path}: {e}")))
        }
    }
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
        let (v, priors) = match (&mut remote, &local) {
            (Some(r), _) => r.evaluate(&board, &moves),
            (None, Some(n)) => n.evaluate(&board, &moves),
            _ => unreachable!(),
        };
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
