// Eleventy's configuration for the design record's site. site/build.mjs stages the tracked
// Markdown into an input directory laid out as the repository is, so a file's path under the input
// is its repository path, and passes that directory and the list of staged files through the
// environment. See script/site.
//
// Name: provisional, minted 2026-11-04 (UTC) by lane/record-site-build-eleventy. The file name is
// the one Eleventy looks for.

"use strict";

const fs = require("node:fs");
const path = require("node:path");
const anchor = require("markdown-it-anchor");
const { addressFor } = require("./lib/address");
const { createPlugin } = require("./lib/markdown");

// Heading identifiers as GitHub writes them, so a `#fragment` written against GitHub's rendering
// lands on the same heading here.
function slugify(text) {
  return String(text)
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");
}

module.exports = function configure(config) {
  const input = process.env.NIFE_SITE_INPUT;
  const repository = process.env.NIFE_SITE_REPOSITORY;
  const onSite = fs.readFileSync(process.env.NIFE_SITE_FILES, "utf8").split("\n").filter(Boolean);
  const commit = process.env.NIFE_SITE_COMMIT || "main";
  const markdown = createPlugin({ input, repository, onSite });

  // The input sits under target/, which .gitignore names, so Eleventy must not read .gitignore.
  config.setUseGitIgnore(false);
  config.amendLibrary("md", (md) => md.use(anchor, { slugify }).use(markdown.plugin));

  const repositoryPath = (data) =>
    path.relative(input, path.resolve(data.page.inputPath)).split(path.sep).join("/");
  const isIndex = (data) => repositoryPath(data) === "index.njk";

  config.addGlobalData("eleventyComputed", {
    permalink: (data) => (isIndex(data) ? "/index.html" : addressFor(repositoryPath(data)) + "index.html"),
    sourcePath: (data) => repositoryPath(data),
    sourceUrl: (data) => `https://github.com/nifeos/nife/blob/${commit}/${repositoryPath(data)}`,
    kind: (data) => (isIndex(data) ? "" : addressFor(repositoryPath(data)).split("/")[1]),
    title: (data) => {
      const heading = /^#\s+(.+)$/m.exec(data.page.rawInput || "");
      return heading ? heading[1].replace(/`/g, "") : data.page.fileSlug;
    },
    layout: (data) => (isIndex(data) ? false : "page.njk"),
  });

  config.addCollection("record", (api) => api.getFilteredByGlob("**/*.md"));
  config.addFilter("byKind", (pages, kind) =>
    pages
      .filter((page) => page.data.kind === kind)
      .sort((a, b) => a.url.localeCompare(b.url, undefined, { numeric: true })),
  );
  config.addFilter("kinds", (pages) => [...new Set(pages.map((page) => page.data.kind))].sort());

  config.on("eleventy.after", () => {
    const { assets, ...counts } = markdown.stats;
    const report = { ...counts, assets: [...assets].sort() };
    fs.writeFileSync(process.env.NIFE_SITE_MARKDOWN_STATS, JSON.stringify(report, null, 2));
  });

  return {
    dir: { input, includes: "_includes", output: process.env.NIFE_SITE_OUTPUT },
    templateFormats: ["md", "njk"],
    markdownTemplateEngine: false,
    htmlTemplateEngine: "njk",
  };
};
