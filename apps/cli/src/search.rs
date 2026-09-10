use clap::{Args, Subcommand};
use honkoku_search::{IndexBuilder, Mode, Query, Searcher};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use unicode_width::UnicodeWidthStr;

#[derive(Args)]
pub struct SearchArgs {
    #[command(subcommand)]
    command: Option<SearchCommand>,
    #[arg(value_name = "QUERY")]
    text: Option<String>,
    #[arg(long)]
    folded: bool,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    entry: Option<String>,
    #[arg(long, default_value_t = 20)]
    limit: usize,
    #[arg(long)]
    cursor: Option<String>,
    #[arg(long)]
    timing: bool,
}
#[derive(Subcommand)]
enum SearchCommand {
    Build {
        #[arg(long)]
        dump: PathBuf,
    },
    Status,
}
pub fn run(
    args: SearchArgs,
    cache: &Path,
    json: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let path = cache.join("search");
    match args.command {
        Some(SearchCommand::Build { dump }) => {
            let commit = honkoku_search::dump_commit(&dump)?;
            eprintln!("Dump commit: {commit}");
            let start = Instant::now();
            let mut builder = IndexBuilder::open(&path)?;
            let progress = builder.from_dump(dump, &commit, |p| {
                if p.done.is_multiple_of(10000) || p.done == p.total {
                    eprintln!("{}/{} pages ({} indexed)", p.done, p.total, p.indexed);
                }
            })?;
            builder.finish()?;
            let status = honkoku_search::status(path)?;
            Ok(serde_json::to_string_pretty(
                &serde_json::json!({"status":status,"build":progress,"elapsed_seconds":start.elapsed().as_secs_f64()}),
            )? + "\n")
        }
        Some(SearchCommand::Status) => {
            Ok(serde_json::to_string_pretty(&honkoku_search::status(path)?)? + "\n")
        }
        None => {
            let text = args
                .text
                .ok_or("provide a search query, build, or status")?;
            let query = Query {
                text,
                mode: if args.folded {
                    Mode::Folded
                } else {
                    Mode::Strict
                },
                project: args.project,
                entry: args.entry,
                limit: args.limit,
                cursor: args.cursor,
            };
            let searcher = Searcher::open(path)?;
            let result = searcher.search(query.clone())?;
            if args.timing {
                let mut samples = Vec::new();
                for _ in 0..5 {
                    let start = Instant::now();
                    let _ = searcher.search(query.clone())?;
                    samples.push(start.elapsed().as_secs_f64() * 1000.0);
                }
                samples.sort_by(f64::total_cmp);
                eprintln!(
                    "Warm query: median {:.3} ms (5 runs; min {:.3}, max {:.3})",
                    samples[2], samples[0], samples[4]
                );
            }
            if json {
                return Ok(serde_json::to_string_pretty(&result)? + "\n");
            }
            let clean = |text: &str| {
                text.chars()
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .collect::<String>()
            };
            let width = result
                .hits
                .iter()
                .flat_map(|h| &h.occurrences)
                .map(|o| clean(&o.before).width())
                .max()
                .unwrap_or(0);
            let mut output = format!("{}ページ\n", result.total);
            for hit in result.hits {
                for occurrence in hit.occurrences {
                    let before = clean(&occurrence.before);
                    output.push_str(&format!(
                        "{}{} │ {} │ {}\n  {}・コマ{}・{} [{}]\n",
                        " ".repeat(width.saturating_sub(before.width())),
                        before,
                        clean(&occurrence.matched),
                        clean(&occurrence.after),
                        hit.entry_label,
                        hit.index,
                        hit.project_title,
                        hit.page_id
                    ));
                }
            }
            if let Some(cursor) = result.next_cursor {
                output.push_str(&format!("Cursor: {cursor}\n"));
            }
            Ok(output)
        }
    }
}
