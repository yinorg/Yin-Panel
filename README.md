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
- 支持 SQLite、MySQL、Docker、ARM64 和 Kubernetes

## 快速开始

### Docker Compose

```bash
mkdir yin-panel && cd yin-panel
curl -LO https://raw.githubusercontent.com/yinorg/Yin-Panel/master/docker/docker-compose.yml
curl -LO https://raw.githubusercontent.com/yinorg/Yin-Panel/master/backend/conf.yaml
mkdir -p database uploads
jwt_secret="$(openssl rand -hex 32)"
sed -i "s#^  secret: your_secret_key$#  secret: ${jwt_secret}#" conf.yaml
unset jwt_secret
docker compose up -d
```

每个部署实例都应使用独立的随机 `jwt.secret`。运行目录中的 `conf.yaml` 包含实例密钥和 OAuth 凭据，请勿提交到 Git。

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

## 配置与 OIDC

常用配置包括 `base.root_url`、`base.http_port`、`base.database_drive`、`sqlite.file_path`、`rclone.type`、`rclone.bucket`、`jwt.secret` 和 `oauth.providers`。

OIDC 回调地址：

```text
<base.root_url>/api/oauth/<provider>/callback
```

Provider 必须返回已验证邮箱。系统使用 `provider + sub` 识别外部身份，并使用邮箱关联本地账号。

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

环境要求：Go `1.24`、Node.js `18+` 和 npm/pnpm。

```bash
cd frontend
npm ci
npm run build-only
```

```bash
cd backend
go test ./...
go build -o /tmp/yin-panel-build/yin-panel .
```

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
