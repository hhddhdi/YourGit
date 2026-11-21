use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::domain::{EnvSpec, PortSpec, RunPlan, StackInfo};
use crate::error::AppError;
use crate::source::sanitize_subdir;

pub fn inspect_repository(root: &Path, subdir: &str) -> Result<RunPlan, AppError> {
    let workdir = sanitize_subdir(subdir)?;
    let dir = if workdir == "." {
        root.to_path_buf()
    } else {
        root.join(&workdir)
    };

    if !dir.is_dir() {
        return Err(AppError::new(
            "YOURGIT-3001",
            "Pasta não encontrada",
            format!("A pasta '{workdir}' não existe dentro do repositório."),
        ));
    }

    let has_node = dir.join("package.json").is_file();
    let has_rust = dir.join("Cargo.toml").is_file();
    let has_python = dir.join("pyproject.toml").is_file()
        || dir.join("requirements.txt").is_file()
        || dir.join("setup.py").is_file()
        || dir.join("Pipfile").is_file();
    let has_go = dir.join("go.mod").is_file();
    let has_java = dir.join("pom.xml").is_file()
        || dir.join("build.gradle").is_file()
        || dir.join("build.gradle.kts").is_file();

    let mut detected: Vec<&str> = Vec::new();
    if has_node {
        detected.push("node");
    }
    if has_rust {
        detected.push("rust");
    }
    if has_python {
        detected.push("python");
    }
    if has_go {
        detected.push("go");
    }
    if has_java {
        detected.push("java");
    }

    if detected.is_empty() {
        let found = ["composer.json", "Gemfile", "mix.exs", "CMakeLists.txt"]
            .iter()
            .filter(|name| dir.join(name).is_file())
            .copied()
            .collect::<Vec<_>>();

        let detail = if found.is_empty() {
            "Nenhum manifesto conhecido foi encontrado.".to_string()
        } else {
            format!("Manifests encontrados: {}.", found.join(", "))
        };
        return Err(AppError::new(
            "YOURGIT-3001",
            "Stack nao suportada",
            format!("Ainda nao sei executar este projeto. {detail}"),
        )
        .with_hint("O YourGit detecta Node, Rust, Python, Go e Java."));
    }
    let mut plans = Vec::with_capacity(detected.len());
    for stack in &detected {
        let plan = match *stack {
            "node" => inspect_node(&dir, &workdir)?,
            "rust" => inspect_rust(&dir, &workdir)?,
            "python" => inspect_python(&dir, &workdir)?,
            "go" => inspect_go(&dir, &workdir)?,
            _ => inspect_java(&dir, &workdir)?,
        };
        plans.push(plan);
    }

    let mut plan = if plans.len() == 1 {
        plans.remove(0)
    } else {
        let chosen = plans
            .iter()
            .position(|plan| !plan.start.is_empty())
            .unwrap_or(0);
        let mut plan = plans.remove(chosen);
        plan.warnings.push(format!(
            "O repositorio contem mais de uma stack ({}); foi escolhida '{}'. Ajuste o plano se necessario.",
            detected.join(", "),
            detected[chosen]
        ));
        plan
    };

    plan.plan_hash = plan_hash(&plan);
    validate_plan(&plan)?;
    Ok(plan)
}

fn inspect_node(dir: &Path, workdir: &str) -> Result<RunPlan, AppError> {
    let package_text = fs::read_to_string(dir.join("package.json"))?;
    let package: Value = serde_json::from_str(&package_text).map_err(|error| {
        AppError::new(
            "YOURGIT-3001",
            "package.json inválido",
            format!("Não foi possível ler o package.json: {error}"),
        )
    })?;

    let scripts = package
        .get("scripts")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut dependencies = BTreeSet::new();
    for key in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(map) = package.get(key).and_then(Value::as_object) {
            for name in map.keys() {
                dependencies.insert(name.clone());
            }
        }
    }

    let (package_manager, lockfile) = detect_package_manager(dir);
    let install = install_command(package_manager, lockfile.as_deref());

    let start_script = ["dev", "start", "serve", "preview"]
        .iter()
        .find(|script| scripts.contains_key(**script))
        .copied();

    let start = match (package_manager, start_script) {
        ("npm", Some(script)) => vec![vec!["npm".into(), "run".into(), script.into()]],
        ("pnpm", Some(script)) => vec![vec!["pnpm".into(), "run".into(), script.into()]],
        ("yarn", Some(script)) => vec![vec!["yarn".into(), script.into()]],
        ("bun", Some(script)) => vec![vec!["bun".into(), "run".into(), script.into()]],
        (_, None) => package
            .get("main")
            .and_then(Value::as_str)
            .map(|main| vec![vec!["node".into(), main.into()]])
            .unwrap_or_default(),
        _ => vec![],
    };

    let framework = detect_node_framework(&dependencies);
    let port = detect_node_port(dir, framework.as_deref(), &scripts);

    let mut warnings = Vec::new();
    let lifecycle = ["preinstall", "install", "postinstall", "prepare"]
        .iter()
        .filter(|name| scripts.contains_key(**name))
        .copied()
        .collect::<Vec<_>>();
    if !lifecycle.is_empty() {
        warnings.push(format!(
            "O projeto executa scripts de ciclo de vida do npm: {}.",
            lifecycle.join(", ")
        ));
    }
    if lockfile.is_none() {
        warnings
            .push("Não há lockfile; as dependências podem variar entre instalações.".to_string());
    }
    if start.is_empty() {
        warnings.push(
            "Não foi encontrado um script dev/start/serve. Edite o plano para definir o comando de execução."
                .to_string(),
        );
    }
    if dir.join("pnpm-workspace.yaml").is_file()
        || dir.join("lerna.json").is_file()
        || dir.join("turbo.json").is_file()
        || dir.join("nx.json").is_file()
        || package.get("workspaces").is_some()
    {
        warnings.push(
            "Monorepo detectado. O MVP usa a raiz ou a subpasta da URL; escolher outro pacote entra na v0.2."
                .to_string(),
        );
    }
    if dir.join("Dockerfile").is_file() || dir.join("docker-compose.yml").is_file() {
        warnings.push(
            "O repositório tem Dockerfile/compose; o MVP ainda usa o sandbox nativo e o Docker entra na fase 2."
                .to_string(),
        );
    }

    let env = parse_env_example(dir);
    let mut evidence = vec!["package.json".to_string()];
    if let Some(lockfile) = &lockfile {
        evidence.push(lockfile.clone());
    }
    if let Some(framework) = &framework {
        evidence.push(format!("framework: {framework}"));
    }

    let confidence = if !start.is_empty() && lockfile.is_some() {
        0.95
    } else if !start.is_empty() {
        0.85
    } else {
        0.55
    };

    Ok(RunPlan {
        schema_version: 1,
        stack: StackInfo {
            language: "node".into(),
            framework,
            package_manager: Some(package_manager.into()),
            confidence,
            evidence,
        },
        workdir: workdir.to_string(),
        install,
        start,
        ports: port.map(port_spec).into_iter().collect(),
        env,
        warnings,
        plan_hash: String::new(),
    })
}

fn inspect_rust(dir: &Path, workdir: &str) -> Result<RunPlan, AppError> {
    let manifest = fs::read_to_string(dir.join("Cargo.toml"))?;
    let lower = manifest.to_ascii_lowercase();
    let framework = ["axum", "actix-web", "rocket", "warp", "poem", "tide"]
        .iter()
        .find(|name| lower.contains(**name))
        .map(|name| (*name).to_string());

    let mut warnings = Vec::new();
    if manifest.contains("[workspace]") {
        warnings.push(
            "Workspace Cargo detectado. O MVP executa `cargo run` na raiz; escolha o pacote na v0.2."
                .to_string(),
        );
    }
    if !dir.join("Cargo.lock").is_file() {
        warnings.push("Não há Cargo.lock; as versões podem variar.".to_string());
    }
    if dir.join("Dockerfile").is_file() {
        warnings.push("O repositório tem Dockerfile; o MVP ainda usa execução nativa.".to_string());
    }

    let evidence = ["Cargo.toml", "Cargo.lock"]
        .iter()
        .filter(|name| dir.join(name).is_file())
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();

    Ok(RunPlan {
        schema_version: 1,
        stack: StackInfo {
            language: "rust".into(),
            framework,
            package_manager: Some("cargo".into()),
            confidence: if warnings.iter().any(|w| w.contains("Workspace")) {
                0.65
            } else {
                0.88
            },
            evidence,
        },
        workdir: workdir.to_string(),
        install: vec![vec!["cargo".into(), "fetch".into()]],
        start: vec![vec!["cargo".into(), "run".into()]],
        ports: Some(port_spec(8080)).into_iter().collect(),
        env: parse_env_example(dir),
        warnings,
        plan_hash: String::new(),
    })
}

fn inspect_python(dir: &Path, workdir: &str) -> Result<RunPlan, AppError> {
    let pyproject = dir.join("pyproject.toml");
    let requirements = dir.join("requirements.txt");
    let has_pyproject = pyproject.is_file();
    let has_requirements = requirements.is_file();

    let mut evidence = Vec::new();
    let mut warnings = Vec::new();

    if has_pyproject {
        evidence.push("pyproject.toml".to_string());
    }
    if has_requirements {
        evidence.push("requirements.txt".to_string());
    }
    if dir.join("uv.lock").is_file() {
        evidence.push("uv.lock".to_string());
    }
    if dir.join("poetry.lock").is_file() {
        evidence.push("poetry.lock".to_string());
    }

    let mut manifest_text = String::new();
    if has_pyproject {
        manifest_text.push_str(
            &fs::read_to_string(&pyproject)
                .unwrap_or_default()
                .to_ascii_lowercase(),
        );
    }
    if has_requirements {
        manifest_text.push_str(
            &fs::read_to_string(&requirements)
                .unwrap_or_default()
                .to_ascii_lowercase(),
        );
    }

    let (package_manager, install) = if dir.join("uv.lock").is_file() {
        ("uv", vec![vec!["uv".to_string(), "sync".to_string()]])
    } else if dir.join("poetry.lock").is_file() {
        (
            "poetry",
            vec![vec!["poetry".to_string(), "install".to_string()]],
        )
    } else if dir.join("Pipfile.lock").is_file() {
        (
            "pipenv",
            vec![vec!["pipenv".to_string(), "install".to_string()]],
        )
    } else if has_requirements {
        (
            "pip",
            vec![vec![
                "python".to_string(),
                "-m".to_string(),
                "pip".to_string(),
                "install".to_string(),
                "-r".to_string(),
                "requirements.txt".to_string(),
            ]],
        )
    } else {
        (
            "pip",
            vec![vec![
                "python".to_string(),
                "-m".to_string(),
                "pip".to_string(),
                "install".to_string(),
                ".".to_string(),
            ]],
        )
    };

    let (framework, start, port) = if manifest_text.contains("fastapi") {
        (
            Some("fastapi".to_string()),
            vec![vec![
                "python".to_string(),
                "-m".to_string(),
                "uvicorn".to_string(),
                "main:app".to_string(),
                "--host".to_string(),
                "0.0.0.0".to_string(),
                "--port".to_string(),
                "8000".to_string(),
            ]],
            8000u16,
        )
    } else if manifest_text.contains("django") {
        (
            Some("django".to_string()),
            vec![vec![
                "python".to_string(),
                "manage.py".to_string(),
                "runserver".to_string(),
                "0.0.0.0:8000".to_string(),
            ]],
            8000u16,
        )
    } else if manifest_text.contains("flask") {
        (
            Some("flask".to_string()),
            vec![vec![
                "python".to_string(),
                "-m".to_string(),
                "flask".to_string(),
                "run".to_string(),
                "--host".to_string(),
                "0.0.0.0".to_string(),
            ]],
            5000u16,
        )
    } else if manifest_text.contains("streamlit") {
        (
            Some("streamlit".to_string()),
            vec![vec![
                "python".to_string(),
                "-m".to_string(),
                "streamlit".to_string(),
                "run".to_string(),
                "app.py".to_string(),
            ]],
            8501u16,
        )
    } else {
        (None, Vec::new(), 8000u16)
    };

    if start.is_empty() {
        warnings.push(
            "Nao identifiquei o framework Python; informe o comando de start manualmente."
                .to_string(),
        );
    }
    if dir.join("Dockerfile").is_file() {
        warnings.push(
            "O repositorio tem Dockerfile; o sandbox Docker usa a imagem base da stack."
                .to_string(),
        );
    }

    Ok(RunPlan {
        schema_version: 1,
        stack: StackInfo {
            language: "python".into(),
            framework,
            package_manager: Some(package_manager.into()),
            confidence: if start.is_empty() { 0.5 } else { 0.8 },
            evidence,
        },
        workdir: workdir.to_string(),
        install,
        start: start.clone(),
        ports: if start.is_empty() {
            Vec::new()
        } else {
            vec![port_spec(port)]
        },
        env: parse_env_example(dir),
        warnings,
        plan_hash: String::new(),
    })
}

fn inspect_go(dir: &Path, workdir: &str) -> Result<RunPlan, AppError> {
    let manifest = fs::read_to_string(dir.join("go.mod")).unwrap_or_default();
    let mut warnings = Vec::new();
    let mut evidence = vec!["go.mod".to_string()];

    if dir.join("go.sum").is_file() {
        evidence.push("go.sum".to_string());
    } else {
        warnings.push("Nao ha go.sum; as versoes das dependencias podem variar.".to_string());
    }

    let framework = [
        "gin-gonic/gin",
        "labstack/echo",
        "gofiber/fiber",
        "gorilla/mux",
        "go-chi/chi",
    ]
    .iter()
    .find(|name| manifest.contains(**name))
    .map(|name| (*name).to_string());

    Ok(RunPlan {
        schema_version: 1,
        stack: StackInfo {
            language: "go".into(),
            framework,
            package_manager: Some("go".into()),
            confidence: 0.85,
            evidence,
        },
        workdir: workdir.to_string(),
        install: vec![vec!["go".into(), "mod".into(), "download".into()]],
        start: vec![vec!["go".into(), "run".into(), ".".into()]],
        ports: vec![port_spec(8080)],
        env: parse_env_example(dir),
        warnings,
        plan_hash: String::new(),
    })
}

fn inspect_java(dir: &Path, workdir: &str) -> Result<RunPlan, AppError> {
    let has_maven = dir.join("pom.xml").is_file();
    let mut warnings = Vec::new();
    let mut evidence = Vec::new();

    if has_maven {
        evidence.push("pom.xml".to_string());
    }
    if dir.join("build.gradle").is_file() {
        evidence.push("build.gradle".to_string());
    }
    if dir.join("build.gradle.kts").is_file() {
        evidence.push("build.gradle.kts".to_string());
    }

    let manifest_text = if has_maven {
        fs::read_to_string(dir.join("pom.xml"))
            .unwrap_or_default()
            .to_ascii_lowercase()
    } else {
        let name = if dir.join("build.gradle.kts").is_file() {
            "build.gradle.kts"
        } else {
            "build.gradle"
        };
        fs::read_to_string(dir.join(name))
            .unwrap_or_default()
            .to_ascii_lowercase()
    };

    let is_spring =
        manifest_text.contains("spring-boot") || manifest_text.contains("org.springframework.boot");

    let (package_manager, install, start) = if has_maven {
        let wrapper = dir.join("mvnw").is_file() || dir.join("mvnw.cmd").is_file();
        if wrapper {
            evidence.push("mvnw".to_string());
        } else {
            warnings.push("Sem Maven Wrapper; o sandbox usa o Maven da imagem base.".to_string());
        }
        let mvn = if wrapper { "./mvnw" } else { "mvn" };
        (
            "maven",
            vec![vec![mvn.to_string(), "dependency:go-offline".to_string()]],
            if is_spring {
                vec![vec![mvn.to_string(), "spring-boot:run".to_string()]]
            } else {
                vec![vec![mvn.to_string(), "exec:java".to_string()]]
            },
        )
    } else {
        let wrapper = dir.join("gradlew").is_file() || dir.join("gradlew.bat").is_file();
        if wrapper {
            evidence.push("gradlew".to_string());
        } else {
            warnings.push("Sem Gradle Wrapper; o sandbox usa o Gradle da imagem base.".to_string());
        }
        let gradle = if wrapper { "./gradlew" } else { "gradle" };
        (
            "gradle",
            vec![vec![gradle.to_string(), "dependencies".to_string()]],
            if is_spring {
                vec![vec![gradle.to_string(), "bootRun".to_string()]]
            } else {
                vec![vec![gradle.to_string(), "run".to_string()]]
            },
        )
    };

    if !is_spring {
        warnings.push(
            "Projeto Java sem Spring Boot detectado; ajuste o comando de start se necessario."
                .to_string(),
        );
    }

    Ok(RunPlan {
        schema_version: 1,
        stack: StackInfo {
            language: "java".into(),
            framework: if is_spring {
                Some("spring-boot".into())
            } else {
                None
            },
            package_manager: Some(package_manager.into()),
            confidence: if is_spring { 0.85 } else { 0.6 },
            evidence,
        },
        workdir: workdir.to_string(),
        install,
        start,
        ports: vec![port_spec(8080)],
        env: parse_env_example(dir),
        warnings,
        plan_hash: String::new(),
    })
}

fn detect_package_manager(dir: &Path) -> (&'static str, Option<String>) {
    if dir.join("pnpm-lock.yaml").is_file() {
        ("pnpm", Some("pnpm-lock.yaml".into()))
    } else if dir.join("yarn.lock").is_file() {
        ("yarn", Some("yarn.lock".into()))
    } else if dir.join("bun.lockb").is_file() || dir.join("bun.lock").is_file() {
        let lock = if dir.join("bun.lockb").is_file() {
            "bun.lockb"
        } else {
            "bun.lock"
        };
        ("bun", Some(lock.into()))
    } else if dir.join("package-lock.json").is_file() {
        ("npm", Some("package-lock.json".into()))
    } else if dir.join("npm-shrinkwrap.json").is_file() {
        ("npm", Some("npm-shrinkwrap.json".into()))
    } else {
        ("npm", None)
    }
}

fn install_command(package_manager: &str, lockfile: Option<&str>) -> Vec<Vec<String>> {
    match package_manager {
        "pnpm" if lockfile.is_some() => {
            vec![vec![
                "pnpm".into(),
                "install".into(),
                "--frozen-lockfile".into(),
            ]]
        }
        "pnpm" => vec![vec!["pnpm".into(), "install".into()]],
        "yarn" => vec![vec!["yarn".into(), "install".into()]],
        "bun" => vec![vec!["bun".into(), "install".into()]],
        _ if lockfile.is_some() => vec![vec!["npm".into(), "ci".into()]],
        _ => vec![vec!["npm".into(), "install".into()]],
    }
}

fn detect_node_framework(dependencies: &BTreeSet<String>) -> Option<String> {
    for (dependency, label) in [
        ("next", "Next.js"),
        ("nuxt", "Nuxt"),
        ("@sveltejs/kit", "SvelteKit"),
        ("astro", "Astro"),
        ("vite", "Vite"),
        ("@angular/core", "Angular"),
        ("react-scripts", "React"),
        ("@nestjs/core", "NestJS"),
        ("express", "Express"),
        ("fastify", "Fastify"),
        ("svelte", "Svelte"),
        ("react", "React"),
    ] {
        if dependencies.contains(dependency) {
            return Some(label.to_string());
        }
    }
    None
}

fn detect_node_port(
    dir: &Path,
    framework: Option<&str>,
    scripts: &serde_json::Map<String, Value>,
) -> Option<u16> {
    let port_regex = Regex::new(r#"(?i)\bport\s*[:=]\s*['"]?(\d{2,5})"#).ok();

    for name in [
        "vite.config.ts",
        "vite.config.js",
        "vite.config.mjs",
        "next.config.js",
        "next.config.mjs",
        "astro.config.mjs",
        "nuxt.config.ts",
        "svelte.config.js",
    ] {
        if let Ok(text) = fs::read_to_string(dir.join(name)) {
            if let Some(regex) = &port_regex {
                if let Some(captures) = regex.captures(&text) {
                    if let Some(port) = captures
                        .get(1)
                        .and_then(|value| value.as_str().parse::<u16>().ok())
                    {
                        return Some(port);
                    }
                }
            }
        }
    }

    let commands = scripts
        .values()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();

    for command in &commands {
        if let Some(port) = explicit_port_flag(command) {
            return Some(port);
        }
    }

    // Legacy Svelte toolchains (rollup + sirv) and plain static servers do not
    // follow the Vite default. Without this, a Svelte template that serves on
    // 8080 would be reported as 5173.
    if matches!(framework, None | Some("Svelte")) {
        for command in &commands {
            let lower = command.to_lowercase();
            if lower.contains("sirv") || lower.contains("http-server") {
                return Some(8080);
            }
            if lower.contains("webpack-dev-server") || lower.contains("webpack serve") {
                return Some(8080);
            }
            if lower.contains("parcel") {
                return Some(1234);
            }
            if lower.split_whitespace().any(|part| part == "serve") {
                return Some(3000);
            }
        }
    }

    match framework {
        Some("Next.js") | Some("Nuxt") | Some("React") | Some("NestJS") | Some("Express")
        | Some("Fastify") => Some(3000),
        Some("Vite") | Some("SvelteKit") => Some(5173),
        Some("Svelte") => Some(8080),
        Some("Astro") => Some(4321),
        Some("Angular") => Some(4200),
        _ => Some(3000),
    }
}

fn explicit_port_flag(command: &str) -> Option<u16> {
    let parts = command.split_whitespace().collect::<Vec<_>>();
    let mut index = 0;
    while index < parts.len() {
        let part = parts[index].trim_matches(|character| character == '"' || character == '\'');
        if part == "--port" || part == "-p" {
            if let Some(value) = parts.get(index + 1) {
                if let Ok(port) = value
                    .trim_matches(|character| character == '"' || character == '\'')
                    .parse::<u16>()
                {
                    if port >= 80 {
                        return Some(port);
                    }
                }
            }
        }
        if let Some(value) = part.strip_prefix("--port=") {
            if let Ok(port) = value
                .trim_matches(|character| character == '"' || character == '\'')
                .parse::<u16>()
            {
                if port >= 80 {
                    return Some(port);
                }
            }
        }
        index += 1;
    }
    None
}

fn port_spec(port: u16) -> PortSpec {
    PortSpec {
        container: port,
        protocol: "http".into(),
        health_path: Some("/".into()),
    }
}

fn parse_env_example(dir: &Path) -> Vec<EnvSpec> {
    let mut found: BTreeMap<String, EnvSpec> = BTreeMap::new();
    for name in [".env.example", ".env.sample", ".env.template", ".env.dist"] {
        let path = dir.join(name);
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let mut last_comment: Option<String> = None;
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(comment) = trimmed.strip_prefix('#') {
                let comment = comment.trim();
                if !comment.is_empty() {
                    last_comment = Some(comment.to_string());
                }
                continue;
            }
            let trimmed = trimmed.strip_prefix("export ").unwrap_or(trimmed);
            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key.is_empty()
                || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                || key.chars().next().is_some_and(|c| c.is_ascii_digit())
            {
                continue;
            }
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            found.entry(key.to_string()).or_insert_with(|| EnvSpec {
                key: key.to_string(),
                required: value.is_empty(),
                secret: is_secret_key(key),
                description: last_comment.take(),
            });
            last_comment = None;
        }
    }
    found.into_values().collect()
}

fn is_secret_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    ["KEY", "SECRET", "TOKEN", "PASSWORD", "PASS", "PRIVATE"]
        .iter()
        .any(|needle| upper.contains(needle))
}

fn validate_plan(plan: &RunPlan) -> Result<(), AppError> {
    if plan.install.iter().chain(plan.start.iter()).any(|command| {
        command.is_empty()
            || command[0].trim().is_empty()
            || command.iter().any(|argument| argument.contains('\0'))
    }) {
        return Err(AppError::new(
            "YOURGIT-3001",
            "Plano inválido",
            "Um dos comandos gerados está vazio ou contém caracteres inválidos.",
        ));
    }
    Ok(())
}

pub fn plan_hash(plan: &RunPlan) -> String {
    let mut copy = plan.clone();
    copy.plan_hash.clear();
    let bytes = serde_json::to_vec(&copy).unwrap_or_default();
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

pub fn plan_workdir(root: &Path, plan: &RunPlan) -> Result<PathBuf, AppError> {
    let workdir = sanitize_subdir(&plan.workdir)?;
    let path = if workdir == "." {
        root.to_path_buf()
    } else {
        root.join(&workdir)
    };
    if !path.is_dir() {
        return Err(AppError::new(
            "YOURGIT-3003",
            "Pasta de execução ausente",
            "A pasta de trabalho do plano não existe mais no projeto.",
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_vite_project() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{
              "name": "demo",
              "scripts": { "dev": "vite", "build": "vite build" },
              "devDependencies": { "vite": "^7.0.0" }
            }"#,
        )
        .unwrap();
        fs::write(temp.path().join("package-lock.json"), "{}").unwrap();
        fs::write(
            temp.path().join("vite.config.ts"),
            "export default { server: { port: 4000 } }",
        )
        .unwrap();
        fs::write(
            temp.path().join(".env.example"),
            "# URL da API\nVITE_API_URL=\n",
        )
        .unwrap();

        let plan = inspect_repository(temp.path(), ".").unwrap();
        assert_eq!(plan.stack.language, "node");
        assert_eq!(plan.stack.framework.as_deref(), Some("Vite"));
        assert_eq!(plan.install[0], vec!["npm", "ci"]);
        assert_eq!(plan.start[0], vec!["npm", "run", "dev"]);
        assert_eq!(plan.ports[0].container, 4000);
        assert_eq!(plan.env[0].key, "VITE_API_URL");
        assert!(plan.env[0].required);
        assert!(!plan.plan_hash.is_empty());
    }

    #[test]
    fn detects_legacy_svelte_sirv_port() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{
              "name": "svelte-app",
              "scripts": {
                "dev": "rollup -c -w",
                "start": "sirv public --no-clear"
              },
              "devDependencies": { "svelte": "^3.55.0" },
              "dependencies": { "sirv-cli": "^2.0.0" }
            }"#,
        )
        .unwrap();
        let plan = inspect_repository(temp.path(), ".").unwrap();
        assert_eq!(plan.stack.framework.as_deref(), Some("Svelte"));
        assert_eq!(plan.ports[0].container, 8080);
    }

    #[test]
    fn explicit_port_flag_overrides_server_default() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{
              "name": "svelte-app",
              "scripts": {
                "dev": "rollup -c -w",
                "start": "sirv public --port 4000"
              },
              "devDependencies": { "svelte": "^3.55.0" },
              "dependencies": { "sirv-cli": "^2.0.0" }
            }"#,
        )
        .unwrap();
        let plan = inspect_repository(temp.path(), ".").unwrap();
        assert_eq!(plan.ports[0].container, 4000);
    }

    #[test]
    fn vite_default_port_wins_over_preview_sirv_script() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{
              "name": "demo",
              "scripts": {
                "dev": "vite",
                "preview": "sirv dist"
              },
              "devDependencies": { "vite": "^7.0.0" },
              "dependencies": { "sirv-cli": "^2.0.0" }
            }"#,
        )
        .unwrap();
        let plan = inspect_repository(temp.path(), ".").unwrap();
        assert_eq!(plan.stack.framework.as_deref(), Some("Vite"));
        assert_eq!(plan.ports[0].container, 5173);
    }

    #[test]
    fn detects_rust_project() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\n[dependencies]\naxum = \"0.8\"\n",
        )
        .unwrap();
        fs::write(temp.path().join("Cargo.lock"), "").unwrap();
        let plan = inspect_repository(temp.path(), ".").unwrap();
        assert_eq!(plan.stack.language, "rust");
        assert_eq!(plan.stack.framework.as_deref(), Some("axum"));
        assert_eq!(plan.start[0], vec!["cargo", "run"]);
        assert_eq!(plan.ports[0].container, 8080);
    }

    #[test]
    fn warns_when_no_start_script() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"name":"lib","scripts":{}}"#,
        )
        .unwrap();
        let plan = inspect_repository(temp.path(), ".").unwrap();
        assert!(plan.start.is_empty());
        assert!(plan.warnings.iter().any(|w| w.contains("dev/start")));
    }

    #[test]
    fn rejects_unknown_stack() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("README.md"), "# hi").unwrap();
        assert!(inspect_repository(temp.path(), ".").is_err());
    }

    #[test]
    fn detects_python_fastapi_project() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("pyproject.toml"),
            "[project]\nname = \"demo\"\ndependencies = [\"fastapi\", \"uvicorn\"]\n",
        )
        .unwrap();
        fs::write(temp.path().join("uv.lock"), "").unwrap();
        let plan = inspect_repository(temp.path(), ".").unwrap();
        assert_eq!(plan.stack.language, "python");
        assert_eq!(plan.stack.framework.as_deref(), Some("fastapi"));
        assert_eq!(plan.stack.package_manager.as_deref(), Some("uv"));
        assert_eq!(plan.install[0], vec!["uv", "sync"]);
        assert_eq!(plan.start[0][0], "python");
        assert_eq!(plan.start[0][2], "uvicorn");
        assert_eq!(plan.ports[0].container, 8000);
    }

    #[test]
    fn detects_go_project() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("go.mod"),
            "module demo\n\ngo 1.22\n\nrequire github.com/gin-gonic/gin v1.9.1\n",
        )
        .unwrap();
        fs::write(temp.path().join("go.sum"), "").unwrap();
        let plan = inspect_repository(temp.path(), ".").unwrap();
        assert_eq!(plan.stack.language, "go");
        assert_eq!(plan.stack.framework.as_deref(), Some("gin-gonic/gin"));
        assert_eq!(plan.install[0], vec!["go", "mod", "download"]);
        assert_eq!(plan.start[0], vec!["go", "run", "."]);
        assert_eq!(plan.ports[0].container, 8080);
    }

    #[test]
    fn detects_java_spring_maven_project() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("pom.xml"),
            "<project><dependencies><dependency><artifactId>spring-boot-starter-web</artifactId></dependency></dependencies></project>",
        )
        .unwrap();
        fs::write(temp.path().join("mvnw.cmd"), "").unwrap();
        let plan = inspect_repository(temp.path(), ".").unwrap();
        assert_eq!(plan.stack.language, "java");
        assert_eq!(plan.stack.framework.as_deref(), Some("spring-boot"));
        assert_eq!(plan.stack.package_manager.as_deref(), Some("maven"));
        assert_eq!(plan.start[0], vec!["./mvnw", "spring-boot:run"]);
    }

    #[test]
    fn prefers_stack_with_start_command() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"name":"lib","scripts":{}}"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .unwrap();
        fs::write(temp.path().join("Cargo.lock"), "").unwrap();
        let plan = inspect_repository(temp.path(), ".").unwrap();
        assert_eq!(plan.stack.language, "rust");
        assert!(plan
            .warnings
            .iter()
            .any(|w| w.contains("mais de uma stack")));
    }
}
