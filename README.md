# Yin-Panel

Yin-Panel 是一个轻量、可自托管的个人与团队导航面板，适合家庭网络、NAS、服务器和小型团队。

当前版本：`0.3.11`

## 功能

- 个人空间和共享空间，支持复制、重命名、管理员转让
- 分组树、书签和图标按空间隔离
- 管理员、编辑者、查看者三种角色
- OIDC/OAuth 登录、账号关联和基础 OIDC 分组授权
- 自定义图标、主题、语言、网络模式和网页小窗
- 图片上传、SHA-256 去重和私有 `/uploads/` 代理访问
- SQLite 默认开箱即用，也支持 MySQL、Docker、ARM64 和 Kubernetes

## Open Core

本仓库是公开 Core 的唯一来源，发布 `yin-panel-ce` 镜像。CE 只包含 Core 功能和本地文件存储。企业版位于私有仓库 [yinorg/Yin-Panel-EE](https://github.com/yinorg/Yin-Panel-EE)，通过公开 `backend/pkg/app`、`backend/pkg/extension` 和 `backend/pkg/storage` 接口接入企业模块。

S3 兼容存储、离线许可证、审计日志和高级 OIDC 组同步只存在于 EE 后端。运行中的发行版可通过 `GET /api/system/capabilities` 查询发行版、模块和只读状态；前端可见性不构成后端授权边界。

## 系统要求

预构建镜像需要 Docker 20.10+ 和 Compose v2，容器默认监听 `3002`。建议至少分配 128 MB RAM，并为数据库和上传目录配置持久化磁盘。源码开发需要 Go `1.24`、Node.js `18+`、pnpm 和 SQLite 所需的 C 编译器。

## Docker Compose

```bash
cd docker
cp ../backend/conf.yaml conf.yaml
docker compose up -d
```

访问 <http://localhost:3002>。首次部署请修改 `conf.yaml` 中的 `base.root_url` 和 `jwt.secret`，并通过反向代理启用 HTTPS。Compose 持久化 `database/`、`uploads/` 和 `conf.yaml`。

指定版本或自建镜像：

```bash
YIN_PANEL_IMAGE=ghcr.io/yinorg/yin-panel-ce:0.3.11 docker compose up -d
```

## 源码运行

```bash
pnpm --dir frontend install
pnpm --dir frontend run build-only
cd backend
go test ./...
go build -o ../yin-panel ./main.go
./../yin-panel -c conf.yaml
```

前端开发使用 `pnpm --dir frontend run dev`。后端配置中的相对路径以进程工作目录为准。

## 配置与 OIDC

核心配置位于 `backend/conf.yaml`，Docker/Helm 会挂载到 `/app/conf.yaml`。重要字段包括 `base.root_url`、`base.database_drive`、`base.url_prefix`、`sqlite.file_path`、`rclone.bucket` 和 `jwt.secret`。CE 的 `rclone.type` 必须为 `local`；上传对象按 SHA-256 分片存储，例如 `ab/cd/<sha256>.png`，读取统一经过应用代理。

在 `oauth.providers` 中填写 provider、客户端凭据、issuer 和 `openid profile email` scopes。回调地址为：

```text
<base.root_url>/api/oauth/<provider>/callback
```

Provider 必须返回已验证邮箱。系统使用 `provider + sub` 识别外部身份，并按规范化邮箱关联本地账号。

## 空间与权限

空间管理员可以管理空间、成员、角色和 OIDC 规则；编辑者可以维护分组与书签；查看者只能读取内容。手工成员不会因 OIDC 组同步而被删除。空间名称允许重复，界面会附加空间 ID 以便区分。

## Helm

```bash
helm upgrade --install yin-panel ./distribution/helm-chart/yin-panel \
  --set image.tag=0.3.11
```

默认使用 SQLite、本地上传目录和服务端口 `3002`。生产环境请配置持久化卷、`base.root_url`、JWT secret 和数据库连接，并先执行 `helm template` 检查渲染结果。EE 配置不应写入 CE values。

## 备份与升级

升级前备份数据库、上传目录、配置文件和 Helm values。Core 会保留空间、分组、书签及文件引用；迁移失败时从升级前备份恢复。EE 降级到 CE 不会自动回滚企业表，必须先按 EE 文档恢复备份。

## 开发检查

```bash
cd backend && go test ./...
cd frontend && pnpm run type-check && pnpm run build-only
```

EE 必须依赖已发布的 Core tag，不得复制 Core 源码或导入 Core `internal` 包。许可证见 [LICENSE](./LICENSE)，问题和建议请提交到 [GitHub Issues](https://github.com/yinorg/Yin-Panel/issues)。
