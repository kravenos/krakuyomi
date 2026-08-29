# RakuYomi Fork Rebuild Specification

- Status: Accepted governance specification
- Owner and approval authority: Corvin
- Scope: Fork-only; never include this file in an upstream-facing feature diff
- Accepted: 2026-07-31

Change this specification only when an approved requirement or durable decision
changes. Current status, priorities, branches, evidence, and next actions belong
in [PLAN.md](PLAN.md).

## 1. Mission and intended user

Rebuild the RakuYomi fork deliberately from a clean, pinned upstream baseline.
Preserve user data and all upstream-supported platforms while adding only useful,
verified behavior through small, isolated changes.

Corvin is the product owner, approval authority, and primary Kindle device
tester. The primary device is a Kindle Paperwhite 12 SE using the `kindlehf`
build. Decisions must also preserve users of every other supported target.

The rebuild succeeds through evidence, not feature count. Every retained outcome
must earn its place through the six gates in this document.

## 2. Architecture and supported platforms

RakuYomi consists of:

- a LuaJIT 5.1-compatible KOReader frontend;
- a Rust 1.97.1 Axum HTTP backend;
- SQLite persistence;
- Aidoku/WASM, LNReader, MangaYomi, and Keiyoushi manga sources;
- a JSON boundary between the Lua frontend and Rust backend.

Preserve these upstream architectures and targets:

- generic Unix devices using a server process, Unix domain socket, and HTTP
  proxy;
- Kindle packages: `kindle`, `kindlehf`, and `kindlea9`;
- `aarch64` and desktop Linux packages;
- Android using the JNI companion application and local TCP bridge;
- Linux bridge mode using a user service and local TCP connection;
- macOS.

A feature may touch only the platforms needed for its outcome, but it must not
silently remove, disable, or regress another supported platform.

## 3. Sources of truth

- `SPEC.md` records durable requirements, policies, and accepted dispositions.
- `PLAN.md` records the current baseline, gate, backlog, branches, evidence,
  blockers, risks, and next action.
- A detailed feature specification records feature-level behavior and contracts.
- Immutable incident and rebuild records supply historical evidence; they do not
  override current verified state.
- The cross-project portfolio backlog records only concise project status and the
  next action. It does not duplicate the project backlog.

There is no separate project `BACKLOG.md`. The ordered project backlog belongs in
`PLAN.md`. Split it into a separate file only after an approved decision that it
has become too large to manage there.

## 4. Data-preservation invariants

Treat the following as irreplaceable until a verified copy proves otherwise:

- library membership;
- reading progress and chapter state;
- settings and source-list configuration;
- source packages and provenance metadata;
- downloads;
- poster cache;
- repaired MangaFire database, mappings, and recovery evidence.

Always:

- test migrations and recovery using copies before touching live data;
- verify backups, checksums, SQLite integrity, and restoration;
- handle SQLite WAL and SHM state explicitly;
- preserve rollback material until post-change verification succeeds;
- distinguish offline recovery from successful on-device validation.

Never:

- use the live device data as the first test subject;
- silently delete or hide user rows because a replaceable source is missing;
- infer that a copied SQLite main file contains committed WAL state;
- guess canonical identifiers using string replacement;
- enable global missing-migration relaxation without approved evidence that it is
  necessary and safer than a one-time compatibility bridge.

## 5. Six mandatory gates

Every program phase and feature stops at each applicable gate for Corvin's
approval.

### Gate 1: Discovery

Work read-only. Verify current upstream, prove the problem still exists, inspect
the affected flow and data, classify the change, and surface contradictions.

### Gate 2: Specification

Define objective, scope, non-goals, preserved behavior, failure behavior,
compatibility, data effects, acceptance criteria, automated and Kindle checks,
performance relevance, rollout, rollback, branch base, PR target, dependencies,
and expected upstream value.

### Gate 3: Implementation authorization

Present the exact pinned base, branch, increments, expected files and interfaces,
test order, commit plan, commands, risks, and rollback point. Do not alter tracked
project files before approval.

### Gate 4: Implementation and verification

Implement only the approved scope. Run proportionate automated, data-safety,
security, performance, simplification, regression, diff, and history checks.
Present evidence before any validation-only push.

### Gate 5: Kindle validation

For every runtime-affecting change, test a KindleHF artifact built from the exact
reviewed commit. Record its name, checksum, size delta, backup readiness, rollback
readiness, and Corvin's observed results. CI cannot replace this gate.

Documentation-only or proven non-runtime cleanup may mark this gate not
applicable only with a written reason and an exact-diff review.

### Gate 6: Publication

Present the final commits, diff, automated evidence, Kindle evidence or justified
non-applicability, performance evidence, residual risks, PR text, target, and
merge or release effects. Require explicit approval before pushing beyond
validation infrastructure, opening an upstream PR, merging, changing default
branches, releasing, deleting remote state, or otherwise publishing.

After disposition, update the fork plan, re-rank the backlog after each successful
device-tested feature build, recommend one next feature, and wait before starting
its Gate 1.

## 6. Branch and upstream policy

One branch and one PR deliver one coherent user-visible or operational outcome.
Declare prerequisites; never merge a sibling feature branch to obtain them.

Generally useful changes are upstream candidates by default:

- start from a freshly fetched and approved upstream commit;
- exclude fork governance, cleanup, and unrelated features;
- device-test the exact branch;
- offer it to Tachibana only after Gate 6 approval;
- if accepted, take the result back from upstream instead of retaining a fork
  duplicate;
- if rejected or stalled, stop for an explicit fork-only decision.

Personalized and fork-operational changes are classified fork-only at Gate 2 and
target the fork lineage. Root governance documents and repository cleanup are
always fork-only.

Only one feature may be active in Gates 1 through 5. Submitted upstream PRs may
await review, but requested upstream revisions take priority unless explicitly
deferred.

An upstream-candidate branch must not modify `PLAN.md` or `SPEC.md`. Record its
status afterward through a separate fork-only governance update so the upstream
diff remains clean.

## 7. Lean-engineering contract

- Use the smallest coherent diff that satisfies approved acceptance criteria.
- Follow existing Rust, Lua, JSON, SQLite, and KOReader patterns.
- Preserve the Rust/Lua HTTP boundary.
- Add no dependency, schema change, public interface, or speculative abstraction
  without approval.
- Do not perform opportunistic refactors.
- Permit necessary refactoring only when Gate 2 explains why it is required.
- Avoid N+1 database or network work, unbounded concurrency, interactive per-file
  scans, and repeated eMMC I/O.
- Measure changed hot paths using representative fixtures.
- Explain and obtain approval for a median regression greater than 10 percent.
- Report material KindleHF artifact-size growth.
- Finish every implementation with a simplification and scope pass.
- Prefer correctness, data preservation, and readable code over cleverness.

## 8. Clean-history contract

- Use small, single-purpose development commits.
- Prefer one polished conventional commit for a small feature.
- Keep multiple commits only when each is independently useful to reviewers.
- Remove temporary fixup noise before publication.
- Do not combine formatting, refactoring, cleanup, or unrelated fixes with a
  feature.
- Do not force-push after review begins without explaining why.
- Before upstream publication, rebase onto the approved current upstream commit
  and rerun affected checks.
- The final upstream diff may contain only the feature, its tests, required
  documentation, and translations.

## 9. Release policy

No public release may precede automated verification and required Kindle
validation. Release approval is a separate Gate 6 decision; merge approval is not
release approval.

Before promoting the clean lineage to the fork's default `main`:

- land a separate fork-only release-control change;
- preserve CI and downloadable build artifacts;
- prevent ordinary pushes and merges from publishing a release;
- require an explicit publication action after Gate 6 approval;
- verify tag and version-baseline behavior in the fork;
- test the frozen workflow before changing the default branch.

Old fork release commits, tags, and changelog lineage must not be replayed into the
clean rebuild.

## 10. Repository-cleanup policy

Cleanup is evidence-based and receives its own fork-only PR with no runtime
feature.

Classify every candidate as required, fork governance, generated and rebuildable,
recovery artifact, misplaced, duplicated, proven unused, or unknown and retained.

Before removal:

- trace references through code, builds, CI, documentation, scripts, and history;
- preserve all upstream platforms and functioning capabilities;
- archive unique recovery material outside the repository and verify the archive;
- separate reproducible-cache deletion from tracked cleanup;
- produce a removal matrix and obtain approval.

Never delete an unfamiliar item based only on its name. Preserve Git history and
the remote archive.

## 11. Feature inventory and dispositions

Dispositions describe rebuild intent, not implementation authorization. Every
outcome still requires current-upstream discovery and all applicable gates.

### Supplied by the upstream baseline

Do not rebuild these fork-originated outcomes unless current discovery proves a
regression:

| Outcome | Upstream evidence | Disposition |
|---|---|---|
| Settings and error-handling robustness | PR #256, released in 1.39.0 | Use upstream |
| Storage statistics endpoint and Settings row | PR #257, released in 1.39.0 | Use upstream |
| Optional automatic deletion of downloaded chapters | PR #258, released in 1.39.0 | Use upstream |
| Clearer library sorting labels | PR #259, released in 1.39.0 | Use upstream |

### Previously implemented fork-only outcomes

| Rebuild outcome | Historical evidence | Disposition | Default route |
|---|---|---|---|
| Atomic settings writes | F-01 | Keep, isolated | Upstream candidate |
| Corrupt-settings recovery and last-known-good backup | F-01 | Keep, isolated | Upstream candidate |
| Nil and unknown Settings value safety | F-02 | Supplied by v1.41.4 | Use upstream |
| Configurable library tile metadata | F-03 | Keep | Upstream candidate |
| Manga-tap and chapter-list choice | F-04 | Reassess on device | Undecided |
| Hide fully read manga | F-05 | Keep | Upstream candidate |
| Reading direction | F-06 | Keep, isolated | Upstream candidate |
| Page-turn style | F-06 | Keep, isolated | Upstream candidate |
| Back to library | F-06 | Keep, isolated | Upstream candidate |
| Total downloaded size as library title | F-07 | Reassess on device | Undecided |
| Compatibility for historical missing migrations | F-08 | Conditional; prefer a one-time bridge | Fork compatibility only |
| Manual Build workflow trigger | F-09 | Supplied with fork release control | Fork operational |
| Localization updates | F-10 | Derived; keep with each retained UI feature | Same as feature |
| Old fork releases and changelog entries | F-11 | Do not port | None |

### Current source-management outcomes

The accepted current contract is [SOURCE-MANAGEMENT-SPEC.md](SOURCE-MANAGEMENT-SPEC.md).
Upstream v1.41.4 already supplies broad package/sidecar removal and a basic
source-list screen. The remaining outcomes must be delivered in the isolated
order defined by that specification:

- load source files independently and expose truthful local inventory;
- keep missing-source library entries visible;
- warn before uninstall with the affected manga count;
- cache validated catalogs and select exact provenance deterministically;
- preserve, classify, aggregate, and bound source failure evidence;
- expose complete source/list management and included-source search;
- add bounded read-only diagnosis;
- consider collection cover tiles, long-title wrapping, and other optional UX
  only after higher-priority safety work.

## 12. Detailed specifications and historical evidence

Use these immutable archive records as evidence:

- [fork rebuild inventory](https://github.com/kravenos/krakuyomi/blob/44794ff8112ae3d40bded3fea0cbd9175434d72a/fork-rebuild-spec.md);
- [source-management discovery specification](https://github.com/kravenos/krakuyomi/blob/44794ff8112ae3d40bded3fea0cbd9175434d72a/source-management-spec.md);
- [2026-07-29 incident report](https://github.com/kravenos/krakuyomi/blob/44794ff8112ae3d40bded3fea0cbd9175434d72a/incident-report.md);
- [historical working plan](https://github.com/kravenos/krakuyomi/blob/44794ff8112ae3d40bded3fea0cbd9175434d72a/plan.md);
- [recovery migration tool](https://github.com/kravenos/krakuyomi/blob/44794ff8112ae3d40bded3fea0cbd9175434d72a/scripts/migrate-mangafire-ids.py);
- [recovery migration regression tests](https://github.com/kravenos/krakuyomi/blob/44794ff8112ae3d40bded3fea0cbd9175434d72a/scripts/tests/test_migrate_mangafire_ids.py).

The accepted current-baseline specification is
[SOURCE-MANAGEMENT-SPEC.md](SOURCE-MANAGEMENT-SPEC.md). It records the v1.41.4
audit and supersedes the archived draft's unresolved API and policy choices.

## 13. Success criteria

The controlled rebuild is successful when:

- the exact upstream baseline is recorded and qualified before fork behavior is
  added;
- existing Kindle data has verified backups and a rehearsed rollback path;
- every retained outcome has its own approved scope, branch, evidence, and
  disposition;
- all affected automated checks pass;
- every runtime change passes Kindle validation from the exact reviewed commit;
- upstream candidates contain no fork-only material and are reviewable in
  isolation;
- the fork cannot publish accidentally from an ordinary merge or push;
- cleanup removes only approved, proven candidates after recovery material is
  secured;
- `PLAN.md` names one active feature and one next action;
- the backlog is re-ranked after every successful device-tested feature build.

## 14. Decision boundaries

Always follow the invariants, gates, branch isolation, verification, and evidence
requirements in this document.

Ask first before changing feature dispositions, schemas, migrations,
dependencies, public interfaces, compatibility policy, CI or release behavior,
supported platforms, data handling, or this specification.

Never merge the old fork wholesale, count planned work as implemented, mix
features, publish without Gate 6 approval, discard the only recovery copy, or
silently reinterpret contradictory evidence.
