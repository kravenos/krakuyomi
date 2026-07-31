# Fork rebuild specification

Status: discovery draft; decisions required before rebuild or implementation.
Purpose: define what this fork added to upstream, what is worth carrying into a
clean rebuild, and how to prove the rebuilt fork is complete.

Historical companion: `plan.md`.
Source-management design: `source-management-spec.md`.
Incident evidence: `incident-report.md`.

## 1. Objective

Start from a clean current upstream branch and deliberately reintroduce only
the fork behavior that remains useful, safe, and maintainable. Do not replay
the existing branch wholesale and do not treat release commits, translations,
or recovery artifacts as independent product features.

This document is the proposed source of truth for rebuild scope. `plan.md`
remains useful as a chronological recovery record and machine/device handoff,
but it is not precise enough to decide rebuild scope by itself.

## 2. Audit baseline

Audited 2026-07-29:

- Current fork HEAD: `182e640` on `fix/settings-corruption-recovery`.
- Fork merge base: upstream `fbfca54`, tagged `v1.39.3`.
- Current upstream: `465811d`; latest fetched release tag `v1.39.5`.
- Divergence: 21 commits reachable only from the fork and 5 reachable only
  from current upstream.
- `git cherry upstream/main HEAD` reports no patch-equivalent fork commits in
  current upstream.
- The working tree was clean before this documentation audit.

The five upstream-only commits add Darwin build support, fix playlist
`hideTopClose` state, release 1.39.4/1.39.5, and apply clippy/husky changes.
They are baseline work, not fork scope.

The audit also covered local/origin branches outside HEAD. The
`feat/auto-delete-downloads`, `feat/storage-stats`, `fix/frontend-robustness`,
`feat/clearer-sort-labels`, combined library-UX branches, and pre-rebase backup
branches are older aggregate/review histories for work either accepted
upstream or superseded by the F-items below. `test/bridge-url-fix` is
patch-equivalent to an upstream fix. These refs are historical evidence, not
additional rebuild features, and must not be replayed separately.

Before an actual rebuild, fetch again and replace these hashes. A moving branch
name such as `upstream/main` is not a reproducible baseline.

## 3. Scope classification

Every item receives one of four dispositions:

- **KEEP:** evidence supports carrying the behavior into the clean rebuild.
- **CONDITIONAL:** useful only if a named compatibility or product condition
  still applies.
- **REASSESS:** plausible value, but insufficient evidence to commit now.
- **DO NOT PORT:** history, generated output, or behavior already supplied by
  upstream.

Dispositions below are recommendations for review, not implementation
authorization.

## 4. Upstreamed work: do not rebuild

The following work shipped upstream in 1.39.0 through PRs #256-#259 and should
come from the new upstream baseline:

- settings and error-handling robustness accepted in PR #256;
- `GET /storage-stats` and the Settings total-downloaded row;
- automatic deletion of downloaded chapters on remove/on read;
- clearer library sorting labels.

Reimplement only if a fresh baseline audit proves upstream regressed or removed
required behavior. Maintainer-review lessons in `plan.md` remain useful for
future upstream proposals but are not product requirements.

## 5. Implemented fork-only behavior

### F-01: Atomic settings and corruption recovery — KEEP

Commits: `c36dd60`, `059fde9`, `befc938`.

Behavior:

- settings writes use a same-directory temporary file, flush, fsync, and
  rename;
- a last-known-good backup is written;
- unreadable primary settings are preserved and startup can recover without
  silently discarding configuration;
- tests cover the server recovery path.

Why keep: the incident demonstrated real truncation, loss of `source_lists`,
and startup failure. Current upstream `465811d` still uses `File::create` and
incremental serialization, so the hazard remains.

Rebuild acceptance:

- interruption before publish leaves either the previous or new complete JSON;
- corrupt primary settings are preserved, recovery source is reported, and
  configured source lists survive when a valid backup exists;
- Kindle device test passes before release.

### F-02: Settings UI nil/unknown-value safety — KEEP

Commits: `7d50deb`, `c2a254e`.

Behavior: multi-enum defaults render on first open, and an enum value without a
matching option does not crash the Settings screen.

Why keep: Settings is constructed in one pass, so one unexpected value can
make the entire screen unavailable. This is a small compatibility boundary.

Rebuild acceptance: missing and obsolete values render a safe fallback without
mutating the stored value merely by opening Settings.

### F-03: Configurable library tile metadata — KEEP

Commits: `b6e804e`, corrected by `70d7deb`.

Behavior:

- `rakuyomi_tile_metadata` selects last-read, unread, read, and total metadata;
- read plus total renders as `12/57`;
- one grouped query computes explicit read and total counts for library and
  playlist views;
- search models remain contract-compatible;
- database tests cover count semantics.

Why keep: it is user-visible, configurable, and measured at about 7 ms on the
recorded 100-manga/17k-chapter library. Explicit reads cannot be inferred as
`total - unread`.

Rebuild constraints: no per-manga query loop; do not regenerate `.sqlx` unless
the clean baseline requires it; preserve the explicit `chapter_state` join.

Rebuild acceptance: library and playlist counts match database fixtures for
read, unread, absent-state, and total cases; every view-mode combination
renders without regression.

### F-04: Manga tap and chapter-list flow — REASSESS

Commit: `68e9d27`, corrected by `70d7deb`.

Behavior: tapping a manga opens Continue Reading / Chapter List choice; the
context menu always offers Chapter List; the former tap-action setting is
removed.

Question: is the extra choice preferable on an e-ink device for every manga,
or should direct-continue remain configurable? Keep only after a brief device
UX decision.

### F-05: Hide fully read manga — KEEP

Commit: `c8442be`.

Behavior: a library-menu and Settings toggle hides fully read manga.

Rebuild acceptance: the toggle affects library presentation only, never
membership, playlists, downloads, or refresh behavior.

### F-06: Reader preferences and back-to-library — KEEP, SPLIT

Commit: `8c09157`, corrected by `70d7deb`, translations in `804a83d`.

Behavior: reading direction, page-turn style, and a Back to library reader menu
item.

Why split: the navigation action and two preferences are independently useful
and should be individually removable if current upstream reader behavior has
changed.

Rebuild acceptance: each preference maps to the intended KOReader reader
behavior; Back to library returns to the originating library/playlist context;
device navigation and back-button behavior are tested.

### F-07: Total downloaded size as library title — REASSESS

Commit: `6d692f4`.

Behavior: the existing upstream storage total is displayed as the library menu
title.

Question: does this information deserve the most prominent library label, or
should the upstream Settings row remain the only surface? Port only if the
device UX value exceeds the title-space cost.

### F-08: Ignore missing historical migrations — CONDITIONAL

Commit: `aca9ba6`.

Behavior: the sqlx migrator uses `set_ignore_missing(true)` for startup and hot
replace so databases containing fork-development migration rows can open.

Risk: globally ignoring missing migrations weakens drift detection. It is not
needed for a fresh database.

Condition to keep: an existing user database that must be retained contains
applied migrations absent from the clean upstream tree. Prefer a one-time,
explicit compatibility repair or verified migration bridge over a permanent
global relaxation.

### F-09: Manual Build workflow trigger — KEEP IF STILL ABSENT

Commit: `94d8494`.

Current upstream `build.yml` still lacks `workflow_dispatch` (although other
workflows have it). Keep this one-line operational capability if branch/device
artifact builds remain part of the release process.

### F-10: Localization updates — GENERATED/DERIVED

Commits: bulk changes in `70d7deb` and `804a83d`.

Do not cherry-pick translations as a standalone feature. After final strings
are known, run the repository translation update workflow and verify the
catalog. Translation files are required output of retained UI features.

### F-11: Fork release commits and changelog entries — DO NOT PORT

Commits: `d5fa101`, `6c77d08`, `7520557` and fork release notes.

These describe the old branch lineage. A clean rebuild receives a new version
and changelog entry after verified behavior lands; replaying old release
commits would corrupt release history.

## 6. Planned but unimplemented ideas

These are not current-fork behavior and must never be included in a claim that
the rebuild restored existing functionality.

| Item | Current recommendation | Reason |
|---|---|---|
| Source management and search | Specify, then prioritize | Incident-backed; canonical requirements live in `source-management-spec.md` |
| Collection cover tiles / cover-grid chooser | Reassess after rebuild | Existing playlists supply much of the domain behavior, but UX/value is untested |
| Wrap/shrink long grid titles | Keep as a small later enhancement | Clear defect and existing cover-mode pattern, but unrelated to reset readiness |

Offline tools and device data in `dist/` are incident-recovery artifacts, not
application features. Preserve them separately if still needed; do not make a
clean rebuild depend on git-excluded files.

## 7. Proposed clean-rebuild sequence

This is a decision sequence, not authorization to start:

1. Pin and record the latest upstream commit and release tag.
2. Create a clean branch from that exact commit.
3. Verify upstream build/test/device startup before adding fork behavior.
4. Reintroduce F-01 and F-02 first because they protect configuration and the
   Settings control surface.
5. Decide F-08 against the actual device database before opening it with the
   clean build.
6. Port retained UX features in independent changes: F-05, F-06 components,
   F-03, then any approved F-04/F-07.
7. Add F-09 and regenerate F-10 only after retained behavior stabilizes.
8. Run full verification and a staged Kindle migration; release only after
   on-device confirmation.
9. Treat source management as a new feature program against the rebuilt
   baseline, not as part of reconstructing old behavior.

## 8. Commands and verification

Use the clean baseline's documented toolchain. The current known verification
commands are:

```sh
git archive <branch> | tar -x -C /tmp/src
cd /tmp/src/backend
SQLX_OFFLINE=true cargo check -p shared -p server
SQLX_OFFLINE=true cargo test -p shared -p server
docker run --rm -v <export>:/work -w /work pipelinecomponents/luacheck \
  luacheck frontend/
```

Export rather than mounting the Windows working tree to avoid CRLF-dependent
sqlx migration checksums. Before adopting these commands, reconcile them with
current upstream CI, which now includes Darwin and husky/clippy changes.

Verification levels:

- unit/database tests for retained Rust behavior;
- Lua lint and focused UI tests for every retained frontend behavior;
- contract checks for changed JSON models;
- clean install and existing-data upgrade tests;
- KindleHF package build and on-device smoke/regression pass;
- explicit backup/restore rehearsal for settings and database before migration.

## 9. Boundaries

- Always: start from a pinned upstream commit; port features independently;
  preserve user data; verify on the Kindle before release; update this spec
  when scope changes.
- Ask first: feature disposition changes, database compatibility strategy,
  schema changes, dependencies, CI/release changes, or any destructive device
  operation.
- Never: merge the current branch wholesale into the clean baseline; count
  planned features as restored behavior; replay old release commits; publish
  before device verification; discard the only copy of device data.

## 10. Success criteria for reset readiness

The project is ready to start over only when:

- every implemented fork feature has an approved KEEP, CONDITIONAL, REASSESS,
  or DO NOT PORT disposition;
- the exact upstream base is pinned and current at the time of work;
- the device database/settings backup and F-08 compatibility decision are
  complete;
- accepted behaviors have testable acceptance criteria;
- the source-management spec's open decisions are either answered or
  explicitly deferred from the rebuild;
- release, rollback, and on-device verification steps are agreed.

## 11. Decisions needed from the owner

1. Approve or change the proposed dispositions, especially F-04 and F-07.
2. Confirm whether the existing Kindle database must migrate intact into the
   clean build; this determines F-08.
3. Decide whether the rebuild should remain a private fork or aim to upstream
   F-01/F-02 and later source-management work in small proposals.
4. Confirm that source management begins only after parity with the approved
   retained feature set.
