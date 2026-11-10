// The address of each record file on the site, derived from its repository path alone.
//
// Milestone 1018 (the design record moves to `nifeos/record` and is published as a website) asks for
// one stable address per record number, with no slug in it, because titles are drafts. So a
// numbered record is its kind and its number: design/roadmap/1018-the-design-record-moves.md is
// /roadmap/1018/, and a file in that block's appendix directory sits under it, as
// /roadmap/1018/measurements/. Every other file keeps its path: design/ drops its prefix, so
// design/naming.md is /naming/, and notes/ and briefs/ keep theirs. A README is the leaf `readme`.
// The two repository-root documents the record links to most take their lowercased stem:
// AGENTS.md is /agents/ and ARCHITECTS.md is /architects/.
//
// Name: provisional, minted 2026-11-04 (UTC) by lane/record-site-build-eleventy.

"use strict";

// The repository directories the site is built from, and the root documents it carries.
const ROOTS = ["design/", "notes/", "briefs/"];
const ROOT_DOCUMENTS = ["AGENTS.md", "ARCHITECTS.md"];

// Record directories whose files carry a number, and the first address segment each takes.
const NUMBERED = [
  ["design/roadmap/", "roadmap"],
  ["design/decisions/", "decisions"],
  ["design/journeys/", "journeys"],
  ["design/fatal-risks/", "fatal-risks"],
];

// The record number a file or directory name leads with, without its leading zeros.
function recordNumber(name) {
  const match = /^(\d+)-/.exec(name);
  return match ? String(Number(match[1])) : null;
}

function leaf(stem) {
  return stem === "README" ? "readme" : stem;
}

function join(segments) {
  return "/" + segments.join("/") + "/";
}

// The address of a Markdown file given as a repository-relative path, or null when the file is
// not on the site.
function addressFor(repositoryPath) {
  if (ROOT_DOCUMENTS.includes(repositoryPath)) return join([repositoryPath.slice(0, -".md".length).toLowerCase()]);
  if (!repositoryPath.endsWith(".md")) return null;
  if (!ROOTS.some((root) => repositoryPath.startsWith(root))) return null;
  const stem = repositoryPath.slice(0, -".md".length);
  for (const [directory, kind] of NUMBERED) {
    if (!stem.startsWith(directory)) continue;
    const rest = stem.slice(directory.length).split("/");
    const number = recordNumber(rest[0]);
    if (number === null) return join([kind, ...rest.map(leaf)]);
    return join([kind, number, ...rest.slice(1).map(leaf)]);
  }
  const segments = stem.split("/");
  return join((segments[0] === "design" ? segments.slice(1) : segments).map(leaf));
}

// The numbers of the top-level files in one numbered directory, from a list of repository paths.
function numbersIn(repositoryPaths, directory) {
  const numbers = new Set();
  for (const path of repositoryPaths) {
    if (!path.startsWith(directory) || !path.endsWith(".md")) continue;
    const rest = path.slice(directory.length);
    if (rest.includes("/")) continue;
    const number = recordNumber(rest);
    if (number !== null) numbers.add(number);
  }
  return numbers;
}

module.exports = { ROOTS, ROOT_DOCUMENTS, addressFor, numbersIn, recordNumber };
