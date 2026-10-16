<div align="center">
    <img src="../../frontend/public/assets/apple-touch-icon.png" alt="โลโก้ Yin-Panel" width="128" height="128">

<h1>Yin-Panel</h1>

[![เวอร์ชันล่าสุด](https://img.shields.io/github/v/release/yinorg/Yin-Panel?style=flat-square)](https://github.com/yinorg/Yin-Panel/releases)
[![สัญญาอนุญาต: BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-brightgreen.svg?style=flat-square)](../../LICENSE)
![การตรวจสอบฟรอนต์เอนด์](https://github.com/yinorg/Yin-Panel/workflows/Frontend%20Checks/badge.svg)

[English](../../README.md) | [简体中文](../zh-CN/README.md) | [繁體中文](../zh-TW/README.md) | [日本語](../ja-JP/README.md) | [한국어](../ko-KR/README.md) | [Deutsch](../de-DE/README.md) | [Français](../fr-FR/README.md) | [Español](../es-ES/README.md) | [Português (Brasil)](../pt-BR/README.md) | [Русский](../ru-RU/README.md) | [Bahasa Indonesia](../id-ID/README.md) | [Türkçe](../tr-TR/README.md) | [Polski](../pl-PL/README.md) | [Italiano](../it-IT/README.md) | [Nederlands](../nl-NL/README.md) | [हिन्दी](../hi-IN/README.md) | **ไทย**

</div>

Yin-Panel เป็นแผงนำทางแบบเบาที่โฮสต์เองได้ สำหรับสเปซส่วนตัวและสเปซที่แชร์ เหมาะกับเครือข่ายภายในบ้าน อุปกรณ์ NAS เซิร์ฟเวอร์ และองค์กรขนาดเล็ก

## เดโม

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../images/readme-home-dark.jpg">
  <img alt="หน้าแรกของ Yin-Panel พร้อมกลุ่มบุ๊กมาร์ก ระบบมอนิเตอร์ และการค้นหา" src="../images/readme-home-light.jpg">
</picture>

| ตัวแก้ไขรายการ | การจัดการสเปซ |
| --- | --- |
| ![การเพิ่มบุ๊กมาร์กด้วยตัวแก้ไขไอคอน](../images/readme-editor-light.jpg) | ![การจัดการสเปซพร้อมการเข้าถึงสาธารณะและการนำเข้า/ส่งออกบุ๊กมาร์ก](../images/readme-spaces-light.jpg) |

### ตัวอย่างอินเทอร์เฟซ

| เข้าสู่ระบบ | การเข้าถึงสาธารณะ |
| --- | --- |
| ![เข้าสู่ระบบ](../images/readme-login-light.jpg) | ![การเข้าถึงสาธารณะ](../images/readme-public-light.jpg) |

| ศูนย์ภาพพื้นหลัง | วอลเปเปอร์เริ่มต้นของสเปซ |
| --- | --- |
| ![ศูนย์ภาพพื้นหลัง](../images/readme-wallpaper-light.jpg) | ![วอลเปเปอร์เริ่มต้นของสเปซ](../images/readme-space-wallpaper-light.jpg) |

**ธีม Directory**: แต่ละสเปซเลือกแพ็กเกจธีมของตัวเองได้ สเปซในอินสแตนซ์เดียวกันจึงดูต่างกันได้

![หน้าแรกของธีม Directory](../images/readme-home-space-light.jpg)

![รูปลักษณ์](../images/readme-appearance-light.jpg)

## คุณสมบัติ

- สเปซส่วนตัวและสเปซที่แชร์ โดยมีเนื้อหาแยกจากกัน
- การสร้าง เปลี่ยนชื่อ คัดลอกสเปซ และการโอนสิทธิ์ผู้ดูแล
- ต้นไม้กลุ่ม บุ๊กมาร์ก ไอคอน และการจัดเรียง
- การนำเข้าและส่งออกบุ๊กมาร์กในรูปแบบ HTML ของเบราว์เซอร์
- สมาชิกสเปซพร้อมบทบาทผู้ดูแลระบบ ผู้แก้ไข และผู้ชม
- การเข้าสู่ระบบด้วย OAuth 2.0 / OIDC การเชื่อมโยงบัญชี และการอนุญาตกลุ่ม OIDC
- อีเมลเป็นตัวตนสำหรับเข้าสู่ระบบ พร้อมชื่อเล่นที่เป็นอิสระ
- ไอคอนกำหนดเอง แพ็กเกจธีมที่ติดตั้งได้ ภาษา UI 17 ภาษา ภาพพื้นหลัง โหมด URL แบบ WAN/LAN และมือถือ และหน้าต่างป๊อปอัปภายในหน้า
- ที่จัดเก็บข้อมูลแบบ SQLite, MySQL, MariaDB หรือ PostgreSQL อิมเมจ Docker สำหรับ amd64 และ arm64 และ Helm chart สำหรับ Kubernetes

## เริ่มต้นอย่างรวดเร็ว

### Docker Compose

ต้องมี Docker พร้อมปลั๊กอิน Compose

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

เปิด <http://localhost:3002>.

ผู้ดูแลเริ่มต้น: `admin@yiniot.com` รหัสผ่าน `admin@yiniot.com` เปลี่ยนรหัสผ่านทันทีหลังการเข้าสู่ระบบครั้งแรก และตั้งค่า `base.root_url` และ `jwt.secret` ที่ไม่ซ้ำกันใน `conf.yaml`

ทุกการติดตั้งควรสร้าง `jwt.secret` แบบสุ่มของตัวเอง Repository นี้มาพร้อมเฉพาะ `conf.yaml.example` เท่านั้น ส่วน `conf.yaml` ในไดเรกทอรีรันไทม์ของคุณเก็บความลับของอินสแตนซ์และข้อมูลรับรอง OAuth และต้องไม่คอมมิตขึ้น Git

### อิมเมจ GHCR

```bash
docker pull ghcr.io/yinorg/yin-panel-ce:0.6.0
docker pull ghcr.io/yinorg/yin-panel-ce:latest
```

`0.6.0` และ `latest` เป็น manifest หลายสถาปัตยกรรมสำหรับโฮสต์ amd64 และ arm64 แท็ก release จะไม่ใส่ส่วนต่อท้ายสถาปัตยกรรม แท็กอย่าง `-amd64` หรือ `-arm64` ไม่ใช่ส่วนหนึ่งของ release

## ประสิทธิภาพและการตั้งค่าที่แนะนำ

ตัวเลขด้านล่างมาจากการทดสอบประสิทธิภาพบนเครื่องเดี่ยวในเครื่อง (SQLite ฐานข้อมูลว่าง ลูปแบ็ก และตัวสร้างโหลดบนโฮสต์เดียวกัน) โปรดใช้เป็นข้อมูลอ้างอิงในการกำหนดขนาด ไม่ใช่สัญญารับประกันความจุสำหรับ production ความจุจริงขึ้นอยู่กับปริมาณข้อมูลต่อสเปซเป็นหลัก อัตราการเขียน และการทำงานพร้อมกันสูงสุด

### การใช้ทรัพยากร (ค่าที่วัดได้)

| ตัวชี้วัด | ว่าง | โหลดผสม 5000 req/s | ขีดจำกัด (หลายหมื่น req/s) |
| --- | --- | --- | --- |
| หน่วยความจำ (RSS) | ~16 MB | ~105 MB | สูงสุด ~500 MB; ลดกลับหลังหยุดโหลด |
| CPU | เกือบศูนย์ | ~0.3 คอร์ | 2–3 คอร์ |

- หน่วยความจำคงอยู่ที่ประมาณ **105 MB** จนถึงราว **20k req/s** และไม่เพิ่มขึ้นเป็นเส้นตรงตามโหลด ตัวเลข ~500 MB จะปรากฏเฉพาะภายใต้แรงกดดันสูงสุดแบบจำลองเท่านั้น
- หนึ่งคอร์รับได้ประมาณ **10k–12k req/s** (อ่าน/เขียนผสม ชุดข้อมูลเล็ก)
- ภายใต้โหลดผสม 5000 req/s: ล้มเหลว 0 ครั้ง p95 ประมาณ **0.25 ms**

### การตั้งค่าที่แนะนำ

| สถานการณ์ | CPU | หน่วยความจำ | พื้นที่จัดเก็บ | ฐานข้อมูล |
| --- | --- | --- | --- | --- |
| ส่วนตัว / ภายในบ้าน / NAS | 1 vCPU | 128–256 MB | SSD, 512 MB+ (บวกพื้นที่สำรองสำหรับการอัปโหลดไอคอน) | SQLite |
| ทีมเล็ก / สเปซที่แชร์ | 1–2 vCPU | 256–512 MB | SSD, 1–2 GB | SQLite |
| องค์กร / การทำงานพร้อมกันสูง | 2–4 vCPU | 1–2 GB | SSD | SQLite (โฮสต์เดียว) หรือ MySQL (หลายอินสแตนซ์, HA) |

### ข้อมูลอ้างอิงจำนวนผู้ใช้

สมมติว่าใช้ประมาณ 0.1 req/s ต่อผู้ใช้ออนไลน์หนึ่งคน (ส่วนใหญ่อยู่ว่าง) และกันพื้นที่สำรองสำหรับช่วงพีค:

| การตั้งค่า | ปริมาณงานเฉลี่ยตามทฤษฎี | การทำงานพร้อมกันขณะว่าง (เพดานตามทฤษฎี) | ขนาดที่แนะนำ (พร้อมพื้นที่สำรองสำหรับพีค) |
| --- | --- | --- | --- |
| 1 vCPU | ~10k req/s | ~100k | 20k–40k |
| 2 vCPU | ~20k req/s | ~200k | 50k–80k |
| 4 vCPU | ~40k req/s | ~400k | 100k+ |

ยิ่งผู้ใช้มีความเคลื่อนไหวมาก (แก้ไข นำเข้า รีเฟรชบ่อย) อัตราคำขอต่อผู้ใช้ก็จะยิ่งสูงขึ้น และจำนวนผู้ใช้ที่รองรับได้อย่างยั่งยืนก็จะยิ่งต่ำลง

### ปัจจัยสำคัญที่ส่งผลต่อความจุ

1. **ปริมาณข้อมูล** — endpoint แบบรายการจะคืนทุกรายการของสเปซในคำตอบเดียว ยิ่งมีรายการมาก คำขอก็ยิ่งมีต้นทุนสูง นี่คือส่วนที่คุ้มค่าที่สุดในการวัดด้วยข้อมูลของคุณเอง
2. **อัตราการเขียน** — SQLite อนุญาตให้มีผู้เขียนเพียงรายเดียวในแต่ละครั้ง ปริมาณงานการเขียนถูกจำกัดด้วย fsync ของดิสก์ (ระดับหลายร้อยการเขียนต่อวินาที) สำหรับการตั้งค่าที่เขียนหนักหรือหลายอินสแตนซ์ ให้ใช้ MySQL
3. **พีค ไม่ใช่ค่าเฉลี่ย** — กำหนดขนาดสำหรับช่วงพีค โดยปกติกันไว้ 2–5 เท่าของค่าเฉลี่ย
4. **ไฟล์สแตติก** — ให้ reverse proxy หรือ CDN ให้บริการ `/assets/*` และไฟล์ที่มีแฮชอื่นๆ ดู "การแคชของ Cloudflare"

## ฐานข้อมูล

รองรับ SQLite (ค่าเริ่มต้น), MySQL, MariaDB และ PostgreSQL

- `base.database_drive`: `sqlite` (ค่าเริ่มต้น) | `mysql` | `postgres` **MariaDB ใช้ `mysql`**
- SQLite: `sqlite.file_path` (ดู `read_pool_size` ด้วย)
- MySQL / MariaDB: `mysql.{host,port,username,password,db_name,wait_timeout}`
- PostgreSQL: `postgres.{host,port,username,password,db_name,ssl_mode,time_zone,wait_timeout}`

ความสามารถที่มีเฉพาะ SQLite (WAL พูลการเชื่อมต่อแบบอ่านอย่างเดียว และการสำรองข้อมูลก่อนการย้ายข้อมูลด้วยสแนปช็อตไฟล์) ใช้ได้กับ SQLite เท่านั้น ให้สำรองข้อมูล MySQL/MariaDB/PostgreSQL ด้วยเครื่องมือของฐานข้อมูลนั้นๆ

### การย้ายข้อมูลจาก SQLite ไปยัง MySQL/MariaDB/PostgreSQL

```bash
# conf.yaml        — การตั้งค่า SQLite ที่มีอยู่
# conf.target.yaml — การตั้งค่าเป้าหมายที่ชี้ไปที่ mysql หรือ postgres (ฐานข้อมูลเป้าหมายต้องว่างเปล่า)
./yin-panel migrate-db -source conf.yaml -target conf.target.yaml
```

การย้ายข้อมูลจะสร้างสคีมาจากทั้งหมดในฐานข้อมูลเป้าหมาย คัดลอกทุกแถวโดยรักษาคีย์หลักไว้ (และรีเซ็ตลำดับ identity บน PostgreSQL) สามารถรันซ้ำได้ โดยแต่ละครั้งจะแทนที่เนื้อหาตารางเป้าหมาย หลังจากนั้นให้ชี้ `database_drive` ใน `conf.yaml` ไปที่ฐานข้อมูลเป้าหมาย

## การตั้งค่าและ OAuth/OIDC

`oauth` คือจุดตั้งค่าที่รวมกันสำหรับ OAuth 2.0 และ OpenID Connect (OIDC) เทมเพลตเริ่มต้นมีตัวอย่างที่ผ่านการตรวจสอบแล้วสามตัวอย่าง ได้แก่ GitHub (OAuth 2.0), GitLab (OIDC มาตรฐาน) และ Google (OAuth 2.0) ตัวอย่างทั้งหมดถูกคอมเมนต์ไว้โดยค่าเริ่มต้น ผู้ให้บริการจะเปิดใช้งานได้ก็ต่อเมื่อคุณกรอกข้อมูลรับรองจริงและตั้งค่า `oauth.enable: true` คีย์การตั้งค่าทั้งหมดมีเอกสารอธิบายใน `backend/conf.yaml.example`

ผู้ให้บริการ OIDC มาตรฐานต้องรองรับ discovery, authorization code + PKCE และการตรวจสอบลายเซ็น JWKS และต้องให้ `email` และ `email_verified` ในข้อมูลผู้ใช้ ผู้ให้บริการที่เป็นไปตามมาตรฐานเหล่านี้ทำงานได้โดยหลักการ แต่ผู้ให้บริการแต่ละรายอาจยังต้องตั้งค่าการแมปฟิลด์เพิ่มเติม

URL callback ของ OIDC:

```text
<base.root_url>/api/oauth/<provider>/callback
```

หลัง callback ผู้ให้บริการต้องส่งอีเมลที่ยืนยันแล้วกลับมา ระบบระบุตัวตนภายนอกด้วย `provider + sub` และเชื่อมโยงบัญชีภายในด้วยอีเมล

## สเปซและสิทธิ์

สิทธิ์ของสเปซผูกกับ `UserID` ภายในเสมอ ไม่ผูกกับข้อความอีเมล ดังนั้นการเปลี่ยนอีเมลจึงยังคงสิทธิ์สเปซเดิมไว้ และรายการสมาชิกจะแสดงอีเมลล่าสุด

- ผู้ดูแลระบบ: จัดการสเปซ สมาชิก บทบาท และกฎ OIDC
- ผู้แก้ไข: ดูแลกลุ่มและบุ๊กมาร์ก
- ผู้ชม: เข้าถึงแบบอ่านอย่างเดียว

การสร้างสเปซ การเปลี่ยนชื่อ การเพิ่มกลุ่ม การเพิ่มสมาชิก และการเพิ่มกฎ OIDC ล้วนทำในกล่องโต้ตอบ

## Helm

```bash
helm upgrade --install yin-panel ./distribution/helm-chart/yin-panel \
  --set image.tag=0.6.0
```

ก่อนติดตั้ง ให้ดูตัวอย่าง manifest ที่เรนเดอร์แล้วด้วย `helm template yin-panel ./distribution/helm-chart/yin-panel`

## การพัฒนาจากซอร์สโค้ด

ข้อกำหนด: Go `1.27.1`, Node.js `24.21.0`, npm หรือ pnpm `9.15.5`

```bash
cd frontend
npm ci
npm run build-only
```

หรือใช้ pnpm เวอร์ชันที่กำหนดไว้:

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

อัปเดตในเครื่องแบบเต็มรูปแบบ (บิลด์ฟรอนต์เอนด์ ทดสอบและบิลด์แบ็กเอนด์ ติดตั้ง และรีสตาร์ต) ด้วยคำสั่งเดียว:

```bash
./scripts/deploy-local.sh
```

สคริปต์ใช้ `backend/` ของ repository เป็นไดเรกทอรีรันไทม์โดยค่าเริ่มต้น และต้องมี `conf.yaml` ในเครื่องอยู่ที่นั่น `YIN_PANEL_RUNTIME_DIR` ใน `.env.local` สามารถแทนที่ได้ หลังจากบิลด์และการทดสอบทั้งหมดผ่าน จะหยุดเฉพาะตัวรับฟังของ Yin-Panel ใช้ `sudo` เพื่อแทนที่ไบนารีเดิมและไฟล์สแตติก และไม่แตะต้องฐานข้อมูล อัปโหลด หรือไฟล์การตั้งค่าเลย

ระบบธีมของหน้าแรกมีเอกสารอยู่ใน [`frontend/THEME_SYSTEM.md`](../../frontend/THEME_SYSTEM.md); สร้างโครงธีมใหม่ได้ด้วย `npm run create:theme -- init "<name>"` แนวปฏิบัติด้านการพัฒนา AI การออก release และการรีสตาร์ตฉบับเต็มอยู่ใน [AGENTS.md](../../AGENTS.md)

## การสำรองข้อมูลและการอัปเกรด

ก่อนอัปเกรด ให้สำรองฐานข้อมูล `uploads/` `conf.yaml` และค่า Helm การย้ายข้อมูลจะรักษาผู้ใช้ สเปซ กลุ่ม บุ๊กมาร์ก และการอ้างอิงไฟล์ไว้ หากการอัปเกรดผิดพลาด ให้คืนค่าจากข้อมูลสำรองก่อนอัปเกรด อย่าลบฐานข้อมูลแล้วเริ่มใหม่

## การแคชของ Cloudflare

หากโดเมนใช้ Cloudflare โหมดเมฆสีส้ม ให้เพิ่ม Cache Rules ที่ทำให้เอกสารหลักสดใหม่เสมอ และแคชเฉพาะไฟล์สแตติกที่มีแฮชเป็นเวลานาน

ข้ามแคช: `/`, `/index.html`, `/login`, `/sw.js`, `/registerSW.js`, `/manifest.webmanifest` และ `/api/*`

แคชระยะยาว: `/assets/*` และ `/workbox-*.js` ไฟล์เหล่านี้มีชื่อที่มีแฮชของเนื้อหา และต้นทางให้บริการด้วยการแคชแบบ `immutable` นาน 30 วัน

อย่าเปิดใช้งาน `Cache Everything` สำหรับทั้งไซต์ หลังออก release ให้ล้างเฉพาะเอกสารหลักและไฟล์ service worker ออกจากแคชของ Cloudflare ส่วนไฟล์ที่มีแฮชสามารถคงอยู่ที่ edge ได้

## Open Core

รุ่น Yin-Panel Core ใช้งานฟรีและจะได้รับการดูแลและอัปเดตต่อไป repository นี้เป็นแหล่งสาธารณะเพียงแห่งเดียวของ Core และเผยแพร่อิมเมจ `yin-panel-ce` รุ่น Enterprise อยู่ใน repository ส่วนตัว [yinorg/Yin-Panel-EE](https://github.com/yinorg/Yin-Panel-EE) และเชื่อมต่อผ่านอินเทอร์เฟซสาธารณะ

อินสแตนซ์ที่ทำงานอยู่จะเปิดเผยรุ่น ความสามารถ สถานะอ่านอย่างเดียว และเวอร์ชันที่ `GET /api/system/capabilities` รายงานปัญหาและข้อเสนอแนะได้ที่ [GitHub Issues](https://github.com/yinorg/Yin-Panel/issues)

การมีส่วนร่วมที่ถูก merge เข้า repository นี้จะอยู่ใน Core เสมอ: จะไม่ถูกย้ายไปยังรุ่น Enterprise และจะไม่ถูกเปลี่ยนสัญญาอนุญาต ดู [Core และ Enterprise](../../GOVERNANCE.md)

## กิตติกรรมประกาศ

Yin-Panel เติบโตมาจากโปรเจกต์โอเพนซอร์ส [hslr-s/sun-panel](https://github.com/hslr-s/sun-panel) ขอขอบคุณผู้เขียนต้นฉบับที่สร้างรากฐานนี้

ขอขอบคุณ [PhantomMaa/sun-panel](https://github.com/PhantomMaa/sun-panel) สำหรับการปรับแต่งและงานดูแลรักษา Yin-Panel รับช่วงการดูแลต่อจากฐานดังกล่าวและเพิ่มฟีเจอร์ การปรับปรุงการโต้ตอบ และการเปลี่ยนแปลงที่เน้นการโฮสต์เองอย่างต่อเนื่อง

ขอขอบคุณผู้เขียนต้นฉบับ ผู้มีส่วนร่วม และผู้ใช้ทุกคน

## สัญญาอนุญาต

Yin-Panel ได้รับสัญญาอนุญาตภายใต้ [Business Source License 1.1](../../LICENSE) การ fork การแก้ไข และการโฮสต์เองไม่มีค่าใช้จ่าย รวมถึงการใช้งานภายในองค์กรและการให้บริการลูกค้าของคุณเอง การให้บริการ Yin-Panel เป็นบริการโฮสต์แก่บุคคลที่สามต้องมีสัญญาอนุญาตเชิงพาณิชย์ แต่ละเวอร์ชันจะพร้อมใช้งานภายใต้สัญญาอนุญาต AGPL-3.0-or-later สามปีหลังการเผยแพร่ต่อสาธารณะครั้งแรกของเวอร์ชันนั้น ดู [Core และ Enterprise](../../GOVERNANCE.md)

ประกาศของบุคคลที่สามระบุอยู่ใน [THIRD-PARTY-NOTICES.md](../../THIRD-PARTY-NOTICES.md) และการมีส่วนร่วมได้รับการยอมรับภายใต้ [Contributor License Agreement](../../CLA.md)
