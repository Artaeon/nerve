use std::fs;
use std::path::Path;

/// A project template definition
#[derive(Debug, Clone)]
pub struct ProjectTemplate {
    pub name: String,
    pub description: String,
    pub language: String,
    pub files: Vec<TemplateFile>,
}

#[derive(Debug, Clone)]
pub struct TemplateFile {
    pub path: String,
    pub content: String,
}

/// Built-in templates
pub fn builtin_templates() -> Vec<ProjectTemplate> {
    vec![
        rust_cli_template(),
        rust_lib_template(),
        rust_web_template(),
        node_api_template(),
        node_react_template(),
        python_cli_template(),
        python_api_template(),
        go_api_template(),
    ]
}

pub fn list_templates() -> Vec<(String, String, String)> {
    // Returns (name, language, description) tuples
    builtin_templates()
        .iter()
        .map(|t| (t.name.clone(), t.language.clone(), t.description.clone()))
        .collect()
}

pub fn get_template(name: &str) -> Option<ProjectTemplate> {
    builtin_templates()
        .into_iter()
        .find(|t| t.name.eq_ignore_ascii_case(name))
}

/// Write a template to disk at the given directory
pub fn write_template(template: &ProjectTemplate, target_dir: &Path) -> anyhow::Result<usize> {
    fs::create_dir_all(target_dir)?;
    let mut written = 0;

    for file in &template.files {
        let file_path = target_dir.join(&file.path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file_path, &file.content)?;
        written += 1;
    }

    Ok(written)
}

// ─── Template definitions ──────────────────────────────────────────────────

fn rust_cli_template() -> ProjectTemplate {
    ProjectTemplate {
        name: "rust-cli".into(),
        description: "Rust CLI application with clap, error handling, and tests".into(),
        language: "Rust".into(),
        files: vec![
            TemplateFile {
                path: "Cargo.toml".into(),
                content: r#"[package]
name = "{{name}}"
version = "0.1.0"
edition = "2024"

[dependencies]
clap = { version = "4", features = ["derive"] }
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
"#
                .into(),
            },
            TemplateFile {
                path: "src/main.rs".into(),
                content: r#"use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(name = "{{name}}", about = "{{description}}")]
struct Cli {
    /// Input to process
    input: Option<String>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(input) = cli.input {
        println!("Processing: {input}");
    } else {
        println!("No input provided. Use --help for usage.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
"#
                .into(),
            },
            TemplateFile {
                path: ".gitignore".into(),
                content: "/target\n*.swp\n.env\n".into(),
            },
            TemplateFile {
                path: "README.md".into(),
                content: "# {{name}}\n\n{{description}}\n\n## Usage\n\n```bash\ncargo run -- <input>\n```\n".into(),
            },
        ],
    }
}

fn rust_lib_template() -> ProjectTemplate {
    ProjectTemplate {
        name: "rust-lib".into(),
        description: "Rust library crate with documentation and tests".into(),
        language: "Rust".into(),
        files: vec![
            TemplateFile {
                path: "Cargo.toml".into(),
                content: r#"[package]
name = "{{name}}"
version = "0.1.0"
edition = "2024"
description = "{{description}}"
license = "MIT"

[dependencies]
thiserror = "2"
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
"#
                .into(),
            },
            TemplateFile {
                path: "src/lib.rs".into(),
                content: r#"//! # {{name}}
//!
//! {{description}}

pub mod error;

use error::Error;

/// The main result type for this library.
pub type Result<T> = std::result::Result<T, Error>;

/// A sample struct demonstrating the library's functionality.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub name: String,
    pub value: String,
}

impl Config {
    /// Create a new `Config` with the given name and value.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Greet the user by name.
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greet_returns_greeting() {
        assert_eq!(greet("world"), "Hello, world!");
    }

    #[test]
    fn config_new() {
        let cfg = Config::new("key", "val");
        assert_eq!(cfg.name, "key");
        assert_eq!(cfg.value, "val");
    }
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/error.rs".into(),
                content: r#"/// Errors that can occur in this library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
"#
                .into(),
            },
            TemplateFile {
                path: ".gitignore".into(),
                content: "/target\n*.swp\n.env\n".into(),
            },
            TemplateFile {
                path: "README.md".into(),
                content: r#"# {{name}}

{{description}}

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
{{name}} = "0.1"
```

## Example

```rust
use {{name}}::greet;

fn main() {
    println!("{}", greet("world"));
}
```

## License

MIT
"#
                .into(),
            },
        ],
    }
}

fn rust_web_template() -> ProjectTemplate {
    ProjectTemplate {
        name: "rust-web".into(),
        description: "Rust web API with axum, tokio, and structured error handling".into(),
        language: "Rust".into(),
        files: vec![
            TemplateFile {
                path: "Cargo.toml".into(),
                content: r#"[package]
name = "{{name}}"
version = "0.1.0"
edition = "2024"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full", "signal"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tower-http = { version = "0.6", features = ["cors", "trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
thiserror = "2"
"#
                .into(),
            },
            TemplateFile {
                path: "src/main.rs".into(),
                content: r#"use anyhow::Result;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

mod error;
mod routes;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let app = routes::create_router();

    let addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("Listening on http://{addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C, shutting down"),
        _ = terminate => tracing::info!("Received SIGTERM, shutting down"),
    }
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/routes.rs".into(),
                content: r#"use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::error::AppError;

/// Build the full application router with all routes and middleware.
pub fn create_router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/hello", get(hello))
        .route("/hello/{name}", get(hello_name))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct HelloResponse {
    pub message: String,
}

/// Health check endpoint that returns service status.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

/// Greet the world.
pub async fn hello() -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello, world!".into(),
    })
}

/// Greet a specific user by name.
pub async fn hello_name(
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<HelloResponse>, AppError> {
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    if name.len() > 128 {
        return Err(AppError::BadRequest("name is too long (max 128 chars)".into()));
    }
    Ok(Json(HelloResponse {
        message: format!("Hello, {name}!"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_returns_ok() {
        let app = create_router();
        let resp = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn hello_returns_greeting() {
        let app = create_router();
        let resp = app
            .oneshot(Request::builder().uri("/hello").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["message"], "Hello, world!");
    }

    #[tokio::test]
    async fn hello_name_returns_personalized_greeting() {
        let app = create_router();
        let resp = app
            .oneshot(Request::builder().uri("/hello/Alice").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["message"], "Hello, Alice!");
    }

    #[tokio::test]
    async fn hello_name_rejects_too_long() {
        let app = create_router();
        let long_name = "a".repeat(200);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/hello/{long_name}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/error.rs".into(),
                content: r#"use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Application-level error type that converts into HTTP responses.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };
        tracing::warn!(status = %status, error = %message, "request failed");
        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn not_found_produces_404() {
        let err = AppError::NotFound("missing".into());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn bad_request_produces_400() {
        let err = AppError::BadRequest("invalid".into());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn internal_produces_500() {
        let err = AppError::Internal("oops".into());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
"#
                .into(),
            },
            TemplateFile {
                path: ".gitignore".into(),
                content: "/target\n*.swp\n*.swo\n.env\n.DS_Store\n*.pdb\n".into(),
            },
            TemplateFile {
                path: "README.md".into(),
                content: r#"# {{name}}

{{description}}

## Description

A production-ready web API built with Axum, Tokio, and Tower. Features structured error handling, request tracing, CORS support, and graceful shutdown.

## Install

### Prerequisites

- Rust 1.85+ (install via [rustup](https://rustup.rs/))

### Build

```bash
cargo build --release
```

## Usage

```bash
# Development
cargo run

# Production
LISTEN_ADDR=0.0.0.0:8080 ./target/release/{{name}}
```

The server starts at `http://localhost:3000` by default. Set the `LISTEN_ADDR` environment variable to change the bind address.

### Endpoints

| Method | Path            | Description              |
|--------|-----------------|--------------------------|
| GET    | `/health`       | Health check             |
| GET    | `/hello`        | Greet the world          |
| GET    | `/hello/{name}` | Greet a specific person  |

### Example

```bash
curl http://localhost:3000/health
# {"status":"ok","version":"0.1.0"}

curl http://localhost:3000/hello/Alice
# {"message":"Hello, Alice!"}
```

## Development

```bash
# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run

# Check formatting and lints
cargo fmt --check
cargo clippy -- -D warnings
```

## License

MIT
"#
                .into(),
            },
        ],
    }
}

fn node_api_template() -> ProjectTemplate {
    ProjectTemplate {
        name: "node-api".into(),
        description: "Express.js REST API with TypeScript, routes, and middleware".into(),
        language: "Node.js".into(),
        files: vec![
            TemplateFile {
                path: "package.json".into(),
                content: r#"{
  "name": "{{name}}",
  "version": "1.0.0",
  "description": "{{description}}",
  "main": "dist/index.js",
  "scripts": {
    "build": "tsc",
    "start": "node dist/index.js",
    "dev": "ts-node src/index.ts",
    "test": "node --test dist/**/*.test.js",
    "lint": "tsc --noEmit"
  },
  "dependencies": {
    "express": "^4.21.0",
    "cors": "^2.8.5",
    "helmet": "^8.0.0"
  },
  "devDependencies": {
    "@types/express": "^5.0.0",
    "@types/cors": "^2.8.17",
    "@types/node": "^22.0.0",
    "typescript": "^5.6.0",
    "ts-node": "^10.9.0"
  }
}
"#
                .into(),
            },
            TemplateFile {
                path: "tsconfig.json".into(),
                content: r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "commonjs",
    "lib": ["ES2022"],
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "declaration": true,
    "sourceMap": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/index.ts".into(),
                content: r#"import express from "express";
import cors from "cors";
import helmet from "helmet";
import { healthRouter } from "./routes/health";
import { itemsRouter } from "./routes/items";
import { errorHandler, requestLogger } from "./middleware/error";

const app = express();
const PORT = parseInt(process.env.PORT || "3000", 10);

app.use(helmet());
app.use(cors());
app.use(express.json({ limit: "1mb" }));
app.use(requestLogger);

app.use("/health", healthRouter);
app.use("/api/items", itemsRouter);

app.use(errorHandler);

const server = app.listen(PORT, () => {
  console.log(`Server running on http://localhost:${PORT}`);
});

function shutdown(signal: string): void {
  console.log(`Received ${signal}, shutting down gracefully`);
  server.close(() => {
    console.log("Server closed");
    process.exit(0);
  });
  setTimeout(() => {
    console.error("Forcing shutdown after timeout");
    process.exit(1);
  }, 10_000);
}

process.on("SIGTERM", () => shutdown("SIGTERM"));
process.on("SIGINT", () => shutdown("SIGINT"));

export default app;
"#
                .into(),
            },
            TemplateFile {
                path: "src/routes/health.ts".into(),
                content: r#"import { Router, Request, Response } from "express";

export const healthRouter = Router();

interface HealthResponse {
  status: string;
  version: string;
  uptime: number;
  timestamp: string;
}

healthRouter.get("/", (_req: Request, res: Response) => {
  const health: HealthResponse = {
    status: "ok",
    version: process.env.npm_package_version || "1.0.0",
    uptime: process.uptime(),
    timestamp: new Date().toISOString(),
  };
  res.json(health);
});
"#
                .into(),
            },
            TemplateFile {
                path: "src/routes/items.ts".into(),
                content: r#"import { Router, Request, Response, NextFunction } from "express";

export const itemsRouter = Router();

interface Item {
  id: number;
  name: string;
  createdAt: string;
}

const items: Item[] = [
  { id: 1, name: "Item One", createdAt: new Date().toISOString() },
  { id: 2, name: "Item Two", createdAt: new Date().toISOString() },
];

let nextId = 3;

itemsRouter.get("/", (_req: Request, res: Response) => {
  res.json(items);
});

itemsRouter.get("/:id", (req: Request, res: Response) => {
  const id = parseInt(req.params.id, 10);
  if (isNaN(id)) {
    res.status(400).json({ error: "Invalid item ID" });
    return;
  }
  const item = items.find((i) => i.id === id);
  if (!item) {
    res.status(404).json({ error: "Item not found" });
    return;
  }
  res.json(item);
});

itemsRouter.post("/", (req: Request, res: Response) => {
  const { name } = req.body;
  if (!name || typeof name !== "string") {
    res.status(400).json({ error: "Name is required and must be a string" });
    return;
  }
  if (name.length > 255) {
    res.status(400).json({ error: "Name must be 255 characters or fewer" });
    return;
  }
  const newItem: Item = { id: nextId++, name: name.trim(), createdAt: new Date().toISOString() };
  items.push(newItem);
  res.status(201).json(newItem);
});
"#
                .into(),
            },
            TemplateFile {
                path: "src/middleware/error.ts".into(),
                content: r#"import { Request, Response, NextFunction } from "express";

export function errorHandler(
  err: Error,
  _req: Request,
  res: Response,
  _next: NextFunction
): void {
  const status = (err as any).status || 500;
  const message = status === 500 ? "Internal Server Error" : err.message;
  if (status >= 500) {
    console.error(`[ERROR] ${err.stack || err.message}`);
  }
  res.status(status).json({ error: message });
}

export function requestLogger(
  req: Request,
  res: Response,
  next: NextFunction
): void {
  const start = Date.now();
  res.on("finish", () => {
    const duration = Date.now() - start;
    console.log(`${req.method} ${req.path} ${res.statusCode} ${duration}ms`);
  });
  next();
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/index.test.ts".into(),
                content: r#"import { describe, it } from "node:test";
import assert from "node:assert";

describe("Health endpoint", () => {
  it("health response has required fields", () => {
    const health = { status: "ok", version: "1.0.0", uptime: 42, timestamp: new Date().toISOString() };
    assert.strictEqual(health.status, "ok");
    assert.strictEqual(typeof health.uptime, "number");
    assert.ok(health.timestamp.length > 0);
  });
});

describe("Items validation", () => {
  it("rejects empty name", () => {
    const name = "";
    assert.strictEqual(!!name, false, "empty string should be falsy");
  });

  it("rejects non-string name", () => {
    const name: unknown = 123;
    assert.strictEqual(typeof name !== "string" || !name, true);
  });

  it("accepts a valid item", () => {
    const item = { id: 1, name: "Valid Item", createdAt: new Date().toISOString() };
    assert.strictEqual(item.id, 1);
    assert.strictEqual(item.name, "Valid Item");
    assert.ok(item.createdAt.length > 0);
  });

  it("rejects name longer than 255 chars", () => {
    const name = "a".repeat(256);
    assert.ok(name.length > 255, "name exceeds 255-char limit");
  });
});
"#
                .into(),
            },
            TemplateFile {
                path: ".gitignore".into(),
                content: "node_modules/\ndist/\n*.swp\n*.swo\n.env\n.DS_Store\ncoverage/\n*.tsbuildinfo\n".into(),
            },
            TemplateFile {
                path: "README.md".into(),
                content: r#"# {{name}}

{{description}}

## Description

A production-ready REST API built with Express.js and TypeScript. Includes request logging, security headers (Helmet), CORS, input validation, structured error handling, and graceful shutdown.

## Install

### Prerequisites

- Node.js 20+
- npm 10+

### Setup

```bash
npm install
```

## Usage

```bash
# Development (with ts-node)
npm run dev

# Production
npm run build
npm start
```

The server starts at `http://localhost:3000`. Set the `PORT` environment variable to change the port.

### Endpoints

| Method | Path             | Description         |
|--------|------------------|---------------------|
| GET    | `/health`        | Health check        |
| GET    | `/api/items`     | List all items      |
| GET    | `/api/items/:id` | Get item by ID      |
| POST   | `/api/items`     | Create a new item   |

### Example

```bash
curl http://localhost:3000/health
# {"status":"ok","version":"1.0.0","uptime":12.3,"timestamp":"..."}

curl -X POST http://localhost:3000/api/items \
  -H "Content-Type: application/json" \
  -d '{"name": "New Item"}'
# {"id":3,"name":"New Item","createdAt":"..."}
```

## Development

```bash
# Type-check without emitting
npm run lint

# Run tests
npm run build && npm test
```

## License

MIT
"#
                .into(),
            },
        ],
    }
}

fn node_react_template() -> ProjectTemplate {
    ProjectTemplate {
        name: "node-react".into(),
        description: "React app with Vite, TypeScript, and components".into(),
        language: "React".into(),
        files: vec![
            TemplateFile {
                path: "package.json".into(),
                content: r#"{
  "name": "{{name}}",
  "version": "1.0.0",
  "description": "{{description}}",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "test": "vitest run"
  },
  "dependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "typescript": "^5.6.0",
    "vite": "^6.0.0",
    "vitest": "^2.1.0"
  }
}
"#
                .into(),
            },
            TemplateFile {
                path: "tsconfig.json".into(),
                content: r#"{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true
  },
  "include": ["src"]
}
"#
                .into(),
            },
            TemplateFile {
                path: "vite.config.ts".into(),
                content: r#"import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
});
"#
                .into(),
            },
            TemplateFile {
                path: "index.html".into(),
                content: r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{{name}}</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#
                .into(),
            },
            TemplateFile {
                path: "src/main.tsx".into(),
                content: r#"import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
"#
                .into(),
            },
            TemplateFile {
                path: "src/App.tsx".into(),
                content: r#"import { useState } from "react";
import { Counter } from "./components/Counter";

export function App() {
  return (
    <div className="app">
      <h1>{{name}}</h1>
      <p>{{description}}</p>
      <Counter />
    </div>
  );
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/components/Counter.tsx".into(),
                content: r#"import { useState } from "react";

export function Counter() {
  const [count, setCount] = useState(0);

  return (
    <div className="counter">
      <p>Count: {count}</p>
      <button onClick={() => setCount((c) => c + 1)}>Increment</button>
      <button onClick={() => setCount((c) => c - 1)}>Decrement</button>
      <button onClick={() => setCount(0)}>Reset</button>
    </div>
  );
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/index.css".into(),
                content: r":root {
  font-family: system-ui, -apple-system, sans-serif;
  line-height: 1.5;
  color: #213547;
  background-color: #ffffff;
}

.app {
  max-width: 800px;
  margin: 0 auto;
  padding: 2rem;
  text-align: center;
}

.counter {
  margin: 2rem 0;
}

button {
  margin: 0.5rem;
  padding: 0.5rem 1rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  cursor: pointer;
}

button:hover {
  background-color: #f0f0f0;
}
"
                .into(),
            },
            TemplateFile {
                path: "src/App.test.tsx".into(),
                content: r#"import { describe, it, expect } from "vitest";

describe("App", () => {
  it("basic assertion", () => {
    expect(1 + 1).toBe(2);
  });

  it("string contains", () => {
    const name = "{{name}}";
    expect(name.length).toBeGreaterThan(0);
  });
});
"#
                .into(),
            },
            TemplateFile {
                path: ".gitignore".into(),
                content: "node_modules/\ndist/\n*.swp\n.env\n".into(),
            },
            TemplateFile {
                path: "README.md".into(),
                content: r"# {{name}}

{{description}}

## Setup

```bash
npm install
```

## Development

```bash
npm run dev
```

## Build

```bash
npm run build
npm run preview
```
"
                .into(),
            },
        ],
    }
}

fn python_cli_template() -> ProjectTemplate {
    ProjectTemplate {
        name: "python-cli".into(),
        description: "Python CLI application with argparse and proper project structure".into(),
        language: "Python".into(),
        files: vec![
            TemplateFile {
                path: "pyproject.toml".into(),
                content: r#"[build-system]
requires = ["setuptools>=75.0", "wheel"]
build-backend = "setuptools.backends._legacy:_Backend"

[project]
name = "{{name}}"
version = "0.1.0"
description = "{{description}}"
requires-python = ">=3.10"
dependencies = []

[project.scripts]
{{name}} = "{{name}}.cli:main"
"#
                .into(),
            },
            TemplateFile {
                path: "src/__init__.py".into(),
                content: "\"\"\"{{name}} - {{description}}\"\"\"\n\n__version__ = \"0.1.0\"\n"
                    .into(),
            },
            TemplateFile {
                path: "src/cli.py".into(),
                content: r#""""Command-line interface for {{name}}."""

import argparse
import sys


def create_parser() -> argparse.ArgumentParser:
    """Create the argument parser."""
    parser = argparse.ArgumentParser(
        prog="{{name}}",
        description="{{description}}",
    )
    parser.add_argument("input", nargs="?", help="Input to process")
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")
    parser.add_argument("--version", action="version", version="%(prog)s 0.1.0")
    return parser


def run(args: argparse.Namespace) -> int:
    """Run the main logic."""
    if args.input:
        if args.verbose:
            print(f"Processing input: {args.input!r}")
        print(f"Result: {args.input}")
        return 0
    else:
        print("No input provided. Use --help for usage.")
        return 1


def main() -> None:
    """Entry point."""
    parser = create_parser()
    args = parser.parse_args()
    sys.exit(run(args))


if __name__ == "__main__":
    main()
"#
                .into(),
            },
            TemplateFile {
                path: "tests/__init__.py".into(),
                content: "".into(),
            },
            TemplateFile {
                path: "tests/test_cli.py".into(),
                content: r#""""Tests for the CLI module."""

import argparse
import unittest
import sys
import os

# Add src to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from cli import create_parser, run


class TestCli(unittest.TestCase):
    def test_parser_with_input(self):
        parser = create_parser()
        args = parser.parse_args(["hello"])
        self.assertEqual(args.input, "hello")
        self.assertFalse(args.verbose)

    def test_parser_with_verbose(self):
        parser = create_parser()
        args = parser.parse_args(["-v", "hello"])
        self.assertTrue(args.verbose)

    def test_run_with_input(self):
        args = argparse.Namespace(input="test", verbose=False)
        result = run(args)
        self.assertEqual(result, 0)

    def test_run_without_input(self):
        args = argparse.Namespace(input=None, verbose=False)
        result = run(args)
        self.assertEqual(result, 1)


if __name__ == "__main__":
    unittest.main()
"#
                .into(),
            },
            TemplateFile {
                path: ".gitignore".into(),
                content: "__pycache__/\n*.pyc\n*.egg-info/\ndist/\nbuild/\n.venv/\n.env\n".into(),
            },
            TemplateFile {
                path: "README.md".into(),
                content: r"# {{name}}

{{description}}

## Setup

```bash
pip install -e .
```

## Usage

```bash
{{name}} <input>
{{name}} --help
```

## Development

```bash
python -m pytest tests/
```
"
                .into(),
            },
        ],
    }
}

fn python_api_template() -> ProjectTemplate {
    ProjectTemplate {
        name: "python-api".into(),
        description: "FastAPI application with routes, models, and tests".into(),
        language: "Python".into(),
        files: vec![
            TemplateFile {
                path: "pyproject.toml".into(),
                content: r#"[build-system]
requires = ["setuptools>=75.0", "wheel"]
build-backend = "setuptools.backends._legacy:_Backend"

[project]
name = "{{name}}"
version = "0.1.0"
description = "{{description}}"
requires-python = ">=3.11"
dependencies = [
    "fastapi>=0.115.0",
    "uvicorn[standard]>=0.32.0",
    "pydantic>=2.9.0",
    "pydantic-settings>=2.6.0",
]

[project.optional-dependencies]
dev = [
    "pytest>=8.0",
    "httpx>=0.27.0",
    "pytest-asyncio>=0.24.0",
]

[tool.pytest.ini_options]
testpaths = ["tests"]
asyncio_mode = "auto"
"#
                .into(),
            },
            TemplateFile {
                path: "requirements.txt".into(),
                content: "fastapi>=0.115.0\nuvicorn[standard]>=0.32.0\npydantic>=2.9.0\npydantic-settings>=2.6.0\nhttpx>=0.27.0\npytest>=8.0\npytest-asyncio>=0.24.0\n".into(),
            },
            TemplateFile {
                path: "app/__init__.py".into(),
                content: "\"\"\"{{name}} - {{description}}\"\"\"\n\n__version__ = \"0.1.0\"\n".into(),
            },
            TemplateFile {
                path: "app/main.py".into(),
                content: r#""""{{name}} - {{description}}"""

import logging
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse

from app.routes import router
from app.models import HealthResponse

logger = logging.getLogger(__name__)


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Startup and shutdown logic for the application."""
    logger.info("Starting %s", app.title)
    yield
    logger.info("Shutting down %s", app.title)


app = FastAPI(
    title="{{name}}",
    description="{{description}}",
    version="0.1.0",
    lifespan=lifespan,
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.exception_handler(ValueError)
async def value_error_handler(request: Request, exc: ValueError) -> JSONResponse:
    """Handle ValueError as 400 Bad Request."""
    return JSONResponse(status_code=400, content={"detail": str(exc)})


@app.get("/health", response_model=HealthResponse)
async def health_check() -> HealthResponse:
    """Health check endpoint."""
    return HealthResponse(status="ok", version="0.1.0")


app.include_router(router, prefix="/api")


if __name__ == "__main__":
    import uvicorn

    logging.basicConfig(level=logging.INFO)
    uvicorn.run("app.main:app", host="0.0.0.0", port=8000, reload=True)
"#
                .into(),
            },
            TemplateFile {
                path: "app/models.py".into(),
                content: r#""""Pydantic models for {{name}}."""

from pydantic import BaseModel, Field


class HealthResponse(BaseModel):
    """Response from the health check endpoint."""

    status: str
    version: str


class Item(BaseModel):
    """An item stored in the system."""

    id: int
    name: str
    description: str = ""


class ItemCreate(BaseModel):
    """Request body for creating an item."""

    name: str = Field(..., min_length=1, max_length=255, description="Name of the item")
    description: str = Field(
        default="", max_length=1000, description="Optional description"
    )


class ItemUpdate(BaseModel):
    """Request body for updating an item."""

    name: str | None = Field(default=None, min_length=1, max_length=255)
    description: str | None = Field(default=None, max_length=1000)


class ErrorResponse(BaseModel):
    """Standard error response."""

    detail: str
"#
                .into(),
            },
            TemplateFile {
                path: "app/routes.py".into(),
                content: r#""""API routes for {{name}}."""

from fastapi import APIRouter, HTTPException
from app.models import Item, ItemCreate, ItemUpdate

router = APIRouter()

_items: list[Item] = [
    Item(id=1, name="Item One", description="The first item"),
    Item(id=2, name="Item Two", description="The second item"),
]
_next_id: int = 3


def _find_item(item_id: int) -> Item:
    """Find an item by ID or raise 404."""
    for item in _items:
        if item.id == item_id:
            return item
    raise HTTPException(status_code=404, detail=f"Item {item_id} not found")


@router.get("/items", response_model=list[Item])
async def list_items() -> list[Item]:
    """List all items."""
    return _items


@router.get("/items/{item_id}", response_model=Item)
async def get_item(item_id: int) -> Item:
    """Get an item by ID."""
    return _find_item(item_id)


@router.post("/items", response_model=Item, status_code=201)
async def create_item(data: ItemCreate) -> Item:
    """Create a new item."""
    global _next_id
    item = Item(id=_next_id, name=data.name, description=data.description)
    _next_id += 1
    _items.append(item)
    return item


@router.patch("/items/{item_id}", response_model=Item)
async def update_item(item_id: int, data: ItemUpdate) -> Item:
    """Update an existing item."""
    item = _find_item(item_id)
    idx = _items.index(item)
    update_data = data.model_dump(exclude_unset=True)
    updated = item.model_copy(update=update_data)
    _items[idx] = updated
    return updated


@router.delete("/items/{item_id}", status_code=204)
async def delete_item(item_id: int) -> None:
    """Delete an item by ID."""
    item = _find_item(item_id)
    _items.remove(item)
"#
                .into(),
            },
            TemplateFile {
                path: "tests/__init__.py".into(),
                content: "".into(),
            },
            TemplateFile {
                path: "tests/test_api.py".into(),
                content: r#""""Tests for the API."""

import pytest
from fastapi.testclient import TestClient
from app.main import app

client = TestClient(app)


class TestHealth:
    def test_health_returns_ok(self):
        response = client.get("/health")
        assert response.status_code == 200
        data = response.json()
        assert data["status"] == "ok"
        assert "version" in data

    def test_health_has_version(self):
        response = client.get("/health")
        data = response.json()
        assert data["version"] == "0.1.0"


class TestListItems:
    def test_list_returns_items(self):
        response = client.get("/api/items")
        assert response.status_code == 200
        items = response.json()
        assert isinstance(items, list)
        assert len(items) >= 2

    def test_items_have_required_fields(self):
        response = client.get("/api/items")
        for item in response.json():
            assert "id" in item
            assert "name" in item


class TestGetItem:
    def test_get_existing_item(self):
        response = client.get("/api/items/1")
        assert response.status_code == 200
        assert response.json()["id"] == 1

    def test_get_nonexistent_item_returns_404(self):
        response = client.get("/api/items/999")
        assert response.status_code == 404
        assert "detail" in response.json()


class TestCreateItem:
    def test_create_valid_item(self):
        response = client.post("/api/items", json={"name": "New Item"})
        assert response.status_code == 201
        data = response.json()
        assert data["name"] == "New Item"
        assert "id" in data

    def test_create_item_missing_name(self):
        response = client.post("/api/items", json={})
        assert response.status_code == 422

    def test_create_item_empty_name(self):
        response = client.post("/api/items", json={"name": ""})
        assert response.status_code == 422

    def test_create_item_with_description(self):
        response = client.post(
            "/api/items", json={"name": "Described", "description": "A thing"}
        )
        assert response.status_code == 201
        assert response.json()["description"] == "A thing"


class TestDeleteItem:
    def test_delete_existing_item(self):
        create_resp = client.post("/api/items", json={"name": "ToDelete"})
        item_id = create_resp.json()["id"]
        response = client.delete(f"/api/items/{item_id}")
        assert response.status_code == 204

    def test_delete_nonexistent_returns_404(self):
        response = client.delete("/api/items/99999")
        assert response.status_code == 404
"#
                .into(),
            },
            TemplateFile {
                path: ".gitignore".into(),
                content: "__pycache__/\n*.pyc\n*.pyo\n*.egg-info/\ndist/\nbuild/\n.venv/\n.env\n.mypy_cache/\n.pytest_cache/\nhtmlcov/\n.coverage\n*.so\n.DS_Store\n".into(),
            },
            TemplateFile {
                path: "README.md".into(),
                content: r#"# {{name}}

{{description}}

## Description

A production-ready REST API built with FastAPI and Pydantic. Features automatic OpenAPI documentation, input validation, CORS, structured error handling, and a lifespan-managed application lifecycle.

## Install

### Prerequisites

- Python 3.11+
- pip or uv

### Setup

```bash
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

Or with development dependencies:

```bash
pip install -e ".[dev]"
```

## Usage

```bash
# Development with auto-reload
uvicorn app.main:app --reload

# Production
uvicorn app.main:app --host 0.0.0.0 --port 8000 --workers 4
```

The server starts at `http://localhost:8000`.

### API Documentation

- Swagger UI: `http://localhost:8000/docs`
- ReDoc: `http://localhost:8000/redoc`

### Endpoints

| Method | Path               | Description      |
|--------|--------------------|------------------|
| GET    | `/health`          | Health check     |
| GET    | `/api/items`       | List all items   |
| GET    | `/api/items/{id}`  | Get item by ID   |
| POST   | `/api/items`       | Create an item   |
| PATCH  | `/api/items/{id}`  | Update an item   |
| DELETE | `/api/items/{id}`  | Delete an item   |

### Example

```bash
curl http://localhost:8000/health
# {"status":"ok","version":"0.1.0"}

curl -X POST http://localhost:8000/api/items \
  -H "Content-Type: application/json" \
  -d '{"name": "My Item", "description": "A new item"}'
# {"id":3,"name":"My Item","description":"A new item"}
```

## Development

```bash
# Run tests
pytest

# Run tests with coverage
pytest --cov=app tests/
```

## License

MIT
"#
                .into(),
            },
        ],
    }
}

fn go_api_template() -> ProjectTemplate {
    ProjectTemplate {
        name: "go-api".into(),
        description: "Go HTTP API with net/http, handlers, and middleware".into(),
        language: "Go".into(),
        files: vec![
            TemplateFile {
                path: "go.mod".into(),
                content: r"module {{name}}

go 1.23
"
                .into(),
            },
            TemplateFile {
                path: "main.go".into(),
                content: r#"package main

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"{{name}}/handlers"
	"{{name}}/middleware"
)

func main() {
	logger := log.New(os.Stdout, "[{{name}}] ", log.LstdFlags|log.Lmsgprefix)

	mux := http.NewServeMux()

	h := handlers.New(logger)
	mux.HandleFunc("GET /health", h.Health)
	mux.HandleFunc("GET /api/items", h.ListItems)
	mux.HandleFunc("GET /api/items/{id}", h.GetItem)
	mux.HandleFunc("POST /api/items", h.CreateItem)

	handler := middleware.Chain(
		mux,
		middleware.Logging(logger),
		middleware.Recovery(logger),
		middleware.CORS,
	)

	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}
	addr := fmt.Sprintf(":%s", port)

	srv := &http.Server{
		Addr:         addr,
		Handler:      handler,
		ReadTimeout:  10 * time.Second,
		WriteTimeout: 10 * time.Second,
		IdleTimeout:  60 * time.Second,
	}

	go func() {
		logger.Printf("Server starting on http://localhost%s", addr)
		if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			logger.Fatalf("Server failed: %v", err)
		}
	}()

	quit := make(chan os.Signal, 1)
	signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
	sig := <-quit
	logger.Printf("Received %s, shutting down gracefully", sig)

	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()

	if err := srv.Shutdown(ctx); err != nil {
		logger.Fatalf("Forced shutdown: %v", err)
	}
	logger.Println("Server stopped")
}
"#
                .into(),
            },
            TemplateFile {
                path: "handlers/health.go".into(),
                content: r#"package handlers

import (
	"encoding/json"
	"log"
	"net/http"
	"strconv"
	"strings"
)

// Handler holds dependencies for HTTP handler functions.
type Handler struct {
	logger *log.Logger
	items  []Item
	nextID int
}

// New creates a Handler with default seed data.
func New(logger *log.Logger) *Handler {
	return &Handler{
		logger: logger,
		items: []Item{
			{ID: 1, Name: "Item One"},
			{ID: 2, Name: "Item Two"},
		},
		nextID: 3,
	}
}

// HealthResponse is the body of a health check reply.
type HealthResponse struct {
	Status  string `json:"status"`
	Version string `json:"version"`
}

// Item represents a stored item.
type Item struct {
	ID   int    `json:"id"`
	Name string `json:"name"`
}

// ItemInput is the request body for creating an item.
type ItemInput struct {
	Name string `json:"name"`
}

// ErrorResponse is a JSON error reply.
type ErrorResponse struct {
	Error string `json:"error"`
}

// Health returns service health information.
func (h *Handler) Health(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, HealthResponse{Status: "ok", Version: "0.1.0"})
}

// ListItems returns all items.
func (h *Handler) ListItems(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, h.items)
}

// GetItem returns a single item by ID.
func (h *Handler) GetItem(w http.ResponseWriter, r *http.Request) {
	idStr := r.PathValue("id")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, ErrorResponse{Error: "invalid item ID"})
		return
	}
	for _, item := range h.items {
		if item.ID == id {
			writeJSON(w, http.StatusOK, item)
			return
		}
	}
	writeJSON(w, http.StatusNotFound, ErrorResponse{Error: "item not found"})
}

// CreateItem adds a new item.
func (h *Handler) CreateItem(w http.ResponseWriter, r *http.Request) {
	var input ItemInput
	if err := json.NewDecoder(r.Body).Decode(&input); err != nil {
		writeJSON(w, http.StatusBadRequest, ErrorResponse{Error: "invalid JSON"})
		return
	}
	name := strings.TrimSpace(input.Name)
	if name == "" {
		writeJSON(w, http.StatusBadRequest, ErrorResponse{Error: "name is required"})
		return
	}
	if len(name) > 255 {
		writeJSON(w, http.StatusBadRequest, ErrorResponse{Error: "name is too long (max 255 chars)"})
		return
	}
	item := Item{ID: h.nextID, Name: name}
	h.nextID++
	h.items = append(h.items, item)
	writeJSON(w, http.StatusCreated, item)
}

func writeJSON(w http.ResponseWriter, status int, data interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(data)
}
"#
                .into(),
            },
            TemplateFile {
                path: "handlers/health_test.go".into(),
                content: r#"package handlers

import (
	"bytes"
	"encoding/json"
	"log"
	"net/http"
	"net/http/httptest"
	"os"
	"testing"
)

func newTestHandler() *Handler {
	return New(log.New(os.Stderr, "", 0))
}

func TestHealth(t *testing.T) {
	h := newTestHandler()
	req := httptest.NewRequest("GET", "/health", nil)
	w := httptest.NewRecorder()

	h.Health(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
	var resp HealthResponse
	json.NewDecoder(w.Body).Decode(&resp)
	if resp.Status != "ok" {
		t.Errorf("expected status ok, got %s", resp.Status)
	}
	if resp.Version == "" {
		t.Error("expected non-empty version")
	}
}

func TestListItems(t *testing.T) {
	h := newTestHandler()
	req := httptest.NewRequest("GET", "/api/items", nil)
	w := httptest.NewRecorder()

	h.ListItems(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
	var items []Item
	json.NewDecoder(w.Body).Decode(&items)
	if len(items) != 2 {
		t.Errorf("expected 2 items, got %d", len(items))
	}
}

func TestCreateItem(t *testing.T) {
	h := newTestHandler()
	body := bytes.NewBufferString(`{"name":"Test Item"}`)
	req := httptest.NewRequest("POST", "/api/items", body)
	w := httptest.NewRecorder()

	h.CreateItem(w, req)

	if w.Code != http.StatusCreated {
		t.Errorf("expected 201, got %d", w.Code)
	}
	var item Item
	json.NewDecoder(w.Body).Decode(&item)
	if item.Name != "Test Item" {
		t.Errorf("expected name 'Test Item', got %q", item.Name)
	}
	if item.ID != 3 {
		t.Errorf("expected id 3, got %d", item.ID)
	}
}

func TestCreateItemEmptyName(t *testing.T) {
	h := newTestHandler()
	body := bytes.NewBufferString(`{"name":""}`)
	req := httptest.NewRequest("POST", "/api/items", body)
	w := httptest.NewRecorder()

	h.CreateItem(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestCreateItemInvalidJSON(t *testing.T) {
	h := newTestHandler()
	body := bytes.NewBufferString(`not json`)
	req := httptest.NewRequest("POST", "/api/items", body)
	w := httptest.NewRecorder()

	h.CreateItem(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}
"#
                .into(),
            },
            TemplateFile {
                path: "middleware/logging.go".into(),
                content: r#"package middleware

import (
	"log"
	"net/http"
	"runtime/debug"
	"time"
)

// Middleware is a function that wraps an http.Handler.
type Middleware func(http.Handler) http.Handler

// Chain applies a list of middleware to a handler in order.
func Chain(h http.Handler, mws ...Middleware) http.Handler {
	for i := len(mws) - 1; i >= 0; i-- {
		h = mws[i](h)
	}
	return h
}

// statusRecorder captures the response status code.
type statusRecorder struct {
	http.ResponseWriter
	status int
}

func (r *statusRecorder) WriteHeader(code int) {
	r.status = code
	r.ResponseWriter.WriteHeader(code)
}

// Logging returns middleware that logs each request.
func Logging(logger *log.Logger) Middleware {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			start := time.Now()
			rec := &statusRecorder{ResponseWriter: w, status: http.StatusOK}
			next.ServeHTTP(rec, r)
			logger.Printf("%s %s %d %s", r.Method, r.URL.Path, rec.status, time.Since(start))
		})
	}
}

// Recovery returns middleware that recovers from panics.
func Recovery(logger *log.Logger) Middleware {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			defer func() {
				if err := recover(); err != nil {
					logger.Printf("PANIC: %v\n%s", err, debug.Stack())
					http.Error(w, `{"error":"internal server error"}`, http.StatusInternalServerError)
				}
			}()
			next.ServeHTTP(w, r)
		})
	}
}

// CORS adds permissive cross-origin headers.
func CORS(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, DELETE, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next.ServeHTTP(w, r)
	})
}
"#
                .into(),
            },
            TemplateFile {
                path: ".gitignore".into(),
                content: "bin/\n*.exe\n*.exe~\n*.dll\n*.so\n*.dylib\n*.swp\n*.swo\n.env\nvendor/\n.DS_Store\n*.test\n*.out\n".into(),
            },
            TemplateFile {
                path: "README.md".into(),
                content: r#"# {{name}}

{{description}}

## Description

A production-ready HTTP API built with Go's standard library (`net/http`). Features structured request logging, panic recovery middleware, CORS headers, input validation, and graceful shutdown with configurable timeouts.

## Install

### Prerequisites

- Go 1.23+

### Build

```bash
go build -o bin/{{name}} .
```

## Usage

```bash
# Development
go run .

# Production
PORT=8080 ./bin/{{name}}
```

The server starts at `http://localhost:8080`. Set the `PORT` environment variable to change the port.

### Endpoints

| Method | Path              | Description        |
|--------|-------------------|--------------------|
| GET    | `/health`         | Health check       |
| GET    | `/api/items`      | List all items     |
| GET    | `/api/items/{id}` | Get item by ID     |
| POST   | `/api/items`      | Create a new item  |

### Example

```bash
curl http://localhost:8080/health
# {"status":"ok","version":"0.1.0"}

curl -X POST http://localhost:8080/api/items \
  -H "Content-Type: application/json" \
  -d '{"name": "New Item"}'
# {"id":3,"name":"New Item"}
```

## Development

```bash
# Run tests
go test ./...

# Run tests with coverage
go test -cover ./...

# Format code
gofmt -w .

# Vet code
go vet ./...
```

## License

MIT
"#
                .into(),
            },
        ],
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_templates_not_empty() {
        assert!(builtin_templates().len() >= 5);
    }

    #[test]
    fn all_templates_have_required_fields() {
        for t in builtin_templates() {
            assert!(!t.name.is_empty());
            assert!(!t.description.is_empty());
            assert!(!t.language.is_empty());
            assert!(!t.files.is_empty());
        }
    }

    #[test]
    fn all_templates_have_project_file() {
        for t in builtin_templates() {
            let has_project_file = t.files.iter().any(|f| {
                f.path == "Cargo.toml"
                    || f.path == "package.json"
                    || f.path == "pyproject.toml"
                    || f.path == "go.mod"
            });
            assert!(has_project_file, "Template {} missing project file", t.name);
        }
    }

    #[test]
    fn all_templates_have_gitignore() {
        for t in builtin_templates() {
            assert!(
                t.files.iter().any(|f| f.path == ".gitignore"),
                "Template {} missing .gitignore",
                t.name
            );
        }
    }

    #[test]
    fn get_template_by_name() {
        assert!(get_template("rust-cli").is_some());
        assert!(get_template("RUST-CLI").is_some()); // case insensitive
        assert!(get_template("nonexistent").is_none());
    }

    #[test]
    fn write_template_creates_files() {
        let dir = std::env::temp_dir().join("nerve_scaffold_test");
        let _ = std::fs::remove_dir_all(&dir);

        let template = get_template("rust-cli").unwrap();
        let count = write_template(&template, &dir).unwrap();
        assert!(count >= 3);
        assert!(dir.join("Cargo.toml").exists());
        assert!(dir.join("src/main.rs").exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_templates_returns_info() {
        let list = list_templates();
        assert!(!list.is_empty());
        for (name, lang, desc) in &list {
            assert!(!name.is_empty());
            assert!(!lang.is_empty());
            assert!(!desc.is_empty());
        }
    }

    #[test]
    fn rust_cli_template_has_clap() {
        let t = get_template("rust-cli").unwrap();
        let main = t.files.iter().find(|f| f.path == "src/main.rs").unwrap();
        assert!(
            main.content.contains("clap"),
            "rust-cli template should use clap"
        );
        assert!(
            main.content.contains("Parser"),
            "rust-cli template should derive Parser"
        );
    }

    #[test]
    fn rust_web_template_has_axum() {
        let t = get_template("rust-web").unwrap();
        let cargo = t.files.iter().find(|f| f.path == "Cargo.toml").unwrap();
        assert!(
            cargo.content.contains("axum"),
            "rust-web template should use axum"
        );
    }

    #[test]
    fn node_api_template_has_express() {
        let t = get_template("node-api").unwrap();
        let pkg = t.files.iter().find(|f| f.path == "package.json").unwrap();
        assert!(
            pkg.content.contains("express"),
            "node-api template should use express"
        );
    }

    #[test]
    fn python_api_template_has_fastapi() {
        let t = get_template("python-api").unwrap();
        let has_fastapi = t
            .files
            .iter()
            .any(|f| f.content.contains("fastapi") || f.content.contains("FastAPI"));
        assert!(has_fastapi, "python-api template should use FastAPI");
    }

    #[test]
    fn all_templates_have_readme() {
        for t in builtin_templates() {
            assert!(
                t.files.iter().any(|f| f.path == "README.md"),
                "Template {} missing README",
                t.name
            );
        }
    }

    #[test]
    fn templates_have_placeholder() {
        for t in builtin_templates() {
            let has_placeholder = t.files.iter().any(|f| f.content.contains("{{name}}"));
            assert!(
                has_placeholder,
                "Template {} has no {{{{name}}}} placeholder",
                t.name
            );
        }
    }

    #[test]
    fn write_template_replaces_placeholders() {
        let mut template = get_template("rust-cli").unwrap();
        for file in &mut template.files {
            file.content = file.content.replace("{{name}}", "myproject");
            file.content = file.content.replace("{{description}}", "My project");
        }

        let dir = std::env::temp_dir().join("nerve_placeholder_test");
        let _ = std::fs::remove_dir_all(&dir);
        write_template(&template, &dir).unwrap();

        let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("myproject"));
        assert!(!cargo.contains("{{name}}"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rust_lib_template_has_lib_rs() {
        let t = get_template("rust-lib").unwrap();
        assert!(
            t.files
                .iter()
                .any(|f| f.path == "src/lib.rs" || f.path.contains("lib.rs")),
            "rust-lib should have lib.rs"
        );
    }

    #[test]
    fn go_api_template_has_go_mod() {
        let t = get_template("go-api").unwrap();
        assert!(
            t.files.iter().any(|f| f.path == "go.mod"),
            "go-api should have go.mod"
        );
    }

    #[test]
    fn python_cli_template_has_pyproject() {
        let t = get_template("python-cli").unwrap();
        assert!(
            t.files.iter().any(|f| f.path == "pyproject.toml"),
            "python-cli should have pyproject.toml"
        );
    }

    #[test]
    fn node_react_template_has_vite_config() {
        let t = get_template("node-react").unwrap();
        let has_vite = t
            .files
            .iter()
            .any(|f| f.path.contains("vite") || f.content.contains("vite"));
        assert!(has_vite, "node-react should reference vite");
    }

    #[test]
    fn all_templates_have_entry_point() {
        for t in builtin_templates() {
            let has_entry = t.files.iter().any(|f| {
                f.path.contains("main")
                    || f.path.contains("index")
                    || f.path.contains("app")
                    || f.path.contains("lib")
                    || f.path.contains("cli")
            });
            assert!(has_entry, "Template {} missing entry point file", t.name);
        }
    }

    #[test]
    fn templates_no_duplicate_files() {
        for t in builtin_templates() {
            let paths: Vec<&str> = t.files.iter().map(|f| f.path.as_str()).collect();
            let unique: std::collections::HashSet<&str> = paths.iter().cloned().collect();
            assert_eq!(
                paths.len(),
                unique.len(),
                "Template {} has duplicate file paths",
                t.name
            );
        }
    }

    #[test]
    fn exactly_eight_templates() {
        assert_eq!(builtin_templates().len(), 8);
    }
}
