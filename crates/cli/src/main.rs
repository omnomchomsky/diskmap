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
    #[arg(default_value = ".")]
    path: String,

    /// Display mode for the treemap
    #[arg(short, long, value_enum, default_value_t = DisplayMode::Default)]
    display: DisplayMode,

    /// Show free space on the filesystem
    #[arg(short, long)]
    free_space: bool,

    /// Scale blocks proportionally to actual disk usage (default: true, use --no-proportional to disable)
    #[arg(short, long, default_value_t = true, action = clap::ArgAction::Set)]
    proportional: bool,

    /// Enable parallel scanning (auto-detects SSD, or specify number of threads)
    #[arg(long, value_name = "THREADS")]
    parallel: Option<Option<usize>>,

    /// Enable real-time visualization with live updates
    #[arg(short = 'r', long)]
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
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        let c_path = CString::new(path).ok()?;
        let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();

        unsafe {
            if libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) == 0 {
                let stat = stat.assume_init();
                let block_size = stat.f_frsize as u64;
                let total_blocks = stat.f_blocks as u64;
                let available_blocks = stat.f_bavail as u64;

                let total_bytes = total_blocks * block_size;
                let available_bytes = available_blocks * block_size;

                return Some((total_bytes, available_bytes));
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    None
}

fn is_ssd(path: &str) -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        use std::path::Path;

        // Get the device for this path
        let metadata = fs::metadata(path).ok()?;
        let dev = metadata.dev();

        // Major device number
        let major = (dev >> 8) & 0xff;

        // Try to find the device in /sys/block
        let sys_block = fs::read_dir("/sys/block").ok()?;

        for entry in sys_block.flatten() {
            let dev_path = entry.path();
            let dev_name = dev_path.file_name()?.to_str()?;

            // Check if this is our device by reading dev file
            let dev_file = dev_path.join("dev");
            if let Ok(content) = fs::read_to_string(&dev_file) {
                if content.starts_with(&format!("{}:", major)) {
                    // Found our device, check if rotational
                    let rotational_file = dev_path.join("queue/rotational");
                    if let Ok(rotational) = fs::read_to_string(&rotational_file) {
                        return Some(rotational.trim() == "0");
                    }
                }
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        // Use diskutil to check if it's an SSD
        let output = Command::new("diskutil")
            .args(["info", path])
            .output()
            .ok()?;

        let stdout = String::from_utf8(output.stdout).ok()?;

        // Look for "Solid State" in the output
        if stdout.contains("Solid State: Yes") {
            return Some(true);
        } else if stdout.contains("Solid State: No") {
            return Some(false);
        }

        None
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = path;
        None
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
    let cli = Cli::parse();
    let now = std::time::Instant::now();
    let fs = UnixFsAdapter;

    // Get filesystem stats early if requested
    let fs_stats = if cli.free_space {
        get_filesystem_stats(&cli.path)
    } else {
        None
    };

    // Determine parallelism
    let num_threads = match cli.parallel {
        Some(Some(n)) => Some(n), // Explicit thread count
        Some(None) => {
            // --parallel flag without value: auto-detect based on SSD
            match is_ssd(&cli.path) {
                Some(true) => Some(num_cpus::get().min(8)), // SSD: use CPU cores (cap at 8)
                Some(false) => None, // HDD: sequential
                None => Some(4), // Unknown: use conservative default
            }
        }
        None => None, // No flag: sequential
    };

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