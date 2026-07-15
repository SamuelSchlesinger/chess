# Rust engine import

The `engine/` subtree preserves the complete history of the former local
repository at `~/projects/games/chess`.

- Imported branch: `main`
- Imported commit: `77a88d324ecbaf8d02bae2cf0afc011e427ef0ea`
- Import method: non-squashed `git subtree add --prefix=engine`
- Import date: 2026-07-14

After the history merge, the source checkout's four modified `chess-web` files
and untracked `src/bin/chess-trainer/` prototype were copied byte-for-byte into
the monorepo and validated before their first monorepo commit. Ignored training
data, neural-network files, logs, virtual environments, and build outputs were
deliberately not imported.

`engine/` is now authoritative. The old checkout is retained only as import
provenance and must not be maintained in parallel or pulled automatically.
