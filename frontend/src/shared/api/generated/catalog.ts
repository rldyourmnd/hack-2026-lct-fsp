// Generated from backend/crates/domain/data/catalog.json by contracts/codegen. Do not edit.

export const CATALOG_VERSION = "catalog-v2" as const;

export const GRADES = [
  {
    "id": "intern",
    "label": "Стажёр",
    "description": "Учится и выполняет задачи под руководством"
  },
  {
    "id": "junior",
    "label": "Джуниор",
    "description": "Самостоятельно решает типовые задачи"
  },
  {
    "id": "middle",
    "label": "Мидл",
    "description": "Самостоятельно ведёт задачи и модули"
  },
  {
    "id": "senior",
    "label": "Сеньор",
    "description": "Отвечает за архитектуру и качество системы"
  },
  {
    "id": "lead",
    "label": "Лид",
    "description": "Ведёт направление и людей, определяет технические решения"
  }
] as const;

export const SPECIALIZATIONS = [
  {
    "id": "backend",
    "label": "Бэкенд-разработка",
    "short": "Бэкенд",
    "role": "Бэкенд-разработчик",
    "group": "development",
    "testTopics": [
      "rust",
      "postgresql"
    ]
  },
  {
    "id": "frontend",
    "label": "Фронтенд-разработка",
    "short": "Фронтенд",
    "role": "Фронтенд-разработчик",
    "group": "development",
    "testTopics": [
      "typescript",
      "react"
    ]
  },
  {
    "id": "fullstack",
    "label": "Фулстек-разработка",
    "short": "Фулстек",
    "role": "Фулстек-разработчик",
    "group": "development",
    "testTopics": [
      "http_api",
      "sql"
    ]
  },
  {
    "id": "ios",
    "label": "iOS-разработка",
    "short": "iOS",
    "role": "iOS-разработчик",
    "group": "mobile",
    "testTopics": [
      "swift",
      "ios_platform"
    ]
  },
  {
    "id": "android",
    "label": "Android-разработка",
    "short": "Android",
    "role": "Android-разработчик",
    "group": "mobile",
    "testTopics": [
      "kotlin",
      "android_platform"
    ]
  },
  {
    "id": "mobile_crossplatform",
    "label": "Кросс-платформенная мобильная разработка",
    "short": "Кросс-платформа",
    "role": "Мобильный разработчик",
    "group": "mobile",
    "testTopics": [
      "flutter",
      "mobile_development"
    ]
  },
  {
    "id": "devops",
    "label": "DevOps и SRE",
    "short": "DevOps",
    "role": "DevOps-инженер",
    "group": "infrastructure",
    "testTopics": [
      "linux",
      "kubernetes"
    ]
  },
  {
    "id": "data_engineering",
    "label": "Инженерия данных",
    "short": "Data Engineering",
    "role": "Инженер данных",
    "group": "data",
    "testTopics": [
      "sql",
      "etl"
    ]
  },
  {
    "id": "data_science",
    "label": "Data Science и машинное обучение",
    "short": "Data Science",
    "role": "Data Scientist",
    "group": "data",
    "testTopics": [
      "python",
      "machine_learning"
    ]
  },
  {
    "id": "ml_engineering",
    "label": "ML-инженерия и MLOps",
    "short": "MLOps",
    "role": "ML-инженер",
    "group": "data",
    "testTopics": [
      "machine_learning",
      "mlops"
    ]
  },
  {
    "id": "qa_manual",
    "label": "Ручное тестирование",
    "short": "QA",
    "role": "Тестировщик",
    "group": "quality",
    "testTopics": [
      "test_design",
      "web_testing"
    ]
  },
  {
    "id": "qa_automation",
    "label": "Автоматизация тестирования",
    "short": "Автотесты",
    "role": "Инженер по автоматизации тестирования",
    "group": "quality",
    "testTopics": [
      "test_design",
      "test_automation"
    ]
  },
  {
    "id": "security",
    "label": "Информационная безопасность",
    "short": "ИБ",
    "role": "Специалист по информационной безопасности",
    "group": "security",
    "testTopics": [
      "web_security",
      "network_security"
    ]
  },
  {
    "id": "embedded",
    "label": "Встраиваемые системы и IoT",
    "short": "Embedded",
    "role": "Embedded-разработчик",
    "group": "systems",
    "testTopics": [
      "c",
      "microcontrollers"
    ]
  },
  {
    "id": "systems",
    "label": "Системное программирование",
    "short": "Системное ПО",
    "role": "Системный программист",
    "group": "systems",
    "testTopics": [
      "cpp",
      "operating_systems"
    ]
  },
  {
    "id": "gamedev",
    "label": "Разработка игр",
    "short": "Геймдев",
    "role": "Разработчик игр",
    "group": "systems",
    "testTopics": [
      "csharp",
      "game_development"
    ]
  },
  {
    "id": "onec",
    "label": "Разработка на 1С",
    "short": "1С",
    "role": "Разработчик 1С",
    "group": "enterprise",
    "testTopics": [
      "onec_language",
      "onec_queries"
    ]
  },
  {
    "id": "erp",
    "label": "ERP и SAP",
    "short": "ERP",
    "role": "ERP-консультант и разработчик",
    "group": "enterprise",
    "testTopics": [
      "erp_systems",
      "abap"
    ]
  },
  {
    "id": "system_analysis",
    "label": "Системный анализ",
    "short": "Системный анализ",
    "role": "Системный аналитик",
    "group": "analysis",
    "testTopics": [
      "requirements",
      "uml_bpmn"
    ]
  },
  {
    "id": "business_analysis",
    "label": "Бизнес-анализ",
    "short": "Бизнес-анализ",
    "role": "Бизнес-аналитик",
    "group": "analysis",
    "testTopics": [
      "requirements",
      "business_processes"
    ]
  },
  {
    "id": "data_analytics",
    "label": "Аналитика данных и BI",
    "short": "Аналитика",
    "role": "Аналитик данных",
    "group": "analysis",
    "testTopics": [
      "sql",
      "statistics"
    ]
  },
  {
    "id": "dba",
    "label": "Администрирование баз данных",
    "short": "DBA",
    "role": "Администратор баз данных",
    "group": "infrastructure",
    "testTopics": [
      "sql",
      "database_administration"
    ]
  },
  {
    "id": "sysadmin",
    "label": "Системное и сетевое администрирование",
    "short": "Сисадмин",
    "role": "Системный администратор",
    "group": "infrastructure",
    "testTopics": [
      "linux",
      "computer_networks"
    ]
  },
  {
    "id": "design",
    "label": "UI/UX-дизайн",
    "short": "Дизайн",
    "role": "UI/UX-дизайнер",
    "group": "design",
    "testTopics": [
      "ui_design",
      "ux_design"
    ]
  },
  {
    "id": "management",
    "label": "Управление продуктом и проектами",
    "short": "Менеджмент",
    "role": "Менеджер продукта или проекта",
    "group": "management",
    "testTopics": [
      "agile",
      "project_management"
    ]
  }
] as const;

export const SPECIALIZATION_GROUPS = [
  {
    "id": "development",
    "label": "Веб и бэкенд"
  },
  {
    "id": "mobile",
    "label": "Мобильная разработка"
  },
  {
    "id": "infrastructure",
    "label": "Инфраструктура и администрирование"
  },
  {
    "id": "data",
    "label": "Данные и машинное обучение"
  },
  {
    "id": "quality",
    "label": "Тестирование"
  },
  {
    "id": "security",
    "label": "Безопасность"
  },
  {
    "id": "systems",
    "label": "Системная разработка и игры"
  },
  {
    "id": "enterprise",
    "label": "Корпоративные системы"
  },
  {
    "id": "analysis",
    "label": "Анализ"
  },
  {
    "id": "design",
    "label": "Дизайн"
  },
  {
    "id": "management",
    "label": "Управление"
  }
] as const;

export const SKILL_CATEGORIES = [
  {
    "id": "language",
    "label": "Языки программирования"
  },
  {
    "id": "backend",
    "label": "Бэкенд и API"
  },
  {
    "id": "frontend",
    "label": "Фронтенд"
  },
  {
    "id": "mobile",
    "label": "Мобильная разработка"
  },
  {
    "id": "database",
    "label": "Базы данных и хранилища"
  },
  {
    "id": "data",
    "label": "Данные и машинное обучение"
  },
  {
    "id": "infrastructure",
    "label": "Инфраструктура, DevOps и облака"
  },
  {
    "id": "quality",
    "label": "Тестирование и качество"
  },
  {
    "id": "security",
    "label": "Информационная безопасность"
  },
  {
    "id": "embedded",
    "label": "Встраиваемые системы и железо"
  },
  {
    "id": "gamedev",
    "label": "Разработка игр и графика"
  },
  {
    "id": "onec",
    "label": "1С"
  },
  {
    "id": "erp",
    "label": "ERP и корпоративные системы"
  },
  {
    "id": "analytics",
    "label": "Анализ и BI"
  },
  {
    "id": "design",
    "label": "Дизайн"
  },
  {
    "id": "management",
    "label": "Управление и процессы"
  },
  {
    "id": "practice",
    "label": "Инженерные практики"
  }
] as const;

export const SKILLS = [
  {
    "id": "python",
    "label": "Python",
    "category": "language"
  },
  {
    "id": "java",
    "label": "Java",
    "category": "language"
  },
  {
    "id": "javascript",
    "label": "JavaScript",
    "category": "language"
  },
  {
    "id": "typescript",
    "label": "TypeScript",
    "category": "language"
  },
  {
    "id": "csharp",
    "label": "C#",
    "category": "language"
  },
  {
    "id": "cpp",
    "label": "C++",
    "category": "language"
  },
  {
    "id": "c",
    "label": "C",
    "category": "language"
  },
  {
    "id": "go",
    "label": "Go",
    "category": "language"
  },
  {
    "id": "rust",
    "label": "Rust",
    "category": "language"
  },
  {
    "id": "kotlin",
    "label": "Kotlin",
    "category": "language"
  },
  {
    "id": "swift",
    "label": "Swift",
    "category": "language"
  },
  {
    "id": "objective_c",
    "label": "Objective-C",
    "category": "language"
  },
  {
    "id": "php",
    "label": "PHP",
    "category": "language"
  },
  {
    "id": "ruby",
    "label": "Ruby",
    "category": "language"
  },
  {
    "id": "scala",
    "label": "Scala",
    "category": "language"
  },
  {
    "id": "elixir",
    "label": "Elixir",
    "category": "language"
  },
  {
    "id": "erlang",
    "label": "Erlang",
    "category": "language"
  },
  {
    "id": "haskell",
    "label": "Haskell",
    "category": "language"
  },
  {
    "id": "clojure",
    "label": "Clojure",
    "category": "language"
  },
  {
    "id": "fsharp",
    "label": "F#",
    "category": "language"
  },
  {
    "id": "ocaml",
    "label": "OCaml",
    "category": "language"
  },
  {
    "id": "dart",
    "label": "Dart",
    "category": "language"
  },
  {
    "id": "lua",
    "label": "Lua",
    "category": "language"
  },
  {
    "id": "perl",
    "label": "Perl",
    "category": "language"
  },
  {
    "id": "r",
    "label": "R",
    "category": "language"
  },
  {
    "id": "julia",
    "label": "Julia",
    "category": "language"
  },
  {
    "id": "matlab",
    "label": "MATLAB",
    "category": "language"
  },
  {
    "id": "sql",
    "label": "SQL",
    "category": "language"
  },
  {
    "id": "plsql",
    "label": "PL/SQL",
    "category": "language"
  },
  {
    "id": "tsql",
    "label": "T-SQL",
    "category": "language"
  },
  {
    "id": "onec_language",
    "label": "Встроенный язык 1С",
    "category": "language"
  },
  {
    "id": "abap",
    "label": "ABAP",
    "category": "language"
  },
  {
    "id": "delphi",
    "label": "Delphi и Object Pascal",
    "category": "language"
  },
  {
    "id": "cobol",
    "label": "COBOL",
    "category": "language"
  },
  {
    "id": "fortran",
    "label": "Fortran",
    "category": "language"
  },
  {
    "id": "assembly",
    "label": "Ассемблер",
    "category": "language"
  },
  {
    "id": "verilog",
    "label": "Verilog и SystemVerilog",
    "category": "language"
  },
  {
    "id": "vhdl",
    "label": "VHDL",
    "category": "language"
  },
  {
    "id": "solidity",
    "label": "Solidity",
    "category": "language"
  },
  {
    "id": "zig",
    "label": "Zig",
    "category": "language"
  },
  {
    "id": "groovy",
    "label": "Groovy",
    "category": "language"
  },
  {
    "id": "bash",
    "label": "Bash и shell",
    "category": "language"
  },
  {
    "id": "powershell",
    "label": "PowerShell",
    "category": "language"
  },
  {
    "id": "visual_basic",
    "label": "Visual Basic и VBA",
    "category": "language"
  },
  {
    "id": "http_api",
    "label": "HTTP и REST API",
    "category": "backend"
  },
  {
    "id": "grpc",
    "label": "gRPC и Protocol Buffers",
    "category": "backend"
  },
  {
    "id": "graphql",
    "label": "GraphQL",
    "category": "backend"
  },
  {
    "id": "websocket",
    "label": "WebSocket",
    "category": "backend"
  },
  {
    "id": "spring",
    "label": "Spring и Spring Boot",
    "category": "backend"
  },
  {
    "id": "quarkus",
    "label": "Quarkus",
    "category": "backend"
  },
  {
    "id": "django",
    "label": "Django",
    "category": "backend"
  },
  {
    "id": "fastapi",
    "label": "FastAPI",
    "category": "backend"
  },
  {
    "id": "flask",
    "label": "Flask",
    "category": "backend"
  },
  {
    "id": "nodejs",
    "label": "Node.js",
    "category": "backend"
  },
  {
    "id": "express",
    "label": "Express",
    "category": "backend"
  },
  {
    "id": "nestjs",
    "label": "NestJS",
    "category": "backend"
  },
  {
    "id": "aspnet",
    "label": "ASP.NET Core",
    "category": "backend"
  },
  {
    "id": "entity_framework",
    "label": "Entity Framework",
    "category": "backend"
  },
  {
    "id": "laravel",
    "label": "Laravel",
    "category": "backend"
  },
  {
    "id": "symfony",
    "label": "Symfony",
    "category": "backend"
  },
  {
    "id": "rails",
    "label": "Ruby on Rails",
    "category": "backend"
  },
  {
    "id": "gin",
    "label": "Gin и Echo",
    "category": "backend"
  },
  {
    "id": "actix",
    "label": "Actix и Axum",
    "category": "backend"
  },
  {
    "id": "tokio",
    "label": "Tokio",
    "category": "backend"
  },
  {
    "id": "ktor",
    "label": "Ktor",
    "category": "backend"
  },
  {
    "id": "phoenix",
    "label": "Phoenix",
    "category": "backend"
  },
  {
    "id": "hibernate",
    "label": "Hibernate и JPA",
    "category": "backend"
  },
  {
    "id": "sqlalchemy",
    "label": "SQLAlchemy",
    "category": "backend"
  },
  {
    "id": "kafka",
    "label": "Apache Kafka",
    "category": "backend"
  },
  {
    "id": "rabbitmq",
    "label": "RabbitMQ",
    "category": "backend"
  },
  {
    "id": "nats",
    "label": "NATS",
    "category": "backend"
  },
  {
    "id": "celery",
    "label": "Celery",
    "category": "backend"
  },
  {
    "id": "microservices",
    "label": "Микросервисная архитектура",
    "category": "backend"
  },
  {
    "id": "html_css",
    "label": "HTML и CSS",
    "category": "frontend"
  },
  {
    "id": "react",
    "label": "React",
    "category": "frontend"
  },
  {
    "id": "vue",
    "label": "Vue",
    "category": "frontend"
  },
  {
    "id": "angular",
    "label": "Angular",
    "category": "frontend"
  },
  {
    "id": "svelte",
    "label": "Svelte",
    "category": "frontend"
  },
  {
    "id": "nextjs",
    "label": "Next.js",
    "category": "frontend"
  },
  {
    "id": "nuxt",
    "label": "Nuxt",
    "category": "frontend"
  },
  {
    "id": "redux",
    "label": "Redux и управление состоянием",
    "category": "frontend"
  },
  {
    "id": "tanstack",
    "label": "TanStack Query и Router",
    "category": "frontend"
  },
  {
    "id": "tailwind",
    "label": "Tailwind CSS",
    "category": "frontend"
  },
  {
    "id": "sass",
    "label": "Sass и CSS-модули",
    "category": "frontend"
  },
  {
    "id": "webpack",
    "label": "Webpack",
    "category": "frontend"
  },
  {
    "id": "vite",
    "label": "Vite",
    "category": "frontend"
  },
  {
    "id": "storybook",
    "label": "Storybook",
    "category": "frontend"
  },
  {
    "id": "threejs",
    "label": "Three.js и WebGL",
    "category": "frontend"
  },
  {
    "id": "web_accessibility",
    "label": "Доступность веб-интерфейсов",
    "category": "frontend"
  },
  {
    "id": "web_performance",
    "label": "Производительность веба",
    "category": "frontend"
  },
  {
    "id": "electron",
    "label": "Electron и Tauri",
    "category": "frontend"
  },
  {
    "id": "ios_platform",
    "label": "iOS SDK",
    "category": "mobile"
  },
  {
    "id": "swiftui",
    "label": "SwiftUI",
    "category": "mobile"
  },
  {
    "id": "uikit",
    "label": "UIKit",
    "category": "mobile"
  },
  {
    "id": "android_platform",
    "label": "Android SDK",
    "category": "mobile"
  },
  {
    "id": "jetpack_compose",
    "label": "Jetpack Compose",
    "category": "mobile"
  },
  {
    "id": "flutter",
    "label": "Flutter",
    "category": "mobile"
  },
  {
    "id": "react_native",
    "label": "React Native",
    "category": "mobile"
  },
  {
    "id": "kotlin_multiplatform",
    "label": "Kotlin Multiplatform",
    "category": "mobile"
  },
  {
    "id": "mobile_development",
    "label": "Основы мобильной разработки",
    "category": "mobile"
  },
  {
    "id": "postgresql",
    "label": "PostgreSQL",
    "category": "database"
  },
  {
    "id": "mysql",
    "label": "MySQL и MariaDB",
    "category": "database"
  },
  {
    "id": "oracle_db",
    "label": "Oracle Database",
    "category": "database"
  },
  {
    "id": "mssql",
    "label": "Microsoft SQL Server",
    "category": "database"
  },
  {
    "id": "sqlite",
    "label": "SQLite",
    "category": "database"
  },
  {
    "id": "mongodb",
    "label": "MongoDB",
    "category": "database"
  },
  {
    "id": "redis",
    "label": "Redis",
    "category": "database"
  },
  {
    "id": "cassandra",
    "label": "Cassandra и ScyllaDB",
    "category": "database"
  },
  {
    "id": "clickhouse",
    "label": "ClickHouse",
    "category": "database"
  },
  {
    "id": "elasticsearch",
    "label": "Elasticsearch и OpenSearch",
    "category": "database"
  },
  {
    "id": "neo4j",
    "label": "Neo4j и графовые БД",
    "category": "database"
  },
  {
    "id": "greenplum",
    "label": "Greenplum",
    "category": "database"
  },
  {
    "id": "tarantool",
    "label": "Tarantool",
    "category": "database"
  },
  {
    "id": "database_administration",
    "label": "Администрирование СУБД",
    "category": "database"
  },
  {
    "id": "data_modeling",
    "label": "Моделирование данных",
    "category": "database"
  },
  {
    "id": "etl",
    "label": "ETL и оркестрация конвейеров",
    "category": "data"
  },
  {
    "id": "spark",
    "label": "Apache Spark",
    "category": "data"
  },
  {
    "id": "airflow",
    "label": "Apache Airflow",
    "category": "data"
  },
  {
    "id": "dbt",
    "label": "dbt",
    "category": "data"
  },
  {
    "id": "hadoop",
    "label": "Hadoop и HDFS",
    "category": "data"
  },
  {
    "id": "flink",
    "label": "Apache Flink",
    "category": "data"
  },
  {
    "id": "pandas",
    "label": "pandas и Polars",
    "category": "data"
  },
  {
    "id": "numpy",
    "label": "NumPy и SciPy",
    "category": "data"
  },
  {
    "id": "statistics",
    "label": "Статистика и теория вероятностей",
    "category": "data"
  },
  {
    "id": "machine_learning",
    "label": "Машинное обучение",
    "category": "data"
  },
  {
    "id": "scikit_learn",
    "label": "scikit-learn",
    "category": "data"
  },
  {
    "id": "pytorch",
    "label": "PyTorch",
    "category": "data"
  },
  {
    "id": "tensorflow",
    "label": "TensorFlow и Keras",
    "category": "data"
  },
  {
    "id": "gradient_boosting",
    "label": "XGBoost, LightGBM и CatBoost",
    "category": "data"
  },
  {
    "id": "computer_vision",
    "label": "Компьютерное зрение",
    "category": "data"
  },
  {
    "id": "nlp",
    "label": "Обработка естественного языка",
    "category": "data"
  },
  {
    "id": "llm",
    "label": "LLM и генеративный ИИ",
    "category": "data"
  },
  {
    "id": "langchain",
    "label": "LangChain и LlamaIndex",
    "category": "data"
  },
  {
    "id": "mlops",
    "label": "MLOps",
    "category": "data"
  },
  {
    "id": "mlflow",
    "label": "MLflow",
    "category": "data"
  },
  {
    "id": "jupyter",
    "label": "Jupyter",
    "category": "data"
  },
  {
    "id": "linux",
    "label": "Linux",
    "category": "infrastructure"
  },
  {
    "id": "docker",
    "label": "Docker",
    "category": "infrastructure"
  },
  {
    "id": "kubernetes",
    "label": "Kubernetes",
    "category": "infrastructure"
  },
  {
    "id": "helm",
    "label": "Helm",
    "category": "infrastructure"
  },
  {
    "id": "terraform",
    "label": "Terraform",
    "category": "infrastructure"
  },
  {
    "id": "ansible",
    "label": "Ansible",
    "category": "infrastructure"
  },
  {
    "id": "ci_cd",
    "label": "CI/CD",
    "category": "infrastructure"
  },
  {
    "id": "github_actions",
    "label": "GitHub Actions",
    "category": "infrastructure"
  },
  {
    "id": "gitlab_ci",
    "label": "GitLab CI",
    "category": "infrastructure"
  },
  {
    "id": "jenkins",
    "label": "Jenkins",
    "category": "infrastructure"
  },
  {
    "id": "argocd",
    "label": "Argo CD и GitOps",
    "category": "infrastructure"
  },
  {
    "id": "nginx",
    "label": "Nginx",
    "category": "infrastructure"
  },
  {
    "id": "prometheus",
    "label": "Prometheus и Grafana",
    "category": "infrastructure"
  },
  {
    "id": "logging",
    "label": "ELK и сбор логов",
    "category": "infrastructure"
  },
  {
    "id": "observability",
    "label": "Наблюдаемость и SRE",
    "category": "infrastructure"
  },
  {
    "id": "aws",
    "label": "AWS",
    "category": "infrastructure"
  },
  {
    "id": "gcp",
    "label": "Google Cloud",
    "category": "infrastructure"
  },
  {
    "id": "azure",
    "label": "Microsoft Azure",
    "category": "infrastructure"
  },
  {
    "id": "yandex_cloud",
    "label": "Yandex Cloud",
    "category": "infrastructure"
  },
  {
    "id": "openstack",
    "label": "OpenStack",
    "category": "infrastructure"
  },
  {
    "id": "virtualization",
    "label": "Виртуализация (KVM, VMware)",
    "category": "infrastructure"
  },
  {
    "id": "service_mesh",
    "label": "Istio и service mesh",
    "category": "infrastructure"
  },
  {
    "id": "computer_networks",
    "label": "Компьютерные сети",
    "category": "infrastructure"
  },
  {
    "id": "windows_server",
    "label": "Windows Server и Active Directory",
    "category": "infrastructure"
  },
  {
    "id": "network_equipment",
    "label": "Сетевое оборудование (Cisco, MikroTik)",
    "category": "infrastructure"
  },
  {
    "id": "test_design",
    "label": "Тест-дизайн",
    "category": "quality"
  },
  {
    "id": "web_testing",
    "label": "Тестирование веб-приложений",
    "category": "quality"
  },
  {
    "id": "api_testing",
    "label": "Тестирование API",
    "category": "quality"
  },
  {
    "id": "mobile_testing",
    "label": "Тестирование мобильных приложений",
    "category": "quality"
  },
  {
    "id": "test_automation",
    "label": "Автоматизация тестирования",
    "category": "quality"
  },
  {
    "id": "selenium",
    "label": "Selenium",
    "category": "quality"
  },
  {
    "id": "playwright",
    "label": "Playwright",
    "category": "quality"
  },
  {
    "id": "cypress",
    "label": "Cypress",
    "category": "quality"
  },
  {
    "id": "pytest",
    "label": "pytest",
    "category": "quality"
  },
  {
    "id": "junit",
    "label": "JUnit и TestNG",
    "category": "quality"
  },
  {
    "id": "jest",
    "label": "Jest и Vitest",
    "category": "quality"
  },
  {
    "id": "appium",
    "label": "Appium",
    "category": "quality"
  },
  {
    "id": "postman",
    "label": "Postman",
    "category": "quality"
  },
  {
    "id": "load_testing",
    "label": "Нагрузочное тестирование (JMeter, k6)",
    "category": "quality"
  },
  {
    "id": "allure",
    "label": "Allure",
    "category": "quality"
  },
  {
    "id": "bug_tracking",
    "label": "Баг-трекинг и отчётность",
    "category": "quality"
  },
  {
    "id": "web_security",
    "label": "Безопасность веб-приложений (OWASP)",
    "category": "security"
  },
  {
    "id": "network_security",
    "label": "Сетевая безопасность",
    "category": "security"
  },
  {
    "id": "pentest",
    "label": "Тестирование на проникновение",
    "category": "security"
  },
  {
    "id": "appsec",
    "label": "Безопасная разработка и DevSecOps",
    "category": "security"
  },
  {
    "id": "siem",
    "label": "SIEM и мониторинг ИБ",
    "category": "security"
  },
  {
    "id": "cryptography",
    "label": "Криптография",
    "category": "security"
  },
  {
    "id": "iam",
    "label": "Управление доступом (IAM)",
    "category": "security"
  },
  {
    "id": "reverse_engineering",
    "label": "Реверс-инжиниринг",
    "category": "security"
  },
  {
    "id": "malware_analysis",
    "label": "Анализ вредоносного ПО",
    "category": "security"
  },
  {
    "id": "forensics",
    "label": "Компьютерная криминалистика",
    "category": "security"
  },
  {
    "id": "compliance",
    "label": "Требования ИБ и соответствие (152-ФЗ, ГОСТ)",
    "category": "security"
  },
  {
    "id": "microcontrollers",
    "label": "Микроконтроллеры (STM32, ESP32, AVR)",
    "category": "embedded"
  },
  {
    "id": "arduino",
    "label": "Arduino",
    "category": "embedded"
  },
  {
    "id": "rtos",
    "label": "RTOS (FreeRTOS, Zephyr)",
    "category": "embedded"
  },
  {
    "id": "embedded_linux",
    "label": "Embedded Linux и Yocto",
    "category": "embedded"
  },
  {
    "id": "fpga",
    "label": "FPGA",
    "category": "embedded"
  },
  {
    "id": "plc",
    "label": "ПЛК и промышленная автоматизация",
    "category": "embedded"
  },
  {
    "id": "iot",
    "label": "Интернет вещей",
    "category": "embedded"
  },
  {
    "id": "hardware_interfaces",
    "label": "Интерфейсы (SPI, I2C, UART, CAN)",
    "category": "embedded"
  },
  {
    "id": "operating_systems",
    "label": "Операционные системы",
    "category": "embedded"
  },
  {
    "id": "compilers",
    "label": "Компиляторы и языковые инструменты",
    "category": "embedded"
  },
  {
    "id": "unity",
    "label": "Unity",
    "category": "gamedev"
  },
  {
    "id": "unreal",
    "label": "Unreal Engine",
    "category": "gamedev"
  },
  {
    "id": "godot",
    "label": "Godot",
    "category": "gamedev"
  },
  {
    "id": "game_development",
    "label": "Основы разработки игр",
    "category": "gamedev"
  },
  {
    "id": "opengl",
    "label": "OpenGL",
    "category": "gamedev"
  },
  {
    "id": "vulkan",
    "label": "Vulkan и DirectX",
    "category": "gamedev"
  },
  {
    "id": "shaders",
    "label": "Шейдеры",
    "category": "gamedev"
  },
  {
    "id": "onec_enterprise",
    "label": "1С:Предприятие 8",
    "category": "onec"
  },
  {
    "id": "onec_queries",
    "label": "Язык запросов 1С",
    "category": "onec"
  },
  {
    "id": "onec_bsp",
    "label": "БСП (Библиотека стандартных подсистем)",
    "category": "onec"
  },
  {
    "id": "onec_skd",
    "label": "СКД (Система компоновки данных)",
    "category": "onec"
  },
  {
    "id": "onec_forms",
    "label": "Управляемые формы 1С",
    "category": "onec"
  },
  {
    "id": "onec_erp",
    "label": "1С:ERP",
    "category": "onec"
  },
  {
    "id": "onec_ut",
    "label": "1С:Управление торговлей",
    "category": "onec"
  },
  {
    "id": "onec_zup",
    "label": "1С:Зарплата и управление персоналом",
    "category": "onec"
  },
  {
    "id": "onec_accounting",
    "label": "1С:Бухгалтерия",
    "category": "onec"
  },
  {
    "id": "onec_document_flow",
    "label": "1С:Документооборот",
    "category": "onec"
  },
  {
    "id": "onec_integration",
    "label": "Обмены и интеграция 1С (КД, EnterpriseData, HTTP-сервисы)",
    "category": "onec"
  },
  {
    "id": "onec_edt",
    "label": "1C:EDT",
    "category": "onec"
  },
  {
    "id": "onec_administration",
    "label": "Администрирование 1С",
    "category": "onec"
  },
  {
    "id": "bitrix",
    "label": "1С-Битрикс",
    "category": "onec"
  },
  {
    "id": "erp_systems",
    "label": "ERP-системы и процессы",
    "category": "erp"
  },
  {
    "id": "sap_s4hana",
    "label": "SAP S/4HANA",
    "category": "erp"
  },
  {
    "id": "sap_fiori",
    "label": "SAP Fiori и UI5",
    "category": "erp"
  },
  {
    "id": "sap_bw",
    "label": "SAP BW и BI",
    "category": "erp"
  },
  {
    "id": "dynamics",
    "label": "Microsoft Dynamics",
    "category": "erp"
  },
  {
    "id": "crm_systems",
    "label": "CRM-системы",
    "category": "erp"
  },
  {
    "id": "requirements",
    "label": "Требования и их управление",
    "category": "analytics"
  },
  {
    "id": "uml_bpmn",
    "label": "Моделирование: UML и BPMN",
    "category": "analytics"
  },
  {
    "id": "business_processes",
    "label": "Бизнес-процессы",
    "category": "analytics"
  },
  {
    "id": "api_design",
    "label": "Проектирование API и интеграций",
    "category": "analytics"
  },
  {
    "id": "excel",
    "label": "Excel и Google Таблицы",
    "category": "analytics"
  },
  {
    "id": "power_bi",
    "label": "Power BI",
    "category": "analytics"
  },
  {
    "id": "tableau",
    "label": "Tableau",
    "category": "analytics"
  },
  {
    "id": "superset",
    "label": "Superset и DataLens",
    "category": "analytics"
  },
  {
    "id": "ab_testing",
    "label": "A/B-тестирование",
    "category": "analytics"
  },
  {
    "id": "product_analytics",
    "label": "Продуктовая аналитика",
    "category": "analytics"
  },
  {
    "id": "web_analytics",
    "label": "Веб-аналитика (Яндекс Метрика, GA)",
    "category": "analytics"
  },
  {
    "id": "ui_design",
    "label": "UI-дизайн",
    "category": "design"
  },
  {
    "id": "ux_design",
    "label": "UX-исследования и проектирование",
    "category": "design"
  },
  {
    "id": "figma",
    "label": "Figma",
    "category": "design"
  },
  {
    "id": "design_systems",
    "label": "Дизайн-системы",
    "category": "design"
  },
  {
    "id": "prototyping",
    "label": "Прототипирование",
    "category": "design"
  },
  {
    "id": "graphic_design",
    "label": "Графический дизайн (Photoshop, Illustrator)",
    "category": "design"
  },
  {
    "id": "motion_design",
    "label": "Моушн-дизайн",
    "category": "design"
  },
  {
    "id": "agile",
    "label": "Agile, Scrum и Kanban",
    "category": "management"
  },
  {
    "id": "project_management",
    "label": "Управление проектами",
    "category": "management"
  },
  {
    "id": "product_management",
    "label": "Управление продуктом",
    "category": "management"
  },
  {
    "id": "team_leadership",
    "label": "Руководство командой",
    "category": "management"
  },
  {
    "id": "jira",
    "label": "Jira и Confluence",
    "category": "management"
  },
  {
    "id": "git",
    "label": "Git",
    "category": "practice"
  },
  {
    "id": "algorithms",
    "label": "Алгоритмы и структуры данных",
    "category": "practice"
  },
  {
    "id": "system_design",
    "label": "Проектирование систем",
    "category": "practice"
  },
  {
    "id": "oop",
    "label": "ООП и паттерны проектирования",
    "category": "practice"
  },
  {
    "id": "functional_programming",
    "label": "Функциональное программирование",
    "category": "practice"
  },
  {
    "id": "code_review",
    "label": "Код-ревью",
    "category": "practice"
  },
  {
    "id": "technical_writing",
    "label": "Техническая документация",
    "category": "practice"
  }
] as const;

export const GRADE_IDS = [
  "intern",
  "junior",
  "middle",
  "senior",
  "lead"
] as const;

export const SPECIALIZATION_IDS = [
  "backend",
  "frontend",
  "fullstack",
  "ios",
  "android",
  "mobile_crossplatform",
  "devops",
  "data_engineering",
  "data_science",
  "ml_engineering",
  "qa_manual",
  "qa_automation",
  "security",
  "embedded",
  "systems",
  "gamedev",
  "onec",
  "erp",
  "system_analysis",
  "business_analysis",
  "data_analytics",
  "dba",
  "sysadmin",
  "design",
  "management"
] as const;

export const SKILL_IDS = [
  "python",
  "java",
  "javascript",
  "typescript",
  "csharp",
  "cpp",
  "c",
  "go",
  "rust",
  "kotlin",
  "swift",
  "objective_c",
  "php",
  "ruby",
  "scala",
  "elixir",
  "erlang",
  "haskell",
  "clojure",
  "fsharp",
  "ocaml",
  "dart",
  "lua",
  "perl",
  "r",
  "julia",
  "matlab",
  "sql",
  "plsql",
  "tsql",
  "onec_language",
  "abap",
  "delphi",
  "cobol",
  "fortran",
  "assembly",
  "verilog",
  "vhdl",
  "solidity",
  "zig",
  "groovy",
  "bash",
  "powershell",
  "visual_basic",
  "http_api",
  "grpc",
  "graphql",
  "websocket",
  "spring",
  "quarkus",
  "django",
  "fastapi",
  "flask",
  "nodejs",
  "express",
  "nestjs",
  "aspnet",
  "entity_framework",
  "laravel",
  "symfony",
  "rails",
  "gin",
  "actix",
  "tokio",
  "ktor",
  "phoenix",
  "hibernate",
  "sqlalchemy",
  "kafka",
  "rabbitmq",
  "nats",
  "celery",
  "microservices",
  "html_css",
  "react",
  "vue",
  "angular",
  "svelte",
  "nextjs",
  "nuxt",
  "redux",
  "tanstack",
  "tailwind",
  "sass",
  "webpack",
  "vite",
  "storybook",
  "threejs",
  "web_accessibility",
  "web_performance",
  "electron",
  "ios_platform",
  "swiftui",
  "uikit",
  "android_platform",
  "jetpack_compose",
  "flutter",
  "react_native",
  "kotlin_multiplatform",
  "mobile_development",
  "postgresql",
  "mysql",
  "oracle_db",
  "mssql",
  "sqlite",
  "mongodb",
  "redis",
  "cassandra",
  "clickhouse",
  "elasticsearch",
  "neo4j",
  "greenplum",
  "tarantool",
  "database_administration",
  "data_modeling",
  "etl",
  "spark",
  "airflow",
  "dbt",
  "hadoop",
  "flink",
  "pandas",
  "numpy",
  "statistics",
  "machine_learning",
  "scikit_learn",
  "pytorch",
  "tensorflow",
  "gradient_boosting",
  "computer_vision",
  "nlp",
  "llm",
  "langchain",
  "mlops",
  "mlflow",
  "jupyter",
  "linux",
  "docker",
  "kubernetes",
  "helm",
  "terraform",
  "ansible",
  "ci_cd",
  "github_actions",
  "gitlab_ci",
  "jenkins",
  "argocd",
  "nginx",
  "prometheus",
  "logging",
  "observability",
  "aws",
  "gcp",
  "azure",
  "yandex_cloud",
  "openstack",
  "virtualization",
  "service_mesh",
  "computer_networks",
  "windows_server",
  "network_equipment",
  "test_design",
  "web_testing",
  "api_testing",
  "mobile_testing",
  "test_automation",
  "selenium",
  "playwright",
  "cypress",
  "pytest",
  "junit",
  "jest",
  "appium",
  "postman",
  "load_testing",
  "allure",
  "bug_tracking",
  "web_security",
  "network_security",
  "pentest",
  "appsec",
  "siem",
  "cryptography",
  "iam",
  "reverse_engineering",
  "malware_analysis",
  "forensics",
  "compliance",
  "microcontrollers",
  "arduino",
  "rtos",
  "embedded_linux",
  "fpga",
  "plc",
  "iot",
  "hardware_interfaces",
  "operating_systems",
  "compilers",
  "unity",
  "unreal",
  "godot",
  "game_development",
  "opengl",
  "vulkan",
  "shaders",
  "onec_enterprise",
  "onec_queries",
  "onec_bsp",
  "onec_skd",
  "onec_forms",
  "onec_erp",
  "onec_ut",
  "onec_zup",
  "onec_accounting",
  "onec_document_flow",
  "onec_integration",
  "onec_edt",
  "onec_administration",
  "bitrix",
  "erp_systems",
  "sap_s4hana",
  "sap_fiori",
  "sap_bw",
  "dynamics",
  "crm_systems",
  "requirements",
  "uml_bpmn",
  "business_processes",
  "api_design",
  "excel",
  "power_bi",
  "tableau",
  "superset",
  "ab_testing",
  "product_analytics",
  "web_analytics",
  "ui_design",
  "ux_design",
  "figma",
  "design_systems",
  "prototyping",
  "graphic_design",
  "motion_design",
  "agile",
  "project_management",
  "product_management",
  "team_leadership",
  "jira",
  "git",
  "algorithms",
  "system_design",
  "oop",
  "functional_programming",
  "code_review",
  "technical_writing"
] as const;
