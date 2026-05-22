import type { Language, NodeKind } from '../types.js';

export interface BuiltInRule {
  /** The symbol name (e.g. 'fs', 'os', 'log.Println', 'print') */
  name: string;
  /** Optional: only match if the reference has this language */
  language?: Language | Language[];
  /** Optional: only match if the reference has this kind */
  referenceKind?: NodeKind | NodeKind[];
  /** Match whole name exactly */
  pattern?: 'exact';
  /** Match name.startsWith(prefix) */
  prefixOf?: boolean;
  /** The name is a module with members, so 'name.*' also matches */
  hasMembers?: boolean;
}

/**
 * Registry of built-in / external symbols that should be excluded
 * from resolution. Replaces the 90-line `isBuiltInOrExternal()`
 * cascading if-else in ReferenceResolver.
 *
 * Rules registered here short-circuit resolution — references
 * matching any rule are considered unresolvable (external/built-in).
 */
export class BuiltInRegistry {
  // Separate indices for O(1) lookup
  private names: Set<string> = new Set();
  private prefixModules: Set<string> = new Set();
  private languageRules: Map<string, BuiltInRule[]> = new Map();
  private globalRules: BuiltInRule[] = [];

  /**
   * Register a built-in rule.
   */
  register(rule: BuiltInRule): void {
    if (!rule.language) {
      this.globalRules.push(rule);
    } else {
      const langs = Array.isArray(rule.language) ? rule.language : [rule.language];
      for (const lang of langs) {
        const list = this.languageRules.get(lang) || [];
        list.push(rule);
        this.languageRules.set(lang, list);
      }
    }

    if (rule.hasMembers) {
      this.prefixModules.add(rule.name + '.');
    }
    this.names.add(rule.name);
  }

  /**
   * Register multiple rules at once.
   */
  registerAll(rules: BuiltInRule[]): void {
    for (const rule of rules) {
      this.register(rule);
    }
  }

  /**
   * Check if a name is a known built-in.
   */
  isBuiltIn(name: string, language?: Language, kind?: NodeKind): boolean {
    // Fast path: O(1) direct name lookup
    if (this.names.has(name)) {
      return this.checkRules(name, language, kind);
    }

    // Check dot-prefixed module members (e.g., 'fs.readFileSync'
    // matches module 'fs' if 'fs' rule has hasMembers=true)
    for (const prefix of this.prefixModules) {
      if (name.startsWith(prefix)) {
        const moduleName = prefix.slice(0, -1);
        // Recurse to check the base module name
        return this.checkRules(moduleName, language, kind);
      }
    }

    return false;
  }

  /**
   * Check whether a known built-in name matches language/kind constraints.
   */
  private checkRules(name: string, language?: Language, kind?: NodeKind): boolean {
    // Check language-specific rules
    if (language) {
      const rules = this.languageRules.get(language);
      if (rules) {
        for (const rule of rules) {
          if (rule.name === name) {
            if (rule.referenceKind && kind) {
              const kinds = Array.isArray(rule.referenceKind) ? rule.referenceKind : [rule.referenceKind];
              if (!kinds.includes(kind)) continue;
            }
            return true;
          }
        }
      }
    }

    // Check global rules
    for (const rule of this.globalRules) {
      if (rule.name === name) return true;
    }

    return false;
  }
}

/**
 * Create the default built-in registry with all hardcoded rules.
 */
export function createDefaultBuiltInRegistry(): BuiltInRegistry {
  const registry = new BuiltInRegistry();

  // ── JavaScript / TypeScript ──
  const jsBuiltins = [
    'fs', 'path', 'os', 'crypto', 'http', 'https', 'net', 'tls',
    'dns', 'stream', 'util', 'events', 'buffer', 'url', 'querystring',
    'assert', 'child_process', 'cluster', 'readline', 'repl', 'vm',
    'zlib', 'dgram', 'dns/promises', 'fs/promises', 'timers/promises',
    'v8', 'worker_threads', 'perf_hooks', 'async_hooks', 'wasi',
    'diagnostics_channel', 'inspector', 'trace_events', 'string_decoder',
    'tty', 'module',
  ];
  for (const name of jsBuiltins) {
    registry.register({
      name,
      language: ['typescript', 'javascript', 'tsx', 'jsx'],
      hasMembers: true,
    });
  }

  const reactHooks = [
    'useState', 'useEffect', 'useContext', 'useReducer', 'useCallback',
    'useMemo', 'useRef', 'useImperativeHandle', 'useLayoutEffect',
    'useDebugValue', 'useDeferredValue', 'useTransition', 'useId',
    'useSyncExternalStore', 'useInsertionEffect',
  ];
  for (const name of reactHooks) {
    registry.register({
      name,
      language: ['typescript', 'javascript', 'tsx', 'jsx'],
    });
  }

  // Console methods
  const consoleMethods = ['log', 'error', 'warn', 'info', 'debug', 'trace',
    'assert', 'clear', 'count', 'countReset', 'dir', 'dirxml', 'group',
    'groupCollapsed', 'groupEnd', 'table', 'time', 'timeEnd', 'timeLog',
    'timeStamp', 'profile', 'profileEnd'];
  for (const name of consoleMethods) {
    registry.register({
      name: `console.${name}`,
      language: ['typescript', 'javascript', 'tsx', 'jsx'],
    });
  }

  // ── Python ──
  const pythonBuiltins = [
    'os', 'sys', 'json', 're', 'math', 'random', 'datetime', 'collections',
    'itertools', 'functools', 'typing', 'pathlib', 'argparse', 'logging',
    'hashlib', 'base64', 'uuid', 'copy', 'enum', 'io', 'csv', 'pickle',
    'shutil', 'tempfile', 'subprocess', 'threading', 'multiprocessing',
    'asyncio', 'socket', 'email', 'urllib', 'xml', 'html', 'http',
    'unittest', 'pdb', 'traceback', 'warnings', 'contextlib', 'abc',
    'dataclasses', 'textwrap', 'pprint', 'statistics',
  ];
  for (const name of pythonBuiltins) {
    registry.register({ name, language: 'python', hasMembers: true });
  }

  const pythonBuiltinTypes = ['str', 'int', 'float', 'bool', 'list', 'dict',
    'tuple', 'set', 'frozenset', 'bytes', 'bytearray', 'memoryview',
    'range', 'slice', 'type', 'object', 'None', 'True', 'False',
    'Ellipsis', 'NotImplemented'];
  for (const name of pythonBuiltinTypes) {
    registry.register({ name, language: 'python' });
  }

  const pythonBuiltinMethods = [
    'append', 'extend', 'insert', 'remove', 'pop', 'clear', 'index',
    'count', 'sort', 'reverse', 'copy', 'keys', 'values', 'items',
    'get', 'update', 'setdefault', 'popitem', 'split', 'join', 'strip',
    'lstrip', 'rstrip', 'upper', 'lower', 'capitalize', 'title', 'swapcase',
    'replace', 'find', 'rfind', 'startswith', 'endswith', 'encode', 'decode',
    'format', 'isalpha', 'isdigit', 'isalnum', 'isspace', 'islower', 'isupper',
    'add', 'discard', 'union', 'intersection', 'difference', 'symmetric_difference',
    'issubset', 'issuperset', 'isdisjoint', 'update',
  ];
  for (const name of pythonBuiltinMethods) {
    registry.register({ name, language: 'python' });
  }

  // ── Go ──
  const goStdlib = [
    'fmt', 'net', 'http', 'os', 'io', 'strings', 'strconv', 'bytes',
    'errors', 'sync', 'context', 'time', 'encoding/json', 'encoding/xml',
    'encoding/csv', 'encoding/base64', 'encoding/hex', 'encoding/binary',
    'math', 'math/rand', 'sort', 'path', 'path/filepath', 'log',
    'flag', 'testing', 'reflect', 'regexp', 'unicode', 'crypto',
  ];
  for (const name of goStdlib) {
    registry.register({ name, language: 'go', hasMembers: true });
  }

  const goBuiltins = ['append', 'cap', 'close', 'complex', 'copy', 'delete',
    'imag', 'len', 'make', 'new', 'panic', 'print', 'println', 'real', 'recover'];
  for (const name of goBuiltins) {
    registry.register({ name, language: 'go' });
  }

  // ── Pascal ──
  const pascalBuiltins = ['System', 'SysUtils', 'Classes', 'Math', 'StrUtils',
    'DateUtils', 'TypInfo', 'Variants', 'Generics.Collections', 'Generics.Defaults'];
  for (const name of pascalBuiltins) {
    registry.register({ name, language: 'pascal', hasMembers: true });
  }

  return registry;
}

// Singleton instance for production use
let defaultRegistry: BuiltInRegistry | null = null;

export function getDefaultBuiltInRegistry(): BuiltInRegistry {
  if (!defaultRegistry) {
    defaultRegistry = createDefaultBuiltInRegistry();
  }
  return defaultRegistry;
}
