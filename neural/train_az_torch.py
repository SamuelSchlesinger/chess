#!/usr/bin/env python3
"""CUDA/PyTorch trainer for the AlphaZero policy+value net — the 3080 drop-in.

Byte-for-byte compatible with the MLX trainers and the Rust loader
(src/eval/policyvalue.rs): same architecture, same .azn export, same record
formats. This is the trainer to run on the RTX 3080; on the M4 it runs on CPU/MPS
for verification. The validated recipe (FINDINGS.md) lives here:
  * value warm-start from Stockfish evals          (--value-data)
  * policy RL from MCTS visits, value frozen        (--records --freeze-value)
  * value-unfreeze with the q-target + anchor       (--records --value-blend --beta)

Formats:
  self-play  : [packed 34][result i8][root_q i16][n u8] then n×[mi u16][vis u16]
  value-data : [packed 34][score_white i16][wdl i8]   (label-sf output, 37 B)
Export (.azn): [magic u32=0x415A4E54][ver u32][FT u32][HID u32] then f32
  ft_w[768*FT] feature-major, ft_b[FT], h_w[FT*HID] out-major, h_b[HID],
  v_w[HID], v_b, p_w[HID*POLICY] move-major, p_b[POLICY].
"""
import argparse, glob, os, struct, time
import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

INPUTS, FT, HID, POLICY = 768, 256, 256, 4096
MAX_FEAT = 32
MAGIC = 0x415A4E54


def stm_features(b34):
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


def load_selfplay(prefix, max_rows):
    paths = sorted(glob.glob(prefix + "*")) if not os.path.isfile(prefix) else [prefix]
    feats, z, q, pidx, pp = [], [], [], [], []
    for p in paths:
        d = open(p, "rb").read(); o = 0
        while o + 38 <= len(d):
            b34 = d[o:o+34]; o += 34
            res = struct.unpack("b", d[o:o+1])[0]; o += 1
            rq = struct.unpack("<h", d[o:o+2])[0] / 10000.0; o += 2
            n = d[o]; o += 1
            if o + 4 * n > len(d):
                break  # truncated tail (e.g. unflushed shard from a killed run)
            mv = []; tot = 0
            for _ in range(n):
                mi = d[o] | (d[o+1] << 8); vis = d[o+2] | (d[o+3] << 8); o += 4
                mv.append((mi, vis)); tot += vis
            f, sw = stm_features(b34)
            feats.append(f); z.append(float(res if sw else -res)); q.append(rq)
            pidx.append(np.array([m for m, _ in mv], dtype=np.int64))
            pp.append(np.array([v/max(1, tot) for _, v in mv], dtype=np.float32))
            if len(feats) >= max_rows:
                return feats, z, q, pidx, pp
    return feats, z, q, pidx, pp


def load_value(prefixes, max_rows, blend):
    paths = []
    for pre in prefixes:
        paths += sorted(glob.glob(pre + "*")) if not os.path.isfile(pre) else [pre]
    feats, vals = [], []
    for p in paths:
        d = open(p, "rb").read()
        for o in range(0, len(d) - 37 + 1, 37):
            b34 = d[o:o+34]
            score = struct.unpack("<h", d[o+34:o+36])[0]
            wdl = struct.unpack("b", d[o+36:o+37])[0]
            f, sw = stm_features(b34)
            cp = max(-2000, min(2000, score if sw else -score))
            wp = 1.0 / (1.0 + 10 ** (-cp / 400.0))
            v = (1 - blend) * (2 * wp - 1) + blend * float(wdl if sw else -wdl)
            feats.append(f); vals.append(v)
            if len(feats) >= max_rows:
                return feats, vals
    return feats, vals


class Net(nn.Module):
    def __init__(self):
        super().__init__()
        self.ft = nn.Linear(INPUTS + 1, FT)
        self.h = nn.Linear(FT, HID)
        self.v = nn.Linear(HID, 1)
        self.p = nn.Linear(HID, POLICY)

    def trunk(self, idx):
        # idx: [B, MAX_FEAT] with pad index INPUTS. Sparse accumulate + clipped-ReLU.
        w = self.ft.weight.t()                      # [INPUTS+1, FT]
        acc = self.ft.bias + w[idx].sum(dim=1)       # [B, FT]
        acc = torch.clamp(acc, 0.0, 1.0)
        return F.relu(self.h(acc))

    def forward(self, idx):
        h = self.trunk(idx)
        return torch.tanh(self.v(h)[:, 0]), self.p(h)  # tanh matches Rust inference


def pad_features(feats):
    n = len(feats)
    arr = np.full((n, MAX_FEAT), INPUTS, dtype=np.int64)
    for i, f in enumerate(feats):
        arr[i, : min(len(f), MAX_FEAT)] = f[:MAX_FEAT]
    return arr


def zero_pad_row(net):
    with torch.no_grad():
        net.ft.weight[:, INPUTS] = 0.0


def load_azn(net, path, device):
    d = open(path, "rb").read()
    magic, _v, ft, hid = struct.unpack("<IIII", d[:16])
    assert magic == MAGIC and ft == FT and hid == HID
    o = 16
    def take(k):
        nonlocal o
        a = np.frombuffer(d[o:o+k*4], dtype="<f4").astype(np.float32); o += k*4
        return a
    ftw = take(INPUTS*FT).reshape(INPUTS, FT).T
    ftb = take(FT); hw = take(FT*HID).reshape(HID, FT); hb = take(HID)
    vw = take(HID).reshape(1, HID); vb = take(1)
    pw = take(HID*POLICY).reshape(POLICY, HID); pb = take(POLICY)
    with torch.no_grad():
        w = np.zeros((FT, INPUTS+1), dtype=np.float32); w[:, :INPUTS] = ftw
        net.ft.weight.copy_(torch.tensor(w, device=device))
        net.ft.bias.copy_(torch.tensor(ftb, device=device))
        net.h.weight.copy_(torch.tensor(hw, device=device)); net.h.bias.copy_(torch.tensor(hb, device=device))
        net.v.weight.copy_(torch.tensor(vw, device=device)); net.v.bias.copy_(torch.tensor(vb, device=device))
        net.p.weight.copy_(torch.tensor(pw, device=device)); net.p.bias.copy_(torch.tensor(pb, device=device))


def export(net, path):
    g = {k: v.detach().cpu().numpy() for k, v in net.state_dict().items()}
    ftw = g["ft.weight"][:, :INPUTS].T.reshape(-1)
    hw = g["h.weight"].reshape(-1)
    vw = g["v.weight"][0]; pw = g["p.weight"].reshape(-1)
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "wb") as f:
        f.write(struct.pack("<IIII", MAGIC, 1, FT, HID))
        for arr in (ftw, g["ft.bias"], hw, g["h.bias"], vw):
            f.write(arr.astype("<f4").tobytes())
        f.write(struct.pack("<f", float(g["v.bias"][0])))
        for arr in (pw, g["p.bias"]):
            f.write(arr.astype("<f4").tobytes())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--records", default=None, help="self-play prefix (policy/value RL)")
    ap.add_argument("--value-data", nargs="+", default=None,
                    help="label-sf prefix(es) (value warm-start)")
    ap.add_argument("--out", default="nets/az.azn")
    ap.add_argument("--warm", default=None)
    ap.add_argument("--anchor", default=None)
    ap.add_argument("--freeze-value", action="store_true")
    ap.add_argument("--value-blend", type=float, default=None)
    ap.add_argument("--beta", type=float, default=0.3)
    ap.add_argument("--blend", type=float, default=0.15, help="value-data: outcome vs eval")
    ap.add_argument("--epochs", type=int, default=8)
    ap.add_argument("--batch", type=int, default=4096)
    ap.add_argument("--lr", type=float, default=1e-3)
    ap.add_argument("--max-rows", type=int, default=8_000_000)
    ap.add_argument("--device", default=None)
    args = ap.parse_args()
    assert bool(args.records) ^ bool(args.value_data), "exactly one of --records / --value-data"

    dev = args.device or ("cuda" if torch.cuda.is_available()
                          else "mps" if torch.backends.mps.is_available() else "cpu")
    print(f"device: {dev}")
    t0 = time.time()

    if args.value_data:
        feats, vals = load_value(args.value_data, args.max_rows, args.blend)
        pidx = pp = None
        z = np.array(vals, dtype=np.float32); q = None
    else:
        feats, zl, ql, pidx, pp = load_selfplay(args.records, args.max_rows)
        z = np.array(zl, dtype=np.float32); q = np.array(ql, dtype=np.float32)
    n = len(feats)
    print(f"loaded {n:,} positions in {time.time()-t0:.0f}s")

    Fpad = torch.tensor(pad_features(feats), device=dev)
    net = Net().to(dev)
    zero_pad_row(net)
    if args.warm:
        load_azn(net, args.warm, dev); print(f"warm-started from {args.warm}")
    if args.value_data:
        with torch.no_grad():
            net.p.weight.zero_(); net.p.bias.zero_()  # near-uniform priors

    # Build the value target.
    Vt = torch.tensor(z, device=dev)
    if args.records and args.value_blend is not None:
        anchor_path = args.anchor or args.warm
        anc = Net().to(dev); load_azn(anc, anchor_path, dev); anc.eval()
        with torch.no_grad():
            va = torch.cat([anc(Fpad[i:i+8192])[0] for i in range(0, n, 8192)])
        lam, beta = args.value_blend, args.beta
        qy = torch.tensor(beta*z + (1-beta)*q, device=dev)
        Vt = lam*qy + (1-lam)*va
        print(f"value target = {lam:.2f}*({beta:.2f}z+{1-beta:.2f}q) + {1-lam:.2f}*anchor")

    if args.freeze_value:
        for m in (net.ft, net.h, net.v):
            for pm in m.parameters():
                pm.requires_grad_(False)
        print("value+trunk frozen; policy only")

    Pt = None
    if pidx is not None:
        Pt = torch.zeros((n, POLICY), dtype=torch.float32)
        for i in range(n):
            # accumulate=True: promotions share a move_index in older records
            Pt[i].index_put_((torch.from_numpy(pidx[i]),), torch.from_numpy(pp[i]),
                             accumulate=True)
        Pt = Pt.to(dev)

    opt = torch.optim.Adam([p for p in net.parameters() if p.requires_grad],
                           lr=args.lr, betas=(0.9, 0.99))
    rng = np.random.default_rng(0)
    for ep in range(args.epochs):
        perm = rng.permutation(n); run = 0.0; nb = 0; te = time.time()
        for i in range(0, n, args.batch):
            b = torch.tensor(perm[i:i+args.batch], device=dev)
            v, logits = net(Fpad[b])
            loss = torch.tensor(0.0, device=dev)
            if not args.freeze_value:
                loss = loss + ((v - Vt[b]) ** 2).mean()
            if Pt is not None:
                logp = logits - torch.logsumexp(logits, dim=1, keepdim=True)
                loss = loss + (-(Pt[b] * logp).sum(dim=1)).mean()
            opt.zero_grad(); loss.backward(); opt.step()
            zero_pad_row(net)
            run += loss.item(); nb += 1
        print(f"epoch {ep+1}/{args.epochs}: loss {run/nb:.4f}  ({time.time()-te:.1f}s)")
    export(net, args.out)
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
