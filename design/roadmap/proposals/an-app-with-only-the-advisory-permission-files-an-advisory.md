---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# An App with only the advisory permission files an advisory

Raised by lane/fuzz-findings-without-a-person on 2026-11-01 (UTC), from the open question in
[fuzz-findings-without-a-person](fuzz-findings-without-a-person.md): GitHub's documentation says the
caller that creates a repository security advisory "must be a security manager or administrator",
and does not say whether an App installation with the advisory permission alone qualifies.

Reuse: `gh api` and the token code of `helpers/smelter-token`; no new code beyond a short script.

## What it builds

A test on a throwaway public repository in the nifeos organization, run before any `nife-fuzz` key
reaches the fuzz host.

1. A person creates the throwaway repository and a throwaway App that holds Repository security
   advisories write and nothing else, installed on that repository only.
2. The test mints an installation token and calls the create endpoint with a fake advisory that
   names no real finding. Then it adds a comment, publishes the advisory, and reads the public page
   without a credential, which also answers whether the public sees the comments.
3. The person deletes the throwaway App and repository, and the result goes into
   [`notes/fuzzing/the-fuzz-app.md`](../../../notes/fuzzing/the-fuzz-app.md) and the filing
   milestone.

If creation fails, the filing milestone stops and an architect decides what the App needs instead.

## How it is checked

The recorded HTTP status of each call, and the public page read without a credential.

## Status

Not started. It needs a person: it creates a repository and an App, which no lane does.
