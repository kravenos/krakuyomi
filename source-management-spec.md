# Source management - design spec

Status: draft, not implemented. Fork-local document.
Companion to `plan.md` section 5.1.

## 1. Motivation

Three separate failures hit one library within a week. Each was silent, each
was diagnosable only by reading the database and the sources folder by hand,
and in every case **the information needed to detect it was already on the
device**.

| # | What happened | What the user saw |
|---|---|---|
| 1 | `en.mangakatana` was uninstalled; its 18 library rows stayed in the database | Manga vanished from the library. Refresh failed. Nothing said which source was gone. |
| 2 | `settings.json` was corrupted and reset, dropping the `source_lists` entry that mangakatana came from | The source could not be reinstalled, and nothing explained why it was no longer offered. |
| 3 | MangaFire changed its URL scheme; the installed source (v5) predates it, while v8 exists in a list the user already had configured | 20 lines of `source error: Unknown error`, with no hint that an update existed. |

The common thread is not that things broke - sources break, sites change,
schemas change, and that is normal. The problem is that **breakage is
indistinguishable from silence**. A source that is missing, a source that is
stale, and a source that is merely having a bad network day all present
identically.

This spec makes source state explicit, observable, and actionable.

## 2. Goals

1. Never lose library data because a source went away. Entries must remain
   recoverable and visibly explained.
2. Detect and name the three failure classes above, using local data first.
3. Turn `Unknown error` into a diagnosis with a suggested action.
4. Make sources manageable from the device, including source lists.
5. Warn before any action that would orphan library entries.

## 3. Non-goals

- Automatically installing or updating sources without user consent.
- Background network polling. All network work stays on explicit user action
  or the existing source-list refresh.
- Scanning the downloads folder. e-reader eMMC is slow enough that per-file
  scanning is unacceptable; every check here is a database query, a directory
  listing, or an already-cached index lookup.
- Recovering content from a source that no longer exists anywhere.

## 4. Source health model

The centre of the design. Every installed or referenced source resolves to
exactly one state, computed cheaply and refreshed when the user opens the
source screen.

| State | Meaning | Detection | Cost |
|-------|---------|-----------|------|
| `Ok` | Installed, current, no recent failures | default | free |
| `UpdateAvailable` | A configured list offers a higher version | installed version vs cached index | free |
| `Missing` | Library rows reference it, no `.aix` present | `manga_library.source_id` vs `sources/*.aix` | one query, one listing |
| `Unavailable` | Installed, but no configured list offers it, so it cannot be updated or reinstalled | installed id vs union of cached indexes | free |
| `Failing` | Recent operations failed above threshold | rolling in-memory counter | free |
| `Incompatible` | Loaded but rejected, e.g. `minAppVersion` exceeds the app | load result | free |

States compose. The MangaFire case is `UpdateAvailable` + `Failing`, which is
the single most valuable signal in this spec: *it is broken, and the fix is
already available to you*. That combination should be surfaced first and
phrased as an instruction, not a diagnosis.

`Missing` and `Unavailable` are deliberately distinct. Mangakatana was both,
but for different reasons and with different remedies: `Missing` is fixed by
installing, `Unavailable` is fixed by adding a source list.

### 4.1 Failure tracking

Source operations already return errors; they are simply discarded. Keep a
small per-source rolling window in memory (last N operations, N ~ 20) holding
outcome, timestamp, and the underlying error. No persistence is required - a
restart clearing the counters is acceptable and avoids database writes.

A source enters `Failing` when a majority of recent operations failed. One
timeout does not mark a source unhealthy; twenty consecutive failures do.

## 5. Error reporting

Current behaviour, from the incident: twenty lines of
`<title>: source error: Unknown error`. Every line is identical, none names
the source, and the underlying cause is discarded.

Required changes:

1. **Preserve the cause.** `Unknown error` means an error was swallowed
   somewhere between the WASM boundary and the dialog. Whatever the source
   returned - HTTP status, parse failure, trap - must survive to the report.
2. **Aggregate by source, not by manga.** Twenty failures from one source is
   one fact. Report `MangaFire: 20 of 23 manga failed to update` with the
   cause once, expandable to the list.
3. **Attach the health state.** When the failing source is also
   `UpdateAvailable`, the report leads with that:
   *"MangaFire is failing. Version 8 is available (you have 5). Update?"*
   with an inline action.
4. **Do not report unusable sources per-manga.** If a source is `Missing`,
   its manga do not each produce an error; the source produces one.

### 5.1 On-demand diagnosis

A `Diagnose` action per source, run only when the user asks, performing a
handful of network requests:

- fetch the source's base URL - distinguishes "site down" from "source broken"
- fetch one stored manga id and observe redirects

The second check is what identified failure 3: `/manga/vagabondd.4mx`
returning `301 -> /title/4mx-vagabondd` is unambiguous evidence that the site's
URL scheme moved and the installed source predates it. When a redirect pattern
is detected across several ids, report it as a probable scheme change and
point at the update.

## 6. Source management screen

Replaces the current installed-sources list, which shows names and an
uninstall action and nothing else.

```
Sources                                          [+ Add]  [Lists]

  MangaKatana                    18 manga    Ok           v3
  WeebCentral                     3 manga    Ok           v6
  MangaDex                        1 manga    Ok           v4
  MangaFire                      23 manga    Failing      v5 -> v8
      20 of 23 failed to update. Update available.
      [Update]  [Diagnose]  [Details]

  Not installed
  Comix                           0 manga    Unavailable  v2
      No configured source list offers this source.
      [Find a list]  [Remove]
```

Per row: name, library manga count, health state, installed version and
available version when they differ. Actions are contextual - `Update` only
when an update exists, `Install` for `Missing`, and so on.

Sorting puts problems first: `Missing`, then `Failing`, then
`UpdateAvailable`, then `Ok`. A healthy library shows a flat uneventful list,
which is the point.

The manga count per source is the number that makes consequences legible. It
is one grouped query over `manga_library`, and it is what turns "uninstall
this source" from an abstract action into "this will orphan 18 manga".

## 7. Source list management

Source lists are currently editable only by hand in `settings.json`. On a
Kindle that means unplugging, editing JSON, and remounting - and losing that
file, as happened here, silently removes the ability to install or update
whole sources.

Add a `Lists` screen:

- show configured list URLs, each with reachability and source count
- add and remove list URLs
- refresh a list on demand
- flag installed sources that no list covers - the `Unavailable` state,
  surfaced from the list side

This is also the natural place to warn about the settings dependency: source
lists live only in `settings.json`, so the file is worth backing up. Better
still, the app should treat loss of `source_lists` as recoverable by keeping
the last known good list set alongside the sources themselves.

## 8. Safety rails

1. **Uninstall confirmation.** If a source has library entries:
   *"Uninstall MangaKatana? 18 manga in your library use this source and will
   stop working until it is reinstalled. Your reading progress is kept."*
   The reassurance is as important as the warning - the data genuinely does
   survive, and users who do not know that will avoid legitimate cleanup or
   panic when it happens.
2. **Uninstall must remove the metadata sidecar.**
   `SourceManager::uninstall_source` (`source_manager.rs:72`) deletes only
   `<id>.aix`, leaving `.{id}.source` behind forever. The folder then lists
   sources the app does not have, which actively misleads anyone diagnosing by
   hand - it cost real time during this incident.
3. **Orphan visibility.** Library entries whose source is `Missing` stay in
   the library, shown with a badge rather than hidden, so the user can see
   what is affected instead of watching manga disappear.
4. **No silent destructive cleanup.** Never delete library rows because a
   source is absent.

## 9. Recovery: id migration

Failure 3 needed more than a source update: the site's manga ids changed
form, so stored ids no longer resolved. Rewriting them preserved 23 library
entries and 1,238 read chapters.

The general shape is worth having in the app, because sites will do this
again:

- detect the case (stored id redirects to a different canonical id)
- resolve each affected manga's canonical id by following the redirect
- rewrite `manga_id` across every table keyed by it, in one transaction
- never touch `chapter_id` unless it also changed - MangaFire's chapter ids
  are numeric and site-internal, so read progress survived untouched

Constraints learned from doing it manually:
- derive new ids by resolving them, not by transforming strings; two of the
  23 slugs had changed beyond the mechanical rule
  (`a-distant-neighborhoodd` -> `haruka-na-machi-e`)
- check for primary-key collisions before writing
- verify read-chapter counts before and after as an integrity check

Until this exists in-app, `dist/migrate-mangafire-ids.py` does it offline
against a copy of the database, and `dist/diagnose-library.py` reports
orphans and stale sidecars.

## 10. Search

Related, and part of why the incident was confusing to inspect:

1. **Show the source on every result.** `MangaSearchResults.lua:150` sets
   `post_text = is_cover and mandatory or manga.source.name`, so the source
   name appears only in base/list mode; in cover and grid mode it is replaced
   by read/unread indicators. When results from many sources are merged, not
   knowing where a result came from makes it impossible to tell a broken
   source from a source with no matches.
2. **Search a single source.** Filtering today is exclusion-only, via a
   persisted list (`exlucde_source_ids_select_search` - note the upstream
   typo) applied by the `Search*` button. Add positive selection.
3. **Report per-source search outcomes.** A source erroring during search
   should be distinguishable from one returning nothing.

## 11. Backend surface

Roughly what the frontend needs; names indicative.

```
GET  /sources/health
  -> [{ id, name, installed_version, available_version, state,
        library_manga_count, last_error }]

POST /sources/{id}/diagnose      # on-demand network probe
POST /sources/{id}/update        # install newest from configured lists
GET  /source-lists               # configured lists + reachability
POST /source-lists               # add
DELETE /source-lists/{index}     # remove
```

`/sources/health` must be answerable from local data alone: one grouped
query over `manga_library`, one listing of the sources folder, and the cached
list indexes. No filesystem scanning, no network.

## 12. Phasing

Ordered by value per unit of work; each phase is independently useful.

1. **Detection and honest errors.** Health states, per-source aggregation,
   preserve the underlying cause, surface `UpdateAvailable` on failure. This
   alone would have reduced all three incidents to a readable message.
2. **Safety rails.** Uninstall warning with manga count, remove the sidecar,
   orphan badges in the library.
3. **Management screen.** The list above, with contextual actions.
4. **Source list management.** The `Lists` screen.
5. **In-app diagnosis and id migration.** The expensive, rarely-needed parts.

## 13. Open questions

- Should `Failing` state persist across restarts? Persisting means database
  writes on a hot path; not persisting means the state resets whenever the
  server is killed, which the auto-kill-server delay makes routine.
- When two configured lists offer the same source at different versions
  (exactly the MangaFire case - tachibana-next had v5, aidoku-community had
  v8), which wins? Highest version is the obvious answer, but that silently
  changes which list a source comes from. At minimum this should be visible.
- Should the app keep a local cache of source `.aix` files it has installed,
  so an uninstall is reversible offline and a source dropped from all lists
  remains reinstallable?
- How much of this is upstreamable? Detection and honest errors are broadly
  useful; the id-migration machinery is more speculative.
