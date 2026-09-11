# Yin-Panel

Yin-Panel 是轻量级个人与共享导航面板，适合服务器、NAS 和家庭网络，支持书签管理、空间隔离、协作权限与 OIDC 单点登录。

## 功能

- 默认个人空间，刷新后优先进入“我的空间”
- 共享空间创建、复制、重命名和管理员转让
- 分组与书签按空间隔离
- 管理员、编辑者、查看者三种角色
- 使用邮箱管理成员和空间转让
- OIDC 验证邮箱自动合并账号，支持分组授权
- 自定义图标、主题、网络模式和网页小窗
- SQLite 本地数据库，支持 Docker 与 ARM

## 技术栈

Go、Gin、GORM、SQLite、Vue 3、TypeScript、Vite、Naive UI。

## 快速运行

Docker 部署：在 `docker` 目录执行 `docker compose up -d`，默认访问 `http://localhost:3002`。

源码运行：先在 `frontend` 执行 `pnpm install` 和 `pnpm run build-only`，再在 `backend` 执行 `go build -o ../yin-panel` 和 `./../yin-panel -c conf.yaml`。

首次启动后请修改配置文件中的安全密钥和管理员配置。

## OIDC

在 `backend/conf.yaml` 的 `oauth.providers` 中配置 Provider、客户端凭据、Issuer 和 `openid profile email` scopes。Provider 必须返回已验证邮箱。系统使用 `provider + sub` 识别外部身份，并以规范化邮箱自动关联本地账号。

## 空间与权限

空间管理员可管理名称、转让管理员、成员角色和 OIDC 规则；编辑者可维护分组和书签；查看者只有查看权限。同名空间允许存在，界面会追加空间 ID，例如 `DevOps 1`、`DevOps 2`。

## 开发检查

后端执行 `go test ./...`，前端执行 `pnpm run build-only`。

当前稳定版本：`0.3.1`。许可证详见 [LICENSE](./LICENSE)。
