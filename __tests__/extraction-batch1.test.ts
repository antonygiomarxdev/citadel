/**
 * Extraction Tests
 *
 * Tests for the tree-sitter extraction system.
 */

import { describe, it, expect, beforeAll, beforeEach, afterEach } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import { CodeGraph } from '../src';
import { extractFromSource, scanDirectory, shouldIncludeFile } from '../src/extraction';
import { detectLanguage, isLanguageSupported, getSupportedLanguages, initGrammars, loadAllGrammars } from '../src/extraction/grammars';
import { normalizePath } from '../src/utils';
import { DEFAULT_CONFIG } from '../src/types';

beforeAll(async () => {
  await initGrammars();
  await loadAllGrammars();
});

// Create a temporary directory for each test
function createTempDir(): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'codegraph-test-'));
}

// Clean up temporary directory
function cleanupTempDir(dir: string): void {
  if (fs.existsSync(dir)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

describe('Language Detection', () => {
  it('should detect TypeScript files', () => {
    expect(detectLanguage('src/index.ts')).toBe('typescript');
    expect(detectLanguage('components/Button.tsx')).toBe('tsx');
  });

  it('should detect JavaScript files', () => {
    expect(detectLanguage('index.js')).toBe('javascript');
    expect(detectLanguage('App.jsx')).toBe('jsx');
    expect(detectLanguage('config.mjs')).toBe('javascript');
  });

  it('should detect Python files', () => {
    expect(detectLanguage('main.py')).toBe('python');
  });

  it('should detect Go files', () => {
    expect(detectLanguage('main.go')).toBe('go');
  });

  it('should detect Rust files', () => {
    expect(detectLanguage('lib.rs')).toBe('rust');
  });

  it('should detect Java files', () => {
    expect(detectLanguage('Main.java')).toBe('java');
  });

  it('should detect C files', () => {
    expect(detectLanguage('main.c')).toBe('c');
    expect(detectLanguage('utils.h')).toBe('c');
  });

  it('should detect C++ files', () => {
    expect(detectLanguage('main.cpp')).toBe('cpp');
    expect(detectLanguage('class.hpp')).toBe('cpp');
  });

  it('should detect C# files', () => {
    expect(detectLanguage('Program.cs')).toBe('csharp');
  });

  it('should detect PHP files', () => {
    expect(detectLanguage('index.php')).toBe('php');
  });

  it('should detect Ruby files', () => {
    expect(detectLanguage('app.rb')).toBe('ruby');
  });

  it('should detect Swift files', () => {
    expect(detectLanguage('ViewController.swift')).toBe('swift');
  });

  it('should detect Kotlin files', () => {
    expect(detectLanguage('MainActivity.kt')).toBe('kotlin');
    expect(detectLanguage('build.gradle.kts')).toBe('kotlin');
  });

  it('should detect Dart files', () => {
    expect(detectLanguage('main.dart')).toBe('dart');
  });

  it('should return unknown for unsupported extensions', () => {
    expect(detectLanguage('styles.css')).toBe('unknown');
    expect(detectLanguage('data.json')).toBe('unknown');
  });
});

describe('Language Support', () => {
  it('should report supported languages', () => {
    expect(isLanguageSupported('typescript')).toBe(true);
    expect(isLanguageSupported('python')).toBe(true);
    expect(isLanguageSupported('go')).toBe(true);
    expect(isLanguageSupported('unknown')).toBe(false);
  });

  it('should list all supported languages', () => {
    const languages = getSupportedLanguages();
    expect(languages).toContain('typescript');
    expect(languages).toContain('javascript');
    expect(languages).toContain('python');
    expect(languages).toContain('go');
    expect(languages).toContain('rust');
    expect(languages).toContain('java');
    expect(languages).toContain('csharp');
    expect(languages).toContain('php');
    expect(languages).toContain('ruby');
    expect(languages).toContain('swift');
    expect(languages).toContain('kotlin');
    expect(languages).toContain('dart');
  });
});

describe('TypeScript Extraction', () => {
  it('should extract function declarations', () => {
    const code = `
export function processPayment(amount: number): Promise<Receipt> {
  return stripe.charge(amount);
}
`;
    const result = extractFromSource('payment.ts', code);

    // File node + function node
    const fileNode = result.nodes.find((n) => n.kind === 'file');
    expect(fileNode).toBeDefined();
    expect(fileNode?.name).toBe('payment.ts');

    const funcNode = result.nodes.find((n) => n.kind === 'function');
    expect(funcNode).toMatchObject({
      kind: 'function',
      name: 'processPayment',
      language: 'typescript',
      isExported: true,
    });
    expect(funcNode?.signature).toContain('amount: number');
  });

  it('should extract class declarations', () => {
    const code = `
export class PaymentService {
  private stripe: StripeClient;

  constructor(apiKey: string) {
    this.stripe = new StripeClient(apiKey);
  }

  async charge(amount: number): Promise<Receipt> {
    return this.stripe.charge(amount);
  }
}
`;
    const result = extractFromSource('service.ts', code);

    const classNode = result.nodes.find((n) => n.kind === 'class');
    const methodNodes = result.nodes.filter((n) => n.kind === 'method');

    expect(classNode).toBeDefined();
    expect(classNode?.name).toBe('PaymentService');
    expect(classNode?.isExported).toBe(true);

    expect(methodNodes.length).toBeGreaterThanOrEqual(1);
    const chargeMethod = methodNodes.find((m) => m.name === 'charge');
    expect(chargeMethod).toBeDefined();
  });

  it('should extract interfaces', () => {
    const code = `
export interface User {
  id: string;
  name: string;
  email: string;
}
`;
    const result = extractFromSource('types.ts', code);

    const fileNode = result.nodes.find((n) => n.kind === 'file');
    expect(fileNode).toBeDefined();

    const ifaceNode = result.nodes.find((n) => n.kind === 'interface');
    expect(ifaceNode).toMatchObject({
      kind: 'interface',
      name: 'User',
      isExported: true,
    });
  });

  it('should track function calls', () => {
    const code = `
function main() {
  const result = processData();
  console.log(result);
}
`;
    const result = extractFromSource('main.ts', code);

    expect(result.unresolvedReferences.length).toBeGreaterThan(0);
    const calls = result.unresolvedReferences.filter((r) => r.referenceKind === 'calls');
    expect(calls.some((c) => c.referenceName === 'processData')).toBe(true);
  });
});

describe('Arrow Function Export Extraction', () => {
  it('should extract exported arrow functions assigned to const', () => {
    const code = `
export const useAuth = (): AuthContextValue => {
  return useContext(AuthContext);
};
`;
    const result = extractFromSource('hooks.ts', code);

    const funcNode = result.nodes.find((n) => n.kind === 'function' && n.name === 'useAuth');
    expect(funcNode).toBeDefined();
    expect(funcNode).toMatchObject({
      kind: 'function',
      name: 'useAuth',
      isExported: true,
    });
  });

  it('should extract exported function expressions assigned to const', () => {
    const code = `
export const processData = function(input: string): string {
  return input.trim();
};
`;
    const result = extractFromSource('utils.ts', code);

    const funcNode = result.nodes.find((n) => n.kind === 'function' && n.name === 'processData');
    expect(funcNode).toBeDefined();
    expect(funcNode).toMatchObject({
      kind: 'function',
      name: 'processData',
      isExported: true,
    });
  });

  it('should not extract non-exported arrow functions as exported', () => {
    const code = `
const internalHelper = () => {
  return 42;
};
`;
    const result = extractFromSource('internal.ts', code);

    const helperNode = result.nodes.find((n) => n.name === 'internalHelper');
    expect(helperNode).toBeDefined();
    expect(helperNode?.isExported).toBeFalsy();
  });

  it('should still skip truly anonymous arrow functions', () => {
    const code = `
const items = [1, 2, 3].map((x) => x * 2);
`;
    const result = extractFromSource('anon.ts', code);

    // The inline arrow function passed to .map() has no variable_declarator parent
    // and should remain anonymous (skipped)
    const anonFunctions = result.nodes.filter(
      (n) => n.kind === 'function' && n.name === '<anonymous>'
    );
    expect(anonFunctions).toHaveLength(0);
  });

  it('should extract multiple exported arrow functions from the same file', () => {
    const code = `
export const add = (a: number, b: number): number => a + b;

export const subtract = (a: number, b: number): number => a - b;

const internal = () => 'not exported';
`;
    const result = extractFromSource('math.ts', code);

    const exported = result.nodes.filter((n) => n.kind === 'function' && n.isExported);
    expect(exported).toHaveLength(2);
    expect(exported.map((n) => n.name).sort()).toEqual(['add', 'subtract']);

    const internalNode = result.nodes.find((n) => n.name === 'internal');
    expect(internalNode).toBeDefined();
    expect(internalNode?.isExported).toBeFalsy();
  });

  it('should extract arrow functions in JavaScript files', () => {
    const code = `
export const fetchData = async () => {
  const response = await fetch('/api/data');
  return response.json();
};
`;
    const result = extractFromSource('api.js', code);

    const funcNode = result.nodes.find((n) => n.kind === 'function' && n.name === 'fetchData');
    expect(funcNode).toBeDefined();
    expect(funcNode).toMatchObject({
      kind: 'function',
      name: 'fetchData',
      isExported: true,
    });
  });
});

describe('Type Alias Extraction', () => {
  it('should extract exported type aliases in TypeScript', () => {
    const code = `
export type AuthContextValue = {
  user: User | null;
  login: () => void;
  logout: () => void;
};
`;
    const result = extractFromSource('types.ts', code);

    const typeNode = result.nodes.find((n) => n.kind === 'type_alias');
    expect(typeNode).toMatchObject({
      kind: 'type_alias',
      name: 'AuthContextValue',
      isExported: true,
    });
  });

  it('should extract non-exported type aliases', () => {
    const code = `
type InternalState = {
  loading: boolean;
  error: string | null;
};
`;
    const result = extractFromSource('internal.ts', code);

    const typeNode = result.nodes.find((n) => n.kind === 'type_alias');
    expect(typeNode).toMatchObject({
      kind: 'type_alias',
      name: 'InternalState',
      isExported: false,
    });
  });

  it('should extract multiple type aliases from the same file', () => {
    const code = `
export type UnitSystem = 'metric' | 'imperial';
export type DateFormat = 'ISO' | 'US' | 'EU';
type Internal = string;
`;
    const result = extractFromSource('config.ts', code);

    const typeAliases = result.nodes.filter((n) => n.kind === 'type_alias');
    expect(typeAliases).toHaveLength(3);

    const exported = typeAliases.filter((n) => n.isExported);
    expect(exported).toHaveLength(2);
    expect(exported.map((n) => n.name).sort()).toEqual(['DateFormat', 'UnitSystem']);
  });
});

describe('Exported Variable Extraction', () => {
  it('should extract exported const with call expression (Zustand store)', () => {
    const code = `
export const useUIStore = create<UIState>((set) => ({
  isOpen: false,
  toggle: () => set((s) => ({ isOpen: !s.isOpen })),
}));
`;
    const result = extractFromSource('store.ts', code);

    const varNode = result.nodes.find((n) => n.kind === 'constant' && n.name === 'useUIStore');
    expect(varNode).toBeDefined();
    expect(varNode?.isExported).toBe(true);
  });

  it('should extract exported const with object literal', () => {
    const code = `
export const config = {
  apiUrl: 'https://api.example.com',
  timeout: 5000,
};
`;
    const result = extractFromSource('config.ts', code);

    const varNode = result.nodes.find((n) => n.kind === 'constant' && n.name === 'config');
    expect(varNode).toBeDefined();
    expect(varNode?.isExported).toBe(true);
  });

  it('should extract exported const with array literal', () => {
    const code = `
export const SCREEN_NAMES = ['home', 'settings', 'profile'] as const;
`;
    const result = extractFromSource('constants.ts', code);

    const varNode = result.nodes.find((n) => n.kind === 'constant' && n.name === 'SCREEN_NAMES');
    expect(varNode).toBeDefined();
    expect(varNode?.isExported).toBe(true);
  });

  it('should extract exported const with primitive value', () => {
    const code = `
export const MAX_RETRIES = 3;
export const API_VERSION = "v2";
`;
    const result = extractFromSource('constants.ts', code);

    const variables = result.nodes.filter((n) => n.kind === 'constant');
    expect(variables).toHaveLength(2);
    expect(variables.map((n) => n.name).sort()).toEqual(['API_VERSION', 'MAX_RETRIES']);
  });

  it('should NOT duplicate arrow functions as both function and variable', () => {
    const code = `
export const useAuth = () => {
  return useContext(AuthContext);
};
`;
    const result = extractFromSource('hooks.ts', code);

    // Should be extracted as function (from arrow function handler), NOT as variable
    const funcNodes = result.nodes.filter((n) => n.kind === 'function' && n.name === 'useAuth');
    const varNodes = result.nodes.filter((n) => n.kind === 'variable' && n.name === 'useAuth');
    expect(funcNodes).toHaveLength(1);
    expect(varNodes).toHaveLength(0);
  });

  it('should extract non-exported const as non-exported variable', () => {
    const code = `
const internalConfig = {
  debug: true,
};
`;
    const result = extractFromSource('internal.ts', code);

    // Non-exported const at file level should be extracted as a constant (not exported)
    const varNodes = result.nodes.filter((n) => (n.kind === 'variable' || n.kind === 'constant') && n.name === 'internalConfig');
    expect(varNodes).toHaveLength(1);
    expect(varNodes[0]?.isExported).toBeFalsy();
  });

  it('should extract Zod schema exports', () => {
    const code = `
export const userSchema = z.object({
  id: z.string(),
  name: z.string(),
  email: z.string().email(),
});
`;
    const result = extractFromSource('schemas.ts', code);

    const varNode = result.nodes.find((n) => n.kind === 'constant' && n.name === 'userSchema');
    expect(varNode).toBeDefined();
    expect(varNode?.isExported).toBe(true);
  });

  it('should extract XState machine exports', () => {
    const code = `
export const authMachine = createMachine({
  id: "auth",
  initial: "idle",
  states: {
    idle: {},
    authenticated: {},
  },
});
`;
    const result = extractFromSource('machine.ts', code);

    const varNode = result.nodes.find((n) => n.kind === 'constant' && n.name === 'authMachine');
    expect(varNode).toBeDefined();
    expect(varNode?.isExported).toBe(true);
  });
});

describe('File Node Extraction', () => {
  it('should create a file-kind node for each parsed file', () => {
    const code = `
export function greet(name: string): string {
  return "Hello " + name;
}
`;
    const result = extractFromSource('greeter.ts', code);

    const fileNode = result.nodes.find((n) => n.kind === 'file');
    expect(fileNode).toBeDefined();
    expect(fileNode?.name).toBe('greeter.ts');
    expect(fileNode?.filePath).toBe('greeter.ts');
    expect(fileNode?.language).toBe('typescript');
    expect(fileNode?.startLine).toBe(1);
  });

  it('should create file nodes for Python files', () => {
    const code = `
def main():
    pass
`;
    const result = extractFromSource('main.py', code);

    const fileNode = result.nodes.find((n) => n.kind === 'file');
    expect(fileNode).toBeDefined();
    expect(fileNode?.name).toBe('main.py');
    expect(fileNode?.language).toBe('python');
  });

  it('should create containment edges from file node to top-level declarations', () => {
    const code = `
export function foo() {}
export function bar() {}
`;
    const result = extractFromSource('fns.ts', code);

    const fileNode = result.nodes.find((n) => n.kind === 'file');
    expect(fileNode).toBeDefined();

    // There should be contains edges from the file node to each function
    const containsEdges = result.edges.filter(
      (e) => e.source === fileNode?.id && e.kind === 'contains'
    );
    expect(containsEdges.length).toBeGreaterThanOrEqual(2);
  });
});
