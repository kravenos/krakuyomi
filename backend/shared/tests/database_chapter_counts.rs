use shared::{
    database::Database,
    model::{ChapterId, ChapterInformation, ChapterState, MangaId},
};

fn chapter(manga_id: &MangaId, id: &str, number: f32) -> ChapterInformation {
    ChapterInformation {
        id: ChapterId::new(manga_id.clone(), id.to_owned()),
        title: None,
        scanlator: None,
        chapter_number: Some(number),
        volume_number: None,
        last_updated: None,
        thumbnail: None,
        lang: None,
        url: None,
        locked: None,
    }
}

#[tokio::test]
async fn cached_chapter_counts_report_actual_read_rows() {
    let temp_dir = tempfile::tempdir().expect("create temporary database directory");
    let database = Database::new(&temp_dir.path().join("rakuyomi.db"))
        .await
        .expect("open database");
    let manga_id = MangaId::from_strings("source".to_owned(), "manga".to_owned());
    let chapters = vec![
        chapter(&manga_id, "chapter-1", 1.0),
        chapter(&manga_id, "chapter-2", 2.0),
        chapter(&manga_id, "chapter-3", 3.0),
    ];

    database
        .upsert_cached_chapter_informations(&manga_id, &chapters)
        .await
        .expect("cache chapter information");
    database
        .upsert_chapter_state(
            &chapters[2].id,
            ChapterState {
                read: true,
                last_read: Some(1),
            },
        )
        .await
        .expect("mark one chapter read");

    let counts = database
        .get_cached_chapter_counts()
        .await
        .expect("load chapter counts");
    let manga_counts = counts
        .get(&("source".to_owned(), "manga".to_owned()))
        .expect("manga counts");

    assert_eq!(manga_counts.total, 3);
    assert_eq!(manga_counts.read, 1);
}
