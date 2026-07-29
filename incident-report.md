# Incident report - library loss and source failures (2026-07-29)

Handoff note for Codex and any other agent picking up rakuyomi-v2.
Companions: `plan.md` (plan of record), `source-management-spec.md` (design).

## Summary

A single user-visible symptom - "manga disappeared from my library and refresh
fails" - turned out to be **three unrelated failures stacked on top of each
other**. All three were silent, none was diagnosable from the app, and in
every case the information needed to detect the problem was already present
on the device.

All three are now resolved. No user data was lost: 45 library entries and
2,305 read chapters are intact.

---

## Problem 1 - Uninstalling a source silently orphans its library entries

**What happened.** `en.mangakatana` was removed. Its 18 rows stayed in
`manga_library`, so the manga vanished from the library view while remaining
in the database, and refresh failed for each of them.

**Why it was invisible.** Sources load from `sources/<source_id>.aix`, where
the filename is the source id. Nothing compares `manga_library.source_id`
against the installed set, so an orphaned entry is indistinguishable from a
healthy one until it is used.

**Contributing defect.** `SourceManager::uninstall_source`
(`backend/shared/src/source_manager.rs:72`) deletes only `<id>.aix` and leaves
the `.{id}.source` metadata sidecar behind permanently. The sources folder
therefore lists sources the app does not have. This directly caused a wrong
diagnosis during the investigation - the sidecars made mangakatana look
installed when it was not.

**Fix.** Reinstalled the source. Entries and progress recovered untouched.

---

## Problem 2 - Losing `settings.json` silently removes the ability to reinstall a source

**What happened.** `settings.json` was corrupted (truncated mid-string) and
reset to defaults. `source_lists` is the **first field** of the settings
struct, so it lived only in that file. The reset dropped the
`raw.githubusercontent.com` list that `en.mangakatana` came from - it was the
only source not from the two default lists, and the only one built for the
legacy Aidoku SDK (`is_next_sdk: false`).

The source then could not be reinstalled, with no explanation offered.

**Root cause of the corruption.** `Settings::save_to_file` used
`File::create` (which truncates immediately) and then serialized incrementally.
Any interruption - notably the auto-kill-server delay killing the process
mid-save - left a half-written file, and startup then aborted on the parse
error with no recovery path.

**Fix.** Settings are now written atomically (temp file, flush, fsync, rename)
and an unreadable settings file is preserved rather than blocking startup.
Commits `c36dd60`, `059fde9`, `befc938` on
`fix/settings-corruption-recovery`. **Still pending device testing.**

Source list restored by re-adding:
`https://raw.githubusercontent.com/Skittyblock/aidoku-community-sources/gh-pages/index.min.json`

---

## Problem 3 - A source can outlive its site, and the app cannot tell you

**What happened.** MangaFire changed its URL scheme:

- old `mangafire.to/manga/{slug}.{hash}`
- new `mangafire.to/title/{hash}-{slug}`

The installed source was **v5**, built for the old scheme. The site now
301-redirects old URLs, which v5 does not handle, so all 23 MangaFire manga
failed with `source error: Unknown error`.

**v8 - built for the new scheme - was already available** in the
`aidoku-community` list the user had configured. Confirmed by inspecting the
compiled wasm:

| version | `/manga/` | `/title/` |
|---------|-----------|-----------|
| v5 (installed) | 1 | 0 |
| v8 (available) | 0 | 2 |

**Why it was invisible.** Nothing compares the installed version against the
version offered by configured lists, so "your source is three versions behind
and that is why it is failing" was never surfaced. The error text discarded
the underlying cause and repeated one identical line per manga.

**Second-order problem.** Updating the source alone is not enough: v8 expects
`/title/...` ids while the database stored `/manga/...` ids. Both the source
and the stored ids had to change together.

**Fix.** Install v8, plus an offline id migration rewriting 4,681 rows:

```
manga_informations       23
chapter_informations   3953
manga_library            23
chapter_state           682
```

Chapter ids are numeric and site-internal (`chapter/4653697`) and were
unaffected, so all read progress survived.

**Important detail:** new ids must be **resolved by following the redirect**,
not derived by transforming the string. Two of 23 slugs had changed outright
(`a-distant-neighborhoodd` -> `haruka-na-machi-e`), so a mechanical rewrite
would have silently produced broken entries.

---

## Problem 4 - Copying a WAL-mode database corrupts it

**What happened.** After copying the migrated database to the device:
`error returned from database: (code: 11) database disk image is malformed`.

**Cause.** The database is in WAL mode. Replacing `database.db` while the
device still holds the previous `database.db-wal` and `database.db-shm` makes
SQLite replay a log belonging to a different database.

**Fix.** Ship a `VACUUM INTO` copy (single self-contained file, WAL collapsed)
and delete the `-wal`/`-shm` sidecars on the device before copying. The
migration script now does the vacuum step automatically.

**Process defect worth noting.** The migration script originally called
`PRAGMA integrity_check` without reading its result, so it reported "PASS"
based only on its own row counts. Fixed - it now reads the verdict and refuses
to emit a file that fails.

---

## Artifacts

All in `dist/` (git-excluded, local only):

| File | Purpose |
|------|---------|
| `diagnose-library.py` | Reports orphaned library entries and stale `.source` sidecars from a database plus a sources listing |
| `migrate-mangafire-ids.py` | Offline id migration; dry-run by default, never writes in place, emits a vacuumed copy |
| `mangafire-id-map.json` | The 23 resolved old -> new id pairs |
| `database-clean.db` | Migrated database, `integrity_check: ok`, ready for the device |
| `multi.mangafire-v8.aix` | The updated source |
| `en.mangakatana-v3.aix` | Legacy source, sideload fallback |

## Verification

- 45 library rows: mangakatana 18, mangafire 23, weebcentral 3, mangadex 1
- 2,305 read chapters preserved (1,238 of them MangaFire)
- MangaFire: 23 rows on new ids, 0 on old
- `PRAGMA integrity_check` = ok

## Outstanding

1. Device-test `fix/settings-corruption-recovery`, then merge and release.
2. Apply the database and source files per the steps above.
3. Back up `settings.json` once correct - source lists exist nowhere else.
4. Implement source management per `source-management-spec.md`. Phase 1
   (health detection and honest error reporting) would have reduced all three
   failures above to a readable message.

## Code defects found

| Location | Defect |
|----------|--------|
| `backend/shared/src/source_manager.rs:72` | `uninstall_source` leaves the `.{id}.source` sidecar behind |
| `backend/shared/src/settings/implementation.rs` | Non-atomic settings write (fixed) |
| `frontend/rakuyomi.koplugin/MangaSearchResults.lua:150` | Source name shown only in base/list mode, hidden in cover and grid |
| Source error path | Underlying cause discarded, surfaced as `Unknown error`, repeated per manga instead of aggregated per source |
| Settings UI | Source lists editable only by hand-editing `settings.json`; no in-app management |
