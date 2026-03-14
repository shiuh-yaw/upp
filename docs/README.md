# UPP Documentation Site

Complete mdBook documentation for Universal Prediction Protocol (UPP).

## Structure

- **book.toml** — mdBook configuration
- **src/SUMMARY.md** — Table of contents
- **src/introduction.md** — What is UPP and why it exists
- **src/getting-started/** — Installation and quickstart
- **src/architecture/** — System design and internals
- **src/api/** — REST, gRPC, WebSocket API reference
- **src/sdk/** — Rust client library documentation
- **src/cli/** — Command-line tool guide
- **src/operations/** — Deployment, monitoring, configuration
- **src/development/** — Contributing and development guide

## Building

### Prerequisites

- mdBook: `cargo install mdbook`
- mdBook Mermaid plugin: `cargo install mdbook-mermaid`

### Serve Locally

```bash
cd docs
mdbook serve --open
```

Visit http://localhost:3000

### Build Static Site

```bash
cd docs
mdbook build
```

Output in `docs-build/html/`

## Content Overview

### Getting Started (150 lines each)
- **Quickstart** — 5-minute setup with docker-compose
- **Installation** — Docker, binaries, source build
- **Overview** — New user introduction

### Architecture (180-200 lines each)
- **System Overview** — Mermaid diagrams, data flow
- **Gateway Internals** — Router, middleware, caching
- **Provider Adapters** — Pattern design, adding new providers

### API Reference (200+ lines each)
- **REST API** — Complete endpoint reference with curl examples
- **gRPC** — Service definitions and Rust/Go examples
- **WebSocket** — Real-time subscriptions and message format

### SDK (150+ lines each)
- **Rust Client** — Complete usage guide with examples
- **Overview** — SDK features and patterns

### CLI (200 lines)
- **Command Reference** — All commands with examples
- **Output Formats** — Table, JSON, CSV

### Operations (200 lines each)
- **Deployment** — Docker, Kubernetes, cloud platforms
- **Monitoring** — Prometheus, Grafana, Jaeger, logging
- **Configuration** — All environment variables and config options

### Development (200+ lines each)
- **Contributing** — PR process, code style, standards
- **Testing** — Unit, integration, E2E, benchmarking
- **Overview** — Getting started contributing

## Statistics

- **24 documentation pages**
- **~4500 lines of content**
- **Code examples throughout**
- **Mermaid diagrams**
- **Real curl examples**
- **Rust code examples**

## Features

- Multi-level table of contents
- Full-text search
- Mobile-responsive design
- Dark theme (navy blue)
- Code syntax highlighting
- External links to GitHub

## Navigation

All pages interconnect logically:
- Introduction → Getting Started → API Reference
- Architecture explains internals
- Operations covers deployment
- Development guides contributors
- CLI provides quick reference

## Maintenance

Update content by editing Markdown files in `src/`. Changes appear immediately when running `mdbook serve`.

Rebuild the site with: `mdbook build`

Deploy the built site from `docs-build/html/` to your hosting.
