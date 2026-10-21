# Grouped iteration: exploration record

Recorded on 2026-10-15. Covers the iteration experiments through the broad performance
snapshot of 2026-10-14, including the earlier step 5 / 5.1 and dispatch prototypes.

## Decision at the end of exploration

The exploration is finished. The remaining ordinary `next` slowdown, approximately 4–13%
in the measured primary cases, is accepted so work can proceed on completing grouping.
The ordinary `for_each` regression is also deferred. Aligned grouped bulk iteration works
and vectorizes for suitable bodies. Further optimization is not a prerequisite for finishing
the feature.

This is an explicit change of priorities, not a claim that the old no-regression gates passed.
Historical instructions to keep exploring or to block planner work until ordinary iteration
matches the original baseline no longer describe the decision. The next implementation is
to be planned cleanly on top of `8477a1c1d9aa318753268d729b8d8fe07dccf71d`, which already
contains the metadata, declaration and regroup work.

The final performance evidence concerns the segment engine with fixture-installed schedules.
It does not establish the performance or correctness of a finished automatic planner. That
selection and installation work still needs implementation and validation.

This document preserves ideas, outcomes and reasons to reconsider them. It does not require
old executables, PDBs, disassembly files, temporary worktrees or stashes. Those are not intended
as a long-term reproduction kit. Historical report names at the end are optional provenance;
the lessons needed to understand the experiments are included here.

## How to read the evidence

The experiments mostly used rustc 1.98.0 / LLVM 22.1.8, Windows x86-64 MSVC, an AMD Ryzen
9 7900X and Criterion 0.8.2. Later v2 builds used the optimized bench profile with one codegen
unit, debug information and no stripping; the CPU target remained baseline x86-64, not native.
Earlier probes used the default bench profile. Windows calling conventions, compiler inlining
choices, code placement and consumer bodies matter to these findings.

Early dispatch probes used mutable Position/Velocity loops, both with a reduction and with
independent updates. Later v2 probes mostly used wrapping `u32` multiply/add reductions and
mutable component updates. Their absolute times and speedup ratios are not interchangeable.
“Aligned” means matching entities at corresponding local positions; physical storage starts
can differ. It does not mean the allocations have a special SIMD alignment.

Three kinds of statement are kept separate:

- **Contract:** required for correct traversal or a settled feature choice.
- **Observation:** measured timing or inspected generated code on the tested build.
- **Explanation:** an interpretation consistent with those observations, sometimes supported
  by a reduced reproducer, sometimes still unisolated.

“Retained” below means used in the exploration's eventual implementation direction, not
already merged into the production implementation. “Rejected” means that particular candidate
did not justify retention on its tested base. Experiment numbers were reused: the early
dispatch E1–E6 are unrelated to v2 E1–E4.

## Vocabulary and the implementation direction that survived

A **captain** supplies the candidate entity stream and can use a local position for direct
access. A **sailor** resolves that entity through its storage lookup. A **segment** is a
contiguous candidate range with fixed access roles. **Coverage** describes storage ranges
already owned by earlier planned segments, so later walks can reject duplicates.

The final exploration engine combined these ideas:

- One typed iterator engine, with current traversal state separated from continuation ownership.
  A small ordinary/planned branch inside `next` is compatible with this choice.
- A non-owning entity cursor: next unconsumed entity pointer, end pointer and segment-local offset.
  Each component window keeps its original data pointer and a segment start offset.
- A shared immutable plan holding entity ranges, per-segment rows of node state, and coverage.
  The schedule owns the plan and the remaining follow-up range. Nodes hold their current state,
  rather than independent continuation queues.
- Each `Mixed` keeps its own immediate-child captain mask and current coverage span. Nested
  masks remain local to their node; plan centralization does not flatten their semantics.
- Bulk traversal selects checked, aligned, single-captain or partial strategy outside the
  element loop. The aligned loop performs direct component access and contains the user body.
- A cached single-captain position selects compile-time-specialized lookup for ordinary `next`.
- Fold adapters own their wrapped callback using `move`, avoiding an extra reference layer.
- Plain and WithId `next` have separate small control loops so IDs are captured explicitly.
- The final measured combination used inlinable planned-next helpers and forced inlining of
  `Mixed::captain_indices_of` and `Mixed::next_segment`. Schedule cleanup used a by-value release
  through `ManuallyDrop` to avoid exposing the active iterator to drop glue.

These last inlining and cleanup details are compiler-sensitive choices with evidence behind
them, not timeless API requirements. A clean implementation should understand why they existed.

## Contracts and feature choices worth preserving

These are independent of the timings:

1. Resolve all indices and reject covered candidates before creating component references.
   In particular, never create a duplicate mutable reference and discard it afterwards.
2. Sailor lookup is independent of active segment state and does not advance the cursor.
   Nested queries must retain that property.
3. Resolve the current candidate, capture its ID when needed, consume it exactly once, then
   create the output and invoke user code. Rejected candidates are consumed too. Bulk after
   any `next` prefix must visit exactly the remaining results.
4. Current cursors may be empty or exhausted. Queued plan segments have positive candidate
   lengths. A nonempty segment can still yield no matches. Construction omits empty ranges;
   the engine does not need a retry solely for an empty follow-up.
5. A successful segment entry installs a coherent entity source, local offsets, masks and
   coverage for participating nodes. Sailor subtrees must not consume unrelated state changes.
6. Split only the current remaining segment, and only when it has at least two candidates.
   Do not enter a later segment to make splitting possible. Left gets half and no follow-ups;
   right gets the rest and all follow-ups. Both keep the current plan's coverage alive, even
   if right is consumed or dropped first. Preserve original segment coordinates after splitting.
7. Coverage is planned ownership, not a record of how far another worker has executed.
   An empty follow-up list does not imply an absent plan or absent coverage.
8. For Or, omit empty B; start directly in B with B's state if A is empty. Both empty means
   an exhausted ordinary iterator. Preserve A precedence and duplicate suppression when both exist.
9. Groups are hints. A cheaper ordinary captain must remain eligible. Skip evaluated ungrouped
   regions known not to provide matches, while preserving matches discoverable through other
   members' unevaluated ranges. Do not manufacture fast traversal by precomputing entity joins.
10. Grouping remains automatic. Initial grouping within the same `Mixed` is sufficient;
    cross-boundary grouping is deferred. WithId, tracking, nested queries, Or, derive forwarding
    and parallel consumption remain required; reverse iteration was dropped by agreement.

Pointer-based traversal was a chosen representation, not an experimentally established universal
advantage over integer ranges. One original data pointer plus a segment offset was chosen over
an additional rebased data pointer. Any raw coverage access requires the immutable backing plan
to outlive it, including after cloning or splitting.

## Early experiments: obtaining useful segment loops

### Original step 5 and step 5.1

**Step 5:** stored segment state in `Mixed`, used offsets and exclusions, and walked multiple
members' unevaluated regions. Grouped queries improved, but ordinary dense traversal regressed
about 82% and sparse traversal about 34% against the preceding implementation in the old harness.
Disabling planning restored ordinary performance. Preventing segment entry helped only partly;
an invariant construction flag did not fix it. Inlining planner early exits helped filter queries
whose grouping eligibility was compile-time absent, not plain views with runtime metadata.

**Step 5.1:** changed to a single walker, sliced into aligned and ordinary regions, with a
construction-time comparison against ordinary iteration. It still regressed ordinary dense/sparse
traversal by about 45%/33%. Making planning always return no plan recovered most of that; hiding
the ordinary cursor behind `black_box` made it substantially slower again.

**Lesson:** a cursor that looks free only because construction proves every grouped alternative
dead is not representative of a shared runtime engine. Test opaque prepared state as well as
construction-visible consumption. The single-walker scheme also walks its evaluated ungrouped
region, contrary to the later choice to skip regions known not to yield matches. It is not the
settled planner design.

### Dispatch and loop experiments before v2

These probes shared ordinary, aligned, partial and checked access operations, so changes in
dispatch could be examined separately. None of their `next` implementations met the original
combined ordinary-parity and aligned-zip target.

| Probe | Idea and observed outcome | Lesson / reason to revisit |
| --- | --- | --- |
| Enum vs bounds dispatch (B0) | An enum selected the access kind; bounds encoded kind through ordinary/aligned/partial/checked end positions. Bounds beat the enum, whose hot loop used an indirect jump, but ordinary dense traversal remained about 29–35% slower. | Fewer dispatch instructions can help without making a flat state-machine loop equivalent to a segment loop. Revisit for different state sets or compiler lowering. |
| Early E1: retain checked indices until yield | Avoid a nested Option/output merge by returning indices and constructing data in the accepted arm. Inspected consumers compiled identically; not timed. | A source simplification need not change the generated niche check or shared yield path. |
| Early E2: resolve partial access to final pointers | Choose direct/sparse offsets once and then access prepared handles. Partial code improved, but other paths gained spills and ordinary traversal regressed. | Local instruction removal can increase pressure elsewhere in a shared consumer. Include ordinary guards. |
| Early E3: outline partial search | Search through rejected candidates in a helper, returning once per accepted item. Low-yield partial traversal improved roughly 23–30%. At high yield the call cost became substantial; a later 100%-yield independent case was roughly twice E5's cost. | Evaluate rejection rate: a call amortized over ten candidates is different from a call per item. Not retained as a general fast path. |
| Early E4: outline the whole tail | Pass transition/search state by value and return updated state plus output. The ABI still used temporary argument/result buffers, with many stores and reloads; checked yields gained roughly 11 ns of overhead. | Rust by-value syntax does not guarantee register passing. Inspect the actual ABI before repeating this design. |
| Early E5: explicit captain cases | Select `resolve_single::<CAPTAIN>` rather than testing every child's bit. Ordinary traversal improved roughly 4–16% over B0, but selection remained per candidate and original parity was not reached. | A useful specialization, later reused in v2; distinguish choosing a specialized lookup from moving the choice outside the whole loop. |
| Early C1: E5 plus partial outlining | Partial gains survived, but the dense reduction lost E5's ordinary gain. | Optimization effects are not additive; test combinations directly. |
| Early E6: one fold loop per segment | Put the consumer closure inside the selected segment loop. Aligned traversal reached zip-like code and timing; ordinary traversal improved substantially but still selected captain in the loop. | Bulk consumption supplies the loop structure that external `next` does not. |
| R1: select the ordinary captain before folding | Added a const-captain loop to E6. Large ordinary traversal matched or beat the old reference in those probes, and aligned traversal matched zip. About 13 ns of extra fixed cost remained. | This became the bulk strategy design. It did not solve flat `for`, which never invokes `fold`. |
| N1: merge inactive bounds | Merge partial/checked bounds to free a register. The compiler assigned it to pending-segment state; ordinary stack loads increased and the earlier gain disappeared. | A smaller state representation does not guarantee that the desired value receives the freed register. |
| N2: separate scan loops inside `next` | Pick captain once per call and keep rejection scanning in its own loop. Partial improved about 23–27%; dense ordinary regressed 8–29%. | On dense input, once per `next` is once per item. This is not once per segment. |
| N3 | Helper-boundary narrowing was skipped in this batch, not implemented and rejected. | Later call-boundary experiments supplied actual evidence; the skip is not a negative result. |
| T1: cold preparation | Mark the outlined preparation helper cold. Some values were preserved only around executed calls; one independent body lost a stack read. No broad parity. | Cold hints can alter allocation, but their benefit was body-specific. |
| T2: compact prepared captain | Return a one-hot captain value from preparation instead of letting LLVM reconstruct it from storage lengths. Representation changed as intended, but the initial large gain did not reproduce. | A successful representation change is not automatically a speed improvement. |
| T3e / T3c / T3b | Inline preparation, checked search, or both. Both returning boundaries really disappeared in T3b; ordinary traversal still regressed and aligned `next` stayed scalar. Some partial/checked cases improved. | “Remove all helper calls” was insufficient. Calls are not a universal explanation for the remaining gap. |

The early assertion that helper call sites force all live state into the eight Windows
callee-saved registers was too strong. Later inspection demonstrated preservation around
specific call paths. Register allocation must be read from the actual consumer.

### Testing whether flat `for` vectorization was a realistic requirement

The first two-view prototype was scalar and roughly twelve times slower than a vectorized
slice zip for the small arithmetic body. That comparison included the SIMD difference; it was
not a twelvefold regression against ordinary Shipyard iteration.

Several reductions tested the obstruction:

- Making construction visible, forcing its inlining, copying switched fields individually,
  and adding an empty inline Drop did not by themselves make the general loop vectorize.
- Deleting all transitions made even an opaque prepared single-segment iterator vectorize.
  Restoring only window switching made it scalar again.
- Direct construction plus omitted destruction, or direct construction plus an increased LLVM
  inlining threshold, could vectorize a single-segment fixture. These were diagnostics: omitted
  destruction could leak, and neither result established actual multi-segment performance.
- Outlining raw transitions or the entire transition/retry path did not make flat `for` vectorize.
- A standard-library `zip(...).chain(zip(...))` control stayed scalar under flat `for`, while
  `for_each` produced vector loops for both segments. A custom outer transition loop with an
  inner segment loop also vectorized with opaque construction.
- The tested bevy_ecs 0.20 and hecs 0.11 query `for` loops stayed scalar, even for one archetype;
  their tested bulk consumers vectorized using per-storage loops.

**Decision:** stop requiring SIMD through `next`; make eligible individual bulk segments vectorize.
This is a toolchain observation, not proof that flat iterators can never vectorize. Before reopening
that goal, first demonstrate a successful multi-segment flat-loop control on the new compiler.

## V2 experiments: state, specialization and consumption

### Initial engine, pointer cursor and empty segments

The first v2 engine had integer start/end traversal, per-node queues and specialized segment
folds. Aligned read/mutable folds reached packed arithmetic over 1, 2 and 64 segments with
unequal starts. Ordinary `next` was roughly 1.15–2 times the original baseline on large cases;
captured-accumulator read `for_each` remained scalar.

Intermediate probes changed queue contents to plain data, inlined or outlined bulk dispatch,
marked transitions cold, simplified coverage flags, and temporarily removed destruction or
coverage checks. No probe reached the old requirements. The invalid reductions were diagnostic
only. Claims in that report that a 25–40% ordinary penalty was an architectural floor were
later disproved by the smaller final gap.

Replacing start/end indices with an advancing entity pointer, end pointer, current pointer
and local offset preserved bulk behavior but initially made next about 10–20% slower in a
single-run reconstructed-baseline comparison. It established no general pointer-speed advantage.
The subsequent work reduced duplicated state and changed consumption semantics.

Separately, parallel splitting was restricted to the current segment, and Or construction stopped
queueing empty storage ranges. Later the plan constructor and fixture validator rejected empty
planned segments too. This removed the reason for an empty-follow-up retry in `next`; filtering
that produces no result still requires continued scanning. These were contract changes, not
isolated speed experiments.

### V2 E1: remove the redundant current pointer

**Idea:** keep the advancing pointer and derive the previously consumed entity as `next - 1`.
On the original pointer-cursor base, some ordinary next cases improved 7–11%, but other measured
paths regressed and the candidate was rejected under the then-strict gates.

**Revisit after single-captain specialization:** one per-candidate pointer store disappeared.
Dense, sparse and WithId next improved approximately 3–4.5%; the earlier ordinary-fold loss
did not recur. Retained at that stage.

**Final disposition:** the redundant pointer stayed removed, but `next - 1` was superseded by
explicit candidate-before-consumption semantics. Do not restore the fragile past-state contract
just to reproduce this intermediate optimization. Revisit only the general question of redundant
state if a future cursor again stores the same fact twice.

### V2 E2b: cache the single captain and specialize lookup

**Idea:** cache a captain position when exactly one child drives the segment and coverage is
empty; otherwise use general resolution. Each captain case fixes child roles at compile time.

The original broad run rejected it mainly over grouped-next losses. After priorities changed,
the revisit found dense read next about 11% faster, nested about 40% faster, mutable about 8%
faster and WithId about 9% faster than the pointer-cursor predecessor. Nested resolution lost a
per-candidate outlined call. Coverage fold stayed within the practical band. Retained.

**Lesson:** specialize roles without repeating each child's mask test. Recompute the cached state
after segment entry and child installation. This did not initially hoist captain selection out
of every next consumer; that depends on the larger loop and compiler.

### V2 E2a / E2c: cache the entire segment kind

**Idea:** cache Aligned / Unchecked / Checked, reducing repeated classification and exposing a
direct aligned-next path. The first attempt helped some nested/aligned-next cases but hurt mutable,
WithId, triples and coverage fold. It was not retained.

On the later shared-plan base, the single-captain fast path already handled ordinary queries.
The extra fallback arms grew `captain_indices_of` enough that dense and WithId consumers stopped
inlining it. WithId became about 69% slower; the nested target did not improve. Rejected again.
The proposed merged classification, E2c, was skipped, not tested.

**Revisit condition:** classification is demonstrably repeated in a new hot path, and a compact
representation avoids duplicating already-specialized cases. The later success of forced resolver
inlining was not a test of cached SegmentKind plus forced inlining; that combination remains untested.

### V2 E3: centralize the plan, then cache the current coverage span

**Idea:** replace per-node queues with one immutable shared plan. Rows describe the typed tree's
state in preorder; nodes install only their participating children's state. Current masks remain
in `Mixed`, not in a flattened global resolver.

The first attempt improved several ordinary paths but regressed coverage and WithId. On the
revisit base containing E1 and E2b, centralization improved dense next around 11% and WithId
around 8%, but coverage fold and `for_each` lost about 10%.

Inspection found repeated plan access and range checks before each coverage scan. A refinement
installed a coverage pointer/length in `Mixed` once per segment. It recovered the coverage loss
while keeping the ordinary gains. The combination was retained.

**Lesson:** centralize ownership, not every hot lookup. Resolve metadata spans at transitions.
The raw span must remain backed by a live immutable plan. The exact contribution of smaller
iterator size versus other code changes to ordinary gains was not isolated.

### V2 E4: inline aligned bulk dispatch

**Idea:** inline `fold_segment` and its aligned loop into the consumer, outline the ordinary,
partial and checked strategies. The original attempt did not improve the slow read `for_each`.
Revisiting it after shared-plan changes showed the aligned body did move into the consumer, but
still updated its captured accumulator through memory and stayed scalar. No new timing was
needed for that failed mechanism; rejected.

**Lesson:** a missing inlining boundary was not the root cause here. Inspect how the callback
environment reaches its state. Revisit inlining only if a new call boundary demonstrably blocks
useful optimization; more inline code is not intrinsically better.

### Owning callback adapters: the grouped `for_each` breakthrough

**Idea:** move the user's callback into the fold adapter instead of borrowing it. The old shape
added a second reference layer between the segment function and a captured accumulator.

```rust
// Schematic: the adapter borrows f, which itself holds &mut total.
self.fold_segments(init, |acc, id, item| f(acc, id, item))

// The adapter owns f; its captured state is one reference layer closer.
self.fold_segments(init, move |acc, id, item| f(acc, id, item))
```

A reduced reproducer distinguished callback-by-value, one mutable-reference layer, and a wrapper
borrowing another callback. The extra layer prevented promotion of the accumulator in the tested
loop. Moving the callback allowed a runtime overlap check, a packed arithmetic loop with register
accumulators, and a store after the vector reduction. No new alias promise or unsafe code was added.

Aligned read `for_each` fell from roughly 40 microseconds to 11–12 microseconds, a 70–73%
reduction, for one and 64 segments. Coverage `for_each` improved about 15%; measured fold and
ordinary-next guards stayed within band. Retained for both plain and WithId adapters.

**Lesson:** inspect callback ownership and indirection before adding custom reduction APIs or
more inlining. Preserve callback state across segments and correct drop/unwind behavior. This
fixed eligible aligned loops; it did not prove all ordinary or checked `for_each` paths optimal.

### Transition inlining and separating cursor from ownership

**Initial diagnosis:** an outlined transition received the active iterator's address. The
consumer copied the iterator into a stack object and repeatedly stored/reloaded fields. Inlining
that transition removed the call but did not remove the entry copy. Cleanup still passed an Arc
field address to drop glue; transition panics connected the loop to that cleanup.

A simple custom Drop using `Option::take` isolated the normal release but introduced an outlined
unwind drop of the iterator. This did not achieve the intended isolation and was not timed.

The separately timed transition-inline-only candidate made dense read next about 10% slower,
despite reducing some cursor traffic. The accumulator spilled on the accepted path. Mutable next
improved around 4%. It was rejected as a standalone optimization; the accumulator explanation
was supported by assembly but not isolated with counters or a reduction.

**Ownership combination:** split the non-owning `EntityCursor` from `SegmentSchedule`, inline
the transition, and release the schedule's Arc by value using an inline Drop with `ManuallyDrop`.
Splitting the fields alone did not help enough. The combined change removed the entry copy and
allowed the compiler to break up the iterator state. Dense next improved around 8% against its
predecessor, with measured bulk guards preserved. Retained.

**Cleanup simplification follow-up:** replacing that manual release with safe `Option::take`,
plain field destruction, or ordinary rather than forced transition inlining each restored the
entry copy and cursor stores in inspected consumers. None was timed or retained.

**Lesson:** cold cleanup and transitions can affect hot code even when ordinary traversal never
executes them. However, manual destruction is a compiler workaround, not a correctness requirement.
Recheck safe forms on a changed compiler or ownership layout. Preserve exactly-once destruction,
plan replacement, clone/split lifetimes and unwind safety when changing it. Do not blindly remove
bounds checks: a suggested unchecked decoder was not the demonstrated fix.

### Decode transitions separately, install inline

**Idea:** give each node a typed decoded state, perform checked decoding in an outlined function
without borrowing the active iterator, then install assignments inline. Nested child states used
Options to represent nonparticipating sailors.

The first version accidentally outlined entry and exposed the iterator again. Forcing entry
inline corrected that. Consumers became smaller and aligned bulk bodies remained identical,
but frames grew. Dense next regressed about 5%, WithId about 7%. Rejected; the slowdown's exact
cause was not isolated. The decoder never executes on the ordinary cases, so its direct call
cost cannot explain their loss.

**Revisit condition:** transition decoding has an independent maintenance or safety benefit,
or new generated code shows an actual hot-path improvement. Smaller consumer size alone was
not sufficient and the extra associated types/operations had a real complexity cost.

### Consume after resolution and capture IDs explicitly

**Idea:** make the cursor point at the next unconsumed candidate everywhere. Remove priming,
sentinel offsets and past-state ID reads. Duplicate WithId's small next loop, capture ID before
consumption, and specialize bulk ID capture with `const WITH_ID: bool` and `Option<EntityId>`.

The initial implementation preserved bulk SIMD but slowed ordinary next about 6–13% against
the ownership predecessor. Inspection found a repeated exhaustion test in safe `advance`,
lowered to a conditional pointer increment. A cold hint changed its lowering and recovered
only part of the cost.

Two diagnostics removed that repeated check: unchecked consumption, and retaining safe advance
while asserting nonexhaustion after successful plan entry. Both removed the conditional increment
and recovered most of the new read/ID cost. Mutable next was still about 6% slower at that point.
These were separate runs against a common predecessor, not paired proof that the variants were
equivalent. The later ordinary/planned branch recovered the mutable loss.

**Final disposition:** retain the simpler consumption contract. The measured final combination
used checked advance and a justified nonexhaustion assumption on the planned path. Ordinary
iteration's loop condition already supplied the fact. Later inspection found no capture-only ID
loads in the inspected `WITH_ID=false` aligned loops and no Option presence tests in the inspected
true/false aligned loops. Mutable, triple, Not and nested ID shapes were not all instantiated or audited.

**Safety lesson:** `assert_unchecked` is a safety obligation, not a harmless hint. Likewise, an
unchecked pointer advance cannot be a safe public function if exhaustion makes it invalid.
A separate diagnostic accidentally exposed safe `consume`; it was corrected to unsafe with a
documented caller precondition. The correction changed no inspected consumer instructions.
That diagnostic was not merged into the final measured combination, and no distinct performance
benefit for adding consume to that final combination was established.

### Ordinary/planned branch, then resolver inlining

**Idea:** test plan ownership at the start of next. Ordinary traversal scans only its current
segment; planned traversal may enter follow-ups. Apply the same division to WithId. Plan presence,
not follow-up count, distinguishes the modes.

**Forced-outlined planned helper:** ordinary next regressed 31–71%; aligned grouped next became
about three times slower. Passing the iterator to the helper exposed its state, and some ordinary
resolvers became outlined too. Grouped bulk stayed unchanged. Rejected. This tested the branch
plus that outlining decision, not every possible split implementation.

**Inlinable planned helper:** the compiler inlined it. Dense read improved about 7.5%, mutable and
WithId about 22% against the pre-branch cursor variant. Bulk stayed within band, but nested next
regressed about 48%, with a per-candidate resolver call remaining. Useful but incomplete.

**Forced resolver inlining:** marking `Mixed::captain_indices_of` inline-always removed the call
on the measured nested hot path. Nested and triple next improved about 35% against the inline-helper
variant; dense read improved another 5.5%. Nested also beat the pre-branch variant by about 5%
in a direct comparison. This combination became the final exploration base.

**Other follow-ups:** adding a cold hint to advance on the inline-helper variant produced identical
code and was not timed. Adding consume plus a cold ordinary-exit hint changed some timings,
including grouped next whose source path was unchanged. Those comparisons did not isolate a
consume benefit, and the nested resolver problem remained until the separate inlining change.

**Lesson:** neither “always outline the rare path” nor “never split next” follows from these
results. Inspect whether the executed resolver is inline, whether the iterator's address escapes,
and whether dispatch is actually outside the consumer loop. A branch outside next's rejection
loop is not automatically outside every call made by a user's `for` loop.

## Final performance state accepted for implementation

The final broad snapshot used the resolver-inlining combination. Ordinary comparisons used
the original production iterator at the clean implementation base. Grouped comparisons used
the earlier retained engine immediately after the callback-ownership fix. These are different
references: a grouped ratio near one means preservation of that engine, not parity with ordinary
iteration or a fresh comparison against slice zip.

| Workload | Approximate final observation |
| --- | --- |
| Ordinary dense read / mutable next | 11% / 6.5% slower than original; mutable magnitude less precise |
| Ordinary sparse / metadata fallback / unequal-start blocks next | 9.6% / 8.9% / 10.8% slower |
| Ordinary nested / triple / WithId next | 13% / 5.8% / 3.7% slower |
| Ordinary Not next | 12% faster |
| Ordinary dense fold / for_each | Fold within the practical band; for_each 14.5% slower |
| Aligned grouped read/mutable fold and for_each | Within the 3% practical band of the grouped reference; one mutable-fold outlier prompted one follow-up, which was within band |
| Grouped coverage fold / for_each | Fold about 4% faster; for_each about 2.9% slower, with blocks on either side of the band boundary |
| Partial-checked triple fold | About 3.3% slower than grouped reference |
| Planned single ordinary-access segment fold | About 16% faster than grouped reference |
| Grouped aligned / coverage next | Aligned within band; coverage about 13% slower |
| Empty owned construction + fold + cleanup | About 10 ns slower; not a pure constructor measurement |
| Small dense next, 100 candidates | About 15 ns slower; launch spread around 5% |

Eight of nine primary ordinary-next cases regressed, so the remaining cost is broad. The
accepted decision is to proceed with those tradeoffs, not to reinterpret them as noise.
It is not a blanket allowance for additional regressions during feature implementation.

The snapshot combined 23 new pairs with two reused exact-candidate dense pairs. New timing,
including one follow-up, took under ten minutes. Most late comparisons used only two balanced
blocks. They give useful direction and scale, not precise confidence or proof of equivalence.

Two explanations in the broad report need qualification for future readers:

- Per-candidate cursor stores and an `is_covered` call are not a demonstrated common cause of
  the final ordinary-next gap. The final flat-pair single-captain loops keep cursor advancement
  in registers and bypass coverage; nested paths differ. Whole-function stack/call totals include
  paths a fixture does not execute. The final gap's full cause remains unresolved.
- Level fold timing does not prove that the ordinary for_each loss is only in its outer wrapper.
  They call different segment-function instantiations. The ordinary for_each specialization
  still has per-item accumulator memory updates and cursor writes in inspected code. That is
  a lead for future investigation, not an isolated explanation of the 14.5% loss.

Not covered by the final timing map: every size and captain order, ordinary mutable bulk,
grouped mutable next, nested WithId, tracking and Or timing, all ID bulk shapes, automatic
planner selection/installation, and fresh slice-control comparisons. Earlier correctness and
static evidence covered some of these, but is not timing evidence.

## Measurement experiments and lessons

**Code placement was a measured effect.** An early padding experiment moved normalized-identical
consumer instruction streams to different addresses. Some aligned timings changed by up to about
45%, associated with placement modulo 64. Unmoved consumers did not show the same changes, and
changing only the executable's path did not reproduce them. The microarchitectural cause was
not identified. Padding was diagnostic, never a retained optimization.

**Process variation was also real.** The same executable sometimes landed at distinct timing
levels. A later A/A campaign found 31 of 32 case/executable comparisons within its band, but a
mutable aligned for_each case varied about 16% between launches despite narrow within-launch
Criterion intervals. Stable unrelated slice controls did not certify stability of every ECS loop.
Unchanged source in an unrelated path does not prove unchanged machine code or placement.

**More benchmarking became counterproductive.** Broad calibration and confirmation took over an
hour. The process changed to static inspection first, short balanced candidate/reference launches,
small declared case sets, a fixed timing budget and at most a justified targeted follow-up. A
failed codegen prediction could end an experiment without timing. Repeating an obvious loss was
not useful. The historical 3% band was a practical tolerance, not a universal noise estimate or
a statistically established equivalence threshold.

For future work:

- Rebuild a contemporary baseline from the accepted implementation and use matching fixtures,
  bodies, profiles, dependencies and target features. These old timings are historical context.
- Inspect the timed executable's actual consumer and relevant callees. Diagnostic IR builds
  must use the same profile; `--release` and `--profile bench` were not interchangeable here.
- Map the workload to the executed captain/strategy arm. Smaller functions, fewer whole-function
  stack references or fewer total calls do not measure dynamic work. Include outlined callees.
- Count packed component arithmetic to establish vectorization, not vector instructions used
  only for copying state. Check remainders and meaningful read/mutable bodies.
- Keep setup, installation, traversal and cleanup timing boundaries explicit. Construction-only
  black-boxing can force materialization absent from a consumed iterator. Do not subtract timer
  floors or separate construction measurements from traversal estimates.
- Validate visited candidates and yielded entities independently. Partial plans may walk far
  more candidates than a shorter ordinary captain; their ratio is not pure dispatcher overhead.
- Pair variants directly. Do not multiply ratios from separate predecessor runs into a claimed
  final speedup. Preserve contradictory launches instead of rerunning until a result passes.
- Keep correctness checks broad even when timing coverage is small: held mutable outputs,
  generation checks, zero-sized/tracked components, IDs, prefix-to-bulk, nested state, Or,
  split order/drop order, callback state and unwind behavior.

An earlier cache-evicted Vec experiment compared equal and staggered starts across two/five
64-byte element arrays. Results varied by run order and did not establish a universal advantage
for equal physical starts. It used a different memory-bound body and is not evidence about ECS
dispatch or the later arithmetic SIMD loops.

## What remains worth revisiting later

These are recorded opportunities, not the next implementation work queue:

- Ordinary next's residual 4–13% gap, starting from actual executed loops against a fresh baseline.
- Ordinary and checked for_each accumulator behavior; preserve the successful aligned adapter shape.
- Safe schedule cleanup if a new compiler no longer materializes the active iterator for it.
- Dispatch and resolver inlining when new query forms change size or compiler thresholds.
- Typed transition decoding if maintainability justifies it and the previous performance loss disappears.
- Grouped coverage next and borderline partial/checked bulk costs after the primary feature is complete.
- Flat-next SIMD only after a convincing multi-segment control succeeds on the future toolchain.

Separate ordinary/grouped public iterator types, a by-value planned-next helper in the final
engine, cached-kind plus forced resolver inlining, and grouping across nested Mixed boundaries
were not validated by this exploration. Do not list them as failed experiments. Conversely, the
old no-drop and no-coverage reductions were intentionally incomplete diagnostics, not candidates
whose correctness should be reconstructed from their benchmark checksums.

## Optional provenance

This record synthesizes the following reports. Their historical gates, retained-base instructions
and artifact paths describe their own dates; the decision at the top of this document supersedes
those instructions. Reading or restoring their artifacts is not necessary to use this record.

- Earlier stash reports: `grouping-step-5-benchmark-results.md`,
  `grouping-step-5.1-benchmark-results.md`, `grouping-dispatch-experiment-results.md`,
  `grouping-loop-optimization-results.md`, `grouping-loop-isolation-results.md`,
  `grouping-call-boundary-results.md`, and `grouping-iteration-benchmark-results.md`.
- [Prototype benchmark](grouping-test-iterator-benchmark-results.md),
  [codegen reductions](grouping-iteration-codegen-diagnosis.md),
  [segment controls](grouping-iteration-segment-split.md),
  [ECS controls](grouping-iteration-ecs-control.md).
- [Baseline](grouping-v2-step-0-results.md), [initial engine](grouping-v2-step-1-results.md),
  [pointer cursor](grouping-v2-pointer-cursor-results.md),
  [original optimization batch](grouping-v2-optimization-results.md),
  [measurement calibration](grouping-v2-reliable-baseline-results.md).
- Revisits: [E1](grouping-v2-revisit-e1-results.md), [E2a/E2c](grouping-v2-revisit-e2a-results.md),
  [E2b](grouping-v2-revisit-e2b-results.md), [E3](grouping-v2-revisit-e3-results.md),
  [E4](grouping-v2-revisit-e4-results.md).
- [Ordinary-gap diagnosis](grouping-v2-ordinary-next-gap-results.md),
  [callback ownership](grouping-v2-for-each-vectorization-results.md),
  [transition inlining](grouping-v2-next-transition-inlining-results.md),
  [state ownership](grouping-v2-next-state-ownership-results.md),
  [cleanup isolation](grouping-v2-next-cleanup-isolation-results.md),
  [transition decoding](grouping-v2-next-transition-decoding-results.md).
- [Cursor consumption](grouping-v2-cursor-consumption-results.md),
  [ordinary/planned branch and follow-ups](grouping-v2-next-plan-branch-results.md),
  [resolver inlining](grouping-v2-next-resolver-inlining-results.md),
  [consume safety](grouping-v2-consume-safety-results.md),
  [final broad snapshot](grouping-v2-broad-performance-snapshot-results.md).
- The earlier [engine-validation result](grouping-v2-engine-validation-results.md) was a deferral,
  not another benchmark or negative experiment. The later broad snapshot supplied the performance
  map used to close exploration.
