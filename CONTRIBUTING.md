# Contributing to Yin-Panel

Thanks for your interest in Yin-Panel. This document explains how to
contribute and the terms your contribution is accepted under.

## Before you start

- Search [open issues](https://github.com/yinorg/Yin-Panel/issues) and pull
  requests before opening a new one.
- For anything larger than a bug fix, open an issue first so the design can be
  agreed before you write code.
- Report security problems privately rather than in a public issue.

## Development

See the [Source development](README.md#source-development) section of the
README for building the frontend and backend, running the tests, and the local
development workflow. Backend tests belong under `backend/`, frontend tests
under `frontend/`, and real-browser acceptance tests belong in the separate E2E
project.

## Contributor License Agreement

**Every contribution requires a Contributor License Agreement (CLA).** By
submitting a pull request, a patch, or any other contribution to this project,
you agree to the [Yin-Panel Contributor License Agreement](./CLA.md).

The CLA does two things. It lets the project distribute your contribution as
part of the Core, under the Core license, and as part of the commercial
Enterprise build, which bundles the Core. In exchange, the project commits in
writing that **your contribution stays in the Core**: it will not be moved
behind the Enterprise edition, and it will not be relicensed under other terms.
You keep the copyright to your contribution, and the right to use it however you
like. See [Core and Enterprise](./GOVERNANCE.md).

A plain `Signed-off-by` trailer (the Developer Certificate of Origin) is **not**
sufficient on its own, because it does not grant the project the right to
distribute your contribution as part of the Enterprise build. If you are
contributing on behalf of an employer, see the corporate section of the CLA
before you submit.

You do not need to sign anything by hand: agreement is recorded when you submit
a contribution after being asked to accept the CLA. If a maintainer asks you to
confirm, reply on the pull request.

## Pull requests

- Keep each pull request focused on one change.
- Match the surrounding code style and keep unrelated changes out of the patch.
- Update the user-facing translations in `frontend/src/locales/` together with
  the code that adds or changes strings.
- Add or update tests for behavior you change.
- Describe what changed and why, and note anything you could not verify.

All contributed code must be compatible with the project's license. Do not
submit code copied from another project unless its license permits it and its
notice is added to [THIRD-PARTY-NOTICES.md](./THIRD-PARTY-NOTICES.md).

## 中文说明

所有贡献都需要同意[贡献者许可协议（CLA）](./CLA.md)。提交 Pull Request 即表示你
接受该协议。CLA 一边授权项目把你的贡献作为 Core（按 Core 许可证）以及企业版构建的
一部分分发，一边**由项目承诺：你的贡献永远留在 Core** —— 不会被挪进企业版，也不会
被换成其他条款再授权。你保留自己贡献的著作权。详见
[Core 与企业版](./GOVERNANCE.md)。仅有 `Signed-off-by`（DCO）签名不足以覆盖企业版
分发的授权，因此本仓库要求 CLA。协议的英文文本为权威版本。

Do not submit code you cannot license under these terms.
