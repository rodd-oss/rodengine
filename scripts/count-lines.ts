#!/usr/bin/env bun

import { glob } from "glob";
import fs from "fs";
import path from "path";

interface CountOptions {
  excludeMd?: boolean;
  excludeBunLock?: boolean;
  excludeNodeModules?: boolean;
  excludeTarget?: boolean;
  includeMd?: boolean;
}

async function countLines(options: CountOptions = {}): Promise<void> {
  const {
    excludeMd = false,
    excludeBunLock = true,
    excludeNodeModules = true,
    excludeTarget = true,
    includeMd = false,
  } = options;

  const patterns = ["**/*"];
  const ignorePatterns = [];

  if (excludeMd) {
    ignorePatterns.push("**/*.md");
  }

  if (excludeBunLock) {
    ignorePatterns.push("**/bun.lock");
  }

  if (excludeNodeModules) {
    ignorePatterns.push("**/node_modules/**");
  }

  if (excludeTarget) {
    ignorePatterns.push("**/target/**");
  }

  ignorePatterns.push("**/.git/**");

  const files = await glob(patterns, {
    ignore: ignorePatterns,
    nodir: true,
    absolute: false,
  });

  let totalLines = 0;
  const fileCounts: Record<string, number> = {};

  for (const file of files) {
    try {
      const content = fs.readFileSync(file, "utf-8");
      const lines = content.split("\n").length;
      totalLines += lines;

      const ext = path.extname(file).toLowerCase();
      fileCounts[ext] = (fileCounts[ext] || 0) + lines;
    } catch {
      console.warn(`Could not read file: ${file}`);
    }
  }

  console.log(`Total files: ${files.length.toString()}`);
  console.log(`Total lines: ${totalLines.toString()}`);
  console.log("\nLines by file type:");

  const sortedExtensions = Object.entries(fileCounts).sort(
    (a, b) => b[1] - a[1],
  );
  for (const [ext, count] of sortedExtensions) {
    const percentage = ((count / totalLines) * 100).toFixed(1);
    console.log(
      `  ${ext || "(no extension)"}: ${count.toString()} lines (${percentage}%)`,
    );
  }

  if (includeMd && excludeMd) {
    const mdFiles = await glob("**/*.md", {
      ignore: ["**/.git/**", "**/node_modules/**", "**/target/**"],
      nodir: true,
    });

    let mdLines = 0;
    for (const file of mdFiles) {
      try {
        const content = fs.readFileSync(file, "utf-8");
        mdLines += content.split("\n").length;
      } catch {
        console.warn(`Could not read markdown file: ${file}`);
      }
    }

    console.log(`\nMarkdown files (excluded from main count):`);
    console.log(`  Files: ${mdFiles.length.toString()}`);
    console.log(`  Lines: ${mdLines.toString()}`);
    console.log(
      `  Total with markdown: ${(totalLines + mdLines).toString()} lines`,
    );
  }
}

const args = process.argv.slice(2);
const options: CountOptions = {};

for (const arg of args) {
  if (arg === "--exclude-md") {
    options.excludeMd = true;
  } else if (arg === "--include-md") {
    options.includeMd = true;
  } else if (arg === "--keep-bun-lock") {
    options.excludeBunLock = false;
  } else if (arg === "--keep-node-modules") {
    options.excludeNodeModules = false;
  } else if (arg === "--keep-target") {
    options.excludeTarget = false;
  } else if (arg === "--help" || arg === "-h") {
    console.log(`
Usage: bun run scripts/count-lines.ts [options]

Options:
  --exclude-md           Exclude markdown files from count
  --include-md           Show markdown file count separately
  --keep-bun-lock        Include bun.lock file (excluded by default)
  --keep-node-modules    Include node_modules (excluded by default)
  --keep-target          Include target directories (excluded by default)
  --help, -h            Show this help message

Default behavior:
  - Excludes: .git, node_modules, target, bun.lock
  - Includes: all other files including markdown
`);
    process.exit(0);
  }
}

countLines(options).catch(console.error);
