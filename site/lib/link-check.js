// The built site's internal link check: every href that names a page on the site reaches a page
// that exists, and every fragment names an `id` on that page. External links are not fetched; a
// remote outage says nothing about the record.
//
// The check reads the HTML the build wrote, so it also catches what lib/markdown.js could not see:
// a citation link to a record whose page failed to render, or a fragment written against a heading
// that has since changed. It counts and returns; site/build.mjs reports.
//
// Name: provisional, minted 2026-11-04 (UTC) by lane/record-site-build-eleventy.

"use strict";

const fs = require("node:fs");
const path = require("node:path");

const EXAMPLE_LIMIT = 40;

function htmlFiles(root) {
  const found = [];
  const walk = (directory) => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const full = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        if (entry.name !== "pagefind" && entry.name !== "files") walk(full);
      } else if (entry.name.endsWith(".html")) found.push(full);
    }
  };
  walk(root);
  return found;
}

function checkLinks(root) {
  const files = htmlFiles(root);
  const identifiers = new Map();
  const pages = new Map();
  for (const file of files) {
    const html = fs.readFileSync(file, "utf8");
    pages.set(file, html);
    identifiers.set(file, new Set(Array.from(html.matchAll(/\bid="([^"]+)"/g), (match) => match[1])));
  }
  const report = { pages: files.length, internalLinks: 0, missingPages: 0, missingFragments: 0, examples: [] };
  const example = (entry) => {
    if (report.examples.length < EXAMPLE_LIMIT) report.examples.push(entry);
  };
  for (const [file, html] of pages) {
    const here = "/" + path.relative(root, path.dirname(file)).split(path.sep).join("/") + "/";
    for (const match of html.matchAll(/<a [^>]*href="([^"]*)"/g)) {
      const href = match[1].replace(/&amp;/g, "&");
      if (/^[a-z][a-z0-9+.-]*:/i.test(href)) continue;
      report.internalLinks++;
      const [target, fragment] = href.split("#");
      const resolved = target ? path.posix.normalize(path.posix.join(target.startsWith("/") ? "/" : here, target)) : here;
      if (resolved.startsWith("/files/") || resolved.startsWith("/pagefind/")) {
        if (!fs.existsSync(path.join(root, resolved))) {
          report.missingPages++;
          example(`${path.relative(root, file)} -> ${href}`);
        }
        continue;
      }
      const page = path.join(root, resolved, resolved.endsWith("/") ? "index.html" : "");
      if (!identifiers.has(page)) {
        report.missingPages++;
        example(`${path.relative(root, file)} -> ${href}`);
        continue;
      }
      if (fragment && !identifiers.get(page).has(decodeURIComponent(fragment))) {
        report.missingFragments++;
        example(`${path.relative(root, file)} -> ${href} (fragment)`);
      }
    }
  }
  return report;
}

module.exports = { checkLinks };
