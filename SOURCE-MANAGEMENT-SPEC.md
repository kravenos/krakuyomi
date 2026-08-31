# RakuYomi Source Management Specification

- Status: Accepted for the fork-only v1.41.4 rebuild
- Baseline: upstream `v1.41.4` at `df0ef29fc07d87966a1a2558ab257743f29efaf4`
- Scope: source inventory, catalog, failure handling, management, search, and diagnosis
- Excluded: automatic source changes and canonical-id migration

This is the current source-management contract. The archived 2026-07-29
incident and discovery draft remain evidence, but this document supersedes their
code observations and resolves their open design choices for this rebuild.

## 1. Outcome

A user can tell which sources are installed, missing, broken, stale, or
unavailable; how many library entries each affects; and which safe action to
take. Source failures must reduce online capability without hiding or deleting
library membership, reading progress, playlists, tracking data, or downloads.

## 2. v1.41.4 audit

Upstream v1.41.4 materially changed source support. It now loads Aidoku,
LNReader, MangaYomi, and Keiyoushi packages, records a provider key, exposes
versions and runtime resource usage, returns per-source search errors, offers a
basic source-list screen, and removes the known package, metadata, and probe
files during uninstall.

The following gaps remain:

- one malformed package still stops the complete startup source scan;
- installed-source inventory contains only successfully loaded sources;
- library and playlist queries silently drop entries whose source is not loaded;
- uninstall has no affected-library preview or stale-count guard;
- list results have no durable last-known-good cache;
- provenance is a repository-style key rather than the exact normalized URL;
- duplicate candidates are displayed but not selected by an explicit shared rule;
- update and provider state are split across screens rather than one status record;
- refresh failures are still repeated per manga and lose useful categories;
- runtime health is not persisted across routine server restarts;
- grid search results do not consistently show source identity;
- search stores exclusions instead of a direct included-source selection;
- diagnosis is not an explicit bounded operation.

## 3. Invariants

- Treat library membership and progress as user data; treat source packages as
  replaceable dependencies.
- Never delete database rows as a side effect of source install, update,
  uninstall, list removal, list failure, or diagnosis.
- Never hide a library or playlist entry merely because its source is missing
  or broken.
- Distinguish unknown state from unavailable or healthy state.
- Validate remote indexes and packages as untrusted input.
- Never expose cookies, credentials, full response bodies, backtraces, or full
  filesystem paths in user-facing errors.
- Never install, update, migrate, or rewrite an identifier automatically.
- Never derive a canonical manga id by editing strings.
- Keep routine work bounded: grouped database counts, limited concurrency,
  bounded cache and health records, and no normal full-download scan.

## 4. Status contract

One local status record exists for every successfully loaded package, failed
package, and source id referenced by the library. Its dimensions are
independent:

| Dimension | Values |
|---|---|
| Presence | `installed`, `missing` |
| Load | `loaded`, `load_failed`, `not_applicable` |
| Catalog | `available`, `unavailable`, `unknown` |
| Freshness | `current`, `update_available`, `unknown` |
| Runtime | `healthy`, `failing`, `unknown` |
| Compatibility | `compatible`, `incompatible`, `unknown` |

Each record includes source id, best known display name, affected library manga
count, installed version, package kind and path label, exact provider URL when
known, selected candidate, cache age, last bounded operation summary, and a
sanitized load or runtime error. Problem ordering is missing, load failed or
incompatible, failing, update available, unavailable, then healthy or unknown.

## 5. Phase A: truthful local inventory

### Per-file loading

Scan supported package files in stable filename order. Load each independently.
A failed package produces a diagnostic and does not prevent later files from
loading. When one container registers several sources, preserve the common file
identity. Duplicate loaded ids are resolved deterministically by stable package
path; the rejected duplicate is reported and never silently replaces the first.

### Missing-source library entries

Library and playlist queries return cached entries even when no source is
loaded. Such entries use a placeholder source record containing the real source
id and a clear missing label. Offline-safe actions remain available: view cached
metadata, read downloaded chapters, change read state, remove from a playlist,
or deliberately remove from the library. Online refresh, search, source
settings, and new downloads fail with a structured `missing_source` error.

### Safe uninstall

The backend supplies the current affected library count before confirmation.
The confirmed delete request repeats that count. If it changed, return a
conflict and require a fresh preview. A successful uninstall removes every
known package, metadata, and probe file belonging to the selected package,
verifies their absence, unloads all sources registered by that package, and
leaves all database rows intact. Partial deletion reports the remaining file
label and offers a retry.

Acceptance:

- one corrupt package appears as `load_failed` while valid packages still work;
- deleting a package with 18 library manga still returns all 18 entries after a
  restart;
- a stale uninstall preview cannot remove a package;
- successful uninstall leaves no known sidecar or probe artifact and no user
  database row is deleted.

## 6. Phase B: deterministic catalog

Store one last-known-good cache per configured source list. Each cache contains
the exact normalized URL, a stable id derived from that URL, configured order,
fetch time, validation metadata when available, last fetch error, and validated
candidate fields needed for display and installation. A failed refresh retains
the previous valid cache and does not affect another list.

List order is the user-controlled priority. For one source id, select:

1. the highest compatible version using the source format's version rules;
2. the earliest enabled list in user order;
3. the normalized list URL as a stable final tie-breaker.

Show every provider in details. Install and update requests identify the exact
list id, source id, and version. Store the exact normalized provider URL beside
the installed package. Download and validate a replacement before publishing
it. Keep the prior package until the replacement loads, then remove the rollback
copy after verification.

The list screen can add, validate, reorder, refresh, disable, remove, export,
and import exact URLs. Disabling or removing a list previews installed or
missing sources that lose catalog coverage. A bulk import cannot bypass this
preview: active lists must be disabled or removed individually first. List
management never uninstalls a source.

## 7. Phase C: structured operations and health

Use these user-facing categories across source, use-case, job API, and Lua UI
boundaries: `timeout`, `network`, `http`, `parse`, `source_trap`,
`incompatible`, `missing_source`, and `internal`.

Aggregate refresh results per source with attempted, succeeded, failed, skipped,
representative category and message, and optionally expandable manga ids.
Equivalent failures produce one summary. Search separately reports no results,
timeout, and source failure.

Persist only a bounded health summary per source: operation class, normalized
category, sanitized message, timestamp, attempted item key hash, and success or
failure. Retain the newest 20 observations for 14 days. Mark a source `failing`
only after at least three failures across at least two distinct items or
operation classes within 24 hours. A later successful operation clears the
active failing state while retaining bounded history until expiry. The UI shows
the sample count and latest time.

## 8. Phase D: device management and search

The source screen shows the status dimensions, library count, installed and
available versions, exact provider, cache age, last operation, and contextual
safe actions.

Every search view mode shows a compact source name without replacing existing
read/unread metadata. Search accepts an explicit included-source id set. During
migration, the legacy exclusion setting remains readable and is converted once
to the equivalent included set. Results, zero-result outcomes, and errors are
grouped by source.

## 9. Phase E: bounded diagnosis

Diagnosis is manual and read-only. It may refresh one configured list, validate
one installed package, probe a source-declared base URL, and test at most three
stored manga ids. Every step has a timeout and reports a structured outcome.
Redirects are evidence only. Diagnosis may report a probable identifier change
but never rewrites ids or files.

The diagnosis response never contains stored manga ids, redirect targets,
credential-bearing URLs, or local paths. It reports only safe categories,
bounded messages, HTTP status numbers, package filenames, durations, and an
ordinal for each of at most three manga checks. Source-list refresh may replace
only its disposable cache; diagnosis never writes library rows or identifiers.

Canonical-id migration remains an expert offline recovery capability. It is not
part of this rebuild because no general resolver and collision-safe migration
contract exists. Any future implementation needs a separate specification,
dry-run mapping, verified backup, one database transaction, cache handling,
collision checks, row-count checks, and consumed SQLite integrity results.

## 10. Interfaces

Additive endpoints may be introduced behind the existing local server boundary:

```text
GET    /sources/status
GET    /installed-sources/{source_id}/uninstall-preview
DELETE /installed-sources/{source_id}?confirmed_library_count=N
POST   /source-catalogs/refresh
GET    /source-catalogs/status
POST   /source-catalogs/validate
POST   /source-catalogs/{list_id}/refresh
GET    /source-catalogs/{list_id}/change-preview
POST   /sources/{source_id}/diagnoses
```

Existing settings routes remain the source-list management write boundary.
Exact request and response structs live with each implementation PR. New error
responses use the existing server error envelope while adding a stable code,
safe message, retryable flag, and allowlisted details.

## 11. Delivery and verification

Deliver these as separate fork-only PRs in order:

1. per-file loading and truthful inventory;
2. missing-source library and playlist visibility;
3. uninstall preview and stale-count guard;
4. catalog cache, exact provenance, deterministic selection, and update state;
5. structured operation summaries and bounded health;
6. management UI and included-source search;
7. bounded diagnosis.

Each PR starts from the latest accepted fork `main`, contains its own tests and
translations, and adds no unrelated cleanup. Use grouped queries and fixed
bounds in tests. Run Rust formatting, strict compiler checks, all Rust tests,
Lua checks, relevant package builds, diff review, and secret/path review.
Runtime changes are included in the final exact-commit KindleHF candidate. No
GitHub release or upstream PR is created by this program.

## 12. Rollback

Code rollback is the PR's parent commit. Settings/cache additions must be
backward compatible or ignored by older code. Package replacement retains the
previous validated file until the new package loads. Rollback never restores a
database by copying only its main file while WAL mode is active.
