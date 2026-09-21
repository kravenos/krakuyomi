# MangaFire saved-library compatibility

## Scope

Fork-only repair for saved MangaFire entries that fail to open or refresh while
fresh search results work. No release, upstream submission, database migration,
source installation, library re-addition, or device copying is part of this fix.

The installed next-SDK source, `multi.mangafire` version 8, expects bare manga
and chapter keys. An offline probe of the supplied WASM intercepted every HTTP
request before transmission: all 22 saved-library references produced malformed
details, chapter-list and page-list paths; short-key controls were well formed.
The package is byte-identical to the older backup, so this does not establish
that a recent source update caused the problem.

## Compatibility rule

| Boundary | Saved form | Source form |
| --- | --- | --- |
| Manga request | `/title/<hid>-<slug>` | `<hid>` |
| Chapter request for that saved parent | `chapter/<digits>` | `<digits>` |
| Returned manga | Original saved parent | Restored before caching |
| Returned numeric chapter | `chapter/<digits>` | Restored before caching |

Apply only to next-SDK MangaFire version 8 and the recognized saved parent form:
ASCII alphanumeric `hid`, nonempty ASCII alphanumeric/hyphen/underscore slug.
Other sources, package versions, SDKs, fresh short-key parents, and unrecognized
references retain their existing behavior. Future package versions require new
evidence before extending this compatibility rule.

Keep source metadata and nonnumeric chapter keys unchanged. Fail the operation
if restored chapter keys collide, before a caller can write a partial refresh.
The database, progress and downloaded-file naming rules are unchanged.

## Automated evidence

- Before implementation: [test-only CI run](https://github.com/kravenos/krakuyomi/actions/runs/35636665952)
  at `9cfc443` compiled and reproduced all five intended failures. Six MangaFire
  control tests passed; the shared suite reported 334 passed, 5 failed, 7 ignored.
- The fixtures execute real `BlockingSource` entry points through a synthetic
  WASM module. They inspect outgoing source descriptors and decode ordinary
  source results, without network traffic or private manga data.
- An additional test exercises the actual SQLite refresh operation and downloaded
  file lookup in a temporary directory. It checks that the original chapter row,
  read flag, read timestamp and stored file remain accessible without creating a
  second chapter namespace. This twelfth test was added after the before run.
- After implementation: verification pending. Do not treat this as device-tested.

## Manual Kindle acceptance

Use a combined KindleHF preview that also retains the accepted reader-direction
and cover-rendering changes. Corvin handles every backup and file transfer.

1. Exit KOReader fully. Back up the installed plugin and the entire `rakuyomi`
   data directory before replacing only the plugin with the preview.
2. Open an existing MangaFire library title directly, without searching or
   removing/re-adding it. Check its chapter list and open an unread chapter.
3. Refresh that title, then the library. Confirm the prior MangaFire failures
   disappear and chapter counts and reading progress remain plausible.
4. Open a previously downloaded chapter with Wi-Fi off. Re-enable Wi-Fi and
   download a new chapter through an existing library entry.
5. Search MangaFire and open a result. Check another source as well.
6. Restart KOReader. Confirm progress, downloads and reading direction persist.

Stop if progress disappears, duplicates appear, or chapter counts unexpectedly
drop. Do not remove/re-add titles or refresh repeatedly to repair them. Roll back
the plugin while KOReader is closed; restore the complete backed-up data folder
only if needed. Do not mix database files from different snapshots.

Device acceptance remains pending. The patch cannot establish live source
availability, actual download performance, or Kindle behavior by itself.
