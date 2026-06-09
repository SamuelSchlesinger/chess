//! A Zobrist-keyed transposition table for the search.
//!
//! Each entry caches the result of a previously searched node: its best move,
//! the score, the depth it was searched to, and whether that score is exact or
//! a bound (alpha/beta cutoff). Indexed by the low bits of [`Board::hash`]; the
//! full key is stored for collision detection. Mate scores are stored relative
//! to the node (the search adjusts by ply on store/probe).
//!
//! [`Board::hash`]: crate::Board::hash

use crate::moves::Move;

/// The kind of bound a stored score represents.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Bound {
    /// No entry.
    None = 0,
    /// Exact score (the node was a PV node, score is precise).
    Exact = 1,
    /// Lower bound (a beta cutoff: the true score is ≥ stored).
    Lower = 2,
    /// Upper bound (failed low: the true score is ≤ stored).
    Upper = 3,
}

#[derive(Clone, Copy)]
struct Entry {
    key: u64,
    mv: Move,
    score: i16,
    depth: i8,
    bound: Bound,
    age: u8,
}

impl Entry {
    const EMPTY: Entry = Entry {
        key: 0,
        mv: Move::NONE,
        score: 0,
        depth: 0,
        bound: Bound::None,
        age: 0,
    };
}

/// Data returned from a successful probe.
#[derive(Clone, Copy, Debug)]
pub struct TtData {
    pub mv: Move,
    pub score: i32,
    pub depth: i32,
    pub bound: Bound,
}

/// A fixed-size transposition table (power-of-two number of entries).
pub struct Tt {
    table: Vec<Entry>,
    mask: usize,
    age: u8,
}

impl Tt {
    /// Create a table of about `mb` mebibytes (rounded down to a power-of-two
    /// entry count, minimum 1024 entries).
    pub fn new(mb: usize) -> Tt {
        let bytes = mb.max(1) * 1024 * 1024;
        let mut entries = (bytes / std::mem::size_of::<Entry>()).max(1024);
        entries = entries.next_power_of_two();
        if entries * std::mem::size_of::<Entry>() > bytes {
            entries /= 2; // keep within the requested budget
        }
        let entries = entries.max(1024);
        Tt {
            table: vec![Entry::EMPTY; entries],
            mask: entries - 1,
            age: 0,
        }
    }

    /// Drop all entries.
    pub fn clear(&mut self) {
        for e in &mut self.table {
            *e = Entry::EMPTY;
        }
        self.age = 0;
    }

    /// Resize to about `mb` MiB, clearing the table.
    pub fn resize(&mut self, mb: usize) {
        *self = Tt::new(mb);
    }

    /// Begin a new search generation (so old entries can be preferentially
    /// overwritten).
    pub fn new_generation(&mut self) {
        self.age = self.age.wrapping_add(1);
    }

    #[inline]
    fn index(&self, key: u64) -> usize {
        key as usize & self.mask
    }

    /// Look up `key`; returns the cached data if a matching entry exists.
    #[inline]
    pub fn probe(&self, key: u64) -> Option<TtData> {
        let e = &self.table[self.index(key)];
        if e.bound != Bound::None && e.key == key {
            Some(TtData {
                mv: e.mv,
                score: e.score as i32,
                depth: e.depth as i32,
                bound: e.bound,
            })
        } else {
            None
        }
    }

    /// Store a result. Uses a depth-preferred-with-aging replacement policy:
    /// overwrite when the slot is empty/from an older search, a different
    /// position, or searched at least as deep.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn store(&mut self, key: u64, mv: Move, score: i32, depth: i32, bound: Bound) {
        let idx = self.index(key);
        let slot = &mut self.table[idx];
        let replace = slot.bound == Bound::None
            || slot.age != self.age
            || slot.key != key
            || depth as i8 >= slot.depth;
        if !replace {
            return;
        }
        // Keep a best move even on an upper-bound store that lacks one.
        let mv = if mv == Move::NONE && slot.key == key {
            slot.mv
        } else {
            mv
        };
        *slot = Entry {
            key,
            mv,
            score: score as i16,
            depth: depth as i8,
            bound,
            age: self.age,
        };
    }

    /// Approximate fill level in permille (0..1000), sampled over the first
    /// 1000 entries (UCI `hashfull`).
    pub fn hashfull(&self) -> usize {
        let n = self.table.len().min(1000);
        if n == 0 {
            return 0;
        }
        let used = self.table[..n]
            .iter()
            .filter(|e| e.bound != Bound::None && e.age == self.age)
            .count();
        used * 1000 / n
    }
}
