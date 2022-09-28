// #![deny(warnings)]
use clap::Parser;
use colored::{Color, Colorize};
use env_logger::Env;
use git2::{Error, ErrorCode, Repository, StatusOptions};
use log::debug;

extern crate log;

#[derive(Parser, Debug)]
#[clap(version, long_about = None)]
struct Args {
  /// git directory to analyze
  #[clap(name = "dir", long = "git-dir")]
  flag_git_dir: Option<String>,

  /// enable color
  #[clap(long = "color")]
  color: bool,
}

fn is_ahead_behind_remote(repo: &Repository) -> Result<(usize, usize), Error> {
  let head = repo.revparse_single("HEAD")?.id();
  let upstream = repo.revparse_ext("@{u}")?.0.id();
  Ok(repo.graph_ahead_behind(head, upstream)?)
}

fn stringify_status(status: (i32, i32, i32), prefix: &str, color: Color) -> String {
  let (new, modified, deleted) = status;
  if new > 0 || modified > 0 || deleted > 0 {
    format!("{}+{} ~{} -{}", prefix.yellow(), new, modified, deleted)
      .color(color)
      .to_string()
  } else {
    "".to_string()
  }
}

fn get_branch_name(repo: &Repository) -> Result<String, Error> {
  let head = match repo.head() {
    Ok(head) => Some(head),
    Err(ref e) if e.code() == ErrorCode::UnbornBranch || e.code() == ErrorCode::NotFound => None,
    Err(e) => return Err(e),
  };
  let head = head.as_ref().and_then(|h| h.shorthand());

  Ok(head.unwrap_or("no branch").to_string())
}

fn get_short_status(statuses: &git2::Statuses) -> ((i32, i32, i32), (i32, i32, i32)) {
  let mut index_newfile_count: i32 = 0;
  let mut index_modified_count: i32 = 0;
  let mut index_deleted_count: i32 = 0;
  let mut wt_newfile_count: i32 = 0;
  let mut wt_modified_count: i32 = 0;
  let mut wt_deleted_count: i32 = 0;

  // Compute counter on index
  for entry in statuses.iter().filter(|e| e.status() != git2::Status::CURRENT) {
    match entry.status() {
      s if s.contains(git2::Status::INDEX_NEW) => index_newfile_count += 1,
      s if s.contains(git2::Status::INDEX_MODIFIED) => index_modified_count += 1,
      s if s.contains(git2::Status::INDEX_DELETED) => index_deleted_count += 1,
      s if s.contains(git2::Status::INDEX_RENAMED) => index_modified_count += 1,
      s if s.contains(git2::Status::INDEX_TYPECHANGE) => index_modified_count += 1,
      _ => continue,
    };
  }

  // Compute counter on index
  // Print workdir changes to tracked files
  for entry in statuses.iter() {
    // With `Status::OPT_INCLUDE_UNMODIFIED` (not used here)
    // `index_to_workdir` may not be `None` even if there are no differences,
    // in which case it will be a `Delta::Unmodified`.
    if entry.status() == git2::Status::CURRENT || entry.index_to_workdir().is_none() {
      continue;
    }

    match entry.status() {
      s if s.contains(git2::Status::WT_NEW) => wt_newfile_count += 1,
      s if s.contains(git2::Status::WT_MODIFIED) => wt_modified_count += 1,
      s if s.contains(git2::Status::WT_DELETED) => wt_deleted_count += 1,
      s if s.contains(git2::Status::WT_RENAMED) => wt_modified_count += 1,
      s if s.contains(git2::Status::WT_TYPECHANGE) => wt_modified_count += 1,
      _ => continue,
    };
  }

  (
    (index_newfile_count, index_modified_count, index_deleted_count),
    (wt_newfile_count, wt_modified_count, wt_deleted_count),
  )
}

fn run(args: &Args) -> Result<(), Error> {
  let path = args.flag_git_dir.clone().unwrap_or(".".to_string());

  let repo = Repository::open(&path)?;

  if repo.is_bare() {
    return Err(Error::from_str("Cannot report status on bare repository"));
  }

  let mut opts = StatusOptions::new();
  opts.include_untracked(true).recurse_untracked_dirs(true);
  opts.exclude_submodules(true);

  let statuses = repo.statuses(Some(&mut opts))?;

  let (index_status, wt_status) = get_short_status(&statuses);

  let branch_name = get_branch_name(&repo)?;

  let (ahead, behind) = match is_ahead_behind_remote(&repo) {
    Ok((commits_ahead, commits_behind)) => (commits_ahead, commits_behind),
    Err(_) => (0, 0),
  };

  let vec_of_status = vec![
    branch_name.blue().to_string(),
    if ahead > 0 {
      format!("↑{}", ahead).green().to_string()
    } else {
      "".to_string()
    },
    if behind > 0 {
      format!("↓{}", behind).red().to_string()
    } else {
      "".to_string()
    },
    stringify_status(index_status, "", Color::Green),
    stringify_status(wt_status, "| ", Color::Red),
  ];

  println!(
    "{}{}{}",
    "[".yellow().to_string(),
    vec_of_status
      .into_iter()
      .filter(|elem| !elem.is_empty())
      .collect::<Vec<_>>()
      .join(" "),
    "]".yellow().to_string(),
  );

  return Ok(());
}

fn main() {
  let env = Env::default()
    .filter_or("RUST_LOG_LEVEL", "info")
    .write_style_or("RUST_LOG_STYLE", "auto");

  env_logger::init_from_env(env);

  let args = Args::parse();
  colored::control::set_override(args.color);

  match run(&args) {
    Ok(()) => {}
    Err(e) => {
      debug!("{}", e);
      return ();
    }
  }
}
