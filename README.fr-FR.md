<div align="center">
    <img src="frontend/public/assets/apple-touch-icon.png" alt="Logo Yin-Panel" width="128" height="128">

<h1>Yin-Panel</h1>

[![Dernière version](https://img.shields.io/github/v/release/yinorg/Yin-Panel?style=flat-square)](https://github.com/yinorg/Yin-Panel/releases)
[![Licence : BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-brightgreen.svg?style=flat-square)](LICENSE)
![Vérifications frontend](https://github.com/yinorg/Yin-Panel/workflows/Frontend%20Checks/badge.svg)

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [日本語](README.ja-JP.md) | [한국어](README.ko-KR.md) | [Deutsch](README.de-DE.md) | **Français** | [Español](README.es-ES.md) | [Português (Brasil)](README.pt-BR.md) | [Русский](README.ru-RU.md)

</div>

Yin-Panel est un panneau de navigation léger et auto-hébergeable pour les espaces personnels et partagés. Il convient aux réseaux domestiques, aux NAS, aux serveurs et aux petites organisations.

## Démonstration

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="doc/images/readme-home-dark.jpg">
  <img alt="Page d'accueil de Yin-Panel avec groupes de favoris, état du système et recherche" src="doc/images/readme-home-light.jpg">
</picture>

| Éditeur d'éléments | Gestion des espaces |
| --- | --- |
| ![Ajouter un favori avec l'éditeur d'icônes](doc/images/readme-editor-light.jpg) | ![Gestion des espaces avec accès public et import/export des favoris](doc/images/readme-spaces-light.jpg) |

## Fonctionnalités

- Espaces personnels et partagés, contenus isolés entre eux
- Création, renommage, copie et transfert d'administration des espaces
- Arborescences de groupes, signets, icônes et tri
- Import et export de signets au format HTML du navigateur
- Membres d'espace avec les rôles administrateur, éditeur et lecteur
- Connexion OAuth 2.0 / OIDC, liaison de comptes et autorisation par groupes OIDC
- L'adresse e-mail comme identifiant de connexion, pseudonyme géré séparément
- Icônes personnalisées, thèmes installables, 10 langues d'interface, arrière-plans, modes d'URL WAN/LAN et mobile, fenêtres contextuelles dans la page
- Stockage SQLite, MySQL, MariaDB ou PostgreSQL, images Docker amd64 et arm64, chart Helm pour Kubernetes

## Démarrage rapide

### Docker Compose

Nécessite Docker avec le plugin Compose.

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

Ouvrez <http://localhost:3002>.

Administrateur par défaut : `admin@yiniot.com`, mot de passe `admin@yiniot.com`. Changez le mot de passe immédiatement après la première connexion, et définissez `base.root_url` et un `jwt.secret` unique dans `conf.yaml`.

Chaque déploiement doit générer son propre `jwt.secret` aléatoire. Le dépôt ne fournit que `conf.yaml.example` ; le fichier `conf.yaml` de votre répertoire d'exécution contient les secrets de l'instance et les identifiants OAuth et ne doit pas être commité dans Git.

### Images GHCR

```bash
docker pull ghcr.io/yinorg/yin-panel-ce:0.6.0
docker pull ghcr.io/yinorg/yin-panel-ce:latest
```

`0.6.0` et `latest` sont des manifestes multi-architectures pour les hôtes amd64 et arm64. Les tags de version ne portent jamais de suffixe d'architecture ; les tags comme `-amd64` ou `-arm64` ne font pas partie d'une version publiée.

## Performances et configuration recommandée

Les chiffres ci-dessous proviennent de benchmarks locaux sur une seule machine (SQLite, base vide, loopback, test de charge sur le même hôte). Ils servent de référence de dimensionnement, pas de promesse de capacité en production. La capacité réelle dépend principalement du volume de données par espace, du débit d'écriture et de la concurrence de pointe.

### Utilisation des ressources (ordres de grandeur mesurés)

| Métrique | Inactif | Charge mixte 5000 req/s | Limite (dizaines de milliers de req/s) |
| --- | --- | --- | --- |
| Mémoire (RSS) | ~16 Mo | ~105 Mo | Pic ~500 Mo, redescend après l'arrêt de la charge |
| CPU | proche de 0 | ~0,3 cœur | 2–3 cœurs |

- La mémoire reste autour de **105 Mo** jusqu'à environ **20 000 req/s** et n'augmente pas linéairement avec la charge ; le pic de ~500 Mo n'apparaît que sous une pression synthétique extrême.
- Un seul cœur supporte environ **10 000–12 000 req/s** (lecture/écriture mixte, petit volume de données).
- Sous une charge mixte de 5000 req/s : 0 échec, p95 d'environ **0,25 ms**.

### Configuration recommandée

| Scénario | CPU | Mémoire | Stockage | Base de données |
| --- | --- | --- | --- | --- |
| Personnel / maison / NAS | 1 vCPU | 128–256 Mo | SSD, 512 Mo+ (plus marge pour les icônes téléversées) | SQLite |
| Petite équipe / espaces partagés | 1–2 vCPU | 256–512 Mo | SSD, 1–2 Go | SQLite |
| Organisation / forte concurrence | 2–4 vCPU | 1–2 Go | SSD | SQLite (machine unique) ou MySQL (plusieurs instances, HA) |

### Ordre de grandeur des utilisateurs

En supposant environ 0,1 req/s par utilisateur connecté (majoritairement inactif) et en réservant une marge de pointe :

| Configuration | Débit moyen théorique | Concurrence inactive (plafond théorique) | Échelle conseillée (avec marge de pointe) |
| --- | --- | --- | --- |
| 1 vCPU | ~10k req/s | ~100k | 20k–40k |
| 2 vCPU | ~20k req/s | ~200k | 50k–80k |
| 4 vCPU | ~40k req/s | ~400k | 100k+ |

Plus les utilisateurs sont actifs (édition, import, actualisations fréquentes), plus la demande par utilisateur est élevée, et plus le nombre d'utilisateurs soutenable diminue.

### Facteurs clés influant sur la capacité

1. **Volume de données** — les endpoints de liste renvoient toutes les entrées d'un espace en une seule réponse ; plus il y a d'entrées, plus la requête coûte cher. C'est le point le plus utile à mesurer avec vos propres données.
2. **Débit d'écriture** — SQLite n'autorise qu'un seul écrivain à la fois ; le débit d'écriture est limité par les fsync du disque (de l'ordre de quelques centaines d'écritures par seconde). Pour les charges d'écriture intense ou les déploiements multi-instances, utilisez MySQL.
3. **Le pic, pas la moyenne** — dimensionnez pour les pics, en prévoyant généralement 2 à 5× la moyenne.
4. **Ressources statiques** — faites servir `/assets/*` et les autres ressources hachées par un reverse proxy ou un CDN ; voir « Cache Cloudflare ».

## Base de données

SQLite (par défaut), MySQL, MariaDB et PostgreSQL sont pris en charge.

- `base.database_drive` : `sqlite` (par défaut) | `mysql` | `postgres`. **MariaDB utilise `mysql`.**
- SQLite : `sqlite.file_path` (voir aussi `read_pool_size`).
- MySQL / MariaDB : `mysql.{host,port,username,password,db_name,wait_timeout}`.
- PostgreSQL : `postgres.{host,port,username,password,db_name,ssl_mode,time_zone,wait_timeout}`.

Les capacités propres à SQLite (WAL, pool de connexions en lecture seule et sauvegardes de migration par instantané de fichier) ne s'appliquent qu'à SQLite ; sauvegardez MySQL/MariaDB/PostgreSQL avec leurs propres outils.

### Migration de SQLite vers MySQL/MariaDB/PostgreSQL

```bash
# conf.yaml        — configuration SQLite existante
# conf.target.yaml — configuration cible pointant vers mysql ou postgres (la base cible doit être vide)
./yin-panel migrate-db -source conf.yaml -target conf.target.yaml
```

La migration crée le schéma complet dans la cible, copie toutes les lignes en **conservant les clés primaires** (et réinitialise les séquences d'identité sur PostgreSQL). Elle peut être relancée — chaque exécution remplace le contenu des tables cibles. Ensuite, pointez `database_drive` dans `conf.yaml` vers la cible.

## Configuration et OAuth/OIDC

`oauth` est le point d'entrée de configuration unifié pour OAuth 2.0 et OpenID Connect (OIDC). Le modèle par défaut contient trois exemples vérifiés : GitHub (OAuth 2.0), GitLab (OIDC standard) et Google (OAuth 2.0). Les exemples sont tous commentés par défaut ; un fournisseur n'est activé qu'après avoir renseigné de vrais identifiants et défini `oauth.enable: true`. Toutes les clés de configuration sont documentées dans `backend/conf.yaml.example`.

Un fournisseur OIDC standard doit prendre en charge la découverte (discovery), le code d'autorisation + PKCE et la validation de signature JWKS, et fournir `email` et `email_verified` dans les informations utilisateur. Les fournisseurs conformes fonctionnent en principe, mais certains prestataires peuvent nécessiter une configuration supplémentaire du mappage des champs.

URL de rappel OIDC :

```text
<base.root_url>/api/oauth/<provider>/callback
```

Après le rappel, le fournisseur doit renvoyer une adresse e-mail vérifiée. Le système identifie les identités externes par `provider + sub` et associe les comptes locaux par e-mail.

## Espaces et permissions

Les permissions d'espace sont liées au `UserID` interne, jamais au texte de l'e-mail. Changer d'e-mail conserve donc les permissions d'espace existantes, et la liste des membres affiche l'e-mail le plus récent.

- Administrateur : gère l'espace, les membres, les rôles et les règles OIDC
- Éditeur : gère les groupes et les signets
- Lecteur : accès en lecture seule

La création d'un espace, le renommage, l'ajout de groupes, de membres et de règles OIDC se font dans des boîtes de dialogue.

## Helm

```bash
helm upgrade --install yin-panel ./distribution/helm-chart/yin-panel \
  --set image.tag=0.6.0
```

Avant le déploiement, prévisualisez les manifestes rendus avec `helm template yin-panel ./distribution/helm-chart/yin-panel`.

## Développement à partir des sources

Prérequis : Go `1.27.1`, Node.js `24.21.0`, npm ou pnpm `9.15.5`.

```bash
cd frontend
npm ci
npm run build-only
```

Ou avec le pnpm épinglé :

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

Mise à jour locale complète (build frontend, tests et build backend, déploiement, redémarrage) en une seule commande :

```bash
./scripts/deploy-local.sh
```

Le script utilise `backend/` du dépôt comme répertoire d'exécution par défaut et exige un `conf.yaml` local à cet endroit ; `YIN_PANEL_RUNTIME_DIR` dans `.env.local` peut le remplacer. Une fois les builds et les tests réussis, il arrête uniquement l'écouteur Yin-Panel, remplace l'ancien binaire et les fichiers statiques avec `sudo`, et ne touche jamais à la base de données, aux fichiers téléversés ni aux fichiers de configuration.

Le système de thèmes de la page d'accueil est documenté dans [`frontend/THEME_SYSTEM.md`](frontend/THEME_SYSTEM.md) ; créez un nouveau thème avec `npm run create:theme -- init "<nom>"`. Les conventions complètes de développement AI, de publication et de redémarrage se trouvent dans [AGENTS.md](./AGENTS.md).

## Sauvegarde et mise à niveau

Avant toute mise à niveau, sauvegardez la base de données, `uploads/`, `conf.yaml` et les values Helm. Les migrations préservent les utilisateurs, les espaces, les groupes, les signets et les références de fichiers. En cas de problème, restaurez la sauvegarde d'avant mise à niveau ; ne supprimez pas la base de données pour la recréer.

## Cache Cloudflare

Si le domaine utilise le nuage orange de Cloudflare, ajoutez des Cache Rules pour garder le document d'entrée à jour et ne mettre en cache à long terme que les ressources statiques hachées.

Contourner le cache : `/`, `/index.html`, `/login`, `/sw.js`, `/registerSW.js`, `/manifest.webmanifest` et `/api/*`.

Cache longue durée : `/assets/*` et `/workbox-*.js`. Ces fichiers portent un nom avec hash de contenu, et l'origine les sert avec un cache `immutable` de 30 jours.

N'activez pas `Cache Everything` pour tout le site. Après une publication, purgez uniquement le document d'entrée et les fichiers du service worker du cache Cloudflare ; les ressources hachées peuvent rester en périphérie.

## Open Core

L'édition Core de Yin-Panel est gratuite et continue d'être maintenue et mise à jour. Ce dépôt est la seule source publique du Core et publie l'image `yin-panel-ce`. L'édition Enterprise se trouve dans le dépôt privé [yinorg/Yin-Panel-EE](https://github.com/yinorg/Yin-Panel-EE) et s'intègre via des interfaces publiques.

Une instance en cours d'exécution expose son édition, ses capacités, son état en lecture seule et sa version sur `GET /api/system/capabilities`. Signalez problèmes et suggestions dans les [GitHub Issues](https://github.com/yinorg/Yin-Panel/issues).

## Remerciements

Yin-Panel est issu du projet open source [hslr-s/sun-panel](https://github.com/hslr-s/sun-panel) ; merci à l'auteur original d'avoir posé les fondations.

Merci également à [PhantomMaa/sun-panel](https://github.com/PhantomMaa/sun-panel) pour ses optimisations et son travail de maintenance. Yin-Panel a repris la maintenance sur cette base et continue d'ajouter des fonctionnalités, des améliorations d'interaction et des évolutions pensées pour l'auto-hébergement.

Merci à tous les auteurs originaux, contributeurs et utilisateurs.

## Licence

Yin-Panel est publié sous [Business Source License 1.1](./LICENSE) : l'utilisation en production est gratuite pour un usage personnel ou pour les opérations internes de votre organisation. Trois ans après la première distribution publique d'une version, cette version passe sous licence AGPL-3.0-or-later.
