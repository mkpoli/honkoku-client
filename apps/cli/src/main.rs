use clap::{Parser, Subcommand, ValueEnum};
use honkoku_core::{
    HonkokuClient,
    auth::{FileStore, KeyringStore, SessionStore, TokenManager, import_dev_session},
    cache::{SharedStorage, blocking},
    editing::SaveOptions,
    home::{RankingSort, TimelineFilter},
    model::PageStatus,
};
use honkoku_storage::Storage;
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};
use unicode_width::UnicodeWidthStr;

#[derive(Parser)]
#[command(name = "honkoku", version, about = "みんなで翻刻の公開資料を読む")]
struct Args {
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true, help = "キャッシュを更新")]
    refresh: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Edit {
        #[command(subcommand)]
        command: EditCommand,
    },
    Projects,
    Login {
        #[arg(long, required = true)]
        import: bool,
    },
    Whoami,
    Notifications {
        #[arg(long)]
        count: bool,
    },
    Timeline {
        #[arg(long)]
        project: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    Ranking {
        #[arg(long, value_enum, default_value_t = Sort::Exp)]
        by: Sort,
    },
    Announcements,
    Project {
        id: String,
        #[arg(long)]
        progress: bool,
    },
    Collection {
        id: String,
        #[arg(long)]
        progress: bool,
        #[arg(long, conflicts_with = "progress")]
        entries: bool,
        #[arg(long, requires = "entries")]
        entry_progress: bool,
    },
    Entry {
        id: String,
    },
    Pages {
        #[arg(value_name = "entryId")]
        entry_id: String,
        #[arg(long)]
        status: bool,
    },
}
#[derive(Subcommand)]
enum EditCommand {
    Lock {
        #[arg(value_name = "entryId")]
        entry_id: String,
        index: u32,
        #[arg(long)]
        sync: bool,
    },
    Draft {
        #[arg(value_name = "entryId")]
        entry_id: String,
        index: u32,
        #[arg(long)]
        text_file: std::path::PathBuf,
    },
    Save {
        #[arg(value_name = "entryId")]
        entry_id: String,
        index: u32,
        #[arg(long, value_enum)]
        status: Option<SaveStatus>,
        #[arg(long)]
        share: bool,
        #[arg(long)]
        request_review: bool,
        #[arg(long, default_value = "")]
        comment: String,
    },
    Discard {
        #[arg(value_name = "entryId")]
        entry_id: String,
        index: u32,
    },
    Status {
        #[arg(value_name = "entryId")]
        entry_id: String,
        index: u32,
    },
}
#[derive(Clone, Copy, ValueEnum)]
enum SaveStatus {
    Initiated,
    Completed,
}
#[derive(Clone, Copy, ValueEnum)]
enum Sort {
    Exp,
    #[value(name = "charCount")]
    CharCount,
    #[value(name = "likeCount")]
    LikeCount,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let dirs = directories::BaseDirs::new().ok_or("cache directory unavailable")?;
    let data_dir = std::env::var_os("XDG_DATA_HOME")
        .filter(|p| !p.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| dirs.data_dir().to_owned());
    let store = Arc::new(FileStore::new(data_dir.join("honkoku-client/session.json")));
    if let Command::Login { import: true } = args.command {
        let session = import_dev_session(
            dirs.home_dir()
                .join(".local/share/honkoku-client/session.json"),
        )?;
        store.save(&session)?;
        let uid = session.uid.clone();
        let keyring = tokio::task::spawn_blocking(move || KeyringStore::new(&uid)).await?;
        if let Err(error) = keyring {
            eprintln!("Keyring unavailable ({error}); using FileStore (0600 on Unix).");
        }
        let identity = serde_json::json!({"uid":session.uid,"displayName":session.display_name,"email":session.email,"providers":session.providers});
        return write_output(if args.json {
            pretty(&identity)?
        } else {
            format!(
                "Signed in as {} ({})\n",
                session
                    .display_name
                    .as_deref()
                    .or(session.email.as_deref())
                    .unwrap_or(&session.uid),
                session.uid
            )
        });
    }
    let cache = dirs.cache_dir().join("honkoku-client");
    std::fs::create_dir_all(&cache)?;
    let storage: SharedStorage = Arc::new(Mutex::new(Storage::open(cache.join("cache.sqlite"))?));
    let mut client = HonkokuClient::new()?.with_storage(storage.clone());
    if let Some(session) = store.load()? {
        client = client.with_session(TokenManager::new(session, store)?);
    }
    let output = match args.command {
        Command::Edit { command } => match command {
            EditCommand::Lock {
                entry_id,
                index,
                sync,
            } => {
                let session = client.lock_page(&entry_id, index, sync).await?;
                if args.json {
                    pretty(session.page())?
                } else {
                    format!("Locked {} (sync: {sync})\n", session.page().id)
                }
            }
            EditCommand::Draft {
                entry_id,
                index,
                text_file,
            } => {
                let text = std::fs::read_to_string(text_file)?;
                let mut session = client.resume_editing(&entry_id, index).await?;
                session.draft(&text).await?;
                if args.json {
                    pretty(session.page())?
                } else {
                    format!("Draft stored for {}\n", session.page().id)
                }
            }
            EditCommand::Save {
                entry_id,
                index,
                status,
                share,
                request_review,
                comment,
            } => {
                let mut session = client.resume_editing(&entry_id, index).await?;
                let saved = session
                    .save(SaveOptions {
                        status: status.map(|s| match s {
                            SaveStatus::Initiated => PageStatus::Initiated,
                            SaveStatus::Completed => PageStatus::Completed,
                        }),
                        share,
                        request_review,
                        comment,
                        is_approval: None,
                    })
                    .await?;
                if args.json {
                    pretty(&saved)?
                } else {
                    format!(
                        "Saved {} ({}, event {})\n",
                        saved.page.id,
                        saved.page.status.as_str(),
                        saved.timeline_event_id
                    )
                }
            }
            EditCommand::Discard { entry_id, index } => {
                client
                    .resume_editing(&entry_id, index)
                    .await?
                    .discard()
                    .await?;
                if args.json {
                    pretty(&serde_json::json!({"discarded":true}))?
                } else {
                    format!("Discarded {entry_id}_{index}\n")
                }
            }
            EditCommand::Status { entry_id, index } => {
                let state = client.page_lock_state(&entry_id, index).await?;
                if args.json {
                    pretty(&state)?
                } else {
                    let lock = if state.status != PageStatus::Editing {
                        "unlocked"
                    } else if state.is_mine {
                        "locked by you"
                    } else {
                        "locked by another user"
                    };
                    format!(
                        "{}: {} ({lock}, sync: {})\n",
                        state.page_id,
                        state.status.as_str(),
                        state.sync_mode
                    )
                }
            }
        },
        Command::Login { .. } => return Err("use login --import".into()),
        Command::Whoami => {
            let user = client.me().await?;
            if args.json {
                pretty(&user)?
            } else {
                table(
                    &["UID", "名前", "Lv", "経験値", "文字数", "いいね", "石"],
                    vec![vec![
                        user.uid,
                        user.display_name,
                        count(user.level),
                        count(user.exp),
                        count(user.char_count),
                        count(user.like_count),
                        count(user.stone_count),
                    ]],
                )
            }
        }
        Command::Notifications { count: only_count } => {
            if only_count {
                let count = client.unread_notification_count().await?;
                if args.json {
                    pretty(&serde_json::json!({"count":count}))?
                } else {
                    format!("未読通知: {count}\n")
                }
            } else {
                let notifications = client.notifications(100).await?;
                if args.json {
                    pretty(&notifications)?
                } else {
                    table(
                        &["日時", "種類", "状態", "内容"],
                        notifications
                            .iter()
                            .map(|n| {
                                vec![
                                    stamp(&n.created_at),
                                    n.kind.clone(),
                                    n.state.clone(),
                                    notification_summary(n),
                                ]
                            })
                            .collect(),
                    )
                }
            }
        }
        Command::Timeline { project, limit } => {
            let filter = project.map(TimelineFilter::Project).unwrap_or_default();
            let items = client
                .timeline_with_refresh(filter, Some(limit), args.refresh)
                .await?;
            if args.json {
                pretty(&items)?
            } else {
                table(
                    &[
                        "日時",
                        "名前",
                        "プロジェクト",
                        "資料",
                        "コマ",
                        "文字数",
                        "本文",
                    ],
                    items
                        .iter()
                        .map(|item| {
                            vec![
                                stamp(&item.event.created_at),
                                item.actor
                                    .as_ref()
                                    .map(|u| u.display_name.clone())
                                    .unwrap_or_else(|| item.event.uid.clone()),
                                item.project_title
                                    .clone()
                                    .unwrap_or_else(|| item.event.project_id.clone()),
                                item.entry_label
                                    .as_ref()
                                    .map(|l| l.preferred(&["ja", "en"]))
                                    .unwrap_or_else(|| item.event.entry_id.clone()),
                                (u64::from(item.event.index) + 1).to_string(),
                                item.event.count.to_string(),
                                item.excerpt.clone(),
                            ]
                        })
                        .collect(),
                )
            }
        }
        Command::Ranking { by } => {
            let sort = match by {
                Sort::Exp => RankingSort::Exp,
                Sort::CharCount => RankingSort::CharCount,
                Sort::LikeCount => RankingSort::LikeCount,
            };
            let users = client.ranking(sort, None).await?;
            if args.json {
                pretty(&users)?
            } else {
                table(
                    &["順位", "名前", "Lv", "経験値", "文字数", "いいね"],
                    users
                        .iter()
                        .enumerate()
                        .map(|(i, u)| {
                            vec![
                                (i + 1).to_string(),
                                u.display_name.clone(),
                                count(u.level),
                                count(u.exp),
                                count(u.char_count),
                                count(u.like_count),
                            ]
                        })
                        .collect(),
                )
            }
        }
        Command::Announcements => {
            let announcements = client.announcements(None).await?;
            if args.json {
                pretty(&announcements)?
            } else {
                table(
                    &["日時", "お知らせ", "内容"],
                    announcements
                        .iter()
                        .map(|a| vec![stamp(&a.created_at), a.title.clone(), a.description.clone()])
                        .collect(),
                )
            }
        }
        Command::Projects => {
            let projects = client.cached_projects(&storage, args.refresh).await?;
            if args.json {
                serde_json::to_string_pretty(&projects)? + "\n"
            } else {
                table(
                    &["ID", "プロジェクト", "資料", "コマ"],
                    projects
                        .iter()
                        .map(|p| {
                            vec![
                                p.id.clone(),
                                p.title.clone(),
                                count(p.total_entry_count),
                                count(p.total_image_count),
                            ]
                        })
                        .collect(),
                )
            }
        }
        Command::Project { id, progress: true } => {
            let collections = client
                .cached_collections(&storage, &id, args.refresh)
                .await?;
            let mut figures = Vec::new();
            let mut rows = Vec::new();
            let mut total = [0u64; 5];
            for collection in collections {
                let p = client
                    .collection_progress(&collection.id, args.refresh)
                    .await?;
                let values = [
                    p.entries as u64,
                    p.size,
                    p.counts.completed,
                    p.counts.initiated,
                    p.counts.editing,
                ];
                for (total, value) in total.iter_mut().zip(values) {
                    *total += value;
                }
                rows.push(
                    std::iter::once(collection.title)
                        .chain(values.map(|n| n.to_string()))
                        .collect(),
                );
                figures.push(p);
            }
            if args.json {
                pretty(&figures)?
            } else {
                rows.push(
                    std::iter::once("合計".into())
                        .chain(total.map(|n| n.to_string()))
                        .collect(),
                );
                table(
                    &["コレクション", "資料", "コマ", "完了", "翻刻途中", "編集中"],
                    rows,
                )
            }
        }
        Command::Collection {
            id, progress: true, ..
        } => {
            let p = client.collection_progress(&id, args.refresh).await?;
            if args.json {
                pretty(&p)?
            } else {
                table(
                    &["コレクション", "資料", "コマ", "完了", "翻刻途中", "編集中"],
                    vec![vec![
                        p.collection_id,
                        p.entries.to_string(),
                        p.size.to_string(),
                        p.counts.completed.to_string(),
                        p.counts.initiated.to_string(),
                        p.counts.editing.to_string(),
                    ]],
                )
            }
        }
        Command::Collection {
            id,
            entries: true,
            entry_progress,
            ..
        } => {
            let entries = client.list_entries_with_refresh(&id, args.refresh).await?;
            if entry_progress {
                let ids: Vec<_> = entries.iter().map(|e| e.id.clone()).collect();
                pretty(
                    &client
                        .entry_progress_with_refresh(&ids, args.refresh)
                        .await?,
                )?
            } else {
                pretty(&entries)?
            }
        }
        Command::Project {
            id,
            progress: false,
        } => {
            let project = client.cached_project(&storage, &id, args.refresh).await?;
            if args.json {
                return write_output(pretty(&project)?);
            }
            let mut rows = vec![
                vec!["プロジェクト".into(), project.title],
                vec!["ID".into(), project.id],
                vec!["説明".into(), project.description.unwrap_or_default()],
            ];
            rows.extend(
                project
                    .collections
                    .unwrap_or_default()
                    .into_iter()
                    .map(|id| vec!["コレクション".into(), id]),
            );
            table(&["項目", "内容"], rows)
        }
        Command::Collection { id, .. } => {
            let collection = client
                .cached_collection(&storage, &id, args.refresh)
                .await?;
            if args.json {
                return write_output(pretty(&collection)?);
            }
            let mut rows = vec![
                vec!["コレクション".into(), collection.title],
                vec!["説明".into(), collection.description.unwrap_or_default()],
            ];
            rows.extend(
                collection
                    .entries
                    .unwrap_or_default()
                    .into_iter()
                    .map(|id| vec!["資料".into(), id]),
            );
            table(&["項目", "内容"], rows)
        }
        Command::Entry { id } => {
            let entry = client.cached_entry(&storage, &id, args.refresh).await?;
            if args.json {
                return write_output(pretty(&entry)?);
            }
            table(
                &["項目", "内容"],
                vec![
                    vec!["資料".into(), entry.label.preferred(&["ja", "en"])],
                    vec!["ID".into(), entry.id],
                    vec!["コマ".into(), count(entry.size.map(u64::from))],
                    vec!["IIIF".into(), entry.manifest_url],
                ],
            )
        }
        Command::Pages { entry_id, status } => {
            let pages = client
                .cached_pages(&storage, &entry_id, args.refresh)
                .await?;
            if status {
                let counts = blocking(&storage, move |db| db.page_status_counts(&entry_id)).await?;
                if args.json {
                    return write_output(pretty(&counts)?);
                }
                table(
                    &["状態", "コマ数"],
                    counts
                        .into_iter()
                        .map(|(status, count)| vec![status, count.to_string()])
                        .collect(),
                )
            } else {
                if args.json {
                    return write_output(pretty(&pages)?);
                }
                table(
                    &["コマ", "状態", "本文"],
                    pages
                        .iter()
                        .map(|page| {
                            vec![
                                (page.index + 1).to_string(),
                                page.status.as_str().into(),
                                honkoku_text::excerpt(&page.text, 60),
                            ]
                        })
                        .collect(),
                )
            }
        }
    };
    write_output(output)
}
fn pretty(value: &impl serde::Serialize) -> Result<String, serde_json::Error> {
    Ok(serde_json::to_string_pretty(value)? + "\n")
}
fn stamp(value: &honkoku_core::model::Timestamp) -> String {
    value.0.to_string()
}
fn notification_summary(notification: &honkoku_core::model::Notification) -> String {
    let data = &notification.data;
    for key in ["title", "content"] {
        if let Some(text) = data[key].as_str().filter(|text| !text.is_empty()) {
            return honkoku_text::excerpt(text, 60);
        }
    }
    match notification.kind.as_str() {
        "levelup" => format!("Lv. {}", data["level"]),
        "transcription" => {
            let excerpt = data["data"]["text"]
                .as_str()
                .map(|text| honkoku_text::excerpt(text, 60))
                .unwrap_or_default();
            format!(
                "{} {}",
                data["entryId"].as_str().unwrap_or_default(),
                excerpt
            )
        }
        "like" => format!(
            "{} / {}",
            data["collectionName"].as_str().unwrap_or_default(),
            data["targetId"].as_str().unwrap_or_default()
        ),
        _ => honkoku_text::excerpt(&data.to_string(), 60),
    }
}
fn write_output(output: String) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(error) = io::stdout().lock().write_all(output.as_bytes())
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(error.into());
    }
    Ok(())
}
fn count(value: Option<u64>) -> String {
    value.map(|n| n.to_string()).unwrap_or_else(|| "—".into())
}
fn table(headers: &[&str], rows: Vec<Vec<String>>) -> String {
    let rows: Vec<Vec<String>> = std::iter::once(headers.iter().map(|s| s.to_string()).collect())
        .chain(rows)
        .map(|row| {
            row.into_iter()
                .map(|cell| {
                    cell.chars()
                        .map(|ch| if ch.is_control() { ' ' } else { ch })
                        .collect()
                })
                .collect()
        })
        .collect();
    let widths: Vec<usize> = (0..headers.len())
        .map(|col| rows.iter().map(|row| row[col].width()).max().unwrap_or(0))
        .collect();
    let mut result = String::new();
    for row in rows {
        for (column, cell) in row.iter().enumerate() {
            result.push_str(cell);
            if column + 1 < widths.len() {
                result.push_str(&" ".repeat(widths[column] - cell.width() + 2));
            }
        }
        result.push('\n');
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn japanese_columns_and_terminal_controls() {
        let output = table(
            &["名", "値"],
            vec![
                vec!["abc".into(), "x\ny".into()],
                vec!["日本".into(), "z".into()],
            ],
        );
        let lines: Vec<_> = output.lines().collect();
        assert_eq!(
            lines[0].split('値').next().map(UnicodeWidthStr::width),
            Some(6)
        );
        assert_eq!(lines[1], "abc   x y");
        assert_eq!(lines[2], "日本  z");
    }
}
