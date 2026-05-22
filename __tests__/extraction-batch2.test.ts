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
  it('should extract function definitions', () => {
    const code = `
def calculate_total(items: list, tax_rate: float) -> float:
    """Calculate total with tax."""
    subtotal = sum(item.price for item in items)
    return subtotal * (1 + tax_rate)
`;
    const result = extractFromSource('calc.py', code);

    const fileNode = result.nodes.find((n) => n.kind === 'file');
    expect(fileNode).toBeDefined();

    const funcNode = result.nodes.find((n) => n.kind === 'function');
    expect(funcNode).toMatchObject({
      kind: 'function',
      name: 'calculate_total',
      language: 'python',
    });
  });

  it('should extract class definitions', () => {
    const code = `
class UserService:
    """Service for managing users."""

    def __init__(self, db):
        self.db = db

    def get_user(self, user_id: str) -> User:
        return self.db.find_user(user_id)
`;
    const result = extractFromSource('service.py', code);

    const classNode = result.nodes.find((n) => n.kind === 'class');
    expect(classNode).toBeDefined();
    expect(classNode?.name).toBe('UserService');
  });
});

describe('Go Extraction', () => {
  it('should extract function declarations', () => {
    const code = `
package main

func ProcessOrder(order Order) (Receipt, error) {
    // Process the order
    return Receipt{}, nil
}
`;
    const result = extractFromSource('main.go', code);

    const funcNode = result.nodes.find((n) => n.kind === 'function');
    expect(funcNode).toBeDefined();
    expect(funcNode?.name).toBe('ProcessOrder');
  });

  it('should extract method declarations', () => {
    const code = `
package main

type Service struct {
    db *Database
}

func (s *Service) GetUser(id string) (*User, error) {
    return s.db.FindUser(id)
}
`;
    const result = extractFromSource('service.go', code);

    const methodNode = result.nodes.find((n) => n.kind === 'method');
    expect(methodNode).toBeDefined();
    expect(methodNode?.name).toBe('GetUser');
  });
});

describe('Rust Extraction', () => {
  it('should extract function declarations', () => {
    const code = `
pub fn process_data(input: &str) -> Result<Output, Error> {
    // Process data
    Ok(Output::new())
}
`;
    const result = extractFromSource('lib.rs', code);

    const funcNode = result.nodes.find((n) => n.kind === 'function');
    expect(funcNode).toBeDefined();
    expect(funcNode?.name).toBe('process_data');
    expect(funcNode?.visibility).toBe('public');
  });

  it('should extract struct declarations', () => {
    const code = `
pub struct User {
    pub id: String,
    pub name: String,
    email: String,
}
`;
    const result = extractFromSource('models.rs', code);

    const structNode = result.nodes.find((n) => n.kind === 'struct');
    expect(structNode).toBeDefined();
    expect(structNode?.name).toBe('User');
  });

  it('should extract trait declarations', () => {
    const code = `
pub trait Repository {
    fn find(&self, id: &str) -> Option<Entity>;
    fn save(&mut self, entity: Entity) -> Result<(), Error>;
}
`;
    const result = extractFromSource('traits.rs', code);

    const traitNode = result.nodes.find((n) => n.kind === 'trait');
    expect(traitNode).toBeDefined();
    expect(traitNode?.name).toBe('Repository');
  });

  it('should extract impl Trait for Type as implements edges', () => {
    const code = `
pub struct MyCache {}

pub trait Cache {
    fn get(&self, key: &str) -> Option<String>;
}

impl Cache for MyCache {
    fn get(&self, key: &str) -> Option<String> {
        None
    }
}
`;
    const result = extractFromSource('cache.rs', code);

    // Should have an unresolved reference for implements
    const implRef = result.unresolvedReferences.find(
      (r) => r.referenceKind === 'implements' && r.referenceName === 'Cache'
    );
    expect(implRef).toBeDefined();

    // The struct MyCache should be the source
    const myCacheNode = result.nodes.find((n) => n.name === 'MyCache' && n.kind === 'struct');
    expect(myCacheNode).toBeDefined();
    expect(implRef?.fromNodeId).toBe(myCacheNode?.id);
  });

  it('should extract trait supertraits as extends references', () => {
    const code = `
pub trait Display {}

pub trait Error: Display {
    fn description(&self) -> &str;
}
`;
    const result = extractFromSource('error.rs', code);

    const extendsRef = result.unresolvedReferences.find(
      (r) => r.referenceKind === 'extends' && r.referenceName === 'Display'
    );
    expect(extendsRef).toBeDefined();

    const errorTrait = result.nodes.find((n) => n.name === 'Error' && n.kind === 'trait');
    expect(errorTrait).toBeDefined();
    expect(extendsRef?.fromNodeId).toBe(errorTrait?.id);
  });

  it('should not create implements edges for plain impl blocks', () => {
    const code = `
pub struct Counter {
    count: u32,
}

impl Counter {
    pub fn new() -> Counter {
        Counter { count: 0 }
    }
    pub fn increment(&mut self) {
        self.count += 1;
    }
}
`;
    const result = extractFromSource('counter.rs', code);

    // Should have no implements references (no trait involved)
    const implRefs = result.unresolvedReferences.filter(
      (r) => r.referenceKind === 'implements'
    );
    expect(implRefs).toHaveLength(0);
  });
});

describe('Java Extraction', () => {
  it('should extract class declarations', () => {
    const code = `
public class UserService {
    private final UserRepository repository;

    public UserService(UserRepository repository) {
        this.repository = repository;
    }

    public User getUser(String id) {
        return repository.findById(id);
    }
}
`;
    const result = extractFromSource('UserService.java', code);

    const classNode = result.nodes.find((n) => n.kind === 'class');
    expect(classNode).toBeDefined();
    expect(classNode?.name).toBe('UserService');
    expect(classNode?.visibility).toBe('public');
  });

  it('should extract method declarations', () => {
    const code = `
public class Calculator {
    public static int add(int a, int b) {
        return a + b;
    }
}
`;
    const result = extractFromSource('Calculator.java', code);

    const methodNode = result.nodes.find((n) => n.kind === 'method' && n.name === 'add');
    expect(methodNode).toBeDefined();
    expect(methodNode?.isStatic).toBe(true);
  });
});

describe('C# Extraction', () => {
  it('should extract class declarations', () => {
    const code = `
public class OrderService
{
    private readonly IOrderRepository _repository;

    public OrderService(IOrderRepository repository)
    {
        _repository = repository;
    }

    public async Task<Order> GetOrderAsync(string id)
    {
        return await _repository.FindByIdAsync(id);
    }
}
`;
    const result = extractFromSource('OrderService.cs', code);

    const classNode = result.nodes.find((n) => n.kind === 'class');
    expect(classNode).toBeDefined();
    expect(classNode?.name).toBe('OrderService');
    expect(classNode?.visibility).toBe('public');
  });
});

describe('PHP Extraction', () => {
  it('should extract class declarations', () => {
    const code = `<?php

class UserController
{
    private UserService $userService;

    public function __construct(UserService $userService)
    {
        $this->userService = $userService;
    }

    public function show(string $id): User
    {
        return $this->userService->find($id);
    }
}
`;
    const result = extractFromSource('UserController.php', code);

    const classNode = result.nodes.find((n) => n.kind === 'class');
    expect(classNode).toBeDefined();
    expect(classNode?.name).toBe('UserController');
  });

  it('should extract class inheritance (extends) and interface implementation', () => {
    const code = `<?php

class ChildController extends BaseController implements Serializable, JsonSerializable
{
    public function serialize(): string
    {
        return json_encode($this);
    }
}
`;
    const result = extractFromSource('ChildController.php', code);

    const classNode = result.nodes.find((n) => n.kind === 'class');
    expect(classNode).toBeDefined();
    expect(classNode?.name).toBe('ChildController');

    const extendsRef = result.unresolvedReferences.find(
      (r) => r.referenceKind === 'extends'
    );
    expect(extendsRef).toBeDefined();
    expect(extendsRef?.referenceName).toBe('BaseController');

    const implementsRefs = result.unresolvedReferences.filter(
      (r) => r.referenceKind === 'implements'
    );
    expect(implementsRefs.length).toBe(2);
    expect(implementsRefs.map((r) => r.referenceName)).toContain('Serializable');
    expect(implementsRefs.map((r) => r.referenceName)).toContain('JsonSerializable');
  });
});

describe('Swift Extraction', () => {
  it('should extract class declarations', () => {
    const code = `
public class NetworkManager {
    private let session: URLSession

    public init(session: URLSession = .shared) {
        self.session = session
    }

    public func fetchData(from url: URL) async throws -> Data {
        let (data, _) = try await session.data(from: url)
        return data
    }
}
`;
    const result = extractFromSource('NetworkManager.swift', code);

    const classNode = result.nodes.find((n) => n.kind === 'class');
    expect(classNode).toBeDefined();
    expect(classNode?.name).toBe('NetworkManager');
  });

  it('should extract function declarations', () => {
    const code = `
func calculateSum(_ numbers: [Int]) -> Int {
    return numbers.reduce(0, +)
}

public func formatCurrency(amount: Double) -> String {
    return String(format: "$%.2f", amount)
}
`;
    const result = extractFromSource('utils.swift', code);

    const functions = result.nodes.filter((n) => n.kind === 'function');
    expect(functions.length).toBeGreaterThanOrEqual(1);
  });

  it('should extract struct declarations', () => {
    const code = `
public struct User {
    let id: UUID
    var name: String
    var email: String

    func displayName() -> String {
        return name
    }
}
`;
    const result = extractFromSource('User.swift', code);

    const structNode = result.nodes.find((n) => n.kind === 'struct');
    expect(structNode).toBeDefined();
    expect(structNode?.name).toBe('User');
  });

  it('should extract protocol declarations', () => {
    const code = `
public protocol Repository {
    associatedtype Entity

    func find(id: String) async throws -> Entity?
    func save(_ entity: Entity) async throws
}
`;
    const result = extractFromSource('Repository.swift', code);

    const protocolNode = result.nodes.find((n) => n.kind === 'interface');
    expect(protocolNode).toBeDefined();
    expect(protocolNode?.name).toBe('Repository');
  });

  it('should extract class inheritance and protocol conformance', () => {
    const code = `
class DataRequest: Request {
    func validate() {}
}

class UploadRequest: DataRequest, Sendable {
    func upload() {}
}

enum AFError: Error {
    case invalidURL
}

struct HTTPMethod: RawRepresentable {
    let rawValue: String
}

protocol UploadConvertible: URLRequestConvertible {
    func asURLRequest() throws -> URLRequest
}
`;
    const result = extractFromSource('Inheritance.swift', code);

    const extendsRefs = result.unresolvedReferences.filter(
      (r) => r.referenceKind === 'extends'
    );

    // DataRequest extends Request
    expect(extendsRefs.find((r) => r.referenceName === 'Request')).toBeDefined();
    // UploadRequest extends DataRequest and Sendable
    expect(extendsRefs.find((r) => r.referenceName === 'DataRequest')).toBeDefined();
    expect(extendsRefs.find((r) => r.referenceName === 'Sendable')).toBeDefined();
    // AFError extends Error
    expect(extendsRefs.find((r) => r.referenceName === 'Error')).toBeDefined();
    // HTTPMethod extends RawRepresentable
    expect(extendsRefs.find((r) => r.referenceName === 'RawRepresentable')).toBeDefined();
    // UploadConvertible extends URLRequestConvertible
    expect(extendsRefs.find((r) => r.referenceName === 'URLRequestConvertible')).toBeDefined();
  });
});

describe('Kotlin Extraction', () => {
  it('should extract class declarations', () => {
    const code = `
class UserRepository(private val database: Database) {
    fun findById(id: String): User? {
        return database.query("SELECT * FROM users WHERE id = ?", id)
    }

    suspend fun save(user: User) {
        database.insert(user)
    }
}
`;
    const result = extractFromSource('UserRepository.kt', code);

    const classNode = result.nodes.find((n) => n.kind === 'class');
    expect(classNode).toBeDefined();
    expect(classNode?.name).toBe('UserRepository');
  });

  it('should extract function declarations', () => {
    const code = `
fun calculateTotal(items: List<Item>): Double {
    return items.sumOf { it.price }
}

suspend fun fetchUserData(userId: String): User {
    return api.getUser(userId)
}
`;
    const result = extractFromSource('utils.kt', code);

    const functions = result.nodes.filter((n) => n.kind === 'function');
    expect(functions.length).toBeGreaterThanOrEqual(1);
  });

  it('should detect suspend functions as async', () => {
    const code = `
suspend fun loadData(): List<String> {
    delay(1000)
    return listOf("a", "b", "c")
}
`;
    const result = extractFromSource('loader.kt', code);

    const funcNode = result.nodes.find((n) => n.kind === 'function');
    expect(funcNode).toBeDefined();
    expect(funcNode?.isAsync).toBe(true);
  });

  it('should extract fun interface declarations', () => {
    const code = `
fun interface OnObjectRetainedListener {
  fun onObjectRetained()
}
`;
    const result = extractFromSource('listener.kt', code);

    const ifaceNode = result.nodes.find((n) => n.kind === 'interface');
    expect(ifaceNode).toBeDefined();
    expect(ifaceNode?.name).toBe('OnObjectRetainedListener');

    const methodNode = result.nodes.find((n) => n.kind === 'method');
    expect(methodNode).toBeDefined();
    expect(methodNode?.name).toBe('onObjectRetained');
    expect(methodNode?.qualifiedName).toBe('OnObjectRetainedListener::onObjectRetained');
  });

  it('should extract complex fun interface with nested classes', () => {
    const code = `
fun interface EventListener {
  fun onEvent(event: Event)

  sealed class Event {
    class DumpingHeap : Event()
  }
}
`;
    const result = extractFromSource('events.kt', code);

    const ifaceNode = result.nodes.find((n) => n.kind === 'interface');
    expect(ifaceNode).toBeDefined();
    expect(ifaceNode?.name).toBe('EventListener');

    // Nested sealed class should still be extracted (as sibling due to grammar limitations)
    const eventClass = result.nodes.find((n) => n.kind === 'class' && n.name === 'Event');
    expect(eventClass).toBeDefined();

    const dumpingHeap = result.nodes.find((n) => n.kind === 'class' && n.name === 'DumpingHeap');
    expect(dumpingHeap).toBeDefined();
  });

  it('should not affect regular function declarations', () => {
    const code = `
fun interface MyCallback {
  fun invoke(value: Int)
}

fun regularFunction(): String {
  return "hello"
}
`;
    const result = extractFromSource('mixed.kt', code);

    const ifaceNode = result.nodes.find((n) => n.kind === 'interface');
    expect(ifaceNode).toBeDefined();
    expect(ifaceNode?.name).toBe('MyCallback');

    const funcNode = result.nodes.find((n) => n.kind === 'function');
    expect(funcNode).toBeDefined();
    expect(funcNode?.name).toBe('regularFunction');
  });

  it('should extract fun interface with annotation on method (Pattern 2b)', () => {
    // When the SAM method has annotations like @Throws, tree-sitter produces a different
    // misparse: function_declaration > ERROR("interface Name {") instead of
    // function_declaration > user_type("interface"). This is the OkHttp Interceptor pattern.
    const code = `
import java.io.IOException

fun interface Interceptor {
  @Throws(IOException::class)
  fun intercept(chain: Chain): Response
}
`;
    const result = extractFromSource('interceptor.kt', code);

    const ifaceNode = result.nodes.find((n) => n.kind === 'interface');
    expect(ifaceNode).toBeDefined();
    expect(ifaceNode?.name).toBe('Interceptor');
  });

  it('should extract methods from interface with nested fun interface', () => {
    // When an interface contains a nested `fun interface`, tree-sitter misparsed
    // the parent body as ERROR. Methods inside should still be extracted.
    const code = `
interface WebSocket {
  fun request(): Request
  fun send(text: String): Boolean
  fun cancel()
  fun interface Factory {
    fun newWebSocket(request: Request): WebSocket
  }
}
`;
    const result = extractFromSource('websocket.kt', code);

    const wsIface = result.nodes.find((n) => n.kind === 'interface' && n.name === 'WebSocket');
    expect(wsIface).toBeDefined();

    const methods = result.nodes.filter((n) => n.kind === 'method' && n.qualifiedName?.startsWith('WebSocket::'));
    const methodNames = methods.map((m) => m.name);
    expect(methodNames).toContain('request');
    expect(methodNames).toContain('send');
    expect(methodNames).toContain('cancel');
  });
});

describe('Dart Extraction', () => {
  it('should extract class declarations', () => {
    const code = `
class UserService {
  final Database _db;

  Future<User> findById(String id) async {
    return await _db.query(id);
  }

  void _privateMethod() {}
}
`;
    const result = extractFromSource('service.dart', code);

    const classNode = result.nodes.find((n) => n.kind === 'class');
    expect(classNode).toBeDefined();
    expect(classNode?.name).toBe('UserService');
    expect(classNode?.visibility).toBe('public');

    const methodNodes = result.nodes.filter((n) => n.kind === 'method');
    expect(methodNodes.length).toBeGreaterThanOrEqual(2);

    const findById = methodNodes.find((m) => m.name === 'findById');
    expect(findById).toBeDefined();
    expect(findById?.isAsync).toBe(true);

    const privateMethod = methodNodes.find((m) => m.name === '_privateMethod');
    expect(privateMethod).toBeDefined();
    expect(privateMethod?.visibility).toBe('private');
  });

  it('should extract top-level function declarations', () => {
    const code = `
void topLevelFunction(String name) {
  print(name);
}
`;
    const result = extractFromSource('utils.dart', code);

    const funcNode = result.nodes.find((n) => n.kind === 'function');
    expect(funcNode).toBeDefined();
    expect(funcNode?.name).toBe('topLevelFunction');
    expect(funcNode?.language).toBe('dart');
  });

  it('should extract enum declarations', () => {
    const code = `
enum Status { active, inactive, pending }
`;
    const result = extractFromSource('models.dart', code);

    const enumNode = result.nodes.find((n) => n.kind === 'enum');
    expect(enumNode).toBeDefined();
    expect(enumNode?.name).toBe('Status');
  });

  it('should extract mixin declarations', () => {
    const code = `
mixin LoggerMixin {
  void log(String message) {}
}
`;
    const result = extractFromSource('mixins.dart', code);

    const classNode = result.nodes.find((n) => n.kind === 'class');
    expect(classNode).toBeDefined();
    expect(classNode?.name).toBe('LoggerMixin');

    const methodNode = result.nodes.find((n) => n.kind === 'method');
    expect(methodNode).toBeDefined();
    expect(methodNode?.name).toBe('log');
  });

  it('should extract extension declarations', () => {
    const code = `
extension StringExt on String {
  bool get isBlank => trim().isEmpty;
}
`;
    const result = extractFromSource('extensions.dart', code);

    const classNode = result.nodes.find((n) => n.kind === 'class');
    expect(classNode).toBeDefined();
    expect(classNode?.name).toBe('StringExt');
  });

  it('should detect static methods', () => {
    const code = `
class Utils {
  static void doWork() {}
}
`;
    const result = extractFromSource('utils.dart', code);

    const methodNode = result.nodes.find((n) => n.kind === 'method');
    expect(methodNode).toBeDefined();
    expect(methodNode?.name).toBe('doWork');
    expect(methodNode?.isStatic).toBe(true);
  });

  it('should detect async functions', () => {
    const code = `
Future<String> fetchData() async {
  return await http.get('/data');
}
`;
    const result = extractFromSource('api.dart', code);

    const funcNode = result.nodes.find((n) => n.kind === 'function');
    expect(funcNode).toBeDefined();
    expect(funcNode?.name).toBe('fetchData');
    expect(funcNode?.isAsync).toBe(true);
  });

  it('should detect private visibility via underscore convention', () => {
    const code = `
void _privateHelper() {}

void publicFunction() {}
`;
    const result = extractFromSource('helpers.dart', code);

    const functions = result.nodes.filter((n) => n.kind === 'function');
    const privateFunc = functions.find((f) => f.name === '_privateHelper');
    const publicFunc = functions.find((f) => f.name === 'publicFunction');

    expect(privateFunc?.visibility).toBe('private');
    expect(publicFunc?.visibility).toBe('public');
  });
});
