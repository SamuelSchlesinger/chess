# Exploratory novelty-search log

Date: 2026-07-14

Scope: exploratory web and bibliographic search for prior work on chess-opening
transposition graphs, position-keyed opening trainers, relation presentations,
and partial-order methods. This log narrows claims and records practitioner
prior art; it is not a systematic review or a priority search.

## Queries

The general web-search interface was queried with the following exact strings:

1. `chess opening repertoire trainer transpositions position based spaced repetition`
2. `academic chess opening training transposition graph repertoire`
3. `chess transposition trainer position cards opening repertoire software`
4. `site:patents.google.com chess opening transposition training`
5. `Fishburn chess transposition graph DAG DOI opening`
6. `Dynamic partial-order reduction Flanagan Godefroid DOI`
7. `chess opening graph path presentation transposition relation basis`
8. `chess opening transposition algebra trace monoid`

## Included boundary-setting results

- Fishburn's 2018 paper explicitly represents a chess opening book as a
  position DAG. It defeats any novelty claim for the opening-DAG representation.
- Chess Position Trainer's official manual and feature matrix document a
  position database, scheduled/flash-card training, and transposition handling,
  including cross-opening detection. It defeats any novelty claim for
  position-keyed transposition-aware opening study.
- GambitLab's official product page explicitly says scheduled cards are keyed
  by board position so transpositions do not duplicate work. It confirms that
  this feature remains current practitioner practice.
- Flanagan and Godefroid concern concurrent interleavings. They provide an
  analogy for state-sensitive independence but no alternating-game preservation
  theorem.

These sources are entered canonically in `../sources.md`.

## Exclusions and unresolved search space

- Product performance percentages and testimonials were excluded because they
  are not independent evidence of learning benefit.
- Forum and Reddit descriptions were excluded from the bibliography; they were
  treated only as leads to first-party documentation.
- Generic transposition-graph results unrelated to chess openings were excluded
  from the substantive novelty boundary.
- No exhaustive search of patents, theses, non-English chess literature,
  ChessBase/Bookup archives, or every historical trainer version was completed.
- No included source was found that publishes a cardinality-minimal concrete
  rooted-path basis for a named opening graph and tests explicit basis-derived
  relation/deviation prompts against a position-keyed baseline. Because the
  search is exploratory, this is a scoped absence statement, not a priority
  claim.

## Inclusion rule for future updates

Add a source when it establishes at least one of: a graph quotient for opening
study, automatic transposition-aware scheduling, an explicit algebra of chess
move-order relations, a minimal relation presentation, or controlled transfer
evidence. Record the exact version/date and distinguish a marketed feature from
validated learning efficacy.
