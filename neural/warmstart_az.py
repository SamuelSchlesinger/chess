#!/usr/bin/env python3
"""Warm-start the AlphaZero policy+value net's VALUE head from Stockfish evals.

The from-scratch self-play cycle failed to bootstrap (gen-1 ~= gen-0) because a
random net produces near-random games -> no learning signal. The fix is to give
generation 0 a *meaningful* value function distilled from Stockfish: then its
value-driven MCTS plays real chess, self-play games carry signal, and RL can
improve from a strong start (combining distillation's floor with RL's no ceiling).

Reads the 37-byte label-sf records:
  [packed 34][score_white i16][wdl i8]
trains the shared trunk + value head (MSE to the logistic win-model of the eval,
side-to-move), leaves the policy head near-uniform (MCTS falls back to
value-driven PUCT), and exports the .azn the Rust engine + selfplay load.

Usage:
  ./neural/.venv/bin/python neural/warmstart_az.py --data data/sp_sf data/sp2_sf \
      --epochs 6 --batch 4096 --lr 1e-3 --out nets/az_warm.azn
"""
import argparse, glob, os, struct, time
import numpy as np
import mlx.core as mx
import mlx.nn as nn
import mlx.optimizers as optim

INPUTS, FT, HID, POLICY = 768, 256, 256, 4096
MAX_FEAT = 32
MAGIC = 0x415A4E54  # "AZNT"
REC = 37  # [packed 34][score i16][wdl i8]


def stm_features(b34):
    """Decode a 34-byte Packed board -> (stm-relative feature list, stm_white)."""
    state = b34[32] | (b34[33] << 8)
    stm_white = (state & 1) == 0
    feats = []
    for sq in range(64):
        byte = b34[sq >> 1]
        code = (byte & 0x0F) if (sq & 1) == 0 else (byte >> 4)
        if code == 0:
            continue
        white = (code & 0b1000) == 0
        t = (code & 0b111) - 1
        rel_color = 0 if (white == stm_white) else 1
        rel_sq = sq if stm_white else (sq ^ 56)
        feats.append((rel_color * 6 + t) * 64 + rel_sq)
    return feats, stm_white


def win_value(cp, z, blend):
    """Side-to-move value in [-1,1]: logistic win model of eval, blended with outcome."""
    cp = max(-2000, min(2000, cp))
    wp = 1.0 / (1.0 + 10 ** (-cp / 400.0))  # P(win) for the eval's side (white)
    v_eval = 2.0 * wp - 1.0
    return (1.0 - blend) * v_eval + blend * float(z)


def load(prefixes, max_rows, blend):
    paths = []
    for pre in prefixes:
        paths += sorted(glob.glob(pre + "*")) if not os.path.isfile(pre) else [pre]
    feats, vals = [], []
    for p in paths:
        data = open(p, "rb").read()
        for o in range(0, len(data) - REC + 1, REC):
            b34 = data[o : o + 34]
            score = struct.unpack("<h", data[o + 34 : o + 36])[0]  # white POV
            wdl = struct.unpack("b", data[o + 36 : o + 37])[0]     # white POV
            f, sw = stm_features(b34)
            cp = score if sw else -score
            z = wdl if sw else -wdl
            feats.append(f)
            vals.append(win_value(cp, z, blend))
            if len(feats) >= max_rows:
                return feats, vals
    return feats, vals


class Net(nn.Module):
    def __init__(self):
        super().__init__()
        self.ft = nn.Linear(INPUTS + 1, FT)  # +1 zero-pad row
        self.h = nn.Linear(FT, HID)
        self.v = nn.Linear(HID, 1)
        self.p = nn.Linear(HID, POLICY)

    def trunk(self, idx):
        acc = self.ft.bias + mx.sum(self.ft.weight.T[idx], axis=1)
        acc = mx.clip(acc, 0.0, 1.0)
        return nn.relu(self.h(acc))

    def __call__(self, idx):
        return self.v(self.trunk(idx))[:, 0]


def export(net, path):
    a = lambda x: np.array(x)
    ftw = a(net.ft.weight)[:, :INPUTS].T.reshape(-1)
    ftb = a(net.ft.bias)
    hw = a(net.h.weight).reshape(-1)
    hb = a(net.h.bias)
    vw = a(net.v.weight)[0]
    vb = float(a(net.v.bias)[0])
    pw = a(net.p.weight).reshape(-1)
    pb = a(net.p.bias)
    with open(path, "wb") as f:
        f.write(struct.pack("<IIII", MAGIC, 1, FT, HID))
        for arr in (ftw, ftb, hw, hb, vw):
            f.write(arr.astype("<f4").tobytes())
        f.write(struct.pack("<f", vb))
        for arr in (pw, pb):
            f.write(arr.astype("<f4").tobytes())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", nargs="+", required=True)
    ap.add_argument("--out", default="nets/az_warm.azn")
    ap.add_argument("--epochs", type=int, default=6)
    ap.add_argument("--batch", type=int, default=4096)
    ap.add_argument("--lr", type=float, default=1e-3)
    ap.add_argument("--blend", type=float, default=0.2, help="weight on game outcome vs eval")
    ap.add_argument("--max-rows", type=int, default=4_000_000)
    args = ap.parse_args()

    t0 = time.time()
    feats, vals = load(args.data, args.max_rows, args.blend)
    n = len(feats)
    print(f"loaded {n:,} positions in {time.time()-t0:.0f}s")

    F = np.full((n, MAX_FEAT), INPUTS, dtype=np.int32)
    for i, f in enumerate(feats):
        F[i, : min(len(f), MAX_FEAT)] = f[:MAX_FEAT]
    F = mx.array(F)
    V = mx.array(np.array(vals, dtype=np.float32))

    # 90/10 split for an honest val number.
    rng = np.random.default_rng(0)
    perm = rng.permutation(n)
    nval = n // 10
    vi, ti = perm[:nval], perm[nval:]

    net = Net()
    mx.eval(net.parameters())
    pad = mx.concatenate([mx.ones((FT, INPUTS)), mx.zeros((FT, 1))], axis=1)
    net.ft.weight = net.ft.weight * pad
    # Shrink policy logits so untrained priors stay near-uniform after softmax.
    net.p.weight = net.p.weight * 0.0
    net.p.bias = net.p.bias * 0.0
    opt = optim.Adam(learning_rate=args.lr)

    def loss_fn(net, idx, z):
        return mx.mean((net(idx) - z) ** 2)

    lg = nn.value_and_grad(net, loss_fn)
    best = 1e9
    for ep in range(args.epochs):
        tp = rng.permutation(ti)
        run = 0.0; nb = 0; te = time.time()
        for i in range(0, len(tp), args.batch):
            b = tp[i : i + args.batch]
            loss, grads = lg(net, F[mx.array(b)], V[mx.array(b)])
            opt.update(net, grads)
            mx.eval(net.parameters(), opt.state)
            net.ft.weight = net.ft.weight * pad
            run += float(loss); nb += 1
        # Validation MSE.
        vloss = 0.0; vb_ = 0
        for i in range(0, len(vi), args.batch):
            b = vi[i : i + args.batch]
            vloss += float(mx.mean((net(F[mx.array(b)]) - V[mx.array(b)]) ** 2)); vb_ += 1
        vloss /= max(1, vb_)
        tag = ""
        if vloss < best:
            best = vloss; export(net, args.out); tag = "  *"
        print(f"epoch {ep+1}/{args.epochs}: train {run/nb:.4f}  val {vloss:.4f}  ({time.time()-te:.1f}s){tag}")
    print(f"wrote {args.out} (best val {best:.4f})")


if __name__ == "__main__":
    main()
