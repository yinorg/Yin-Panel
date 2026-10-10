<div align="center">
    <img src="frontend/public/assets/apple-touch-icon.png" alt="Yin-Panel ロゴ" width="128" height="128">

<h1>Yin-Panel</h1>

[![最新リリース](https://img.shields.io/github/v/release/yinorg/Yin-Panel?style=flat-square)](https://github.com/yinorg/Yin-Panel/releases)
[![ライセンス：BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-brightgreen.svg?style=flat-square)](LICENSE)
![フロントエンドチェック](https://github.com/yinorg/Yin-Panel/workflows/Frontend%20Checks/badge.svg)

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | **日本語** | [한국어](README.ko-KR.md) | [Deutsch](README.de-DE.md) | [Français](README.fr-FR.md) | [Español](README.es-ES.md) | [Português (Brasil)](README.pt-BR.md) | [Русский](README.ru-RU.md)

</div>

Yin-Panel は、個人用と共有のスペースのための軽量でセルフホスト可能なナビゲーションパネルです。家庭内ネットワーク、NAS、サーバー、小規模組織に適しています。

## デモ

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="doc/images/readme-home-dark.jpg">
  <img alt="Yin-Panel のホーム画面：ブックマークのグループ、システム状態、検索" src="doc/images/readme-home-light.jpg">
</picture>

| 項目エディター | スペース管理 |
| --- | --- |
| ![アイコンエディターでブックマークを追加](doc/images/readme-editor-light.jpg) | ![公開アクセスとブックマークのインポート/エクスポートを備えたスペース管理](doc/images/readme-spaces-light.jpg) |

## 機能

- 個人スペースと共有スペース（コンテンツは互いに分離）
- スペースの作成、名前変更、複製、管理者譲渡
- グループツリー、ブックマーク、アイコン、並べ替え
- ブラウザーの HTML 形式によるブックマークのインポートとエクスポート
- スペースメンバーの役割：管理者、編集者、閲覧者
- OAuth 2.0 / OIDC ログイン、アカウント連携、OIDC グループ認可
- ログイン ID はメールアドレス、ニックネームは独立して管理
- カスタムアイコン、インストール可能なテーマパッケージ、10 種類の UI 言語、壁紙、WAN/LAN・モバイル URL モード、ページ内ポップアップ表示
- SQLite、MySQL、MariaDB、PostgreSQL に対応し、amd64 / arm64 の Docker イメージと Kubernetes 用 Helm チャートを提供

## クイックスタート

### Docker Compose

Docker と Compose プラグインが必要です。

```bash
mkdir yin-panel && cd yin-panel
curl -LO https://raw.githubusercontent.com/yinorg/Yin-Panel/master/docker/docker-compose.yml
curl -o conf.yaml https://raw.githubusercontent.com/yinorg/Yin-Panel/master/backend/conf.yaml.example
mkdir -p database uploads
jwt_secret="$(openssl rand -hex 32)"
sed -i "s#^  secret: your_secret_key$#  secret: ${jwt_secret}#" conf.yaml
unset jwt_secret
YIN_PANEL_IMAGE=ghcr.io/yinorg/yin-panel-ce:0.6.0 docker compose up -d
```

<http://localhost:3002> を開きます。

既定の管理者：`admin@yiniot.com`、パスワード `admin@yiniot.com`。初回ログイン後にすぐパスワードを変更し、`conf.yaml` で `base.root_url` と一意の `jwt.secret` を設定してください。

各デプロイ環境はそれぞれ独立したランダムな `jwt.secret` を生成してください。リポジトリが提供するのは `conf.yaml.example` のみです。実行ディレクトリの `conf.yaml` にはインスタンスのシークレットと OAuth 認証情報が含まれるため、Git にコミットしないでください。

### GHCR イメージ

```bash
docker pull ghcr.io/yinorg/yin-panel-ce:0.6.0
docker pull ghcr.io/yinorg/yin-panel-ce:latest
```

`0.6.0` と `latest` はどちらも amd64 / arm64 ホスト向けのマルチアーキテクチャマニフェストです。リリースタグにアーキテクチャの接尾辞は付きません。`-amd64` や `-arm64` のようなタグは正式リリースには含まれません。

## パフォーマンスと推奨構成

以下の数値はローカル単体ベンチマーク（SQLite、空のデータベース、loopback、同一ホストでの負荷テスト）によるもので、構成選定の参考値です。本番の容量保証ではありません。実際の容量は主にスペースごとのデータ量、書き込み頻度、ピーク同時接続数に依存します。

### リソース使用量（実測オーダー）

| 指標 | アイドル | 5000 req/s 混合負荷 | 限界（数万 req/s） |
| --- | --- | --- | --- |
| メモリ RSS | ~16 MB | ~105 MB | ピーク ~500 MB、負荷停止後に低下 |
| CPU | ほぼ 0 | ~0.3 コア | 2–3 コア |

- メモリは約 **2 万 req/s** 付近までは **105 MB** 程度でほぼ一定となり、負荷に比例して増加しません。~500 MB は合成の極限負荷時のみ発生します。
- シングルコアで約 **1 万–1.2 万 req/s**（混合読み書き、小規模データ）を処理できます。
- 5000 req/s の混合負荷時：失敗 0、p95 約 **0.25 ms**。

### 推奨構成

| シナリオ | CPU | メモリ | ストレージ | データベース |
| --- | --- | --- | --- | --- |
| 個人 / 家庭 / NAS | 1 vCPU | 128–256 MB | SSD、512 MB 以上（アイコンアップロード用の余裕を含む） | SQLite |
| 小規模チーム / 共有スペース | 1–2 vCPU | 256–512 MB | SSD、1–2 GB | SQLite |
| 組織 / 高負荷 | 2–4 vCPU | 1–2 GB | SSD | SQLite（単一ホスト）または MySQL（マルチインスタンス、HA） |

### ユーザー規模の目安

オンラインユーザー 1 人あたり約 0.1 req/s（アイドル中心）と見積もり、ピーク余裕を確保した場合：

| 構成 | 理論上の平均スループット | アイドル同時接続（理論上限） | 推奨規模（ピーク余裕込み） |
| --- | --- | --- | --- |
| 1 vCPU | ~10k req/s | ~10 万 | 2 万–4 万 |
| 2 vCPU | ~20k req/s | ~20 万 | 5 万–8 万 |
| 4 vCPU | ~40k req/s | ~40 万 | 10 万以上 |

ユーザーが活発なほど（編集、インポート、頻繁な更新）、1 人あたりのリクエスト率が上がり、支えられるユーザー数は相応に減少します。

### 容量に影響する主な要因

1. **データ量**：一覧 API は 1 回の応答でスペース内の全項目を返すため、項目が多いほど 1 リクエストのコストが増えます。自分のデータで実測する価値が最も高い部分です。
2. **書き込み頻度**：SQLite は同時に 1 つの書き込み者しか持てず、書き込みスループットはディスク fsync に制約されます（毎秒数百回程度）。書き込みが多い場合やマルチインスタンス構成では MySQL を推奨します。
3. **平均ではなくピーク**：ピークを基準に選定し、通常は平均の 2–5 倍の余裕を見込みます。
4. **静的ファイル**：`/assets/*` とハッシュ付きアセットはリバースプロキシまたは CDN からの配信を推奨します。「Cloudflare キャッシュ」を参照してください。

## データベース

SQLite（デフォルト）、MySQL、MariaDB、PostgreSQL に対応しています。

- `base.database_drive`：`sqlite`（デフォルト）| `mysql` | `postgres`。**MariaDB は `mysql` を使用します。**
- SQLite：`sqlite.file_path`（`read_pool_size` も参照）。
- MySQL / MariaDB：`mysql.{host,port,username,password,db_name,wait_timeout}`。
- PostgreSQL：`postgres.{host,port,username,password,db_name,ssl_mode,time_zone,wait_timeout}`。

SQLite 専用の機能（WAL、読み取り専用コネクションプール、ファイルスナップショットによる移行バックアップ）は SQLite にのみ適用されます。MySQL/MariaDB/PostgreSQL は各ツールでバックアップしてください。

### SQLite から MySQL/MariaDB/PostgreSQL への移行

```bash
# conf.yaml        — 既存の SQLite 設定
# conf.target.yaml — mysql または postgres を指す移行先設定（移行先データベースは空である必要があります）
./yin-panel migrate-db -source conf.yaml -target conf.target.yaml
```

移行は移行先に完全なスキーマを作成し、全データを主キーを**保持したまま**コピーします（PostgreSQL では ID シーケンスもリセットされます）。繰り返し実行でき、実行のたびに移行先テーブルの内容が置き換わります。完了後、`conf.yaml` の `database_drive` を移行先に変更してください。

## 設定と OAuth/OIDC

`oauth` は OAuth 2.0 と OpenID Connect（OIDC）の共通設定エントリーです。既定のテンプレートには検証済みの 3 つの例（GitHub（OAuth 2.0）、GitLab（標準 OIDC）、Google（OAuth 2.0））があります。例は既定ですべてコメントアウトされています。実際の認証情報を入力し `oauth.enable` を `true` に設定した場合のみ有効になります。すべての設定項目は `backend/conf.yaml.example` を参照してください。

標準的な OIDC プロバイダーは discovery、authorization code + PKCE、JWKS 署名検証をサポートし、ユーザー情報で `email` と `email_verified` を返す必要があります。これらの基準を満たすプロバイダーは原理的に利用できますが、サービスによっては追加のフィールドマッピング設定が必要になる場合があります。

OIDC コールバック URL：

```text
<base.root_url>/api/oauth/<provider>/callback
```

コールバック後、プロバイダーは検証済みメールアドレスを返す必要があります。システムは `provider + sub` で外部 ID を識別し、メールアドレスでローカルアカウントを関連付けます。

## スペースと権限

スペースの権限は内部の `UserID` で関連付け、メールアドレスを権限の主キーにはしません。そのためメールアドレスを変更しても既存のスペース権限は有効なまま、メンバー一覧には最新のメールアドレスが表示されます。

- 管理者：スペース、メンバー、役割、OIDC ルールを管理
- 編集者：グループとブックマークを管理
- 閲覧者：読み取り専用

スペースの作成、名前変更、グループ追加、メンバー追加、OIDC ルール追加はすべてダイアログで行います。

## Helm

```bash
helm upgrade --install yin-panel ./distribution/helm-chart/yin-panel \
  --set image.tag=0.6.0
```

デプロイ前に `helm template yin-panel ./distribution/helm-chart/yin-panel` でレンダリング結果を確認できます。

## ソースからの開発

環境要件：Go `1.27.1`、Node.js `24.21.0`、npm または pnpm `9.15.5`。

```bash
cd frontend
npm ci
npm run build-only
```

固定バージョンの pnpm を使う場合：

```bash
cd frontend
corepack pnpm install --frozen-lockfile
pnpm run build-only
```

```bash
cd backend
go test ./...
go build ./...
```

ローカルの一括更新（フロントエンドのビルド、バックエンドのテストとビルド、デプロイ、再起動）は次の 1 コマンドで行います：

```bash
./scripts/deploy-local.sh
```

スクリプトは既定でリポジトリ内の `backend/` を実行ディレクトリとし、そこにローカルの `conf.yaml` が必要です。`.env.local` の `YIN_PANEL_RUNTIME_DIR` で上書きできます。ビルドとテストがすべて成功した後、Yin-Panel のリスナーのみを停止し、`sudo` で旧バイナリと静的ファイルを置き換えます。データベース、アップロードファイル、設定ファイルには触れません。

ホームページのテーマシステムは [`frontend/THEME_SYSTEM.md`](frontend/THEME_SYSTEM.md) を参照してください。`npm run create:theme -- init "<名前>"` で新しいテーマの雛形を作成できます。AI 開発・リリース・再起動の規約は [AGENTS.md](./AGENTS.md) にあります。

## バックアップとアップグレード

アップグレード前にデータベース、`uploads/`、`conf.yaml`、Helm values をバックアップしてください。マイグレーションはユーザー、スペース、グループ、ブックマーク、ファイル参照を保持します。問題が発生した場合はアップグレード前のバックアップから復元し、データベースを削除して作り直さないでください。

## Cloudflare キャッシュ

ドメインで Cloudflare のオレンジ色の雲を有効にしている場合は、Cache Rules で入口ファイルを常に最新に保ち、ハッシュ付き静的アセットのみを長期キャッシュすることを推奨します。

キャッシュをバイパス：`/`、`/index.html`、`/login`、`/sw.js`、`/registerSW.js`、`/manifest.webmanifest`、`/api/*`。

長期キャッシュ：`/assets/*` と `/workbox-*.js`。これらのファイルは内容ハッシュ付きの名前を持ち、オリジンは 30 日間の `immutable` として返します。

サイト全体に `Cache Everything` を有効にしないでください。リリース後は入口ファイルと Service Worker の Cloudflare キャッシュのみをパージし、ハッシュ付きアセットはエッジキャッシュに残して構いません。

## Open Core

Yin-Panel Core エディションは無償で、継続的に保守・更新されます。このリポジトリは公開 Core の唯一のソースであり、`yin-panel-ce` イメージを公開しています。エンタープライズ版は非公開リポジトリ [yinorg/Yin-Panel-EE](https://github.com/yinorg/Yin-Panel-EE) にあり、公開インターフェース経由でエンタープライズモジュールを組み込みます。

実行中のインスタンスは `GET /api/system/capabilities` でエディション、機能、読み取り専用状態、バージョンを確認できます。問題や提案は [GitHub Issues](https://github.com/yinorg/Yin-Panel/issues) へお寄せください。

## クレジット

Yin-Panel はオープンソースプロジェクト [hslr-s/sun-panel](https://github.com/hslr-s/sun-panel) から発展しました。プロジェクトの基盤を築いた原作者に感謝します。

[PhantomMaa/sun-panel](https://github.com/PhantomMaa/sun-panel) の最適化と保守作業にも感謝します。Yin-Panel はその基盤を引き継いで継続的な保守を行い、新機能、操作性の向上、セルフホストに適した改善を続けています。

すべての原作者、コントリビューター、ユーザーに感謝します。

## ライセンス

Yin-Panel は [Business Source License 1.1](./LICENSE) でライセンスされています。個人利用または組織の内部業務での本番利用は無償です。各バージョンは初回公開から 3 年後、AGPL-3.0-or-later ライセンスに移行します。
