#!/usr/bin/env python3
"""Batched GPU evaluation server for Rust self-play — the 3080 throughput unlock.

Serves the policy+value net over a Unix socket to `selfplay --eval-server`.
Each Rust game thread pipelines K leaves per round trip (virtual-loss batching);
the server aggregates requests across ALL connections into one GPU forward, so
the effective batch is K x games-in-flight. The Rust side sends precomputed
stm-relative feature indices and legal-move indices: no chess logic here.

Wire format (little-endian). One FRAME per Rust `evaluate_batch` call (K leaves
per frame from virtual-loss batching), so decode and reply cost one pass per
frame, not per leaf. Per-connection response order matches request order.
  frame   : [body_len u32][n u16] then n x ([nf u8][nmoves u16],
            nf x u16 feature idx, nmoves x u16 move idx)
  response: [n x f32 values][all priors f32, items in order], one write per
            frame (tanh values, softmax over each item's moves). The bulk
            layout lets the batcher slice numpy arrays per frame instead of
            packing per item — the per-item Python loop was the throughput
            ceiling (~25 ms/batch at B=4096 on the 3080).

Usage:
  neural/eval_server.py --net nets/gpu_gen0.azn --socket /tmp/chess_eval.sock
"""
import argparse, os, queue, socket, struct, sys, threading, time
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from train_az_torch import INPUTS, MAX_FEAT, Net, load_azn

FRAME_HDR = struct.Struct("<IH")  # body_len, n_items


def reader(conn, q):
    """Read framed requests off one connection into the shared batch queue."""
    f = conn.makefile("rb")
    try:
        while True:
            hdr = f.read(6)
            if len(hdr) < 6:
                break
            blen, n = FRAME_HDR.unpack(hdr)
            body = f.read(blen)
            if len(body) < blen or n == 0:
                break
            # Decode to per-frame arrays here, on the (many) reader threads,
            # so the single batcher thread only concatenates.
            feats_pad = np.full((n, MAX_FEAT), INPUTS, dtype=np.int64)
            mlens = np.empty(n, dtype=np.int64)
            moves_parts, o = [], 0
            for i in range(n):
                nf = body[o]
                nm = body[o + 1] | (body[o + 2] << 8)
                o += 3
                if nm == 0 or o + 2 * (nf + nm) > blen:
                    raise ValueError(f"malformed frame (nf={nf} nm={nm})")
                a = np.frombuffer(body, dtype="<u2", offset=o, count=nf + nm)
                feats_pad[i, : min(nf, MAX_FEAT)] = a[:MAX_FEAT if nf > MAX_FEAT else nf]
                mlens[i] = nm
                moves_parts.append(a[nf:])
                o += 2 * (nf + nm)
            q.put((conn, feats_pad, np.concatenate(moves_parts).astype(np.int64), mlens))
    except OSError:
        pass
    finally:
        # Only shut down reads here — never close: the batcher may still hold
        # this conn in a queued frame, and closing would let the OS reuse the
        # fd for a NEW connection, silently misrouting that frame's response.
        # The fd is reclaimed at process exit (the server is per-generation).
        try:
            conn.shutdown(socket.SHUT_RD)
        except OSError:
            pass


def batcher(q, net, dev, max_batch, max_wait):
    # Fail FAST and LOUD: if the forward/assembly ever raises (CUDA error, OOM,
    # malformed frame), kill the whole process so every Rust worker sees EOF and
    # self-play aborts — a dead daemon thread would instead hang all workers
    # forever in read_exact with no signal.
    try:
        batcher_loop(q, net, dev, max_batch, max_wait)
    except BaseException:
        import traceback
        traceback.print_exc()
        sys.stdout.flush(); sys.stderr.flush()
        os._exit(1)


def batcher_loop(q, net, dev, max_batch, max_wait):
    served, t0, last = 0, time.time(), time.time()
    while True:
        frames = [q.get()]
        count = len(frames[0][3])
        deadline = time.perf_counter() + max_wait
        while count < max_batch:
            left = deadline - time.perf_counter()
            if left <= 0:
                break
            try:
                fr = q.get(timeout=left)
            except queue.Empty:
                break
            frames.append(fr)
            count += len(fr[3])

        # Assemble entirely with numpy bulk ops (no per-item Python work).
        idx = np.concatenate([f[1] for f in frames])              # [B, MAX_FEAT]
        moves = np.concatenate([f[2] for f in frames])            # [total]
        mlens = np.concatenate([f[3] for f in frames])            # [B]
        B = len(mlens)
        nmax = int(mlens.max())
        starts = np.zeros(B, dtype=np.int64)
        np.cumsum(mlens[:-1], out=starts[1:])
        rows = np.repeat(np.arange(B), mlens)
        cols = np.arange(len(moves)) - np.repeat(starts, mlens)
        midx = np.zeros((B, nmax), dtype=np.int64)
        mask = np.zeros((B, nmax), dtype=bool)
        midx[rows, cols] = moves
        mask[rows, cols] = True

        with torch.no_grad():
            v, logits = net(torch.from_numpy(idx).to(dev))
            g = logits.gather(1, torch.from_numpy(midx).to(dev))
            g = g.masked_fill(~torch.from_numpy(mask).to(dev), -1e9)
            p = torch.softmax(g, dim=1)
            v = v.float().cpu().numpy().astype("<f4")
            p = p.float().cpu().numpy()
        pflat = p[rows, cols].astype("<f4")                       # request order

        oi = 0; om = 0
        for f in frames:
            k = len(f[3]); t = int(f[3].sum())
            try:
                f[0].sendall(v[oi:oi + k].tobytes() + pflat[om:om + t].tobytes())
            except OSError:
                pass  # game thread went away; its reader will clean up
            oi += k; om += t

        served += B
        now = time.time()
        if now - last >= 5:
            print(f"[{now - t0:7.1f}s] {served:,} evals ({served / (now - t0):,.0f}/s), last batch {B}",
                  flush=True)
            last = now


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--net", required=True, help=".azn weights to serve")
    ap.add_argument("--socket", default="/tmp/chess_eval.sock")
    ap.add_argument("--device", default=None)
    ap.add_argument("--max-batch", type=int, default=2048)
    ap.add_argument("--max-wait-ms", type=float, default=1.5,
                    help="how long to gather a batch after its first request")
    args = ap.parse_args()

    dev = args.device or ("cuda" if torch.cuda.is_available()
                          else "mps" if torch.backends.mps.is_available() else "cpu")
    net = Net().to(dev)
    load_azn(net, args.net, dev)
    net.eval()
    with torch.no_grad():  # warm up the full serving path before accepting work
        _, wl = net(torch.full((args.max_batch, MAX_FEAT), INPUTS, dtype=torch.int64, device=dev))
        wg = wl.gather(1, torch.zeros((args.max_batch, 64), dtype=torch.int64, device=dev))
        torch.softmax(wg.masked_fill(torch.zeros_like(wg, dtype=torch.bool), -1e9), dim=1).cpu()
    print(f"serving {args.net} on {args.socket} (device {dev}, "
          f"max batch {args.max_batch}, wait {args.max_wait_ms}ms)", flush=True)

    q = queue.Queue()
    threading.Thread(target=batcher, args=(q, net, dev, args.max_batch, args.max_wait_ms / 1e3),
                     daemon=True).start()

    if os.path.exists(args.socket):
        os.unlink(args.socket)
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(args.socket)
    srv.listen(4096)
    print("ready", flush=True)
    while True:
        conn, _ = srv.accept()
        threading.Thread(target=reader, args=(conn, q), daemon=True).start()


if __name__ == "__main__":
    main()
