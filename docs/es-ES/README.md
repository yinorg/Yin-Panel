<div align="center">
    <img src="../../frontend/public/assets/apple-touch-icon.png" alt="Logotipo de Yin-Panel" width="128" height="128">

<h1>Yin-Panel</h1>

[![Última versión](https://img.shields.io/github/v/release/yinorg/Yin-Panel?style=flat-square)](https://github.com/yinorg/Yin-Panel/releases)
[![Licencia: BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-brightgreen.svg?style=flat-square)](LICENSE)
![Comprobaciones del frontend](https://github.com/yinorg/Yin-Panel/workflows/Frontend%20Checks/badge.svg)

[English](../../README.md) | [简体中文](../zh-CN/README.md) | [繁體中文](../zh-TW/README.md) | [日本語](../ja-JP/README.md) | [한국어](../ko-KR/README.md) | [Deutsch](../de-DE/README.md) | [Français](../fr-FR/README.md) | **Español** | [Português (Brasil)](../pt-BR/README.md) | [Русский](../ru-RU/README.md)

</div>

Yin-Panel es un panel de navegación ligero y autoalojable para espacios personales y compartidos. Encaja en redes domésticas, NAS, servidores y pequeñas organizaciones.

## Demostración

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../images/readme-home-dark.jpg">
  <img alt="Página de inicio de Yin-Panel con grupos de marcadores, estado del sistema y búsqueda" src="../images/readme-home-light.jpg">
</picture>

| Editor de elementos | Gestión de espacios |
| --- | --- |
| ![Añadir un marcador con el editor de iconos](../images/readme-editor-light.jpg) | ![Gestión de espacios con acceso público e importación/exportación de marcadores](../images/readme-spaces-light.jpg) |

## Funciones

- Espacios personales y compartidos con contenido aislado entre sí
- Creación, cambio de nombre, copia y transferencia de administración de espacios
- Árboles de grupos, marcadores, iconos y ordenación
- Importación y exportación de marcadores en formato HTML del navegador
- Miembros del espacio con roles de administrador, editor y lector
- Inicio de sesión OAuth 2.0 / OIDC, vinculación de cuentas y autorización por grupos OIDC
- El correo electrónico como cuenta de inicio de sesión, con apodo gestionado por separado
- Iconos personalizados, temas instalables, 10 idiomas de interfaz, fondos, modos de URL WAN/LAN y móvil, ventanas emergentes dentro de la página
- Almacenamiento SQLite, MySQL, MariaDB o PostgreSQL, imágenes Docker para amd64 y arm64 y chart de Helm para Kubernetes

## Inicio rápido

### Docker Compose

Requiere Docker con el plugin de Compose.

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

Abra <http://localhost:3002>.

Administrador predeterminado: `admin@yiniot.com`, contraseña `admin@yiniot.com`. Cambie la contraseña inmediatamente después del primer inicio de sesión y establezca `base.root_url` y un `jwt.secret` único en `conf.yaml`.

Cada despliegue debe generar su propio `jwt.secret` aleatorio. El repositorio solo incluye `conf.yaml.example`; el `conf.yaml` de su directorio de ejecución contiene secretos de la instancia y credenciales OAuth y no debe subirse a Git.

### Imágenes GHCR

```bash
docker pull ghcr.io/yinorg/yin-panel-ce:0.6.0
docker pull ghcr.io/yinorg/yin-panel-ce:latest
```

`0.6.0` y `latest` son manifiestos multiarquitectura para hosts amd64 y arm64. Las etiquetas de versión nunca llevan sufijo de arquitectura; etiquetas como `-amd64` o `-arm64` no forman parte de una versión publicada.

## Rendimiento y configuración recomendada

Las cifras siguientes proceden de pruebas locales en una sola máquina (SQLite, base de datos vacía, loopback, prueba de carga en el mismo host). Sirven como referencia de dimensionamiento, no como promesa de capacidad en producción. La capacidad real depende principalmente del volumen de datos por espacio, la frecuencia de escritura y la concurrencia máxima.

### Uso de recursos (órdenes de magnitud medidos)

| Métrica | Inactivo | Carga mixta de 5000 req/s | Límite (decenas de miles de req/s) |
| --- | --- | --- | --- |
| Memoria (RSS) | ~16 MB | ~105 MB | Pico de ~500 MB; baja tras detener la carga |
| CPU | casi 0 | ~0,3 núcleo | 2–3 núcleos |

- La memoria se mantiene en torno a **105 MB** hasta unos **20 000 req/s** y no crece de forma lineal con la carga; el valor de ~500 MB solo aparece bajo presión sintética extrema.
- Un solo núcleo soporta unos **10 000–12 000 req/s** (lectura/escritura mixta, conjunto de datos pequeño).
- Con una carga mixta de 5000 req/s: 0 fallos, p95 de unos **0,25 ms**.

### Configuración recomendada

| Escenario | CPU | Memoria | Almacenamiento | Base de datos |
| --- | --- | --- | --- | --- |
| Personal / hogar / NAS | 1 vCPU | 128–256 MB | SSD, 512 MB+ (más margen para iconos subidos) | SQLite |
| Equipo pequeño / espacios compartidos | 1–2 vCPU | 256–512 MB | SSD, 1–2 GB | SQLite |
| Organización / alta concurrencia | 2–4 vCPU | 1–2 GB | SSD | SQLite (máquina única) o MySQL (varias instancias, HA) |

### Referencia de escala de usuarios

Suponiendo unos 0,1 req/s por usuario en línea (mayormente inactivo) y reservando margen para picos:

| Configuración | Rendimiento medio teórico | Concurrencia inactiva (techo teórico) | Escala recomendada (con margen de pico) |
| --- | --- | --- | --- |
| 1 vCPU | ~10k req/s | ~100k | 20k–40k |
| 2 vCPU | ~20k req/s | ~200k | 50k–80k |
| 4 vCPU | ~40k req/s | ~400k | 100k+ |

Cuanto más activos sean los usuarios (editar, importar, refrescar con frecuencia), mayor será la tasa de peticiones por usuario y menor el número de usuarios sostenible.

### Factores clave que afectan a la capacidad

1. **Volumen de datos**: los endpoints de lista devuelven todas las entradas de un espacio en una sola respuesta; cuantas más entradas, más costosa es la petición. Es la parte que más merece la pena medir con sus propios datos.
2. **Frecuencia de escritura**: SQLite permite un solo escritor a la vez; el rendimiento de escritura está limitado por el fsync del disco (del orden de cientos de escrituras por segundo). Para cargas intensivas de escritura o despliegues con varias instancias, use MySQL.
3. **Picos, no medias**: dimensione para los picos, reservando normalmente 2–5× la media.
4. **Recursos estáticos**: sirva `/assets/*` y otros recursos con hash desde un proxy inverso o CDN; consulte «Caché de Cloudflare».

## Base de datos

Se admiten SQLite (predeterminada), MySQL, MariaDB y PostgreSQL.

- `base.database_drive`: `sqlite` (predeterminada) | `mysql` | `postgres`. **MariaDB usa `mysql`.**
- SQLite: `sqlite.file_path` (véase también `read_pool_size`).
- MySQL / MariaDB: `mysql.{host,port,username,password,db_name,wait_timeout}`.
- PostgreSQL: `postgres.{host,port,username,password,db_name,ssl_mode,time_zone,wait_timeout}`.

Las capacidades exclusivas de SQLite (WAL, el pool de conexiones de solo lectura y las copias de migración basadas en instantáneas de archivo) solo se aplican a SQLite; haga copias de seguridad de MySQL/MariaDB/PostgreSQL con sus propias herramientas.

### Migración de SQLite a MySQL/MariaDB/PostgreSQL

```bash
# conf.yaml        — configuración existente de SQLite
# conf.target.yaml — configuración de destino apuntando a mysql o postgres (la base de datos de destino debe estar vacía)
./yin-panel migrate-db -source conf.yaml -target conf.target.yaml
```

La migración crea el esquema completo en el destino, copia todas las filas **conservando las claves primarias** (y restablece las secuencias de identidad en PostgreSQL). Se puede ejecutar repetidamente: cada ejecución reemplaza el contenido de las tablas de destino. Después, apunte `database_drive` en `conf.yaml` al destino.

## Configuración y OAuth/OIDC

`oauth` es el punto de entrada de configuración unificado para OAuth 2.0 y OpenID Connect (OIDC). La plantilla predeterminada incluye tres ejemplos verificados: GitHub (OAuth 2.0), GitLab (OIDC estándar) y Google (OAuth 2.0). Todos los ejemplos están comentados por defecto; un proveedor solo se activa tras rellenar credenciales reales y establecer `oauth.enable: true`. Todas las claves de configuración están documentadas en `backend/conf.yaml.example`.

Un proveedor OIDC estándar debe admitir discovery, código de autorización + PKCE y validación de firma JWKS, y proporcionar `email` y `email_verified` en la información del usuario. Los proveedores que cumplen estos estándares funcionan en principio, pero algunos servicios aún pueden necesitar configuración adicional de mapeo de campos.

URL de callback de OIDC:

```text
<base.root_url>/api/oauth/<provider>/callback
```

Tras el callback, el proveedor debe devolver un correo electrónico verificado. El sistema identifica identidades externas por `provider + sub` y vincula cuentas locales por correo electrónico.

## Espacios y permisos

Los permisos de espacio se vinculan al `UserID` interno, nunca al texto del correo electrónico. Por eso, cambiar el correo conserva los permisos de espacio existentes y la lista de miembros muestra el correo más reciente.

- Administrador: gestiona el espacio, los miembros, los roles y las reglas OIDC
- Editor: mantiene grupos y marcadores
- Lector: acceso de solo lectura

Crear un espacio, renombrarlo, añadir grupos, miembros y reglas OIDC se hace en diálogos.

## Helm

```bash
helm upgrade --install yin-panel ./distribution/helm-chart/yin-panel \
  --set image.tag=0.6.0
```

Antes de desplegar, revise los manifiestos renderizados con `helm template yin-panel ./distribution/helm-chart/yin-panel`.

## Desarrollo desde el código fuente

Requisitos: Go `1.27.1`, Node.js `24.21.0`, npm o pnpm `9.15.5`.

```bash
cd frontend
npm ci
npm run build-only
```

O con el pnpm fijado:

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

Actualización local completa (compilación del frontend, pruebas y compilación del backend, despliegue, reinicio) en un solo comando:

```bash
./scripts/deploy-local.sh
```

El script usa `backend/` del repositorio como directorio de ejecución de forma predeterminada y requiere un `conf.yaml` local allí; `YIN_PANEL_RUNTIME_DIR` en `.env.local` puede sobrescribirlo. Tras pasar todas las compilaciones y pruebas, detiene solo el listener de Yin-Panel, reemplaza el binario antiguo y los archivos estáticos con `sudo`, y nunca toca la base de datos, las subidas ni los archivos de configuración.

El sistema de temas de la página de inicio está documentado en [`frontend/THEME_SYSTEM.md`](../../frontend/THEME_SYSTEM.md); cree un tema nuevo con `npm run create:theme -- init "<nombre>"`. Las convenciones completas de desarrollo con IA, publicación y reinicio están en [AGENTS.md](../../AGENTS.md).

## Copia de seguridad y actualización

Antes de actualizar, haga copia de seguridad de la base de datos, `uploads/`, `conf.yaml` y los values de Helm. Las migraciones conservan usuarios, espacios, grupos, marcadores y referencias de archivos. Si algo va mal, restaure la copia previa a la actualización; no elimine la base de datos para recrearla.

## Caché de Cloudflare

Si el dominio usa la nube naranja de Cloudflare, añada Cache Rules para mantener el documento de entrada actualizado y cachear a largo plazo solo los recursos estáticos con hash.

Omitir caché: `/`, `/index.html`, `/login`, `/sw.js`, `/registerSW.js`, `/manifest.webmanifest` y `/api/*`.

Caché a largo plazo: `/assets/*` y `/workbox-*.js`. Estos archivos llevan nombres con hash de contenido y el origen los sirve con caché `immutable` de 30 días.

No active `Cache Everything` para todo el sitio. Tras una publicación, purgue solo el documento de entrada y los archivos del service worker de la caché de Cloudflare; los recursos con hash pueden permanecer en el edge.

## Open Core

La edición Core de Yin-Panel es gratuita y seguirá recibiendo mantenimiento y actualizaciones. Este repositorio es la única fuente pública del Core y publica la imagen `yin-panel-ce`. La edición Enterprise está en el repositorio privado [yinorg/Yin-Panel-EE](https://github.com/yinorg/Yin-Panel-EE) y se integra mediante interfaces públicas.

Una instancia en ejecución expone su edición, capacidades, estado de solo lectura y versión en `GET /api/system/capabilities`. Informe de problemas y sugerencias en los [GitHub Issues](https://github.com/yinorg/Yin-Panel/issues).

## Créditos

Yin-Panel nació del proyecto de código abierto [hslr-s/sun-panel](https://github.com/hslr-s/sun-panel); gracias al autor original por sentar las bases.

Gracias también a [PhantomMaa/sun-panel](https://github.com/PhantomMaa/sun-panel) por sus optimizaciones y labor de mantenimiento. Yin-Panel retomó el mantenimiento sobre esa base y sigue añadiendo funciones, mejoras de interacción y cambios pensados para el autoalojamiento.

Gracias a todos los autores originales, colaboradores y usuarios.

## Licencia

Yin-Panel se distribuye bajo la [Business Source License 1.1](../../LICENSE): el uso en producción es gratuito para uso personal o para las operaciones internas de su organización. Tres años después de la primera distribución pública de cada versión, esa versión pasa a la licencia AGPL-3.0-or-later.
