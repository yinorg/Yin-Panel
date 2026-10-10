## Yin-Panel

Yin-Panel 是一个轻量、开源、可自托管的个人与共享空间导航面板，适合家庭网络、NAS、服务器和小型组织。

当前版本：`0.3.15`

## 功能

- 个人空间与共享空间，内容彼此隔离
- 空间创建、重命名、复制和管理员转让
- 分组树、书签、图标和排序管理
- Chrome 书签导入和导出
- 空间成员管理：管理员、编辑者、查看者
- OIDC/OAuth 登录、账号关联和 OIDC 分组授权
- 邮箱作为登录账号，昵称独立维护
- 自定义图标、主题、语言、背景、网络模式和网页小窗
- 支持 SQLite、MySQL、MariaDB、PostgreSQL、Docker、ARM64 和 Kubernetes

## 快速开始

### Docker Compose

```bash
mkdir yin-panel && cd yin-panel
curl -LO https://raw.githubusercontent.com/yinorg/Yin-Panel/master/docker/docker-compose.yml
curl -o conf.yaml https://raw.githubusercontent.com/yinorg/Yin-Panel/master/backend/conf.yaml.example
mkdir -p database uploads
jwt_secret="$(openssl rand -hex 32)"
sed -i "s#^  secret: your_secret_key$#  secret: ${jwt_secret}#" conf.yaml
unset jwt_secret
docker compose up -d
```

每个部署实例都应使用独立的随机 `jwt.secret`。仓库只提供 `conf.yaml.example`；运行目录中的 `conf.yaml` 包含实例密钥和 OAuth 凭据，请勿提交到 Git。

打开 <http://localhost:3002>。

默认管理员：`admin@yiniot.com`，默认密码：`admin@yiniot.com`。首次登录后请立即修改密码，并修改 `conf.yaml` 中的 `base.root_url` 和 `jwt.secret`。

指定版本：

```bash
YIN_PANEL_IMAGE=ghcr.io/yinorg/yin-panel-ce:0.3.15 docker compose up -d
```

### GHCR 镜像

```bash
docker pull ghcr.io/yinorg/yin-panel-ce:0.3.15
docker pull ghcr.io/yinorg/yin-panel-ce:latest
```

`0.3.15` 和 `latest` 都是 multi-arch 镜像，可直接在 amd64 或 arm64 主机使用。`-amd64`、`-arm64` 仅作为构建过程中的临时 tag，不属于最终发布 tag。

## 性能与推荐配置

以下数据来自本地单机基准（SQLite、空库、loopback、同机压测），用于选型参考，不代表生产容量承诺。实际容量主要取决于**每个空间的数据量**、**写入频率**和**峰值并发**。

### 资源占用（实测量级）

| 指标 | 空闲 | 5000 req/s 混合负载 | 极限（数万 req/s） |
| --- | --- | --- | --- |
| 内存 RSS | ~16 MB | ~105 MB | 峰值 ~500 MB，负载停止后回落 |
| CPU | 接近 0 | ~0.3 核 | 2–3 核 |

- 内存到约 **2 万 req/s** 前基本恒定在 ~105 MB，不随负载线性增长；那 ~500 MB 只在合成极压时出现。
- 单核约可承载 **1 万–1.2 万 req/s**（混合读写、小数据量）。
- 5000 req/s 混合负载下：0 失败、p95 约 0.25 ms。

### 推荐配置

| 场景 | CPU | 内存 | 存储 | 数据库 |
| --- | --- | --- | --- | --- |
| 个人 / 家庭 / NAS | 1 vCPU | 128–256 MB | SSD，512 MB 起（另加图标上传余量） | SQLite |
| 小团队 / 共享空间 | 1–2 vCPU | 256–512 MB | SSD，1–2 GB | SQLite |
| 组织级 / 高并发 | 2–4 vCPU | 1–2 GB | SSD | SQLite（单机）或 MySQL（多实例、HA） |

### 用户规模参考

按「每在线用户约 0.1 req/s（挂机为主）」估算，并预留峰值余量：

| 配置 | 理论平均吞吐 | 挂机并发（理论上限） | 建议规模（含峰值余量） |
| --- | --- | --- | --- |
| 1 vCPU | ~10k req/s | ~10 万 | 2 万–4 万 |
| 2 vCPU | ~20k req/s | ~20 万 | 5 万–8 万 |
| 4 vCPU | ~40k req/s | ~40 万 | 10 万以上 |

用户越活跃（编辑、导入、频繁刷新），每人产生的请求率越高，可支撑的用户数相应下降。

### 影响容量的关键因素

1. **数据量**：列表接口返回整个空间的全部条目，条目越多，单请求成本越高；这是最需要实测的部分。
2. **写入频率**：SQLite 同一时刻只有一个写者，写吞吐受磁盘 fsync 限制（数百次写/秒量级）。写密集或多实例场景建议改用 MySQL。
3. **峰值而非平均**：按峰值选型，通常预留均值 2–5 倍的余量。
4. **静态资源**：建议由反向代理或 CDN 承载 `/assets/*` 和带 hash 的资源，见「Cloudflare 缓存」。

## 数据库

支持 SQLite（默认）、MySQL、MariaDB 和 PostgreSQL。

- `base.database_drive`：`sqlite`（默认）| `mysql` | `postgres`。**MariaDB 使用 `mysql`**。
- SQLite：`sqlite.file_path`（另见 `read_pool_size`）。
- MySQL / MariaDB：`mysql.{host,port,username,password,db_name,wait_timeout}`。
- PostgreSQL：`postgres.{host,port,username,password,db_name,ssl_mode,time_zone,wait_timeout}`。

SQLite 专属能力（WAL、只读连接池、基于文件快照的迁移备份）只适用于 SQLite；MySQL/MariaDB/PostgreSQL 请使用各自工具备份。

### 从 SQLite 迁移到 MySQL/MariaDB/PostgreSQL

```bash
# conf.yaml        —— 现有 SQLite 配置
# conf.target.yaml —— 指向 mysql 或 postgres 的目标配置（目标库须为空）
./yin-panel migrate-db -source conf.yaml -target conf.target.yaml
```

迁移会在目标库创建完整 schema、按表复制全部数据并**保留主键**（PostgreSQL 会同步重置自增序列），可重复执行（每次替换目标表内容）。完成后把 `conf.yaml` 的 `database_drive` 指向目标库即可。

## 配置与 OAuth/OIDC

`oauth` 是 OAuth 2.0 和 OpenID Connect（OIDC）的统一配置入口。默认配置模板提供了三个已验证的示例：GitHub（OAuth 2.0）、GitLab（标准 OIDC）和 Google（OAuth 2.0）。示例默认全部注释，填写实际凭据并将 `oauth.enable` 设置为 `true` 后才会启用。

标准 OIDC provider 需要支持 discovery、authorization code + PKCE、JWKS 签名校验，并在用户信息中提供 `email` 和 `email_verified` claims。符合这些标准的 OIDC provider 理论上可用，但不同服务商可能仍需要额外的字段映射配置。

OIDC 回调地址：

```text
<base.root_url>/api/oauth/<provider>/callback
```

回调完成后，Provider 必须返回已验证邮箱。系统使用 `provider + sub` 识别外部身份，并使用邮箱关联本地账号。

## 空间与权限

空间权限使用内部 `UserID` 关联，不依赖邮箱作为权限主键。因此修改邮箱后，已有空间权限仍然有效，成员列表会显示最新邮箱。

- 管理员：管理空间、成员、角色和 OIDC 规则
- 编辑者：维护分组和书签
- 查看者：只读访问

创建空间、重命名、添加分组、添加成员和添加 OIDC 规则均使用弹窗操作。

## Helm

```bash
helm upgrade --install yin-panel ./distribution/helm-chart/yin-panel \
  --set image.tag=0.3.15
```

部署前建议运行 `helm template yin-panel ./distribution/helm-chart/yin-panel` 检查渲染结果。

## 源码开发

环境要求：Go `1.24`、Node.js `24.21.0`、npm 或 pnpm `9.15.5`。

```bash
cd frontend
npm ci
npm run build-only
```

也可以使用锁定版本的 pnpm：

```bash
cd frontend
corepack pnpm install --frozen-lockfile
pnpm run build-only
```

```bash
cd backend
go test ./...
go build -o /tmp/yin-panel-build/yin-panel .
```

本地完整更新（前端、后端、部署和重启）统一使用：

```bash
./scripts/deploy-local.sh
```

脚本默认将仓库内的 `backend/` 作为运行目录，要求其中存在本地 `conf.yaml`；可通过 `.env.local` 中的 `YIN_PANEL_RUNTIME_DIR` 覆盖。脚本会在构建和测试全部通过后，使用 `sudo` 删除旧二进制与静态文件并替换新产物，不会修改数据库、上传文件或配置文件。

完整的 AI 开发、发布和重启约定见 [AGENTS.md](./AGENTS.md)。

## 备份与升级

升级前请备份数据库、`uploads/`、`conf.yaml` 和 Helm values。迁移会保留用户、空间、分组、书签和文件引用。发生异常时使用升级前备份恢复，不要直接删除数据库重建。

## Cloudflare 缓存

如果域名启用了 Cloudflare 橙色小云，建议使用 Cache Rules 保持入口文件实时更新、只长期缓存带 hash 的静态资源。

绕过缓存：`/`、`/index.html`、`/login`、`/sw.js`、`/registerSW.js`、`/manifest.webmanifest` 和 `/api/*`。

长期缓存：`/assets/*` 和 `/workbox-*.js`。这些文件由构建系统生成 hash 文件名，源站会返回 30 天有效期和 `immutable`。

不要对整个站点启用 `Cache Everything`。发布后只清理入口文件和 Service Worker 的 Cloudflare 缓存，带 hash 的资源可以继续复用边缘缓存。

## Open Core

Yin-Panel Core 版本永久免费，并会持续维护和更新。本仓库是公开 Core 的唯一来源，发布 `yin-panel-ce` 镜像。企业版位于私有仓库 [yinorg/Yin-Panel-EE](https://github.com/yinorg/Yin-Panel-EE)，通过公开接口接入企业模块。

运行中的发行版可通过 `GET /api/system/capabilities` 查询发行版、模块和只读状态。许可证见 [LICENSE](./LICENSE)。问题和建议请提交到 [GitHub Issues](https://github.com/yinorg/Yin-Panel/issues)。

## 项目沿革与致谢

Yin-Panel 基于开源项目 [hslr-s/sun-panel](https://github.com/hslr-s/sun-panel) 发展，感谢原作者建立了项目基础。

同时感谢 [PhantomMaa/sun-panel](https://github.com/PhantomMaa/sun-panel) 对项目进行的优化和维护工作。Yin-Panel 在此基础上接任后续维护，并持续加入新的功能、交互体验和适合自托管场景的改进。

感谢所有原作者、贡献者和使用者。
