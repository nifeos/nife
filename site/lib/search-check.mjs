// Runs a few searches against the built Pagefind index, in Node, the way a browser would: the
// index's own client (pagefind/pagefind.js in the built site) with `fetch` answered from disk. It
// reports whether each query found pages, and how many bytes the first search fetched, which is
// what a reader's first search costs over the network.
//
// Name: provisional, minted 2026-11-04 (UTC) by lane/record-site-build-eleventy.

import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

// Each names something the record holds, so a query with no hit means the index lost it.
const QUERIES = [
  "page table",
  "frame allocator",
  "stable addresses",
  "Kani harness",
  "login to kilo",
  "private advisory",
  "register constants",
];

export async function searchCheck(root) {
  let bytes = 0;
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url) => {
    const file = path.join(root, new URL(String(url), "http://site/").pathname);
    const body = fs.readFileSync(file);
    bytes += body.length;
    return new Response(body, { status: 200 });
  };
  try {
    const pagefind = await import(pathToFileURL(path.join(root, "pagefind", "pagefind.js")).href);
    await pagefind.options({ basePath: "/pagefind/" });
    const results = [];
    let firstSearchBytes = null;
    for (const query of QUERIES) {
      const found = await pagefind.search(query);
      const top = found.results.length ? await found.results[0].data() : null;
      results.push({ query, hits: found.results.length, top: top ? top.url : null });
      if (firstSearchBytes === null) firstSearchBytes = bytes;
    }
    return { results, firstSearchBytes };
  } finally {
    globalThis.fetch = previousFetch;
  }
}
