<div align="center">
    <img src="frontend/public/assets/apple-touch-icon.png" alt="Logotipo do Yin-Panel" width="128" height="128">

<h1>Yin-Panel</h1>

[![Última versão](https://img.shields.io/github/v/release/yinorg/Yin-Panel?style=flat-square)](https://github.com/yinorg/Yin-Panel/releases)
[![Licença: BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-brightgreen.svg?style=flat-square)](LICENSE)
![Verificações do frontend](https://github.com/yinorg/Yin-Panel/workflows/Frontend%20Checks/badge.svg)

[English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [日本語](README.ja-JP.md) | [한국어](README.ko-KR.md) | [Deutsch](README.de-DE.md) | [Français](README.fr-FR.md) | [Español](README.es-ES.md) | **Português (Brasil)** | [Русский](README.ru-RU.md)

</div>

O Yin-Panel é um painel de navegação leve e auto-hospedável para espaços pessoais e compartilhados. Ele atende redes domésticas, NAS, servidores e pequenas organizações.

## Demonstração

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="doc/images/readme-home-dark.jpg">
  <img alt="Página inicial do Yin-Panel com grupos de favoritos, status do sistema e pesquisa" src="doc/images/readme-home-light.jpg">
</picture>

| Editor de itens | Gerenciamento de espaços |
| --- | --- |
| ![Adicionar um favorito com o editor de ícones](doc/images/readme-editor-light.jpg) | ![Gerenciamento de espaços com acesso público e importação/exportação de favoritos](doc/images/readme-spaces-light.jpg) |

## Funcionalidades

- Espaços pessoais e compartilhados com conteúdo isolado entre si
- Criação, renomeação, cópia e transferência de administração de espaços
- Árvores de grupos, favoritos, ícones e ordenação
- Importação e exportação de favoritos no formato HTML do navegador
- Membros do espaço com papéis de administrador, editor e visualizador
- Login OAuth 2.0 / OIDC, vinculação de contas e autorização por grupos OIDC
- E-mail como conta de login, apelido mantido separadamente
- Ícones personalizados, temas instaláveis, 10 idiomas de interface, papéis de parede, modos de URL WAN/LAN e móvel, janelas pop-up dentro da página
- Armazenamento SQLite, MySQL, MariaDB ou PostgreSQL, imagens Docker para amd64 e arm64 e chart Helm para Kubernetes

## Início rápido

### Docker Compose

Requer Docker com o plugin Compose.

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

Administrador padrão: `admin@yiniot.com`, senha `admin@yiniot.com`. Altere a senha imediatamente após o primeiro login e defina `base.root_url` e um `jwt.secret` exclusivo no `conf.yaml`.

Cada implantação deve gerar seu próprio `jwt.secret` aleatório. O repositório fornece apenas o `conf.yaml.example`; o `conf.yaml` no diretório de execução contém segredos da instância e credenciais OAuth e não deve ser enviado ao Git.

### Imagens GHCR

```bash
docker pull ghcr.io/yinorg/yin-panel-ce:0.6.0
docker pull ghcr.io/yinorg/yin-panel-ce:latest
```

`0.6.0` e `latest` são manifestos multiarquitetura para hosts amd64 e arm64. As tags de versão nunca têm sufixo de arquitetura; tags como `-amd64` ou `-arm64` não fazem parte de uma versão publicada.

## Desempenho e configuração recomendada

Os números abaixo vêm de benchmarks locais em uma única máquina (SQLite, banco vazio, loopback, teste de carga no mesmo host). São referência de dimensionamento, não promessa de capacidade em produção. A capacidade real depende principalmente do volume de dados por espaço, da taxa de escrita e da concorrência de pico.

### Uso de recursos (ordens de magnitude medidas)

| Métrica | Ocioso | Carga mista de 5000 req/s | Limite (dezenas de milhares de req/s) |
| --- | --- | --- | --- |
| Memória (RSS) | ~16 MB | ~105 MB | Pico de ~500 MB; cai após o fim da carga |
| CPU | quase 0 | ~0,3 núcleo | 2–3 núcleos |

- A memória fica em torno de **105 MB** até cerca de **20 mil req/s** e não cresce linearmente com a carga; o valor de ~500 MB só aparece sob pressão sintética extrema.
- Um único núcleo suporta cerca de **10 mil–12 mil req/s** (leitura/escrita mista, conjunto de dados pequeno).
- Sob carga mista de 5000 req/s: 0 falhas, p95 de cerca de **0,25 ms**.

### Configuração recomendada

| Cenário | CPU | Memória | Armazenamento | Banco de dados |
| --- | --- | --- | --- | --- |
| Pessoal / casa / NAS | 1 vCPU | 128–256 MB | SSD, 512 MB+ (mais margem para ícones enviados) | SQLite |
| Equipe pequena / espaços compartilhados | 1–2 vCPU | 256–512 MB | SSD, 1–2 GB | SQLite |
| Organização / alta concorrência | 2–4 vCPU | 1–2 GB | SSD | SQLite (máquina única) ou MySQL (várias instâncias, HA) |

### Referência de escala de usuários

Supondo cerca de 0,1 req/s por usuário online (a maioria ocioso) e reservando margem para picos:

| Configuração | Throughput médio teórico | Concorrência ociosa (teto teórico) | Escala sugerida (com margem de pico) |
| --- | --- | --- | --- |
| 1 vCPU | ~10k req/s | ~100k | 20k–40k |
| 2 vCPU | ~20k req/s | ~200k | 50k–80k |
| 4 vCPU | ~40k req/s | ~400k | 100k+ |

Quanto mais ativos os usuários (edição, importação, atualizações frequentes), maior a taxa de requisições por usuário e menor o número sustentável de usuários.

### Principais fatores que afetam a capacidade

1. **Volume de dados**: os endpoints de listagem retornam todas as entradas de um espaço em uma única resposta; mais entradas significam requisições mais caras. É a parte que mais vale a pena medir com os seus próprios dados.
2. **Taxa de escrita**: o SQLite permite apenas um escritor por vez; o throughput de escrita é limitado pelo fsync do disco (na ordem de centenas de gravações por segundo). Para cargas intensas de escrita ou várias instâncias, use MySQL.
3. **Pico, não média**: dimensione para os picos, reservando normalmente 2–5× a média.
4. **Arquivos estáticos**: sirva `/assets/*` e outros recursos com hash por um proxy reverso ou CDN; consulte "Cache do Cloudflare".

## Banco de dados

SQLite (padrão), MySQL, MariaDB e PostgreSQL são suportados.

- `base.database_drive`: `sqlite` (padrão) | `mysql` | `postgres`. **MariaDB usa `mysql`.**
- SQLite: `sqlite.file_path` (veja também `read_pool_size`).
- MySQL / MariaDB: `mysql.{host,port,username,password,db_name,wait_timeout}`.
- PostgreSQL: `postgres.{host,port,username,password,db_name,ssl_mode,time_zone,wait_timeout}`.

Os recursos exclusivos do SQLite (WAL, pool de conexões somente leitura e backups de migração por snapshot de arquivo) se aplicam apenas ao SQLite; faça backup de MySQL/MariaDB/PostgreSQL com as próprias ferramentas.

### Migração de SQLite para MySQL/MariaDB/PostgreSQL

```bash
# conf.yaml        — configuração existente do SQLite
# conf.target.yaml — configuração de destino apontando para mysql ou postgres (o banco de dados de destino deve estar vazio)
./yin-panel migrate-db -source conf.yaml -target conf.target.yaml
```

A migração cria o esquema completo no destino, copia todas as linhas **preservando as chaves primárias** (e redefine as sequências de identidade no PostgreSQL). Pode ser executada repetidamente — cada execução substitui o conteúdo das tabelas de destino. Depois, aponte `database_drive` no `conf.yaml` para o destino.

## Configuração e OAuth/OIDC

`oauth` é o ponto de entrada de configuração unificado para OAuth 2.0 e OpenID Connect (OIDC). O modelo padrão inclui três exemplos verificados: GitHub (OAuth 2.0), GitLab (OIDC padrão) e Google (OAuth 2.0). Todos os exemplos vêm comentados por padrão; um provedor só é ativado depois de preencher credenciais reais e definir `oauth.enable: true`. Todas as chaves de configuração estão documentadas em `backend/conf.yaml.example`.

Um provedor OIDC padrão precisa oferecer suporte a discovery, authorization code + PKCE e validação de assinatura JWKS, além de fornecer `email` e `email_verified` nas informações do usuário. Provedores que atendem a esses padrões funcionam em princípio, mas alguns serviços ainda podem exigir configuração extra de mapeamento de campos.

URL de callback do OIDC:

```text
<base.root_url>/api/oauth/<provider>/callback
```

Após o callback, o provedor deve retornar um e-mail verificado. O sistema identifica identidades externas por `provider + sub` e vincula contas locais por e-mail.

## Espaços e permissões

As permissões de espaço são vinculadas ao `UserID` interno, nunca ao texto do e-mail. Por isso, alterar o e-mail mantém as permissões de espaço existentes, e a lista de membros mostra o e-mail mais recente.

- Administrador: gerencia o espaço, membros, papéis e regras OIDC
- Editor: mantém grupos e favoritos
- Visualizador: acesso somente leitura

Criar um espaço, renomear, adicionar grupos, membros e regras OIDC acontecem em caixas de diálogo.

## Helm

```bash
helm upgrade --install yin-panel ./distribution/helm-chart/yin-panel \
  --set image.tag=0.6.0
```

Antes de implantar, visualize os manifestos renderizados com `helm template yin-panel ./distribution/helm-chart/yin-panel`.

## Desenvolvimento a partir do código-fonte

Requisitos: Go `1.27.1`, Node.js `24.21.0`, npm ou pnpm `9.15.5`.

```bash
cd frontend
npm ci
npm run build-only
```

Ou com o pnpm fixado:

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

Atualização local completa (build do frontend, testes e build do backend, implantação, reinício) em um único comando:

```bash
./scripts/deploy-local.sh
```

O script usa `backend/` do repositório como diretório de execução por padrão e exige um `conf.yaml` local nele; `YIN_PANEL_RUNTIME_DIR` em `.env.local` pode substituí-lo. Depois que todos os builds e testes passam, ele para apenas o listener do Yin-Panel, substitui o binário antigo e os arquivos estáticos com `sudo`, e nunca toca no banco de dados, nos uploads ou nos arquivos de configuração.

O sistema de temas da página inicial está documentado em [`frontend/THEME_SYSTEM.md`](frontend/THEME_SYSTEM.md); crie um tema novo com `npm run create:theme -- init "<nome>"`. As convenções completas de desenvolvimento com IA, publicação e reinício estão em [AGENTS.md](./AGENTS.md).

## Backup e atualização

Antes de atualizar, faça backup do banco de dados, de `uploads/`, do `conf.yaml` e dos values do Helm. As migrações preservam usuários, espaços, grupos, favoritos e referências de arquivos. Se algo der errado, restaure o backup anterior à atualização; não exclua o banco de dados para recriá-lo.

## Cache do Cloudflare

Se o domínio usa a nuvem laranja do Cloudflare, adicione Cache Rules para manter o documento de entrada atualizado e armazenar em cache de longo prazo apenas os recursos estáticos com hash.

Ignorar cache: `/`, `/index.html`, `/login`, `/sw.js`, `/registerSW.js`, `/manifest.webmanifest` e `/api/*`.

Cache de longo prazo: `/assets/*` e `/workbox-*.js`. Esses arquivos têm nomes com hash de conteúdo, e a origem os serve com cache `immutable` de 30 dias.

Não ative `Cache Everything` para o site inteiro. Após uma publicação, limpe apenas o documento de entrada e os arquivos do service worker do cache do Cloudflare; os recursos com hash podem permanecer no edge.

## Open Core

A edição Core do Yin-Panel é gratuita e continua recebendo manutenção e atualizações. Este repositório é a única fonte pública do Core e publica a imagem `yin-panel-ce`. A edição Enterprise fica no repositório privado [yinorg/Yin-Panel-EE](https://github.com/yinorg/Yin-Panel-EE) e se conecta por interfaces públicas.

Uma instância em execução expõe edição, capacidades, estado somente leitura e versão em `GET /api/system/capabilities`. Relate problemas e sugestões nas [GitHub Issues](https://github.com/yinorg/Yin-Panel/issues).

## Créditos

O Yin-Panel nasceu do projeto de código aberto [hslr-s/sun-panel](https://github.com/hslr-s/sun-panel); agradecemos ao autor original por construir a base.

Agradecemos também a [PhantomMaa/sun-panel](https://github.com/PhantomMaa/sun-panel) pelas otimizações e pelo trabalho de manutenção. O Yin-Panel assumiu a manutenção continuada a partir dessa base e segue adicionando recursos, melhorias de interação e mudanças voltadas à auto-hospedagem.

Agradecemos a todos os autores originais, contribuidores e usuários.

## Licença

O Yin-Panel é licenciado sob a [Business Source License 1.1](./LICENSE): o uso em produção é gratuito para uso pessoal ou para as operações internas da sua organização. Três anos após a primeira distribuição pública de cada versão, essa versão passa para a licença AGPL-3.0-or-later.
