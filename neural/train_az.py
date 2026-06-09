#!/usr/bin/env python3
"""MLX trainer for the AlphaZero policy+value net (matches src/eval/policyvalue.rs).

Reads self-play records from `selfplay`:
  [packed 34][result_white i8][n u8] then n x ([move_index u16][visits u16])
and trains value (MSE to the game outcome, side-to-move) + policy
(cross-entropy to the MCTS visit distribution), exporting the .azn net the Rust
engine loads.

Usage:
  ./neural/.venv/bin/python neural/train_az.py --records data/sp_az \
      --epochs 6 --batch 2048 --lr 1e-3 --out nets/az_gen1.azn
"""
import argparse, glob, os, struct, time
import numpy as np
import mlx.core as mx
import mlx.nn as nn
import mlx.optimizers as optim

INPUTS, FT, HID, POLICY = 768, 256, 256, 4096
MAX_FEAT = 32
MAGIC = 0x415A4E54  # "AZNT"


def stm_features(b34):
    """Decode a 34-byte Packed board -> (stm-relative feature list, value sign)."""
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


def load(prefix, max_rows):
    paths = sorted(glob.glob(prefix + "*")) if not os.path.isfile(prefix) else [prefix]
    feats, vals, pol_idx, pol_p = [], [], [], []
    for p in paths:
        data = open(p, "rb").read()
        o = 0
        while o + 36 <= len(data):
            b34 = data[o : o + 34]; o += 34
            result = struct.unpack("b", data[o : o + 1])[0]; o += 1
            n = data[o]; o += 1
            moves = []
            tot = 0
            for _ in range(n):
                mi = data[o] | (data[o + 1] << 8)
                vis = data[o + 2] | (data[o + 3] << 8)
                o += 4
                moves.append((mi, vis)); tot += vis
            f, sw = stm_features(b34)
            z = float(result if sw else -result)  # outcome, side-to-move
            idx = np.array([m for m, _ in moves], dtype=np.int32)
            pp = np.array([v / max(1, tot) for _, v in moves], dtype=np.float32)
            feats.append(f); vals.append(z); pol_idx.append(idx); pol_p.append(pp)
            if len(feats) >= max_rows:
                return feats, vals, pol_idx, pol_p
    return feats, vals, pol_idx, pol_p


class Net(nn.Module):
    def __init__(self):
        super().__init__()
        self.ft = nn.Linear(INPUTS + 1, FT)   # +1 zero-pad row
        self.h = nn.Linear(FT, HID)
        self.v = nn.Linear(HID, 1)
        self.p = nn.Linear(HID, POLICY)

    def __call__(self, idx):
        acc = self.ft.bias + mx.sum(self.ft.weight.T[idx], axis=1)
        acc = mx.clip(acc, 0.0, 1.0)
        h = nn.relu(self.h(acc))
        return self.v(h)[:, 0], self.p(h)


def load_azn(net, path):
    """Import .azn weights into the Net (reverse of export) for warm-starting."""
    d = open(path, "rb").read()
    magic, _v, ft, hid = struct.unpack("<IIII", d[:16])
    assert magic == MAGIC and ft == FT and hid == HID, "azn header mismatch"
    o = 16
    def take(n):
        nonlocal o
        a = np.frombuffer(d[o : o + n * 4], dtype="<f4").astype(np.float32)
        o += n * 4
        return a
    ftw = take(INPUTS * FT).reshape(INPUTS, FT).T            # -> [FT, INPUTS]
    ftb = take(FT)
    hw = take(FT * HID).reshape(HID, FT)
    hb = take(HID)
    vw = take(HID).reshape(1, HID)
    vb = take(1)
    pw = take(HID * POLICY).reshape(POLICY, HID)
    pb = take(POLICY)
    w = np.zeros((FT, INPUTS + 1), dtype=np.float32)
    w[:, :INPUTS] = ftw
    net.ft.weight = mx.array(w)
    net.ft.bias = mx.array(ftb)
    net.h.weight = mx.array(hw); net.h.bias = mx.array(hb)
    net.v.weight = mx.array(vw); net.v.bias = mx.array(vb)
    net.p.weight = mx.array(pw); net.p.bias = mx.array(pb)
    mx.eval(net.parameters())


def export(net, path):
    def a(x):
        return np.array(x)
    ftw = a(net.ft.weight)[:, :INPUTS].T.reshape(-1)   # feature-major
    ftb = a(net.ft.bias)
    hw = a(net.h.weight).reshape(-1)                    # out-major
    hb = a(net.h.bias)
    vw = a(net.v.weight)[0]
    vb = float(a(net.v.bias)[0])
    pw = a(net.p.weight).reshape(-1)                    # move-major
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
    ap.add_argument("--records", required=True)
    ap.add_argument("--out", default="nets/az.azn")
    ap.add_argument("--epochs", type=int, default=6)
    ap.add_argument("--batch", type=int, default=2048)
    ap.add_argument("--lr", type=float, default=1e-3)
    ap.add_argument("--max-rows", type=int, default=4_000_000)
    ap.add_argument("--warm", default=None, help="warm-start .azn to continue from")
    ap.add_argument("--freeze-value", action="store_true",
                    help="train only the policy head (preserve the warm value)")
    ap.add_argument("--anchor", default=None,
                    help=".azn whose value regularizes the target (defaults to --warm)")
    ap.add_argument("--value-blend", type=float, default=None,
                    help="UNFREEZE value: target = blend*outcome + (1-blend)*anchor_value")
    args = ap.parse_args()
    assert not (args.value_blend is not None and args.freeze_value), \
        "--value-blend trains the value; incompatible with --freeze-value"

    t0 = time.time()
    feats, vals, pidx, pp = load(args.records, args.max_rows)
    n = len(feats)
    print(f"loaded {n:,} positions in {time.time()-t0:.0f}s")

    # Pad features to [n, MAX_FEAT] with the zero-pad index INPUTS.
    F = np.full((n, MAX_FEAT), INPUTS, dtype=np.int32)
    for i, f in enumerate(feats):
        F[i, : min(len(f), MAX_FEAT)] = f[:MAX_FEAT]
    F = mx.array(F)
    z = np.array(vals, dtype=np.float32)  # game outcome, side-to-move
    V = mx.array(z)

    # Outcome-grounded value training with anchor regularization. The self-play
    # outcome z has no teacher ceiling (it is ground truth), but early-game
    # outcomes are high-variance and drawish (~0), which would erode the
    # SF-distilled value. Blending toward a FIXED anchor's value keeps the
    # material/positional sense while injecting the (no-ceiling) outcome signal:
    #   target = blend * z  +  (1 - blend) * anchor_value(position)
    if args.value_blend is not None:
        anchor_path = args.anchor or args.warm
        assert anchor_path, "--value-blend needs --anchor (or --warm) for the value anchor"
        anc = Net(); mx.eval(anc.parameters()); load_azn(anc, anchor_path)
        chunks = []
        for i in range(0, n, 8192):
            av, _ = anc(F[i : i + 8192])
            chunks.append(np.array(av))
        v_anchor = np.concatenate(chunks).astype(np.float32)
        lam = args.value_blend
        V = mx.array((lam * z + (1.0 - lam) * v_anchor).astype(np.float32))
        print(f"value target = {lam:.2f}*outcome + {1-lam:.2f}*anchor[{anchor_path}]"
              f"  (anchor mean |v|={np.abs(v_anchor).mean():.3f})")

    net = Net()
    mx.eval(net.parameters())
    pad = mx.concatenate([mx.ones((FT, INPUTS)), mx.zeros((FT, 1))], axis=1)
    net.ft.weight = net.ft.weight * pad
    if args.warm:
        load_azn(net, args.warm)
        print(f"warm-started from {args.warm}")
    if args.freeze_value:
        # Keep the SF-distilled value + trunk; shape only the policy from search.
        net.ft.freeze(); net.h.freeze(); net.v.freeze()
        print("value+trunk frozen; training policy head only")
    opt = optim.Adam(learning_rate=args.lr)

    def loss_fn(net, idx, target_pol, z):
        v, logits = net(idx)
        logp = logits - mx.logsumexp(logits, axis=1, keepdims=True)
        ploss = mx.mean(-mx.sum(target_pol * logp, axis=1))
        if args.freeze_value:
            return ploss
        vloss = mx.mean((v - z) ** 2)
        return vloss + ploss

    lg = nn.value_and_grad(net, loss_fn)
    rng = np.random.default_rng(0)
    for ep in range(args.epochs):
        perm = rng.permutation(n)
        run = 0.0; nb = 0; te = time.time()
        for i in range(0, n, args.batch):
            b = perm[i : i + args.batch]
            # Build dense policy target for the batch.
            tp = np.zeros((len(b), POLICY), dtype=np.float32)
            for r, j in enumerate(b):
                tp[r, pidx[j]] = pp[j]
            loss, grads = lg(net, F[mx.array(b)], mx.array(tp), V[mx.array(b)])
            opt.update(net, grads)
            mx.eval(net.parameters(), opt.state)
            net.ft.weight = net.ft.weight * pad
            run += float(loss); nb += 1
        print(f"epoch {ep+1}/{args.epochs}: loss {run/nb:.4f}  ({time.time()-te:.1f}s)")
    export(net, args.out)
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
