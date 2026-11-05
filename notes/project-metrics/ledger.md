# The cost ledger: what was paid, and what a token lists for

**This file is appended to and never regenerated.** Every other file in this directory is written by
a script from evidence the tree already holds, and rerunning the script reproduces it byte for byte.
This one holds facts no script can derive: what a person paid, on what day, and what a vendor was
charging at the time. `script/metrics` reads it to fill two columns of [`cost.csv`](cost.csv) and
nothing writes it back.

So the rule is the one a ledger has always had. **Add a row; do not edit a row.** A figure that turns
out to be wrong gets a new row with a date and a note saying what it corrects, because the point of a
ledger is that somebody can see what was believed when.

Every date here is UTC, like every date in this tree.

## Hardware

Bought for this project, in the order it was bought.

| date | item | price (USD) | source |
|---|---|---|---|
| 2026-08-15 | **argon**, NVIDIA Jetson TX1 developer kit, used | 89.99 | milestone 127 (the seL4 machine: a Jetson TX1, so identical silicon referees the comparison), which records [the purchase and the delivery window](../../design/roadmap/0127-the-sel4-machine.md) |
| 2026-08-15 | **xenon**, Dell OptiPlex 7050 Micro (i5-7500T, 16 GB, 256 GB NVMe) with AC adapter | 139.00 | milestone 87 (the x86_64 bare-metal machine), whose [block](../../design/roadmap/0087-x86-machine.md) records the purchase |
| 2026-08-15 | xenon's Dell C4PDJ serial module, with cable | 18.88 | milestone 87 (the x86_64 bare-metal machine) |
| 2026-08-15 | FTDI USB-to-RS-232 adapter, 0.46 m, for xenon's dev side | 15.96 | milestone 87 (the x86_64 bare-metal machine) |
| 2026-08-15 | StarTech NM9FF null-modem barrel | 7.98 | milestone 87 (the x86_64 bare-metal machine) |
| | **recorded total** | **271.81** | |

### What is missing, named rather than estimated

- radon, the StarFive VisionFive 2, is the second ISA's board and no purchase record exists in
  this tree. `notes/riscv-port.md` says "a ~$70 StarFive VisionFive 2", which is a sentence about
  the market written before the board was chosen, not a receipt. The order date is not recorded
  either; the board arrived around 2026-08-21. This row is left out rather than filled with the $70,
  because a ledger that guesses is worth less than one with a hole in it. calef can close it in one
  line.
- The UART adapters and smart plugs on the bench rig are unpriced for the same reason. Milestone
  87 prices a smart plug at "$15 and works on anything" as part of an argument for choosing one, not
  as a record of buying one.
- Everything before 2026-08-15. The first five weeks of the project bought no hardware, which is
  a fact rather than a gap: it ran entirely under QEMU on a machine calef already owned.

The machine this is built on is not in this ledger and should not be. patagonia is calef's own
Mac and predates the project; charging a laptop he already had to nife would be inventing a cost.
The same goes for cordoba, which runs the family's backups and was never bought for this.

### A correction this ledger makes to milestone 519's own block

Milestone 519 (what this project costs, tracked where it cannot rot) has a table saying "xenon was
not bought for this project". That is wrong, and milestone 87's record is the reason. The OptiPlex
was selected on 2026-08-03 and bought on 2026-08-15 specifically as the dedicated bring-up machine
for the third ISA target that DECISIONS §19 (architectural parity is a tenet; the targets are
aarch64, riscv64, and x86_64) names. Its own block says it "completes when the machine has
printed a byte over serial". It is as much a project purchase as argon is, and the block saying
otherwise is wrong. Correcting it triples the recorded hardware figure, from $89.99 to $271.81, and
it is recorded here rather than silently. The block's number was written from memory in the
conversation that minted the milestone, and the tree disagreed.

## Subscriptions

| from | to | item | rate | source |
|---|---|---|---|---|
| 2026-07-12 | (current) | Claude subscription, the inference this project runs on | 200.00 / month | calef, 2026-09-21 |
| 2026-09-24 | (current) | GitHub Team plan, one seat: CI concurrency 20 → 60 jobs (Actions minutes stay free on a public repository) | 4.00 / month | calef, 2026-09-24 |
| 2026-09-29 | (current) | z.ai coding plan subscription, the glm-5.3 maintainer sessions and glm-5.3-flash lanes | 80.00 / month | invoice dated 2026-09-29, relayed by calef 2026-10-04; see the correction below |

The z.ai plan is expected to end after this month (calef, 2026-10-04: "probably won't renew").
Its first charge is the invoice dated 2026-09-29, which covers 2026-09-29 to 2026-10-28 on
calef's belief that the invoice is for one month (the invoice itself is not in the tree).
No later charge is recorded because none is expected. Once the plan lapses, a week
with no GLM tokens is the expected shape, not a gap in `effort.csv`. `script/metrics` keeps
charging every anniversary until this file says otherwise. So the first renewal that does not
happen needs a row here saying the plan ended. That row is still owed.

**Correction, 2026-10-04.** The z.ai row first read 2026-09-28, written from calef's 2026-09-29
message with no invoice date. The invoice is dated 2026-09-29. The row's start date was changed in
place rather than adding a second row, because `script/metrics` charges every row's anniversaries
and a second row would bill the plan twice. The charge stays in 2026W40 (28 September starts that
ISO week), so `cost.csv` is unchanged.

The other billing days are assumptions and the only ones in this file. Only the z.ai invoice date
is known, and no invoice is recorded in this tree, so `script/metrics` places each charge in the ISO week containing the monthly
anniversary of the start date: 2026-07-12, 2026-08-12, 2026-09-12, and so on. If the real billing
day is different, at most one charge lands in a neighboring week and the running total is unchanged.

Why `cash_spend` is lumpy, and why it disagrees with a figure you may have seen. Milestone 519's
block says "about $470 to date", which is 2.35 months of subscription accrued over the ten weeks
since the first commit. `cash_spend` is what was paid, so it puts $200 in three weeks and nothing
in the other seven, and sums to $600 by 2026-09-21. Both numbers are correct and they are answers to
different questions. The column is the paid one because milestone 519's own table defines it that
way, and because an accrual is derivable from this ledger by anyone who wants it while a payment is
not derivable from an accrual.

## Rented capacity

§203 (capacity is rented rather than bought), in practice: credit bought from a metered provider
and spent by the open-weight lanes and the model bake-offs. One row per purchase, hardware's
shape.

| date | item | price (USD) | source |
|---|---|---|---|
| 2026-09-27 | OpenRouter credit, the rented open-weight lanes and the bake-offs of 2026-09-26 to 27 | 115.31 | calef, 2026-09-29 |

## Retail rates per million tokens

This is a shadow price and nothing else. This project pays a fixed monthly subscription, so the
marginal cost of one more lane is zero dollars. These rates answer a different question, which is
what somebody without that subscription would pay a vendor to reproduce the same work. Milestone 519
says both figures are legitimate and that quoting one while implying the other is the dishonest
version. So the paid number is `cash_spend` above, and the shadow number is this table times the
token counts in [`effort.csv`](effort.csv). Neither is ever printed without its label.

Rates are list prices on the date recorded, in USD per million tokens.

| recorded | model | input | cache write 5m | cache write 1h | cache read | output | source |
|---|---|---|---|---|---|---|---|
| 2026-09-21 | `claude-opus-5` | 5.00 | 6.25 | 10.00 | 0.50 | 25.00 | a |
| 2026-09-21 | `claude-sonnet-5` | 2.00 | 2.50 | 4.00 | 0.20 | 10.00 | a |
| 2026-09-21 | `claude-opus-4-8` | 5.00 | 6.25 | 10.00 | 0.50 | 25.00 | a |
| 2026-09-21 | `claude-fable-5` | 10.00 | 12.50 | 20.00 | 1.00 | 50.00 | a |
| 2026-09-21 | `claude-fable-5-1` | 10.00 | 12.50 | 20.00 | 0.25 | 50.00 | a, b |
| 2026-09-21 | `claude-haiku-4-5-20251001` | 1.00 | 1.25 | 2.00 | 0.10 | 5.00 | a |
| 2026-10-04 | `claude-opus-5-5` | 4.00 | 5.00 | 8.00 | 0.20 | 20.00 | d |
| 2026-10-04 | `claude-sonnet-5-5` | 2.00 | 2.50 | 4.00 | 0.20 | 10.00 | d |
| 2026-11-05 | `claude-haiku-5-5` | 0.10 | 0.125 | 0.20 | 0.01 | 0.50 | e |
| 2026-10-04 | `glm-5.3` | 0.1121 | 0.1121 | 0.1121 | 0.1121 | 0.1121 | c |
| 2026-10-04 | `glm-5.3-flash` | 0.1121 | 0.1121 | 0.1121 | 0.1121 | 0.1121 | c |

Source c: not a list price. z.ai bills a flat $80 a month, so the rate is derived. It is $80
divided by the GLM tokens `script/effort` counts in the billing month, with every token class and
both models at one rate, because a flat fee has no class multipliers. The count is 713,543,216
tokens, all in 2026W40: `glm-5.3` 629,380,610 over 3,874 requests and `glm-5.3-flash` 84,162,606
over 632. That gives $0.11212 per million, written as 0.1121.

The window is 2026-09-29 to 2026-10-28: the start is the invoice date, and the one-month length is
calef's belief, not something read off the invoice. All z.ai usage so far (2026-09-29 02:33 to 2026-10-01 05:07) sits inside it. A calendar-month reading gives the same count (all of it is in 2026W40). If more GLM tokens are spent before
the plan lapses, the true rate is lower. The correction is a new dated row, not an edit of this one.
Like the Claude rows, it is a shadow price: here the actual fee spread over actual use.

Source d: the `claude-api` skill bundled with the harness (version 2.1.287), model table cached
2026-09-25. `claude-opus-5-5` is $4 input and $20 output with cache reads at $0.20.
`claude-sonnet-5-5` is $2 and $10 with reads at $0.20. The cache-write columns are source a's
multipliers (1.25x and 2x input) applied to those inputs. This skill version states no write price
for either model, so those four cells are derived, not read. The rows exist because 2026W39 and
2026W40 had tokens on these models with no row, which left both blended rates blank.

Source e: Anthropic's pricing page, platform.claude.com/docs/en/about-claude/pricing, which I read
on 2026-11-05. The page states all five cells, so none needs deriving. It sets the Haiku 5.5 rate
by prompt length, and the row above holds the rate for a request with a prompt up to 100,000
tokens. A longer request pays 0.50 input, 0.625 five-minute write, 1.00 one-hour write, 0.05 read
and 2.50 output. `effort.csv` aggregates by week and keeps no per-request prompt size, so this table
cannot apply the higher tier. The largest prompt on any `claude-haiku-5-5` row so far (the
`peak_context_tokens` column) stays under 66,000 tokens, so the lower tier fits every week on
record. A week with a larger peak on this model would come out underpriced.

Source a: the `claude-api` skill bundled with the agent harness (version 2.1.277), whose model table
carries input and output prices and is itself dated 2026-06-24. Its `shared/prompt-caching.md` gives
the cache multipliers: a write is 1.25x input at the five-minute
time-to-live and 2x at the one-hour one, and a read is 0.1x input. The four cache columns above are
that arithmetic, written out so a reader does not have to redo it and so a future rate change is a
new row rather than a new formula.

Source b: the same file records `claude-fable-5-1` as the exception to the read multiplier, at
0.025x rather than 0.1x. It is 0.3% of this project's tokens, so it changes no total that matters;
it is written down because a rounded-off exception is how a table stops being checkable.

A model with no row here is priced at nothing and counted at nothing. `<synthetic>` is the
harness's own label for a response it generated locally, with zero tokens on it, and it has no rate
because no vendor billed for it. If a model appears in `effort.csv` with no row here,
`script/metrics` leaves that week's blended rate blank rather than pricing part of the week, because
a rate derived from three quarters of a week's tokens is not that week's rate.

## BUGS

- **Nothing checks this file against reality.** There is no receipt, no invoice, no API bill in the
  tree, and no gate that could tell a typo from a purchase. It is exactly as reliable as the person
  who typed it, which is the property every ledger has had and is worth saying once.
- **The rates are list prices, read from a document, on one day.** They are not what anyone was
  charged, they do not include any discount, batch rate or tier, and they will drift. A rate change
  is a new row; until somebody adds one, `script/metrics` prices every week at the newest row it has,
  so a rate that changed silently would restate old weeks at the new price. That is the same
  restatement hazard `notes/project-metrics.md` opens with, and it is worse here because a dollar
  figure reads as a measurement.
- **Hardware is not amortized and not depreciated.** A board bought in 2026W33 lands entirely in
  2026W33. Over a project this short that is the honest shape; over a longer one it would make a
  purchase week look like a spending problem.
- **calef's time is not in this file and never will be.** Milestone 519 refuses time tracking, and
  the standing datum is one calendar week of full-time work per calendar week, carried in
  `cost.csv`'s `human_person_weeks` column. At any plausible rate for an experienced engineer's
  time it is the overwhelming majority of what this project has cost, and a ledger of cash alone
  would invite a reader to forget that.
