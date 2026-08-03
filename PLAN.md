# RakuYomi Controlled Rebuild Plan

- Status: Opening step 2, upstream baseline qualification
- Active gate: Gate 6, ready fork PR #4 open; merge separately gated
- Last verified: 2026-08-03
- Active change: `codex/qualify-upstream-baseline`
- Publication authority: Branch push and ready fork PR completed; no merge, default-branch change, release, cleanup, or remote deletion granted

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
| Clean rebuild branch | `codex/upstream-rebuild` at `a0efffbd430d72575c44773dc0d7feec0c726f49`; only `PLAN.md` and `SPEC.md` differ from upstream `v1.39.6` |
| Fork default branch | Old fork lineage `main` at `c2a254e8c12761045a733d56fccf897209922c13` |
| Historical archive | `codex/archive-pre-upstream-rebuild-2026-07-31` at `44794ff8112ae3d40bded3fea0cbd9175434d72a` |
| Fork releases | None |
| Fork tags | 116 upstream-matching tags; no fork-only or mismatched tags |
| Upstream tags absent from fork | `v1.33.0` and `v1.39.1` through `v1.39.6` |
| Rebuild-branch CI | Fork PR #3 Build and CI passed on the runtime-equivalent governance tree; its temporary-merge artifact is not the device candidate |

The upstream release commit itself has no source-code Build run. Its code parent,
`443652e1bb717ea546041497dff53531489537c0`, passed upstream CI and Build on
2026-07-31, and `0ef01d0` adds only `CHANGELOG.md`. The official v1.39.6
KindleHF release asset passed checksum, package, backup, rollback, and on-device
validation and is the qualified baseline candidate for this rebuild.

If live upstream advances, report the drift and request a baseline decision. Do
not silently move the pinned rebuild branch.

## 2. Active program position

Exactly one change may be active in Gates 1 through 5.

| Field | Current value |
|---|---|
| Active outcome | Qualify the untouched upstream v1.39.6 baseline on KindleHF |
| Classification | Fork-operational validation |
| Gate 1 | Approved |
| Gate 2 | Approved |
| Gate 3 | Approved |
| Gate 4 | Approved |
| Gate 5 | Approved |
| Gate 6 | Ready fork PR #4 open; merge remains separately gated |
| Branch base | `a0efffbd430d72575c44773dc0d7feec0c726f49` |
| Intended fork PR target | `codex/upstream-rebuild` |
| Intended upstream target | None |

## 3. Opening program sequence

Do not combine these steps in one branch or PR.

| Order | Outcome | State | Dependency |
|---:|---|---|---|
| 1 | Add fork-only `PLAN.md` and `SPEC.md` | Complete | Merged through fork PR #3 |
| 2 | Qualify untouched upstream baseline and run Kindle smoke test | Active, Gate 6 | Gates 1-5 approved |
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
| [Fork PR #4](https://github.com/kravenos/krakuyomi/pull/4), `codex/qualify-upstream-baseline` | Target `codex/upstream-rebuild` | Artifact, backup, and Kindle qualification evidence | Open, ready | Fork-only; merge requires explicit approval |
| [Fork PR #3](https://github.com/kravenos/krakuyomi/pull/3), `codex/fork-governance-docs` | Target `codex/upstream-rebuild` | Root fork governance | Merged at `a0efffb` | Fork-only |
| `codex/upstream-rebuild` | Upstream `v1.39.6` plus root governance documents | Clean rebuild lineage | Remote, runtime-unmodified, Kindle-qualified | Do not promote to `main` yet |
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
| Fork governance Build and CI | `0a15f153952910c70a711e60b392b54dd099b613` | Passed | All package targets, Rust checks, tests, and luacheck passed; release job skipped |
| Fork PR artifact suitability | Temporary merge `b7e020dee04af898c7d6ae7b30dc227824cbc784` | Rejected for device use | Built as version `1.0.0`, not the exact official release artifact |
| Official KindleHF artifact | Upstream v1.39.6 `rakuyomi-kindlehf.zip` | Passed | Size, SHA-256, ZIP safety, target metadata, and extracted package verified |
| Post-exit MTP backup | Current plugin, full RakuYomi data, and KOReader settings | Passed | Device and backup match path-by-path and byte-for-byte; 3,921 files sealed by SHA-256 manifest |
| Disposable database verification | Database plus WAL/SHM copied from the sealed backup | Passed | SQLite integrity and quick checks returned `ok`; no foreign-key violations |
| Library/source diagnostic | 45 library entries across four installed sources | Passed | No orphaned library entries; 13 SQLx migrations present |
| Gate 6 live-ref review | Upstream and fork refs on 2026-08-03 | Passed | Upstream `main` and v1.39.6 remain `0ef01d0`; fork rebuild remains `a0efffb`; fork `main`, PR #2, PR #3, and no-release state remain as recorded |
| Gate 6 fork publication | [Fork PR #4](https://github.com/kravenos/krakuyomi/pull/4) | Published | Ready PR targets `codex/upstream-rebuild`; merge and release remain unauthorized |

### Baseline qualification contract

- Use only the official `rakuyomi-kindlehf.zip` from upstream v1.39.6,
  expected size `13,569,515` bytes and SHA-256
  `54e0c735369027160fd2ba7e3a70fa521fb9c819c0db79b803a443bf5d3deebb`.
- Before installation, fully stop KOReader and back up the current plugin, the
  complete `koreader/rakuyomi` directory including SQLite WAL/SHM files, and
  KOReader settings through the verified Windows MTP device.
- Verify local backup inventory, checksums, SQLite integrity, library counts,
  installed-source inventory, available device space, and reversible paths.
- Preserve the original plugin and data on-device. Install and test only against
  a complete disposable data clone first.
- Gate 5 covers offline startup, library and Settings access, source management,
  a downloaded chapter, one working online source, progress persistence, and a
  KOReader restart. Do not uninstall, clean, refresh all, or update sources.
- Any artifact mismatch, backup failure, database failure, crash, or loss of
  membership or progress stops validation and triggers rollback.

### Gate 4 qualification evidence

- The official archive is `13,569,515` bytes with SHA-256
  `54e0c735369027160fd2ba7e3a70fa521fb9c819c0db79b803a443bf5d3deebb`.
  Its 175 ZIP entries are path-safe; the extracted KindleHF plugin contains 121
  files totaling `32,319,968` bytes and identifies itself as v1.39.6.
- The extracted baseline plugin is `61,638` bytes smaller than the preserved
  recovery plugin snapshot (`32,381,606` bytes).
- After KOReader fully exited, the external snapshot matched the mounted device
  exactly by relative path and byte size:

  | Snapshot | Files | Folders including root | Bytes |
  |---|---:|---:|---:|
  | Current plugin | 121 | 54 | 32,381,606 |
  | Complete RakuYomi data | 3,765 | 636 | 2,194,478,209 |
  | KOReader settings | 35 | 2 | 32,177,043 |
  | Total | 3,921 | 692 | 2,259,036,858 |

- The post-exit manifest supersedes the earlier pre-exit estimate: shutdown
  exposed 30 additional data files, 10 additional data folders, and one
  additional settings file. All post-exit items are present in the backup.
- The checksum manifest itself has SHA-256
  `202deb3e7b899e8534b6558c39dd2576ec85ce0affab8cf43f039bd56688e130`.
- The disposable database copy includes `database.db`, `database.db-wal`, and
  `database.db-shm`. Integrity and quick checks returned `ok`, with zero
  foreign-key violations. It contains 13 migrations, 45 library rows, 24,078
  chapter-information rows, and 2,414 chapter-state rows.
- All 45 library entries resolve through the four installed sources:
  `multi.mangafire`, `en.mangakatana`, `en.weebcentral`, and
  `multi.mangadex`. Ten metadata sidecars from previously uninstalled sources
  remain as preserved evidence but are not treated as installed sources.
- The device reports `23,466,647,552` bytes free. A full data clone plus the
  extracted baseline plugin requires `2,226,798,177` bytes, leaving at least
  `21,239,849,375` bytes before runtime overhead.
- Evidence is stored outside the repository under
  `C:\Users\Corv\Documents\rakuyomi-backups\2026-08-01-v1.39.6-baseline`.
  At the Gate 4 checkpoint, the original on-device plugin and data had not been
  installed over, renamed, moved, or deleted.

### Gate 5 disposable-clone checkpoint

- Before testing, the recovery plugin and original data were moved device-locally,
  not copied over or deleted, to these inactive preservation paths:
  `koreader/rakuyomi-gate5/preserved-plugin/rakuyomi.koplugin` and
  `koreader/rakuyomi-gate5/preserved-data/rakuyomi`.
- After the move, both preserved trees still matched the sealed external backup
  exactly: 121 plugin files and 3,765 data files, with no missing, extra, or
  size-mismatched paths.
- For the test, the active paths contained the untouched v1.39.6 plugin and a
  complete disposable clone of the sealed data. Their full path-and-size
  manifests matched their local sources exactly.
- A pre-start read-back hash check passed for all 121 plugin files and 22
  critical data files: the database, WAL/SHM, settings, and every installed-source
  or source metadata file.
- On 2026-08-03, the downloaded-chapter progress check passed across a full
  KOReader restart. Library, Settings, and Sources opened normally, and one
  online source successfully opened a manga listing.
- All bounded manual checks passed. The tested v1.39.6 plugin and data are now
  parked under `tested-plugin` and `tested-data` inside the inactive Gate 5
  workspace.
- The parked post-test clone remains healthy: SQLite integrity and quick checks
  returned `ok`, there are no foreign-key violations or orphaned library
  sources, all 13 migrations and 45 library entries remain, and progress changes
  persisted. The post-test clone additionally contains `multi.nhentai` with no
  library membership; that change is confined to the disposable clone.
- The recovery plugin and original data were restored to their canonical active
  paths. Their complete path-and-size manifests still match the sealed backup,
  and read-back hashes passed for all 121 plugin files and 22 critical data
  files. Active recovery metadata is `1.41.2+recovery.befc938` for KindleHF.
- KOReader settings were not rolled back: ten newer ZenPM files and ten changed
  operational files appeared after the 2026-08-01 snapshot, so overwriting them
  would discard unrelated current user state.
- No device file was deleted or overwritten, and no repository change was pushed
  or released during Gate 5.

### Gate 6 publication package

- The expected final tracked diff is only `PLAN.md`; there is no runtime, build,
  dependency, schema, interface, translation, release-workflow, or performance
  change.
- The qualification evidence commit uses
  `docs: record upstream baseline qualification`.
- The fork PR targets `codex/upstream-rebuild`; no upstream PR is
  appropriate because this is fork-operational qualification evidence.
- The branch is pushed and ready fork PR #4 is open against
  `codex/upstream-rebuild`. Merge, default-branch, tag, release, cleanup, and
  remote-deletion authority remain separate and are not granted by this package.

### Kindle and artifact evidence

| Change | KindleHF commit and artifact | Result | Reason or observations |
|---|---|---|---|
| Fork governance documents | Not applicable | Approved at Gate 5 | Exact diff is Markdown only; no runtime, build, packaging, data, or release effect |
| Untouched baseline qualification | Official upstream v1.39.6 KindleHF asset | Approved at Gate 5 | Startup, Library, Settings, Sources, downloaded chapter, restart persistence, one online source, post-test integrity, and recovery restoration passed |

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
4. Runtime qualification passed and its fork-operational evidence is published
   in ready fork PR #4, but the PR is not approved to merge.
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

- Whether ready fork PR #4 may merge after its checks and review; merge and
  release authority remain separate.
- The release-freeze trigger and fork tag-baseline design in opening step 3.
- The disposition of legacy PR #2 before default-branch promotion.
- The external archive location and verification method for unique recovery
  material before cleanup.

Feature-specific decisions remain deferred to each feature's Gate 2.

## 10. Recommended next action

Verify fork PR #4 checks and review, then present the evidence for explicit merge
approval. Do not delete the parked clone, merge, change `main`, tag, release, or
delete remote state.

## 11. Historical references

The immutable rebuild inventory, source-management discovery specification,
incident report, historical plan, and recovery tool with regression tests are
linked from
[SPEC.md section 12](SPEC.md#12-detailed-specifications-and-historical-evidence).
They are evidence, not the current plan.
