# RakuYomi Controlled Rebuild Plan

- Status: MangaFire saved-library repair implemented; automated verification and a combined KindleHF preview are pending.
- Active gate: All six gates are pre-approved for the bounded fork-only program
- Last verified: 2026-09-21
- Active change: Restore opening and refreshing existing MangaFire library entries without changing their stored identities.
- Publication authority: Fork branches, pull requests, sequential fork merges, validation builds, and final KindleHF artifact preparation are approved; upstream publication, GitHub releases, destructive device work, and unverified cleanup remain prohibited

This file is the operational source of truth for the controlled rebuild. The
durable requirements and policies are in [SPEC.md](SPEC.md). Keep detailed
feature behavior in separately approved feature specifications.

## 1. Current baseline

| Item | Verified state |
|---|---|
| Upstream | `tachibana-shin/rakuyomi` |
| Upstream branch | `main` at `66d592f5118d00ef899a049032f5cad0c6ace2c0`; one unreleased test-only commit beyond the pinned baseline |
| Pinned upstream release | [`v1.41.4`](https://github.com/tachibana-shin/rakuyomi/releases/tag/v1.41.4) at `df0ef29fc07d87966a1a2558ab257743f29efaf4` |
| Fork | `kravenos/krakuyomi` |
| Clean rebuild branch | Fork `main` at `ca36a992ed400efa1f5c3dc0be10b65c012d66fe`; pinned upstream v1.41.4 plus isolated rebuild outcomes and Sources/search repairs |
| Fork default branch | `main` at `ca36a992ed400efa1f5c3dc0be10b65c012d66fe` after fork PR #33 |
| Historical archive | `codex/archive-pre-upstream-rebuild-2026-07-31` at `44794ff8112ae3d40bded3fea0cbd9175434d72a` |
| Fork releases | None |
| Fork release baseline tag | `v1.41.4` points exactly to upstream release commit `df0ef29`; no GitHub release exists |
| Latest completed repair CI | PR run `34348172572` and post-merge run `34349641015` succeeded; release publication was skipped |

The v1.41.4 release commit `df0ef29` adds release metadata to code parent
`9b06ec2`, whose upstream Build and Lua checks passed on 2026-08-27 and produced
the published stable artifacts. The official KindleHF asset is 15,892,561 bytes
with SHA-256 `c4720bf29918266401ec0a4f6c677dda28cf887ebb7570ecd4f2dd0e54829e0b`.
The fork must build its own exact candidate because its version metadata and
release-control workflow differ from upstream.

Upstream `66d592f` is deliberately excluded because it is one unreleased,
test-only commit beyond v1.41.4.

## 2. Active program position

Exactly one change may be active in Gates 1 through 5.

| Field | Current value |
|---|---|
| Active outcome | MangaFire saved-library compatibility; preserve progress and existing downloads |
| Classification | Fork-only bug repair; preserve installed sources and library data |
| Gate 1 | Offline installed-source probe confirmed malformed requests for all 22 saved MangaFire entries; fresh short-key controls work |
| Gate 2 | Exact next-SDK MangaFire v8 boundary repair; no database migration, source replacement, or library re-addition |
| Gate 3 | Approved by Corvin's go-ahead; isolated branch `codex/fix-mangafire-library-references` from `ca36a99` |
| Gate 4 | Five expected regression failures reproduced; fix `f409814` independently reviewed; after-fix CI pending |
| Gate 5 | Pending for MangaFire. Prior accepted installed preview `1.41.4+ci.93.fffe1c3` retains reading-direction and UI fixes |
| Gate 6 | Fork PRs and merges pre-approved after review and green checks; releases prohibited |
| Branch base | Isolated fix: fork main `ca36a99`. Combined preview: accepted reader/UI code plus preserved local evidence at `8b4c179` |
| Intended fork PR target | `main` |
| Intended upstream target | None; the complete program is fork-only |

## 3. Opening program sequence

Do not combine these steps in one branch or PR.

| Order | Outcome | State | Dependency |
|---:|---|---|---|
| 1 | Add fork-only `PLAN.md` and `SPEC.md` | Complete | Merged through fork PR #3 |
| 2 | Qualify untouched upstream baseline and run Kindle smoke test | Complete | Merged through fork PR #4 |
| 3 | Freeze automatic fork releases while preserving CI artifacts | Complete | Merged through fork PR #5; v1.39.6 tag baseline added |
| 4 | Promote the qualified clean lineage to default `main` | Complete | `main` moved to `8c732a3`; old `main` archived exactly |
| 5 | Refresh the clean lineage to stable upstream v1.41.4 | Complete | Fork PR #7 merged at `b7d97c8`; every platform build passed; release skipped |
| 6 | Reassess and rebuild every approved specification in isolated fork PRs | Complete | Fork PRs #8-#25 and #28-#29 merged; PR #27 contains isolated test maintenance |
| 7 | Remove only approved unnecessary repository material | Complete in cleanup change | Eight tracked files removed from three proven-unused groups; final CI and merge required |

### Cleanup removal matrix

| Candidate | Classification and evidence | Decision |
|---|---|---|
| `.github/workflows/issue-label-flow.yml%`, `.github/workflows/test.yml%` | Disabled copies: the `%` suffix prevents GitHub from treating them as workflows; neither is referenced; active build, CI, and Lua workflows cover current checks | Remove |
| `mv_frontend.sh`, `mv_linux.sh` | Superseded and unreferenced copy shortcuts; the documented development environment and `tools/` commands provide the maintained paths | Remove |
| `.opencode/`, `prompts/`, `skills/` | Four tool-specific or raw AI working notes; no runtime, build, CI, documentation, or current agent configuration references them | Remove |
| `telegram-cookie-bot/` | Product companion referenced by the root README and its deployment workflow | Retain |
| `ci/`, `packages/`, `tools/`, Nix/devenv, editor and external-service configuration | Referenced by builds, documented development commands, or filename-based integrations | Retain |
| Ignored `dist/` recovery material | Unique recovery evidence is not yet proven archived elsewhere | Retain; never include in tracked cleanup |

## 4. Ordered feature backlog

Current priority (2026-09-21): finish automated checks for the MangaFire repair,
then supply a combined KindleHF preview for manual copying and device testing.
Keep this fix separate from still-open reader/UI PRs #34 and #35. No merge or
release is part of the current handoff. Further UI tuning stays deferred and
MangaKatana Server 3 remains the accepted setting.

See [MangaFire compatibility contract and checks](docs/verification/2026-09-21-mangafire-compatibility.md)
and [offline diagnosis](docs/verification/2026-09-21-mangafire-diagnosis.md).

Current execution checkpoint (2026-09-09): fork PR #34 contains the reader fix
at `50ac7ed`; fork PR #35 contains the UI fix at `c2c11ae`. Both remain open.
Combined KindleHF `1.41.4+ci.93.fffe1c3` passed packaging, Rust CI, Lua CI and 26
local tests in both Lua 5.1 and LuaJIT. Corvin reports direction seems preserved,
Server 3 is much faster, and UI improvement is sufficient for now. Accept these
observed outcomes; do not infer completion of the full manual checklist.
The preview combines both fixes without merging either PR into main. Overall
build run `34356659856` failed in Android-aarch64 SDK emulator unpacking only.

MangaKatana: a bounded PC comparison used the public example chapter documented
by the source author, not identifiers from Corvin's library. Same first image,
589,032 bytes: Server 1 = 1.36 s (headers 1.33 s), Server 2 = 7.09 s (headers
6.73 s), Server 3 = 0.16 s (headers 0.12 s). Page-list retrieval was 0.36-0.44 s.
This is one sequential sample per server on a PC, not a Kindle benchmark or a
universal server ranking. The installed v3 package already offers all three image
servers and the author implementation sets Referer without custom image processing.
Corvin's subsequent Kindle comparison confirms Server 3 is much faster for him;
retain that setting and defer further download work. Do not automatically replace
the source or increase concurrency. No downloader runtime change was made.
A private-library test was blocked before execution and was not run; it is no
longer needed for this investigation.

### Approved follow-up fixes (2026-09-09)

Corvin requested work on reading-direction persistence, MangaKatana download
speed, and UI speed. Keep each fix in its own fork-only branch/PR. Preserve all
functionality and dependencies; no releases or automatic device copies.

Reading-direction branch: `codex/fix-reading-direction-persistence`, based on
fork main `ca36a99`. A standalone regression suite reproduced five failures:
LTR/RTL reset on delayed first open and chapter switch, and scroll style reset.
Apply explicit preferences to the new reader in KOReader's after-open callback,
after its saved settings and source viewer. Do not force a direction when unset.
All nine reader cases now pass on Lua 5.1 and LuaJIT; three existing source-screen
cases also pass. Combined Rust/Lua CI and KindleHF packaging passed; Corvin reports
direction seems preserved. Unreported device cases remain unverified. The lifecycle matches
KOReader v2026.07.2 (version found in the preserved crash log).
The settings-default follow-up reproduced two additional failures: unset values
displayed LTR/paginated despite the runtime following viewer mode. Both controls
now offer "Follow viewer mode" as the automatic default; all four explicit choices
remain. Six settings tests pass on Lua 5.1 and LuaJIT; no stored setting is rewritten.

Device check: choose LTR in RakuYomi Settings, cross two chapter boundaries,
close/reopen, then repeat with RTL and with continuous scroll. Verify downloaded
reading, progress, chapter list, and Back to library remain intact. Corvin performs
installation manually. Retain the accepted search-repair package for rollback.

### Approved UI-speed follow-up (2026-09-09)

Corvin requested UI speed improvements without removing functionality or
dependencies. Branch `codex/speed-up-cover-rendering` starts at fork main
`ca36a99`, independently of the reading-direction fix.

Cover rendering now measures and scales within one image widget's normal loading
cycle, eliminating the separate size-probe decode and its discarded buffer.
No persistent image cache is added; sizing, border, placeholder, and disposal
behavior remain unchanged. The cache-miss test baseline decoded twice per cover;
the candidate decodes once. Three reproduction failures became eight passing
cases on both Lua 5.1 and LuaJIT, including same-widget free/re-render. Full Lua
lint passes. This is a deterministic work/ownership reduction, not a measured
Kindle wall-clock speedup. Combined CI and KindleHF packaging passed. Corvin reports
some UI improvement and accepts it for now; further optimization is deferred.

Kindle check: compare cold/warm grid opening and page changes on the same library;
check portrait/landscape covers, text, borders, and repeated opening/closing.
Install manually, retain the accepted package for rollback, and do not touch data.
All changes and PRs remain fork-only; release freeze remains in force.

The order is safety-first. Classification is provisional until each feature's
Gate 2. Re-rank this table after every successful device-tested feature build.
Do not re-rank merely because a branch was opened or CI passed.

| Priority | Outcome | Default classification | State | Rationale or prerequisite |
|---:|---|---|---|---|
| 1 | Atomic settings writes | Upstream candidate | Merged in PR #9 | Prevent interrupted saves from truncating configuration |
| 2 | Corrupt-settings recovery and last-known-good backup | Upstream candidate | Merged in PR #10 | Recover source lists and preserve corrupt evidence |
| 3 | Nil and unknown Settings value safety | Use upstream | Supplied by v1.41.4 | Current widgets safely handle nil, unknown, and wrong-type values |
| 4 | Remove the source package and metadata sidecar together | Use upstream | Supplied by v1.41.4 | Uninstall covers package, metadata, and probe files for all four source families |
| 5 | Keep missing-source library entries visible | Upstream candidate | Merged in PR #12 | Preserve visible access to user-owned membership and progress |
| 6 | Warn before uninstall with affected manga count | Upstream candidate | Merged in PR #13 | Make orphaning consequences explicit before action |
| 7 | Preserve and aggregate source errors honestly | Upstream candidate | Merged in PRs #11 and #16 | Replace repeated opaque failures with actionable source summaries |
| 8 | Hide fully read manga | Upstream candidate | Merged in PR #21 | Restore approved library filtering without altering membership |
| 9 | Reading direction | Upstream candidate | Merged in PR #22 | Restore one independently reviewable reader preference |
| 10 | Page-turn style | Upstream candidate | Merged in PR #23 | Restore one independently reviewable reader preference |
| 11 | Back to library | Upstream candidate | Merged in PR #24 | Restore one independently reviewable navigation outcome |
| 12 | Configurable library tile metadata | Upstream candidate | Merged in PR #25 | Restore useful counts without N+1 database work |
| 13 | Deterministic source provenance and catalog selection | Upstream candidate | Merged in PRs #14-#15 | Make provider and version choice exact and reproducible |
| 14 | Source update visibility and source-list management | Upstream candidate | Merged in PRs #17 and #19 | Complete status and guarded list controls |
| 15 | Diagnostics and source-aware search improvements | Upstream candidate | Merged in PRs #18 and #20 | Bounded diagnosis and direct included-source selection |
| 16 | Manga-tap action dialog | Fork-only | Merged in PR #28; device review pending | Retain both Continue Reading and Chapter List without hiding either action |
| 17 | Downloaded storage total in the library title | Fork-only | Merged in PR #29; device review pending | Reuse the existing aggregate endpoint; no per-item work |
| 19 | Long grid-title wrapping/shrinking | Use upstream | Supplied by v1.41.4 | Current grid and menu widgets already include bounded multiline/shrink behavior |

Translations and tests remain with the outcome they support. Reading direction,
page-turn style, and Back to library remain separate branches and PRs.

### Backlog review decisions

- 2026-09-09: Corvin removed collection cover tiles from the backlog. This was
  an unbuilt visual idea with no accepted requirements. Keep this decision as
  history; no code or user data is removed.

### New user reports and review recommendations (2026-09-09)

Corvin reports slow UI, reading direction sometimes reverting to right-to-left
at chapter boundaries, and very slow source downloads. These are open issues,
not covered by the earlier successful search-repair retest. This is a diagnostic
review only: no runtime changes, dependency removals, builds, transfers, PRs,
or releases. Preserve all functionality and existing dependencies.

Download follow-up: Corvin identifies MangaKatana as slower than other sources
and says progress is slow throughout, rather than only before downloading starts.
The preserved package identifies itself as `en.mangakatana` version 3. The progress
counter advances after image retrieval/processing, just before each archive write;
it is not a network-speed measurement. Prioritize timings per page (request setup,
transfer, any source image processing, and storage) over startup-only theories.
No source-specific processing requirement or server throttling is yet confirmed.

Proposed order, not yet a new implementation commitment:

1. Reading-direction persistence. `MangaReader.lua` applies viewer mode during
   reader initialization and explicit direction in a separate next-tick callback;
   the document-switch completion callback does not reapply that preference.
   `Settings.lua` displays LTR as the unset default, while the runtime deliberately
   does nothing for an unset preference. Confirm which control Corvin uses and
   reproduce chapter transitions before choosing precedence or changing defaults.
   Existing helper tests do not cover the document-switch lifecycle.
2. UI responsiveness. `patch/MenuItemCover.lua:427` constructs two uncached image
   widgets for each local cover: load for dimensions, then load/render for display.
   Review single-load reuse, explicit image lifetime, and bounded caching. Library
   loading also makes synchronous backend calls; the library route holds storage,
   settings, and source-manager locks across database work. Measure cold/warm grid
   loading, paging, menu opening, and memory before and after any isolated change.
3. Download throughput. Pages already download concurrently, but batch chapters
   run sequentially and each chapter creates a new HTTP client. Archive writes
   are synchronous inside async code. The preserved debug settings show four
   concurrent page requests, image optimization disabled, and grid view; current
   device settings are not independently verified. Measure page-list retrieval,
   network transfer, source processing, and storage separately. Consider connection
   reuse and bounded scheduling only where measurements support them; retain
   source headers, cookies, image processing, cancellation, and resource limits.

For the older pending items: recommend retaining safe repository cleanup after
verified preservation and documentation reconciliation; defer canonical-id repair
until a real blocked manga needs it; remove historical missing-migration support
from active work unless an affected database is demonstrated. These are proposals,
not approved removals. Retain completed features and their tests as history, not
as unfinished development. Collection cover tiles remain the sole approved drop.

Static review identifies candidates, not measured Kindle speedups or a confirmed
cause of the reported download slowness. Current official KOReader source was
consulted for image-cache and reader callback semantics; the installed KOReader
revision still needs matching before relying on lifecycle details.

### v1.41.4 audit summary

| Area | Result |
|---|---|
| Settings robustness | Supplied upstream; do not rebuild |
| Atomic settings writes | Missing; retain as isolated change |
| Corruption recovery and backup | Missing; retain after atomic writes |
| Hide fully read | Missing; retain |
| Reader direction, page-turn style, Back to library | Missing; retain as three PRs |
| Configurable tile metadata | Only a show/hide switch exists; retain the selected metadata fields without N+1 queries |
| Manga-tap flow | Upstream has one configured direct action; retain the explicit two-choice dialog as fork-only behavior |
| Library storage title | Existing aggregate storage endpoint makes the display cheap; retain as fork-only behavior |
| Missing migration relaxation | Condition not proven on the preserved 13-migration database; do not add a global relaxation |
| Source package cleanup | Supplied upstream for all current package types |
| Truthful source inventory and missing library rows | Missing; retain |
| Catalog cache, exact provenance, deterministic choice | Missing; retain |
| Structured search errors | Partially supplied; normalize and aggregate with refresh and persisted health |
| Source/list UI | Partially supplied; extend through the current source specification |
| Search source identity | Base/cover are supplied; grid and included-source selection remain |
| Diagnosis | Missing; retain as bounded read-only work |
| Canonical-id migration | Explicit later capability; not part of this rebuild |
| Collection covers | Removed from the backlog by Corvin on 2026-09-09; no implementation |
| Long grid titles | Current widgets already wrap/shrink; do not duplicate |

## 5. Branch and PR map

| Branch or PR | Base or target | Purpose | State | Publication constraint |
|---|---|---|---|---|
| [Fork PR #29](https://github.com/kravenos/krakuyomi/pull/29) | Fork `main` | Downloaded storage total | Merged at `096de31`; all checks passed including KindleHF | Fork-only; never release |
| [Fork PRs #21-#25 and #28](https://github.com/kravenos/krakuyomi/pulls?q=is%3Apr+is%3Amerged) | Fork `main` | Library and reader outcomes | Merged as isolated PRs; all required checks passed | Fork-only program; no upstream PRs opened |
| [Fork PRs #11-#20](https://github.com/kravenos/krakuyomi/pulls?q=is%3Apr+is%3Amerged) | Fork `main` | Source-management specification | Merged as isolated PRs; all required checks passed | Fork-only program; no upstream PRs opened |
| [Fork PRs #9-#10](https://github.com/kravenos/krakuyomi/pulls?q=is%3Apr+is%3Amerged) | Fork `main` | Atomic settings and corruption recovery | Merged as isolated PRs; all required checks passed | Fork-only program; no upstream PRs opened |
| [Fork PR #8](https://github.com/kravenos/krakuyomi/pull/8) | Fork `main` | Current v1.41.4 rebuild and source-management specifications | Merged at `bde20d7` | Fork-only; never upstream |
| [Fork PR #27](https://github.com/kravenos/krakuyomi/pull/27) | Fork `main` | Make unreliable live LNReader website smoke tests opt-in | Merged at `fbed8fd`; deterministic CI remains mandatory | Fork-only maintenance |
| [Fork PR #7](https://github.com/kravenos/krakuyomi/pull/7), `codex/sync-upstream-v1.41.4` | Fork `main` at `8c732a3` | Merge exact stable upstream v1.41.4 and retain manual release control | Merged at `b7d97c8`; all checks passed; release skipped | Fork-only; never release |
| [Fork PR #8](https://github.com/kravenos/krakuyomi/pull/8), `codex/specify-v1.41.4-rebuild` | Fork `main` | Record the current audit and accepted source-management contract | Merged at `bde20d7` | Fork-only; never upstream |
| `codex/update-rebuild-plan` | Target `codex/upstream-rebuild` | Record completed qualification, release control, and tag baseline | Merged through fork PR #6 at `8c732a3` | Fork-only |
| [Fork PR #5](https://github.com/kravenos/krakuyomi/pull/5), `codex/freeze-automatic-releases` | Target `codex/upstream-rebuild` | Preserve builds while requiring deliberate release publication | Squash-merged at `330d367` | Fork-only; no release created |
| [Fork PR #4](https://github.com/kravenos/krakuyomi/pull/4), `codex/qualify-upstream-baseline` | Target `codex/upstream-rebuild` | Artifact, backup, and Kindle qualification evidence | Squash-merged at `95781ce` | Fork-only |
| [Fork PR #3](https://github.com/kravenos/krakuyomi/pull/3), `codex/fork-governance-docs` | Target `codex/upstream-rebuild` | Root fork governance | Merged at `a0efffb` | Fork-only |
| `codex/upstream-rebuild` | Qualified v1.39.6 lineage plus fork governance and release control | Clean rebuild lineage | Remote at `8c732a3`, equal to current fork `main` | Historical baseline after v1.41.4 refresh |
| `codex/archive-pre-upstream-rebuild-2026-07-31` | Archive `44794ff` | Historical recovery point | Preserved locally and remotely | Never merge wholesale |
| [Fork PR #2](https://github.com/kravenos/krakuyomi/pull/2), `test/bridge-url-fix` to old `main` | Old fork lineage | Legacy test/fix work | Closed as superseded; source branch retained | Never merge wholesale |

No upstream PR was opened. The labels above describe future suitability only;
this complete unattended program remains fork-only.

## 6. Verification evidence

| Evidence | Commit or artifact | Result | Notes |
|---|---|---|---|
| Upstream v1.41.4 Build | `9b06ec2d0cce4fa93e9f04c07e5b115a92f9fe84` | Passed | Upstream run 33117913535 produced the stable release artifacts |
| Upstream v1.41.4 release | `df0ef29fc07d87966a1a2558ab257743f29efaf4` | Passed | Published stable on 2026-08-27; not copied as a fork release |
| Fork v1.41.4 refresh | [Fork PR #7](https://github.com/kravenos/krakuyomi/pull/7) | Passed | Rust, Lua, schema, desktop, macOS, three Kindle, and three Android jobs passed; release skipped; merged at `b7d97c8` |
| Isolated runtime rebuild | Fork PRs #9-#25 and #28-#29 | Passed in CI | Every accepted runtime outcome was merged separately; the latest PR passed Rust, Lua, schema, desktop, macOS, three Kindle, and three Android jobs |
| Live-network test maintenance | [Fork PR #27](https://github.com/kravenos/krakuyomi/pull/27) | Passed | Existing external LNReader website smoke tests are opt-in; deterministic coverage remains active |
| Latest KindleHF build | PR #29 commit `6d3f8ea071b3537eb1f80ac67cef080dfaa46dc6` | Passed in CI | Packaging passed; final exact-`main` artifact and on-device validation remain |
| Fork v1.41.4 tag | `v1.41.4` at `df0ef29fc07d87966a1a2558ab257743f29efaf4` | Passed | Exact upstream release tag commit; no fork GitHub release created |
| v1.41.4 fork snapshot scope | `c9869e4` versus `v1.41.4` | Passed | Snapshot differs only in `.github/workflows/build.yml`, `PLAN.md`, and `SPEC.md` before the plan commit |
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
| Gate 6 fork publication | [Fork PR #4](https://github.com/kravenos/krakuyomi/pull/4) | Published | The PR was opened ready for review; merge was approved later as a separate action |
| Baseline qualification merge | [Fork PR #4](https://github.com/kravenos/krakuyomi/pull/4) | Passed | Squash-merged into the rebuild branch at `95781ce` |
| Release-control verification | [Fork PR #5](https://github.com/kravenos/krakuyomi/pull/5) | Passed | All tests and seven platform builds passed; release job skipped |
| Non-release artifact version | PR #5 KindleHF artifact | Passed | `1.39.6+ci.18.539f05f`, replacing the incorrect `1.0.0` fallback |
| Fork tag baseline | `v1.39.6` at `0ef01d0bab2ab90a436f4884fd3192f821d4a996` | Passed | Exact upstream tag commit; no GitHub release created |

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
- Fork PR #4 was squash-merged into `codex/upstream-rebuild` at `95781ce`.
  Default-branch, release, cleanup, and remote-deletion authority remain
  separate and were not granted by the qualification package.

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
| Stable upstream refresh | v1.41.4 at `df0ef29`; pinned baseline is present in fork `main` |
| New upstream feature PRs | Prohibited for this unattended program |
| Requested upstream revisions | None |
| Fork governance documents | Intentionally never upstreamed |

## 8. Blockers and residual risks

1. The final Kindle validation needs the device mounted and KOReader fully
   exited; automated evidence cannot substitute for Corvin's observed UI results.
2. Ignored recovery material under `dist/` includes unique databases, source
   packages, settings, mappings, and poster repairs. It is not approved for
   deletion and has not yet been verified in an external archive.

## 9. Decisions awaiting approval

All six gates are pre-approved for the bounded fork-only run. Unresolved feature
details must use the safest behavior consistent with the specifications and
existing project patterns. A genuine data-loss, security, or missing-authority
conflict stops the affected outcome without blocking independent outcomes.

## 10. Recommended next action

Use combined KindleHF `1.41.4+ci.93.fffe1c3` as the current user-accepted package;
retain `1.41.4+ci.87.56081e4` for rollback. The reported reader, UI and MangaKatana
outcomes are sufficient for current use; leave further tuning deferred.
Before integrating the two separate open fork PRs, resolve outstanding Android
SDK setup checks and review merge readiness. Reprioritize before another feature.
Corvin performs all file copies. Preserve recovery material and the release
freeze. No upstream PRs or releases.

On 2026-09-09, Corvin reported "everything passes" in response to the supplied
manual retest guide. Record search, cancellation responsiveness, Sources and
diagnosis screens, downloaded reading, and progress after restart as passed by
user report, not as independently observed by Codex. The earlier failed test
on `1.41.4+ci.80.e408f67` remains historical incident evidence.

Repair evidence: run `34341914191` reproduces settings/storage locks held while
waiting for the source manager. After the lock fix, run `34347405062` passes
all 25 server tests and reproduces busy-worker metadata and pre-cancellation
failures. Added tests also cover cancellation after the search starts and
timeout outcomes saved to source health. Native Windows execution is blocked
by existing Unix-only dependencies; Linux CI is the automated execution record.

The repair snapshots shared state, keeps immutable Aidoku metadata outside the
worker lock, stops awaiting cancelled searches, signals child operations on
drop, and applies the request deadline while waiting for shared state as well
as while searching. Other source runtimes keep their existing dynamic metadata
behavior. Source-selection rules, package versions and library records are not
changed by this repair. These choices follow
[Tokio's blocking-worker limitations](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)
and [cooperative cancellation](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html).

Acceptance requires: Sources and diagnosis screens open; search completes or
reports source-specific failures within its deadline; cancelling a running
search leaves menus/settings usable; downloaded reading and progress still
work after restart. Do not claim that responsive cancellation forcibly stops
arbitrary source code already running in a blocking worker. Investigate any
remaining CPU/memory pressure on-device before accepting the build.

## 11. Historical references

The immutable rebuild inventory, source-management discovery specification,
incident report, historical plan, and recovery tool with regression tests are
linked from
[SPEC.md section 12](SPEC.md#12-detailed-specifications-and-historical-evidence).
They are evidence, not the current plan.
