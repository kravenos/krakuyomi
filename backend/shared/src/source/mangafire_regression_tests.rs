//! Synthetic source-boundary regressions. No network or user-library fixtures.
//!
//! The WAT module forwards source exports to fixture callbacks. Those callbacks
//! inspect the real host descriptors and return normal postcard/legacy results,
//! leaving request adaptation, result decoding and model conversion under test.

use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use tempfile::{tempdir, TempDir};
use tokio_util::sync::CancellationToken;
use wasmi::{Caller, Engine, Extern, Linker, Module, Store};
use zip::{write::SimpleFileOptions, ZipWriter};

use super::{
    model::{Chapter, Manga},
    wasm_store::{ObjectValue, Value, WasmStore},
    BlockingSource,
};
use crate::{
    model::{ChapterId, ChapterInformation, MangaId},
    settings::Settings,
    source_manager::SourceManager,
};

const SOURCE: &str = "multi.mangafire";
const SHORT: &str = "ab12cd";
const SAVED: &str = "/title/ab12cd-synthetic-title";

#[derive(Clone, Debug, PartialEq)]
struct Request {
    operation: &'static str,
    manga: String,
    chapter: Option<String>,
}

struct Fixture {
    _directory: TempDir,
    source: BlockingSource,
    requests: Arc<Mutex<Vec<Request>>>,
}

fn next_manga(key: &str) -> aidoku::Manga {
    // The ordinary constructor fills all unrelated metadata exactly as it does
    // for a real request; the synthetic response supplies observable metadata.
    let mut manga = BlockingSource::create_aidoku_manga(key.to_owned());
    manga.title = "Synthetic title".to_owned();
    manga.chapters = Some(
        ["101", "chapter/102", "opaque"]
            .into_iter()
            .enumerate()
            .map(|(index, key)| {
                let mut chapter = BlockingSource::create_aidoku_chapter(key.to_owned());
                chapter.chapter_number = Some(index as f32 + 1.0);
                chapter.title = Some(format!("Chapter {}", index + 1));
                chapter
            })
            .collect(),
    );
    manga
}

fn write_next(caller: &mut Caller<'_, WasmStore>, result: &impl Serialize) -> i32 {
    let bytes = postcard::to_allocvec(result).expect("serialize fixture response");
    let length = u32::try_from(bytes.len()).unwrap().to_le_bytes();
    let mut result = Vec::from(length);
    result.extend_from_slice(&length);
    result.extend(bytes);
    let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
        panic!("fixture memory export missing");
    };
    memory
        .write(caller, 64, &result)
        .expect("write source response");
    64
}

fn fixture(source_id: &str, version: u32, next_sdk: bool) -> Fixture {
    fixture_with_chapter_keys(
        source_id,
        version,
        next_sdk,
        &["101", "chapter/102", "opaque"],
    )
}

fn fixture_with_chapter_keys(
    source_id: &str,
    version: u32,
    next_sdk: bool,
    chapter_keys: &[&str],
) -> Fixture {
    let directory = tempdir().unwrap();
    let package = directory.path().join("fixture.aix");
    let mut archive = ZipWriter::new(File::create(&package).unwrap());
    archive
        .start_file("Payload/source.json", SimpleFileOptions::default())
        .unwrap();
    archive
        .write_all(
            serde_json::to_string(&serde_json::json!({
                "info": { "id": source_id, "name": "Synthetic fixture", "version": version }
            }))
            .unwrap()
            .as_bytes(),
        )
        .unwrap();
    archive
        .start_file("Payload/main.wasm", SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"\0asm\x01\0\0\0").unwrap();
    archive.finish().unwrap();
    let settings = Settings::default();
    let manager = Arc::new(tokio::sync::Mutex::new(SourceManager::new(
        directory.path().to_path_buf(),
        HashMap::new(),
        settings.clone(),
    )));
    let mut source =
        BlockingSource::from_aix_file(&package, &manager.blocking_lock(), &manager).unwrap();
    let engine = Engine::default();
    let mut store = Store::new(
        &engine,
        WasmStore::new(
            source_id.to_owned(),
            source.source_settings.clone().unwrap(),
            settings,
        ),
    );
    let mut linker = Linker::new(&engine);
    let requests = Arc::new(Mutex::new(Vec::new()));

    let captured = requests.clone();
    let mut response = next_manga(SHORT);
    response.chapters = Some(
        chapter_keys
            .iter()
            .map(|key| {
                let mut chapter = BlockingSource::create_aidoku_chapter((*key).to_owned());
                chapter.chapter_number = Some(1.0);
                chapter
            })
            .collect(),
    );
    linker
        .func_wrap(
            "fixture",
            "update",
            move |mut caller: Caller<'_, WasmStore>,
                  manga: i32,
                  details: i32,
                  _chapters: i32|
                  -> i32 {
                let value = caller.data().get_std_value(manga as usize).unwrap();
                let Value::NextManga(manga) = value.as_ref() else {
                    panic!("expected next Manga");
                };
                captured.lock().unwrap().push(Request {
                    operation: if details != 0 { "details" } else { "chapters" },
                    manga: manga.key.clone(),
                    chapter: None,
                });
                write_next(&mut caller, &response)
            },
        )
        .unwrap();
    let captured = requests.clone();
    linker
        .func_wrap(
            "fixture",
            "pages",
            move |mut caller: Caller<'_, WasmStore>, manga: i32, chapter: i32| -> i32 {
                let manga = caller.data().get_std_value(manga as usize).unwrap();
                let chapter = caller.data().get_std_value(chapter as usize).unwrap();
                let Value::NextManga(manga) = manga.as_ref() else {
                    panic!("expected next Manga");
                };
                let Value::NextChapter(chapter) = chapter.as_ref() else {
                    panic!("expected next Chapter");
                };
                captured.lock().unwrap().push(Request {
                    operation: "pages",
                    manga: manga.key.clone(),
                    chapter: Some(chapter.key.clone()),
                });
                write_next(&mut caller, &Vec::<aidoku::Page>::new())
            },
        )
        .unwrap();
    let captured = requests.clone();
    let legacy_source = source_id.to_owned();
    linker
        .func_wrap(
            "fixture",
            "legacy",
            move |mut caller: Caller<'_, WasmStore>, descriptor: i32, operation: i32| -> i32 {
                let value = caller.data().get_std_value(descriptor as usize).unwrap();
                let Value::Object(ObjectValue::ValueMap(values)) = value.as_ref() else {
                    panic!("expected legacy map");
                };
                let string = |key: &str| match values.get(key).unwrap() {
                    Value::String(value) => value.clone(),
                    _ => panic!("expected string field"),
                };
                let manga = string(if operation == 2 { "mangaId" } else { "id" });
                captured.lock().unwrap().push(Request {
                    operation: ["details", "chapters", "pages"][operation as usize],
                    manga: manga.clone(),
                    chapter: (operation == 2).then(|| string("id")),
                });
                let result = match operation {
                    0 => Value::Object(ObjectValue::Manga(Manga {
                        id: manga,
                        source_id: legacy_source.clone(),
                        ..Manga::default()
                    })),
                    1 => Value::Array(vec![Value::Object(ObjectValue::Chapter(Chapter {
                        id: "chapter/101".to_owned(),
                        manga_id: manga,
                        source_id: legacy_source.clone(),
                        ..Chapter::default()
                    }))]),
                    _ => Value::Array(Vec::new()),
                };
                caller.data_mut().store_std_value(result.into(), None) as i32
            },
        )
        .unwrap();

    let exports = if next_sdk {
        r#"(func (export "get_manga_update") (param i32 i32 i32) (result i32)
               local.get 0 local.get 1 local.get 2 call $update)
           (func (export "get_page_list") (param i32 i32) (result i32)
               local.get 0 local.get 1 call $pages)"#
    } else {
        r#"(func (export "get_manga_details") (param i32) (result i32) local.get 0 i32.const 0 call $legacy)
           (func (export "get_chapter_list") (param i32) (result i32) local.get 0 i32.const 1 call $legacy)
           (func (export "get_page_list") (param i32) (result i32) local.get 0 i32.const 2 call $legacy)"#
    };
    let wat = format!(
        r#"(module
        (import "fixture" "update" (func $update (param i32 i32 i32) (result i32)))
        (import "fixture" "pages" (func $pages (param i32 i32) (result i32)))
        (import "fixture" "legacy" (func $legacy (param i32 i32) (result i32)))
        (memory (export "memory") 1)
        {exports})"#
    );
    let module = Module::new(&engine, wat.as_bytes()).expect("parse synthetic source WAT");
    source.instance = Some(linker.instantiate_and_start(&mut store, &module).unwrap());
    source.store = Some(store);
    source.next_sdk = next_sdk;
    Fixture {
        _directory: directory,
        source,
        requests,
    }
}

#[test]
fn mangafire_saved_details_send_short_key_and_keep_library_identity() {
    let mut fixture = fixture(SOURCE, 8, true);
    let manga = fixture
        .source
        .get_manga_details(CancellationToken::new(), SAVED.to_owned())
        .unwrap();
    assert_eq!(
        fixture.requests.lock().unwrap()[0].manga,
        SHORT,
        "details must send the source's short key"
    );
    assert_eq!(
        manga.id, SAVED,
        "details must not move a saved library entry to a new identity"
    );
    assert_eq!(manga.title.as_deref(), Some("Synthetic title"));
}

#[test]
fn mangafire_saved_chapters_send_short_key_and_retain_saved_chapter_keys() {
    let mut fixture = fixture(SOURCE, 8, true);
    let chapters = fixture
        .source
        .get_chapter_list(CancellationToken::new(), SAVED.to_owned())
        .unwrap();
    assert_eq!(
        fixture.requests.lock().unwrap()[0].manga,
        SHORT,
        "chapter list must send the source's short key"
    );
    assert_eq!(
        chapters
            .iter()
            .map(|chapter| chapter.id.as_str())
            .collect::<Vec<_>>(),
        ["chapter/101", "chapter/102", "opaque"]
    );
    assert!(chapters.iter().all(|chapter| chapter.manga_id == SAVED));
    assert_eq!(chapters[0].chapter_num, Some(1.0));
}

#[test]
fn mangafire_saved_pages_send_short_manga_and_numeric_chapter_keys() {
    let mut fixture = fixture(SOURCE, 8, true);
    fixture
        .source
        .get_page_list(
            CancellationToken::new(),
            SAVED.to_owned(),
            "chapter/101".to_owned(),
            Some(1.0),
        )
        .unwrap();
    assert_eq!(
        fixture.requests.lock().unwrap().as_slice(),
        &[Request {
            operation: "pages",
            manga: SHORT.to_owned(),
            chapter: Some("101".to_owned()),
        }]
    );
}

#[test]
fn mangafire_refreshed_chapters_still_find_saved_progress_and_download_keys() {
    let mut fixture = fixture(SOURCE, 8, true);
    let saved_id = ChapterId::from_strings(
        SOURCE.to_owned(),
        SAVED.to_owned(),
        "chapter/101".to_owned(),
    );
    let progress = HashMap::from([(saved_id.clone(), 17_usize)]);
    let downloads = HashMap::from([(saved_id.clone(), "existing-chapter.cbz")]);
    let chapters = fixture
        .source
        .get_chapter_list(CancellationToken::new(), SAVED.to_owned())
        .unwrap();
    let refreshed = ChapterInformation::from(chapters[0].clone());
    assert_eq!(
        refreshed.id, saved_id,
        "refresh must keep the existing composite identity"
    );
    assert_eq!(progress.get(&refreshed.id), Some(&17));
    assert_eq!(downloads.get(&refreshed.id), Some(&"existing-chapter.cbz"));
    let fresh_id = ChapterId::new(
        MangaId::from_strings(SOURCE.to_owned(), SHORT.to_owned()),
        "101".to_owned(),
    );
    assert_ne!(
        refreshed.id, fresh_id,
        "compatibility must not merge saved and fresh library namespaces"
    );
}

#[test]
fn mangafire_fresh_short_ids_keep_the_source_result_keys() {
    let mut fixture = fixture(SOURCE, 8, true);
    let manga = fixture
        .source
        .get_manga_details(CancellationToken::new(), SHORT.to_owned())
        .unwrap();
    let chapters = fixture
        .source
        .get_chapter_list(CancellationToken::new(), SHORT.to_owned())
        .unwrap();
    fixture
        .source
        .get_page_list(
            CancellationToken::new(),
            SHORT.to_owned(),
            "101".to_owned(),
            None,
        )
        .unwrap();
    assert_eq!(manga.id, SHORT);
    assert_eq!(chapters[0].id, "101");
    assert!(chapters.iter().all(|chapter| chapter.manga_id == SHORT));
    assert!(fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|request| request.manga == SHORT));
    assert_eq!(
        fixture.requests.lock().unwrap()[2].chapter.as_deref(),
        Some("101")
    );
}

#[test]
fn mangafire_fresh_parent_does_not_reinterpret_a_path_style_chapter() {
    let mut fixture = fixture(SOURCE, 8, true);
    fixture
        .source
        .get_page_list(
            CancellationToken::new(),
            SHORT.to_owned(),
            "chapter/101".to_owned(),
            None,
        )
        .unwrap();
    assert_eq!(
        fixture.requests.lock().unwrap()[0].chapter.as_deref(),
        Some("chapter/101")
    );
}

#[test]
fn unrelated_source_and_older_mangafire_version_keep_existing_contract() {
    for (source_id, version) in [("other.source", 8), (SOURCE, 5)] {
        let mut fixture = fixture(source_id, version, true);
        fixture
            .source
            .get_manga_details(CancellationToken::new(), SAVED.to_owned())
            .unwrap();
        let chapters = fixture
            .source
            .get_chapter_list(CancellationToken::new(), SAVED.to_owned())
            .unwrap();
        fixture
            .source
            .get_page_list(
                CancellationToken::new(),
                SAVED.to_owned(),
                "chapter/101".to_owned(),
                None,
            )
            .unwrap();
        assert!(fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|request| request.manga == SAVED));
        assert_eq!(
            fixture.requests.lock().unwrap()[2].chapter.as_deref(),
            Some("chapter/101")
        );
        assert_eq!(
            chapters[0].id, "101",
            "unrelated response IDs must not be prefixed"
        );
    }
}

#[test]
fn mangafire_legacy_sdk_keeps_path_based_inputs_and_outputs() {
    let mut fixture = fixture(SOURCE, 8, false);
    let manga = fixture
        .source
        .get_manga_details(CancellationToken::new(), SAVED.to_owned())
        .unwrap();
    let chapters = fixture
        .source
        .get_chapter_list(CancellationToken::new(), SAVED.to_owned())
        .unwrap();
    fixture
        .source
        .get_page_list(
            CancellationToken::new(),
            SAVED.to_owned(),
            "chapter/101".to_owned(),
            None,
        )
        .unwrap();
    assert_eq!(manga.id, SAVED);
    assert_eq!(chapters[0].id, "chapter/101");
    assert!(fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|request| request.manga == SAVED));
    assert_eq!(
        fixture.requests.lock().unwrap()[2].chapter.as_deref(),
        Some("chapter/101")
    );
}

#[test]
fn mangafire_malformed_or_unrecognized_parent_keys_are_not_reinterpreted() {
    for key in [
        "/title/",
        "/title/ab12cd",
        "/title/-slug",
        "/title/ab12cd-",
        "/title/ab12cd-slug/extra",
        "/title/ab12cd-slug?query=1",
        "/title/ab12cd-slug#fragment",
        "/title/ab%2fcd-slug",
        "https://example.invalid/title/ab12cd-slug",
        "title/ab12cd-slug",
        "/title/../escape",
    ] {
        let mut fixture = fixture(SOURCE, 8, true);
        let result = fixture
            .source
            .get_manga_details(CancellationToken::new(), key.to_owned());
        let requests = fixture.requests.lock().unwrap();
        if result.is_err() {
            assert!(
                requests.is_empty(),
                "rejection must happen before sending an altered key"
            );
        } else {
            assert_eq!(requests[0].manga, key);
        }
    }
}

#[test]
fn mangafire_malformed_chapter_keys_are_not_reinterpreted() {
    for chapter in [
        "chapter/",
        "chapter/12/34",
        "chapter/12?x=1",
        "chapter/-12",
        "chapter/1.5",
        "chapter/abc",
        "/chapter/101",
    ] {
        let mut fixture = fixture(SOURCE, 8, true);
        let result = fixture.source.get_page_list(
            CancellationToken::new(),
            SAVED.to_owned(),
            chapter.to_owned(),
            None,
        );
        let requests = fixture.requests.lock().unwrap();
        if result.is_err() {
            assert!(requests.is_empty());
        } else {
            assert_eq!(requests[0].chapter.as_deref(), Some(chapter));
        }
    }
}

#[test]
fn mangafire_chapter_key_collision_fails_without_dropping_either_chapter() {
    let mut fixture = fixture_with_chapter_keys(SOURCE, 8, true, &["42", "chapter/42"]);
    let result = fixture
        .source
        .get_chapter_list(CancellationToken::new(), SAVED.to_owned());
    assert!(
        result.is_err(),
        "ambiguous chapter keys must fail rather than overwrite progress or discard a chapter"
    );
}
