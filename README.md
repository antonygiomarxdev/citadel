<div align="center">

# Citadel

### Supercharge Claude Code, Cursor, Codex, and OpenCode with Semantic Code Intelligence

**~35% cheaper · ~70% fewer tool calls · 100% local**

[![npm version](https://img.shields.io/npm/v/citadel.svg)](https://www.npmjs.com/package/citadel)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Node.js](https://img.shields.io/badge/Node.js-20--24-green.svg)](https://nodejs.org/)
[![Rust](https://img.shields.io/badge/Rust-Core-ff69b4.svg)](https://www.rust-lang.org/)

[![Windows](https://img.shields.io/badge/Windows-supported-blue.svg)](#)
[![macOS](https://img.shields.io/badge/macOS-supported-blue.svg)](#)
[![Linux](https://img.shields.io/badge/Linux-supported-blue.svg)](#)

[![Claude Code](https://img.shields.io/badge/Claude_Code-supported-blueviolet.svg)](#)
[![Cursor](https://img.shields.io/badge/Cursor-supported-blueviolet.svg)](#)
[![Codex CLI](https://img.shields.io/badge/Codex_CLI-supported-blueviolet.svg)](#)
[![opencode](https://img.shields.io/badge/opencode-supported-blueviolet.svg)](#)

<br />

### Get Started

```bash
npx citadel
```

<sub>Interactive installer auto-configures your agent(s) — Claude Code, Cursor, Codex CLI, opencode</sub>

#### Initialize Projects

```bash
cd your-project
citadel init -i
```

![1_C_VYnhpys0UHrOuOgpgoyw](https://github.com/user-attachments/assets/f168182f-4d9a-44e0-94d7-08d018cc8a3a)

</div>

---

## Why Citadel?

When Claude Code explores a codebase, it spawns **Explore agents** that scan files with grep, glob, and Read — consuming tokens on every tool call.

**Citadel gives those agents a pre-indexed knowledge graph** — symbol relationships, call graphs, and code structure. Agents query the graph instantly instead of scanning files.

### Benchmark Results

Tested across **7 real-world open-source codebases** spanning 7 languages, comparing an agent (Claude Code, headless) answering one architecture question **with** and **without** Citadel. Each cell is the savings at the **median of 4 runs per arm**.

> **Average: 35% cheaper · 59% fewer tokens · 49% faster · 70% fewer tool calls**

| Codebase | Language | Cost | Tokens | Time | Tool calls |
|----------|----------|------|--------|------|------------|
| **VS Code** | TypeScript · ~10k files | 35% cheaper | 73% fewer | 41% faster | 72% fewer |
| **Excalidraw** | TypeScript · ~600 | 47% cheaper | 73% fewer | 60% faster | 86% fewer |
| **Django** | Python · ~2.7k | 34% cheaper | 64% fewer | 59% faster | 81% fewer |
| **Tokio** | Rust · ~700 | 52% cheaper | 81% fewer | 63% faster | 89% fewer |
| **OkHttp** | Java · ~640 | 17% cheaper | 41% fewer | 36% faster | 64% fewer |
| **Gin** | Go · ~150 | 22% cheaper | 23% fewer | 34% faster | 19% fewer |
| **Alamofire** | Swift · ~100 | 38% cheaper | 59% fewer | 51% faster | 77% fewer |

The gains scale with codebase size: on large repos the agent answers from the index in a handful of calls with **zero file reads**, while the no-Citadel agent fans out across grep/find/Read (and the sub-agents it spawns). On a small repo like Gin (~150 files) native search is already cheap, so the margin narrows.

<details>
<summary><strong>Full benchmark details</strong></summary>

**Methodology.** Each arm is `claude -p` (Claude Opus 4.7, Claude Code v2.1.145) run headlessly against the repo with `--strict-mcp-config`: **WITH** = Citadel's MCP server enabled, **WITHOUT** = an empty MCP config. Built-in Read/Grep/Bash stay available to both. Same question per repo, **4 runs per arm, median reported**. Cost = the run's `total_cost_usd`; Tokens = total tokens processed (input incl. cached + output); Time = wall-clock; Tool calls = every tool invocation, including those inside any sub-agents the model spawns. Repos cloned at `--depth 1` and indexed by the same Citadel build that served them.

**Queries:**
| Codebase | Query |
|----------|-------|
| VS Code | "How does the extension host communicate with the main process?" |
| Excalidraw | "How does Excalidraw render and update canvas elements?" |
| Django | "How does Django's ORM build and execute a query from a QuerySet?" |
| Tokio | "How does tokio schedule and run async tasks on its runtime?" |
| OkHttp | "How does OkHttp process a request through its interceptor chain?" |
| Gin | "How does gin route requests through its middleware chain?" |
| Alamofire | "How does Alamofire build, send, and validate a request?" |

**Raw medians — WITH → WITHOUT:**
| Codebase | Cost | Tokens | Time | Tool calls |
|----------|------|--------|------|------------|
| VS Code | $0.42 → $0.64 | 393k → 1.4M | 1m 0s → 1m 43s | 7 → 23 |
| Excalidraw | $0.54 → $1.02 | 851k → 3.2M | 1m 17s → 3m 14s | 12 → 83 |
| Django | $0.41 → $0.62 | 499k → 1.4M | 1m 0s → 2m 25s | 9 → 48 |
| Tokio | $0.50 → $1.04 | 657k → 3.4M | 1m 5s → 2m 56s | 9 → 75 |
| OkHttp | $0.36 → $0.44 | 352k → 596k | 45s → 1m 11s | 5 → 14 |
| Gin | $0.36 → $0.46 | 431k → 562k | 47s → 1m 11s | 7 → 8 |
| Alamofire | $0.61 → $0.99 | 1.1M → 2.6M | 1m 19s → 2m 41s | 15 → 64 |

**Why Citadel wins:** with the index available, the agent answers directly — `citadel_context` to map the area, then one `citadel_explore` for the relevant source — and stops, usually with zero file reads. Without it, the agent (and the Explore sub-agents it spawns) spends most of its budget on discovery (find/ls/grep) before reading the right code. Citadel only helps when queried *directly*, so its instructions steer agents to answer directly rather than delegate exploration to file-reading sub-agents — otherwise a sub-agent reads files regardless and Citadel becomes overhead.

</details>

### Index Performance

Measured on a Ryzen 7 5800X, NVMe SSD, Node 24, Rust native backend:

| Project | Files | Nodes | DB Size | Index Time | Search (warm) | Context |
|---------|-------|-------|---------|------------|---------------|---------|
| **citadel** (this repo) | 155 | 2,278 | 5 MB | 2.0s | 96ms | <1s |
| **rustc** (rust-lang/rust) | 36,198 | 347,192 | 679 MB | 413s (7 min) | 369ms | 0.9s |

**Key observations:**
- **Search scales sub-linearly** — only 3.8× slower for 124× more data thanks to SQLite FTS5 B-tree indices
- **Context is near-constant time** — graph expansion capped at 50 nodes regardless of project size
- **Index is the bottleneck** — single-threaded tree-sitter parsing; parallel extraction coming in a future release

</details>

---

## Key Features

| | |
|---|---|
| **Smart Context Building** | One tool call returns entry points, related symbols, and code snippets — no expensive exploration agents |
| **Full-Text Search** | Find code by name instantly across your entire codebase, powered by FTS5 |
| **Impact Analysis** | Trace callers, callees, and the full impact radius of any symbol before making changes |
| **Always Fresh** | File watcher uses native OS events (FSEvents/inotify/ReadDirectoryChangesW) with debounced auto-sync — the graph stays current as you code, zero config |
| **19+ Languages** | TypeScript, JavaScript, Python, Go, Rust, Java, C#, PHP, Ruby, C, C++, Swift, Kotlin, Dart, Lua, Luau, Svelte, Liquid, Pascal/Delphi |
| **Framework-aware Routes** | Recognizes web-framework routing files and links URL patterns to their handlers across 13 frameworks |
| **100% Local** | No data leaves your machine. No API keys. No external services. SQLite database only |

---

## Rust Core Migration

Citadel's core engine now runs in **Rust** via napi-rs for maximum performance. The TypeScript shell (MCP server, CLI, installer) remains; all performance-critical paths bind to native Rust.

| What | Before (TypeScript) | After (Rust) |
|------|-------------------|--------------|
| **Storage** | better-sqlite3 / WASM fallback (5-10x slower) | rusqlite bundled, native always |
| **Graph ops** | Single-threaded BFS/DFS in JS | Native BFS/DFS with ahash-accelerated visited sets |
| **Search** | FTS5 via SQLite adapter | FTS5 + hybrid keyword extraction in Rust |
| **Parsing** | tree-sitter via WASM in worker threads | tree-sitter native crate for TypeScript + Python |
| **Context** | ContextBuilder in TypeScript | Hybrid search + graph expansion in Rust |
| **Resolution** | Import resolver + name matcher in JS | Import resolution + name matching + 9 framework detectors in Rust |
| **Memory** | WASM heap growth requires worker recycling | No heap issues — parsing in same thread |

### Roadmap

| Phase | Slice | Status |
|-------|-------|--------|
| 1 | Rust workspace + napi-rs + CI | ✅ Complete |
| 2 | SQLite storage layer (rusqlite) | ✅ Complete |
| 0 | Storage Trait Hardening + Rango readiness | ✅ Complete |
| 3 | Graph traversal (BFS/DFS) | ✅ Complete |
| 4 | Tree-sitter parsing nativo | ✅ Complete |
| 5 | Reference resolution | ✅ Complete |
| 6 | Context builder + hybrid search | ✅ Complete |
| 7 | Thin TS cleanup + benchmarks | 🔜 In progress |

→ [View full plan on GitHub](https://github.com/antonygiomarxdev/citadel/issues/1)

---

Citadel detects web-framework routing files and emits `route` nodes linked by `references` edges to their handler classes or functions. Querying callers of a view/controller now surfaces the URL pattern that binds it.

| Framework | Shapes recognized |
|---|---|
| **Django** | `path()`, `re_path()`, `url()`, `include()` in `urls.py` (CBV `.as_view()`, dotted paths) |
| **Flask** | `@app.route('/path', methods=[...])`, blueprint routes |
| **FastAPI** | `@app.get(...)`, `@router.post(...)`, all standard methods |
| **Express** | `app.get(...)`, `router.post(...)` with middleware chains |
| **NestJS** | `@Controller` + `@Get/@Post/...`, GraphQL `@Resolver` + `@Query/@Mutation`, `@MessagePattern`/`@EventPattern`, `@SubscribeMessage` |
| **Laravel** | `Route::get()`, `Route::resource()`, `Controller@action`, tuple syntax |
| **Rails** | `get '/x', to: 'users#index'`, hash-rocket `=>` syntax |
| **Spring** | `@GetMapping`, `@PostMapping`, `@RequestMapping` on methods |
| **Gin / chi / gorilla / mux** | `r.GET(...)`, `router.HandleFunc(...)` |
| **Axum / actix / Rocket** | `.route("/x", get(handler))` |
| **ASP.NET** | `[HttpGet("/x")]` attributes on action methods |
| **Vapor** | `app.get("x", use: handler)` |
| **React Router** / **SvelteKit** | Route component nodes |

---

## Quick Start

### 1. Run the Installer

```bash
npx citadel
```

The installer will:
- Ask which agent(s) to configure — auto-detects installed ones from: **Claude Code**, **Cursor**, **Codex CLI**, **opencode**
- Prompt to install `citadel` on your PATH (so agents can launch the MCP server)
- Ask whether configs apply to all your projects or just this one
- Write each chosen agent's MCP server config + an instructions file (e.g. `CLAUDE.md`, `.cursor/rules/citadel.mdc`, `~/.codex/AGENTS.md`)
- Set up auto-allow permissions when Claude Code is one of the targets
- Initialize your current project (local installs only)

**Non-interactive (scripting / CI):**

```bash
citadel install --yes                              # auto-detect agents, install global
citadel install --target=cursor,claude --yes       # explicit target list
citadel install --target=auto --location=local     # detected agents, project-local
citadel install --print-config codex               # print snippet, no file writes
```

| Flag | Values | Default |
|---|---|---|
| `--target` | `auto`, `all`, `none`, or csv (`claude,cursor,...`) | prompt |
| `--location` | `global`, `local` | prompt |
| `--yes` | (boolean) | prompt every step |
| `--no-permissions` | (boolean) skip Claude auto-allow list | permissions on |
| `--print-config <id>` | dump snippet for one agent and exit | — |

### 2. Restart Your Agent

Restart your agent (Claude Code / Cursor / Codex CLI / opencode) for the MCP server to load.

### 3. Initialize Projects

```bash
cd your-project
citadel init -i
```

Builds the per-project knowledge graph index. Also wires up any project-local agent surfaces (e.g. Cursor's `.cursor/rules/citadel.mdc`) so a single global `citadel install` works in every project you open — no need to re-run the installer per project.

That's it — your agent will use Citadel tools automatically when a `.citadel/` directory exists.

<details>
<summary><strong>Manual Setup (Alternative)</strong></summary>

**Install globally:**
```bash
npm install -g citadel
```

**Add to `~/.claude.json`:**
```json
{
  "mcpServers": {
    "citadel": {
      "type": "stdio",
      "command": "citadel",
      "args": ["serve", "--mcp"]
    }
  }
}
```

**Add to `~/.claude/settings.json` (optional, for auto-allow):**
```json
{
  "permissions": {
    "allow": [
      "mcp__citadel__citadel_search",
      "mcp__citadel__citadel_context",
      "mcp__citadel__citadel_callers",
      "mcp__citadel__citadel_callees",
      "mcp__citadel__citadel_impact",
      "mcp__citadel__citadel_node",
      "mcp__citadel__citadel_status",
      "mcp__citadel__citadel_files"
    ]
  }
}
```

</details>

<details>
<summary><strong>Global Instructions Reference</strong></summary>

The installer automatically adds these instructions to `~/.claude/CLAUDE.md`:

```markdown
## Citadel

Citadel builds a semantic knowledge graph of codebases for faster, smarter code exploration.

### If `.citadel/` exists in the project

**NEVER call `citadel_explore` or `citadel_context` directly in the main session.** These tools return large amounts of source code that fills up main session context. Instead, ALWAYS spawn an Explore agent for any exploration question (e.g., "how does X work?", "explain the Y system", "where is Z implemented?").

**When spawning Explore agents**, include this instruction in the prompt:

> This project has Citadel initialized (.citadel/ exists). Use `citadel_explore` as your PRIMARY tool — it returns full source code sections from all relevant files in one call.
>
> **Rules:**
> 1. Follow the explore call budget in the `citadel_explore` tool description — it scales automatically based on project size.
> 2. Do NOT re-read files that citadel_explore already returned source code for. The source sections are complete and authoritative.
> 3. Only fall back to grep/glob/read for files listed under "Additional relevant files" if you need more detail, or if citadel returned no results.

**The main session may only use these lightweight tools directly** (for targeted lookups before making edits, not for exploration):

| Tool | Use For |
|------|---------|
| `citadel_search` | Find symbols by name |
| `citadel_callers` / `citadel_callees` | Trace call flow |
| `citadel_impact` | Check what's affected before editing |
| `citadel_node` | Get a single symbol's details |

### If `.citadel/` does NOT exist

At the start of a session, ask the user if they'd like to initialize Citadel:

"I notice this project doesn't have Citadel initialized. Would you like me to run `citadel init -i` to build a code knowledge graph?"
```

</details>

---

## How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│                        Claude Code                               │
│                                                                  │
│  "Implement user authentication"                                 │
│           │                                                      │
│           ▼                                                      │
│  ┌─────────────────┐      ┌─────────────────┐                   │
│  │  Explore Agent  │ ──── │  Explore Agent  │                   │
│  └────────┬────────┘      └────────┬────────┘                   │
│           │                        │                             │
└───────────┼────────────────────────┼─────────────────────────────┘
            │                        │
            ▼                        ▼
┌───────────────────────────────────────────────────────────────────┐
│                     Citadel MCP Server                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐               │
│  │   Search    │  │   Callers   │  │   Context   │               │
│  │  "auth"     │  │  "login()"  │  │  for task   │               │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘               │
│         │                │                │                       │
│         └────────────────┼────────────────┘                       │
│                          ▼                                        │
│              ┌──────────────────────────────────┐                 │
│              │        Rust Native Core          │                 │
│              │  ┌──────────┐  ┌──────────────┐  │                 │
│              │  │  Graph   │  │    Search    │  │                 │
│              │  │ Traversal│  │  (FTS5+tantivy)│  │                 │
│              │  └────┬─────┘  └──────┬───────┘  │                 │
│              │       │               │          │                 │
│              │  ┌────▼───────────────▼───────┐  │                 │
│              │  │   rusqlite / SQLite DB     │  │                 │
│              │  │   • 387 symbols            │  │                 │
│              │  │   • 1,204 edges            │  │                 │
│              │  │   • Instant lookups        │  │                 │
│              │  └────────────────────────────┘  │                 │
│              └──────────────────────────────────┘                 │
└───────────────────────────────────────────────────────────────────┘
```

1. **Extraction** — [tree-sitter](https://tree-sitter.github.io/) parses source code into ASTs. Language-specific queries extract nodes (functions, classes, methods) and edges (calls, imports, extends, implements). **Native Rust** tree-sitter replaces the WASM worker pool — no heap recycling, no serialization overhead.

2. **Storage** — Everything goes into a local SQLite database (`.citadel/citadel.db`) with FTS5 full-text search. Powered by **rusqlite** (Rust) — no WASM fallback, no adapter layer.

3. **Resolution** — After extraction, references are resolved: function calls → definitions, imports → source files, class inheritance, and framework-specific patterns. Runs in **native Rust** with concurrent name matching.

4. **Graph Traversal** — BFS/DFS impact analysis, callers/callees, and path finding execute in **parallel Rust** via rayon — sub-millisecond even on graphs with 100k+ nodes.

5. **Auto-Sync** — The MCP server watches your project using native OS file events. Changes are debounced (2-second quiet window), filtered to source files only, and incrementally synced. The graph stays fresh as you code — no configuration needed.

---

## CLI Reference

```bash
citadel                         # Run interactive installer
citadel install                 # Run installer (explicit)
citadel init [path]             # Initialize in a project (--index to also index)
citadel uninit [path]           # Remove Citadel from a project (--force to skip prompt)
citadel index [path]            # Full index (--force to re-index, --quiet for less output)
citadel sync [path]             # Incremental update
citadel status [path]           # Show statistics
citadel query <search>          # Search symbols (--kind, --limit, --json)
citadel files [path]            # Show file structure (--format, --filter, --max-depth, --json)
citadel context <task>          # Build context for AI (--format, --max-nodes)
citadel affected [files...]     # Find test files affected by changes (see below)
citadel serve --mcp             # Start MCP server
```

### `citadel affected`

Traces import dependencies transitively to find which test files are affected by changed source files.

```bash
citadel affected src/utils.ts src/api.ts         # Pass files as arguments
git diff --name-only | citadel affected --stdin   # Pipe from git diff
citadel affected src/auth.ts --filter "e2e/*"     # Custom test file pattern
```

| Option | Description | Default |
|--------|-------------|---------|
| `--stdin` | Read file list from stdin | `false` |
| `-d, --depth <n>` | Max dependency traversal depth | `5` |
| `-f, --filter <glob>` | Custom glob to identify test files | auto-detect |
| `-j, --json` | Output as JSON | `false` |
| `-q, --quiet` | Output file paths only | `false` |

**CI/hook example:**

```bash
#!/usr/bin/env bash
AFFECTED=$(git diff --name-only HEAD | citadel affected --stdin --quiet)
if [ -n "$AFFECTED" ]; then
  npx vitest run $AFFECTED
fi
```

---

## MCP Tools

When running as an MCP server, Citadel exposes these tools to Claude Code:

| Tool | Purpose |
|------|---------|
| `citadel_search` | Find symbols by name across the codebase |
| `citadel_context` | Build relevant code context for a task |
| `citadel_callers` | Find what calls a function |
| `citadel_callees` | Find what a function calls |
| `citadel_impact` | Analyze what code is affected by changing a symbol |
| `citadel_node` | Get details about a specific symbol (optionally with source code) |
| `citadel_files` | Get indexed file structure (faster than filesystem scanning) |
| `citadel_status` | Check index health and statistics |

---

## Library Usage

```typescript
import Citadel from 'citadel';

const cg = await Citadel.init('/path/to/project');
// Or: const cg = await Citadel.open('/path/to/project');

await cg.indexAll({
  onProgress: (p) => console.log(`${p.phase}: ${p.current}/${p.total}`)
});

const results = cg.searchNodes('UserService');
const callers = cg.getCallers(results[0].node.id);
const context = await cg.buildContext('fix login bug', { maxNodes: 20, includeCode: true, format: 'markdown' });
const impact = cg.getImpactRadius(results[0].node.id, 2);

cg.watch();   // auto-sync on file changes
cg.unwatch(); // stop watching
cg.close();
```

---

## Configuration

The `.citadel/config.json` file controls indexing:

```json
{
  "version": 1,
  "languages": ["typescript", "javascript"],
  "exclude": ["node_modules/**", "dist/**", "build/**", "*.min.js"],
  "frameworks": [],
  "maxFileSize": 1048576,
  "extractDocstrings": true,
  "trackCallSites": true
}
```

| Option | Description | Default |
|--------|-------------|---------|
| `languages` | Languages to index (auto-detected if empty) | `[]` |
| `exclude` | Glob patterns to ignore | `["node_modules/**", ...]` |
| `frameworks` | Framework hints for better resolution | `[]` |
| `maxFileSize` | Skip files larger than this (bytes) | `1048576` (1MB) |
| `extractDocstrings` | Extract docstrings from code | `true` |
| `trackCallSites` | Track call site locations | `true` |

## Supported Languages

| Language | Extension | Status |
|----------|-----------|--------|
| TypeScript | `.ts`, `.tsx` | Full support |
| JavaScript | `.js`, `.jsx`, `.mjs` | Full support |
| Python | `.py` | Full support |
| Go | `.go` | Full support |
| Rust | `.rs` | Full support |
| Java | `.java` | Full support |
| C# | `.cs` | Full support |
| PHP | `.php` | Full support |
| Ruby | `.rb` | Full support |
| C | `.c`, `.h` | Full support |
| C++ | `.cpp`, `.hpp`, `.cc` | Full support |
| Swift | `.swift` | Full support |
| Kotlin | `.kt`, `.kts` | Full support |
| Scala | `.scala`, `.sc` | Full support (classes, traits, methods, type aliases, Scala 3 enums) |
| Dart | `.dart` | Full support |
| Svelte | `.svelte` | Full support (script extraction, Svelte 5 runes, SvelteKit routes) |
| Vue | `.vue` | Full support (script + script-setup extraction, Nuxt page/API/middleware routes) |
| Liquid | `.liquid` | Full support |
| Pascal / Delphi | `.pas`, `.dpr`, `.dpk`, `.lpr` | Full support (classes, records, interfaces, enums, DFM/FMX form files) |
| Lua | `.lua` | Full support (functions, methods with receivers, local variables, `require` imports, call edges) |
| Luau | `.luau` | Full support (everything in Lua, plus `type`/`export type` aliases, typed signatures, and Roblox instance-path `require`) |

## Troubleshooting

**"Citadel not initialized"** — Run `citadel init` in your project directory first.

**Indexing is slow** — Check that `node_modules` and other large directories are excluded. Use `--quiet` to reduce output overhead.

**Indexing is slow / MCP `database is locked` / WASM fallback active** — `citadel` ships with a WASM SQLite fallback for environments where `better-sqlite3` (a native module, declared as `optionalDependencies`) can't install. The fallback is 5-10x slower than the native backend and uses a journal mode that lets writers block readers, so MCP queries can also hit `database is locked` while indexing runs.

> ⚡ **Upcoming:** The Rust core migration eliminates both the WASM fallback and better-sqlite3 entirely — rusqlite with bundled SQLite is the only backend. See the [Rust Roadmap](#rust-core-migration).

Run `citadel status` and look at the `Backend:` line:

- `Backend: native` — you're on the fast path, nothing to do.
- `Backend: wasm` — you're on the slow fallback. Common causes: missing C build tools, prebuilt binary unavailable for your Node version, or your Node version changed after install. Fix:

  ```bash
  # macOS
  xcode-select --install                                  # installs the C compiler

  # Linux (Debian / Ubuntu)
  sudo apt install build-essential python3 make

  # Linux (RHEL / Fedora)
  sudo yum groupinstall "Development Tools"

  # Then rebuild on any platform:
  npm rebuild better-sqlite3

  # Or force-include as a hard dep:
  npm install better-sqlite3 --save
  ```

  After the fix, `citadel status` should show `Backend: native`.

**MCP server not connecting** — Ensure the project is initialized/indexed, verify the path in your MCP config, and check that `citadel serve --mcp` works from the command line.

**Missing symbols** — The MCP server auto-syncs on save (wait a couple seconds). Run `citadel sync` manually if needed. Check that the file's language is supported and isn't excluded by config patterns.

## Star History

<a href="https://www.star-history.com/?repos=antonygiomarxdev%2Fcitadel&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=antonygiomarxdev/citadel&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=antonygiomarxdev/citadel&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=antonygiomarxdev/citadel&type=date&legend=top-left" />
 </picture>
</a>

## License

MIT

---

<div align="center">

**Made for AI coding agents — Claude Code, Cursor, Codex CLI, and opencode**

[Report Bug](https://github.com/antonygiomarxdev/citadel/issues) · [Request Feature](https://github.com/antonygiomarxdev/citadel/issues)

</div>
