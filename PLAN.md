# RakuYomi Controlled Rebuild Plan

- Status: Opening step 1, fork-governance documents
- Active gate: Gate 6, branch and PR publication authorized
- Last verified: 2026-07-31
- Active change: `codex/fork-governance-docs`
- Publication authority: Branch push and ready fork PR granted; merge and release not granted

This file is the operational source of truth for the controlled rebuild. The
durable requirements and policies are in [SPEC.md](SPEC.md). Keep detailed
feature behavior in separately approved feature specifications.

## 1. Current baseline

| Item | Verified state |
|---|---|
| Upstream | `tachibana-shin/rakuyomi` |
| Upstream branch | `main` at `0ef01d0bab2ab90a436f4884fd3192f821d4a996` |
| Upstream release | [`v1.39.6`](https://github.com/tachibana-shin/rakuyomi/releases/tag/v1.39.6) at the same commit |
| Fork | `kravenos/krakuyomi` |
| Clean rebuild branch | `codex/upstream-rebuild` at `0ef01d0bab2ab90a436f4884fd3192f821d4a996` |
| Fork default branch | Old fork lineage `main` at `c2a254e8c12761045a733d56fccf897209922c13` |
| Historical archive | `codex/archive-pre-upstream-rebuild-2026-07-31` at `44794ff8112ae3d40bded3fea0cbd9175434d72a` |
| Fork releases | None |
| Fork tags | 116 upstream-matching tags; no fork-only or mismatched tags |
| Upstream tags absent from fork | `v1.33.0` and `v1.39.1` through `v1.39.6` |
| Rebuild-branch CI | No independent fork run yet |

The upstream release commit itself has no source-code Build run. Its code parent,
`443652e1bb717ea546041497dff53531489537c0`, passed upstream CI and Build on
2026-07-31. The untouched fork baseline still requires its own qualification and
Kindle smoke test.

If live upstream advances, report the drift and request a baseline decision. Do
not silently move the pinned rebuild branch.

## 2. Active program position

Exactly one change may be active in Gates 1 through 5.

| Field | Current value |
|---|---|
| Active outcome | Add fork-only root governance documents |
| Classification | Fork governance |
| Gate 1 | Approved |
| Gate 2 | Approved |
| Gate 3 | Approved |
| Gate 4 | Approved |
| Gate 5 | Approved as not applicable; documentation-only exact diff |
| Gate 6 | Branch push and ready fork PR approved; merge pending |
| Branch base | `0ef01d0bab2ab90a436f4884fd3192f821d4a996` |
| Intended fork PR target | `codex/upstream-rebuild` |
| Intended upstream target | None |

## 3. Opening program sequence

Do not combine these steps in one branch or PR.

| Order | Outcome | State | Dependency |
|---:|---|---|---|
| 1 | Add fork-only `PLAN.md` and `SPEC.md` | Active, Gate 6 | Approved Gates 1-5 |
| 2 | Qualify untouched upstream baseline and run Kindle smoke test | Pending | Step 1 published |
| 3 | Freeze automatic fork releases while preserving CI artifacts | Pending | Step 2 qualified |
| 4 | Promote the qualified clean lineage to default `main` | Pending | Step 3 verified |
| 5 | Remove only approved unnecessary repository material | Pending | Step 4 completed and recovery archive verified |
| 6 | Rebuild one isolated feature at a time | Pending | Steps 1-5 completed |

## 4. Ordered feature backlog

The order is safety-first. Classification is provisional until each feature's
Gate 2. Re-rank this table after every successful device-tested feature build.
Do not re-rank merely because a branch was opened or CI passed.

| Priority | Outcome | Default classification | State | Rationale or prerequisite |
|---:|---|---|---|---|
| 1 | Atomic settings writes | Upstream candidate | Queued | Prevent interrupted saves from truncating configuration |
| 2 | Corrupt-settings recovery and last-known-good backup | Upstream candidate | Queued | Recover source lists and preserve corrupt evidence |
| 3 | Nil and unknown Settings value safety | Upstream candidate | Queued | Keep the complete Settings screen usable with old or missing values |
| 4 | Remove the source package and metadata sidecar together | Upstream candidate | Queued | Make installed-source state truthful |
| 5 | Keep missing-source library entries visible | Upstream candidate | Queued | Preserve visible access to user-owned membership and progress |
| 6 | Warn before uninstall with affected manga count | Upstream candidate | Queued | Make orphaning consequences explicit before action |
| 7 | Preserve and aggregate source errors honestly | Upstream candidate | Queued | Replace repeated opaque failures with actionable source summaries |
| 8 | Hide fully read manga | Upstream candidate | Queued | Restore approved library filtering without altering membership |
| 9 | Reading direction | Upstream candidate | Queued | Restore one independently reviewable reader preference |
| 10 | Page-turn style | Upstream candidate | Queued | Restore one independently reviewable reader preference |
| 11 | Back to library | Upstream candidate | Queued | Restore one independently reviewable navigation outcome |
| 12 | Configurable library tile metadata | Upstream candidate | Queued | Restore useful counts without N+1 database work |
| 13 | Deterministic source provenance and catalog selection | Upstream candidate | Queued | Make provider and version choice exact and reproducible |
| 14 | Source update visibility and source-list management | Upstream candidate | Queued | Make stale and unavailable dependencies recoverable on-device |
| 15 | Diagnostics and source-aware search improvements | Upstream candidate | Queued | Diagnose bounded failures and preserve source identity in search |
| 16 | Optional UX after reassessment | Decide at Gate 2 | Deferred | Includes manga-tap flow, storage title, collection covers, and long-title wrapping |

Translations and tests remain with the outcome they support. Reading direction,
page-turn style, and Back to library remain separate branches and PRs.

## 5. Branch and PR map

| Branch or PR | Base or target | Purpose | State | Publication constraint |
|---|---|---|---|---|
| `codex/fork-governance-docs` | Base `0ef01d0`; target `codex/upstream-rebuild` | Root fork governance | Publication approved; checks pending | Merge not approved |
| `codex/upstream-rebuild` | Exact upstream `v1.39.6` | Clean rebuild lineage | Remote, unqualified | Do not promote to `main` yet |
| `codex/archive-pre-upstream-rebuild-2026-07-31` | Archive `44794ff` | Historical recovery point | Preserved locally and remotely | Never merge wholesale |
| [Fork PR #2](https://github.com/kravenos/krakuyomi/pull/2), `test/bridge-url-fix` to old `main` | Old fork lineage | Legacy test/fix work | Open | Disposition before changing default `main` |

No upstream candidate is active or awaiting review.

## 6. Verification evidence

| Evidence | Commit or artifact | Result | Notes |
|---|---|---|---|
| Upstream CI | `443652e1bb717ea546041497dff53531489537c0` | Passed | [Upstream run 30637032464](https://github.com/tachibana-shin/rakuyomi/actions/runs/30637032464) |
| Upstream Build | `443652e1bb717ea546041497dff53531489537c0` | Passed | [Upstream run 30637032762](https://github.com/tachibana-shin/rakuyomi/actions/runs/30637032762) |
| Rebuild ref equality | `0ef01d0bab2ab90a436f4884fd3192f821d4a996` | Passed | Local, fork rebuild branch, upstream `main`, and `v1.39.6` matched at Gate 4 preflight |
| Archive availability | `44794ff8112ae3d40bded3fea0cbd9175434d72a` | Passed | Local and remote archive refs matched |
| Governance-documents exact diff | `codex/fork-governance-docs` | Passed | Contains only `PLAN.md` and `SPEC.md` |
| Governance-documents content, whitespace, and link review | `codex/fork-governance-docs` | Passed | Required sections, backlog sequence, relative links, and immutable archive links verified |

### Kindle and artifact evidence

| Change | KindleHF commit and artifact | Result | Reason or observations |
|---|---|---|---|
| Fork governance documents | Not applicable | Approved at Gate 5 | Exact diff is Markdown only; no runtime, build, packaging, data, or release effect |
| Untouched baseline qualification | Not built | Not started | Opening step 2 supplies the first fork baseline artifact and smoke test |

No successful Kindle-validated feature build exists on the clean lineage yet, so
the feature backlog has not been re-ranked.

## 7. Upstream status

| Work | Status |
|---|---|
| Upstream PRs #256-#259 | Accepted and supplied by the clean baseline |
| New upstream feature PRs | None |
| Requested upstream revisions | None |
| Fork governance documents | Intentionally never upstreamed |

## 8. Blockers and residual risks

1. The current Build workflow can publish automatically from `main`; release
   control must land before default-branch promotion.
2. The fork lacks upstream tags `v1.39.1` through `v1.39.6`, so release-version
   baseline behavior must be defined and tested.
3. Fork `main` is still the old lineage; its promotion requires an explicit,
   separately reviewed operation.
4. The clean rebuild branch has not passed independent fork CI or a Kindle smoke
   test.
5. Legacy fork PR #2 still targets `main` and may become misleading when that ref
   changes.
6. Ignored recovery material under `dist/` includes unique databases, source
   packages, settings, mappings, and poster repairs. It is not approved for
   deletion and has not yet been verified in an external archive.
7. The archived source-management specification predates upstream 1.39.6 WASM
   error changes; error-path claims require fresh discovery.
8. Two tracked workflow filenames ending in `%` are cleanup candidates only;
   their purpose and references have not yet been proven.

## 9. Decisions awaiting approval

- Whether that PR may merge after its automatically triggered checks and review;
  merge authority remains separate from PR-publication authority.
- The release-freeze trigger and fork tag-baseline design in opening step 3.
- The disposition of legacy PR #2 before default-branch promotion.
- The external archive location and verification method for unique recovery
  material before cleanup.

Feature-specific decisions remain deferred to each feature's Gate 2.

## 10. Recommended next action

Publish the approved ready fork PR from `codex/fork-governance-docs` to
`codex/upstream-rebuild`, then verify its checks and review. Present that evidence
for separate merge approval. Do not change `main`, release, or delete remote
state.

## 11. Historical references

The immutable rebuild inventory, source-management discovery specification,
incident report, historical plan, and recovery tool with regression tests are
linked from
[SPEC.md section 12](SPEC.md#12-detailed-specifications-and-historical-evidence).
They are evidence, not the current plan.
