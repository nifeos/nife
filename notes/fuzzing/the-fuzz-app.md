# The fuzz host's GitHub App: a runbook for the person who creates it

*Name: provisional, minted 2026-11-01 (UTC) by lane/fuzz-findings-without-a-person. The App's name,
`nife-fuzz`, is also provisional. No architect has ruled on either.*

An appendix to [`continuous-host.md`](continuous-host.md). The continuous fuzzing host publishes a
daily release and files and publishes private security advisories with no person in the loop. It
acts as one GitHub App, `nife-fuzz`, and holds that App's private key. An architect ruled this on
2026-11-01 (UTC) in
[`fuzz-findings-without-a-person`](../../design/roadmap/proposals/fuzz-findings-without-a-person.md).

The App exists. A person created it on 2026-11-01 (UTC) and installed it on nifeos/nife only. Its
App ID is 5571801 and its installation ID is 177641059; neither is secret. The steps below stay as
the procedure for replacing it or its key. A person does every step here. No lane creates the App,
handles the key, reads it or looks for it.

## Before you start: what the key can do

Read this first. It is the exposure you accept by placing the key.

- The host runs main's code unattended. Its supervisor fast-forwards to each green commit on main
  and re-executes itself as the fuzz account, so anything merged to main runs on the host within one
  window.
- The key stays root-only, and the code that reads it does not update itself. The fuzz account can
  still read the current installation token, which lives one hour, and the host mints a new one every
  hour. Code on main can therefore act as the App for as long as it runs.
- With Contents write, a token can create, move or delete any `fuzz-*` tag and any release on
  nifeos/nife. The ruleset "tags: only fuzz-* tags may be made" refuses creating, updating or
  deleting any other tag, and it has no bypass actor. An architect accepted the remaining exposure on
  2026-11-01 (UTC) on that condition.
- It cannot push to main: main's ruleset requires a pull request and has no bypass actor.
- With Repository security advisories write, a token can create, edit and publish any advisory on
  nifeos/nife, including a held finding before its fix.
- The App reaches no other repository. It holds no Workflows, Administration or other permission,
  and neither ruleset lists it as a bypass actor.
- The key does not expire, so nothing rotates it on a schedule. Revoke it when you suspect a leak.

## Create the App

1. In the nifeos organization's settings, open Developer settings, then GitHub Apps, then New GitHub
   App.
2. Name: `nife-fuzz`. Homepage: the nifeos/nife repository page.
3. Switch the webhook off. The App has no server.
4. Repository permissions: Contents, Read and write. Repository security advisories, Read and
   write. Leave every other permission at No access. Metadata stays Read-only, which GitHub sets on
   every App.
5. Organization and account permissions: none.
6. Under "Where can this GitHub App be installed", choose only on this account.
7. Create the App. Note the App ID shown at the top of its settings page.

Check the permission list once more on the App's settings page. It must show exactly Contents,
Repository security advisories and Metadata.

## Install it on one repository

1. On the App's page, choose Install App, then the nifeos organization.
2. Choose Only select repositories, and select nifeos/nife alone.
3. Install. The address of the installation's settings page ends in the installation ID; note it.

## Generate the key and place it on the host

1. Before any key exists, run the throwaway-repository test that the ruling's proposal lists. It
   confirms that an App with the advisory permission alone can create an advisory.
2. On the App's settings page, under Private keys, generate a private key. The browser downloads one
   `.pem` file.
3. Copy it to the fuzz host over a channel you already trust, straight into a directory only root
   can enter. Make the file owned by root with mode 0400.
4. Delete every other copy: the download, any temporary copy, the trash.
5. Write the App ID (5571801) and the installation ID (177641059) into the host's fuzz
   configuration. Neither is secret.
6. Install the mint helper and enable its timer, then enable the services that read its token.
   [`continuous-host.md`](continuous-host.md) lists the units.

Never paste the key anywhere else. That covers a chat, a pull request, an issue, a comment, a lane's
environment, a repository secret, a log, a shell history and a clipboard manager.

## Confirm it works

1. Run `helpers/fuzz-daily-publish.sh --dry-run` as the fuzz account. It prints the tag and the
   assets and calls nothing.
2. Set `NIFE_FUZZ_PUBLISH=1` and enable the publish timer. The next day a release
   `fuzz-YYYY-MM-DD` appears on nifeos/nife, made by `nife-fuzz[bot]`.

## Revoke or replace the key

On the App's settings page, generate a new key first, place it as above, then delete the old one.
GitHub refuses to delete an App's only key. To stop the host acting at all, suspend or uninstall the
App on nifeos/nife.

## BUGS

- Nobody has followed the key steps yet. The first person to do so confirms each one or corrects it.
- The mint helper and the filing service do not exist yet. The ruling's proposal lists them as proposed
  milestones.
- The read-only credential for fix lanes is not covered here. Its form is an open question.
- Nobody has confirmed that an App with the advisory permission alone may create an advisory. GitHub's
  documentation names "a security manager or administrator" as the caller. The throwaway-repository
  test answers it before any key exists.
- A nife release that uses a tag outside `fuzz-*` needs the tag ruleset amended first, as its own
  step recorded in a pull request.
