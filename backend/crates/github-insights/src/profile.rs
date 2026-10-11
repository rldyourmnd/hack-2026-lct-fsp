//! Developer portrait: which kinds of work the account does, in which languages and with which
//! technologies.
//!
//! Every non-merge commit's changed files are attributed to an area by extension, path and the
//! repository's stack (a TypeScript file is frontend work in a React repository and backend work
//! in an Express one). Shares are counted in changed files, so they describe the account's own
//! work rather than the repositories' composition.

use std::collections::BTreeMap;

use fsp_domain::{Archetype, GitHubArchetypeShare, GitHubLanguageShare, StackCategory};

use crate::tech::{Detected, language_of};

/// The archetype as it reads inside a sentence.
fn phrase(archetype: Archetype) -> &'static str {
    match archetype {
        Archetype::Backend => "бэкенд",
        Archetype::Frontend => "фронтенд",
        Archetype::Platform => "DevOps и платформа",
        Archetype::Systems => "системные инструменты и CLI",
        Archetype::Data => "данные и AI",
        Archetype::Mobile => "мобильная разработка",
    }
}

pub fn label(archetype: Archetype) -> &'static str {
    match archetype {
        Archetype::Backend => "Бэкенд",
        Archetype::Frontend => "Фронтенд",
        Archetype::Platform => "DevOps и платформа",
        Archetype::Systems => "Системные инструменты и CLI",
        Archetype::Data => "Данные и AI",
        Archetype::Mobile => "Мобильная разработка",
    }
}

/// Analytics, machine learning and LLM libraries: the code that makes a project data or AI work.
/// Storage, ORMs, topics and AI coding-agent configuration do not.
const DATA_WORK: [&str; 13] = [
    "pandas / Polars",
    "NumPy / SciPy",
    "scikit-learn",
    "PyTorch",
    "TensorFlow",
    "JAX",
    "Hugging Face Transformers",
    "LangChain",
    "LlamaIndex",
    "OpenAI API",
    "Anthropic API",
    "Vercel AI SDK",
    "Сбор веб-данных",
];

/// Configuration for AI coding agents: a way of working, not an area of the code.
const AGENT_TOOLING: &str = "AI-агенты в разработке";

/// What a repository's stack says about ambiguous languages.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepositoryKind {
    frontend: bool,
    backend: bool,
    data: bool,
    tooling: bool,
    mobile: bool,
}

impl RepositoryKind {
    pub fn of(detected: &Detected) -> Self {
        let mut kind = Self::default();
        for technology in detected.keys() {
            match technology.category {
                StackCategory::Frontend => kind.frontend = true,
                StackCategory::Backend => kind.backend = true,
                StackCategory::Ai | StackCategory::Data => {
                    if DATA_WORK.contains(&technology.name) {
                        kind.data = true;
                    }
                }
                StackCategory::Tooling => {
                    if technology.name.contains("CLI") || technology.name.contains("TUI") {
                        kind.tooling = true;
                    }
                }
                StackCategory::Mobile => kind.mobile = true,
                _ => {}
            }
        }
        kind
    }
}

/// The area one changed file belongs to; `None` for documentation and data files.
pub fn area(path: &str, kind: RepositoryKind) -> Option<Archetype> {
    let lower = path.to_ascii_lowercase();
    let segments = lower.split('/').collect::<Vec<_>>();
    let under = |names: &[&str]| segments.iter().any(|segment| names.contains(segment));
    if lower.starts_with(".github/workflows/")
        || under(&[
            "k8s",
            "helm",
            "charts",
            "terraform",
            "ansible",
            "deploy",
            "infra",
            "ops",
            "nginx",
            "systemd",
        ])
        || lower.ends_with(".service")
        || lower.ends_with(".timer")
    {
        return Some(Archetype::Platform);
    }
    let language = language_of(path)?;
    Some(match language {
        "Dockerfile" | "Shell" | "PowerShell" | "HCL" | "Nix" | "Makefile" | "Just" => {
            Archetype::Platform
        }
        "HTML" | "CSS" | "SCSS" | "Vue" | "Svelte" | "Astro" => Archetype::Frontend,
        "Swift" | "Dart" | "Objective-C" => Archetype::Mobile,
        // Enterprise platforms: 1C, SAP and classic business stacks.
        "1C Enterprise" | "ABAP" | "Delphi" | "COBOL" | "Visual Basic" | "PL/SQL" => {
            Archetype::Backend
        }
        "Kotlin" if kind.mobile || under(&["android"]) => Archetype::Mobile,
        "TypeScript" | "JavaScript" => {
            if lower.ends_with(".tsx")
                || lower.ends_with(".jsx")
                || under(&[
                    "frontend",
                    "web",
                    "ui",
                    "client",
                    "components",
                    "pages",
                    "app",
                ])
                || (kind.frontend && !kind.backend)
            {
                Archetype::Frontend
            } else if kind.mobile {
                Archetype::Mobile
            } else if kind.backend || under(&["server", "api", "backend"]) {
                Archetype::Backend
            } else if kind.frontend {
                Archetype::Frontend
            } else {
                Archetype::Systems
            }
        }
        "Jupyter Notebook" | "R" | "Julia" => Archetype::Data,
        "Python" => {
            let service = under(&[
                "api",
                "server",
                "backend",
                "routes",
                "routers",
                "views",
                "handlers",
                "services",
                "endpoints",
            ]);
            if kind.backend && (service || !kind.data) {
                Archetype::Backend
            } else if kind.data {
                Archetype::Data
            } else {
                Archetype::Systems
            }
        }
        "SQL" | "GraphQL" | "Protocol Buffers" => Archetype::Backend,
        "Rust" | "Go" | "C" | "C++" | "Zig" | "Nim" => {
            if kind.backend && !kind.tooling {
                Archetype::Backend
            } else if kind.frontend && !kind.backend {
                Archetype::Frontend
            } else {
                Archetype::Systems
            }
        }
        "Java" | "Kotlin" | "C#" | "F#" | "Ruby" | "PHP" | "Scala" | "Elixir" | "Erlang"
        | "Clojure" | "Haskell" | "OCaml" | "Groovy" | "Perl" => Archetype::Backend,
        "Solidity" => Archetype::Backend,
        _ => Archetype::Systems,
    })
}

/// Language shares of the files the account changed.
pub fn languages(changes: &BTreeMap<&'static str, u32>) -> Vec<GitHubLanguageShare> {
    let total: u32 = changes.values().sum();
    if total == 0 {
        return Vec::new();
    }
    let mut shares = changes
        .iter()
        .map(|(language, files)| GitHubLanguageShare {
            language: (*language).to_owned(),
            percent: crate::analysis::percent(*files, total),
            files_changed: *files,
        })
        .filter(|share| share.percent >= 1)
        .collect::<Vec<_>>();
    shares.sort_by_key(|share| std::cmp::Reverse(share.files_changed));
    shares.truncate(10);
    shares
}

/// Archetype shares with evidence: languages and technologies behind each.
pub fn archetypes(
    areas: &BTreeMap<Archetype, (u32, BTreeMap<&'static str, u32>)>,
    stack_by_area: &BTreeMap<Archetype, Vec<&str>>,
) -> Vec<GitHubArchetypeShare> {
    let total: u32 = areas.values().map(|(count, _)| count).sum();
    if total == 0 {
        return Vec::new();
    }
    let mut shares = areas
        .iter()
        .map(|(archetype, (count, languages))| {
            let mut top = languages.iter().collect::<Vec<_>>();
            top.sort_by(|a, b| b.1.cmp(a.1));
            let mut evidence = top
                .iter()
                .take(2)
                .map(|(language, files)| {
                    format!(
                        "{language} — {} {}",
                        group(**files),
                        plural(
                            **files,
                            "изменённый файл",
                            "изменённых файла",
                            "изменённых файлов"
                        )
                    )
                })
                .collect::<Vec<_>>();
            if let Some(stack) = stack_by_area
                .get(archetype)
                .filter(|stack| !stack.is_empty())
            {
                evidence.push(stack.iter().take(5).copied().collect::<Vec<_>>().join(", "));
            }
            GitHubArchetypeShare {
                id: *archetype,
                label: label(*archetype).into(),
                percent: crate::analysis::percent(*count, total),
                evidence,
            }
        })
        .filter(|share| share.percent >= 5)
        .collect::<Vec<_>>();
    shares.sort_by(|a, b| b.percent.cmp(&a.percent).then(a.id.cmp(&b.id)));
    shares.truncate(4);
    shares
}

/// The stack categories that support an archetype in the evidence line.
/// The area a stack item is evidence for: storage and ORMs belong to the backend.
pub fn archetype_of(name: &str, category: StackCategory) -> Option<Archetype> {
    Some(match category {
        StackCategory::Backend => Archetype::Backend,
        StackCategory::Frontend => Archetype::Frontend,
        StackCategory::Infrastructure => Archetype::Platform,
        StackCategory::Data if !DATA_WORK.contains(&name) => Archetype::Backend,
        StackCategory::Ai if name == AGENT_TOOLING => return None,
        StackCategory::Ai | StackCategory::Data => Archetype::Data,
        StackCategory::Mobile => Archetype::Mobile,
        StackCategory::Tooling => Archetype::Systems,
        StackCategory::Language | StackCategory::Quality => return None,
    })
}

pub struct SummaryInput<'a> {
    pub archetypes: &'a [GitHubArchetypeShare],
    pub languages: &'a [GitHubLanguageShare],
    pub key_technologies: &'a [&'a str],
    pub led_repositories: u32,
    pub own_organizations: &'a [String],
    pub focus_topics: &'a [String],
    pub commits_last_year: u32,
}

/// One paragraph for the candidate (with organization names) and one for employers (without).
pub fn summaries(input: &SummaryInput<'_>) -> (String, String) {
    let roles = match input.archetypes {
        [] => "Публичная разработка".to_owned(),
        [only] => format!("Основное направление — {}", phrase(only.id)),
        [first, second, ..] => format!(
            "Основные направления — {} ({}%) и {} ({}%)",
            phrase(first.id),
            first.percent,
            phrase(second.id),
            second.percent
        ),
    };
    let languages = match input.languages {
        [] => String::new(),
        [only] => format!("пишет на {} ({}% изменений)", only.language, only.percent),
        [first, second, ..] => format!(
            "пишет в основном на {} ({}%) и {} ({}%)",
            first.language, first.percent, second.language, second.percent
        ),
    };
    let mut sentence = roles;
    if !languages.is_empty() {
        sentence.push_str("; ");
        sentence.push_str(&languages);
    }
    sentence.push('.');
    let mut public = sentence.clone();
    let mut private = sentence;
    if input.led_repositories > 0 {
        let projects = format!(
            " Ведёт {} {}",
            input.led_repositories,
            plural(input.led_repositories, "проект", "проекта", "проектов")
        );
        let count = input.own_organizations.len();
        let organizations = if count == 0 {
            String::new()
        } else {
            format!(
                " в {count} {}",
                plural(count as u32, "организации", "организациях", "организациях")
            )
        };
        public.push_str(&format!("{projects}{organizations}."));
        let names = if count == 0 {
            String::new()
        } else {
            format!(" ({})", input.own_organizations.join(", "))
        };
        private.push_str(&format!("{projects}{organizations}{names}."));
    }
    if input.commits_last_year > 0 {
        let commits = |value: u32| {
            plural(
                value,
                "публичный коммит",
                "публичных коммита",
                "публичных коммитов",
            )
        };
        private.push_str(&format!(
            " За год — {} {}.",
            group(input.commits_last_year),
            commits(input.commits_last_year)
        ));
        let rounded = approx_value(input.commits_last_year);
        public.push_str(&format!(
            " За год — {}{} {}.",
            if rounded == input.commits_last_year {
                ""
            } else {
                "около "
            },
            group(rounded),
            commits(rounded)
        ));
    }
    if !input.key_technologies.is_empty() {
        let technologies = format!(
            " Ключевые технологии: {}.",
            input.key_technologies.join(", ")
        );
        private.push_str(&technologies);
        public.push_str(&technologies);
    }
    if !input.focus_topics.is_empty() {
        private.push_str(&format!(
            " Темы проектов: {}.",
            input.focus_topics.join(", ")
        ));
    }
    (private, public)
}

fn plural<'a>(value: u32, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let tail = value % 100;
    if (11..=14).contains(&tail) {
        return many;
    }
    match value % 10 {
        1 => one,
        2..=4 => few,
        _ => many,
    }
}

fn group(value: u32) -> String {
    let digits = value.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push('\u{a0}');
        }
        grouped.push(digit);
    }
    grouped
}

/// Two significant figures, rounded down so a public figure never overstates.
fn approx_value(value: u32) -> u32 {
    if value < 100 {
        return value;
    }
    let magnitude = 10u32.pow(value.ilog10() - 1);
    value / magnitude * magnitude
}

/// Whether a repository topic describes the work rather than its license or the profile itself.
pub fn meaningful_topic(topic: &str) -> bool {
    const LICENSES: [&str; 8] = [
        "agpl",
        "gpl",
        "lgpl",
        "mit",
        "apache",
        "bsd",
        "mpl",
        "unlicense",
    ];
    let licensed = LICENSES.iter().any(|license| {
        topic == *license
            || topic
                .strip_prefix(license)
                .is_some_and(|rest| rest.starts_with(['-', '2', '3', 'v']))
    });
    !licensed
        && !topic.contains("license")
        && !matches!(
            topic,
            "about-me"
                | "aboutme"
                | "readme"
                | "profile"
                | "github"
                | "portfolio"
                | "hacktoberfest"
                | "open-source"
                | "opensource"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_attributed_by_language_path_and_stack() {
        let react = RepositoryKind {
            frontend: true,
            ..RepositoryKind::default()
        };
        let api = RepositoryKind {
            backend: true,
            ..RepositoryKind::default()
        };
        assert_eq!(area("src/App.tsx", api), Some(Archetype::Frontend));
        assert_eq!(area("src/server.ts", api), Some(Archetype::Backend));
        assert_eq!(area("src/store.ts", react), Some(Archetype::Frontend));
        assert_eq!(area("src/main.rs", api), Some(Archetype::Backend));
        assert_eq!(
            area("src/main.rs", RepositoryKind::default()),
            Some(Archetype::Systems)
        );
        assert_eq!(area("deploy/app.yaml", api), Some(Archetype::Platform));
        assert_eq!(
            area(".github/workflows/ci.yml", api),
            Some(Archetype::Platform)
        );
        assert_eq!(area("Dockerfile", api), Some(Archetype::Platform));
        assert_eq!(area("README.md", api), None);
        let ml_service = RepositoryKind {
            backend: true,
            data: true,
            ..RepositoryKind::default()
        };
        assert_eq!(
            area("app/api/routes.py", ml_service),
            Some(Archetype::Backend)
        );
        assert_eq!(area("models/train.py", ml_service), Some(Archetype::Data));
        assert_eq!(area("app/models.py", api), Some(Archetype::Backend));
        assert_eq!(area("analysis.ipynb", api), Some(Archetype::Data));
    }

    #[test]
    fn storage_and_agent_configuration_are_not_data_work() {
        assert_eq!(
            archetype_of("SQLAlchemy", StackCategory::Data),
            Some(Archetype::Backend)
        );
        assert_eq!(
            archetype_of("PyTorch", StackCategory::Ai),
            Some(Archetype::Data)
        );
        assert_eq!(archetype_of(AGENT_TOOLING, StackCategory::Ai), None);
    }

    #[test]
    fn summaries_name_organizations_only_for_the_candidate() {
        let archetypes = vec![
            GitHubArchetypeShare {
                id: Archetype::Systems,
                label: label(Archetype::Systems).into(),
                percent: 55,
                evidence: Vec::new(),
            },
            GitHubArchetypeShare {
                id: Archetype::Platform,
                label: label(Archetype::Platform).into(),
                percent: 30,
                evidence: Vec::new(),
            },
        ];
        let languages = vec![
            GitHubLanguageShare {
                language: "Rust".into(),
                percent: 60,
                files_changed: 600,
            },
            GitHubLanguageShare {
                language: "Python".into(),
                percent: 25,
                files_changed: 250,
            },
        ];
        let organizations = vec!["acme-labs".to_owned()];
        let (private, public) = summaries(&SummaryInput {
            archetypes: &archetypes,
            languages: &languages,
            key_technologies: &["Tokio", "Docker"],
            led_repositories: 12,
            own_organizations: &organizations,
            focus_topics: &["cli".into()],
            commits_last_year: 4321,
        });
        assert!(private.contains("acme-labs"));
        assert!(private.contains("4\u{a0}321"));
        assert!(!public.contains("acme-labs"));
        assert!(private.contains("4\u{a0}321 публичный коммит."));
        assert!(public.contains("около 4\u{a0}300 публичных коммитов"));
        assert!(public.contains("Ведёт 12 проектов в 1 организации"));
        assert!(public.contains("Rust (60%)"));
    }

    #[test]
    fn license_and_profile_topics_are_not_focus_topics() {
        for topic in [
            "agpl-3",
            "mit",
            "gpl-v3",
            "apache-2",
            "mit-license",
            "about-me",
        ] {
            assert!(!meaningful_topic(topic), "{topic}");
        }
        for topic in [
            "rust",
            "mcp",
            "ai-agents",
            "mitmproxy",
            "bsdiff",
            "developer-tools",
        ] {
            assert!(meaningful_topic(topic), "{topic}");
        }
    }
}
