// Builds the design record as a static site, indexes it for search, checks its links and writes a
// report. script/site runs this after `npm ci`; the header there says what the site is for and how
// to run it.
//
// Every count here is a report and never a verdict: the build exits non-zero only when a step
// itself fails, not when it finds a broken link (milestone 1018 (the design record moves to
// `nifeos.org` and is published as a website), step 3).
//
// Name: provisional, minted 2026-11-04 (UTC) by lane/record-site-build-eleventy.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { searchCheck } from "./lib/search-check.mjs";

const require = createRequire(import.meta.url);
const { checkLinks } = require("./lib/link-check.js");

// script/site copies site/ to target/site/builder and installs the packages there, so no
// node_modules directory sits in the source tree for the tree's own checks to walk. So `here` is
// that copy, and the repository comes from the environment.
const here = path.dirname(fileURLToPath(import.meta.url));
const repository = path.resolve(process.env.NIFE_SITE_REPOSITORY || path.join(here, ".."));
const work = path.resolve(process.env.NIFE_SITE_WORK || path.join(repository, "target", "site", "build"));
const input = path.join(work, "input");
const output = path.join(work, "site");
const ROOTS = ["design", "notes", "briefs"];

function seconds(start) {
  return Number(process.hrtime.bigint() - start) / 1e9;
}

function directoryBytes(directory) {
  let bytes = 0;
  let files = 0;
  for (const entry of fs.readdirSync(directory, { withFileTypes: true, recursive: true })) {
    if (!entry.isFile()) continue;
    bytes += fs.statSync(path.join(entry.parentPath, entry.name)).size;
    files++;
  }
  return { bytes, files };
}

function stage() {
  fs.rmSync(work, { recursive: true, force: true });
  fs.mkdirSync(path.join(input, "_includes"), { recursive: true });
  const listed = execFileSync("git", ["-C", repository, "ls-files", "-z", "--", ...ROOTS], { encoding: "utf8" });
  const tracked = listed.split("\0").filter((file) => file && fs.existsSync(path.join(repository, file)));
  let markdown = 0;
  let words = 0;
  for (const file of tracked) {
    if (!file.endsWith(".md")) continue;
    const text = fs.readFileSync(path.join(repository, file), "utf8");
    fs.mkdirSync(path.join(input, path.dirname(file)), { recursive: true });
    fs.writeFileSync(path.join(input, file), text);
    markdown++;
    words += text.split(/\s+/).filter(Boolean).length;
  }
  fs.copyFileSync(path.join(here, "layouts", "page.njk"), path.join(input, "_includes", "page.njk"));
  fs.copyFileSync(path.join(here, "layouts", "index.njk"), path.join(input, "index.njk"));
  const filesList = path.join(work, "files-on-site.txt");
  fs.writeFileSync(filesList, tracked.join("\n") + "\n");
  return { tracked, markdown, words, filesList };
}

function commit() {
  try {
    return execFileSync("git", ["-C", repository, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  } catch {
    return "main";
  }
}

async function build(staged) {
  const markdownStats = path.join(work, "markdown-stats.json");
  Object.assign(process.env, {
    NIFE_SITE_INPUT: input,
    NIFE_SITE_OUTPUT: output,
    NIFE_SITE_REPOSITORY: repository,
    NIFE_SITE_FILES: staged.filesList,
    NIFE_SITE_COMMIT: process.env.NIFE_SITE_COMMIT || commit(),
    NIFE_SITE_MARKDOWN_STATS: markdownStats,
  });
  const { Eleventy } = await import("@11ty/eleventy");
  const eleventy = new Eleventy(undefined, undefined, {
    configPath: path.join(here, "eleventy.config.js"),
    quietMode: true,
  });
  const start = process.hrtime.bigint();
  await eleventy.write();
  const elapsed = seconds(start);
  const stats = JSON.parse(fs.readFileSync(markdownStats, "utf8"));
  for (const asset of stats.assets) {
    const target = path.join(output, "files", asset);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repository, asset), target);
  }
  fs.copyFileSync(path.join(here, "layouts", "style.css"), path.join(output, "style.css"));
  return { elapsed, stats };
}

async function index() {
  const pagefind = await import("pagefind");
  const start = process.hrtime.bigint();
  const { index: created, errors } = await pagefind.createIndex({});
  if (errors.length) throw new Error("pagefind: " + errors.join("; "));
  const added = await created.addDirectory({ path: output });
  if (added.errors.length) throw new Error("pagefind: " + added.errors.join("; "));
  const written = await created.writeFiles({ outputPath: path.join(output, "pagefind") });
  if (written.errors.length) throw new Error("pagefind: " + written.errors.join("; "));
  await pagefind.close();
  return { elapsed: seconds(start), pages: added.page_count };
}

function lockedPackages() {
  const lock = JSON.parse(fs.readFileSync(path.join(here, "package-lock.json"), "utf8"));
  const locked = Object.keys(lock.packages).filter(Boolean).length;
  const installed = fs.existsSync(path.join(here, "node_modules", ".package-lock.json"))
    ? Object.keys(JSON.parse(fs.readFileSync(path.join(here, "node_modules", ".package-lock.json"), "utf8")).packages).filter(Boolean).length
    : null;
  return { locked, installed };
}

const megabytes = (bytes) => (bytes / 1048576).toFixed(1) + " MB";

function summary(report) {
  const { markdown, links, search } = report;
  const breaks = markdown.inRecordBreaks + links.missingPages + links.missingFragments;
  const lines = [
    "## Design record site (report only)",
    "",
    "Built from `design/`, `notes/` and `briefs/` at `" + report.commit.slice(0, 12) + "`. Nothing is deployed; the site is this run's artifact.",
    "",
    "| measure | value |",
    "|---|---|",
    `| Eleventy build | ${report.build.toFixed(1)} s |`,
    `| Pagefind index | ${report.pagefind.toFixed(1)} s |`,
    `| pages | ${links.pages} (from ${report.source.markdown} Markdown files, ${report.source.words.toLocaleString("en-US")} words) |`,
    `| search index | ${megabytes(report.index.bytes)} in ${report.index.files} files; a first search fetches ${(search.firstSearchBytes / 1024).toFixed(0)} KB |`,
    `| whole site | ${megabytes(report.site.bytes)} in ${report.site.files} files |`,
    `| citations (\`milestone N\`, \`§N\`) | ${markdown.citations}, of which ${markdown.unresolvedCitations} name no record |`,
    `| in-record breaks | ${breaks}: ${markdown.inRecordBreaks} relative links to no file, ${links.missingPages} links to no page, ${links.missingFragments} fragments to no heading |`,
    `| off-record links | ${markdown.offRecordLinks} links to repository files the site does not carry, shown as plain text |`,
    `| lockfile | ${report.packages.locked} packages locked${report.packages.installed === null ? "" : `, ${report.packages.installed} installed on this platform`} |`,
    "",
    "Searches: " + search.results.map((result) => `\`${result.query}\` ${result.hits}`).join(", ") + ".",
  ];
  const examples = (title, list) => {
    if (!list.length) return;
    lines.push("", `<details><summary>${title} (first ${list.length})</summary>`, "", "```", ...list, "```", "", "</details>");
  };
  examples("Citations that name no record", markdown.unresolvedExamples);
  examples("Relative links to no file", markdown.inRecordBreakExamples);
  examples("Links to no page or heading", links.examples);
  examples("Off-record links", markdown.offRecordExamples);
  return lines.join("\n") + "\n";
}

const staged = stage();
const built = await build(staged);
const indexed = await index();
const links = checkLinks(output);
const search = await searchCheck(output);
const report = {
  commit: process.env.NIFE_SITE_COMMIT,
  source: { markdown: staged.markdown, words: staged.words, tracked: staged.tracked.length },
  build: built.elapsed,
  pagefind: indexed.elapsed,
  pagefindPages: indexed.pages,
  index: directoryBytes(path.join(output, "pagefind")),
  site: directoryBytes(output),
  markdown: built.stats,
  links,
  search,
  packages: lockedPackages(),
};
fs.writeFileSync(path.join(work, "report.json"), JSON.stringify(report, null, 2) + "\n");
const text = summary(report);
fs.writeFileSync(path.join(work, "summary.md"), text);
if (process.env.GITHUB_STEP_SUMMARY) fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, text);
process.stdout.write(text);
process.stdout.write(`\nsite: ${output}\n`);
