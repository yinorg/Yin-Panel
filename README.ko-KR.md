<div align="center">
    <img src="frontend/public/assets/apple-touch-icon.png" alt="Yin-Panel 로고" width="128" height="128">

<h1>Yin-Panel</h1>

[![최신 릴리스](https://img.shields.io/github/v/release/yinorg/Yin-Panel?style=flat-square)](https://github.com/yinorg/Yin-Panel/releases)
[![라이선스: BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-brightgreen.svg?style=flat-square)](LICENSE)
![프런트엔드 검사](https://github.com/yinorg/Yin-Panel/workflows/Frontend%20Checks/badge.svg)

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [日本語](README.ja-JP.md) | **한국어** | [Deutsch](README.de-DE.md) | [Français](README.fr-FR.md) | [Español](README.es-ES.md) | [Português (Brasil)](README.pt-BR.md) | [Русский](README.ru-RU.md)

</div>

Yin-Panel은 개인 및 공유 공간을 위한 가볍고 셀프 호스팅 가능한 내비게이션 패널입니다. 가정 네트워크, NAS, 서버, 소규모 조직에 적합합니다.

## 데모

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="doc/images/readme-home-dark.jpg">
  <img alt="Yin-Panel 홈 화면: 북마크 그룹, 시스템 상태, 검색" src="doc/images/readme-home-light.jpg">
</picture>

| 항목 편집기 | 공간 관리 |
| --- | --- |
| ![아이콘 편집기로 북마크 추가](doc/images/readme-editor-light.jpg) | ![공개 액세스와 북마크 가져오기/내보내기가 있는 공간 관리](doc/images/readme-spaces-light.jpg) |

## 기능

- 개인 공간과 공유 공간, 콘텐츠는 서로 격리
- 공간 생성, 이름 변경, 복사, 관리자 양도
- 그룹 트리, 북마크, 아이콘, 정렬 관리
- 브라우저 HTML 형식 북마크 가져오기와 내보내기
- 공간 멤버 역할: 관리자, 편집자, 뷰어
- OAuth 2.0 / OIDC 로그인, 계정 연결, OIDC 그룹 권한 부여
- 이메일을 로그인 계정으로 사용하고 닉네임은 별도로 관리
- 사용자 지정 아이콘, 설치 가능한 테마 패키지, 10개 UI 언어, 배경 화면, WAN/LAN 및 모바일 URL 모드, 페이지 내 팝업 열기
- SQLite, MySQL, MariaDB, PostgreSQL 스토리지, amd64/arm64 Docker 이미지, Kubernetes용 Helm 차트

## 빠른 시작

### Docker Compose

Docker와 Compose 플러그인이 필요합니다.

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

브라우저에서 <http://localhost:3002>를 엽니다.

기본 관리자: `admin@yiniot.com`, 비밀번호 `admin@yiniot.com`. 첫 로그인 후 즉시 비밀번호를 변경하고 `conf.yaml`에서 `base.root_url`과 고유한 `jwt.secret`을 설정하세요.

모든 배포 환경은 각자의 무작위 `jwt.secret`을 생성해야 합니다. 저장소는 `conf.yaml.example`만 제공합니다. 실행 디렉터리의 `conf.yaml`에는 인스턴스 비밀과 OAuth 자격 증명이 포함되므로 Git에 커밋하지 마세요.

### GHCR 이미지

```bash
docker pull ghcr.io/yinorg/yin-panel-ce:0.6.0
docker pull ghcr.io/yinorg/yin-panel-ce:latest
```

`0.6.0`과 `latest`는 모두 amd64와 arm64 호스트를 위한 멀티 아키텍처 매니페스트입니다. 릴리스 태그에는 아키텍처 접미사가 붙지 않습니다. `-amd64`, `-arm64` 같은 태그는 정식 릴리스에 포함되지 않습니다.

## 성능 및 권장 구성

아래 수치는 로컬 단일 머신 벤치마크(SQLite, 빈 데이터베이스, loopback, 동일 호스트 부하 테스트)에서 나온 값으로, 구성 선택의 참고 자료일 뿐 프로덕션 용량 보장이 아닙니다. 실제 용량은 주로 공간별 데이터 양, 쓰기 빈도, 최대 동시성에 따라 달라집니다.

### 리소스 사용량(실측 규모)

| 지표 | 유휴 | 5000 req/s 혼합 부하 | 한계(수만 req/s) |
| --- | --- | --- | --- |
| 메모리 RSS | ~16 MB | ~105 MB | 최대 ~500 MB, 부하 중지 후 감소 |
| CPU | 거의 0 | ~0.3 코어 | 2–3 코어 |

- 메모리는 약 **2만 req/s**까지 **105 MB** 수준에서 거의 일정하며 부하에 따라 선형으로 증가하지 않습니다. ~500 MB는 합성 극한 부하에서만 나타납니다.
- 단일 코어로 약 **1만–1.2만 req/s**(혼합 읽기/쓰기, 소규모 데이터)를 처리할 수 있습니다.
- 5000 req/s 혼합 부하에서 실패 0건, p95 약 **0.25 ms**.

### 권장 구성

| 시나리오 | CPU | 메모리 | 스토리지 | 데이터베이스 |
| --- | --- | --- | --- | --- |
| 개인 / 가정 / NAS | 1 vCPU | 128–256 MB | SSD, 512 MB 이상(아이콘 업로드 여유 포함) | SQLite |
| 소규모 팀 / 공유 공간 | 1–2 vCPU | 256–512 MB | SSD, 1–2 GB | SQLite |
| 조직 / 높은 동시성 | 2–4 vCPU | 1–2 GB | SSD | SQLite(단일 호스트) 또는 MySQL(다중 인스턴스, HA) |

### 사용자 규모 참고

온라인 사용자당 약 0.1 req/s(대부분 유휴)로 추정하고 최대 여유를 확보한 경우:

| 구성 | 이론상 평균 처리량 | 유휴 동시 접속(이론상 한도) | 권장 규모(최대 여유 포함) |
| --- | --- | --- | --- |
| 1 vCPU | ~10k req/s | ~10만 | 2만–4만 |
| 2 vCPU | ~20k req/s | ~20만 | 5만–8만 |
| 4 vCPU | ~40k req/s | ~40만 | 10만 이상 |

사용자가 활발할수록(편집, 가져오기, 잦은 새로 고침) 사용자당 요청률이 높아지고 지원 가능한 사용자 수는 그만큼 줄어듭니다.

### 용량에 영향을 주는 핵심 요인

1. **데이터 양**: 목록 API는 한 번의 응답으로 공간의 모든 항목을 반환하므로 항목이 많을수록 요청 비용이 커집니다. 직접 실측해 볼 가치가 가장 큰 부분입니다.
2. **쓰기 빈도**: SQLite는 한 번에 하나의 쓰기만 허용하며 쓰기 처리량은 디스크 fsync에 제한됩니다(초당 수백 회 수준). 쓰기가 많거나 다중 인스턴스 구성이면 MySQL을 권장합니다.
3. **평균이 아닌 최대치**: 최대치를 기준으로 선정하고 보통 평균의 2–5배 여유를 둡니다.
4. **정적 자산**: `/assets/*`와 해시가 포함된 자산은 리버스 프록시나 CDN에서 제공하는 것이 좋습니다. "Cloudflare 캐시"를 참고하세요.

## 데이터베이스

SQLite(기본), MySQL, MariaDB, PostgreSQL을 지원합니다.

- `base.database_drive`: `sqlite`(기본) | `mysql` | `postgres`. **MariaDB는 `mysql`을 사용합니다.**
- SQLite: `sqlite.file_path`(`read_pool_size`도 참고).
- MySQL / MariaDB: `mysql.{host,port,username,password,db_name,wait_timeout}`.
- PostgreSQL: `postgres.{host,port,username,password,db_name,ssl_mode,time_zone,wait_timeout}`.

SQLite 전용 기능(WAL, 읽기 전용 연결 풀, 파일 스냅샷 기반 마이그레이션 백업)은 SQLite에만 적용됩니다. MySQL/MariaDB/PostgreSQL은 각자의 도구로 백업하세요.

### SQLite에서 MySQL/MariaDB/PostgreSQL로 마이그레이션

```bash
# conf.yaml        — 기존 SQLite 구성
# conf.target.yaml — mysql 또는 postgres를 가리키는 대상 구성(대상 데이터베이스는 비어 있어야 합니다)
./yin-panel migrate-db -source conf.yaml -target conf.target.yaml
```

마이그레이션은 대상에 전체 스키마를 생성하고 모든 행을 **기본 키를 유지한 채** 복사합니다(PostgreSQL에서는 ID 시퀀스도 재설정됩니다). 반복 실행할 수 있으며, 실행할 때마다 대상 테이블 내용이 교체됩니다. 완료 후 `conf.yaml`의 `database_drive`를 대상으로 변경하세요.

## 구성과 OAuth/OIDC

`oauth`는 OAuth 2.0과 OpenID Connect(OIDC)의 통합 구성 진입점입니다. 기본 템플릿에는 검증된 세 가지 예시가 있습니다: GitHub(OAuth 2.0), GitLab(표준 OIDC), Google(OAuth 2.0). 예시는 기본적으로 모두 주석 처리되어 있으며, 실제 자격 증명을 입력하고 `oauth.enable`을 `true`로 설정해야 활성화됩니다. 모든 구성 키는 `backend/conf.yaml.example`에 문서화되어 있습니다.

표준 OIDC 공급자는 discovery, authorization code + PKCE, JWKS 서명 검증을 지원해야 하며 사용자 정보에서 `email`과 `email_verified`를 제공해야 합니다. 이 기준을 충족하는 공급자는 원칙적으로 사용할 수 있지만 공급자마다 추가 필드 매핑 구성이 필요할 수 있습니다.

OIDC 콜백 URL:

```text
<base.root_url>/api/oauth/<provider>/callback
```

콜백 후 공급자는 검증된 이메일을 반환해야 합니다. 시스템은 `provider + sub`로 외부 신원을 식별하고 이메일로 로컬 계정을 연결합니다.

## 공간과 권한

공간 권한은 내부 `UserID`로 연결되며 이메일 텍스트를 권한 기본 키로 사용하지 않습니다. 따라서 이메일을 변경해도 기존 공간 권한은 유지되고 멤버 목록에는 최신 이메일이 표시됩니다.

- 관리자: 공간, 멤버, 역할, OIDC 규칙 관리
- 편집자: 그룹과 북마크 관리
- 뷰어: 읽기 전용 접근

공간 생성, 이름 변경, 그룹 추가, 멤버 추가, OIDC 규칙 추가는 모두 대화 상자에서 수행합니다.

## Helm

```bash
helm upgrade --install yin-panel ./distribution/helm-chart/yin-panel \
  --set image.tag=0.6.0
```

배포 전에 `helm template yin-panel ./distribution/helm-chart/yin-panel`로 렌더링 결과를 확인할 수 있습니다.

## 소스 개발

환경 요구 사항: Go `1.27.1`, Node.js `24.21.0`, npm 또는 pnpm `9.15.5`.

```bash
cd frontend
npm ci
npm run build-only
```

고정된 pnpm을 사용하는 경우:

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

로컬 전체 업데이트(프런트엔드 빌드, 백엔드 테스트와 빌드, 배포, 재시작)는 한 명령으로 실행합니다:

```bash
./scripts/deploy-local.sh
```

스크립트는 기본적으로 저장소의 `backend/`를 실행 디렉터리로 사용하며 로컬 `conf.yaml`이 필요합니다. `.env.local`의 `YIN_PANEL_RUNTIME_DIR`로 재정의할 수 있습니다. 빌드와 테스트가 모두 통과하면 Yin-Panel 리스너만 중지하고 `sudo`로 이전 바이너리와 정적 파일을 교체하며 데이터베이스, 업로드 파일, 구성 파일은 건드리지 않습니다.

홈 페이지 테마 시스템은 [`frontend/THEME_SYSTEM.md`](frontend/THEME_SYSTEM.md)에 문서화되어 있습니다. `npm run create:theme -- init "<이름>"`으로 새 테마 뼈대를 만들 수 있습니다. AI 개발·릴리스·재시작 규칙은 [AGENTS.md](./AGENTS.md)에 있습니다.

## 백업과 업그레이드

업그레이드 전에 데이터베이스, `uploads/`, `conf.yaml`, Helm values를 백업하세요. 마이그레이션은 사용자, 공간, 그룹, 북마크, 파일 참조를 보존합니다. 문제가 발생하면 업그레이드 전 백업으로 복원하고 데이터베이스를 삭제해 다시 만들지 마세요.

## Cloudflare 캐시

도메인에 Cloudflare 주황색 구름을 사용한다면 Cache Rules로 진입 파일을 항상 최신으로 유지하고 해시된 정적 자산만 장기 캐시하는 것을 권장합니다.

캐시 우회: `/`, `/index.html`, `/login`, `/sw.js`, `/registerSW.js`, `/manifest.webmanifest`, `/api/*`.

장기 캐시: `/assets/*`와 `/workbox-*.js`. 이 파일들은 콘텐츠 해시 이름을 가지며 오리진이 30일 `immutable`로 응답합니다.

사이트 전체에 `Cache Everything`을 활성화하지 마세요. 릴리스 후에는 진입 파일과 Service Worker의 Cloudflare 캐시만 정리하고 해시된 자산은 엣지 캐시에 남겨 두어도 됩니다.

## Open Core

Yin-Panel Core 에디션은 무료이며 계속 유지 관리되고 업데이트됩니다. 이 저장소는 공개 Core의 유일한 소스이며 `yin-panel-ce` 이미지를 게시합니다. 엔터프라이즈 에디션은 비공개 저장소 [yinorg/Yin-Panel-EE](https://github.com/yinorg/Yin-Panel-EE)에 있으며 공개 인터페이스로 연결됩니다.

실행 중인 인스턴스는 `GET /api/system/capabilities`에서 에디션, 기능, 읽기 전용 상태, 버전을 확인할 수 있습니다. 문제와 제안은 [GitHub Issues](https://github.com/yinorg/Yin-Panel/issues)에 남겨 주세요.

## 감사의 글

Yin-Panel은 오픈 소스 프로젝트 [hslr-s/sun-panel](https://github.com/hslr-s/sun-panel)에서 발전했습니다. 프로젝트 기반을 만든 원저자에게 감사드립니다.

[PhantomMaa/sun-panel](https://github.com/PhantomMaa/sun-panel)의 최적화와 유지 관리 작업에도 감사드립니다. Yin-Panel은 그 기반을 이어받아 지속적으로 유지 관리하며 새로운 기능, 사용성 개선, 셀프 호스팅에 적합한 개선을 계속 추가하고 있습니다.

모든 원저자, 기여자, 사용자에게 감사드립니다.

## 라이선스

Yin-Panel은 [Business Source License 1.1](./LICENSE)에 따라 라이선스됩니다. 개인 사용 또는 조직의 내부 비즈니스 운영을 위한 프로덕션 사용은 무료입니다. 각 버전은 최초 공개 배포 후 3년이 지나면 AGPL-3.0-or-later 라이선스로 전환됩니다.
