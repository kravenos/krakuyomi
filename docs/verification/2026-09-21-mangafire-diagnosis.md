# MangaFire saved-library reference mismatch

Status: diagnosed request-construction defect; no repair applied. 2026-09-21.

## Report and evidence

Corvin reports the same manga works through search but fails through the library.
Photos show `Failed to get page list / Unknown error` and 22 failed library
refreshes, with zero skipped. No specific HTTP status is exposed by those errors.

The manually supplied database passes SQLite quick_check and foreign_key_check.
Read-only inspection includes its supplied WAL. All 22 MangaFire library entries
use `/title/<id>-<slug>` keys; their 3,828 cached chapters use `chapter/<id>` keys.
The installed v8 source package is byte-identical to the earlier device backup:
SHA-256 `8bb0f63685005b50f936a307eb86815ad8765c29092844aff9a9c634bcc58ce7`.
The older backup already had these path-style keys. The date/reason this became
visible to the user has not been established; do not blame a recent package update.

## Offline reproduction

Ignored local diagnostic: `build/mangafire-reference-probe.py`.
Run `python build/mangafire-reference-probe.py --expect-valid` (requires Node).
Expected on the supplied snapshot: exit 1, with 22 malformed saved requests and
22 well-formed control requests for each of details, chapter-list and page-list.

The probe executes the actual installed WASM with minimal serialized Manga and
Chapter inputs matching the backend boundary. Host network functions capture the
URL and stop before sending. The database is opened read-only; database and WAL
hashes are checked for changes. No user identifiers are sent externally or printed.
This proves the generated paths differ, not that a repaired live download passed.

| Operation | Saved reference generates | Short-ID control generates |
|---|---|---|
| Details | `/api/titles//title/<id>-<slug>` | `/api/titles/<id>` |
| Chapters | `/api/titles//title/<id>-<slug>/chapters` | `/api/titles/<id>/chapters` |
| Pages | `/api/chapters/chapter/<id>` | `/api/chapters/<id>` |

The backend passes stored IDs unchanged at `BlockingSource::create_aidoku_manga`
and `create_aidoku_chapter`. The [public source implementation](https://github.com/Amqx/sources/blob/main/sources/multi.mangafire/src/lib.rs)
uses keys in these API paths and extracts short IDs from title deep links.
Executing the supplied WASM avoids assuming the current public source equals
the installed package.

Two library titles also have cached short-ID manga records; 329 cached chapters
match by short chapter ID and all 329 match chapter number. This is supporting
local evidence, not permission to match unrelated entries by title alone.

## Proposed repair boundary — not implemented

Prefer a narrowly scoped compatibility fix translating old MangaFire reference
formats at the source boundary, while keeping stored identities unchanged.
It must preserve 587 saved chapter-state rows (573 marked read) for these library
entries, downloaded-file lookups, playlists, tracking and library membership.
Returned chapter keys must remain compatible with each existing library entry;
changing only outbound requests could otherwise create duplicate chapters or
lose the association with saved progress. Fresh short-ID search results must keep
working. Never apply this normalization to other sources or arbitrary paths.

Before implementation: reproduce with synthetic fixtures, verify page/chapter
identity round trips and collision behavior, and keep the fix in its own fork PR.
After implementation: automated regressions, KindleHF build and manual device
comparison. Do not rewrite the database or tell Corvin to remove/re-add titles.
No cookies, library identifiers or chapter identifiers were transmitted externally.
