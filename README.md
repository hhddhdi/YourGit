# YourGit

Gerenciador desktop de projetos GitHub: baixe um repositório, veja a stack detectada, revise o plano de execução, instale dependências e rode tudo em um só painel.

- **Backend:** Rust + Tauri 2
- **Frontend:** Svelte 5 + Vite + TypeScript
- **Execução atual:** nativa (o sandbox Docker é a fase 2)
- **Plataforma inicial:** Windows 10/11; Linux depois

> Status: **MVP funcional validado ponta a ponta**. O fluxo URL → download → extração segura → detecção de stack → revisão do plano → execução nativa → logs → parada/remoção foi exercitado no app desktop com projetos reais. O catálogo tem a estrutura dos canais oficial e comunidade, mas a instalação por catálogo entra na fase 3.

O plano completo está em [PLANO.md](./PLANO.md). O histórico de versões está em [CHANGELOG.md](./CHANGELOG.md).

## Instalação (Windows)

Baixe o instalador `YourGit_0.1.0_x64-setup.exe` na página de releases e execute. A instalação é por usuário e não exige privilégios de administrador.

O instalador ainda **não é assinado**: o SmartScreen pode exibir um aviso e pedir confirmação. Para compilar localmente, veja [Desenvolvimento](#desenvolvimento).

## O que já funciona

- Adicionar projeto por URL do GitHub (HTTPS, SSH ou `github.com/owner/repo`).
- Download do `.zip` via `codeload.github.com`, sem exigir Git instalado.
- Extração com proteção contra zip-slip, caminhos absolutos, symlinks, nomes reservados do Windows e limites de tamanho.
- Detecção de Node (npm, pnpm, yarn, bun) e Rust, com escolha de comando de instalação/start, porta provável e variáveis de `.env.example`.
- Porta inicial detectada por framework/configuração/scripts (Vite 5173, sirv 8080, CRA 3000 etc.) e corrigida em tempo real quando o servidor anuncia outra porta nos logs.
- Plano de execução persistido com hash e avisos antes de rodar qualquer comando.
- Execução nativa com logs em tempo real (`stdout`, `stderr`, `system`), histórico de execuções e parada por árvore de processos no Windows.
- Remoção do projeto com ou sem os arquivos, sempre restrita ao workspace.
- Confirmações dentro do app (sem diálogos nativos do WebView) para execução nativa e remoção.
- Redação de tokens conhecidos e códigos ANSI nos logs.
- UI com biblioteca, filtros, detalhe do projeto, plano, logs, histórico, catálogo (oficial/comunidade) e configurações.
- Identidade visual com o kit de logo do YourGit (ícone do app, favicon e tema azul; verde reservado para execução/sucesso).

## Estrutura

```text
yourgit/
├─ crates/yourgit-core/       # núcleo Rust sem dependência de Tauri
│  ├─ src/source.rs           # parser de URLs do GitHub
│  ├─ src/download.rs         # download do archive
│  ├─ src/archive.rs          # extração segura
│  ├─ src/inspect.rs          # detecção de stack e RunPlan
│  ├─ src/installer.rs        # instalação e remoção
│  ├─ src/store.rs            # SQLite (projetos, planos, execuções)
│  └─ src/paths.rs            # diretórios do app
├─ apps/desktop/              # app Tauri + frontend Svelte
│  ├─ src/                    # App.svelte, CSS, API e tipos
│  ├─ public/brand/           # logos, favicon e ícones do kit
│  └─ src-tauri/              # comandos IPC, supervisor e ícones do app
├─ assets/brand/              # kit completo de logos (variantes)
├─ .github/workflows/         # CI e release automatizado
├─ docs/                      # exemplos de schema
├─ CHANGELOG.md
└─ PLANO.md
```

## Requisitos

- Windows 10/11 com WebView2 (instalado por padrão no Windows 11).
- Rust estável e Visual Studio Build Tools (MSVC).
- Node.js 22+ e npm.
- Para rodar projetos Node: npm/pnpm/yarn/bun conforme o projeto.
- Para rodar projetos Rust: toolchain Cargo.
- Docker é opcional nesta fase; o daemon é detectado, mas ainda não usado para isolamento.

## Desenvolvimento

```powershell
# dependências do frontend
cd apps/desktop
npm install

# checagem de tipos e build do frontend
npm run check
npm run build

# app desktop em modo desenvolvimento
npm run tauri dev

# build de produção + instalador NSIS
npm run tauri build
# saída: target/release/bundle/nsis/YourGit_0.1.0_x64-setup.exe
```

Backend e testes do núcleo (28 testes):

```powershell
cargo test -p yourgit-core
cargo check -p yourgit-desktop
```

## CI e release

- `.github/workflows/ci.yml` roda testes do núcleo em Linux e Windows, Clippy e a checagem/build do frontend em cada push na `main` e em pull requests.
- `.github/workflows/release.yml` é disparado por tags `v*`, compila o instalador NSIS no Windows e cria um rascunho de release com o artefato.
- Para publicar uma nova versão: atualize a versão em `Cargo.toml`, `apps/desktop/package.json` e `tauri.conf.json`, registre no `CHANGELOG.md`, crie a tag e envie.

```powershell
git tag v0.1.0
git push origin main --tags
```

O release é criado como rascunho; revise o texto e publique manualmente no GitHub.

## Diretórios de dados

Por padrão, no Windows:

- Dados e banco: `%APPDATA%\YourGit\yourgit.db`
- Logs: `%APPDATA%\YourGit\logs\<project-id>\<run-id>.log`
- Workspace: `%USERPROFILE%\YourGit\projects\<owner>\<repo>`
- Cache de downloads: `%LOCALAPPDATA%\YourGit\cache`

Variáveis de ambiente para desenvolvimento e testes:

- `YOURGIT_DATA_DIR`
- `YOURGIT_CACHE_DIR`
- `YOURGIT_WORKSPACE`

## Segurança

- Nada é executado durante o download ou a inspeção; o plano é exibido antes do primeiro comando.
- A extração recusa symlinks, path traversal, caminhos absolutos e nomes proibidos no Windows.
- A remoção de arquivos é bloqueada fora do workspace configurado.
- Logs passam por limpeza de ANSI e redação de tokens/segredos conhecidos.
- CSP restrita em produção (com CSP separada para o servidor de desenvolvimento) e `freezePrototype` habilitado no WebView.
- **A execução nativa atual roda com as permissões do seu usuário.** Rode apenas repositórios confiáveis até o sandbox Docker chegar.

## Próximos passos

1. Edição do plano pela UI (comando de start, porta, workdir e variáveis).
2. Sandbox Docker com `install`/`start`/`ports` vindos da receita.
3. Catálogo oficial embutido + canal comunidade, com assinatura e snapshot offline.
4. Login GitHub para repositórios privados e atualização de projetos.
5. Instalador NSIS assinado e testes em Linux.

## Licença

Distribuído sob **MIT ou Apache-2.0**, à sua escolha. Veja [LICENSE-MIT](./LICENSE-MIT) e [LICENSE-APACHE](./LICENSE-APACHE).
