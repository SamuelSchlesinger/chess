# Sources for transposition algebra

These are the primary or authoritative sources used by this branch. The
relevance note states what each source supports; it is not a claim that the
source contains this project's chess-specific results.

<a id="aalbersberg88"></a>
## aalbersberg88

IJsbrand Jan Aalbersberg and Grzegorz Rozenberg, “Theory of Traces,”
*Theoretical Computer Science* 60(1), 1988, pp. 1–82.
[DOI](https://doi.org/10.1016/0304-3975(88)90051-5).

Relevance: defines the partially commutative trace framework against which the
single-ply alternation obstruction and braid proposal are compared.

<a id="butler10"></a>
## butler10

Andrew C. Butler, “Repeated Testing Produces Superior Transfer of Learning
Relative to Repeated Studying,” *Journal of Experimental Psychology: Learning,
Memory, and Cognition* 36(5), 2010, pp. 1118–1133.
[DOI](https://doi.org/10.1037/a0019902).

Relevance: motivates measuring transfer to unseen move orders rather than only
immediate rehearsal accuracy.

<a id="cepeda06"></a>
## cepeda06

Nicholas J. Cepeda, Harold Pashler, Edward Vul, John T. Wixted, and Doug
Rohrer, “Distributed Practice in Verbal Recall Tasks: A Review and Quantitative
Synthesis,” *Psychological Bulletin* 132(3), 2006, pp. 354–380.
[DOI](https://doi.org/10.1037/0033-2909.132.3.354).

Relevance: supports spaced review as a general retention intervention while
leaving its chess-specific effectiveness to experiment.

<a id="chase73"></a>
## chase73

William G. Chase and Herbert A. Simon, “Perception in Chess,” *Cognitive
Psychology* 4(1), 1973, pp. 55–81.
[DOI](https://doi.org/10.1016/0010-0285(73)90004-2).

Relevance: provides the foundational chess evidence for structured positional
chunks rather than unstructured move-string memory.

<a id="clerc15"></a>
## clerc15

Florence Clerc and Samuel Mimram, “Presenting a Category Modulo a Rewriting
System,” *26th International Conference on Rewriting Techniques and
Applications*, LIPIcs 36, 2015, pp. 89–105.
[DOI](https://doi.org/10.4230/LIPIcs.RTA.2015.89).

Relevance: supplies the established categorical setting for typed path
presentations and quotient equations.

<a id="cpt14"></a>
## cpt14

Stefan Renzewitz, *Chess Position Trainer 5 Manual*, 2014.
[Official manual](https://www.chesspositiontrainer.com/download/manuals/CPT5_manual.pdf)
and [official feature matrix](https://www.chesspositiontrainer.com/index.php/en/download)
(accessed 2026-07-14).

Relevance: documents practitioner prior art for position-database opening
study, cross-opening transposition detection, flash-card training, and
scheduled review. It narrows the player-product novelty claim to the explicit
relation/deviation intervention rather than position-keyed deduplication.

<a id="fide23"></a>
## fide23

International Chess Federation, *FIDE Laws of Chess taking effect from 1
January 2023*, approved 7 August 2022, especially Article 9.2.3.
[Official handbook](https://handbook1090.fide.com/chapter/E012023) (accessed
2026-07-14).

Relevance: fixes the authoritative position-equality fields, including
castling and effective en-passant rights.

<a id="fishburn18"></a>
## fishburn18

John P. Fishburn, “Search-Based Opening Book Construction,” *ICGA Journal*
40(1), 2018, pp. 2–14.
[DOI](https://doi.org/10.3233/ICG-180039).

Relevance: establishes that position-indexed directed acyclic opening books
already exist in computer-chess research, narrowing the novelty claim.

<a id="flanagan05"></a>
## flanagan05

Cormac Flanagan and Patrice Godefroid, “Dynamic Partial-Order Reduction for
Model Checking Software,” *ACM SIGPLAN Notices* 40(1), 2005, pp. 110–121;
presented at POPL 2005.
[DOI](https://doi.org/10.1145/1047659.1040315).

Relevance: provides the primary dynamic, state-sensitive partial-order
reduction precedent for concurrent interleavings. It motivates an analogy but
does not establish an alternating-game reduction theorem. The POPL proceedings
record is also available as DOI `10.1145/1040305.1040315`.

<a id="gambitlab26"></a>
## gambitlab26

GambitLab, “Spaced Repetition for Chess Openings,” product documentation.
[Official site](https://gambit-lab.com/) (accessed 2026-07-14).

Relevance: is current practitioner evidence that scheduled cards keyed by board
position to avoid transposition duplication are already a marketed feature. It
does not validate GambitLab's performance claims or this branch's proposed
relation exercises.

<a id="kishimoto04"></a>
## kishimoto04

Akihiro Kishimoto and Martin Müller, “A General Solution to the Graph History
Interaction Problem,” *Proceedings of AAAI 2004*, 2004, pp. 644–649.
[AAAI paper](https://s.aaai.org/Papers/AAAI/2004/AAAI04-102.pdf).

Relevance: demonstrates why merging equal game positions can still be unsound
when search values depend on path history.

<a id="knuth70"></a>
## knuth70

Donald E. Knuth and Peter B. Bendix, “Simple Word Problems in Universal
Algebras,” in *Computational Problems in Abstract Algebra*, Pergamon Press,
1970, pp. 263–297.
[DOI](https://doi.org/10.1016/B978-0-08-012975-4.50028-X).

Relevance: supplies the canonical completion method for orienting equations
toward a convergent rewriting system.

<a id="lichess-openings"></a>
## lichess-openings

Lichess contributors, *chess-openings: An Aggregated Data Set of Chess Opening
Names*, CC0; pinned locally at commit
`292fd0468068f58bb244f7fe1c3e573e493c3c53`.
[Official repository](https://github.com/lichess-org/chess-openings) (accessed
2026-07-14).

Relevance: is the authoritative upstream source and license for the exact
3,803-row corpus analyzed by the pilot.

<a id="newman42"></a>
## newman42

M. H. A. Newman, “On Theories with a Combinatorial Definition of
‘Equivalence’,” *Annals of Mathematics* 43(2), 1942, pp. 223–243.
[DOI](https://doi.org/10.2307/1968867).

Relevance: is the classical source for deriving confluence from termination
and local confluence.

<a id="vanoostrom23"></a>
## vanoostrom23

Vincent van Oostrom, “On Causal Equivalence by Tracing in String Rewriting,”
*Electronic Proceedings in Theoretical Computer Science* 377, 2023, pp. 27–43.
[DOI](https://doi.org/10.4204/EPTCS.377.2).

Relevance: relates permutation equivalence of rewrite sequences to explicit
causal representations and helps separate reordering from coalescence.

<a id="zobrist90"></a>
## zobrist90

Albert L. Zobrist, “A New Hashing Method with Application for Game Playing,”
*ICCA Journal* 13(2), 1990, pp. 69–73; reprint of University of Wisconsin
Technical Report 88, 1970.
[DOI](https://doi.org/10.3233/ICG-1990-13203).

Relevance: establishes the classical hash-based mechanism for game
transposition lookup while underscoring that hashes are not semantic IDs.
