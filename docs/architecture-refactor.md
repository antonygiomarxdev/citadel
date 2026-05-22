# Plan: Refactorización Arquitectónica de Citadel-Core

## Resumen

Análisis sistemático del codebase completo aplicando SOLID, Clean Code, Clean Architecture. Resultado: **~140 violaciones documentadas** con file:line específico.

El plan cubre los 3 issues abiertos (#10, #11, #12) reparando los cimientos estructurales primero.

---

## Análisis Detallado de Anti-Patrones

### 1. MAGIC NUMBERS (89 ocurrencias)

| Archivo | Línea | Valor | Qué es |
|---------|-------|-------|--------|
| `types.rs` | 332 | `50` | default search limit |
| `types.rs` | 393 | `100` | default max depth |
| `types.rs` | 394 | `1000` | default traversal limit |
| `types.rs` | 400 | `100` | duplicado de L393 |
| `types.rs` | 404 | `1000` | duplicado de L394 |
| `context/mod.rs` | 36-37 | `50`, `20` | maxNodes, maxFiles defaults |
| `context/mod.rs` | 153 | `10` | top 10 entry points |
| `context/mod.rs` | 235-236 | `3`, `3` | context lines before/after |
| `context/search.rs` | 22 | `2` | min symbol length CamelCase |
| `context/search.rs` | 31 | `3` | min symbol length snake_case |
| `storage/mod.rs` | 321,331,341 | `1000` | traversals limit (3x duplicado) |
| `storage/mod.rs` | 357,362 | `20` | max depth (2x duplicado) |
| `sqlite.rs` | 30 | `120000` | busy_timeout |
| `sqlite.rs` | 32 | `-64000` | cache_size 64MB |
| `sqlite.rs` | 34 | `268435456` | mmap_size 256MB |
| `sqlite.rs` | 93,101,114,122 | `1→4` | schema version migration |
| `sqlite.rs` | 244 | `4096` | Vec pre-allocation |
| `sqlite.rs` | 529 | `256` | file pre-allocation |
| `sqlite.rs` | 980 | `100` | get_ancestors loop guard |
| `import_resolver.rs` | 58-82 | `200,100,15,50,80,10` | scoring weights |
| `name_matcher.rs` | 12-129 | `0.9,0.95,0.5,0.85,100,80,15,50,80,25,10,200` | scoring + confidence |
| `extraction/index.ts` | 52,59,66 | `10, 10000, 250` | batch size, timeout, recycle |
| `db/queries.ts` | 159 | `1000` | LRU cache size |
| `db/queries.ts` | 489 | `100` | search limit default |
| `db/queries.ts` | 544,663,681 | `20, 100, 5` | SQL limits |
| `db/queries.ts` | 735,840,878,931 | `100, 50, 8, 30` | query limits |
| `db/index.ts` | 47-50 | `120000, -64000, 268435456` | duplicados SQLite PRAGMA |
| `context/index.ts` | 154-178 | `20, 5, 1500, 3, 1, 0.3, 20` | defaults duplicados |
| `graph/traversal.ts` | 14 | `1000` | default limit |
| `resolution/index.ts` | 674, 785 | `5000, 0.9` | batch size, confidence |
| `types.ts` (TS) | 437 | `1024*1024` | maxFileSize 1MB |
| `config.ts` | 43 | `500` | regex length check |

**Fix:** centralizar en archivos de constantes tipadas por dominio:
- `citadel-core/src/constants.rs` — `DEFAULT_SEARCH_LIMIT`, `DEFAULT_MAX_DEPTH`, `DEFAULT_TRAVERSAL_LIMIT`, `SCHEMA_VERSION`, etc.
- `src/constants.ts` — ídem TS-side
- SQLite PRAGMAs en `SqlitePragmas` struct con defaults documentados
- Scoring weights en `scoring.rs` como named constants con rationale

### 2. MAGIC STRINGS (127+ ocurrencias)

| Archivo | Línea | String | Categoría |
|---------|-------|--------|-----------|
| `types.rs` | 36-221 | `"file"`, `"class"`, `"struct"`, `"contains"`, `"calls"`, `"typescript"`, `"javascript"`, ... | 22+12+24 strings duplicados entre `as_str()` y `from_str()` |
| `sqlite.rs` | 26-32 | `"WAL"`, `"ON"`, `"NORMAL"`, `"MEMORY"` | PRAGMA values |
| `sqlite.rs` | 1405-1442 | 22+12+24 strings | parse_node_kind, parse_edge_kind, parse_language — triplicados de types.rs |
| `extraction/mod.rs` | 41-64 | `".ts"`, `".tsx"`, ... (30+) | extensiones de archivo |
| `typescript.rs` | 66-100 | `"function_declaration"`, `"method_definition"`, `"class_declaration"`, ... (13) | tree-sitter node kinds |
| `python.rs` | 50 | `"function_definition"`, `"class_definition"`, ... | tree-sitter node kinds |
| `import_resolver.rs` | 10-24 | `".ts"`, `".tsx"`, `".d.ts"`, ... (30+) | extension resolution |
| `import_resolver.rs` | 103-120 | `"typescript"`, `"javascript"`, ... | language dispatch |
| `resolution/index.ts` | 38-92 | `'console'`, `'window'`, `'Promise'`, `'useState'`, `'print'`, `'len'`, `'fmt'`, `'os'`, ... (100+) | built-ins por lenguaje |
| `context/index.ts` | 82-118 | `'the'`, `'and'`, `'for'`, ... (100+) | stop words |
| `db/index.ts` | 45-51 | `'foreign_keys = ON'`, `'journal_mode = WAL'`, ... | PRAGMAs duplicados |
| `db/sqlite-adapter.ts` | 161-170 | `'DELETE'`, `'FULL'`, `'BEGIN'`, `'COMMIT'`, `'ROLLBACK'` | SQL strings |
| `types.ts` | 14-37 | `'file'`, `'module'`, `'class'`, ... (22) | NODE_KINDS array |
| `types.ts` | 56-58 | `'typescript'`, `'javascript'`, ... (24) | LANGUAGES array |
| `types.ts` | 318-515 | 150+ glob strings | include/exclude patterns |

**Fix:**
- `NodeKind::as_str()` y `from_str()` generados con macro `#[derive(EnumString, Display)]` para eliminar duplicación manual
- Tree-sitter node kinds como `TS_NODE_KINDS` constantes por extractor
- Extensiones de archivo en `Language::extensions()` método en el enum
- Built-ins por lenguaje en arrays `const` modulares
- SQL PRAGMAs en struct `DatabasePragmas` con defaults

### 3. OCP VIOLATIONS (13 switch/match que requieren modificación para extender)

| # | Archivo | Línea | Dispatch |
|---|---------|-------|----------|
| 1 | `extraction/languages/mod.rs` | 18-25 | `get_extractor()` — match Language |
| 2 | `extraction/mod.rs` | 40-65 | `detect_language()` — if-else 30+ extensiones |
| 3 | `import_resolver.rs` | 10-24 | `extension_order()` — match language string |
| 4 | `import_resolver.rs` | 103-120 | `is_external_import()` — match language string |
| 5 | `typescript.rs` | 66-100 | `walk_tree()` — match 13+ tree-sitter node kinds |
| 6 | `python.rs` | 50-67 | `walk_python()` — match 4 tree-sitter node kinds |
| 7 | `database.rs` (napi) | 31-34 | `with_backend()` — match "sqlite" string |
| 8 | `frameworks/mod.rs` | 12-22 | `all_frameworks()` — vec de 9 frameworks |
| 9 | `grammars.ts` | detectLanguage() | if-else 30+ extensiones (duplicado de #2) |
| 10 | `languages/index.ts` | 32-52 | `EXTRACTORS` — Partial<Record> 18 entries |
| 11 | `frameworks/index.ts` | 30-51 | `FRAMEWORK_RESOLVERS` — array hardcoded |
| 12 | `resolution/index.ts` | 778-842 | `isBuiltInOrExternal()` — 5 if-else paths |
| 13 | `traversal.ts` | 93-95 | edge priority sort — `contains ? 0 : calls ? 1 : 2` |

**Fix:**
- `Language` enum: agregar método `extensions() -> &[&str]`, `builtins() -> &[&str]`
- `get_extractor()` → `LazyLock<HashMap<Language, ExtractorFactory>>` registry
- `detect_language()` → loop sobre `Language::all().find(|l| l.matches_extension(ext))`
- `all_frameworks()` → `inventory` crate (compile-time registration) o macro
- `EXTRACTORS` → `Map<Language, ExtractorFactory>` con registro dinámico
- `isBuiltInOrExternal()` → `Language::is_builtin(name: &str) -> bool`
- tree-sitter node kinds: visitor pattern con `HashMap<&str, Box<dyn NodeHandler>>`
- `with_backend()` → `LazyLock<HashMap<String, BackendFactory>>` registry

### 4. DIP VIOLATIONS (15 dependencias de capas altas a bajas)

| # | Archivo | Línea | Violación |
|---|---------|-------|-----------|
| 1 | `database.rs` | 4,27,33 | NAPI importa + instancia `SqliteStorage` directamente |
| 2 | `extraction/mod.rs` | 137 | Caso de uso depende de `tree_sitter` crate |
| 3 | `typescript.rs` | 5 | Extractor depende de `tree-sitter-typescript` |
| 4 | `python.rs` | 5 | Extractor depende de `tree-sitter-python` |
| 5 | `context/mod.rs` | 231-233 | ContextBuilder llama `std::fs::read_to_string` |
| 6 | `error.rs` | 4 | `StorageError::Io` envuelve `std::io::Error` |
| 7 | `sqlite-adapter.ts` | dynamic require | `require('better-sqlite3')` / `require('node-sqlite3-wasm')` |
| 8 | `extraction/index.ts` | dynamic require | `require('../citadel-native.linux-x64-gnu.node')` |
| 9 | `extraction/index.ts` | 159-161 | `execFileSync('git', ...)` directo |
| 10 | `context/index.ts` | 11 | `import * as fs from 'fs'` en ContextBuilder |
| 11 | `resolution/index.ts` | 11 | `import * as fs from 'fs'` en ReferenceResolver |
| 12 | `db/index.ts` | direct import | `import Database from 'better-sqlite3'` |
| 13 | `citadel-core/src/lib.rs` | re-exports | `pub use storage::sqlite::SqliteStorage` expone implementación concreta |
| 14 | `extraction/languages/mod.rs` | use statements | `use super::typescript::TypeScriptExtractor` — dependencia concreta entre módulos |
| 15 | `resolution/frameworks/mod.rs` | use statements | `use super::express::ExpressResolver` — 9 imports concretos |

**Fix:**
- NAPI Database → recibe `Box<dyn FullStore>` por constructor, factory externa
- `SqliteStorage` no se re-exporta desde `lib.rs`
- `TreeSitterParser` trait abstrae `tree_sitter::Parser`
- `FileSystem` trait (Phase 3 del plan anterior) — implementación `LocalFileSystem` + `VirtualFileSystem` para tests
- `VcsProvider` trait — `GitProvider` implementa `execFileSync('git')`, mock para tests
- `StorageBackend` trait para `sqlite-adapter.ts` — `NativeSqliteBackend`, `WasmSqliteBackend`
- `NativeExtractor` interface en TS — `LinuxX64Extractor`, `DarwinArm64Extractor`

### 5. LSP VIOLATIONS (6 subtipos que no sustituyen correctamente)

| # | Archivo | Línea | Violación |
|---|---------|-------|-----------|
| 1 | `storage/mod.rs` | 84-120 | `search_nodes` default es O(n) — frágil para backends sin FTS |
| 2 | `storage/mod.rs` | 143-159 | `find_edges_between_nodes` default es O(n²) |
| 3 | `frameworks/nestjs.rs` | 17-19 | `resolve()` retorna `None` siempre (y 8 de 9 frameworks igual) |
| 4 | `frameworks/express.rs` | 37-39 | ídem |
| 5 | `frameworks/django.rs` | 44-46 | ídem |
| 6 | `resolution/types.ts` | 133-138 | Métodos opcionales que cambian comportamiento cualitativo |

**Fix:**
- `search_nodes` y `find_edges_between_nodes` → sin default, requeridos
- `FrameworkResolver` → separar en `FrameworkDetector` (siempre implementado) + `FrameworkResolver` (opcional vía `Option` return type explícito en el registry, no en el trait)
- `ResolutionContext` → split en `BasicResolutionContext` (métodos requeridos) + `ExtendedResolutionContext` (opcionales)
- `LanguageExtractor` → rename a `LanguageParser` y exigir `parse()` obligatorio

---

## Clean Architecture Layer Map

### Entity Layer (innermost — zero dependencies)
```
citadel-core/src/types.rs
├── Node, Edge, NodeKind, EdgeKind, Language  ← puros, sin deps
├── ExtractionResult, ExtractionError        ← DEBERÍAN ser puros
│   └── ❌ ExtractionError usa String severity, no enum
├── TraversalOptions, SearchOptions          ← puros
└── ❌ FromStr impls retornan Result<_, ()> en vez de error tipado
```

### Use Case Layer (business logic)
```
citadel-core/src/context/mod.rs              ← ContextBuilder
  ❌ DIP: std::fs::read_to_string directo
  ❌ OCP: estrategias de búsqueda hardcodeadas
  ✅ DI: recibe Box<dyn Storage> por constructor (pero debería ser &dyn)
citadel-core/src/resolution/mod.rs           ← ReferenceResolver
  ✅ DI: recibe Vec<Box<dyn FrameworkResolver>> por constructor
  ❌ ResolutionContext sin implementación concreta (dead code)
  ❌ LSP: FrameworkResolver::resolve() es stub en 8/9 implementaciones
citadel-core/src/graph/                      ← NO EXISTE como módulo
  ❌ La lógica de traversal vive en Storage (adapter layer)
```

### Interface Adapter Layer (bridges)
```
citadel-core/src/storage/mod.rs              ← Storage trait
  ❌ ISP: 50 métodos en un solo trait
  ❌ Violación de capa: incluye lógica de traversal (use case)
citadel-core/src/storage/traits/             ← NO EXISTE (debería)
citadel-core/src/extraction/mod.rs           ← LanguageExtractor trait
  ❌ DIP: usa tree_sitter::Parser directamente
  ❌ OCP: detect_language() con if-else de 30+ extensiones
citadel-napi/src/database.rs                 ← Database class
  ❌ DIP: instancia SqliteStorage directamente
  ❌ God Object: 40+ métodos en una clase
  ❌ 366 líneas de boilerplate lock→call→serialize→map_err
citadel-napi/src/extraction.rs               ← extract_files()
  ✅ Interfaz limpia: función pura, Vec<Vec<String>> → Vec<JsExtractionResult>
  ❌ Sin catch_unwind en FFI boundary
```

### Framework Layer (outermost — concrete implementations)
```
citadel-core/src/storage/sqlite.rs           ← SqliteStorage
  ✅ Implementa Storage trait
  ❌ parse_node_kind/parse_edge_kind/parse_language duplican types.rs
  ❌ DFS sin override SQL-optimizado (usa default N+1)
citadel-core/src/extraction/languages/       ← language extractors
  ❌ typescript.rs y python.rs usan walk_tree recursivo sin depth limit
  ❌ tree-sitter version mismatch (0.24 core vs 0.23 grammar)
src/db/sqlite-adapter.ts                     ← NativeSqliteAdapter
  ❌ DIP: require('better-sqlite3') directo
src/extraction/grammars.ts                   ← WASM tree-sitter
src/extraction/worker.ts                     ← Worker thread
```

### Dependency Rule Violations (dependencias hacia afuera)

```
Entity ────────────────────────────────────────────────────────────► types.rs → from_str() retorna ()
                                                                      (dependencia innecesaria en std::str::FromStr con error type pobre)

Use Case ───────────► context/mod.rs ─────► std::fs::read_to_string  (framework detail)
                     context/mod.rs ─────► estrategias hardcodeadas  (sin abstracción)
                     resolution/mod.rs ──► ResolutionContext trait   (sin adapter a Storage)

Adapter ────────────► storage/mod.rs ────► lógica de BFS/DFS         (use case en capa equivocada)
                     extraction/mod.rs ──► tree_sitter::Parser       (framework en adapter)
                     napi/database.rs ───► SqliteStorage::new()      (framework en adapter)

Framework ──────────► sqlite.rs ─────────► PRAGMAs hardcodeados      (sin configuración tipada)
                     typescript.rs ──────► tree-sitter 0.23          (version mismatch)
```

---

## Plan de Refactorización (Fases Revisadas)

### Fase -1: Documentar el estado actual

1. Committear `docs/architecture-refactor.md` con este análisis completo. ← **HECHO**
2. Crear issues de GitHub para cada categoría de violación (Magic Numbers, Magic Strings, OCP, DIP, LSP)
3. Agregar `cargo deny` al CI para evitar que nuevas DIP violations entren

### Fase 0: Infraestructura de Errores + Constantes

**Nuevos archivos:**
- `citadel-core/src/error.rs` — `CitadelError` enum unificado
- `citadel-core/src/constants.rs` — todas las constantes mágicas con nombres y documentación

**Cambios:**
- `types.rs` — `ExtractionError` gana `kind: ExtractionErrorKind`; `FromStr` impls usan `CitadelError`
- `storage/error.rs` — deprecar `StorageError`, re-exportar `CitadelError`
- `sqlite.rs` — usar constantes de `constants.rs` para PRAGMAs, schema version, pre-allocations
- `context/mod.rs` — `FindContextOptions` defaults desde constantes
- `context/search.rs` — min symbol lengths desde constantes
- `import_resolver.rs` + `name_matcher.rs` — scoring weights desde `constants.rs`
- Todos los TS files con magic numbers — importar desde `src/constants.ts`

**Verificación:** `cargo clippy`, `cargo test`, `npm test`

### Fase 1: Descomponer Storage (ISP) + Eliminar Magic Strings

**Nuevo archivo:** `citadel-core/src/storage/traits.rs` — traits enfocados:
```rust
pub trait NodeStore { ... }       // 12 métodos
pub trait EdgeStore { ... }       // 6 métodos
pub trait FileStore { ... }       // 7 métodos
pub trait UnresolvedRefStore { ... }  // 11 métodos
pub trait MetadataStore { ... }   // 3 métodos
pub trait StatsProvider { ... }   // 2 métodos
pub trait Lifecycle { ... }       // 5 métodos
pub trait FullStore: NodeStore + EdgeStore + FileStore + UnresolvedRefStore
    + MetadataStore + StatsProvider + Lifecycle + Send + Sync {}
```

**Eliminar magic strings en `types.rs`:**
- Usar `strum` derive macros: `#[derive(EnumString, Display)]` para `NodeKind`, `EdgeKind`, `Language`
- Eliminar implementaciones manuales de `as_str()` y `from_str()` (~100 líneas)
- `parse_node_kind`/`parse_edge_kind`/`parse_language` en `sqlite.rs` → usar `FromStr`
- `detect_language()` en `extraction/mod.rs` → `Language::from_extension(ext: &str)`

**Archivos modificados:**
- `storage/traits.rs` — nuevo
- `storage/mod.rs` — `pub use traits::FullStore as Storage;`
- `sqlite.rs` — reorganizar en `impl` blocks por trait
- `types.rs` — strum derives + eliminar duplicación
- `extraction/mod.rs` — `Language::from_extension()`
- `database.rs` (napi) — `parse_*` funciones → usar `FromStr`

### Fase 2: Extraer Graph + Resolver LSP

**Nuevo archivo:** `citadel-core/src/graph/mod.rs` — `GraphQuery` trait con blanket impl

**LSP fixes:**
- `search_nodes` y `find_edges_between_nodes` → requeridos (sin default frágil)
- `FrameworkResolver` → split: `trait FrameworkDetector { fn detect() }` + `trait FrameworkResolver { fn resolve() }` (solo NestJS implementa resolve hoy)
- `ResolutionContext` → métodos opcionales en trait separado: `ExtendedResolutionContext`
- `SqliteStorage` → implementar `GraphQuery` con overrides para TODOS los métodos de traversal (no solo BFS)

### Fase 3: FileSystem + VcsProvider (DIP)

**Nuevos archivos:**
- `citadel-core/src/filesystem.rs` — `FileSystem` trait + `LocalFileSystem`
- `src/extraction/vcs.ts` — `VcsProvider` interface + `GitProvider`

### Fase 4: Puente ResolutionContext + Registry OCP

**Nuevo archivo:** `citadel-core/src/resolution/storage_adapter.rs`

**OCP fixes:**
- `get_extractor()` → `LazyLock<HashMap<Language, ExtractorFactory>>`
- `all_frameworks()` → registro dinámico
- `with_backend()` → `LazyLock<HashMap<String, BackendFactory>>`
- `EXTRACTORS` (TS) → `Map` con registro
- `isBuiltInOrExternal()` → `Language::is_builtin(name) -> bool`

### Fase 5: ContextBuilder Strategy Pattern

**Nuevos archivos:**
- `citadel-core/src/context/strategies.rs`
- `citadel-core/src/context/formatting.rs`

### Fase 6: Unificar NAPI Bridge + TreeWalker

**Nuevos archivos:**
- `citadel-napi/src/macros.rs`
- `citadel-core/src/extraction/walker.rs` — `TreeWalker` + `NodeVisitor` trait

### Fase 7: SRP ExtractionOrchestrator TS

**Nuevos archivos:**
- `src/extraction/file-scanner.ts`
- `src/extraction/worker-pool.ts`
- `src/extraction/store.ts`

### Fase 8: Crash Fixes + Nuevos Extractores

- Tree-sitter version alignment
- `catch_unwind` per-file + FFI boundary
- Rust, Go, Java extractors (~100 líneas c/u con TreeWalker)

---

## Verificación por Fase

```bash
# Cada fase
cargo clippy --all -- -D warnings
cargo test
npm run build
npm test

# Fase 1 extra
grep -r '[0-9]{2,}' citadel-core/src/ | grep -v '0\|1\|test\|mod\|use\|//'
# ^^ no debe retornar nuevos magic numbers sin nombre
```
