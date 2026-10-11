//! Technology detection: languages from file extensions, frameworks and services from manifests,
//! infrastructure and quality tooling from the repository tree.
//!
//! A technology counts only with a concrete artifact, and every detection keeps a short reason so
//! the candidate can see why it was attributed.

use std::collections::{BTreeMap, BTreeSet};

use fsp_domain::StackCategory;

use StackCategory::{
    Ai, Backend, Data, Frontend, Infrastructure, Language, Mobile, Quality, Tooling,
};

/// A detected technology with its category and, for catalog skills, the skill id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tech {
    pub name: &'static str,
    pub category: StackCategory,
    pub skill: Option<&'static str>,
}

const fn tech(name: &'static str, category: StackCategory) -> Tech {
    Tech {
        name,
        category,
        skill: None,
    }
}

const fn skill(name: &'static str, category: StackCategory, id: &'static str) -> Tech {
    Tech {
        name,
        category,
        skill: Some(id),
    }
}

impl Tech {
    /// The platform catalog skill this technology is evidence for, if any.
    pub fn catalog_skill(&self) -> Option<&'static str> {
        self.skill.or_else(|| catalog_skill(self.name))
    }
}

/// Catalog skills (`fsp_domain::catalog`) for detected technology and language names.
pub fn catalog_skill(name: &str) -> Option<&'static str> {
    Some(match name {
        "Python" => "python",
        "Java" => "java",
        "JavaScript" => "javascript",
        "TypeScript" => "typescript",
        "C#" => "csharp",
        "C++" => "cpp",
        "C" => "c",
        "Go" => "go",
        "Rust" => "rust",
        "Kotlin" => "kotlin",
        "Swift" => "swift",
        "Objective-C" => "objective_c",
        "PHP" => "php",
        "Ruby" => "ruby",
        "Scala" => "scala",
        "Elixir" => "elixir",
        "Erlang" => "erlang",
        "Haskell" => "haskell",
        "Clojure" => "clojure",
        "F#" => "fsharp",
        "OCaml" => "ocaml",
        "Dart" => "dart",
        "Lua" => "lua",
        "Perl" => "perl",
        "R" => "r",
        "Julia" => "julia",
        "SQL" => "sql",
        "PL/SQL" => "plsql",
        "1C Enterprise" => "onec_language",
        "ABAP" => "abap",
        "Delphi" => "delphi",
        "COBOL" => "cobol",
        "Fortran" => "fortran",
        "Assembly" => "assembly",
        "Verilog" => "verilog",
        "VHDL" => "vhdl",
        "Solidity" => "solidity",
        "Zig" => "zig",
        "Groovy" => "groovy",
        "Shell" => "bash",
        "PowerShell" => "powershell",
        "Visual Basic" => "visual_basic",
        "HTML" | "CSS" | "SCSS" => "html_css",
        "Vue" => "vue",
        "Svelte" | "SvelteKit" => "svelte",
        "GraphQL" => "graphql",
        "gRPC / Protocol Buffers" => "grpc",
        "Shaders" => "shaders",
        "GDScript" => "godot",
        "Tokio" => "tokio",
        "Axum" | "Actix Web" => "actix",
        "Gin" | "Echo" | "Fiber" | "chi" => "gin",
        "Django" => "django",
        "FastAPI" => "fastapi",
        "Flask" => "flask",
        "Express" | "Koa" | "Fastify" | "Hono" => "express",
        "NestJS" => "nestjs",
        "SQLAlchemy" => "sqlalchemy",
        "Celery" => "celery",
        "Socket.IO" => "websocket",
        "React" => "react",
        "Angular" => "angular",
        "Next.js" => "nextjs",
        "Nuxt" => "nuxt",
        "Redux" | "Zustand" => "redux",
        "TanStack Query" | "TanStack Router" => "tanstack",
        "Tailwind CSS" => "tailwind",
        "Vite" => "vite",
        "webpack" => "webpack",
        "Three.js" => "threejs",
        "Electron" | "Tauri" => "electron",
        "React Native" => "react_native",
        "PostgreSQL" => "postgresql",
        "SQLite" => "sqlite",
        "MongoDB" => "mongodb",
        "Redis" => "redis",
        "pandas / Polars" => "pandas",
        "NumPy / SciPy" => "numpy",
        "scikit-learn" => "scikit_learn",
        "PyTorch" => "pytorch",
        "TensorFlow" | "JAX" => "tensorflow",
        "Hugging Face Transformers" => "nlp",
        "LangChain" | "LlamaIndex" => "langchain",
        "OpenAI API" | "Anthropic API" | "Vercel AI SDK" | "LLM и AI-агенты" => "llm",
        "Docker" | "Docker Compose" => "docker",
        "Kubernetes" => "kubernetes",
        "Helm" => "helm",
        "Terraform" => "terraform",
        "Ansible" => "ansible",
        "GitHub Actions" => "github_actions",
        "GitLab CI" => "gitlab_ci",
        "Nginx" => "nginx",
        "Prometheus" => "prometheus",
        "systemd" => "linux",
        "Playwright" => "playwright",
        "Cypress" => "cypress",
        "Jest" | "Vitest" | "Testing Library" => "jest",
        "pytest" => "pytest",
        "Сбор веб-данных" => "etl",
        "1С:Предприятие 8" | "OneScript" => "onec_enterprise",
        "1C:EDT" => "onec_edt",
        "1С-Битрикс" => "bitrix",
        "Unity" => "unity",
        "Unreal Engine" => "unreal",
        "Godot" => "godot",
        "Flutter" => "flutter",
        "Arduino" | "PlatformIO" => "arduino",
        _ => return None,
    })
}

/// Detected technology → reasons.
pub type Detected = BTreeMap<Tech, BTreeSet<String>>;

/// Programming and markup languages by extension; data and prose formats return `None`.
pub fn language_of(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "dockerfile" | "containerfile" => return Some("Dockerfile"),
        "makefile" | "gnumakefile" => return Some("Makefile"),
        "justfile" => return Some("Just"),
        _ => {}
    }
    if lower.starts_with("dockerfile.") {
        return Some("Dockerfile");
    }
    let extension = lower.rsplit_once('.').map(|(_, extension)| extension)?;
    Some(match extension {
        "rs" => "Rust",
        "ts" | "tsx" | "mts" | "cts" => "TypeScript",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "py" | "pyi" => "Python",
        "ipynb" => "Jupyter Notebook",
        "go" => "Go",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "swift" => "Swift",
        "c" | "h" => "C",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "C++",
        "cs" => "C#",
        "fs" | "fsx" => "F#",
        "rb" => "Ruby",
        "php" => "PHP",
        "scala" | "sc" => "Scala",
        "dart" => "Dart",
        "ex" | "exs" => "Elixir",
        "erl" | "hrl" => "Erlang",
        "hs" => "Haskell",
        "clj" | "cljs" | "cljc" => "Clojure",
        "ml" | "mli" => "OCaml",
        "lua" => "Lua",
        "r" => "R",
        "jl" => "Julia",
        "zig" => "Zig",
        "nim" => "Nim",
        "sol" => "Solidity",
        "pl" | "pm" => "Perl",
        "groovy" => "Groovy",
        "sh" | "bash" | "zsh" | "fish" => "Shell",
        "ps1" | "psm1" => "PowerShell",
        "sql" => "SQL",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "scss" | "sass" | "less" => "SCSS",
        "vue" => "Vue",
        "svelte" => "Svelte",
        "astro" => "Astro",
        "tf" | "hcl" => "HCL",
        "nix" => "Nix",
        "proto" => "Protocol Buffers",
        "graphql" | "gql" => "GraphQL",
        "wgsl" | "glsl" | "hlsl" | "shader" => "Shaders",
        "bsl" | "os" => "1C Enterprise",
        "abap" => "ABAP",
        "pas" | "dpr" | "dpk" | "lpr" => "Delphi",
        "cob" | "cbl" | "cpy" => "COBOL",
        "f" | "f90" | "f95" | "f03" | "for" => "Fortran",
        "asm" | "s" | "nasm" => "Assembly",
        "v" | "sv" | "svh" => "Verilog",
        "vhd" | "vhdl" => "VHDL",
        "m" | "mm" => "Objective-C",
        "vb" | "bas" | "vba" => "Visual Basic",
        "pks" | "pkb" | "plsql" => "PL/SQL",
        "gd" => "GDScript",
        "ino" => "C++",
        _ => return None,
    })
}

pub(crate) fn is_vendored(path: &str) -> bool {
    path.split('/').any(|segment| {
        matches!(
            segment,
            "node_modules"
                | "vendor"
                | "target"
                | "dist"
                | "build"
                | ".venv"
                | "venv"
                | "third_party"
        )
    })
}

/// Manifests worth downloading: shallow first, at most `limit`.
pub fn manifest_paths(files: &[String], limit: usize) -> Vec<String> {
    let mut paths = files
        .iter()
        .filter(|path| !is_vendored(path))
        .filter(|path| {
            let name = path.rsplit('/').next().unwrap_or(path);
            matches!(
                name,
                "Cargo.toml" | "package.json" | "pyproject.toml" | "go.mod"
            ) || (name.starts_with("requirements") && name.ends_with(".txt"))
        })
        .cloned()
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| (path.matches('/').count(), path.clone()));
    paths.truncate(limit);
    paths
}

/// Technologies of one repository from its tree and downloaded manifests.
pub fn detect(files: &[String], manifests: &[(String, String)], topics: &[String]) -> Detected {
    let mut found = Detected::new();
    let mut add = |technology: Tech, reason: String| {
        found.entry(technology).or_default().insert(reason);
    };
    let mut language_files: BTreeMap<&'static str, u32> = BTreeMap::new();
    for path in files.iter().filter(|path| !is_vendored(path)) {
        if let Some(language) = language_of(path) {
            *language_files.entry(language).or_default() += 1;
        }
        let lower = path.to_ascii_lowercase();
        let name = lower.rsplit('/').next().unwrap_or(&lower);
        for (technology, reason) in file_signals(&lower, name) {
            add(technology, reason.into());
        }
    }
    for (language, count) in &language_files {
        if *count >= 3 || matches!(*language, "Dockerfile" | "HCL") {
            let technology = match *language {
                "Rust" => skill("Rust", Language, "rust"),
                "TypeScript" => skill("TypeScript", Language, "typescript"),
                "Dockerfile" => skill("Docker", Infrastructure, "docker"),
                "HCL" => tech("Terraform", Infrastructure),
                "SQL" => tech("SQL", Data),
                other => tech(other, Language),
            };
            add(technology, format!("{count} файлов {language}"));
        }
    }
    for (path, text) in manifests {
        let name = path.rsplit('/').next().unwrap_or(path);
        let dependencies = match name {
            "Cargo.toml" => cargo_dependencies(text),
            "package.json" => npm_dependencies(text),
            "pyproject.toml" => pyproject_dependencies(text),
            "go.mod" => go_dependencies(text),
            _ => requirement_names(text),
        };
        for (dependency, features) in dependencies {
            for technology in dependency_tech(name, &dependency, &features) {
                add(technology, format!("{name}: {dependency}"));
            }
        }
    }
    for topic in topics {
        let technology = match topic.to_ascii_lowercase().as_str() {
            "postgresql" | "postgres" => skill("PostgreSQL", Data, "postgresql"),
            "redis" => skill("Redis", Data, "redis"),
            "docker" => skill("Docker", Infrastructure, "docker"),
            "kubernetes" | "k8s" => tech("Kubernetes", Infrastructure),
            "react" | "reactjs" => skill("React", Frontend, "react"),
            "mcp" | "model-context-protocol" => tech("MCP", Ai),
            "llm" | "ai-agents" | "agents" | "ai" => tech("LLM и AI-агенты", Ai),
            _ => continue,
        };
        add(technology, format!("тема «{topic}»"));
    }
    found
}

fn file_signals(path: &str, name: &str) -> Vec<(Tech, &'static str)> {
    let mut signals = Vec::new();
    let mut push = |technology, reason| signals.push((technology, reason));
    if name == "dockerfile" || name == "containerfile" || name.starts_with("dockerfile.") {
        push(skill("Docker", Infrastructure, "docker"), "Dockerfile");
    }
    if name.starts_with("docker-compose") || name == "compose.yml" || name == "compose.yaml" {
        push(skill("Docker", Infrastructure, "docker"), "Docker Compose");
        push(tech("Docker Compose", Infrastructure), "compose-файл");
    }
    if path.starts_with(".github/workflows/") {
        push(
            tech("GitHub Actions", Infrastructure),
            "workflow GitHub Actions",
        );
    }
    if name == ".gitlab-ci.yml" {
        push(tech("GitLab CI", Infrastructure), ".gitlab-ci.yml");
    }
    if name.ends_with(".tf") {
        push(tech("Terraform", Infrastructure), "файлы Terraform");
    }
    if name == "chart.yaml" {
        push(tech("Helm", Infrastructure), "Chart.yaml");
        push(tech("Kubernetes", Infrastructure), "Helm-чарт");
    }
    if name == "kustomization.yaml" || path.starts_with("k8s/") || path.contains("/k8s/") {
        push(tech("Kubernetes", Infrastructure), "манифесты Kubernetes");
    }
    if name.ends_with(".service") || name.ends_with(".timer") {
        push(tech("systemd", Infrastructure), "unit-файлы systemd");
    }
    if name.starts_with("nginx") && name.ends_with(".conf") || path.contains("nginx/") {
        push(tech("Nginx", Infrastructure), "конфигурация Nginx");
    }
    if name == "playbook.yml" || path.contains("/roles/") && path.contains("/tasks/") {
        push(tech("Ansible", Infrastructure), "плейбуки Ansible");
    }
    if name == "flake.nix" {
        push(tech("Nix", Infrastructure), "flake.nix");
    }
    if name == "justfile" {
        push(tech("just", Tooling), "justfile");
    }
    if name.ends_with(".proto") {
        push(tech("gRPC / Protocol Buffers", Backend), "файлы .proto");
    }
    if name == "tsconfig.json" {
        push(skill("TypeScript", Language, "typescript"), "tsconfig.json");
    }
    for (prefix, technology) in [
        ("next.config.", tech("Next.js", Frontend)),
        ("vite.config.", tech("Vite", Frontend)),
        ("tailwind.config.", tech("Tailwind CSS", Frontend)),
        ("svelte.config.", tech("SvelteKit", Frontend)),
        ("nuxt.config.", tech("Nuxt", Frontend)),
        ("astro.config.", tech("Astro", Frontend)),
        ("playwright.config.", tech("Playwright", Quality)),
        ("vitest.config.", tech("Vitest", Quality)),
        ("jest.config.", tech("Jest", Quality)),
        ("cypress.config.", tech("Cypress", Quality)),
        ("eslint.config.", tech("ESLint", Quality)),
        (".eslintrc", tech("ESLint", Quality)),
        (".prettierrc", tech("Prettier", Quality)),
    ] {
        if name.starts_with(prefix) {
            push(technology, "конфигурационный файл");
        }
    }
    match name {
        "biome.json" | "biome.jsonc" => push(tech("Biome", Quality), "biome.json"),
        "rustfmt.toml" | ".rustfmt.toml" | "clippy.toml" | ".clippy.toml" => {
            push(
                tech("rustfmt / Clippy", Quality),
                "конфигурация rustfmt/Clippy",
            );
        }
        "ruff.toml" | ".ruff.toml" => push(tech("Ruff", Quality), "ruff.toml"),
        ".pre-commit-config.yaml" => push(tech("pre-commit", Quality), ".pre-commit-config.yaml"),
        "conftest.py" | "pytest.ini" => push(tech("pytest", Quality), "конфигурация pytest"),
        "dependabot.yml" | "dependabot.yaml" => push(tech("Dependabot", Quality), "dependabot.yml"),
        "renovate.json" | "renovate.json5" => push(tech("Renovate", Quality), "renovate.json"),
        "agents.md" | "claude.md" | "gemini.md" | "copilot-instructions.md" | ".cursorrules" => {
            push(
                tech("AI-агенты в разработке", Ai),
                "инструкции для AI-агентов",
            );
        }
        ".mcp.json" => push(tech("MCP", Ai), ".mcp.json"),
        _ => {}
    }
    // 1C:Enterprise: configuration dumps, EDT projects and OneScript packages.
    if name == "configdumpinfo.xml"
        || name == "configuration.mdo"
        || name == "configuration.xml" && path.contains("src/")
    {
        push(
            tech("1С:Предприятие 8", Backend),
            "выгрузка конфигурации 1С",
        );
    }
    if name.ends_with(".bsl") {
        push(tech("1С:Предприятие 8", Backend), "модули 1С (.bsl)");
    }
    if name.ends_with(".mdo") {
        push(tech("1C:EDT", Tooling), "проект 1C:EDT");
    }
    if name == "packagedef" || name.ends_with(".os") {
        push(tech("OneScript", Tooling), "сценарии OneScript");
    }
    if path.contains("bitrix/") && name.ends_with(".php") {
        push(tech("1С-Битрикс", Backend), "модули 1С-Битрикс");
    }
    // Game engines and native mobile or embedded toolchains.
    if name == "projectsettings.asset" || name.ends_with(".unity") {
        push(tech("Unity", Tooling), "проект Unity");
    }
    if name.ends_with(".uproject") {
        push(tech("Unreal Engine", Tooling), "проект Unreal Engine");
    }
    if name == "project.godot" {
        push(tech("Godot", Tooling), "project.godot");
    }
    if name == "pubspec.yaml" {
        push(tech("Flutter", Mobile), "pubspec.yaml");
    }
    if name == "platformio.ini" {
        push(tech("PlatformIO", Tooling), "platformio.ini");
    }
    if name.ends_with(".ino") {
        push(tech("Arduino", Tooling), "скетчи Arduino");
    }
    if path.starts_with(".claude/") || path.starts_with(".cursor/") || path.starts_with(".codex/") {
        push(
            tech("AI-агенты в разработке", Ai),
            "конфигурация AI-агентов",
        );
    }
    signals
}

fn dependency_tech(manifest: &str, dependency: &str, features: &[String]) -> Vec<Tech> {
    let postgres_feature = features.iter().any(|feature| feature.contains("postgres"));
    let name = dependency.to_ascii_lowercase();
    let name = name.as_str();
    let mut found = Vec::new();
    let mut push = |technology| found.push(technology);
    match manifest {
        "Cargo.toml" => match name {
            "tokio" => push(tech("Tokio", Backend)),
            "axum" => push(tech("Axum", Backend)),
            "actix-web" => push(tech("Actix Web", Backend)),
            "rocket" => push(tech("Rocket", Backend)),
            "warp" => push(tech("warp", Backend)),
            "hyper" => push(tech("Hyper", Backend)),
            "tonic" | "prost" => push(tech("gRPC / Protocol Buffers", Backend)),
            "reqwest" => push(tech("reqwest", Backend)),
            "serde" => push(tech("Serde", Tooling)),
            "tracing" => push(tech("tracing", Tooling)),
            "clap" => push(tech("clap (CLI)", Tooling)),
            "ratatui" | "crossterm" => push(tech("Ratatui (TUI)", Tooling)),
            "sqlx" => {
                push(tech("SQLx", Data));
                if postgres_feature {
                    push(skill("PostgreSQL", Data, "postgresql"));
                }
            }
            "diesel" => {
                push(tech("Diesel", Data));
                if postgres_feature {
                    push(skill("PostgreSQL", Data, "postgresql"));
                }
            }
            "sea-orm" => {
                push(tech("SeaORM", Data));
                if postgres_feature {
                    push(skill("PostgreSQL", Data, "postgresql"));
                }
            }
            "tokio-postgres" | "postgres" | "deadpool-postgres" | "bb8-postgres" => {
                push(skill("PostgreSQL", Data, "postgresql"));
            }
            "redis" | "deadpool-redis" | "bb8-redis" | "fred" => {
                push(skill("Redis", Data, "redis"))
            }
            "mongodb" => push(tech("MongoDB", Data)),
            "rusqlite" => push(tech("SQLite", Data)),
            "tauri" => push(tech("Tauri", Frontend)),
            "leptos" | "yew" | "dioxus" => push(tech("Rust для веба (WASM)", Frontend)),
            "wasm-bindgen" => push(tech("WebAssembly", Frontend)),
            "bevy" => push(tech("Bevy", Tooling)),
            "criterion" | "proptest" | "insta" => push(tech(
                "Тестирование Rust (criterion/proptest/insta)",
                Quality,
            )),
            "async-openai" | "openai" => push(tech("OpenAI API", Ai)),
            "rmcp" => push(tech("MCP", Ai)),
            _ => {}
        },
        "package.json" => match name {
            "react" | "react-dom" => push(skill("React", Frontend, "react")),
            "next" => push(tech("Next.js", Frontend)),
            "vue" => push(tech("Vue", Frontend)),
            "nuxt" => push(tech("Nuxt", Frontend)),
            "svelte" | "@sveltejs/kit" => push(tech("Svelte", Frontend)),
            "@angular/core" => push(tech("Angular", Frontend)),
            "solid-js" => push(tech("Solid", Frontend)),
            "@tanstack/react-query" | "@tanstack/query-core" => {
                push(tech("TanStack Query", Frontend))
            }
            "@tanstack/react-router" => push(tech("TanStack Router", Frontend)),
            "redux" | "@reduxjs/toolkit" => push(tech("Redux", Frontend)),
            "zustand" => push(tech("Zustand", Frontend)),
            "tailwindcss" => push(tech("Tailwind CSS", Frontend)),
            "vite" => push(tech("Vite", Frontend)),
            "webpack" => push(tech("webpack", Frontend)),
            "electron" => push(tech("Electron", Frontend)),
            "three" => push(tech("Three.js", Frontend)),
            "d3" => push(tech("D3", Frontend)),
            "express" => push(tech("Express", Backend)),
            "fastify" => push(tech("Fastify", Backend)),
            "@nestjs/core" => push(tech("NestJS", Backend)),
            "hono" => push(tech("Hono", Backend)),
            "koa" => push(tech("Koa", Backend)),
            "socket.io" => push(tech("Socket.IO", Backend)),
            "graphql" | "@apollo/client" | "@apollo/server" => push(tech("GraphQL", Backend)),
            "zod" => push(tech("Zod", Tooling)),
            "typescript" => push(skill("TypeScript", Language, "typescript")),
            "jest" => push(tech("Jest", Quality)),
            "vitest" => push(tech("Vitest", Quality)),
            "@playwright/test" | "playwright" => push(tech("Playwright", Quality)),
            "cypress" => push(tech("Cypress", Quality)),
            "@testing-library/react" => push(tech("Testing Library", Quality)),
            "eslint" => push(tech("ESLint", Quality)),
            "prettier" => push(tech("Prettier", Quality)),
            "@biomejs/biome" => push(tech("Biome", Quality)),
            "prisma" | "@prisma/client" => push(tech("Prisma", Data)),
            "drizzle-orm" => push(tech("Drizzle ORM", Data)),
            "typeorm" => push(tech("TypeORM", Data)),
            "mongoose" | "mongodb" => push(tech("MongoDB", Data)),
            "pg" | "postgres" | "pg-promise" | "@neondatabase/serverless" => {
                push(skill("PostgreSQL", Data, "postgresql"));
            }
            "redis" | "ioredis" | "@upstash/redis" | "bullmq" => {
                push(skill("Redis", Data, "redis"))
            }
            "react-native" | "expo" => push(tech("React Native", Mobile)),
            "openai" => push(tech("OpenAI API", Ai)),
            "@anthropic-ai/sdk" => push(tech("Anthropic API", Ai)),
            "langchain" | "@langchain/core" => push(tech("LangChain", Ai)),
            "@modelcontextprotocol/sdk" => push(tech("MCP", Ai)),
            "ai" => push(tech("Vercel AI SDK", Ai)),
            _ => {}
        },
        "go.mod" => match name {
            "github.com/gin-gonic/gin" => push(tech("Gin", Backend)),
            "github.com/labstack/echo/v4" | "github.com/labstack/echo" => {
                push(tech("Echo", Backend))
            }
            "github.com/gofiber/fiber/v2" | "github.com/gofiber/fiber" => {
                push(tech("Fiber", Backend))
            }
            "github.com/go-chi/chi/v5" | "github.com/go-chi/chi" => push(tech("chi", Backend)),
            "google.golang.org/grpc" => push(tech("gRPC / Protocol Buffers", Backend)),
            "github.com/spf13/cobra" => push(tech("Cobra (CLI)", Tooling)),
            "gorm.io/gorm" => push(tech("GORM", Data)),
            "github.com/jackc/pgx/v5" | "github.com/jackc/pgx/v4" | "github.com/lib/pq" => {
                push(skill("PostgreSQL", Data, "postgresql"));
            }
            "github.com/redis/go-redis/v9" | "github.com/go-redis/redis/v8" => {
                push(skill("Redis", Data, "redis"));
            }
            "github.com/prometheus/client_golang" => push(tech("Prometheus", Infrastructure)),
            "k8s.io/client-go" => push(tech("Kubernetes", Infrastructure)),
            "github.com/stretchr/testify" => push(tech("Testify", Quality)),
            _ => {}
        },
        _ => match name.replace('_', "-").as_str() {
            "django" => push(tech("Django", Backend)),
            "flask" => push(tech("Flask", Backend)),
            "fastapi" => push(tech("FastAPI", Backend)),
            "starlette" => push(tech("Starlette", Backend)),
            "aiohttp" => push(tech("aiohttp", Backend)),
            "httpx" | "requests" => push(tech("HTTP-клиенты Python", Backend)),
            "uvicorn" | "gunicorn" => push(tech("ASGI/WSGI-серверы", Backend)),
            "pydantic" => push(tech("Pydantic", Tooling)),
            "typer" | "click" => push(tech("Python CLI (Typer/Click)", Tooling)),
            "rich" => push(tech("Rich", Tooling)),
            "sqlalchemy" => push(tech("SQLAlchemy", Data)),
            "alembic" => push(tech("Alembic", Data)),
            "psycopg" | "psycopg2" | "psycopg2-binary" | "asyncpg" => {
                push(skill("PostgreSQL", Data, "postgresql"));
            }
            "redis" => push(skill("Redis", Data, "redis")),
            "pymongo" | "motor" => push(tech("MongoDB", Data)),
            "celery" => push(tech("Celery", Backend)),
            "pandas" | "polars" => push(tech("pandas / Polars", Data)),
            "numpy" | "scipy" => push(tech("NumPy / SciPy", Data)),
            "scikit-learn" => push(tech("scikit-learn", Ai)),
            "torch" | "pytorch" => push(tech("PyTorch", Ai)),
            "tensorflow" | "keras" => push(tech("TensorFlow", Ai)),
            "jax" => push(tech("JAX", Ai)),
            "transformers" => push(tech("Hugging Face Transformers", Ai)),
            "langchain" | "langchain-core" => push(tech("LangChain", Ai)),
            "llama-index" => push(tech("LlamaIndex", Ai)),
            "openai" => push(tech("OpenAI API", Ai)),
            "anthropic" => push(tech("Anthropic API", Ai)),
            "mcp" | "fastmcp" => push(tech("MCP", Ai)),
            "pytest" => push(tech("pytest", Quality)),
            "ruff" | "black" | "mypy" => push(tech("Ruff / Black / mypy", Quality)),
            "playwright" | "selenium" => push(tech("Браузерная автоматизация", Quality)),
            "scrapy" | "beautifulsoup4" => push(tech("Сбор веб-данных", Data)),
            _ => {}
        },
    }
    found
}

/// Dependency names (after `package` renames) with their enabled features, from every dependency
/// table including workspace and target-specific ones.
pub fn cargo_dependencies(manifest: &str) -> Vec<(String, Vec<String>)> {
    let Ok(value) = manifest.parse::<toml::Table>() else {
        return Vec::new();
    };
    let mut tables = Vec::new();
    for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
        tables.extend(value.get(key).and_then(toml::Value::as_table));
    }
    if let Some(workspace) = value.get("workspace").and_then(toml::Value::as_table) {
        tables.extend(
            workspace
                .get("dependencies")
                .and_then(toml::Value::as_table),
        );
    }
    if let Some(targets) = value.get("target").and_then(toml::Value::as_table) {
        for target in targets.values().filter_map(toml::Value::as_table) {
            for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
                tables.extend(target.get(key).and_then(toml::Value::as_table));
            }
        }
    }
    let mut dependencies = Vec::new();
    for table in tables {
        for (key, spec) in table {
            let name = spec
                .get("package")
                .and_then(toml::Value::as_str)
                .unwrap_or(key)
                .to_owned();
            let features = spec
                .get("features")
                .and_then(toml::Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            dependencies.push((name, features));
        }
    }
    dependencies
}

pub fn npm_dependencies(manifest: &str) -> Vec<(String, Vec<String>)> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(manifest) else {
        return Vec::new();
    };
    let mut names = BTreeSet::new();
    for key in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        if let Some(object) = value.get(key).and_then(serde_json::Value::as_object) {
            names.extend(object.keys().cloned());
        }
    }
    names.into_iter().map(|name| (name, Vec::new())).collect()
}

/// PEP 621 `project.dependencies`, optional dependencies and Poetry dependency tables.
pub fn pyproject_dependencies(manifest: &str) -> Vec<(String, Vec<String>)> {
    let Ok(value) = manifest.parse::<toml::Table>() else {
        return Vec::new();
    };
    let mut names = BTreeSet::new();
    let project = value.get("project").and_then(toml::Value::as_table);
    if let Some(list) = project
        .and_then(|project| project.get("dependencies"))
        .and_then(toml::Value::as_array)
    {
        names.extend(
            list.iter()
                .filter_map(toml::Value::as_str)
                .filter_map(requirement_name),
        );
    }
    if let Some(groups) = project
        .and_then(|project| project.get("optional-dependencies"))
        .and_then(toml::Value::as_table)
    {
        for list in groups.values().filter_map(toml::Value::as_array) {
            names.extend(
                list.iter()
                    .filter_map(toml::Value::as_str)
                    .filter_map(requirement_name),
            );
        }
    }
    if let Some(poetry) = value
        .get("tool")
        .and_then(|tool| tool.get("poetry"))
        .and_then(toml::Value::as_table)
    {
        for key in ["dependencies", "dev-dependencies"] {
            if let Some(table) = poetry.get(key).and_then(toml::Value::as_table) {
                names.extend(table.keys().map(|key| key.to_ascii_lowercase()));
            }
        }
    }
    names.into_iter().map(|name| (name, Vec::new())).collect()
}

/// `requirements.txt`: one requirement per line; options, includes and comments are skipped.
pub fn requirement_names(text: &str) -> Vec<(String, Vec<String>)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('-'))
        .filter_map(requirement_name)
        .map(|name| (name, Vec::new()))
        .collect()
}

fn requirement_name(requirement: &str) -> Option<String> {
    let name = requirement
        .trim()
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'))
        .next()?;
    (!name.is_empty()).then(|| name.to_ascii_lowercase())
}

/// Module paths from `require` lines and blocks of `go.mod`.
pub fn go_dependencies(text: &str) -> Vec<(String, Vec<String>)> {
    let mut modules = Vec::new();
    let mut block = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with("require (") {
            block = true;
            continue;
        }
        if block && line == ")" {
            block = false;
            continue;
        }
        let entry = if block {
            Some(line)
        } else {
            line.strip_prefix("require ")
        };
        if let Some(module) = entry
            .and_then(|entry| entry.split_whitespace().next())
            .filter(|module| module.contains('/'))
        {
            modules.push((module.to_owned(), Vec::new()));
        }
    }
    modules
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(found: &Detected) -> Vec<&'static str> {
        found.keys().map(|technology| technology.name).collect()
    }

    #[test]
    fn every_catalog_mapping_names_a_catalog_skill() {
        let names = [
            "Python",
            "Java",
            "JavaScript",
            "TypeScript",
            "C#",
            "C++",
            "C",
            "Go",
            "Rust",
            "Kotlin",
            "Swift",
            "Objective-C",
            "PHP",
            "Ruby",
            "Scala",
            "Elixir",
            "Erlang",
            "Haskell",
            "Clojure",
            "F#",
            "OCaml",
            "Dart",
            "Lua",
            "Perl",
            "R",
            "Julia",
            "SQL",
            "PL/SQL",
            "1C Enterprise",
            "ABAP",
            "Delphi",
            "COBOL",
            "Fortran",
            "Assembly",
            "Verilog",
            "VHDL",
            "Solidity",
            "Zig",
            "Groovy",
            "Shell",
            "PowerShell",
            "Visual Basic",
            "HTML",
            "Vue",
            "Svelte",
            "GraphQL",
            "gRPC / Protocol Buffers",
            "Shaders",
            "GDScript",
            "Tokio",
            "Axum",
            "Gin",
            "Django",
            "FastAPI",
            "Flask",
            "Express",
            "NestJS",
            "SQLAlchemy",
            "Celery",
            "Socket.IO",
            "React",
            "Angular",
            "Next.js",
            "Nuxt",
            "Redux",
            "TanStack Query",
            "Tailwind CSS",
            "Vite",
            "webpack",
            "Three.js",
            "Electron",
            "React Native",
            "PostgreSQL",
            "SQLite",
            "MongoDB",
            "Redis",
            "pandas / Polars",
            "NumPy / SciPy",
            "scikit-learn",
            "PyTorch",
            "TensorFlow",
            "Hugging Face Transformers",
            "LangChain",
            "OpenAI API",
            "Docker",
            "Kubernetes",
            "Helm",
            "Terraform",
            "Ansible",
            "GitHub Actions",
            "GitLab CI",
            "Nginx",
            "Prometheus",
            "systemd",
            "Playwright",
            "Cypress",
            "Jest",
            "pytest",
            "Сбор веб-данных",
            "1С:Предприятие 8",
            "1C:EDT",
            "1С-Битрикс",
            "Unity",
            "Unreal Engine",
            "Godot",
            "Flutter",
            "Arduino",
        ];
        for name in names {
            let skill = catalog_skill(name).unwrap_or_else(|| panic!("{name} has no skill"));
            assert!(fsp_domain::catalog::is_skill(skill), "{name} → {skill}");
        }
    }

    #[test]
    fn onec_sources_and_configurations_are_detected() {
        let files = [
            "src/CommonModules/Common/Module.bsl",
            "src/CommonModules/Prices/Module.bsl",
            "src/Documents/Order/ObjectModule.bsl",
            "src/Configuration/Configuration.mdo",
        ]
        .map(String::from);
        let found = detect(&files, &[], &[]);
        let skills = found
            .keys()
            .filter_map(Tech::catalog_skill)
            .collect::<BTreeSet<_>>();
        assert!(skills.contains("onec_language"), "{skills:?}");
        assert!(skills.contains("onec_enterprise"));
        assert!(skills.contains("onec_edt"));
        assert_eq!(
            language_of("src/CommonModules/Common/Module.bsl"),
            Some("1C Enterprise")
        );
    }

    #[test]
    fn languages_come_from_extensions_and_skip_data_files() {
        assert_eq!(language_of("src/main.rs"), Some("Rust"));
        assert_eq!(language_of("web/App.tsx"), Some("TypeScript"));
        assert_eq!(language_of("infra/Dockerfile"), Some("Dockerfile"));
        assert_eq!(language_of("docs/README.md"), None);
        assert_eq!(language_of("Cargo.lock"), None);
    }

    #[test]
    fn manifests_frameworks_and_infrastructure_are_detected_with_reasons() {
        let files = [
            "Cargo.toml",
            "crates/api/Cargo.toml",
            "crates/api/src/lib.rs",
            "crates/api/src/routes.rs",
            "crates/api/src/db.rs",
            "web/package.json",
            "web/src/App.tsx",
            "Dockerfile",
            ".github/workflows/ci.yml",
            "deploy/main.tf",
            "AGENTS.md",
            "node_modules/react/package.json",
        ]
        .map(String::from);
        let manifests = vec![
            (
                "crates/api/Cargo.toml".to_owned(),
                "[dependencies]\naxum = \"0.8\"\ntokio = \"1\"\nsqlx = { version = \"0.8\", features = [\"postgres\"] }\n".to_owned(),
            ),
            (
                "web/package.json".to_owned(),
                r#"{"dependencies":{"react":"19","@tanstack/react-query":"5"},"devDependencies":{"typescript":"7","vitest":"4"}}"#.to_owned(),
            ),
        ];
        let found = detect(&files, &manifests, &["mcp".into()]);
        for expected in [
            "Axum",
            "Docker",
            "GitHub Actions",
            "PostgreSQL",
            "React",
            "Rust",
            "SQLx",
            "TanStack Query",
            "Terraform",
            "Tokio",
            "TypeScript",
            "Vitest",
            "AI-агенты в разработке",
            "MCP",
        ] {
            assert!(
                names(&found).contains(&expected),
                "{expected} missing: {:?}",
                names(&found)
            );
        }
        let postgres = found
            .iter()
            .find(|(technology, _)| technology.name == "PostgreSQL")
            .unwrap();
        assert_eq!(postgres.0.skill, Some("postgresql"));
        assert!(postgres.1.contains("Cargo.toml: sqlx"));
    }

    #[test]
    fn python_and_go_manifests_are_parsed() {
        let pyproject = "[project]\ndependencies = [\"fastapi>=0.110\", \"SQLAlchemy[asyncio]==2.0\", \"anthropic\"]\n";
        assert_eq!(
            pyproject_dependencies(pyproject)
                .into_iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>(),
            ["anthropic", "fastapi", "sqlalchemy"]
        );
        let requirements = "# web\npandas==2.2\n-r base.txt\ntorch>=2\n";
        assert_eq!(
            requirement_names(requirements)
                .into_iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>(),
            ["pandas", "torch"]
        );
        let go = "module x\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.10.0\n\tgithub.com/jackc/pgx/v5 v5.6.0\n)\nrequire github.com/spf13/cobra v1.8.0\n";
        assert_eq!(
            go_dependencies(go)
                .into_iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>(),
            [
                "github.com/gin-gonic/gin",
                "github.com/jackc/pgx/v5",
                "github.com/spf13/cobra"
            ]
        );
    }

    #[test]
    fn manifest_paths_prefer_shallow_files_and_skip_vendored_ones() {
        let files = [
            "node_modules/x/package.json",
            "a/b/c/Cargo.toml",
            "Cargo.toml",
            "requirements-dev.txt",
            "web/package.json",
        ]
        .map(String::from);
        assert_eq!(
            manifest_paths(&files, 3),
            ["Cargo.toml", "requirements-dev.txt", "web/package.json"]
        );
    }
}
