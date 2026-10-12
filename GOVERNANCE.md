# Core and Enterprise

Yin-Panel is developed as an open core project: a freely available Core, and a
commercial Enterprise edition. This document states where the line between the
two is drawn, and why, so that using or contributing to the project does not
require guessing.

## About this document

**Only the [LICENSE](./LICENSE) determines the scope of the software license.**
This document explains how the project is run and what the project intends. It
does not grant rights, and it must not be read as expanding or narrowing what
the LICENSE grants. It is, however, a public statement of the project's policy
and intent, and it can be read as evidence of what the project promised. That
is the point: the intentions below are meant to be checkable, not implied.

Where this document and the LICENSE appear to differ, the LICENSE governs.

## Why it is set up this way

Yin-Panel is maintained by people who need to be paid for that work. That is the
whole argument, and every choice below follows from it.

A navigation panel has no moat. There is no proprietary algorithm, no data
network effect and no patent — the code is the asset. If the code can be taken
and operated commercially with nothing flowing back, then maintaining it has no
funding source, and the project ends when the maintainer stops. So the question
was never "how open can we be?" but "which rights can we give away without
losing the ability to keep working on this?"

That produced these choices:

- **Not a permissive license.** MIT or Apache-2.0 is what this project would
  use if funding were not a constraint. They also allow anyone to operate
  Yin-Panel as a commercial hosted service and return nothing.
- **Not AGPL alone.** AGPL stops a closed fork, which is worth something, but it
  funds nothing: a provider can run AGPL code commercially as long as it
  publishes its changes. It also triggers blanket corporate bans that shrink the
  audience.
- **Not closed source.** The project's value comes from being deployed in other
  people's networks — themes, translations, icon sets, bug reports from hardware
  we do not own. The code is not the secret, so secrecy buys nothing.
- **Not an open core with a paywalled product.** If the Enterprise edition held
  features people reasonably expect from a navigation panel, the project would
  get the worst of both: the community cost of open core, and the reputation of
  having moved a product behind a wall. The rule below exists to make that
  structurally impossible rather than merely unlikely.

What was chosen: **BUSL-1.1**, with an Additional Use Grant that keeps the
source public, allows modification and forking, and makes production use free
for everything except selling access to Yin-Panel itself as a service. Each
version becomes available under AGPL-3.0-or-later three years after its own
first public distribution, automatically, whatever the maintainer wants.

**Yin-Panel is not open source in the OSI sense, and this project does not
describe it that way.** It is source-available, with a delayed transition to
AGPL-3.0-or-later. "Open source" has a definition, this license does not meet
it, and using the label anyway is what turns a licensing decision into a
credibility problem.

## Where the line is

**A feature belongs in the Enterprise edition only if it is operational,
compliance, or support work.**

Anything a user would reasonably expect from a self-hosted navigation panel
belongs in the Core: layouts, spaces, members and roles, OIDC login, themes,
icons, import and export, search and the monitor surfaces.

Two rules make that concrete:

1. **The Core is complete.** The Core must remain a product that can be
   deployed, maintained, backed up, migrated and upgraded on its own, without
   depending on a commercial service or on the Enterprise edition.
2. **The Core is not crippled.** Core functionality will not be reduced,
   degraded or held back in order to create demand for the Enterprise edition.
   The Enterprise edition adds capability; it does not unlock capability that
   the Core is capable of but withheld.

The test for a new feature is not *"could we charge for this?"* but *"would a
contributor be surprised and annoyed to find this behind a paywall?"*. If the
answer is yes, it belongs in the Core.

## What the Core contains

This repository. It builds the `yin-panel-ce` image and is the only public
source for the Core. The Core is free to run in production under the Additional
Use Grant in the [LICENSE](./LICENSE).

## What the Enterprise edition contains

The Enterprise edition is built from the same Core source with additional
modules registered at startup, so it **bundles** the Core rather than replacing
it. Its current scope is remote and object storage backends — S3 and compatible
services, through rclone. The Core rejects any non-local storage type at startup
with a message pointing at the Enterprise image.

That is the whole of it today. Anything else described as an Enterprise
direction — centralised administration, enterprise integrations, compliance
tooling, automation — is a **direction, not a commitment**, and is not available
today.

A running instance reports its edition, capabilities, read-only state, and
version at `GET /api/system/capabilities`.

## What stays in the Core

Contributions merged into this repository stay in the Core:

- They will not be moved behind the Enterprise edition.
- They will not be made available only under terms other than the Core license.

This is not only a statement in a policy document. It is written into the
[Contributor License Agreement](./CLA.md), sections 3 and 4, so it is part of
the license you contribute under. The Core license itself can still change over
time — the Business Source License 1.1 becomes AGPL-3.0-or-later for each
version after its Change Date — but the code stays in the Core.

Contributions can therefore appear inside an Enterprise build, because that
build contains the Core. They cannot become Enterprise-only.

## Commercial boundaries

These are six separate things. Holding one does not imply any of the others.

| | What it is | How it is obtained |
|---|---|---|
| **Software license** | The right to use the Core | Free under the Additional Use Grant in the [LICENSE](./LICENSE). For uses outside that grant, a commercial license from the maintainer |
| **Official hosting** | A Yin-Panel instance operated by the maintainer for you | Separate paid arrangement |
| **Commercial support** | Paid help, review, or engineering time | Separate agreement |
| **SLA** | A contractual availability or response commitment | Only where a signed agreement says so. Nothing in this repository creates one |
| **Trademark** | The Yin-Panel name and logo | **Not granted by the software license.** The license grants no right in any trademark. Using the name for a fork requires permission |
| **OEM / white-label rights** | Redistributing Yin-Panel as part of your own branded product, or under your own terms | Separate written agreement with the maintainer |

Paying for one of these does not grant any of the others, and none of them is
required to use the Core under the Additional Use Grant.

## Common questions

**Can I fork Yin-Panel?**

Yes. You may copy, modify, and redistribute it, and you may run your fork in
production for the uses listed in the Additional Use Grant:

- your own personal use;
- the internal business operations of your organization, including entities
  under common control with it;
- running it for a client or another organization, where that organization is
  the only user of that instance, for that organization's own internal business
  operations — whether you charge for those services or not;
- making an instance available to your own customers, members, or users in order
  to deliver your own content, products, or services to them.

**When do I need a commercial license?**

When you provide third parties with access to an instance of Yin-Panel that you
run, as the service you sell them — hosted, managed, white-label, or
multi-tenant offerings. That is the only thing a commercial license is needed
for.

**Can I use it at work?**

Yes. Internal business use by your organization is free, including by entities
under common control with it, and including running it to serve your own
customers.

**Can I be paid to deploy or maintain it for a client?**

Yes, including where you operate the instance on the client's behalf, as long as
that client is the only user of that instance. You may charge for that work.

**Can I build a product on top of it?**

You may build on the Core, and you must keep the rights you received — the Core
stays under its license, and your modifications to it stay under that license
too. Distributing Yin-Panel as part of a product under your own brand and terms
is an OEM arrangement and needs a separate agreement.

**Can I remove the license notice from my fork?**

No. The license must be displayed conspicuously on each original or modified
copy, and the third-party notices in
[THIRD-PARTY-NOTICES.md](./THIRD-PARTY-NOTICES.md) must be preserved.

## On "production use"

The LICENSE distinguishes production from non-production use but does not define
either term. This document does not define them either, because it cannot: only
the LICENSE grants rights, and reading a definition here into the LICENSE would
be exactly the kind of expansion this document warns against.

The ordinary meaning applies. Developing, evaluating, testing, and staging are
not production use. Running Yin-Panel for real work, with real data, for real
users is production use, and the Additional Use Grant is what makes that free
for the cases listed above.

## Enforcement

The intent is not to catch people out. Where a use is ambiguous, or appears to
fall outside the Additional Use Grant but was not obviously meant to, the
maintainer will raise it directly and offer a commercial license before
asserting any termination of rights. A good-faith reading of this document will
not be treated as a violation.

## Changing this line

This document can be updated as the product grows. When it is, the change is
made in this repository so that it is public and reviewable, and it never
applies retroactively to already-merged contributions.
