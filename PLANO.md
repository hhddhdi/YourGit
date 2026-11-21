# YourGit — Plano do produto

> **Nome:** YourGit.
> **Status:** MVP v0.1.0 concluído para Windows — primeira fatia vertical implementada e validada ponta a ponta no app desktop, com instalador NSIS gerado.
> **Data:** 21/11/2026.
>
> **Já funciona:** URL do GitHub → download do zip → extração segura → detecção de Node/Rust → plano revisável → execução nativa com logs em tempo real, histórico, parada e remoção. A porta anunciada pelo servidor nos logs substitui a estimativa do plano (ex.: sirv em 8080). O catálogo tem os canais oficial/comunidade definidos na UI e nos schemas, mas a instalação por catálogo ainda não foi ligada.
>
> **Ainda não implementado:** sandbox Docker (fase 2), edição do plano pela UI, Python/Java/Go, catálogo remoto assinado, login GitHub e cofre de segredos.

---

## 1. Visão

Um aplicativo desktop que transforma qualquer repositório GitHub em um projeto pronto para rodar, com o mínimo de fricção e o máximo de isolamento.

**Pitch:** cole a URL de um repositório — ou escolha um no catálogo — e o YourGit baixa o código, detecta a stack, mostra exatamente o que pretende executar, instala as dependências e sobe o projeto em um container isolado. Logs, portas, variáveis de ambiente, parar, reiniciar, atualizar e remover ficam em um só painel.

### Objetivos mensuráveis

| Objetivo | Meta |
|---|---|
| Instalar e rodar um projeto Node simples | menos de 3 min com Docker pronto; menos de 90 s no modo nativo |
| Remover um projeto | 1 ação, sem containers, volumes, pastas ou caches órfãos do YourGit |
| Funcionar sem conta | repositórios públicos sem login obrigatório |
| Previsibilidade | nenhum comando executa sem um plano visível e aprovado na primeira execução |
| Qualidade da detecção | pelo menos 90% dos 20 repositórios de teste com plano correto ou editável |
| Pegada do app | binário abaixo de ~30 MB, memória em repouso abaixo de 200 MB |

---

## 2. Princípios

1. **Zero surpresa.** O usuário vê os comandos, o diretório de trabalho, as portas e as variáveis antes de executar.
2. **Isolado por padrão.** Código desconhecido roda em container. Rodar no host é uma escolha explícita e avisada.
3. **Tudo removível.** Toda alteração fora da pasta do projeto é rastreável por labels e registrada no banco.
4. **Sem conta obrigatória.** Repos públicos funcionam offline depois de instalados.
5. **Sem lock-in.** O projeto continua executável fora do YourGit; nada de formato proprietário no código.
6. **Falha explicada.** Todo erro mostra o comando, o log, a causa provável e a ação recomendada.
7. **Poucas dependências.** O app não deve exigir Node, Python, Git ou Java no host — o container fornece o runtime.

---

## 3. Escopo

### 3.1 MVP (v0.1)

- Windows 10/11 (x64).
- Adicionar projeto por URL do GitHub (repositório, branch/tag/commit e subpasta).
- Catálogo embutido com **dois canais**: oficial (10–20 entradas) e comunidade (2–5 exemplos), com filtros e selos distintos.
- Download por arquivo `.zip` (sem exigir Git instalado).
- Detecção de stack: Node, Rust, Python, Java e Go.
- Plano de execução editável e aprovação explícita.
- Instalação de dependências e execução em **Docker sandbox**.
- Execução nativa como opção avançada, com aviso de risco.
- Logs em tempo real, health check HTTP/TCP, portas mapeadas e botão “Abrir”.
- Parar, reiniciar, atualizar (rebaixar versão) e remover com limpeza.
- Variáveis de ambiente simples; segredos no cofre do sistema.
- SQLite para estado; configurações de workspace, Docker e catálogo.
- Tema escuro, busca, filtros e atalhos de teclado.
- Instalador NSIS/MSI e atualização assinada.

### 3.2 v0.2

- Repositórios privados (OAuth Device Flow e PAT).
- Catálogo remoto assinado (oficial) + catálogo comunitário versionado + fontes personalizadas.
- Terminal interativo no container (xterm.js + PTY).
- Suporte a `Dockerfile` e `docker-compose.yml` do repositório.
- Seleção de pacote em monorepos.
- Monitor de CPU/RAM/disco por projeto.
- Importar pasta local já existente.

### 3.3 v0.3+

- Dev Containers (`devcontainer.json`).
- Backend WSL 2 nativo (sem Docker Desktop).
- Podman e Docker remoto/SSH.
- Snapshots e restauração de ambiente.
- GitLab, Bitbucket e GitHub Enterprise.
- macOS e Linux.
- Submissão pela UI/PR, moderação e promoção de receitas da comunidade.

### 3.4 Fora de escopo (por enquanto)

- IDE completa ou editor de código avançado.
- Deploy/hospedagem em nuvem.
- Substituir Docker Desktop ou orquestração de produção.
- Executar código de fontes não confiáveis sem confirmação.
- Serviço Windows em background.
- Marketplace com pagamentos, avaliações e reputação (sem backend no MVP).

---

## 4. Stack técnica

| Camada | Escolha | Por quê | Alternativa |
|---|---|---|---|
| Shell desktop | **Tauri 2** | Binário leve, backend Rust, WebView2 já presente no Windows | Electron (pesado), Winit puro (mais trabalho) |
| Backend | **Rust** (stable) | Segurança de memória, processos, concorrência, ecossistema de CLI | Go |
| Frontend | **Svelte 5 + Vite + TypeScript** | Componentes em HTML/JS, reatividade sem runtime grande, ótimo custo-benefício | Vanilla JS + Web Components; React |
| Estilo | CSS próprio com design tokens | Controle visual e zero dependência de UI kit | Tailwind + shadcn-svelte |
| Terminal/logs | **xterm.js** + `portable-pty` | Terminal real dentro do container | Logs somente texto no MVP |
| Banco | **SQLite** via `rusqlite` (bundled) | Local, transacional, sem servidor; migrations versionadas | `sqlx`, arquivo JSON |
| HTTP | `reqwest` (rustls) | Downloads, GitHub API, health checks | `ureq` |
| Download | `zip` + `tar` + validação de caminhos | GitHub entrega arquivo; dispensa Git | `git2`, Git CLI |
| Docker | **CLI `docker` / `docker compose`** | Comandos visíveis, compose pronto, menos código | `bollard` (API tipada) |
| Processos | `tokio::process` + Job Objects (Windows) | Matar árvore de processos sem órfãos | `sysinfo` kill |
| Segredos | `keyring` (Credential Manager) | Token nunca no SQLite/log | Arquivo criptografado |
| Logs | `tracing` + arquivo rotativo | Diagnóstico local | `log` |
| Updates | `tauri-plugin-updater` assinado | Atualização segura | Download manual |
| Testes | `cargo test`, Vitest, Playwright | Cobertura de núcleo e UI | — |
| CI/CD | GitHub Actions (Windows + Ubuntu) | Build e testes automatizados | — |
| Distribuição | NSIS/MSI + zip portátil | Instalação simples | MSIX |

### 4.1 Estado do ambiente local (verificado)

| Item | Status |
|---|---|
| Rust / Cargo 1.99 | ✅ instalado |
| Node 24 / npm | ✅ instalado; `npm.ps1` bloqueado por Execution Policy — usar `npm.cmd` ou ajustar a policy |
| Docker Client 29.9 | ⚠️ instalado, mas o daemon não está rodando |
| WSL 2 com Ubuntu | ✅ configurado |
| WebView2 | ✅ instalado |
| Visual Studio 2022 Build Tools | ✅ instalado (necessário para Tauri/MSVC) |

**Consequência:** o desenvolvimento pode começar hoje; o modo Docker exige iniciar o Docker Desktop (ou configurar Docker Engine no WSL) antes dos testes de integração.

### 4.2 Por que não as alternativas óbvias

- **Electron:** entrega mais rápido em JS, mas custa ~100 MB+ e memória alta; o Tauri aproveita o WebView2 e mantém o núcleo em Rust.
- **CLI puro:** mais simples, mas o produto é sobre facilidade visual (instalar, ver logs, abrir porta).
- **Só Dev Containers:** depende de repos terem `devcontainer.json`; a maioria não tem. O sandbox gerado cobre o caso geral.
- **Nix:** excelente isolamento, mas péssimo suporte nativo no Windows e curva de aprendizado alta.
- **Máquinas virtuais:** isolamento forte, mas lentas e complexas para o MVP; ficam para o futuro.

---

## 5. Arquitetura

```text
┌─────────────────────────────────────────────────────────────┐
│  UI (Svelte + TypeScript, WebView2)                         │
│  Library · Marketplace · Project · Logs · Settings          │
└──────────────────────────┬──────────────────────────────────┘
                           │ Tauri IPC (comandos + channels)
┌──────────────────────────▼──────────────────────────────────┐
│  Aplicação (Rust)                                           │
│  Catalog · Installer · Inspector · Planner · Supervisor     │
│  PortAllocator · Cleaner · Secrets · Jobs · Audit           │
├─────────────────────────────────────────────────────────────┤
│  Domínio                                                    │
│  Project · Source · RunPlan · Recipe · RunState · Stack     │
├───────────────┬───────────────────┬─────────────────────────┤
│  Infra        │  Runners          │  Persistência           │
│  GitHub API   │  NativeRunner     │  SQLite (rusqlite)      │
│  codeload     │  DockerRunner     │  Arquivos (workspace)   │
│  Archive      │  ComposeRunner*   │  Keyring                │
│  Docker CLI   │  (Devcontainer*)  │  Logs                   │
└───────────────┴───────────────────┴─────────────────────────┘
        │                  │
        ▼                  ▼
   GitHub / CDN       Docker Engine / processos do host
```

`*` fora do MVP, mas previsto na interface `Runner`.

### 5.1 Estrutura do repositório

```text
yourgit/
├─ Cargo.toml                  # workspace Rust
├─ crates/
│  └─ yourgit-core/            # domínio + serviços, sem dependência de Tauri
│     ├─ src/
│     │  ├─ domain/            # Project, RunPlan, Recipe, RunState
│     │  ├─ sources/           # parser de URL GitHub, download
│     │  ├─ archive/           # extração segura (zip-slip, symlink)
│     │  ├─ inspect/           # detecção de stack
│     │  ├─ plan/              # geração/validação de RunPlan
│     │  ├─ runners/           # trait Runner + native/docker
│     │  ├─ docker/            # CLI, labels, compose, portas
│     │  ├─ store/             # SQLite, migrations, repositórios
│     │  ├─ secrets/           # keyring
│     │  ├─ jobs/              # fila, progresso, cancelamento
│     │  └─ audit/             # trilha de ações
│     └─ tests/
├─ apps/
│  └─ desktop/
│     ├─ src/                  # frontend Svelte
│     │  ├─ lib/               # ipc, stores, router, formatação
│     │  ├─ components/
│     │  └─ screens/
│     ├─ src-tauri/            # comandos Tauri, capabilities, updater
│     ├─ index.html
│     └─ vite.config.ts
├─ catalog/                    # fonte do catálogo (pode virar repo próprio)
│  ├─ index.json
│  ├─ index.json.sig
│  └─ entries/
├─ docs/
│  ├─ PLANO.md
│  ├─ ADRs/
│  └─ examples/
└─ scripts/
```

**Decisão:** o núcleo fica em uma crate separada de Tauri. Isso permite testar tudo sem UI e, no futuro, expor uma CLI (`yourgit install <url>`) ou outro cliente.

### 5.2 Responsabilidades

- **`src-tauri`:** adaptador fino. Valida entradas, chama serviços, emite eventos. Não contém regra de negócio.
- **`yourgit-core`:** toda a lógica testável: download, detecção, plano, execução, banco, limpeza.
- **Frontend:** apresentação e estado de UI. Nunca decide comando a executar; apenas exibe e envia o plano aprovado.

---

## 6. Fluxos e estados

### 6.1 Fluxo principal: instalar por URL

1. Usuário cola `https://github.com/owner/repo` (ou com `/tree/<ref>/<subdir>`).
2. App valida e normaliza a URL; consulta metadados públicos (nome, branch padrão, licença, estrelas).
3. App baixa o arquivo para `cache/downloads/<id>.zip.part`, com limite de tamanho e timeout.
4. Extração segura para pasta temporária; validação de zip-slip, symlinks e tamanho descompactado.
5. Inspeção estática: manifests, lockfiles, scripts, Dockerfile/compose, `.env.example`.
6. App propõe um **RunPlan** com confiança, evidências e avisos. Nada foi executado ainda.
7. Usuário revisa/edita e aprova. O plano é salvo com hash.
8. Download concluído é movido atomicamente para `<workspace>/<owner>/<repo>`.
9. Container (ou processo nativo) é preparado e as dependências são instaladas.
10. App inicia, acompanha logs e health check, descobre a porta e mostra “Abrir”.
11. Projeto entra na biblioteca com estado `running`.

### 6.2 Máquina de estados

**Instalação (por projeto):**

```text
new → downloading → verifying → extracting → inspecting
    → awaiting_approval → installing → ready
    → error (a partir de qualquer etapa)
    → removing → removed
```

**Execução (por run):**

```text
preparing → starting → running → stopping → stopped
                └────→ failed
running → unhealthy (health check falhou; ainda pode estar de pé)
```

O estado exibido na biblioteca é derivado do último run + estado de instalação. O banco guarda um cache do estado; na inicialização o app reconcilia com containers e processos reais.

### 6.3 Fluxos secundários

- **Catálogo:** buscar → detalhe → revisar receita → instalar (mesmo pipeline, com receita pré-preenchida).
- **Gerenciar:** parar, reiniciar, abrir URL, copiar logs, atualizar commit, remover.
- **Falhou:** ver erro estruturado, comando, últimas 200 linhas e “copiar diagnóstico”.
- **Limpeza:** remover projeto → escolher manter ou apagar pasta; sempre remover container, volumes e rede do YourGit.
- **Sem Docker:** detectar, explicar e oferecer modo nativo (com aviso) ou instruções para instalar/ligar o Docker.

---

## 7. Modelo de dados

### 7.1 Princípios

- SQLite em WAL, `foreign_keys=ON`, migrations embutidas e versionadas.
- Segredos **nunca** no banco: apenas a chave de referência no keyring.
- Logs completos em arquivo; banco guarda metadados e caminho.
- Timestamps em UTC (ISO 8601, texto).
- Backup automático do banco antes de migrations destrutivas.

### 7.2 Tabelas principais

```sql
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE projects (
  id             TEXT PRIMARY KEY,              -- UUID
  name           TEXT NOT NULL,
  owner          TEXT,
  repo           TEXT,
  source_kind    TEXT NOT NULL,                 -- github | catalog | local
  source_url     TEXT NOT NULL,
  requested_ref  TEXT,
  resolved_commit TEXT,
  subdir         TEXT NOT NULL DEFAULT '.',
  local_path     TEXT NOT NULL UNIQUE,
  install_state  TEXT NOT NULL,                 -- new | downloading | ... | ready | error
  isolation      TEXT NOT NULL DEFAULT 'docker',
  pinned         INTEGER NOT NULL DEFAULT 1,    -- impede update automático
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);

CREATE TABLE run_plans (
  id           TEXT PRIMARY KEY,
  project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  version      INTEGER NOT NULL,
  plan_json    TEXT NOT NULL,
  plan_hash    TEXT NOT NULL,
  approved_at  TEXT,
  created_at   TEXT NOT NULL
);

CREATE TABLE runs (
  id             TEXT PRIMARY KEY,
  project_id     TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  plan_id        TEXT REFERENCES run_plans(id),
  mode           TEXT NOT NULL,                 -- docker | native | compose
  state          TEXT NOT NULL,
  container_id   TEXT,
  host_ports_json TEXT,                         -- {"5173": 49152}
  started_at     TEXT,
  ended_at       TEXT,
  exit_code      INTEGER,
  error_code     TEXT,
  log_path       TEXT
);

CREATE TABLE env_vars (
  id          TEXT PRIMARY KEY,
  project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  key         TEXT NOT NULL,
  value       TEXT,                             -- apenas se não for segredo
  secret_ref  TEXT,                             -- chave no keyring se for segredo
  is_secret   INTEGER NOT NULL DEFAULT 0,
  UNIQUE(project_id, key)
);

CREATE TABLE catalog_sources (
  id           TEXT PRIMARY KEY,
  url          TEXT NOT NULL UNIQUE,
  kind         TEXT NOT NULL,                   -- official | custom
  trusted      INTEGER NOT NULL DEFAULT 0,
  etag         TEXT,
  fetched_at   TEXT,
  signature_ok INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE audit_events (
  id         TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  project_id TEXT,
  action     TEXT NOT NULL,                     -- install | run | stop | remove | ...
  detail_json TEXT NOT NULL                     -- nunca contém segredos
);

CREATE INDEX idx_projects_state ON projects(install_state);
CREATE INDEX idx_runs_project   ON runs(project_id, started_at DESC);
CREATE INDEX idx_audit_project  ON audit_events(project_id, created_at DESC);
```

### 7.3 Armazenamento em disco

```text
%APPDATA%\YourGit\
├─ yourgit.db
├─ config.json                  # preferências simples
├─ logs\
│  └─ <project-id>\<run-id>.log
└─ backups\

%LOCALAPPDATA%\YourGit\
├─ cache\downloads\
├─ cache\catalog\
├─ builds\                      # Dockerfiles gerados, contexto de build
└─ diagnostics\

<workspace>\                    # configurável; padrão %USERPROFILE%\YourGit\projects
└─ <owner>\<repo>\
```

**Regra:** apagar um projeto remove o que o YourGit criou. A pasta de código só é apagada com confirmação explícita; por padrão, oferecemos “mover para a lixeira”.

### 7.4 Retenção

- Logs de runs: 30 dias ou 50 runs por projeto (configurável).
- Downloads: removidos após extração bem-sucedida; mantidos se o usuário marcar “guardar cópia”.
- Cache de catálogo: 24 h.
- Backups do banco: 5 mais recentes.

---

## 8. Detecção de stack e plano de execução

### 8.1 Estratégia

A inspeção é **estática**. O app lê arquivos; não executa nada do repositório antes da aprovação.

1. Verificar `Dockerfile`, `compose.yaml`/`docker-compose.yml` e `.devcontainer/devcontainer.json`.
2. Ler manifests na raiz e na subpasta escolhida.
3. Identificar gerenciador de pacotes pelo lockfile.
4. Extrair scripts e comandos candidatos.
5. Detectar framework e porta provável.
6. Ler `.env.example`/`.env.sample` para montar o formulário de variáveis.
7. Produzir um `RunPlan` com confiança, evidências e avisos.
8. Nunca escolher comando destrutivo; nunca usar `shell=True` com string vinda do repo.

### 8.2 Matriz inicial

| Stack | Sinais | Instalação | Execução | Porta comum |
|---|---|---|---|---|
| Node | `package.json` + lockfile | `npm ci`, `pnpm install --frozen-lockfile`, `yarn install --immutable`, `bun install` | script `dev`/`start`/`serve`, senão `node <main>` | 3000, 5173, 8080 |
| Rust | `Cargo.toml` | `cargo fetch` (e `cargo build`) | `cargo run` | 8080 |
| Python | `requirements.txt`, `pyproject.toml` | `python -m venv` + `pip install -r` ou `uv sync` | Flask/Django/FastAPI detectados; `python -m ...` | 8000, 5000 |
| Java | `pom.xml`, `build.gradle(.kts)` | `./mvnw dependency:go-offline` ou `./gradlew dependencies` | `./mvnw spring-boot:run`, `./gradlew bootRun` | 8080 |
| Go | `go.mod` | `go mod download` | `go run .` | 8080 |
| PHP | `composer.json` | `composer install` | `php -S 0.0.0.0:8000` ou framework | 8000 |
| Ruby | `Gemfile` | `bundle install` | `bundle exec rails server -b 0.0.0.0` | 3000 |

PHP e Ruby entram na v0.2; a arquitetura já os acomoda.

### 8.3 Regras importantes

- **Priorizar wrappers do repo:** `mvnw`, `gradlew`, scripts existentes.
- **Lockfile manda:** se há `package-lock.json`, usar `npm ci`; se há `pnpm-lock.yaml`, `pnpm`; etc.
- **Monorepos:** detectar `workspaces`, `pnpm-workspace.yaml`, `turbo.json`, `nx.json`, workspace Cargo. No MVP, usar a raiz ou a subpasta da URL; na v0.2, permitir escolher o pacote.
- **Scripts de ciclo de vida:** `postinstall`/`prepare` executam código. Avisar sempre; em Docker, seguir; no host, exigir confirmação extra.
- **Porta:** detectar configuração de framework, scripts (`--port`) e servidores conhecidos (Vite 5173, Next/Nuxt/CRA 3000, Astro 4321, Angular 4200, sirv/http-server 8080, Parcel 1234); se incerto, usar a porta padrão e permitir edição. Quando o servidor anuncia `http://localhost:<porta>` nos logs, a UI passa a usar a porta real (cobre conflitos de porta e configurações não detectadas).
- **Variáveis:** apenas nomes e descrições; valores nunca vêm do repositório. Segredos vão para o keyring.
- **Plano versionado:** se arquivos relevantes mudarem (lockfile, manifests), o app marca “plano desatualizado” e pede nova aprovação.
- **Comandos como `argv`:** `[["npm","ci"]]`, nunca uma string de shell. Isso elimina injeção e deixa a execução auditável.

### 8.4 Exemplo de RunPlan

O contrato completo está em [`docs/run-plan.example.json`](./docs/run-plan.example.json). Campos principais:

```json
{
  "schema_version": 1,
  "stack": { "language": "node", "package_manager": "npm", "confidence": 0.94 },
  "isolation": { "mode": "docker", "image": "node:24-bookworm-slim" },
  "install": [["npm", "ci"]],
  "start": [["npm", "run", "dev", "--", "--host", "0.0.0.0"]],
  "ports": [{ "container": 5173, "protocol": "http", "health_path": "/" }],
  "env": [{ "key": "VITE_API_URL", "required": false, "secret": false }],
  "warnings": ["Executa scripts de pós-instalação do npm."],
  "plan_hash": "sha256:..."
}
```

### 8.5 Casos difíceis (e posição do MVP)

| Caso | MVP | Evolução |
|---|---|---|
| Monorepo com 20 apps | usa raiz/subpasta da URL | seletor de pacote |
| Precisa de Postgres/Redis | não suportado; aviso claro | serviços gerenciados/compose |
| Precisa de variáveis obrigatórias | formulário + bloqueio até preencher | templates por framework |
| Build demorado | timeout configurável e logs | cache de camadas/build |
| Comando de start não óbvio | usuário edita o plano | heurísticas + receitas da comunidade |
| Repo só com Dockerfile | v0.2 | build e run automáticos |
| Hot reload | funciona no sandbox com bind mount | tuning de filesystem no Windows |

---

## 9. Execução e isolamento

### 9.1 Modos

| Modo | Quando usar | Isolamento | Padrão |
|---|---|---|---|
| **Docker sandbox** | repo comum, sem Dockerfile | container Linux, rede própria, limites | ✅ recomendado |
| **Dockerfile do repo** | repo traz Dockerfile | imagem construída pelo repo | v0.2 |
| **Compose do repo** | repo traz compose | múltiplos serviços | v0.2 |
| **Nativo** | usuário quer performance/IDE do host | nenhum (processo do usuário) | só com aviso |
| **Dev Container** | repo traz devcontainer | container de desenvolvimento | v0.3 |
| **WSL** | ferramentas Linux sem Docker | distro WSL | v0.3 |

### 9.2 Docker sandbox (MVP)

Para não exigir Dockerfile, o YourGit usa uma imagem base por stack, monta o projeto e executa os comandos dentro do container.

```text
docker run -d --init \
  --name yourgit-<slug>-<short-id> \
  --label com.yourgit.managed=true \
  --label com.yourgit.project=<project-id> \
  --workdir /workspace \
  --mount type=bind,source=<projeto>,target=/workspace \
  --mount type=volume,source=yourgit-cache-<id>,target=/home/dev/.cache \
  --memory 4g --cpus 2 --pids-limit 512 \
  --cap-drop ALL --security-opt no-new-privileges \
  --network yourgit-net-<id> \
  -p 127.0.0.1:<porta-host>:<porta-container> \
  <imagem-base> sleep infinity
```

Depois, instalação e start rodam via `docker exec` (ou um shell interativo no terminal, na v0.2). O container permanece de pé para permitir reinício rápido, terminal e hot reload.

**Imagens base iniciais** (fixadas por digest no release e atualizadas com testes):

| Stack | Imagem |
|---|---|
| Node | `node:24-bookworm-slim` |
| Rust | `rust:1.99-bookworm` |
| Python | `python:3.13-slim` |
| Java | `eclipse-temurin:25-jdk` |
| Go | `golang:1.25-bookworm` |

Detalhes:

- **Usuário:** não-root quando a imagem permitir. No Windows, validar permissões do bind mount; se necessário, criar usuário de desenvolvimento com UID consistente.
- **Cache:** volume nomeado para npm/pnpm/yarn, cargo, pip/uv, maven/gradle e go. Acelera reinstalações.
- **Rede:** rede bridge dedicada por projeto; internet liberada para instalar dependências; opção “sem rede” na v0.3.
- **Portas:** bind em `127.0.0.1` por padrão; opção de expor na LAN.
- **Limites:** memória, CPU e PIDs configuráveis por projeto.
- **Sem Docker socket, sem home do host, sem chaves SSH.** Apenas a pasta do projeto e caches do YourGit são montados.
- **Labels:** todo recurso criado leva `com.yourgit.managed=true` e o ID do projeto. Isso permite limpeza e reconciliação.
- **Ciclo de vida:** `stop` = `docker stop -t 10`; `remove` = stop + `docker rm -v` + remoção de rede/volumes do projeto.
- **Reconciliação:** ao abrir o app, listar containers com a label e comparar com o banco; containers órfãos são oferecidos para remoção.

**Por que não construir uma imagem por projeto no MVP?** Buildar é mais reprodutível, mas lento e exige Dockerfile válido. O sandbox com bind mount entrega o “rodar em minutos” e cobre a maioria dos casos. A opção de build entra na v0.2.

### 9.3 Execução nativa

- Windows: criar processo com `CREATE_NEW_PROCESS_GROUP` + **Job Object** para matar a árvore inteira ao parar.
- Unix (futuro): `setsid` e sinal para o grupo.
- Captura de stdout/stderr, timeout e health check iguais aos do Docker.
- Aviso claro: o código roda com as permissões do usuário e pode acessar arquivos e rede.
- Recomendado apenas para projetos confiáveis ou quando Docker não está disponível.

### 9.4 Portas

- Alocador escolhe uma porta livre no host para cada porta esperada do container.
- Mapeamento fica explícito na UI: `localhost:49152 → 5173`.
- Conflitos: detectar antes de subir; sugerir outra porta.
- Portas reservadas são liberadas quando o run para.
- Health check: HTTP GET no path configurado, TCP connect ou regex de “ready” nos logs; timeout de 60 s (configurável).

### 9.5 Logs

- stdout e stderr separados por origem, com timestamp.
- Buffer em memória (5.000 linhas) para a UI + arquivo completo.
- Envio à UI em lotes (ex.: a cada 100 ms) para não saturar o IPC.
- Preservar ANSI no terminal; escapar HTML na renderização.
- Redigir padrões de token/segredo antes de gravar e antes de enviar.
- Botão “copiar”, “abrir arquivo” e “copiar diagnóstico”.

### 9.6 Compose (v0.2)

- Usar `docker compose -p yourgit_<id> ...`.
- Antes de executar, escanear o arquivo e avisar/bloquear:
  - `privileged: true`
  - `network_mode: host`
  - montagem de `/var/run/docker.sock`
  - montagens fora da pasta do projeto
  - `cap_add` perigosas
- `down` remove containers e rede; volumes só com escolha explícita.
- Portas lidas de `docker compose ps --format json`.

---

## 10. Marketplace / catálogo

### 10.1 Conceito e canais

Um **catálogo** de repositórios que funcionam bem no YourGit. Não é uma loja: é uma lista de receitas com metadados e comandos. O produto terá **dois canais separados e visíveis**:

| Canal | Conteúdo | Curadoria | Assinatura | Selo |
|---|---|---|---|---|
| **Oficial** | receitas revisadas pela equipe | obrigatória, commit fixado | Ed25519 | “Oficial” |
| **Comunidade** | receitas enviadas por usuários | revisão leve + lint automático | opcional | “Comunidade” |

Regras dos dois canais:

- O canal oficial é a fonte padrão e vem embutido no app para funcionar offline.
- O canal comunitário é opt-in, separado visualmente e **nunca** é misturado ao oficial sem indicação clara.
- Receitas comunitárias exibem aviso de risco e o comando exato antes de executar.
- Uma receita comunitária pode ser promovida a oficial após revisão completa.
- Fontes personalizadas (URLs de catálogo de terceiros) continuam existindo como opção avançada, com selo “Não verificado”.

- **MVP:** canal oficial embutido (10–20 entradas) + canal comunitário embutido com 2–5 exemplos de demonstração.
- **v0.2:** atualização remota assinada do canal oficial e atualização do canal comunitário via JSON versionado.
- **v0.3:** submissão pela UI/PR e moderação com histórico de versões.

### 10.2 Formato

Exemplo em [`docs/catalog.example.json`](./docs/catalog.example.json).

Campos de uma entrada:

| Campo | Descrição |
|---|---|
| `id` | identificador estável |
| `channel` | `official` ou `community` |
| `name`, `description` | apresentação |
| `repo` | URL GitHub |
| `ref` | **commit SHA fixado** (nunca “main” solto) |
| `subdir` | pasta do app em monorepos |
| `category`, `tags` | busca e filtros |
| `license` | validada na curadoria |
| `icon` | URL ou asset embutido |
| `verified` | revisado pela equipe |
| `requirements` | Docker, memória, disco |
| `recipe` | install/start/ports/env/warnings |

### 10.3 Segurança do catálogo

- Catálogo oficial versionado em repositório próprio, publicado como JSON estático (GitHub Pages/CDN).
- Assinatura **Ed25519**: `index.json` + `index.json.sig`; a chave pública fica embutida no app.
- App valida a assinatura antes de aceitar atualizações; falha = mantém snapshot anterior.
- O catálogo comunitário usa o mesmo schema, mas exige confirmação explícita para habilitar a fonte e mostra o selo “Comunidade” em cada entrada.
- Commits fixados; atualização de entrada é uma mudança revisada, não automática.
- Lint de receitas no CI: proíbe `sudo`, `curl | sh`, caminhos absolutos do host, uso de Docker socket, comandos destrutivos e variáveis de ambiente do host.
- Fontes personalizadas exigem confirmação de confiança e recebem selo “Não verificado”; nunca são executadas sem revisão do plano.
- A UI mostra o comando exato e os avisos antes de instalar.

### 10.4 Operação

1. Equipe mantém `catalog/official/` com entradas assinadas e um teste automatizado que instala e sobe cada uma em CI (quando viável).
2. Merge no oficial gera novo `index.json` assinado; merge no comunitário gera um snapshot separado.
3. App baixa com `If-None-Match`, valida assinatura (oficial) e atualiza o cache local.
4. Entradas instaladas guardam o commit; “atualizar” compara com o catálogo e mostra o diff de versão.
5. Métricas de estrelas do GitHub são cacheadas, não atualizadas a cada abertura.

### 10.5 Moderação

- **Oficial:** checklist completo (licença, atividade do repo, ausência de malware óbvio, receita testada, tamanho razoável).
- **Comunidade:** lint automático, verificação de URL e commit, aviso de risco; revisão manual antes de qualquer promoção ao oficial.
- Selos: “Oficial”, “Verificado”, “Comunidade”.
- Remoção de entrada não desinstala projetos já instalados; apenas impede novas instalações.

---

## 11. Integração com GitHub

### 11.1 Repositórios públicos (MVP)

- Parser de URLs:
  - `https://github.com/owner/repo`
  - `https://github.com/owner/repo.git`
  - `git@github.com:owner/repo.git`
  - `https://github.com/owner/repo/tree/<ref>/<subdir>`
  - `https://github.com/owner/repo/releases/tag/<tag>`
- Resolver `ref` para commit via API pública; cachear metadados com ETag.
- Download do arquivo:
  - `https://codeload.github.com/<owner>/<repo>/zip/<sha>` para público.
  - `https://api.github.com/repos/<owner>/<repo>/zipball/<ref>` para privado (v0.2), com `Authorization`.
- Sem API quando possível: o codeload não consome a cota de 60 req/h da API.
- User-Agent identificável e respeito aos termos do GitHub.

### 11.2 Privados (v0.2)

- **OAuth Device Flow** com um OAuth App do YourGit; sem client secret embutido.
- Escopo mínimo: `repo` (leitura) e `read:user`.
- Token no Windows Credential Manager via `keyring`; nunca no SQLite, nunca em logs.
- Fallback: PAT colado pelo usuário.
- Logout apaga o token local; opção de revogar no GitHub.
- Git CLI (quando necessário) usa `GIT_ASKPASS` temporário; jamais token na URL ou no histórico de comandos.

### 11.3 Atualizações

- Comparar commit instalado com o remoto.
- Atualização por novo arquivo: baixar em pasta temporária, validar, trocar com backup; preservar dados do app (planos, env, logs) fora do código.
- Alterações locais no código: detectar (hash dos arquivos ou Git, se disponível) e avisar antes de sobrescrever.
- Atualização automática desligada por padrão; notificação na UI.
- Git LFS e submódulos não são suportados no modo arquivo; oferecer “clonar com Git” (v0.2).

### 11.4 Limitações conhecidas

- GitHub API sem token: 60 req/h. Mitigar com cache e codeload.
- Repositórios enormes: limite de download (ex.: 1 GB) e aviso.
- Repos com arquivos > 100 MB via LFS falham no zip; explicar e sugerir Git.
- GitHub Enterprise: v0.3.

---

## 12. UI/UX

### 12.1 Navegação

```text
┌──────────────────────────────────────────────────────────────┐
│ YourGit        [ Buscar…  Ctrl+K ]             [ + Adicionar ]│
├──────────────┬───────────────────────────────────────────────┤
│ Biblioteca   │  Todos · Rodando · Parados · Erros · Favoritos│
│ Catálogo     │                                               │
│ Atividade    │  ┌─────────────┐  ┌─────────────┐             │
│ Config.      │  │ hello-vite  │  │ api-rust    │             │
│              │  │ ● Rodando   │  │ ○ Parado    │             │
│              │  │ :49152      │  │ :49153      │             │
│              │  └─────────────┘  └─────────────┘             │
└──────────────┴───────────────────────────────────────────────┘
```

### 12.2 Telas

1. **Onboarding:** workspace, verificação do Docker, GitHub opcional. Três passos, sem bloqueio.
2. **Biblioteca:** cards/lista, status, stack, porta, ações rápidas (rodar, parar, abrir), filtros e busca.
3. **Adicionar projeto:** URL ou pasta; preview do repo; inspeção; plano; confirmação.
4. **Detalhe do projeto:**
   - Visão geral: origem, commit, stack, licença, uso de recursos.
   - Execução: plano, botões, portas, health, uptime.
   - Logs: stream, filtro, busca, download.
   - Terminal (v0.2).
   - Ambiente: variáveis, segredos, arquivos `.env`.
   - Configurações: isolamento, limites, atualização, zona de perigo.
5. **Catálogo:** busca, categorias, cards, detalhe com receita e requisitos.
6. **Atividade:** downloads, instalações, builds, erros recentes.
7. **Configurações:** geral, execução, Docker, GitHub, catálogo, dados/diagnóstico, sobre.
8. **Command palette:** Ctrl+K para buscar projetos, rodar, parar, abrir pasta.

### 12.3 Diretrizes

- Tema escuro por padrão; claro opcional.
- Status com cor + ícone + texto (nunca só cor).
- Ações destrutivas com confirmação proporcional; remoção total exige digitar o nome.
- Erros em linguagem humana, com detalhes técnicos expansíveis.
- Toasts para ações rápidas; notificação do sistema ao terminar instalações longas.
- Atalhos: `Ctrl+K` busca, `Ctrl+N` adicionar, `Ctrl+Enter` rodar/parar.
- Acessibilidade: foco visível, navegação por teclado, contraste AA.
- Sem modais para logs; painel lateral ou aba.
- Layout responsivo até 1024 px; abaixo disso, modo compacto.

### 12.4 Empty states

- Sem projetos: “Cole uma URL do GitHub ou explore o catálogo.”
- Sem Docker: “Docker não está rodando. [Tentar novamente] [Como instalar] [Rodar nativo].”
- Catálogo offline: “Mostrando o catálogo embutido; tentaremos atualizar depois.”

---

## 13. IPC e API interna

### 13.1 Comandos Tauri

```text
app_info() -> AppInfo
settings_get() -> Settings
settings_set(patch) -> Settings

github_inspect(url) -> RepoPreview
github_auth_start() -> DeviceFlow
github_auth_poll() -> GithubUser
github_logout()

catalog_list(refresh: bool) -> Catalog
catalog_install(entry_id) -> InstallJob

project_inspect(source) -> Inspection
project_install(inspection_id, plan) -> Project
project_list(filter) -> Project[]
project_get(id) -> ProjectDetail
project_update(id) -> UpdatePreview
project_remove(id, options) -> Job

run_plan_generate(project_id) -> RunPlan
run_plan_save(project_id, plan) -> RunPlan
run_start(project_id) -> Run
run_stop(project_id) -> Run
run_restart(project_id) -> Run
run_status(project_id) -> RunStatus
runs_list(project_id) -> Run[]

logs_tail(project_id, run_id, lines) -> LogPage
logs_subscribe(project_id, run_id, channel: Channel<LogEvent>)

terminal_open(project_id, channel) -> TerminalId
terminal_write(terminal_id, data)
terminal_resize(terminal_id, cols, rows)
terminal_close(terminal_id)

docker_status() -> DockerStatus
ports_list() -> PortMapping[]
env_get(project_id) -> EnvVar[]
env_set(project_id, vars) -> EnvVar[]
secrets_set(project_id, key, value)
secrets_delete(project_id, key)

job_cancel(job_id)
diagnostics_export() -> path
open_url(url)  # valida http/https e localhost
```

### 13.2 Eventos

- `job:progress` — download, extração, instalação, build.
- `run:status` — mudanças de estado do run.
- `run:log` — lotes de linhas.
- `project:changed` — biblioteca atualizada.
- `catalog:updated`.
- `notification` — mensagens para o SO.

### 13.3 Contratos e erros

```ts
type AppError = {
  code: string;          // YOURGIT-1001
  title: string;
  message: string;       // linguagem humana
  details?: string;      // comando, stderr, stack
  hint?: string;         // o que fazer
  retryable: boolean;
};
```

- Todo comando retorna `Result<T, AppError>` (nunca `panic` no IPC).
- `tauri::ipc::Channel<T>` para streams; backpressure e fechamento explícito.
- Validação de entrada no backend: paths canônicos, URLs permitidas, IDs existentes.
- Nenhum comando aceita uma string de shell arbitrária vinda da UI.

### 13.4 Segurança do Tauri

- CSP restritiva; sem carregar conteúdo remoto na WebView.
- Capabilities mínimas, apenas a janela principal; sem plugin `shell` aberto.
- Dialogs e opener com escopo; URLs `http/https` apenas.
- IPC commands explicitamente registrados.
- Atualizações assinadas; chaves privadas fora do repositório.

---

## 14. Segurança

### 14.1 Modelo de ameaças

O ativo principal é a máquina do usuário. O risco central é **executar código de terceiros**. O app não pode prometer sandbox perfeito, mas deve reduzir drasticamente o impacto e deixar o risco explícito.

| Ameaça | Vetor | Mitigação |
|---|---|---|
| Código malicioso no repo | scripts de install/start | Docker por padrão, non-root, cap-drop, sem socket, limites, aviso no modo nativo |
| Receita maliciosa no catálogo | comandos no JSON | catálogo assinado, commits fixados, lint, revisão, exibição do comando |
| Zip slip / path traversal | nomes de arquivos no zip | validação de caminho canônico, rejeição de `..`, absolutos, symlinks perigosos, limite de tamanho |
| Injeção de comando | nome de projeto, args, env | `argv` sem shell, validação, sem interpolação |
| Vazamento de segredos | logs, env do host | keyring, env explícito, redaction, sem herdar ambiente inteiro |
| Escape do container | privileged, socket, caps | proibir por padrão; escanear compose; seccomp padrão; no-new-privileges |
| Supply chain | dependências e imagens | lockfiles, imagens por digest, aviso de postinstall, auditoria futura |
| Sequestro de update | binário/JSON adulterado | updater assinado, assinatura Ed25519 no catálogo |
| XSS na UI | README/logs | escapar HTML, sanitizar Markdown, CSP |
| Exfiltração via rede | app malicioso | rede isolada por projeto; opção offline; sem acesso a outros containers |

### 14.2 Regras de ouro

1. Repositório público desconhecido → Docker por padrão.
2. Rodar no host → aviso textual + confirmação explícita.
3. Nenhum segredo do host é herdado automaticamente.
4. Nenhum caminho fora do workspace é montado sem confirmação.
5. Nenhum comando de catálogo roda sem ser exibido.
6. Remoção de dados exige confirmação; remoção total exige digitar o nome.
7. Logs e diagnósticos passam por redaction antes de sair do app.

### 14.3 Hardening do container (MVP)

- `--cap-drop ALL`
- `--security-opt no-new-privileges`
- `--pids-limit 512`
- `--memory` e `--cpus`
- sem `--privileged`
- sem `/var/run/docker.sock`
- sem montar `~`, `.ssh`, `.aws`, etc.
- rede dedicada; portas publicadas em loopback
- imagem base oficial, fixada por digest
- usuário não-root quando possível

### 14.4 Dados e privacidade

- Sem telemetria por padrão; opt-in claro e anônimo.
- Nenhum código do usuário sai da máquina, exceto downloads do GitHub.
- Catálogo e metadados públicos são cacheados localmente.
- Diagnóstico exportado manualmente, com redaction e revisão antes de enviar.

### 14.5 Divulgação

- `SECURITY.md` com canal privado.
- Processo de resposta a vulnerabilidades e política de suporte.

---

## 15. Confiabilidade e tratamento de erros

- **Atomicidade:** download em `.part`; extração em pasta temporária; rename atômico ao final.
- **Idempotência:** repetir install/start não deve duplicar containers, redes ou registros.
- **Locks:** uma instância do app por usuário; lock de projeto para evitar duas operações simultâneas.
- **Banco:** WAL, migrations transacionais, backup antes de atualizar schema.
- **Reconciliação:** ao iniciar, comparar banco com containers/processos; marcar órfãos; oferecer limpeza.
- **Timeouts:** download 10 min, install 15 min, start 2 min, health 60 s; configuráveis.
- **Retries:** rede com backoff exponencial (3 tentativas); nunca repetir comando destrutivo.
- **Disco:** verificar espaço livre antes de baixar/buildar; avisar abaixo de 2 GB.
- **Cancelamento:** jobs cooperativos (token de cancelamento); matar processos/containers ao cancelar.
- **Erros:** códigos estáveis (`YOURGIT-xxxx`), causa provável e ação; logs anexados.
- **Nunca apagar código do usuário sem confirmação.**
- **Crash reporting:** opt-in, apenas metadados e stack, nunca conteúdo de projetos.

### 15.1 Catálogo de erros inicial

| Código | Situação | Ação sugerida |
|---|---|---|
| YOURGIT-1001 | URL inválida | colar URL do GitHub |
| YOURGIT-1002 | repo não encontrado/privado | verificar URL ou entrar com GitHub |
| YOURGIT-1003 | rate limit do GitHub | aguardar ou autenticar |
| YOURGIT-1004 | download excedeu limite | usar Git ou aumentar limite |
| YOURGIT-1005 | zip inválido/unsafe | reportar; não extrair |
| YOURGIT-2001 | Docker indisponível | iniciar Docker Desktop ou usar nativo |
| YOURGIT-2002 | porta ocupada | escolher outra porta |
| YOURGIT-2003 | imagem não encontrada | verificar conexão/digest |
| YOURGIT-3001 | instalação falhou | ver log; editar comando |
| YOURGIT-3002 | health check falhou | ver logs; ajustar porta/path |
| YOURGIT-3003 | processo morreu | ver exit code e log |
| YOURGIT-4001 | plano desatualizado | reinspecionar |
| YOURGIT-4002 | segredo ausente | preencher variável |

---

## 16. Testes e qualidade

### 16.1 Pirâmide

- **Unitários (Rust):** parser de URL, detecção, geração de plano, alocador de portas, extração segura, validação de receita, redaction.
- **Integração (Rust):** SQLite, download com servidor fake, extração de fixtures maliciosas, runner nativo com processo fake, wrapper Docker com trait mock.
- **Frontend:** Vitest para stores/utilitários; Testing Library para componentes críticos.
- **E2E:** Playwright na UI web (modo mock) e `tauri-driver` para fluxos principais no Windows (quando estável).
- **Manual:** checklist em Windows 10/11, com e sem Docker, offline, caminhos com espaços/Unicode, antivírus, múltiplos monitores.

### 16.2 Repositórios de teste

| Caso | Exemplo de stack |
|---|---|
| Frontend Vite | Node + Vite |
| Next.js | Node + Next |
| API Rust | Axum |
| API Python | FastAPI |
| API Java | Spring Boot |
| API Go | net/http |
| Monorepo | pnpm workspaces |
| Com Dockerfile | app com Dockerfile |
| Com compose | app + Postgres |
| Env obrigatória | app que falha sem chave |
| Porta incomum | app em 4321 |
| Repo grande | limite e mensagem |

### 16.3 Fixtures de segurança

- Zip com `../../evil.txt`, caminho absoluto, symlink para fora, bomba de descompactação, nome Unicode malicioso.
- Receita com `; rm -rf`, `curl | sh`, `sudo`, caminho `C:\`.
- Logs com token falso para validar redaction.
- Compose com `privileged`, socket e mount fora do projeto.

### 16.4 CI

- **Windows:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `npm ci`, `npm run check`, `npm run test`, build Tauri.
- **Ubuntu:** testes do core e integração Docker (containers reais).
- **Release:** build assinado, SBOM, checksums, updater manifest.
- Gates: cobertura mínima do core (ex.: 70%), nenhum `unwrap` em caminho de produção sem justificativa, dependências auditadas (`cargo audit`, `npm audit`).

### 16.5 Performance

- Startup < 2 s.
- Lista com 100 projetos sem travar.
- UI mantém 60 fps durante logs (batching).
- Memória < 200 MB em repouso.
- Download/extração em streaming, sem carregar o zip inteiro na RAM.

---

## 17. Roadmap

Estimativas para **uma pessoa** familiarizada com Rust/TS, em meio período. Multiplicar por ~1,5 para imprevistos.

### Fase 0 — Fundação (2–3 dias)

- Confirmar nome, licença e decisões pendentes.
- Criar workspace, scaffold Tauri + Svelte, CI e lint.
- ADRs iniciais.
- Protótipo visual da biblioteca e do detalhe.

### Fase 1 — Fatia vertical local (8–10 dias)

- SQLite + settings + paths.
- Parser de URL + download + extração segura.
- Detecção Node/Rust + plano editável.
- Runner nativo com logs/stop.
- Biblioteca, detalhe, remoção.
- **Entrega:** rodar um repo Node e um Rust por URL, ver logs e remover.

### Fase 2 — Docker e mais stacks (8–12 dias)

- Detecção Docker; onboarding.
- Docker sandbox com labels, limites, portas, health.
- Cache de dependências, reconciliação e limpeza.
- Detecção Python/Java/Go.
- **Entrega:** instalar e rodar em container, parar/reiniciar, remover sem resíduos.

### Fase 3 — Catálogo e GitHub (8–10 dias)

- Schema do catálogo com canais `official` e `community`; 10–20 entradas oficiais e 2–5 comunitárias.
- UI do catálogo com abas/filtros “Oficial” e “Comunidade”, selos e avisos.
- Instalação por receita, assinatura do canal oficial e snapshot do comunitário.
- Metadados do GitHub, cache e ETag.
- Atualização de projetos.
- **Entrega:** instalar pelos dois canais, deixar claro o nível de confiança de cada receita e conviver com rate limits.

### Fase 4 — Beta (8–10 dias)

- Auth GitHub (device flow) e repos privados.
- Terminal (xterm + PTY).
- Env/segredos, redaction, diagnóstico.
- Polimento de UX, onboarding, empty states, acessibilidade.
- Instalador, updater, assinatura de código.
- Testes E2E e checklist manual.
- **Entrega:** beta pública para Windows.

### Fase 5 — Pós-beta

- Compose, Dockerfile, monorepo picker.
- Devcontainers, WSL, Podman.
- Monitor de recursos e snapshots.
- macOS/Linux.

**Total até o beta:** ~35–45 dias de desenvolvimento em meio período (8–12 semanas) ou ~6–8 semanas em tempo integral.

### Critérios de saída do MVP

- [ ] 15 dos 20 repos de teste instalam e rodam com plano correto ou editado.
- [ ] Nenhum comando executa sem aprovação na primeira execução.
- [ ] Remover projeto não deixa containers, volumes, redes ou pastas do YourGit.
- [ ] Zip-slip e injeção de comando cobertos por testes.
- [ ] Logs em tempo real com redaction.
- [ ] App funciona sem Docker (modo nativo avisado) e sem conta GitHub.
- [ ] Instalador Windows e atualização assinada funcionando.

---

## 18. Riscos e mitigação

| Risco | Impacto | Mitigação |
|---|---|---|
| Dependência do Docker Desktop | alto | modo nativo, WSL/Podman depois, instruções claras; não exigir Docker no app |
| Licenciamento do Docker Desktop em empresas | médio | documentar; suportar Docker Engine no WSL e Podman |
| Código malicioso executado | alto | isolamento padrão, limites, avisos, catálogo assinado, auditoria |
| Detecção errada em muitos repos | alto | plano editável, evidências, fallback manual, matriz de testes crescente |
| Performance de bind mount no Windows | médio | cache de dependências, WSL2, opção de volume; medir cedo |
| Escopo grande demais | alto | fases com critérios de saída; MVP sem compose/terminal/privados |
| Catálogo sem conteúdo | médio | 10–20 entradas oficiais boas + 2–5 comunitárias; permitir URL livre; crescimento orgânico |
| Rate limit do GitHub | médio | codeload, cache, ETag, auth opcional |
| Processos/containers órfãos | médio | labels, reconciliação, limpeza no startup |
| Antivírus/long paths no Windows | médio | testes com caminhos longos, prefixo `\\?\`, documentação |
| Atualização quebrar projetos | alto | updates desligados por padrão, backup, diff e rollback |
| Falta de tempo do mantenedor | alto | escopo enxuto, código modular, documentação desde o início |
| WebView2 ausente/corrompido | baixo | detectar e orientar instalação |
| Legal (licenças de repos) | médio | exibir licença; catálogo só com licenças permitidas; não redistribuir código |

---

## 19. Decisões pendentes

| # | Decisão | Recomendação | Impacto |
|---|---|---|---|
| 1 | Nome do produto | **YourGit** (provisório) | branding, domínio, pacote |
| 2 | Framework do frontend | **Svelte 5 + TypeScript** | velocidade de UI vs simplicidade vanilla |
| 3 | Plataforma inicial | **Windows 10/11 x64** | foco e qualidade; macOS/Linux depois |
| 4 | Catálogo | **dois canais**: oficial curado/assinado + comunidade com avisos | segurança, curadoria e expectativa do usuário |
| 5 | Docker | **recomendado, não obrigatório** | alcance vs isolamento |
| 6 | Modo nativo | **permitido com aviso**, nunca padrão | conveniência vs risco |
| 7 | Repos privados | **v0.2** | reduz escopo do MVP |
| 8 | Licença | **MIT OR Apache-2.0** | adoção e contribuições |
| 9 | Monetização | open source, doações/sponsors | evita backend e pagamentos no MVP |
| 10 | Terminal | v0.2 | MVP focado em logs |

**Se você não quiser decidir tudo agora:** siga as recomendações e ajuste depois. As três decisões que mais mudam o cronograma são frontend (2), catálogo (4) e Docker (5).

---

## 20. Próximos passos

1. **Confirmar** as decisões 1–3 (nome, frontend, plataforma) e o escopo do MVP.
2. Criar o scaffold do workspace (Tauri + crate core + Svelte + CI).
3. Escrever os ADRs 001–005 (Tauri, SQLite, Docker CLI, arquivo vs Git, Svelte).
4. Implementar a **fatia vertical**:
   - colar URL → baixar → detectar Node → plano → rodar nativo → logs → remover.
5. Adicionar Docker sandbox e validar com 5 repos reais.
6. Congelar o schema do catálogo e criar 10 entradas com receitas testadas.
7. Empacotar alpha para Windows e testar com usuários.

### Backlog inicial sugerido

| # | Tarefa | Critério de aceite |
|---|---|---|
| 1 | Scaffold Tauri + Svelte + CI | `npm run tauri dev` abre a janela; CI verde |
| 2 | SQLite + migrations + settings | cria DB, persiste workspace, sobrevive a restart |
| 3 | Parser de URL + download + extração segura | fixture zip-slip rejeitada; repo público extraído |
| 4 | Detector Node/Rust + UI de plano | identifica scripts/lockfile e permite editar |
| 5 | Runner nativo + logs | start, stream, stop sem processo órfão |
| 6 | Biblioteca + remoção | listar, filtrar, remover com limpeza |
| 7 | Docker status + sandbox runner | container sobe, logs, porta mapeada, stop/remove |
| 8 | Port allocator + health check | múltiplos projetos sem conflito; “Abrir” funciona |
| 9 | Catálogo embutido + UI | instalar 3 entradas pelo catálogo |
| 10 | Empacotamento + updater | instalador assinado e update de teste |

---

## 21. Referências e inspirações

- **Tauri 2** — shell desktop e IPC.
- **Dev Containers / devcontainer CLI** — padrão de ambientes reproduzíveis.
- **DevPod / Daytona** — ambientes de desenvolvimento portáveis.
- **Docker Compose** — orquestração local.
- **Nix / Flox** — reprodutibilidade (referência conceitual).
- **GitHub CLI** — UX de autenticação e downloads.
- **Portainer** — gestão visual de containers (referência de UI).

---

## 22. Glossário

- **RunPlan:** receita de execução gerada/editada para um projeto (comandos, portas, env).
- **Sandbox:** container base gerado pelo YourGit, sem depender de Dockerfile do repo.
- **Catálogo:** lista curada de repositórios e receitas.
- **Runner:** componente que executa um plano (nativo, Docker, compose).
- **Projeto:** repositório instalado e gerenciado pelo YourGit.
- **Run:** uma execução específica de um projeto, com logs e estado próprios.
- **Workspace:** pasta raiz onde os projetos são armazenados.
