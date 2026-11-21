//! Smoke test manual do pipeline de instalação (usa rede).
//!
//! ```powershell
//! cargo run -p yourgit-core --example install_smoke -- <url-do-github>
//! ```
//!
//! Baixa, extrai, detecta a stack e grava no banco, mas **não executa** nada.
//! Usa um diretório temporário isolado em `%TEMP%\yourgit-smoke`.

use std::time::Instant;

use yourgit_core::installer::install_from_url;
use yourgit_core::paths::AppPaths;
use yourgit_core::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::args().nth(1).unwrap_or_else(|| {
        "https://github.com/vitejs/vite/tree/main/packages/create-vite/template-vanilla".to_string()
    });

    let root = std::env::temp_dir().join("yourgit-smoke");
    let _ = std::fs::remove_dir_all(&root);

    let paths = AppPaths::new(
        root.join("data"),
        root.join("cache"),
        root.join("workspace"),
    );
    paths.ensure()?;
    let store = Store::open(&paths.db_path)?;

    println!("URL: {url}");
    println!("Workspace: {}\n", paths.workspace_dir.display());

    let started = Instant::now();
    let detail = install_from_url(&paths, &store, &url, |message| {
        println!("  · {message}");
    })?;

    println!(
        "\nInstalado em {:.1}s: {}",
        started.elapsed().as_secs_f32(),
        detail.project.local_path
    );
    println!("{}", serde_json::to_string_pretty(&detail)?);
    println!(
        "\nProjeto de teste preservado em {}. Remova manualmente quando quiser.",
        root.display()
    );
    Ok(())
}
