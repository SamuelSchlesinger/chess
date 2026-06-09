#!/usr/bin/env python3
"""MLX NNUE trainer (M4 Pro GPU) — the scaling path past the in-repo Rust trainer.

Trains the 768->HIDDEN x2 ->1 perspective net to bit-compatibility with the Rust
engine (src/eval/nnue.rs) and exports the same .nnue format. Reads either:
  * a Lichess `chess-position-evaluations` parquet shard (fen, cp, depth), or
  * a directory of our 37-byte self-play/SF-labeled records (--records prefix).

Convention (LOCKED to match Rust, do NOT change one side only):
  model output `out` is UNSCALED; train target = sigmoid(cp_stm / 400);
  loss = mean (sigmoid(out) - target)^2 ;  inference eval_cp = round(out*400).

Usage:
  ./neural/.venv/bin/python neural/train_mlx.py \
      --parquet ~/.cache/huggingface/.../train-00000-of-00017.parquet \
      --min-depth 12 --epochs 6 --batch 16384 --lr 1e-3 --hidden 256 \
      --out nets/lichess_v1.nnue
"""
import argparse, math, struct, time
import numpy as np
import mlx.core as mx
import mlx.nn as nn
import mlx.optimizers as optim

SCALE = 400.0
INPUTS = 768
PIECE = {"p": 0, "n": 1, "b": 2, "r": 3, "q": 4, "k": 5}
MAX_FEAT = 32  # max pieces on board


def fen_features(fen):
    """(white_feats, black_feats, stm_white) for a FEN, matching Rust feature_indices."""
    placement, stm = fen.split(" ")[0], fen.split(" ")[1]
    wf, bf = [], []
    rank = 7
    for row in placement.split("/"):
        file = 0
        for ch in row:
            if ch.isdigit():
                file += int(ch)
            else:
                white = ch.isupper()
                t = PIECE[ch.lower()]
                sq = rank * 8 + file
                wrc = 0 if white else 1
                brc = 0 if not white else 1
                wf.append((wrc * 6 + t) * 64 + sq)
                bf.append((brc * 6 + t) * 64 + (sq ^ 56))
                file += 1
        rank -= 1
    return wf, bf, (stm == "w")


def build_arrays(samples):
    """samples: list of (wf, bf, stm_white, target). Returns padded mx arrays."""
    n = len(samples)
    w = np.full((n, MAX_FEAT), INPUTS, dtype=np.int32)  # pad index = INPUTS (zero row)
    b = np.full((n, MAX_FEAT), INPUTS, dtype=np.int32)
    stm = np.zeros((n,), dtype=np.bool_)
    tgt = np.zeros((n,), dtype=np.float32)
    for i, (wf, bf, sw, t) in enumerate(samples):
        w[i, : len(wf)] = wf[:MAX_FEAT]
        b[i, : len(bf)] = bf[:MAX_FEAT]
        stm[i] = sw
        tgt[i] = t
    return mx.array(w), mx.array(b), mx.array(stm), mx.array(tgt)


class Net(nn.Module):
    def __init__(self, hidden, screlu):
        super().__init__()
        self.hidden = hidden
        self.screlu = screlu
        # +1 input row is the zero-pad row (kept zero).
        self.ft = nn.Linear(INPUTS + 1, hidden, bias=True)
        self.out = nn.Linear(2 * hidden, 1, bias=True)

    def __call__(self, w_idx, b_idx, stm_white):
        Wt = self.ft.weight.T  # [INPUTS+1, hidden]
        acc_w = self.ft.bias + mx.sum(Wt[w_idx], axis=1)  # [B, hidden]
        acc_b = self.ft.bias + mx.sum(Wt[b_idx], axis=1)
        sw = stm_white[:, None]
        stm = mx.where(sw, acc_w, acc_b)
        opp = mx.where(sw, acc_b, acc_w)
        x = mx.clip(mx.concatenate([stm, opp], axis=1), 0.0, 1.0)
        if self.screlu:
            x = x * x
        return self.out(x)[:, 0]


def export_nnue(net, path, version=1):
    """Write the Rust .nnue format: magic, version, inputs, hidden, ft_w, ft_b, out_w, out_b.
    Drops the zero-pad input row so the file has exactly INPUTS rows."""
    H = net.hidden
    ftw = np.array(net.ft.weight)[:, :INPUTS]  # [hidden, INPUTS]
    ftw = ftw.T.reshape(-1)  # feature-major [INPUTS*hidden]
    ftb = np.array(net.ft.bias)
    outw = np.array(net.out.weight)[0]  # [2*hidden]
    outb = float(np.array(net.out.bias)[0])
    with open(path, "wb") as f:
        f.write(struct.pack("<IIII", 0x4E4E5545, version, INPUTS, H))
        f.write(ftw.astype("<f4").tobytes())
        f.write(ftb.astype("<f4").tobytes())
        f.write(outw.astype("<f4").tobytes())
        f.write(struct.pack("<f", outb))


def load_parquet(path, min_depth, max_rows):
    import pyarrow.parquet as pq

    pf = pq.ParquetFile(path)
    samples = []
    for batch in pf.iter_batches(batch_size=131072, columns=["fen", "depth", "cp", "mate"]):
        d = batch.to_pydict()
        for fen, depth, cp, mate in zip(d["fen"], d["depth"], d["cp"], d["mate"]):
            if mate is not None or cp is None or depth is None or depth < min_depth:
                continue
            wf, bf, sw = fen_features(fen)
            cp_stm = cp if sw else -cp
            tgt = 1.0 / (1.0 + math.exp(-max(-3000, min(3000, cp_stm)) / SCALE))
            samples.append((wf, bf, sw, tgt))
            if len(samples) >= max_rows:
                return samples
    return samples


def load_records(prefix, max_rows):
    """Read our 37-byte records (packed[34], cp i16 white, wdl i8) -> samples.
    Requires the Rust `dump-fens` is not available, so decode the nibble board here."""
    import glob, os

    paths = sorted(glob.glob(prefix + "*")) if not os.path.isfile(prefix) else [prefix]
    samples = []
    for p in paths:
        data = open(p, "rb").read()
        for off in range(0, len(data) - 36, 37):
            rec = data[off : off + 37]
            cp = struct.unpack("<h", rec[34:36])[0]  # white cp
            wf, bf, sw = packed_features(rec[:34])
            cp_stm = cp if sw else -cp
            tgt = 1.0 / (1.0 + math.exp(-max(-3000, min(3000, cp_stm)) / SCALE))
            samples.append((wf, bf, sw, tgt))
            if len(samples) >= max_rows:
                return samples
    return samples


def packed_features(b34):
    """Decode the 34-byte Packed nibble board -> features (matches src/packed.rs)."""
    state = b34[32] | (b34[33] << 8)
    stm_white = (state & 1) == 0
    wf, bf = [], []
    for sq in range(64):
        byte = b34[sq >> 1]
        code = (byte & 0x0F) if (sq & 1) == 0 else (byte >> 4)
        if code == 0:
            continue
        white = (code & 0b1000) == 0
        t = (code & 0b111) - 1  # 0..5
        wrc = 0 if white else 1
        brc = 0 if not white else 1
        wf.append((wrc * 6 + t) * 64 + sq)
        bf.append((brc * 6 + t) * 64 + (sq ^ 56))
    return wf, bf, stm_white


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--parquet")
    ap.add_argument("--records")
    ap.add_argument("--out", default="nets/mlx_v1.nnue")
    ap.add_argument("--min-depth", type=int, default=12)
    ap.add_argument("--max-rows", type=int, default=20_000_000)
    ap.add_argument("--epochs", type=int, default=6)
    ap.add_argument("--batch", type=int, default=16384)
    ap.add_argument("--lr", type=float, default=1e-3)
    ap.add_argument("--hidden", type=int, default=256)
    ap.add_argument("--screlu", action="store_true")
    ap.add_argument("--val-frac", type=float, default=0.02)
    args = ap.parse_args()

    t0 = time.time()
    if args.parquet:
        samples = load_parquet(args.parquet, args.min_depth, args.max_rows)
    elif args.records:
        samples = load_records(args.records, args.max_rows)
    else:
        raise SystemExit("need --parquet or --records")
    print(f"loaded {len(samples):,} samples in {time.time()-t0:.0f}s")

    rng = np.random.default_rng(0)
    rng.shuffle(samples)
    n_val = max(1, int(len(samples) * args.val_frac))
    val, train = samples[:n_val], samples[n_val:]
    w_tr, b_tr, s_tr, t_tr = build_arrays(train)
    w_va, b_va, s_va, t_va = build_arrays(val)

    net = Net(args.hidden, args.screlu)
    mx.eval(net.parameters())
    opt = optim.Adam(learning_rate=args.lr)

    def loss_fn(net, w, b, s, t):
        out = net(w, b, s)
        pred = mx.sigmoid(out)
        return mx.mean((pred - t) ** 2)

    loss_and_grad = nn.value_and_grad(net, loss_fn)
    ntr = len(train)
    best_val = float("inf")
    for ep in range(args.epochs):
        perm = mx.array(rng.permutation(ntr))
        run = 0.0
        nb = 0
        te = time.time()
        for i in range(0, ntr, args.batch):
            idx = perm[i : i + args.batch]
            loss, grads = loss_and_grad(net, w_tr[idx], b_tr[idx], s_tr[idx], t_tr[idx])
            opt.update(net, grads)
            mx.eval(net.parameters(), opt.state)
            run += float(loss)
            nb += 1
        vloss = float(loss_fn(net, w_va, b_va, s_va, t_va))
        mark = ""
        if vloss < best_val:
            best_val = vloss
            export_nnue(net, args.out)
            mark = " * (saved)"
        print(
            f"epoch {ep+1}/{args.epochs}: train {run/nb:.5f}  val {vloss:.5f}{mark}  "
            f"({time.time()-te:.1f}s)"
        )
    print(f"best val {best_val:.5f} -> {args.out}")


if __name__ == "__main__":
    main()
