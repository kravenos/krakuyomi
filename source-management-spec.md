# Source management specification

Status: discovery draft; no implementation is authorized by this document.
Owner: fork-local until an upstreaming decision is made.
Evidence: `incident-report.md` and the current code at upstream `465811d` /
fork `182e640` (audited 2026-07-29).

## 1. Objective

Make source failures visible, distinguishable, and recoverable without risking
library membership or reading progress. A user must be able to answer:

1. Which sources are installed, missing, stale, unavailable, or unable to load?
2. How many library manga are affected?
3. What action is safe and likely to fix the problem?
4. Which information will be preserved if a source is removed or replaced?

Success is not "sources never break." Sites, source packages, settings, and
storage will fail. Success means the application detects the failure class,
preserves user data, and offers an honest next action.

## 2. Incident evidence

The 2026-07-29 incident contained four stacked problems:

| Problem | Evidence | Product requirement |
|---|---|---|
| A source was removed while 18 library rows still referenced it | The database retained the rows, but library construction dropped them because no loaded source matched the `source_id` | Missing-source entries remain visible; warn before uninstall; never silently delete rows |
| Corrupt settings removed the only list that offered the missing source | `source_lists` existed only in `settings.json` | Source-list configuration and last-known-good indexes must be recoverable |
| An installed source was three versions behind after the site changed its URL scheme | v8 was available while v5 failed with `Unknown error` | Compare installed and available versions; preserve causes; link failures to updates |
| A migrated WAL-mode database was copied without its matching WAL state | SQLite reported a malformed image | Any future migration/recovery workflow must use a transaction and emit a self-contained, integrity-checked database |

No library or reading-progress rows were lost in this incident. That fact is a
required invariant, not proof that the current UI is safe.

## 3. Verified current behavior and gaps

These are code observations, not proposed behavior:

- `SourceManager::uninstall_source` removes `<id>.aix` but leaves
  `.<id>.source`.
- Library queries use `filter_map` and omit rows when their source is not in
  the loaded collection.
- `load_all_sources` returns on the first `.aix` load error, so a bad package
  is not represented as a per-source diagnostic.
- Available lists are fetched live; there is no last-known-good index cache.
- A sidecar records only `source_of_source`, currently reduced to the list's
  domain. Distinct lists on `raw.githubusercontent.com` are therefore
  indistinguishable.
- Duplicate source ids are not resolved explicitly. Available results are
  concatenated and installation finds the first matching id in matching-domain
  lists; "highest version wins" is not current behavior.
- Installed-source listing includes only successfully loaded sources.
- Search already returns one structured error per source, but refresh jobs
  ultimately render repeated strings and can discard useful causes at the
  source/WASM boundary.
- Search results omit the source name in cover and grid modes.
- Source lists have no in-app management surface.

## 4. Scope

### Goals

- Preserve and display library entries when a source is missing or unloadable.
- Provide local, deterministic source status without requiring a network call.
- Make source provenance exact and resolve duplicate ids deterministically.
- Aggregate refresh and search outcomes by source.
- Support safe install, update, uninstall, and list management on-device.
- Define a guarded recovery path when source updates change canonical manga ids.

### Non-goals

- Silent source installation, update, migration, or library deletion.
- Periodic background polling.
- Scanning every downloaded file for routine health checks.
- Promising recovery when neither a usable package nor canonical content exists.
- Implementing source-to-source manga migration in the first release.

## 5. Design principles and invariants

1. Library membership and reading progress are user data. Source packages are
   replaceable dependencies.
2. A missing dependency may reduce functionality but must not hide or delete
   the user's data.
3. Local status and network freshness are separate. Offline state must be
   labelled with the index age rather than guessed.
4. Source identity is `source_id`; package provenance is an exact list URL,
   not a hostname.
5. External list JSON, source archives, redirects, and error text are
   untrusted inputs and must be validated at their boundaries.
6. Destructive or identity-changing actions require a preview, explicit user
   confirmation, an atomic operation, and post-operation verification.

## 6. Source status model

Status is a set of independent dimensions, not one exclusive enum.

| Dimension | Values | Local detection |
|---|---|---|
| Presence | `INSTALLED`, `MISSING` | referenced/known id versus `.aix` files |
| Load | `LOADED`, `LOAD_FAILED`, `NOT_APPLICABLE` | per-file load result with sanitized cause |
| Catalog | `AVAILABLE`, `UNAVAILABLE`, `UNKNOWN` | last-known-good indexes and their age |
| Freshness | `CURRENT`, `UPDATE_AVAILABLE`, `UNKNOWN` | installed version versus deterministically selected candidate |
| Runtime | `HEALTHY`, `FAILING`, `UNKNOWN` | persisted bounded operation summary; absence of failures is not proof of health |
| Compatibility | `COMPATIBLE`, `INCOMPATIBLE`, `UNKNOWN` | manifest/load validation |

Examples:

- Removed MangaKatana: `MISSING + AVAILABLE` if its exact list is cached;
  `MISSING + UNAVAILABLE` if no cached list offers it.
- Stale MangaFire: `INSTALLED + LOADED + UPDATE_AVAILABLE + FAILING`.
- Corrupt `.aix`: `INSTALLED + LOAD_FAILED`; it must not disappear from the
  management screen.
- No cached index while offline: catalog and freshness are `UNKNOWN`, never
  `UNAVAILABLE` or `CURRENT`.

Problem ordering in the UI is: `MISSING`, `LOAD_FAILED`/`INCOMPATIBLE`,
`FAILING`, `UPDATE_AVAILABLE`, `UNAVAILABLE`, healthy/unknown.

### Runtime failure evidence

The existing server is killed routinely, so an in-memory-only rolling window
cannot support a dependable health state. Store a small bounded summary per
source: operation class, success/failure, normalized error code, timestamp,
and a sanitized display message. Do not store manga payloads or raw remote
bodies.

The exact threshold remains a product decision. Required behavior is:

- one failure does not mark a source failing;
- repeated failures across more than one item or operation can;
- a successful later run can clear or age out the state;
- the UI shows the sample size and most recent time, not just a red label.

## 7. Catalog, provenance, and version selection

Add a last-known-good cache for each configured list containing:

- exact normalized list URL and a stable `list_id` derived from it;
- fetch time, HTTP validation metadata when supplied, and last fetch error;
- validated source entries with id, name, version, package URL, checksum when
  available, SDK/compatibility metadata, and list priority.

Refreshing one broken list must not discard other valid cached lists or the
last valid copy of that list.

When multiple lists offer the same source id, choose the install/update
candidate deterministically:

1. highest compatible numeric source version;
2. configured list priority as a tie-breaker;
3. normalized list URL as the final stable tie-breaker.

Show all providers in details. Installation requests must identify the exact
candidate (`list_id`, source id, version), and the installed sidecar must store
that exact provenance. Do not accept a domain string or re-run a first-match
search during installation.

If the selected candidate changes provider, the update confirmation names both
providers. A package must be downloaded successfully and validated before it
replaces the installed package. Replacement of the `.aix` and its sidecar must
behave as one recoverable operation.

## 8. Functional requirements

### FR-1: Inventory and orphan visibility

- Return every installed file, every load failure, and every source id
  referenced by `manga_library`.
- Return a library manga count for each id using grouped database queries.
- Library entries with missing/unloadable sources remain visible with a badge
  and retain offline actions that do not require the source.
- No routine source operation deletes library, chapter-state, playlist, or
  tracking rows.

Acceptance:

- Removing an `.aix` for a source with 18 manga still shows all 18 entries and
  one source-level warning after restart.
- A malformed `.aix` appears as `LOAD_FAILED` without preventing other sources
  from loading.

### FR-2: Safe uninstall

- The preview reports affected manga and explains that membership and reading
  progress remain stored but online refresh/read operations may stop.
- Confirmation is required when the count is greater than zero.
- Successful uninstall removes both `<id>.aix` and `.<id>.source`.
- Partial failure is reported with the exact remaining artifact and a retry
  action; no database cleanup is inferred.

Acceptance:

- Uninstall leaves neither artifact, and referenced manga remain visible as
  missing.

### FR-3: Honest error reporting

- Preserve structured error categories across source/WASM, use-case, job API,
  and Lua UI boundaries: `TIMEOUT`, `NETWORK`, `HTTP`, `PARSE`, `SOURCE_TRAP`,
  `INCOMPATIBLE`, `MISSING_SOURCE`, and `INTERNAL`.
- Aggregate refresh outcomes by source: attempted, succeeded, failed, skipped,
  representative cause, and affected manga ids for optional expansion.
- Never expose secrets, cookies, full response bodies, filesystem roots, or
  backtraces in the user message. Keep detailed diagnostic data in logs.
- A failing source with an available update leads with the update action.

Acceptance:

- Twenty equivalent failures produce one source summary, not twenty identical
  lines.
- Search distinguishes no results, timeout, and source error per source.

### FR-4: Source and list management

The device UI provides:

- a source screen with state dimensions, library count, installed/available
  version, exact provider, last operation, and contextual actions;
- a list screen to add, validate, prioritize, refresh, disable, and remove
  exact URLs;
- warnings showing which installed/missing sources lose catalog coverage when
  a list is disabled or removed;
- export/import of configured list URLs and priorities as part of settings
  recovery.

Removing a list never uninstalls a source. A list-fetch failure never erases
its last-known-good cache.

### FR-5: Search source identity and selection

- Every view mode shows the source name or a compact source badge without
  replacing read/unread metadata.
- Search accepts an explicit included-source id set; the existing exclusion
  setting remains readable for compatibility during migration.
- Results and errors are grouped by source, including zero-result outcomes.

### FR-6: Diagnosis

Diagnosis is explicit and on-demand. It may check list reachability, validate
the installed package, probe a source-defined base URL if available, and test
a small bounded sample of stored manga ids.

Redirects are evidence only. The app may report a probable canonical-id change
but must not rewrite ids during diagnosis.

### FR-7: Canonical-id recovery

Canonical-id migration is a later, separately approved capability. It requires:

- a source-specific resolver or verified redirects; never a guessed string
  transformation;
- a dry-run mapping and affected-row/count preview;
- collision detection across all composite keys;
- one database transaction covering every table keyed by the manga id;
- migration or deliberate invalidation/refetch of every filesystem cache keyed
  by the manga id, including `.posters`, with post-migration existence checks;
- database backup, pre/post row and read-state counts, and
  `PRAGMA integrity_check` result consumption;
- no chapter-id rewrite unless separately proven necessary;
- for offline replacement, a self-contained checkpointed/VACUUMed database and
  explicit WAL/SHM handling.

This requirement does not imply that arbitrary source-to-source migration is
safe or in scope.

## 9. Backend contract sketch

Contracts are additive to existing endpoints during transition. Field names
below follow the project's current snake_case JSON convention.

```text
GET    /sources/status
POST   /source-catalogs/refresh
GET    /source-lists
POST   /source-lists
PATCH  /source-lists/{list_id}
DELETE /source-lists/{list_id}
POST   /sources/{source_id}/diagnoses
POST   /sources/{source_id}/installations
DELETE /installed-sources/{source_id}?confirmed_library_count=N
```

`GET /sources/status` is local-only and returns one stable record per known
source id:

```json
{
  "data": [{
    "source_id": "multi.mangafire",
    "display_name": "MangaFire",
    "library_manga_count": 23,
    "presence": "INSTALLED",
    "load": "LOADED",
    "catalog": "AVAILABLE",
    "freshness": "UPDATE_AVAILABLE",
    "runtime": "FAILING",
    "compatibility": "COMPATIBLE",
    "installed_version": 5,
    "selected_candidate": {"list_id": "...", "version": 8},
    "catalog_checked_at": "2026-07-29T00:00:00Z",
    "last_operation": {"failed": 20, "attempted": 23, "at": "..."}
  }]
}
```

All new error responses use one predictable shape:

```json
{
  "error": {
    "code": "SOURCE_PACKAGE_INVALID",
    "message": "The downloaded source package could not be loaded.",
    "details": {"source_id": "multi.mangafire", "version": 8},
    "retryable": false
  }
}
```

Exact schemas, status codes, pagination needs, and compatibility shims must be
approved before implementation. In particular, the uninstall count parameter
is an optimistic-concurrency guard: return a conflict if the current affected
count differs from the previewed count.

## 10. User-flow requirements

### Missing source

1. User sees affected library entries and one source warning.
2. If a cached candidate exists: offer Install with version/provider details.
3. If catalog state is unknown: offer Refresh lists; retain offline status.
4. If unavailable: offer list management, export diagnostics, or deliberate
   library removal as a separate destructive flow.

### Failing source with update

1. Summary states failure ratio and installed/available versions.
2. User previews exact provider and update.
3. Package is downloaded and validated before replacement.
4. Refresh is retried on a small sample.
5. If ids changed, stop and offer diagnosis; do not silently migrate.

### Uninstall

1. Backend preview supplies current affected count.
2. UI explains consequences and preserved data.
3. Confirmed request includes the previewed count.
4. Backend rejects a stale preview and requires reconfirmation.

## 11. Phasing and gates

1. **Truthful inventory:** orphan visibility, per-file load results, sidecar
   cleanup, uninstall preview/count guard.
2. **Deterministic catalog:** exact provenance, last-known-good caches,
   duplicate resolution, update visibility.
3. **Structured operations:** typed errors, per-source aggregation, persisted
   bounded health evidence.
4. **Device management:** source/list screens and search identity/selection.
5. **Advanced recovery:** diagnosis first; canonical-id migration only after a
   separate design and data-safety review.

Each phase requires unit tests for state derivation and boundary validation,
backend contract tests, Lua UI tests where practical, full existing Rust/Lua
checks, and Kindle on-device verification before release.

## 12. Boundaries

- Always: preserve user rows; validate remote data; show exact source/provider;
  distinguish unknown from unavailable; verify destructive outcomes.
- Ask first: database schema changes, persisted health retention/thresholds,
  compatibility removal, migrations, new dependencies, or upstream API shape.
- Never: auto-delete orphan rows, auto-install/update, derive canonical ids by
  string replacement, copy a live WAL database as one file, or claim a failure
  is resolved without device verification.

## 13. Open decisions before planning

1. What bounded failure threshold and retention period should define
   `FAILING` on a routinely restarted server?
2. Should list priority be user-order, an explicit numeric value, or both?
3. Should the app retain the previous validated `.aix` for one-step rollback?
4. Which source/list-management pieces should be proposed upstream before
   fork-only implementation?
5. Is canonical-id migration worth building after truthful diagnosis exists,
   or should it remain an expert offline tool?
6. Which offline actions remain available for missing-source manga in the
   current library and reader architecture?

No implementation plan should be approved until these decisions and the API
schemas are reviewed.
