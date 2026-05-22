import type { Language } from '../types.js';

/**
 * Maps file extensions to languages and provides extension lists
 * for import resolution (try .ts, .tsx, .js, etc. variants).
 *
 * Replaces the static `EXTENSION_RESOLUTION` map in `import-resolver.ts`.
 */
export class ExtensionRegistry {
  private langExtensions: Map<Language, string[]> = new Map();
  private extToLanguage: Map<string, Language> = new Map();

  /**
   * Register extension mappings for a language.
   * @param language - Target language
   * @param extensions - File extensions including dot (e.g. ['.ts', '.tsx'])
   */
  register(language: Language, extensions: string[]): void {
    this.langExtensions.set(language, extensions);
    for (const ext of extensions) {
      this.extToLanguage.set(ext, language);
    }
  }

  /**
   * Get the extensions to try when resolving an import in `language`.
   */
  resolveOrder(language: Language): string[] {
    return this.langExtensions.get(language) ?? [];
  }

  /**
   * Get the language associated with a file extension.
   */
  languageForExtension(ext: string): Language | undefined {
    return this.extToLanguage.get(ext);
  }
}

/**
 * Create the default extension registry with all language mappings.
 */
export function createDefaultExtensionRegistry(): ExtensionRegistry {
  const registry = new ExtensionRegistry();

  registry.register('typescript', ['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs', '/index.ts', '/index.tsx', '/index.js', '/index.jsx']);
  registry.register('javascript', ['.js', '.jsx', '.mjs', '.cjs', '/index.js', '/index.jsx']);
  registry.register('tsx', ['.tsx', '.ts', '.jsx', '.js', '/index.tsx', '/index.ts']);
  registry.register('jsx', ['.jsx', '.js', '/index.jsx', '/index.js']);
  registry.register('python', ['.py', '/__init__.py']);
  registry.register('go', ['.go']);
  registry.register('java', ['.java']);
  registry.register('c', ['.c', '.h']);
  registry.register('cpp', ['.cpp', '.cxx', '.cc', '.c++', '.hpp', '.hxx', '.hh', '.h++']);
  registry.register('csharp', ['.cs']);
  registry.register('php', ['.php']);
  registry.register('ruby', ['.rb']);
  registry.register('swift', ['.swift']);
  registry.register('kotlin', ['.kt', '.kts']);
  registry.register('dart', ['.dart']);
  registry.register('rust', ['.rs']);
  registry.register('scala', ['.scala', '.sc']);
  registry.register('lua', ['.lua']);
  registry.register('luau', ['.luau', '.lua']);
  registry.register('svelte', ['.svelte']);
  registry.register('vue', ['.vue']);
  registry.register('liquid', ['.liquid']);
  registry.register('pascal', ['.pas', '.pp', '.dpr', '.dpk']);

  return registry;
}

let defaultRegistry: ExtensionRegistry | null = null;

export function getDefaultExtensionRegistry(): ExtensionRegistry {
  if (!defaultRegistry) {
    defaultRegistry = createDefaultExtensionRegistry();
  }
  return defaultRegistry;
}
