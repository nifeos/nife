---
status: PARTIAL
raised: 2026-09-19
promoted_from: a-stick-that-boots-with-secure-boot-on
milestone_dependencies: none
decision_dependencies: none
machine_requirements: an x86_64 UEFI PC that is not a Dell, with Secure Boot on
specific_machine: none
needs_person: yes
---
# 500. A stick that boots with Secure Boot on, or a page that says how to turn it off

The nife boot file a stranger copies to a USB stick, `\EFI\BOOT\BOOTX64.EFI`, is not signed. A PC
with Secure Boot on refuses it. DECISIONS §157 (a trivial install is a web page, a USB drive, and
packages over the internet) measures its x86 row with "Secure Boot off", and this milestone owns
what a stranger does about that.

## The ruling

calef, 2026-10-18 at 23:10 (UTC), asked what the trivial-install page does about Secure Boot:
*"K1: turn it off"*. The page tells the stranger to turn Secure Boot off as its first instruction,
not in a footnote, with per-vendor steps and the BitLocker recovery-key warning ahead of them.

K1 adds no key. The only long-lived key the tree plans is basalt's TUF root, which signs basalt's
package index. That is §195 (a reviewed recipe vouches for a package, and the machine's owner may
overrule it) as amended 2026-10-10. The same amendment narrows clause 1 of §220 (signed builds: a
vendor signs, a developer self-signs, and trusting a key is scoped) so that an image carries that
root and no other key. A boot key would be a second long-lived key, and the first one other
people's firmware trusts.

## The instructions

[`notes/turning-secure-boot-off.md`](../../notes/turning-secure-boot-off.md) is the page's first
instruction, written to be carried onto it as it stands. It leads with the BitLocker recovery key,
then how to reach the firmware settings, then the steps for Dell, HP, Lenovo, ASUS and Acer. Each
step cites the vendor page it was read from on 2026-10-18 (UTC), and each it could not confirm is
marked. Rung 4 of milestone 802 (the trivial install that makes a second customer possible),
publishing the page, needs this note. Rungs 1 to 3 on xenon do not: xenon's Secure Boot is already
off.

`stick_maker`'s closing line and the stick's `NIFE.TXT` say "turn Secure Boot off" and no more,
because the page is where a stranger reads how.

Reuse: Microsoft's and each vendor's own instructions, cited rather than restated in full; no
existing page covers several vendors from a primary source.

## Done when

1. A note holds the steps a stranger follows, BitLocker recovery key first, with each vendor step
   cited to the vendor's own page. Met.
2. Each vendor row has been followed on a PC of that vendor with Secure Boot on, or the vendor's
   page has been read in full. Not met: only a Dell (xenon) has been checked at a machine, and
   the Acer row rests on a search excerpt, because Acer's page sits behind a bot check. Every
   other row is read in full, but several vendor pages give no step for turning Secure Boot off
   (Lenovo IdeaPad, the Surface consumer pages, MSI and Gigabyte desktop boards), and the note marks
   each. One non-Dell PC followed through the note, which milestone 802's rung 1d supplies, closes
   the largest part.

## Refused, and what would reopen each

| | What the stranger does | What nife does | Why refused |
|---|---|---|---|
| **K2. A shim signed by Microsoft's third-party UEFI CA** | Nothing, on most PCs | Ships a signed first-stage loader (`shim`) carrying nife's own key, and signs `BOOTX64.EFI` with that key | A private key whose compromise lets anyone boot code on every machine that trusts it, and which cannot be withdrawn once a signed shim is public: revocation runs through firmware revocation lists nife does not control |
| **K3. The owner enrolls nife's key** | Enrolls a key through the firmware's setup or a machine-owner-key tool | Signs with its own key, publishes the certificate | K2's key custody without K2's reach, and more steps for the stranger than K1 |

Either reopens only by an architect's ruling that creates a boot key, beside basalt's root or as
the same key. That ruling would also answer whether the two are one key; the safer default is
two. K2 needs two further answers: the review process Linux distributions use to get a shim signed
(recalled, not read), and DECISIONS §135 (running GPL software) requirement 1, "no conveyed artifact
carries copyleft", against shim's license (recalled as GPL, not read). K3 reopens most cheaply,
for owners who want Secure Boot kept on, and it composes with K1 on the same page.

## What the tree does in the analogous case

The measured-boot chain (milestone 104 (the measurement continues past init), DECISIONS §26 (the
fault endpoint)) chose a hash over a signature for K2's reason: *"today they are built by one
command in one tree in one sequence, so the hash is strictly better"*, and key custody was the
objection §26 left standing.

## Scope note (architectural parity)

This covers x86_64 PCs only. aarch64 and riscv64 machines a stranger plausibly owns boot the same
stick through U-Boot or EDK2, whose Secure Boot behavior was not looked at. Milestone 802's scope
note carries the plan for those architectures.

## BUGS

- No stranger has followed the note, and every vendor row but Dell's is the vendor's page, not
  a machine. The note's own `BUGS` lists which pages were read in full.
- That turning Secure Boot off triggers BitLocker recovery is an inference from Microsoft's
  pages and a general statement by ASUS; the note gives the sentences it rests on.
- Acer's page is unread. It returned a bot check on 2026-10-21 (UTC), and Acer publishes no
  other page on the subject that was found.
- No Lenovo-owned page names the IdeaPad Secure Boot setting, and the Surface, MSI and Gigabyte
  desktop-board rows have no off step on the vendor's pages. The note marks each.

## Follow-on

- **Outstanding.** Follow the note on one PC that is not a Dell, with Secure Boot and BitLocker
  on, and correct the note by what the machine shows. Checked 2026-10-18 (UTC): the note's `BUGS`
  names xenon as the only machine it has been held against.
- **Done.** Checked 2026-10-21 (UTC): the Lenovo IdeaPad page was read in full; it says how to
  enter setup and nothing about Secure Boot, so the note's row now says so. Surface, MSI and
  Gigabyte rows were added from the vendors' own pages.
- **Outstanding.** Read Acer's page in full. It returned a bot check on 2026-10-21 (UTC) and was
  left unread; a person reading it in a browser closes this.
- **Milestone 802.** Carrying the note onto the published page, ahead of every other instruction.

## Index row

The first instruction a stranger meets on the trivial-install page: turn Secure Boot off, after
writing down the BitLocker recovery key. calef ruled K1 on 2026-10-18 (UTC), so nife holds no boot
key; the steps are in `notes/turning-secure-boot-off.md`, cited to each vendor's page.
