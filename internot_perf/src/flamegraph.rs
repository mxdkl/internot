//! CPU flamegraphs via pprof's in-process sampling profiler (`SIGPROF`
//! timer plus stack unwinding). Needs no root and no external `perf`.
//!
//! Stacks resolve best in a build with debug info: use
//! `cargo run --profile profiling ...` rather than `--release`. Inlined
//! frames collapse into their caller either way.

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

/// Samples per second of CPU time.
const SAMPLE_HZ: i32 = 999;

/// Calls `body` repeatedly for `duration` under the profiler and writes an
/// SVG flamegraph titled `title` to `out`, creating parent directories.
///
/// Make one `body` call cheap relative to `duration` but not trivially
/// cheap: the loop checks the clock between calls, so a ns-scale body
/// should run a batch of calls.
pub fn profile_to_svg(
    out: &Path,
    title: &str,
    duration: Duration,
    mut body: impl FnMut(),
) -> Result<()> {
    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(SAMPLE_HZ)
        // Unwinding inside these while they hold locks can deadlock.
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .map_err(|e| Error::new(format!("starting profiler: {e}")))?;
    let start = Instant::now();
    while start.elapsed() < duration {
        body();
    }
    let report = guard
        .report()
        .build()
        .map_err(|e| Error::new(format!("building profile report: {e}")))?;
    drop(guard);

    if let Some(dir) = out.parent() {
        fs::create_dir_all(dir)
            .map_err(|e| Error::new(format!("creating {}: {e}", dir.display())))?;
    }
    let file =
        File::create(out).map_err(|e| Error::new(format!("creating {}: {e}", out.display())))?;
    let mut options = pprof::flamegraph::Options::default();
    options.title = title.to_owned();
    report
        .flamegraph_with_options(BufWriter::new(file), &mut options)
        .map_err(|e| Error::new(format!("writing {}: {e}", out.display())))
}
