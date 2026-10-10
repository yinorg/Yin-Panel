<div align="center">
    <img src="../../frontend/public/assets/apple-touch-icon.png" alt="Yin-Panel-Logo" width="128" height="128">

<h1>Yin-Panel</h1>

[![Neuestes Release](https://img.shields.io/github/v/release/yinorg/Yin-Panel?style=flat-square)](https://github.com/yinorg/Yin-Panel/releases)
[![Lizenz: BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-brightgreen.svg?style=flat-square)](LICENSE)
![Frontend-Prüfungen](https://github.com/yinorg/Yin-Panel/workflows/Frontend%20Checks/badge.svg)

[English](../../README.md) | [简体中文](../zh-CN/README.md) | [繁體中文](../zh-TW/README.md) | [日本語](../ja-JP/README.md) | [한국어](../ko-KR/README.md) | **Deutsch** | [Français](../fr-FR/README.md) | [Español](../es-ES/README.md) | [Português (Brasil)](../pt-BR/README.md) | [Русский](../ru-RU/README.md)

</div>

Yin-Panel ist ein leichtgewichtiges, selbst hostbares Navigationspanel für persönliche und gemeinsame Bereiche. Es passt zu Heimnetzwerken, NAS-Geräten, Servern und kleinen Organisationen.

## Demo

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../images/readme-home-dark.jpg">
  <img alt="Yin-Panel Startseite mit Lesezeichen-Gruppen, Systemstatus und Suche" src="../images/readme-home-light.jpg">
</picture>

| Eintrag-Editor | Bereichsverwaltung |
| --- | --- |
| ![Lesezeichen mit dem Symbol-Editor hinzufügen](../images/readme-editor-light.jpg) | ![Bereichsverwaltung mit öffentlichem Zugriff und Lesezeichen-Import/-Export](../images/readme-spaces-light.jpg) |

## Funktionen

- Persönliche und gemeinsame Bereiche mit voneinander getrennten Inhalten
- Erstellen, Umbenennen, Kopieren und Übertragen der Administration von Bereichen
- Gruppenbäume, Lesezeichen, Symbole und Sortierung
- Lesezeichen-Import und -Export im HTML-Format des Browsers
- Bereichsmitglieder mit den Rollen Administrator, Bearbeiter und Betrachter
- OAuth 2.0-/OIDC-Anmeldung, Kontoverknüpfung und OIDC-Gruppenautorisierung
- E-Mail als Anmeldekonto, Nickname separat gepflegt
- Eigene Symbole, installierbare Theme-Pakete, 10 UI-Sprachen, Hintergründe, WAN/LAN- und Mobil-URL-Modus, Pop-up-Fenster innerhalb der Seite
- SQLite-, MySQL-, MariaDB- oder PostgreSQL-Speicher, Docker-Images für amd64 und arm64 und ein Helm-Chart für Kubernetes

## Schnellstart

### Docker Compose

Erfordert Docker mit dem Compose-Plugin.

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

Öffnen Sie <http://localhost:3002>.

Standard-Administrator: `admin@yiniot.com`, Passwort `admin@yiniot.com`. Ändern Sie das Passwort direkt nach der ersten Anmeldung und setzen Sie `base.root_url` und ein eindeutiges `jwt.secret` in `conf.yaml`.

Jede Bereitstellung sollte ein eigenes zufälliges `jwt.secret` erzeugen. Das Repository liefert nur `conf.yaml.example`; die `conf.yaml` im Laufzeitverzeichnis enthält Instanz-Geheimnisse und OAuth-Zugangsdaten und darf nicht in Git eingecheckt werden.

### GHCR-Images

```bash
docker pull ghcr.io/yinorg/yin-panel-ce:0.6.0
docker pull ghcr.io/yinorg/yin-panel-ce:latest
```

`0.6.0` und `latest` sind Multi-Architektur-Manifeste für amd64- und arm64-Hosts. Release-Tags tragen nie ein Architektur-Suffix; Tags wie `-amd64` oder `-arm64` sind kein Teil eines Releases.

## Leistung und empfohlene Konfiguration

Die folgenden Zahlen stammen aus lokalen Einzelmaschinen-Benchmarks (SQLite, leere Datenbank, Loopback, Lasttest auf demselben Host). Sie dienen als Dimensionierungshilfe, nicht als Kapazitätszusage für den Produktivbetrieb. Die tatsächliche Kapazität hängt vor allem vom Datenvolumen je Bereich, der Schreibfrequenz und der Spitzenlast ab.

### Ressourcenverbrauch (gemessene Größenordnungen)

| Metrik | Leerlauf | 5000 req/s Mischlast | Grenzbereich (Zehntausende req/s) |
| --- | --- | --- | --- |
| Speicher (RSS) | ~16 MB | ~105 MB | Spitze ~500 MB, fällt nach Lastende zurück |
| CPU | nahe 0 | ~0.3 Kern | 2–3 Kerne |

- Der Speicher bleibt bis etwa **20k req/s** konstant bei rund **105 MB** und wächst nicht linear mit der Last; die ~500 MB treten nur unter synthetischer Extremlast auf.
- Ein einzelner Kern trägt etwa **10k–12k req/s** (gemischtes Lesen/Schreiben, kleine Datenmenge).
- Bei 5000 req/s Mischlast: 0 Fehler, p95 etwa **0.25 ms**.

### Empfohlene Konfiguration

| Szenario | CPU | Arbeitsspeicher | Speicher | Datenbank |
| --- | --- | --- | --- | --- |
| Privat / Zuhause / NAS | 1 vCPU | 128–256 MB | SSD, 512 MB+ (plus Reserve für Symbol-Uploads) | SQLite |
| Kleines Team / gemeinsame Bereiche | 1–2 vCPU | 256–512 MB | SSD, 1–2 GB | SQLite |
| Organisation / hohe Nebenläufigkeit | 2–4 vCPU | 1–2 GB | SSD | SQLite (einzelner Host) oder MySQL (mehrere Instanzen, HA) |

### Referenz zur Benutzerzahl

Unter der Annahme von etwa 0.1 req/s pro Online-Benutzer (überwiegend im Leerlauf) und mit reservierter Spitzenreserve:

| Konfiguration | Theoretischer Durchschnittsdurchsatz | Leerlauf-Nebenläufigkeit (theoretisches Maximum) | Empfohlene Größe (inkl. Spitzenreserve) |
| --- | --- | --- | --- |
| 1 vCPU | ~10k req/s | ~100k | 20k–40k |
| 2 vCPU | ~20k req/s | ~200k | 50k–80k |
| 4 vCPU | ~40k req/s | ~400k | 100k+ |

Je aktiver die Nutzer sind (Bearbeiten, Importieren, häufiges Aktualisieren), desto höher ist die Anfrage-Rate pro Nutzer und desto niedriger die nachhaltig tragfähige Benutzerzahl.

### Wichtigste Faktoren für die Kapazität

1. **Datenvolumen** — Listen-Endpunkte liefern alle Einträge eines Bereichs in einer Antwort; mehr Einträge bedeuten teurere Anfragen. Diesen Teil lohnt es sich, mit eigenen Daten zu messen.
2. **Schreibfrequenz** — SQLite erlaubt nur einen Schreiber gleichzeitig; der Schreibdurchsatz ist durch Disk-fsync begrenzt (Größenordnung Hunderte Schreibvorgänge pro Sekunde). Für schreibintensive oder Multi-Instanz-Setups MySQL verwenden.
3. **Spitze statt Durchschnitt** — für Spitzen dimensionieren, üblicherweise 2–5× des Durchschnitts einplanen.
4. **Statische Assets** — `/assets/*` und andere gehashte Assets von einem Reverse-Proxy oder CDN ausliefern lassen; siehe „Cloudflare-Caching“.

## Datenbank

Unterstützt werden SQLite (Standard), MySQL, MariaDB und PostgreSQL.

- `base.database_drive`: `sqlite` (Standard) | `mysql` | `postgres`. **MariaDB verwendet `mysql`.**
- SQLite: `sqlite.file_path` (siehe auch `read_pool_size`).
- MySQL / MariaDB: `mysql.{host,port,username,password,db_name,wait_timeout}`.
- PostgreSQL: `postgres.{host,port,username,password,db_name,ssl_mode,time_zone,wait_timeout}`.

SQLite-spezifische Funktionen (WAL, schreibgeschützter Verbindungspool und dateischnappschussbasierte Migrations-Backups) gelten nur für SQLite; sichern Sie MySQL/MariaDB/PostgreSQL mit den jeweiligen Werkzeugen.

### Migration von SQLite zu MySQL/MariaDB/PostgreSQL

```bash
# conf.yaml        — bestehende SQLite-Konfiguration
# conf.target.yaml — Zielkonfiguration für mysql oder postgres (die Zieldatenbank muss leer sein)
./yin-panel migrate-db -source conf.yaml -target conf.target.yaml
```

Die Migration erstellt das vollständige Schema im Ziel, kopiert alle Zeilen unter **Beibehaltung der Primärschlüssel** (auf PostgreSQL werden auch ID-Sequenzen zurückgesetzt) und kann wiederholt ausgeführt werden — jeder Lauf ersetzt die Inhalte der Zieltabelle. Richten Sie danach `database_drive` in `conf.yaml` auf das Ziel aus.

## Konfiguration und OAuth/OIDC

`oauth` ist der einheitliche Konfigurationseinstieg für OAuth 2.0 und OpenID Connect (OIDC). Die Standardvorlage enthält drei verifizierte Beispiele: GitHub (OAuth 2.0), GitLab (Standard-OIDC) und Google (OAuth 2.0). Die Beispiele sind standardmäßig auskommentiert; ein Provider wird erst aktiviert, wenn echte Zugangsdaten eingetragen und `oauth.enable: true` gesetzt ist. Alle Konfigurationsschlüssel sind in `backend/conf.yaml.example` dokumentiert.

Ein Standard-OIDC-Provider muss Discovery, Authorization Code + PKCE und JWKS-Signaturprüfung unterstützen und `email` und `email_verified` in den Nutzerinformationen liefern. Provider, die diese Standards erfüllen, funktionieren grundsätzlich, können je nach Anbieter aber zusätzliche Feldzuordnungen erfordern.

OIDC-Callback-URL:

```text
<base.root_url>/api/oauth/<provider>/callback
```

Nach dem Callback muss der Provider eine verifizierte E-Mail-Adresse zurückgeben. Das System identifiziert externe Identitäten über `provider + sub` und verknüpft lokale Konten über die E-Mail-Adresse.

## Bereiche und Berechtigungen

Bereichsberechtigungen sind an die interne `UserID` gebunden, nie an den E-Mail-Text. Eine geänderte E-Mail-Adresse lässt bestehende Bereichsberechtigungen daher gültig, und die Mitgliederliste zeigt die neueste E-Mail-Adresse.

- Administrator: verwaltet Bereich, Mitglieder, Rollen und OIDC-Regeln
- Bearbeiter: pflegt Gruppen und Lesezeichen
- Betrachter: Lesezugriff

Erstellen eines Bereichs, Umbenennen, Hinzufügen von Gruppen, Mitgliedern und OIDC-Regeln geschehen in Dialogen.

## Helm

```bash
helm upgrade --install yin-panel ./distribution/helm-chart/yin-panel \
  --set image.tag=0.6.0
```

Vor dem Deployment können Sie die gerenderten Manifeste mit `helm template yin-panel ./distribution/helm-chart/yin-panel` prüfen.

## Entwicklung aus dem Quellcode

Anforderungen: Go `1.27.1`, Node.js `24.21.0`, npm oder pnpm `9.15.5`.

```bash
cd frontend
npm ci
npm run build-only
```

Oder mit dem gepinnten pnpm:

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

Vollständiges lokales Update (Frontend-Build, Backend-Tests und -Build, Deployment, Neustart) mit einem Befehl:

```bash
./scripts/deploy-local.sh
```

Das Skript verwendet standardmäßig `backend/` aus dem Repository als Laufzeitverzeichnis und erfordert dort eine lokale `conf.yaml`; `YIN_PANEL_RUNTIME_DIR` in `.env.local` kann dies überschreiben. Nach erfolgreichen Builds und Tests stoppt es nur den Yin-Panel-Listener, ersetzt altes Binary und statische Dateien per `sudo` und berührt Datenbank, Uploads und Konfigurationsdateien nicht.

Das Theme-System der Startseite ist in [`frontend/THEME_SYSTEM.md`](../../frontend/THEME_SYSTEM.md) dokumentiert; ein neues Theme lässt sich mit `npm run create:theme -- init "<Name>"` anlegen. Die vollständigen AI-Entwicklungs-, Release- und Neustart-Konventionen stehen in [AGENTS.md](../../AGENTS.md).

## Backup und Upgrade

Sichern Sie vor einem Upgrade Datenbank, `uploads/`, `conf.yaml` und Helm-Values. Migrationen erhalten Benutzer, Bereiche, Gruppen, Lesezeichen und Dateiverweise. Bei Problemen aus dem Backup vor dem Upgrade wiederherstellen; die Datenbank nicht löschen und neu aufbauen.

## Cloudflare-Caching

Wenn die Domain die orange Cloud von Cloudflare nutzt, halten Sie mit Cache Rules das Einstiegsdokument aktuell und cachen Sie nur gehashte statische Assets langfristig.

Cache umgehen: `/`, `/index.html`, `/login`, `/sw.js`, `/registerSW.js`, `/manifest.webmanifest` und `/api/*`.

Langzeit-Cache: `/assets/*` und `/workbox-*.js`. Diese Dateien tragen Namen mit Inhalts-Hash, und der Origin liefert sie mit 30 Tagen `immutable` aus.

Aktivieren Sie `Cache Everything` nicht für die gesamte Website. Nach einem Release nur das Einstiegsdokument und die Service-Worker-Dateien aus dem Cloudflare-Cache bereinigen; gehashte Assets können am Edge bleiben.

## Open Core

Die Core-Edition von Yin-Panel ist kostenlos und wird weiter gepflegt und aktualisiert. Dieses Repository ist die einzige öffentliche Quelle des Cores und veröffentlicht das `yin-panel-ce`-Image. Die Enterprise-Edition liegt im privaten Repository [yinorg/Yin-Panel-EE](https://github.com/yinorg/Yin-Panel-EE) und wird über öffentliche Schnittstellen angebunden.

Eine laufende Instanz zeigt Edition, Fähigkeiten, Nur-Lese-Status und Version unter `GET /api/system/capabilities`. Probleme und Vorschläge bitte in den [GitHub Issues](https://github.com/yinorg/Yin-Panel/issues) melden.

## Danksagung

Yin-Panel entstand aus dem Open-Source-Projekt [hslr-s/sun-panel](https://github.com/hslr-s/sun-panel); Dank an den ursprünglichen Autor für das Fundament.

Ebenso Dank an [PhantomMaa/sun-panel](https://github.com/PhantomMaa/sun-panel) für Optimierungen und Wartung. Yin-Panel übernahm auf dieser Basis die weitere Pflege und ergänzt laufend Funktionen, Interaktionsverbesserungen und Änderungen für Self-Hosting-Szenarien.

Dank an alle ursprünglichen Autoren, Mitwirkenden und Nutzer.

## Lizenz

Yin-Panel ist unter der [Business Source License 1.1](../../LICENSE) lizenziert: Die Produktionsnutzung ist für den persönlichen Gebrauch oder für die internen Geschäftsabläufe Ihrer Organisation kostenlos. Drei Jahre nach der ersten öffentlichen Verbreitung einer Version wechselt diese Version zur AGPL-3.0-or-later-Lizenz.
