// The markdown-it plugin that turns the record's own references into links on the site.
//
// - `milestone N` and `§N` in prose link to /roadmap/N/ and /decisions/N/, when that record
//   exists. A citation of a number with no record is counted and left as text.
// - A bare `#N` links to that pull request or issue in nifeos/nife, where the source text means it.
// - A relative link or image resolves against the source file's repository path. A target on the
//   site becomes its stable address (lib/address.js). A file the site does not carry but the
//   repository does is an off-record link: it is counted and rendered as plain text, with the
//   original target in its title. A target that exists nowhere is a break: counted, kept as an
//   example, and rendered as plain text.
//
// The counts land in `stats`, which site/build.mjs writes to the build's report.
//
// Name: provisional, minted 2026-11-04 (UTC) by lane/record-site-build-eleventy.

"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { addressFor, numbersIn } = require("./address");

const EXAMPLE_LIMIT = 40;

function createPlugin({ input, repository, onSite }) {
  const known = {
    milestone: numbersIn(onSite, "design/roadmap/"),
    decision: numbersIn(onSite, "design/decisions/"),
  };
  const onSiteSet = new Set(onSite);
  const stats = {
    citations: 0,
    unresolvedCitations: 0,
    unresolvedExamples: [],
    pullRequestReferences: 0,
    relativeLinks: 0,
    inRecordBreaks: 0,
    inRecordBreakExamples: [],
    offRecordLinks: 0,
    offRecordExamples: [],
    assets: new Set(),
  };

  const CITATION = /\b([Mm]ilestones?) (\d+)|§(\d+)|(^|[\s(])#(\d{1,5})\b/g;

  function note(list, entry) {
    if (list.length < EXAMPLE_LIMIT) list.push(entry);
  }

  function repositoryPathOf(env) {
    return path.relative(input, path.resolve(env.page.inputPath)).split(path.sep).join("/");
  }

  function existsInRepository(relative) {
    return fs.existsSync(path.join(repository, relative));
  }

  // Resolves a relative href from the file at `from`, both repository paths. Returns the new href,
  // or null for a target the site does not carry (counted here).
  function resolve(href, from) {
    stats.relativeLinks++;
    const [file, fragment] = href.split("#");
    const suffix = fragment ? "#" + fragment : "";
    const target = path.posix.normalize(path.posix.join(path.posix.dirname(from), decodeURI(file)));
    const clean = target.replace(/\/$/, "");
    if (onSiteSet.has(clean)) {
      const address = addressFor(clean);
      if (address) return address + suffix;
      stats.assets.add(clean);
      return "/files/" + clean;
    }
    if (onSiteSet.has(clean + "/README.md")) return addressFor(clean + "/README.md") + suffix;
    if (!clean.startsWith("../") && existsInRepository(clean)) {
      stats.offRecordLinks++;
      note(stats.offRecordExamples, `${from} -> ${href}`);
      return null;
    }
    stats.inRecordBreaks++;
    note(stats.inRecordBreakExamples, `${from} -> ${href}`);
    return null;
  }

  function isRelative(href) {
    return href && !/^[a-z][a-z0-9+.-]*:|^#|^\//i.test(href);
  }

  function citationsRule(state) {
    const Token = state.Token;
    const text = (content) => {
      const token = new Token("text", "", 0);
      token.content = content;
      return token;
    };
    for (const block of state.tokens) {
      if (block.type !== "inline" || !block.children) continue;
      let depth = 0;
      const out = [];
      for (const token of block.children) {
        if (token.type === "link_open") depth++;
        if (token.type === "link_close") depth--;
        if (token.type !== "text" || depth > 0) {
          out.push(token);
          continue;
        }
        let last = 0;
        CITATION.lastIndex = 0;
        let match;
        while ((match = CITATION.exec(token.content))) {
          let href;
          let start = match.index;
          let label = match[0];
          if (match[5]) {
            stats.pullRequestReferences++;
            start += match[4].length;
            label = "#" + match[5];
            href = `https://github.com/nifeos/nife/issues/${match[5]}`;
          } else {
            stats.citations++;
            const isMilestone = Boolean(match[1]);
            const number = String(Number(match[2] || match[3]));
            if (!known[isMilestone ? "milestone" : "decision"].has(number)) {
              stats.unresolvedCitations++;
              note(stats.unresolvedExamples, `${repositoryPathOf(state.env)}: ${match[0]}`);
              continue;
            }
            href = `/${isMilestone ? "roadmap" : "decisions"}/${number}/`;
          }
          if (start > last) out.push(text(token.content.slice(last, start)));
          const open = new Token("link_open", "a", 1);
          open.attrs = [["href", href], ["class", "cite"]];
          out.push(open, text(label), new Token("link_close", "a", -1));
          last = start + label.length;
        }
        if (last === 0) out.push(token);
        else if (last < token.content.length) out.push(text(token.content.slice(last)));
      }
      block.children = out;
    }
  }

  function plugin(md) {
    md.core.ruler.push("nife_citations", citationsRule);
    const renderToken = (tokens, index, options, env, self) => self.renderToken(tokens, index, options);
    const linkOpen = md.renderer.rules.link_open || renderToken;
    md.renderer.rules.link_open = (tokens, index, options, env, self) => {
      const token = tokens[index];
      const href = token.attrGet("href");
      if (isRelative(href)) {
        const resolved = resolve(href, repositoryPathOf(env));
        if (resolved) token.attrSet("href", resolved);
        else {
          token.attrs = token.attrs.filter(([name]) => name !== "href");
          token.attrSet("class", "offsite");
          token.attrSet("title", href);
        }
      }
      return linkOpen(tokens, index, options, env, self);
    };
    const image = md.renderer.rules.image;
    md.renderer.rules.image = (tokens, index, options, env, self) => {
      const token = tokens[index];
      const source = token.attrGet("src");
      if (isRelative(source)) {
        const resolved = resolve(source, repositoryPathOf(env));
        token.attrSet("src", resolved || "");
      }
      return image(tokens, index, options, env, self);
    };
  }

  return { plugin, stats };
}

module.exports = { createPlugin };
