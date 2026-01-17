mod cli_treemap;

use clap::{Parser, ValueEnum};
use dm_core::session::Session;
use dm_core::fs_adapter::UnixFsAdapter;
use cli_treemap::TreeMapView;

#[derive(Parser)]
#[command(name = "dm")]
#[command(about = "Disk usage analyzer with treemap visualization", long_about = None)]
struct Cli {
    /// Directory to analyze
    #[cfg(windows)]
    #[arg(default_value = "C:\\")]
    path: String,

    /// Directory to analyze
    #[cfg(not(windows))]
    #[arg(default_value = "/")]
    path: String,

    /// Display mode for the treemap
    #[arg(short, long, value_enum, default_value_t = DisplayMode::Default)]
    display: DisplayMode,

    /// Show free space on the filesystem
    #[arg(short, long, default_value_t = true, action = clap::ArgAction::Set)]
    free_space: bool,

    /// Scale blocks proportionally to actual disk usage (default: true, use --no-proportional to disable)
    #[arg(short, long, default_value_t = true, action = clap::ArgAction::Set)]
    proportional: bool,

    /// Enable parallel scanning (auto-detects SSD, or specify number of threads)
    #[arg(long, value_name = "THREADS", default_value = "16")]
    parallel: usize,

    /// Enable real-time visualization with live updates
    #[arg(short = 'r', long, default_value_t = true, action = clap::ArgAction::Set)]
    realtime: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum DisplayMode {
    /// Compact 4-line summary
    Compact,
    /// Default medium-sized visualization
    Default,
    /// Full screen treemap
    Full,
}

fn get_filesystem_stats(path: &str) -> Option<(u64, u64)> {
    use std::path::Path;

    // Use fs4 crate for cross-platform disk space queries
    let path_obj = Path::new(path);

    // Try to get a canonical path, fall back to the original if that fails
    let canonical_path = std::fs::canonicalize(path_obj).unwrap_or_else(|_| path_obj.to_path_buf());

    match fs4::statvfs(&canonical_path) {
        Ok(stats) => {
            let total = stats.total_space();
            let free = stats.available_space();
            Some((total, free))
        }
        Err(_) => None,
    }
}

fn enter_alt_screen() {
    use std::io::{self, Write};
    let mut stdout = io::stdout();
    write!(stdout, "\x1b[?1049h\x1b[H").ok(); // Enter alternate screen
    stdout.flush().ok();
}

fn exit_alt_screen() {
    use std::io::{self, Write};
    let mut stdout = io::stdout();
    write!(stdout, "\x1b[?1049l").ok(); // Exit alternate screen
    stdout.flush().ok();
}

fn render_live_frame(session: &Session, cli: &Cli, width: usize, height: usize, fs_stats: Option<(u64, u64)>) {
    use std::io::{self, Write};

    let mut stdout = io::stdout();

    // Move cursor to home and clear from cursor to end
    write!(stdout, "\x1b[H\x1b[J").ok();

    let treemap = TreeMapView::new(width, height);

    if !cli.realtime {
        writeln!(stdout, "=== Directory Tree Map (LIVE) ===").ok();
        writeln!(stdout, "Root: {}", cli.path).ok();
        writeln!(stdout, "Errors: {}", session.errors).ok();
        writeln!(stdout, "Jobs: {} started, {} done\n", session.jobs_started, session.jobs_done).ok();
    }
    writeln!(stdout, "{}", treemap.render_tree(&session.tree, fs_stats, cli.proportional)).ok();
    
    stdout.flush().ok();
}

fn main() {
    let mut cli = Cli::parse();

    // Expand ~ to home directory and resolve path
    let expanded_path = if cli.path.starts_with('~') {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            let home_str = home.to_string_lossy();
            cli.path.replacen('~', &home_str, 1)
        } else {
            cli.path.clone()
        }
    } else {
        cli.path.clone()
    };

    // Use dunce::canonicalize to resolve . and .. without UNC prefix on Windows
    cli.path = match dunce::canonicalize(&expanded_path) {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => expanded_path, // Fallback to expanded path if canonicalization fails
    };

    let now = std::time::Instant::now();
    let fs = UnixFsAdapter;

    // Get filesystem stats early if requested
    let fs_stats = if cli.free_space {
        get_filesystem_stats(&cli.path)
    } else {
        None
    };

    // Determine parallelism
    let num_threads = Some(cli.parallel);

    // Determine treemap dimensions based on display mode
    let (width, height) = match cli.display {
        DisplayMode::Compact => {
            // Use terminal width for compact mode too
            let (term_width, _) = term_size::dimensions().unwrap_or((80, 40));
            (term_width, 4)
        }
        DisplayMode::Default => {
            // Use terminal width, constrained height
            let (term_width, _) = term_size::dimensions().unwrap_or((120, 40));
            (term_width, 25)
        }
        DisplayMode::Full => {
            // Try to get terminal size, fall back to large default
            term_size::dimensions().unwrap_or((120, 40))
        }
    };

    let mut session = Session::new(cli.path.parse().unwrap(), 10);

    // Run with or without real-time visualization
    if cli.realtime {
        // Enter alternate screen buffer
        enter_alt_screen();

        if let Some(threads) = num_threads {
            session.run_parallel_with_callback(&fs, threads, |sess| {
                render_live_frame(sess, &cli, width, height, fs_stats);
            });
        } else {
            eprintln!("Warning: real-time mode works best with --parallel");
            session.run(&fs);
        }

        // Exit alternate screen buffer
        exit_alt_screen();
    } else {
        if let Some(threads) = num_threads {
            session.run_parallel(&fs, threads);
        } else {
            session.run(&fs);
        }
    }

    // Final output
    let treemap = TreeMapView::new(width, height);

    println!("=== Directory Tree Map ===");
    println!("Root: {}", cli.path);
    println!("Time: {:?}", now.elapsed());
    println!("Errors: {}", session.errors);
    println!("Jobs: {} started, {} done", session.jobs_started, session.jobs_done);

    if std::env::var("DEBUG").is_ok() {
        println!("Terminal size: {}x{}", width, height);
    }
    println!();

    println!("{}", treemap.render_tree(&session.tree, fs_stats, cli.proportional));

    println!("Time Elapsed: {:.2}s", now.elapsed().as_secs_f64());
    println!("=== End ===");
}