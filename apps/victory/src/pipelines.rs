//! `victory pipelines`: lists of the title's pipelines without its shader code
//! (`known_pipelines::list`), to share, merge and import, so that `victory
//! warm` can make a first game's pipelines before it is played.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, anyhow};
use clap::Subcommand;
use victory_hle::known_pipelines::list::{self, Keeper, List};

#[derive(Subcommand)]
pub enum Pipelines {
    /// Write the pipelines booked in this cache (runs with
    /// `VICTORY_PIPELINE_BOOK=1`) as a list without shader code: keys, the
    /// shaders' stages and hashes, graphics states, and which runs drew with.
    Export {
        out: PathBuf,
        /// Only the pipelines earlier runs drew with (the early list).
        #[arg(long)]
        used: bool,
        /// Where the list comes from (free text, printable ASCII).
        #[arg(long, default_value = "")]
        label: String,
    },
    /// Add a list's pipelines to this cache's book and early list. The shader
    /// containers come from the player's own files: those `--shaders` names
    /// now, or the next `victory shaders`. Then `victory warm --used`.
    Import {
        file: PathBuf,
        /// Folders or files of the player's own shaders (as `victory
        /// shaders` takes them) to take the containers from.
        #[arg(long)]
        shaders: Vec<PathBuf>,
        /// Threads that read the shaders (default: 4).
        #[arg(long)]
        jobs: Option<usize>,
    },
    /// Merge lists into one: each pipeline once, used if a list says so.
    Merge {
        out: PathBuf,
        #[arg(required = true)]
        lists: Vec<PathBuf>,
        #[arg(long, default_value = "")]
        label: String,
    },
    /// Check a list and say what it holds.
    Info { file: PathBuf },
}

pub fn run(command: Pipelines) -> Result<()> {
    match command {
        Pipelines::Export { out, used, label } => {
            let dir = crate::warm::folder()?;
            let list = List::of_book(&dir, used, &label);
            anyhow::ensure!(
                !list.pipelines.is_empty(),
                "no pipeline booked in {}",
                dir.display()
            );
            let bytes = write(&out, &list)?;
            println!(
                "{}: {} pipelines ({} drawn with), {} bytes, no shader code",
                out.display(),
                list.pipelines.len(),
                list.used(),
                bytes
            );
        }
        Pipelines::Import {
            file,
            shaders,
            jobs,
        } => {
            let list = read(&file)?;
            let dir = crate::warm::folder()?;
            let done = list::import(&dir, &list)
                .with_context(|| format!("importing into {}", dir.display()))?;
            println!(
                "{}: {} pipelines, {} added, {} known already ({} under this build's own key); {} added to the early list",
                file.display(),
                list.pipelines.len(),
                done.added,
                done.there,
                done.rekeyed,
                done.early
            );
            if !shaders.is_empty() {
                keep_shaders(&dir, shaders, jobs.unwrap_or(4))?;
            }
            let lacking = victory_hle::known_pipelines::lacking(&dir).len();
            if lacking > 0 {
                println!(
                    "{lacking} pipelines wait for their shaders: `victory shaders <your shaders>` or `victory pipelines import <list> --shaders <your shaders>`"
                );
            }
        }
        Pipelines::Merge { out, lists, label } => {
            let lists: Vec<List> = lists.iter().map(|f| read(f)).collect::<Result<_>>()?;
            let merged = List::merge(&lists, &label);
            let bytes = write(&out, &merged)?;
            println!(
                "{}: {} pipelines ({} drawn with) from {} lists, {bytes} bytes",
                out.display(),
                merged.pipelines.len(),
                merged.used(),
                lists.len()
            );
        }
        Pipelines::Info { file } => {
            let list = read(&file)?;
            let shaders: std::collections::HashSet<_> = list
                .pipelines
                .iter()
                .flat_map(|p| p.shaders.iter().map(|s| s.1))
                .collect();
            let wrong = list.rekeyed();
            println!(
                "{}: format {}, label {:?}, {} pipelines ({} drawn with{}), {} shaders by hash, {} keys this build makes otherwise",
                file.display(),
                list::VERSION,
                list.label,
                list.pipelines.len(),
                list.used(),
                if list.only_used { ", those alone" } else { "" },
                shaders.len(),
                wrong
            );
        }
    }
    Ok(())
}

fn read(file: &Path) -> Result<List> {
    let text =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    List::from_text(&text).map_err(|e| anyhow!("{}: {e}", file.display()))
}

/// Writes `list` (checked to hold nothing but the format's fields); its bytes.
fn write(out: &Path, list: &List) -> Result<usize> {
    let text = list.to_text().map_err(|e| anyhow!("{e}"))?;
    std::fs::write(out, &text).with_context(|| format!("writing {}", out.display()))?;
    Ok(text.len())
}

/// The containers the book lacks, from the player's files under `paths`.
fn keep_shaders(dir: &Path, paths: Vec<PathBuf>, threads: usize) -> Result<()> {
    let Some(keeper) = Keeper::of(dir)? else {
        println!("every container the book names is there already");
        return Ok(());
    };
    let wanted = keeper.wanted();
    let began = std::time::Instant::now();
    for root in &paths {
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
        {
            if keeper.wanted() == 0 {
                break;
            }
            if !entry.file_type().is_file()
                || entry.metadata().map_or(0, |m| m.len()) > crate::shader::MAX_FILE
            {
                continue;
            }
            let Ok(data) = std::fs::read(entry.path()) else {
                continue;
            };
            let jobs = crate::shader::sources(&data);
            let next = AtomicUsize::new(0);
            std::thread::scope(|s| {
                for _ in 0..threads.max(1).min(jobs.len()) {
                    s.spawn(|| {
                        while let Some(job) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                            if let Some(bytes) = job.bytes() {
                                offer(&keeper, &bytes);
                            }
                        }
                    });
                }
            });
        }
    }
    println!(
        "{} of {wanted} containers the book lacked found in your files ({:.0} s)",
        keeper.kept(),
        began.elapsed().as_secs_f64()
    );
    Ok(())
}

/// Offers one container of the player's files to `keeper`.
fn offer(keeper: &Keeper, container: &[u8]) {
    let hash = blake3::hash(container);
    if let Err(e) = keeper.offer(hash.as_bytes(), container) {
        eprintln!("known-shaders.bin not written: {e}");
    }
}
