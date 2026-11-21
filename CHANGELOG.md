# Changelog

Todas as mudanças relevantes do YourGit são registradas neste arquivo.

O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e o projeto adere ao [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [0.1.0] - 2026-11-21

Primeira versão pública do MVP para Windows.

### Adicionado

- Adição de projetos por URL do GitHub (HTTPS, SSH e `github.com/owner/repo`), com suporte a `/tree/<branch>/<pasta>`.
- Download do `.zip` via `codeload.github.com`, sem exigir Git instalado.
- Extração segura com proteção contra zip-slip, caminhos absolutos, symlinks e nomes reservados do Windows.
- Detecção de projetos Node (npm, pnpm, yarn, bun) e Rust, com plano de instalação, execução, porta provável e variáveis de `.env.example`.
- Revisão do plano antes de qualquer comando, com hash persistido e avisos de execução nativa.
- Execução nativa com logs em tempo real (`stdout`, `stderr`, `system`), histórico e parada por árvore de processos no Windows.
- Porta detectada por framework, configuração e scripts; a porta anunciada pelo servidor nos logs substitui a estimativa do plano.
- Biblioteca com busca, filtros, detalhe do projeto, logs, histórico, catálogo (oficial/comunidade) e configurações.
- Remoção do projeto com ou sem os arquivos, sempre restrita ao workspace.
- Confirmações dentro do app para execução nativa e remoção, sem diálogos nativos do WebView.
- Redação de tokens conhecidos e códigos ANSI nos logs.
- Identidade visual com o kit de logo do YourGit: tema azul, favicon e ícones do aplicativo e do executável.
- Instalador NSIS em português e inglês, com instalação por usuário.
- Workflows de CI (testes do núcleo, Clippy e checagem/build do frontend) e de release em tags `v*`.

### Segurança

- CSP restrita em produção e CSP separada para o servidor de desenvolvimento.
- `freezePrototype` habilitado no WebView.
- Remoção de arquivos bloqueada fora do workspace configurado.

### Limitações conhecidas

- A execução é **nativa**, com as permissões do usuário; o sandbox Docker é a fase 2.
- O instalador não é assinado; o SmartScreen pode exibir um aviso.
- A instalação pelo catálogo ainda não está ligada.
- Python, Java e Go são reconhecidos na detecção, mas ainda não têm plano de execução completo.
- Apenas Windows 10/11 nesta versão.
