<div align="center">
    <img src="../../frontend/public/assets/apple-touch-icon.png" alt="Yin-Panel 標誌" width="128" height="128">

<h1>Yin-Panel</h1>

[![最新版本](https://img.shields.io/github/v/release/yinorg/Yin-Panel?style=flat-square)](https://github.com/yinorg/Yin-Panel/releases)
[![授權條款：BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-brightgreen.svg?style=flat-square)](LICENSE)
![前端檢查](https://github.com/yinorg/Yin-Panel/workflows/Frontend%20Checks/badge.svg)

[English](../../README.md) | [简体中文](../zh-CN/README.md) | **繁體中文** | [日本語](../ja-JP/README.md) | [한국어](../ko-KR/README.md) | [Deutsch](../de-DE/README.md) | [Français](../fr-FR/README.md) | [Español](../es-ES/README.md) | [Português (Brasil)](../pt-BR/README.md) | [Русский](../ru-RU/README.md)

</div>

Yin-Panel 是一個輕量、可自架的個人與共享空間導覽面板，適合家庭網路、NAS、伺服器與小型組織。

## 示範

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../images/readme-home-dark.jpg">
  <img alt="Yin-Panel 首頁：書籤分組、系統狀態與搜尋" src="../images/readme-home-light.jpg">
</picture>

| 項目編輯器 | 空間管理 |
| --- | --- |
| ![使用圖示編輯器新增書籤](../images/readme-editor-light.jpg) | ![空間管理：公開存取與書籤匯入匯出](../images/readme-spaces-light.jpg) |

## 功能

- 個人空間與共享空間，內容彼此隔離
- 空間建立、重新命名、複製與管理員轉讓
- 群組樹、書籤、圖示與排序管理
- 瀏覽器 HTML 格式書籤匯入與匯出
- 空間成員角色：管理員、編輯者、檢視者
- OAuth 2.0 / OIDC 登入、帳號連結與 OIDC 群組授權
- 電子郵件作為登入帳號，暱稱獨立維護
- 自訂圖示、可安裝的主題套件、10 種介面語言、背景、WAN/LAN 與手機網址模式、頁面內彈窗開啟
- 支援 SQLite、MySQL、MariaDB 與 PostgreSQL，提供 amd64 與 arm64 Docker 映像與 Kubernetes Helm Chart

## 快速開始

### Docker Compose

需要 Docker 及 Compose 外掛。

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

開啟 <http://localhost:3002>。

預設管理員：`admin@yiniot.com`，密碼 `admin@yiniot.com`。首次登入後請立即修改密碼，並在 `conf.yaml` 中設定 `base.root_url` 與唯一的 `jwt.secret`。

每個部署執行個體都應產生獨立的隨機 `jwt.secret`。儲存庫只提供 `conf.yaml.example`；執行目錄中的 `conf.yaml` 包含執行個體密鑰與 OAuth 憑證，請勿提交到 Git。

### GHCR 映像

```bash
docker pull ghcr.io/yinorg/yin-panel-ce:0.6.0
docker pull ghcr.io/yinorg/yin-panel-ce:latest
```

`0.6.0` 與 `latest` 都是面向 amd64 與 arm64 主機的 multi-arch 資訊清單。發佈 tag 不帶架構後綴；`-amd64`、`-arm64` 之類的 tag 不屬於正式發佈。

## 效能與建議組態

以下數據來自本機單機基準（SQLite、空資料庫、loopback、同機壓測），用於選型參考，不代表生產容量承諾。實際容量主要取決於每個空間的資料量、寫入頻率與峰值併發。

### 資源佔用（實測數量級）

| 指標 | 閒置 | 5000 req/s 混合負載 | 極限（數萬 req/s） |
| --- | --- | --- | --- |
| 記憶體 RSS | ~16 MB | ~105 MB | 峰值 ~500 MB，負載停止後回落 |
| CPU | 接近 0 | ~0.3 核 | 2–3 核 |

- 記憶體到約 **2 萬 req/s** 前基本恆定在 **105 MB**，不隨負載線性成長；那 ~500 MB 只在合成極壓時出現。
- 單核約可承載 **1 萬–1.2 萬 req/s**（混合讀寫、小資料量）。
- 5000 req/s 混合負載下：0 失敗、p95 約 **0.25 ms**。

### 建議組態

| 情境 | CPU | 記憶體 | 儲存 | 資料庫 |
| --- | --- | --- | --- | --- |
| 個人 / 家庭 / NAS | 1 vCPU | 128–256 MB | SSD，512 MB 起（另加圖示上傳餘量） | SQLite |
| 小型團隊 / 共享空間 | 1–2 vCPU | 256–512 MB | SSD，1–2 GB | SQLite |
| 組織級 / 高併發 | 2–4 vCPU | 1–2 GB | SSD | SQLite（單機）或 MySQL（多執行個體、HA） |

### 使用者規模參考

按「每位線上使用者約 0.1 req/s（掛機為主）」估算，並預留峰值餘量：

| 組態 | 理論平均輸送量 | 掛機併發（理論上限） | 建議規模（含峰值餘量） |
| --- | --- | --- | --- |
| 1 vCPU | ~10k req/s | ~10 萬 | 2 萬–4 萬 |
| 2 vCPU | ~20k req/s | ~20 萬 | 5 萬–8 萬 |
| 4 vCPU | ~40k req/s | ~40 萬 | 10 萬以上 |

使用者越活躍（編輯、匯入、頻繁重新整理），每人產生的請求率越高，可支撐的使用者數相應下降。

### 影響容量的關鍵因素

1. **資料量**：清單介面在單次回應中傳回整個空間的全部項目，項目越多，單請求成本越高；這是最需要用自身資料實測的部分。
2. **寫入頻率**：SQLite 同一時刻只有一個寫入者，寫入輸送量受磁碟 fsync 限制（數百次寫入/秒量級）。寫入密集或多執行個體情境建議改用 MySQL。
3. **峰值而非平均**：按峰值選型，通常預留均值 2–5 倍的餘量。
4. **靜態資源**：建議由反向代理或 CDN 承載 `/assets/*` 與帶 hash 的資源，見「Cloudflare 快取」。

## 資料庫

支援 SQLite（預設）、MySQL、MariaDB 與 PostgreSQL。

- `base.database_drive`：`sqlite`（預設）| `mysql` | `postgres`。**MariaDB 使用 `mysql`**。
- SQLite：`sqlite.file_path`（另見 `read_pool_size`）。
- MySQL / MariaDB：`mysql.{host,port,username,password,db_name,wait_timeout}`。
- PostgreSQL：`postgres.{host,port,username,password,db_name,ssl_mode,time_zone,wait_timeout}`。

SQLite 專屬能力（WAL、唯讀連線池、以檔案快照為基礎的遷移備份）僅適用於 SQLite；MySQL/MariaDB/PostgreSQL 請使用各自的工具備份。

### 從 SQLite 遷移至 MySQL/MariaDB/PostgreSQL

```bash
# conf.yaml        —— 現有 SQLite 設定
# conf.target.yaml —— 指向 mysql 或 postgres 的目標設定（目標資料庫須為空）
./yin-panel migrate-db -source conf.yaml -target conf.target.yaml
```

遷移會在目標資料庫建立完整 schema、逐表複製所有資料並**保留主鍵**（PostgreSQL 會同步重設自增序列），可重複執行（每次取代目標表內容）。完成後將 `conf.yaml` 的 `database_drive` 指向目標資料庫即可。

## 設定與 OAuth/OIDC

`oauth` 是 OAuth 2.0 與 OpenID Connect（OIDC）的統一設定入口。預設範本提供三個已驗證的範例：GitHub（OAuth 2.0）、GitLab（標準 OIDC）與 Google（OAuth 2.0）。範例預設全部註解，填寫實際憑證並將 `oauth.enable` 設為 `true` 後才會啟用。全部設定項見 `backend/conf.yaml.example`。

標準 OIDC provider 需要支援 discovery、authorization code + PKCE 與 JWKS 簽章驗證，並在使用者資訊中提供 `email` 與 `email_verified`。符合這些標準的 provider 理論上可用，但不同服務商可能仍需要額外的欄位對應設定。

OIDC 回呼位址：

```text
<base.root_url>/api/oauth/<provider>/callback
```

回呼完成後，Provider 必須傳回已驗證電子郵件。系統使用 `provider + sub` 識別外部身分，並使用電子郵件關聯本機帳號。

## 空間與權限

空間權限使用內部 `UserID` 關聯，不依賴電子郵件作為權限主鍵。因此修改電子郵件後，既有空間權限仍然有效，成員清單會顯示最新電子郵件。

- 管理員：管理空間、成員、角色與 OIDC 規則
- 編輯者：維護群組與書籤
- 檢視者：唯讀存取

建立空間、重新命名、新增群組、新增成員與新增 OIDC 規則均在彈窗中完成。

## Helm

```bash
helm upgrade --install yin-panel ./distribution/helm-chart/yin-panel \
  --set image.tag=0.6.0
```

部署前可用 `helm template yin-panel ./distribution/helm-chart/yin-panel` 檢查渲染結果。

## 原始碼開發

環境需求：Go `1.27.1`、Node.js `24.21.0`、npm 或 pnpm `9.15.5`。

```bash
cd frontend
npm ci
npm run build-only
```

也可以使用鎖定版本的 pnpm：

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

本機完整更新（前端建置、後端測試與編譯、部署、重新啟動）統一使用：

```bash
./scripts/deploy-local.sh
```

腳本預設將儲存庫內的 `backend/` 作為執行目錄，要求其中存在本機 `conf.yaml`；可透過 `.env.local` 中的 `YIN_PANEL_RUNTIME_DIR` 覆寫。腳本會在建置與測試全部通過後，僅停止 Yin-Panel 監聽程序，使用 `sudo` 替換舊二進位檔與靜態檔案，不會修改資料庫、上傳檔案或設定檔。

首頁主題系統見 [`frontend/THEME_SYSTEM.md`](../../frontend/THEME_SYSTEM.md)；使用 `npm run create:theme -- init "<名稱>"` 可建立新主題骨架。完整的 AI 開發、發佈與重新啟動慣例見 [AGENTS.md](../../AGENTS.md)。

## 備份與升級

升級前請備份資料庫、`uploads/`、`conf.yaml` 與 Helm values。遷移會保留使用者、空間、群組、書籤與檔案參照。升級異常時使用升級前備份還原，不要直接刪除資料庫重建。

## Cloudflare 快取

如果網域啟用了 Cloudflare 橘色小雲，建議使用 Cache Rules 保持入口檔案即時更新、只長期快取帶 hash 的靜態資源。

繞過快取：`/`、`/index.html`、`/login`、`/sw.js`、`/registerSW.js`、`/manifest.webmanifest` 與 `/api/*`。

長期快取：`/assets/*` 與 `/workbox-*.js`。這些檔案帶內容 hash 檔名，原始伺服器會以 30 天 `immutable` 回應。

不要對整個網站啟用 `Cache Everything`。發佈後只清理入口檔案與 Service Worker 的 Cloudflare 快取，帶 hash 的資源可以繼續重用邊緣快取。

## Open Core

Yin-Panel Core 版本免費使用，並會持續維護與更新。本儲存庫是公開 Core 的唯一來源，發佈 `yin-panel-ce` 映像。企業版位於私有儲存庫 [yinorg/Yin-Panel-EE](https://github.com/yinorg/Yin-Panel-EE)，透過公開介面接入企業模組。

執行中的執行個體可透過 `GET /api/system/capabilities` 查詢發行版、能力、唯讀狀態與版本。問題與建議請提交至 [GitHub Issues](https://github.com/yinorg/Yin-Panel/issues)。

## 專案沿革與致謝

Yin-Panel 基於開源專案 [hslr-s/sun-panel](https://github.com/hslr-s/sun-panel) 發展，感謝原作者建立專案基礎。

同時感謝 [PhantomMaa/sun-panel](https://github.com/PhantomMaa/sun-panel) 對專案進行的最佳化與維護工作。Yin-Panel 在此基礎上接任後續維護，並持續加入新功能、互動體驗與適合自架情境的改進。

感謝所有原作者、貢獻者與使用者。

## 授權條款

Yin-Panel 採用 [Business Source License 1.1](../../LICENSE) 授權：個人使用或組織內部營運可免費用於正式環境。每個版本首次公開發佈三年後，該版本將轉為 AGPL-3.0-or-later 授權。
