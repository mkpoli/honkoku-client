use super::*;
use std::fs;
fn query(text: &str) -> Query {
    Query {
        text: text.into(),
        mode: Mode::Strict,
        project: None,
        entry: None,
        limit: 20,
        cursor: None,
    }
}
fn page(entry: &str, project: &str, index: u64, text: &str) -> LivePage {
    LivePage {
        page_id: format!("{entry}_{index}"),
        project_id: project.into(),
        entry_id: entry.into(),
        index,
        text: text.into(),
        updated_at: 1,
        entry_label: "資料".into(),
        project_title: "計画".into(),
    }
}
#[test]
fn matching_offsets_facets_and_stable_cursor() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut builder = IndexBuilder::open(dir.path())?;
    builder.apply_live([
        page(
            "b",
            "p",
            1,
            "【右丁】\n𛀁《振り仮名：蝦夷｜えぞ》紀行\n候候候",
        ),
        page("a", "q", 2, "蝦夷紀行"),
        page("c", "p", 3, "蝦\n夷紀行"),
        page("d", "q", 4, "アィヌ國也"),
    ])?;
    let search = Searcher::open(dir.path())?;
    let results = search.search(query("蝦夷紀"))?;
    assert_eq!(results.total, 2);
    let occurrence = &results.hits[1].occurrences[0];
    assert_eq!(occurrence.matched, "蝦夷｜えぞ》紀");
    assert_eq!(occurrence.column, 0);
    let source: Vec<_> = "【右丁】\n𛀁《振り仮名：蝦夷｜えぞ》紀行\n候候候"
        .chars()
        .collect();
    assert_eq!(
        source[occurrence.original_start as usize..occurrence.original_end as usize]
            .iter()
            .collect::<String>(),
        occurrence.matched
    );
    assert_eq!(search.search(query("候候"))?.hits[0].occurrences.len(), 2);
    assert_eq!(search.search(query("候"))?.hits[0].occurrences.len(), 3);
    assert_eq!(
        search
            .search(Query {
                text: "あいぬ国なり".into(),
                mode: Mode::Folded,
                ..query("")
            })?
            .total,
        1
    );
    assert_eq!(search.search(query("アイヌ"))?.total, 0);
    let first = search.search(Query {
        limit: 1,
        ..query("蝦夷")
    })?;
    assert_eq!(first.total, 2);
    assert_eq!(first.facets, vec![("p".into(), 1), ("q".into(), 1)]);
    let cursor = first.next_cursor.clone();
    builder.apply_live([page("0", "p", 0, "蝦夷")])?;
    let second = search.search(Query {
        limit: 1,
        cursor: cursor.clone(),
        ..query("蝦夷")
    })?;
    assert_eq!(second.hits[0].page_id, "b_1");
    assert!(second.next_cursor.is_none());
    assert!(
        search
            .search(Query {
                cursor,
                ..query("候")
            })
            .is_err()
    );
    assert_eq!(
        search
            .search(Query {
                project: Some("q".into()),
                ..query("蝦夷")
            })?
            .total,
        1
    );
    assert_eq!(
        search
            .search(Query {
                entry: Some("b".into()),
                ..query("蝦夷")
            })?
            .total,
        1
    );
    assert!(search.search(query("蝦\n夷")).is_err());
    builder.remove("b_1")?;
    assert_eq!(search.search(query("候"))?.total, 0);
    builder.finish()?;
    Ok(())
}
#[test]
fn dump_incremental_overlay_and_recovery() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let dump = tempfile::tempdir()?;
    fs::create_dir_all(dump.path().join("v3/p/e"))?;
    fs::write(dump.path().join("v3/p/info.tsv"), "id\tlabel\ne\t資料名\n")?;
    fs::write(dump.path().join("v3/p/e/001.txt"), "蝦夷")?;
    fs::write(dump.path().join("v3/p/e/not-page.txt"), "無効")?;
    let mut builder = IndexBuilder::open(dir.path())?;
    let mut progress = Vec::new();
    let result = builder.from_dump(dump.path(), "abc", |p| progress.push(p))?;
    assert_eq!((result.done, result.total, result.indexed), (1, 1, 1));
    assert_eq!(progress.last().map(|p| p.done), Some(1));
    assert_eq!(builder.from_dump(dump.path(), "abc", |_| {})?.indexed, 0);
    assert_eq!(
        Searcher::open(dir.path())?.search(query("蝦夷"))?.hits[0].entry_label,
        "資料名"
    );
    builder.apply_live([page("e", "p", 1, "ライブ")])?;
    fs::write(dump.path().join("v3/p/e/001.txt"), "別の本文")?;
    assert_eq!(builder.from_dump(dump.path(), "def", |_| {})?.indexed, 0);
    builder.finish()?;
    let ledger = rusqlite::Connection::open(dir.path().join("pages.sqlite"))?;
    ledger.execute_batch("DELETE FROM pages; DELETE FROM checkpoint;")?;
    drop(ledger);
    let mut builder = IndexBuilder::open(dir.path())?;
    assert_eq!(builder.from_dump(dump.path(), "def", |_| {})?.indexed, 0);
    assert_eq!(
        Searcher::open(dir.path())?.search(query("ライブ"))?.total,
        1
    );
    assert_eq!(status(dir.path())?.commit.as_deref(), Some("def"));
    assert_eq!(status(dir.path())?.page_count, 1);
    builder.finish()?;
    Ok(())
}
#[test]
fn original_windows_and_whole_class_offsets() -> Result<()> {
    let source = "前".repeat(30) + "《割書：時｜候》" + &"後".repeat(30);
    let normalized = normalize::folded(&source);
    let occurrences = occurrences(&source, &normalized.text, "とき", &normalized.offsets)?;
    assert_eq!(occurrences.len(), 1);
    assert_eq!(occurrences[0].matched, "時");
    assert_eq!(occurrences[0].before.chars().count(), 20);
    assert_eq!(occurrences[0].after.chars().count(), 20);
    Ok(())
}

#[test]
fn metadata_with_literal_newlines_and_column_indentation() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let dump = tempfile::tempdir()?;
    fs::create_dir_all(dump.path().join("v3/p/e"))?;
    fs::write(
        dump.path().join("v3/p/info.tsv"),
        "id\tlabel\tattribution\tthumbnail\ne\t資料名\t図書館\nLibrary\timage\n",
    )?;
    fs::write(
        dump.path().join("v3/p/e/000.txt"),
        "前の行\r\n【左丁】\r\n  蝦夷",
    )?;
    let mut builder = IndexBuilder::open(dir.path())?;
    builder.from_dump(dump.path(), "version", |_| {})?;
    let result = Searcher::open(dir.path())?.search(query("蝦夷"))?;
    assert_eq!(result.hits[0].entry_label, "資料名");
    assert_eq!(result.hits[0].index, 0);
    assert_eq!(result.hits[0].occurrences[0].column, 1);
    builder.finish()?;
    Ok(())
}
