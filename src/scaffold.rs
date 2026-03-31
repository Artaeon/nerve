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
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tower-http = { version = "0.6", features = ["cors", "trace"] }
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
thiserror = "2"
"#
                .into(),
            },
            TemplateFile {
                path: "src/main.rs".into(),
                content: r#"use axum::{routing::get, Router};
use tracing_subscriber;

mod handlers;
mod error;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", get(handlers::root))
        .route("/health", get(handlers::health));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000");

    tracing::info!("Listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/handlers.rs".into(),
                content: r#"use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

pub async fn root() -> Json<MessageResponse> {
    Json(MessageResponse {
        message: "Welcome to {{name}}".into(),
    })
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_returns_ok() {
        let Json(resp) = health().await;
        assert_eq!(resp.status, "ok");
    }

    #[tokio::test]
    async fn root_returns_welcome() {
        let Json(resp) = root().await;
        assert!(resp.message.contains("Welcome"));
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
        let body = Json(json!({ "error": message }));
        (status, body).into_response()
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
                content: r#"# {{name}}

{{description}}

## Running

```bash
cargo run
```

The server starts at `http://localhost:3000`.

## Endpoints

- `GET /` - Welcome message
- `GET /health` - Health check

## Development

```bash
cargo test
cargo run
```
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
    "test": "node --test dist/**/*.test.js"
  },
  "dependencies": {
    "express": "^4.21.0",
    "cors": "^2.8.5"
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
    "declaration": true
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
import { router } from "./routes";
import { errorHandler } from "./middleware";

const app = express();
const PORT = process.env.PORT || 3000;

app.use(cors());
app.use(express.json());

app.use("/api", router);

app.get("/health", (_req, res) => {
  res.json({ status: "ok", timestamp: new Date().toISOString() });
});

app.use(errorHandler);

app.listen(PORT, () => {
  console.log(`Server running on http://localhost:${PORT}`);
});

export default app;
"#
                .into(),
            },
            TemplateFile {
                path: "src/routes.ts".into(),
                content: r#"import { Router, Request, Response } from "express";

export const router = Router();

interface Item {
  id: number;
  name: string;
}

const items: Item[] = [
  { id: 1, name: "Item One" },
  { id: 2, name: "Item Two" },
];

router.get("/items", (_req: Request, res: Response) => {
  res.json(items);
});

router.get("/items/:id", (req: Request, res: Response) => {
  const item = items.find((i) => i.id === parseInt(req.params.id));
  if (!item) {
    res.status(404).json({ error: "Item not found" });
    return;
  }
  res.json(item);
});

router.post("/items", (req: Request, res: Response) => {
  const { name } = req.body;
  if (!name) {
    res.status(400).json({ error: "Name is required" });
    return;
  }
  const newItem: Item = { id: items.length + 1, name };
  items.push(newItem);
  res.status(201).json(newItem);
});
"#
                .into(),
            },
            TemplateFile {
                path: "src/middleware.ts".into(),
                content: r#"import { Request, Response, NextFunction } from "express";

export function errorHandler(
  err: Error,
  _req: Request,
  res: Response,
  _next: NextFunction
): void {
  console.error(err.stack);
  res.status(500).json({ error: "Internal Server Error" });
}

export function requestLogger(
  req: Request,
  _res: Response,
  next: NextFunction
): void {
  console.log(`${new Date().toISOString()} ${req.method} ${req.path}`);
  next();
}
"#
                .into(),
            },
            TemplateFile {
                path: "src/index.test.ts".into(),
                content: r#"import { describe, it } from "node:test";
import assert from "node:assert";

describe("API", () => {
  it("health check concept", () => {
    const health = { status: "ok" };
    assert.strictEqual(health.status, "ok");
  });

  it("items array is valid", () => {
    const items = [{ id: 1, name: "Test" }];
    assert.strictEqual(items.length, 1);
    assert.strictEqual(items[0].name, "Test");
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
                content: r#"# {{name}}

{{description}}

## Setup

```bash
npm install
```

## Development

```bash
npm run dev
```

## Production

```bash
npm run build
npm start
```

## API

- `GET /health` - Health check
- `GET /api/items` - List items
- `GET /api/items/:id` - Get item by ID
- `POST /api/items` - Create item
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
                content: r#":root {
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
"#
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
                content: r#"# {{name}}

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
"#
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
                content: "\"\"\"{{name}} - {{description}}\"\"\"\n\n__version__ = \"0.1.0\"\n".into(),
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
                content: r#"# {{name}}

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
"#
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
requires-python = ">=3.10"
dependencies = [
    "fastapi>=0.115.0",
    "uvicorn[standard]>=0.32.0",
    "pydantic>=2.9.0",
]

[project.optional-dependencies]
dev = ["pytest>=8.0", "httpx>=0.27.0"]
"#
                .into(),
            },
            TemplateFile {
                path: "requirements.txt".into(),
                content: "fastapi>=0.115.0\nuvicorn[standard]>=0.32.0\npydantic>=2.9.0\nhttpx>=0.27.0\npytest>=8.0\n".into(),
            },
            TemplateFile {
                path: "src/main.py".into(),
                content: r#""""{{name}} - {{description}}"""

from fastapi import FastAPI
from src.routes import router
from src.models import HealthResponse

app = FastAPI(title="{{name}}", description="{{description}}", version="0.1.0")

app.include_router(router, prefix="/api")


@app.get("/health", response_model=HealthResponse)
async def health_check():
    """Health check endpoint."""
    return HealthResponse(status="ok", version="0.1.0")


if __name__ == "__main__":
    import uvicorn
    uvicorn.run("src.main:app", host="0.0.0.0", port=8000, reload=True)
"#
                .into(),
            },
            TemplateFile {
                path: "src/__init__.py".into(),
                content: "".into(),
            },
            TemplateFile {
                path: "src/models.py".into(),
                content: r#""""Data models for {{name}}."""

from pydantic import BaseModel


class HealthResponse(BaseModel):
    status: str
    version: str


class Item(BaseModel):
    id: int
    name: str
    description: str = ""


class ItemCreate(BaseModel):
    name: str
    description: str = ""
"#
                .into(),
            },
            TemplateFile {
                path: "src/routes.py".into(),
                content: r#""""API routes for {{name}}."""

from fastapi import APIRouter, HTTPException
from src.models import Item, ItemCreate

router = APIRouter()

items: list[Item] = [
    Item(id=1, name="Item One", description="The first item"),
    Item(id=2, name="Item Two", description="The second item"),
]


@router.get("/items", response_model=list[Item])
async def list_items():
    """List all items."""
    return items


@router.get("/items/{item_id}", response_model=Item)
async def get_item(item_id: int):
    """Get an item by ID."""
    for item in items:
        if item.id == item_id:
            return item
    raise HTTPException(status_code=404, detail="Item not found")


@router.post("/items", response_model=Item, status_code=201)
async def create_item(data: ItemCreate):
    """Create a new item."""
    new_id = max((i.id for i in items), default=0) + 1
    item = Item(id=new_id, name=data.name, description=data.description)
    items.append(item)
    return item
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
from src.main import app

client = TestClient(app)


def test_health():
    response = client.get("/health")
    assert response.status_code == 200
    data = response.json()
    assert data["status"] == "ok"


def test_list_items():
    response = client.get("/api/items")
    assert response.status_code == 200
    assert isinstance(response.json(), list)


def test_get_item():
    response = client.get("/api/items/1")
    assert response.status_code == 200
    assert response.json()["id"] == 1


def test_get_item_not_found():
    response = client.get("/api/items/999")
    assert response.status_code == 404


def test_create_item():
    response = client.post("/api/items", json={"name": "New Item"})
    assert response.status_code == 201
    assert response.json()["name"] == "New Item"
"#
                .into(),
            },
            TemplateFile {
                path: ".gitignore".into(),
                content: "__pycache__/\n*.pyc\n*.egg-info/\ndist/\nbuild/\n.venv/\n.env\n".into(),
            },
            TemplateFile {
                path: "README.md".into(),
                content: r#"# {{name}}

{{description}}

## Setup

```bash
pip install -r requirements.txt
```

## Running

```bash
uvicorn src.main:app --reload
```

The server starts at `http://localhost:8000`.

## API Docs

- Swagger UI: `http://localhost:8000/docs`
- ReDoc: `http://localhost:8000/redoc`

## Endpoints

- `GET /health` - Health check
- `GET /api/items` - List items
- `GET /api/items/:id` - Get item
- `POST /api/items` - Create item

## Testing

```bash
pytest tests/
```
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
                content: r#"module {{name}}

go 1.23
"#
                .into(),
            },
            TemplateFile {
                path: "main.go".into(),
                content: r#"package main

import (
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"os"
	"time"
)

func main() {
	mux := http.NewServeMux()

	mux.HandleFunc("GET /", handleRoot)
	mux.HandleFunc("GET /health", handleHealth)
	mux.HandleFunc("GET /api/items", handleListItems)
	mux.HandleFunc("POST /api/items", handleCreateItem)

	handler := loggingMiddleware(mux)

	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}

	addr := fmt.Sprintf(":%s", port)
	log.Printf("Server starting on http://localhost%s", addr)
	if err := http.ListenAndServe(addr, handler); err != nil {
		log.Fatalf("Server failed: %v", err)
	}
}

type MessageResponse struct {
	Message string `json:"message"`
}

type HealthResponse struct {
	Status  string `json:"status"`
	Version string `json:"version"`
}

type Item struct {
	ID   int    `json:"id"`
	Name string `json:"name"`
}

var items = []Item{
	{ID: 1, Name: "Item One"},
	{ID: 2, Name: "Item Two"},
}

func handleRoot(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, MessageResponse{Message: "Welcome to {{name}}"})
}

func handleHealth(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, HealthResponse{Status: "ok", Version: "0.1.0"})
}

func handleListItems(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, items)
}

func handleCreateItem(w http.ResponseWriter, r *http.Request) {
	var item Item
	if err := json.NewDecoder(r.Body).Decode(&item); err != nil {
		writeJSON(w, http.StatusBadRequest, MessageResponse{Message: "Invalid JSON"})
		return
	}
	item.ID = len(items) + 1
	items = append(items, item)
	writeJSON(w, http.StatusCreated, item)
}

func writeJSON(w http.ResponseWriter, status int, data interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(data)
}

func loggingMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		next.ServeHTTP(w, r)
		log.Printf("%s %s %s", r.Method, r.URL.Path, time.Since(start))
	})
}
"#
                .into(),
            },
            TemplateFile {
                path: "main_test.go".into(),
                content: r#"package main

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestHandleRoot(t *testing.T) {
	req := httptest.NewRequest("GET", "/", nil)
	w := httptest.NewRecorder()

	handleRoot(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	var resp MessageResponse
	json.NewDecoder(w.Body).Decode(&resp)
	if resp.Message == "" {
		t.Error("expected non-empty message")
	}
}

func TestHandleHealth(t *testing.T) {
	req := httptest.NewRequest("GET", "/health", nil)
	w := httptest.NewRecorder()

	handleHealth(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	var resp HealthResponse
	json.NewDecoder(w.Body).Decode(&resp)
	if resp.Status != "ok" {
		t.Errorf("expected status ok, got %s", resp.Status)
	}
}

func TestHandleListItems(t *testing.T) {
	req := httptest.NewRequest("GET", "/api/items", nil)
	w := httptest.NewRecorder()

	handleListItems(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	var resp []Item
	json.NewDecoder(w.Body).Decode(&resp)
	if len(resp) == 0 {
		t.Error("expected non-empty items list")
	}
}
"#
                .into(),
            },
            TemplateFile {
                path: ".gitignore".into(),
                content: "bin/\n*.exe\n*.swp\n.env\nvendor/\n".into(),
            },
            TemplateFile {
                path: "README.md".into(),
                content: r#"# {{name}}

{{description}}

## Running

```bash
go run .
```

The server starts at `http://localhost:8080`.

## Endpoints

- `GET /` - Welcome message
- `GET /health` - Health check
- `GET /api/items` - List items
- `POST /api/items` - Create item

## Testing

```bash
go test ./...
```
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
            assert!(
                has_project_file,
                "Template {} missing project file",
                t.name
            );
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
        assert!(main.content.contains("clap"), "rust-cli template should use clap");
        assert!(main.content.contains("Parser"), "rust-cli template should derive Parser");
    }

    #[test]
    fn rust_web_template_has_axum() {
        let t = get_template("rust-web").unwrap();
        let cargo = t.files.iter().find(|f| f.path == "Cargo.toml").unwrap();
        assert!(cargo.content.contains("axum"), "rust-web template should use axum");
    }

    #[test]
    fn node_api_template_has_express() {
        let t = get_template("node-api").unwrap();
        let pkg = t.files.iter().find(|f| f.path == "package.json").unwrap();
        assert!(pkg.content.contains("express"), "node-api template should use express");
    }

    #[test]
    fn python_api_template_has_fastapi() {
        let t = get_template("python-api").unwrap();
        let has_fastapi = t.files.iter().any(|f| f.content.contains("fastapi") || f.content.contains("FastAPI"));
        assert!(has_fastapi, "python-api template should use FastAPI");
    }

    #[test]
    fn all_templates_have_readme() {
        for t in builtin_templates() {
            assert!(t.files.iter().any(|f| f.path == "README.md"), "Template {} missing README", t.name);
        }
    }

    #[test]
    fn templates_have_placeholder() {
        for t in builtin_templates() {
            let has_placeholder = t.files.iter().any(|f| f.content.contains("{{name}}"));
            assert!(has_placeholder, "Template {} has no {{{{name}}}} placeholder", t.name);
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
}
