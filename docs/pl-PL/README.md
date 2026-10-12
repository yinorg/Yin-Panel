<div align="center">
    <img src="../../frontend/public/assets/apple-touch-icon.png" alt="Logo Yin-Panel" width="128" height="128">

<h1>Yin-Panel</h1>

[![Najnowsze wydanie](https://img.shields.io/github/v/release/yinorg/Yin-Panel?style=flat-square)](https://github.com/yinorg/Yin-Panel/releases)
[![Licencja: BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-brightgreen.svg?style=flat-square)](../../LICENSE)
![Kontrole frontendu](https://github.com/yinorg/Yin-Panel/workflows/Frontend%20Checks/badge.svg)

[English](../../README.md) | [简体中文](../zh-CN/README.md) | [繁體中文](../zh-TW/README.md) | [日本語](../ja-JP/README.md) | [한국어](../ko-KR/README.md) | [Deutsch](../de-DE/README.md) | [Français](../fr-FR/README.md) | [Español](../es-ES/README.md) | [Português (Brasil)](../pt-BR/README.md) | [Русский](../ru-RU/README.md) | [Bahasa Indonesia](../id-ID/README.md) | [Türkçe](../tr-TR/README.md) | **Polski** | [Italiano](../it-IT/README.md) | [Nederlands](../nl-NL/README.md) | [हिन्दी](../hi-IN/README.md) | [ไทย](../th-TH/README.md)

</div>

Yin-Panel to lekki panel nawigacyjny do samodzielnego hostowania, przeznaczony dla przestrzeni osobistych i współdzielonych. Sprawdza się w sieciach domowych, na urządzeniach NAS, serwerach i w małych organizacjach.

## Demo

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../images/readme-home-dark.jpg">
  <img alt="Strona główna Yin-Panel z grupami zakładek, monitorem systemu i wyszukiwaniem" src="../images/readme-home-light.jpg">
</picture>

| Edytor elementów | Zarządzanie przestrzeniami |
| --- | --- |
| ![Dodawanie zakładki za pomocą edytora ikon](../images/readme-editor-light.jpg) | ![Zarządzanie przestrzeniami z dostępem publicznym oraz importem/eksportem zakładek](../images/readme-spaces-light.jpg) |

## Funkcje

- Przestrzenie osobiste i współdzielone z odizolowaną zawartością
- Tworzenie, zmiana nazwy, kopiowanie przestrzeni i przekazywanie uprawnień administratora
- Drzewa grup, zakładki, ikony i sortowanie
- Import i eksport zakładek w formacie HTML przeglądarki
- Członkowie przestrzeni z rolami administratora, redaktora i widza
- Logowanie OAuth 2.0 / OIDC, łączenie kont i autoryzacja grup OIDC
- E-mail jako tożsamość logowania, z niezależnym pseudonimem
- Własne ikony, instalowalne pakiety motywów, 17 języków interfejsu, tapety, tryby WAN/LAN i mobilnego adresu URL oraz okna popup na stronie
- Magazyn SQLite, MySQL, MariaDB lub PostgreSQL, obrazy Docker dla amd64 i arm64 oraz Helm chart dla Kubernetes

## Szybki start

### Docker Compose

Wymaga Docker z wtyczką Compose.

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

Otwórz <http://localhost:3002>.

Domyślny administrator: `admin@yiniot.com`, hasło `admin@yiniot.com`. Zmień hasło natychmiast po pierwszym logowaniu i ustaw `base.root_url` oraz unikalny `jwt.secret` w `conf.yaml`.

Każde wdrożenie powinno wygenerować własny losowy `jwt.secret`. Repozytorium dostarcza tylko `conf.yaml.example`; plik `conf.yaml` w katalogu środowiska uruchomieniowego przechowuje sekrety instancji i dane uwierzytelniające OAuth i nie wolno go zatwierdzać w Git.

### Obrazy GHCR

```bash
docker pull ghcr.io/yinorg/yin-panel-ce:0.6.0
docker pull ghcr.io/yinorg/yin-panel-ce:latest
```

`0.6.0` i `latest` to manifesty wieloarchitekturowe dla hostów amd64 i arm64. Tagi wydań nigdy nie zawierają sufiksu architektury; tagi takie jak `-amd64` lub `-arm64` nie są częścią wydania.

## Wydajność i zalecana konfiguracja

Poniższe liczby pochodzą z lokalnych testów wydajności na jednej maszynie (SQLite, pusta baza danych, interfejs loopback, generator obciążenia na tym samym hoście). Traktuj je jako punkt odniesienia przy doborze rozmiaru, a nie jako gwarancję wydajności produkcyjnej. Rzeczywista wydajność zależy głównie od ilości danych na przestrzeń, tempa zapisu i szczytowego współbieżnego obciążenia.

### Zużycie zasobów (zmierzone wartości)

| Metryka | Bezczynność | Obciążenie mieszane 5000 req/s | Granica (dziesiątki tysięcy req/s) |
| --- | --- | --- | --- |
| Pamięć (RSS) | ~16 MB | ~105 MB | Szczyt ~500 MB; spada po ustaniu obciążenia |
| CPU | blisko zera | ~0.3 rdzenia | 2–3 rdzenie |

- Pamięć utrzymuje się na poziomie około **105 MB** aż do mniej więcej **20k req/s** i nie rośnie liniowo wraz z obciążeniem; wartość ~500 MB pojawia się tylko pod syntetycznym ekstremalnym obciążeniem.
- Pojedynczy rdzeń obsługuje około **10k–12k req/s** (mieszany odczyt/zapis, mały zbiór danych).
- Przy obciążeniu mieszanym 5000 req/s: 0 błędów, p95 około **0.25 ms**.

### Zalecana konfiguracja

| Scenariusz | CPU | Pamięć | Pamięć masowa | Baza danych |
| --- | --- | --- | --- | --- |
| Osobisty / domowy / NAS | 1 vCPU | 128–256 MB | SSD, 512 MB+ (plus zapas na przesyłanie ikon) | SQLite |
| Mały zespół / współdzielone przestrzenie | 1–2 vCPU | 256–512 MB | SSD, 1–2 GB | SQLite |
| Organizacja / duża współbieżność | 2–4 vCPU | 1–2 GB | SSD | SQLite (pojedynczy host) lub MySQL (wiele instancji, HA) |

### Odniesienie dotyczące skali użytkowników

Przy założeniu około 0.1 req/s na zalogowanego użytkownika (przeważnie bezczynnego) i z rezerwą na szczyty:

| Konfiguracja | Teoretyczna średnia przepustowość | Współbieżność bezczynna (teoretyczny pułap) | Sugerowana skala (z rezerwą na szczyty) |
| --- | --- | --- | --- |
| 1 vCPU | ~10k req/s | ~100k | 20k–40k |
| 2 vCPU | ~20k req/s | ~200k | 50k–80k |
| 4 vCPU | ~40k req/s | ~400k | 100k+ |

Im aktywniejsi są użytkownicy (edycja, import, częste odświeżanie), tym wyższy współczynnik żądań generowany przez każdego użytkownika i tym niższa możliwa do utrzymania liczba użytkowników.

### Kluczowe czynniki wpływające na wydajność

1. **Ilość danych** — punkty końcowe list zwracają wszystkie wpisy przestrzeni w jednej odpowiedzi; więcej wpisów oznacza droższe żądanie. To element, który najbardziej warto zmierzyć na własnych danych.
2. **Tempo zapisu** — SQLite pozwala na tylko jednego piszącego naraz; przepustowość zapisu jest ograniczona przez fsync dysku (rzędu setek zapisów na sekundę). W przypadku intensywnego zapisu lub konfiguracji z wieloma instancjami użyj MySQL.
3. **Szczyt, nie średnia** — rozmiar dobieraj pod szczyty, zwykle rezerwując 2–5× średniej.
4. **Zasoby statyczne** — pozwól serwerowi odwrotnemu proxy lub CDN serwować `/assets/*` i inne zasoby z hashem; zobacz „Buforowanie Cloudflare”.

## Baza danych

Obsługiwane są SQLite (domyślnie), MySQL, MariaDB i PostgreSQL.

- `base.database_drive`: `sqlite` (domyślnie) | `mysql` | `postgres`. **MariaDB używa `mysql`.**
- SQLite: `sqlite.file_path` (zobacz też `read_pool_size`).
- MySQL / MariaDB: `mysql.{host,port,username,password,db_name,wait_timeout}`.
- PostgreSQL: `postgres.{host,port,username,password,db_name,ssl_mode,time_zone,wait_timeout}`.

Funkcje dostępne tylko w SQLite (WAL, pula połączeń tylko do odczytu i kopie zapasowe migracji oparte na migawkach plików) dotyczą wyłącznie SQLite; wykonuj kopie zapasowe MySQL/MariaDB/PostgreSQL za pomocą ich własnych narzędzi.

### Migracja z SQLite do MySQL/MariaDB/PostgreSQL

```bash
# conf.yaml        — istniejąca konfiguracja SQLite
# conf.target.yaml — konfiguracja docelowa wskazująca na mysql lub postgres (docelowa baza danych musi być pusta)
./yin-panel migrate-db -source conf.yaml -target conf.target.yaml
```

Migracja tworzy pełny schemat w bazie docelowej, kopiuje każdy wiersz z zachowaniem kluczy głównych (i resetuje sekwencje tożsamości w PostgreSQL). Można ją uruchamiać wielokrotnie — każde uruchomienie zastępuje zawartość tabel docelowych. Następnie wskaż `database_drive` w `conf.yaml` na bazę docelową.

## Konfiguracja i OAuth/OIDC

`oauth` to jednolite wejście konfiguracyjne dla OAuth 2.0 i OpenID Connect (OIDC). Domyślny szablon zawiera trzy zweryfikowane przykłady: GitHub (OAuth 2.0), GitLab (standardowy OIDC) i Google (OAuth 2.0). Przykłady są domyślnie zakomentowane; dostawca jest włączany dopiero po wpisaniu prawdziwych danych uwierzytelniających i ustawieniu `oauth.enable: true`. Wszystkie klucze konfiguracji opisano w `backend/conf.yaml.example`.

Standardowy dostawca OIDC musi obsługiwać discovery, authorization code + PKCE oraz walidację podpisu JWKS, a także udostępniać `email` i `email_verified` w informacjach o użytkowniku. Dostawcy spełniający te standardy działają zasadniczo, ale różni producenci mogą mimo to wymagać dodatkowej konfiguracji mapowania pól.

Adres URL wywołania zwrotnego OIDC:

```text
<base.root_url>/api/oauth/<provider>/callback
```

Po wywołaniu zwrotnym dostawca musi zwrócić zweryfikowany adres e-mail. System identyfikuje tożsamości zewnętrzne po `provider + sub` i łączy lokalne konta na podstawie adresu e-mail.

## Przestrzenie i uprawnienia

Uprawnienia do przestrzeni są powiązane z wewnętrznym `UserID`, a nigdy z tekstem adresu e-mail. Zmiana adresu e-mail zachowuje zatem istniejące uprawnienia do przestrzeni, a lista członków pokazuje najnowszy adres e-mail.

- Administrator: zarządza przestrzenią, członkami, rolami i regułami OIDC
- Redaktor: utrzymuje grupy i zakładki
- Widz: dostęp tylko do odczytu

Tworzenie przestrzeni, zmiana jej nazwy, dodawanie grup, dodawanie członków i dodawanie reguł OIDC odbywa się w oknach dialogowych.

## Helm

```bash
helm upgrade --install yin-panel ./distribution/helm-chart/yin-panel \
  --set image.tag=0.6.0
```

Przed wdrożeniem przejrzyj wygenerowane manifesty za pomocą `helm template yin-panel ./distribution/helm-chart/yin-panel`.

## Rozwój ze źródeł

Wymagania: Go `1.27.1`, Node.js `24.21.0`, npm lub pnpm `9.15.5`.

```bash
cd frontend
npm ci
npm run build-only
```

Lub z przypiętym pnpm:

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

Pełna lokalna aktualizacja (budowa frontendu, testy i budowa backendu, wdrożenie, restart) jednym poleceniem:

```bash
./scripts/deploy-local.sh
```

Skrypt domyślnie używa katalogu `backend/` z repozytorium jako katalogu środowiska uruchomieniowego i wymaga tam lokalnego pliku `conf.yaml`; `YIN_PANEL_RUNTIME_DIR` w `.env.local` może go nadpisać. Po pomyślnym zakończeniu wszystkich budów i testów zatrzymuje tylko proces nasłuchujący Yin-Panel, używa `sudo` do zastąpienia starego pliku binarnego i plików statycznych i nigdy nie dotyka bazy danych, przesłanych plików ani plików konfiguracji.

System motywów strony głównej opisano w [`frontend/THEME_SYSTEM.md`](../../frontend/THEME_SYSTEM.md); utwórz szkielet nowego motywu za pomocą `npm run create:theme -- init "<name>"`. Pełne konwencje rozwoju z udziałem AI, wydań i restartu znajdują się w [AGENTS.md](../../AGENTS.md).

## Kopia zapasowa i aktualizacja

Przed aktualizacją wykonaj kopię zapasową bazy danych, katalogu `uploads/`, pliku `conf.yaml` i wartości Helm. Migracje zachowują użytkowników, przestrzenie, grupy, zakładki i odwołania do plików. Jeśli aktualizacja się nie powiedzie, przywróć kopię zapasową sprzed aktualizacji; nie usuwaj bazy danych i nie zaczynaj od nowa.

## Buforowanie Cloudflare

Jeśli domena korzysta z pomarańczowej chmury Cloudflare, dodaj reguły Cache Rules, które utrzymują dokument wejściowy w świeżym stanie i buforują na długo tylko statyczne zasoby z hashem.

Pomiń buforowanie: `/`, `/index.html`, `/login`, `/sw.js`, `/registerSW.js`, `/manifest.webmanifest` i `/api/*`.

Długie buforowanie: `/assets/*` i `/workbox-*.js`. Te pliki mają nazwy z hashem zawartości, a serwer źródłowy udostępnia je z 30-dniowym buforowaniem `immutable`.

Nie włączaj `Cache Everything` dla całej witryny. Po wydaniu usuń z pamięci podręcznej Cloudflare tylko dokument wejściowy i pliki service workera; zasoby z hashem mogą pozostać na brzegu sieci.

## Open Core

Wersja Core Yin-Panel jest bezpłatna i będzie nadal rozwijana oraz aktualizowana. To repozytorium jest jedynym publicznym źródłem wersji Core i publikuje obraz `yin-panel-ce`. Wersja Enterprise znajduje się w prywatnym repozytorium [yinorg/Yin-Panel-EE](https://github.com/yinorg/Yin-Panel-EE) i łączy się przez publiczne interfejsy.

Działająca instancja udostępnia swoją edycję, możliwości, stan tylko do odczytu i wersję pod `GET /api/system/capabilities`. Problemy i sugestie zgłaszaj w [GitHub Issues](https://github.com/yinorg/Yin-Panel/issues).

Wkłady scalone z tym repozytorium pozostają w Core: nie są przenoszone do wersji Enterprise ani ponownie licencjonowane. Zobacz [Core i Enterprise](../../GOVERNANCE.md).

## Podziękowania

Yin-Panel powstał na bazie projektu open source [hslr-s/sun-panel](https://github.com/hslr-s/sun-panel); podziękowania dla pierwotnego autora za stworzenie fundamentu.

Podziękowania również dla [PhantomMaa/sun-panel](https://github.com/PhantomMaa/sun-panel) za optymalizacje i prace konserwacyjne. Yin-Panel przejął od tej podstawy dalsze utrzymanie i stale dodaje funkcje, ulepszenia interakcji oraz zmiany ukierunkowane na samodzielne hostowanie.

Dziękujemy wszystkim pierwotnym autorom, współtwórcom i użytkownikom.

## Licencja

Yin-Panel jest objęty licencją [Business Source License 1.1](../../LICENSE). Forkowanie, modyfikowanie i samodzielny hosting są bezpłatne, obejmują użytkowanie wewnętrzne w organizacji oraz obsługę własnych klientów; oferowanie samego Yin-Panel jako usługi hostowanej osobom trzecim wymaga licencji komercyjnej. Każda wersja staje się dostępna na licencji AGPL-3.0-or-later trzy lata po jej pierwszej publicznej dystrybucji. Zobacz [Core i Enterprise](../../GOVERNANCE.md).

Informacje o stronach trzecich znajdują się w [THIRD-PARTY-NOTICES.md](../../THIRD-PARTY-NOTICES.md); wkłady są przyjmowane na podstawie [Contributor License Agreement](../../CLA.md).
