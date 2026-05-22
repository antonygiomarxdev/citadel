# Plan: Refactorización Arquitectónica de Citadel-Core

## Resumen

Transformar el prototipo funcional actual en software de calidad aplicando SOLID, Clean Code y patrones de diseño Rust. El objetivo es que agregar features nuevas sea trivial, no un acto de fe.

El plan cubre los 3 issues abiertos (#10, #11, #12) pero desde los cimientos — arreglando la arquitectura antes de parchar síntomas.

## Diagnóstico Actual

| Problema | Impacto |
|----------|---------|
| `Storage` trait con ~50 métodos (viola ISP) | Cada backend nuevo debe implementarlo TODO. Imposible hacer backend solo-lectura, remoto, o en-memoria |
| Graph traversal acoplado a Storage (N+1 queries) | DFS usa ~2000 queries SQLite para 1000 nodos. BFS tiene override optimizado, DFS no |
| `ContextBuilder` con estrategias hardcodeadas y `std::fs` directo | No se puede testear, no se puede extender, no funciona con backends remotos |
| Código Rust de resolución muerto (`ResolutionContext` sin adaptador para Storage) | 9 framework resolvers implementados pero inusables |
| NAPI Database con 366 líneas de boilerplate idéntico | `lock() → call → to_string → map_err` repetido 40 veces |
| `ExtractionOrchestrator` TS de 1552 líneas (viola SRP) | File scanning + git + workers + WASM + native + DB + frameworks + progress + retry |
| `GraphTraverser` TS (642 líneas) duplica traversal de Rust | Bug fixes en dos lugares. NAPI bridge de traversal existe pero TS no lo usa |
| Tipos de error inconsistentes: `StorageError`, `String`, `napi::Error::from_reason` | Sin structured error handling a través del stack |

## Fases

### Fase 0: Infraestructura de Errores (fundación)

**Nuevo archivo:** `citadel-core/src/error.rs`

- `CitadelError` como tipo de error único para todo el core (reemplaza `StorageError` + `String`)
- `ExtractionErrorKind` enum: `ParseError`, `UnsupportedSyntax`, `FatalPanic`, `StackOverflow`, `InvalidSpan`, `TreeSitterError`, `Other`
- Cada variante tiene `severity()` → `ErrorSeverity` enum (`Critical`, `Error`, `Warning`, `Info`)
- Conversiones From: `std::io::Error`, `rusqlite::Error`, `serde_json::Error`
- Conversión a `napi::Error` que preserva el kind como prefijo `[KindName] mensaje`
- `FromStr` de `NodeKind`/`EdgeKind`/`Language` ahora retornan `Result<_, CitadelError>` (era `Result<_, ()>`)

**Archivos modificados:**
- `citadel-core/src/error.rs` — nuevo
- `citadel-core/src/storage/error.rs` — re-exporta `CitadelError` con deprecation
- `citadel-core/src/types.rs` — `ExtractionError` ahora tiene `kind` field
- `citadel-core/src/context/mod.rs` — firmas cambian de `Result<_, String>` a `Result<_, CitadelError>`
- `citadel-core/src/resolution/mod.rs` — ídem
- `citadel-napi/src/database.rs` — usa `CitadelError` para conversión a napi

**Verificación:** `cargo clippy`, `cargo test`, `npm run build`, `npm test`

---

### Fase 1: Descomponer el Monolito Storage (ISP)

**Nuevo archivo:** `citadel-core/src/storage/traits/mod.rs`

Traits enfocados:

```rust
pub trait NodeStore { ... }          // 12 métodos (CRUD + search)
pub trait EdgeStore { ... }          // 6 métodos
pub trait FileStore { ... }          // 7 métodos
pub trait UnresolvedRefStore { ... } // 11 métodos
pub trait MetadataStore { ... }      // 3 métodos
pub trait StatsProvider { ... }      // 2 métodos
pub trait Lifecycle { ... }          // 5 métodos (init/open/close/get_path/clear)

pub trait FullStore: NodeStore + EdgeStore + FileStore + UnresolvedRefStore 
    + MetadataStore + StatsProvider + Lifecycle + Send + Sync {}
```

**Cambio clave:** `Lifecycle::initialize(&mut self, config: &DatabaseConfig)` — `DatabaseConfig` es una struct con `connection_string: String` + `extra: HashMap<String, String>`. Ya no es `db_path: &str`. Para SQLite, `connection_string` es el path al archivo.

**Archivos modificados:**
- `citadel-core/src/storage/traits/mod.rs` — nuevo (definiciones de traits)
- `citadel-core/src/storage/mod.rs` — `pub use traits::FullStore as Storage;` (backward compat)
- `citadel-core/src/storage/sqlite.rs` — reorganizar `impl NodeStore for SqliteStorage { ... }`, `impl EdgeStore for SqliteStorage { ... }`, etc. Puro movimiento de código, sin cambio de comportamiento.
- `citadel-core/src/storage/contract_tests.rs` — toma `Box<dyn FullStore>`; tests por trait

**Estrategia de migración:** Mantener `Storage` como alias de `FullStore` durante Fases 1-5. Eliminar después de Fase 6.

**Verificación:** `cargo clippy`, `cargo test` (contract tests pasan), `npm test`

---

### Fase 2: Extraer Graph como Abstracción Independiente

**Nuevo archivo:** `citadel-core/src/graph/mod.rs`

```rust
pub trait GraphQuery: NodeStore + EdgeStore {
    fn traverse_bfs(&self, start: &str, opts: &TraversalOptions) -> Result<Vec<(Node, Vec<Edge>)>, CitadelError>;
    fn traverse_dfs(...);
    fn find_shortest_path(...);
    fn get_callers(...);
    fn get_callees(...);
    fn get_impact_radius(...);
    fn get_call_graph(...);
    fn get_type_hierarchy(...);
    fn find_usages(...);
    fn get_ancestors(...);
    fn get_children(...);
    fn get_node_metrics(...);
}

// Blanket impl: cualquier (NodeStore + EdgeStore) recibe traversal gratis
impl<T: NodeStore + EdgeStore> GraphQuery for T {}
```

Los métodos default usan solo `NodeStore` + `EdgeStore` (sin SQL). `SqliteStorage` overridea BFS, get_ancestors, get_node_metrics con versiones SQL-optimizadas.

**Archivos modificados:**
- `citadel-core/src/graph/mod.rs` — nuevo (mueve ~400 líneas de `storage/mod.rs`)
- `citadel-core/src/storage/mod.rs` — elimina métodos de traversal del trait
- `citadel-core/src/storage/sqlite.rs` — los 3 overrides SQL pasan a `impl GraphQuery for SqliteStorage`
- `citadel-core/src/lib.rs` — `pub mod graph;`

**Verificación:** traversal idéntico al anterior, `cargo test`, `npm test`

---

### Fase 3: Abstracción de FileSystem

**Nuevo archivo:** `citadel-core/src/filesystem.rs`

```rust
pub trait FileSystem: Send + Sync {
    fn read_file(&self, path: &str) -> Result<String, CitadelError>;
    fn file_exists(&self, path: &str) -> bool;
    fn get_project_root(&self) -> &str;
}

pub struct LocalFileSystem { root: String }
impl FileSystem for LocalFileSystem { ... }
```

**Archivos modificados:**
- `citadel-core/src/filesystem.rs` — nuevo
- `citadel-core/src/context/mod.rs` — `ContextBuilder` ahora recibe `fs: Box<dyn FileSystem>`; `extract_source_blocks` usa `self.fs.read_file()` en vez de `std::fs::read_to_string`
- `citadel-core/src/lib.rs` — `pub mod filesystem;`

**Verificación:** `cargo clippy`, `cargo test`

---

### Fase 4: Puente ResolutionContext → Storage

**Nuevo archivo:** `citadel-core/src/resolution/storage_adapter.rs`

`StorageResolutionContext` implementa `ResolutionContext` delegando en `&Mutex<Box<dyn FullStore>>` + `&dyn FileSystem`. Esto hace que los 9 framework resolvers + import resolver + name matcher sean usables con datos reales.

**Archivos modificados:**
- `citadel-core/src/resolution/storage_adapter.rs` — nuevo
- `citadel-core/src/resolution/mod.rs` — `ResolutionContext` retorna `Result<_, CitadelError>` (era `String`). Actualizar los 9 frameworks.
- `citadel-core/src/lib.rs` — re-export

**Verificación:** compila, `cargo test`

---

### Fase 5: Context con Strategy Pattern

**Nuevos archivos:**
- `citadel-core/src/context/strategies.rs` — `SearchStrategy` trait + `ExactMatchStrategy`, `PrefixMatchStrategy`, `Fts5SearchStrategy`
- `citadel-core/src/context/formatting.rs` — `ContextFormatter` trait + `MarkdownFormatter`, `JsonFormatter`

`ContextBuilder` ahora acepta `Vec<Box<dyn SearchStrategy>>` y `Box<dyn ContextFormatter>` configurables. El pipeline es el mismo pero las estrategias son inyectables.

**Archivos modificados:**
- `citadel-core/src/context/strategies.rs` — nuevo
- `citadel-core/src/context/formatting.rs` — nuevo
- `citadel-core/src/context/mod.rs` — `ContextBuilder::new(store, fs)` con defaults razonables; métodos `with_search_strategies()`, `with_formatter()`

**Verificación:** comportamiento idéntico, `cargo test`, `npm test`

---

### Fase 6: Unificar NAPI Bridge (eliminar GraphTraverser TS)

**Nuevo archivo:** `citadel-napi/src/macros.rs` — macro `napi_method!` que genera el boilerplate `lock() → call → serialize → map_err`

**Archivos modificados:**
- `citadel-napi/src/macros.rs` — nuevo
- `citadel-napi/src/database.rs` — usar `napi_method!` en vez de las 40 repeticiones manuales. Exponer `db.graph` como sub-objeto con métodos de traversal que llaman `GraphQuery`.
- `src/graph/traversal.ts` — refactorizar `GraphTraverser` para ser wrapper de `db.graph.*` en vez de reimplementar con `QueryBuilder`
- `src/mcp/tools.ts`, `src/exploration/` — usar `graphTraverser` (wrapper) sin cambios de API

**Verificación:** `npm test`. Los tests de traversal deben pasar con los mismos resultados que antes.

---

### Fase 7: Refactorizar ExtractionOrchestrator TS (SRP)

**Nuevos archivos:**
- `src/extraction/file-scanner.ts` — scanning + git + filtering
- `src/extraction/worker-pool.ts` — worker lifecycle (spawn, reciclar, timeout, crash)

**Archivos modificados:**
- `src/extraction/index.ts` — reducir a orquestación pura (~300 líneas). Delegar I/O a FileScanner, workers a WorkerPool.
- `src/extraction/store.ts` — `ExtractionStore` interface que desacopla `storeExtractionResult` de `QueryBuilder`

**Verificación:** `npm test`. Comportamiento de index idéntico.

---

### Fase 8: Crash Fixes + Nuevos Extractores

#### 8.1 Crash fixes
1. `citadel-core/Cargo.toml` — alinear tree-sitter 0.23 o 0.24
2. `citadel-napi/src/extraction.rs` — `catch_unwind` a nivel de archivo individual en `par_iter`
3. `citadel-core/src/extraction/languages/typescript.rs` — no continuar tras `set_language` error
4. `citadel-core/src/extraction/walker.rs` — `TreeWalker` con `max_depth: 500` implementando `NodeVisitor` trait
5. `citadel-core/src/extraction/languages/typescript.rs` — migrar a `TreeWalker`
6. `citadel-core/src/extraction/languages/python.rs` — migrar a `TreeWalker`

#### 8.2 Registry de extractores
- `citadel-core/src/extraction/languages/mod.rs` — `LazyLock<HashMap<Language, ExtractorFactory>>`

#### 8.3 Nuevos extractores
- `citadel-core/src/extraction/languages/rust.rs`
- `citadel-core/src/extraction/languages/go.rs`
- `citadel-core/src/extraction/languages/java.rs`

#### 8.4 TS fast path
- `src/extraction/index.ts` — agregar `'rust'`, `'go'`, `'java'` a `NATIVE_EXTRACTOR_LANGS`

**Verificación:** `test_native_extraction()` con 30 archivos reales sin segfault. `npm test` completo.

---

## Dependencias entre Fases

```
Fase 0 ──► Fase 1 ──► Fase 2 ──► Fase 6 ──► Fase 7
  │                      │
  └──► Fase 3 ──► Fase 4 ──► Fase 5
                                  │
                                  └──► Fase 8
```

## Commits (10 total)

| # | Fase | Mensaje |
|---|------|---------|
| 1 | 0 | refactor: unified CitadelError type + ExtractionErrorKind enum |
| 2 | 1 | refactor: decompose Storage monolith into focused traits (ISP) |
| 3 | 2 | refactor: extract GraphQuery trait from Storage |
| 4 | 3 | feat: add FileSystem abstraction with LocalFileSystem |
| 5 | 4 | feat: bridge ResolutionContext to Storage via adapter |
| 6 | 5 | refactor: strategy pattern for ContextBuilder search + formatting |
| 7 | 6 | refactor: unify NAPI bridge, eliminate TS GraphTraverser duplication |
| 8 | 7 | refactor: split ExtractionOrchestrator into focused modules (SRP) |
| 9 | 8a | fix: tree-sitter version alignment + catch_unwind + TreeWalker |
| 10 | 8b | feat: add Rust, Go, Java native extractors + registry pattern |

## Verificación Final

```bash
cargo clippy --all -- -D warnings
cargo test
npm run build
npm test
# Index real: citadel init + citadel index --force en VSCode src/vs (sin segfault)
# Benchmark: extractFiles nativo con 50 archivos TS/RS/GO/JAVA
```
