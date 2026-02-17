#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { Session, UnixFsAdapter } = require('../dm_core');
const { TreeMapView } = require('./cli_treemap');

function parseBool(value, defaultValue) {
  if (value === undefined || value === null) {
    return defaultValue;
  }
  if (typeof value === 'boolean') {
    return value;
  }
  const lowered = String(value).toLowerCase();
  if (['1', 'true', 'yes', 'y', 'on'].includes(lowered)) {
    return true;
  }
  if (['0', 'false', 'no', 'n', 'off'].includes(lowered)) {
    return false;
  }
  return defaultValue;
}

function expandHome(inputPath) {
  if (!inputPath) {
    return inputPath;
  }
  if (inputPath.startsWith('~')) {
    const home = process.env.HOME || process.env.USERPROFILE;
    if (home) {
      return inputPath.replace('~', home);
    }
  }
  return inputPath;
}

function stripUncPrefix(inputPath) {
  if (inputPath.startsWith('\\\\?\\UNC\\')) {
    return `\\\\${inputPath.slice(8)}`;
  }
  if (inputPath.startsWith('\\\\?\\')) {
    return inputPath.slice(4);
  }
  return inputPath;
}

function canonicalizePath(inputPath) {
  try {
    const resolved = fs.realpathSync(inputPath);
    return stripUncPrefix(resolved);
  } catch (_error) {
    return inputPath;
  }
}

function getFilesystemStats(targetPath) {
  let canonical = targetPath;
  try {
    canonical = fs.realpathSync(targetPath);
  } catch (_error) {
    canonical = targetPath;
  }

  if (typeof fs.statfsSync !== 'function') {
    return null;
  }

  try {
    const stats = fs.statfsSync(canonical);
    const total = stats.bsize * stats.blocks;
    const free = stats.bsize * stats.bavail;
    return [total, free];
  } catch (_error) {
    return null;
  }
}

function enterAltScreen() {
  process.stdout.write('\u001b[?1049h\u001b[H');
}

function exitAltScreen() {
  process.stdout.write('\u001b[?1049l');
}

function renderLiveFrame(session, cli, width, height, fsStats) {
  process.stdout.write('\u001b[H\u001b[J');

  const treemap = new TreeMapView(width, height);

  if (!cli.realtime) {
    process.stdout.write('=== Directory Tree Map (LIVE) ===\n');
    process.stdout.write(`Root: ${cli.path}\n`);
    process.stdout.write(`Errors: ${session.errors}\n`);
    process.stdout.write(`Jobs: ${session.jobsStarted} started, ${session.jobsDone} done\n\n`);
  }

  process.stdout.write(treemap.renderTree(session.tree, fsStats, cli.proportional));
}

function printHelp() {
  const help = `DiskMap (Node.js)

Usage:
  dm-node [OPTIONS] [PATH]

Options:
  -d, --display <mode>       Display mode: compact, default, full
      --free-space           Show free space (default: true)
      --no-free-space        Disable free space display
      --proportional         Proportional free space blocks (default: true)
      --no-proportional      Disable proportional scaling
      --parallel <threads>   Number of parallel workers (default: 16)
  -r, --realtime             Enable realtime rendering (default: true)
      --no-realtime          Disable realtime rendering
  -h, --help                 Show this help
`;
  process.stdout.write(help);
}

function parseArgs(argv) {
  const defaults = {
    path: process.platform === 'win32' ? 'C:\\' : '/',
    display: 'default',
    freeSpace: true,
    proportional: true,
    parallel: 16,
    realtime: true,
  };

  const cli = { ...defaults };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];

    if (arg === '-h' || arg === '--help') {
      cli.help = true;
      continue;
    }

    if (arg === '-d' || arg === '--display') {
      cli.display = argv[i + 1] || cli.display;
      i += 1;
      continue;
    }

    if (arg.startsWith('--display=')) {
      cli.display = arg.split('=')[1];
      continue;
    }

    if (arg === '--free-space') {
      cli.freeSpace = true;
      continue;
    }

    if (arg === '--no-free-space') {
      cli.freeSpace = false;
      continue;
    }

    if (arg === '--proportional') {
      cli.proportional = true;
      continue;
    }

    if (arg === '--no-proportional') {
      cli.proportional = false;
      continue;
    }

    if (arg === '--parallel') {
      const value = argv[i + 1];
      cli.parallel = Number.parseInt(value, 10) || cli.parallel;
      i += 1;
      continue;
    }

    if (arg.startsWith('--parallel=')) {
      const value = arg.split('=')[1];
      cli.parallel = Number.parseInt(value, 10) || cli.parallel;
      continue;
    }

    if (arg === '-r' || arg === '--realtime') {
      const next = argv[i + 1];
      if (next && !next.startsWith('-')) {
        cli.realtime = parseBool(next, true);
        i += 1;
      } else {
        cli.realtime = true;
      }
      continue;
    }

    if (arg.startsWith('--realtime=')) {
      const value = arg.split('=')[1];
      cli.realtime = parseBool(value, true);
      continue;
    }

    if (arg === '--no-realtime') {
      cli.realtime = false;
      continue;
    }

    if (!arg.startsWith('-') && !cli.pathProvided) {
      cli.path = arg;
      cli.pathProvided = true;
      continue;
    }
  }

  return cli;
}

async function main() {
  const cli = parseArgs(process.argv.slice(2));

  if (cli.help) {
    printHelp();
    return;
  }

  const expandedPath = expandHome(cli.path);
  cli.path = canonicalizePath(expandedPath);

  const start = Date.now();
  const fsAdapter = new UnixFsAdapter();

  const fsStats = cli.freeSpace ? getFilesystemStats(cli.path) : null;

  const termWidth = process.stdout.columns || 120;
  const termHeight = process.stdout.rows || 40;

  let width;
  let height;

  switch (cli.display) {
    case 'compact':
      width = termWidth;
      height = 4;
      break;
    case 'full':
      width = termWidth;
      height = termHeight;
      break;
    case 'default':
    default:
      width = termWidth;
      height = 25;
      break;
  }

  const session = new Session(cli.path, 10);
  const threads = Math.max(1, cli.parallel || 1);

  if (cli.realtime) {
    enterAltScreen();
    await session.runParallelWithCallback(fsAdapter, threads, (sess) => {
      renderLiveFrame(sess, cli, width, height, fsStats);
    });
    exitAltScreen();
  } else {
    await session.runParallel(fsAdapter, threads);
  }

  const treemap = new TreeMapView(width, height);
  process.stdout.write('=== Directory Tree Map ===\n');
  process.stdout.write(`Root: ${cli.path}\n`);
  process.stdout.write(`Time: ${(Date.now() - start) / 1000}s\n`);
  process.stdout.write(`Errors: ${session.errors}\n`);
  process.stdout.write(`Jobs: ${session.jobsStarted} started, ${session.jobsDone} done\n`);

  if (process.env.DEBUG) {
    process.stdout.write(`Terminal size: ${width}x${height}\n`);
  }

  process.stdout.write('\n');
  process.stdout.write(treemap.renderTree(session.tree, fsStats, cli.proportional));
  const elapsedSeconds = (Date.now() - start) / 1000;
  process.stdout.write(`Time Elapsed: ${elapsedSeconds.toFixed(2)}s\n`);
  process.stdout.write('=== End ===\n');
}

main().catch((error) => {
  process.stderr.write(`Error: ${error.message || error}\n`);
  process.exitCode = 1;
});
