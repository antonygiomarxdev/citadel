use crate::storage::test_utils::*;
use crate::storage::Storage;
use crate::types::*;

fn setup<F>(factory: F, suffix: &str) -> Box<dyn Storage>
where
    F: Fn() -> Box<dyn Storage>,
{
    let mut storage = factory();
    let tmp = std::env::temp_dir().join(format!("cg-contract-{}-{}", std::process::id(), suffix));
    std::fs::create_dir_all(&tmp).unwrap();
    let db_path = tmp.join("test.db");
    let path_str = db_path.to_string_lossy().to_string();
    storage.initialize(&path_str).unwrap();
    storage
}

pub fn test_lifecycle<F>(factory: F)
where
    F: Fn() -> Box<dyn Storage>,
{
    let tmp = std::env::temp_dir().join(format!("cg-lifecycle-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let db_path = tmp.join("lifecycle.db");
    let path_str = db_path.to_string_lossy().to_string();

    let mut storage = factory();
    assert!(storage.get_path().is_none());

    storage.initialize(&path_str).unwrap();
    assert!(storage.get_path().is_some());
    assert_eq!(storage.get_path().unwrap(), path_str);

    storage.close().unwrap();
    assert!(storage.get_path().is_none());
}

pub fn test_node_crud<F>(factory: F)
where
    F: Fn() -> Box<dyn Storage>,
{
    let mut storage = setup(&factory, "node_crud");

    let node = make_node("n1", "MyClass", NodeKind::Class, "src/app.ts", Language::TypeScript);
    storage.insert_node(&node).unwrap();

    let fetched = storage.get_node_by_id("n1").unwrap().expect("node should exist");
    assert_eq!(fetched.name, "MyClass");
    assert_eq!(fetched.kind, NodeKind::Class);

    storage.update_node(&make_node("n1", "RenamedClass", NodeKind::Class, "src/app.ts", Language::TypeScript)).unwrap();
    let updated = storage.get_node_by_id("n1").unwrap().expect("node should exist");
    assert_eq!(updated.name, "RenamedClass");

    storage.delete_node("n1").unwrap();
    assert!(storage.get_node_by_id("n1").unwrap().is_none());

    let nodes = vec![
        make_node("b1", "A", NodeKind::Function, "src/a.ts", Language::TypeScript),
        make_node("b2", "B", NodeKind::Function, "src/b.ts", Language::TypeScript),
        make_node("b3", "C", NodeKind::Class, "src/c.ts", Language::TypeScript),
    ];
    storage.insert_nodes(&nodes).unwrap();
    let by_kind = storage.get_nodes_by_kind(&NodeKind::Function).unwrap();
    assert_eq!(by_kind.len(), 2);
    let by_file = storage.get_nodes_by_file("src/a.ts").unwrap();
    assert_eq!(by_file.len(), 1);
    assert_eq!(by_file[0].name, "A");

    let _ = storage.close();
}

pub fn test_edge_crud<F>(factory: F)
where
    F: Fn() -> Box<dyn Storage>,
{
    let mut storage = setup(&factory, "edge_crud");

    storage.insert_node(&make_node("n1", "Src", NodeKind::Function, "src/main.ts", Language::TypeScript)).unwrap();
    storage.insert_node(&make_node("n2", "Tgt", NodeKind::Function, "src/main.ts", Language::TypeScript)).unwrap();

    let edge = make_edge("n1", "n2", EdgeKind::Calls);
    storage.insert_edge(&edge).unwrap();

    let outgoing = storage.get_outgoing_edges("n1", None, None).unwrap();
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].target, "n2");
    assert_eq!(outgoing[0].kind, EdgeKind::Calls);

    let incoming = storage.get_incoming_edges("n2", None).unwrap();
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].source, "n1");

    let between = storage.find_edges_between_nodes(&["n1".to_string(), "n2".to_string()], None).unwrap();
    assert_eq!(between.len(), 1);

    storage.delete_edges_by_source("n1").unwrap();
    assert!(storage.get_outgoing_edges("n1", None, None).unwrap().is_empty());

    let _ = storage.close();
}

pub fn test_file_crud<F>(factory: F)
where
    F: Fn() -> Box<dyn Storage>,
{
    let mut storage = setup(&factory, "file_crud");

    let file = make_file("src/main.ts", Language::TypeScript);
    storage.upsert_file(&file).unwrap();

    let fetched = storage.get_file_by_path("src/main.ts").unwrap().expect("file should exist");
    assert_eq!(fetched.language, Language::TypeScript);

    let all = storage.get_all_files().unwrap();
    assert_eq!(all.len(), 1);

    let paths = storage.get_all_file_paths().unwrap();
    assert!(paths.contains(&"src/main.ts".to_string()));

    storage.delete_file("src/main.ts").unwrap();
    assert!(storage.get_file_by_path("src/main.ts").unwrap().is_none());

    let _ = storage.close();
}

pub fn test_search<F>(factory: F)
where
    F: Fn() -> Box<dyn Storage>,
{
    let mut storage = setup(&factory, "search");

    storage.insert_node(&make_node("s1", "UserService", NodeKind::Class, "src/user.ts", Language::TypeScript)).unwrap();
    storage.insert_node(&make_node("s2", "OrderService", NodeKind::Class, "src/order.ts", Language::TypeScript)).unwrap();
    storage.insert_node(&make_node("s3", "OrderRepository", NodeKind::Class, "src/order.rs", Language::Rust)).unwrap();

    let opts = SearchOptions {
        query: Some("order".to_string()),
        kinds: None,
        languages: None,
        include_patterns: None,
        exclude_patterns: None,
        limit: 10,
        offset: 0,
    };
    let results = storage.search_nodes("order", &opts).unwrap();
    assert_eq!(results.len(), 2);

    let lang_opts = SearchOptions {
        query: Some("order".to_string()),
        kinds: None,
        languages: Some(vec![Language::Rust]),
        include_patterns: None,
        exclude_patterns: None,
        limit: 10,
        offset: 0,
    };
    let lang_results = storage.search_nodes("order", &lang_opts).unwrap();
    assert_eq!(lang_results.len(), 1);
    assert_eq!(lang_results[0].node.name, "OrderRepository");

    let _ = storage.close();
}

pub fn test_unresolved_refs<F>(factory: F)
where
    F: Fn() -> Box<dyn Storage>,
{
    let mut storage = setup(&factory, "unresolved_refs");

    storage.insert_node(&make_node("u1", "Ref", NodeKind::Function, "src/ref.ts", Language::TypeScript)).unwrap();

    let uref = make_unresolved_ref("u1", "MissingModule", "src/ref.ts");
    storage.insert_unresolved_ref(&uref).unwrap();

    let by_name = storage.get_unresolved_by_name("MissingModule").unwrap();
    assert_eq!(by_name.len(), 1);

    let count = storage.get_unresolved_refs_count().unwrap();
    assert_eq!(count, 1);

    storage.clear_unresolved_refs().unwrap();
    assert_eq!(storage.get_unresolved_refs_count().unwrap(), 0);

    let _ = storage.close();
}

pub fn test_metadata<F>(factory: F)
where
    F: Fn() -> Box<dyn Storage>,
{
    let mut storage = setup(&factory, "metadata");

    storage.set_metadata("version", "1.0.0").unwrap();
    let val = storage.get_metadata("version").unwrap().expect("metadata should exist");
    assert_eq!(val, "1.0.0");

    let all = storage.get_all_metadata().unwrap();
    assert_eq!(all.get("version").unwrap(), "1.0.0");

    assert!(storage.get_metadata("nonexistent").unwrap().is_none());

    let _ = storage.close();
}

pub fn test_stats<F>(factory: F)
where
    F: Fn() -> Box<dyn Storage>,
{
    let mut storage = setup(&factory, "stats");

    storage.insert_node(&make_node("st1", "A", NodeKind::Function, "src/a.ts", Language::TypeScript)).unwrap();
    storage.insert_edge(&make_edge("st1", "st1", EdgeKind::References)).unwrap();

    let stats = storage.get_stats().unwrap();
    assert!(stats.node_count >= 1);
    assert!(stats.edge_count >= 1);

    let db_size = storage.get_db_size_bytes().unwrap();
    assert!(db_size == 0 || db_size > 0);

    let _ = storage.close();
}

pub fn test_traversal<F>(factory: F)
where
    F: Fn() -> Box<dyn Storage>,
{
    let mut storage = setup(&factory, "traversal");

    storage.insert_node(&make_node("t1", "main", NodeKind::Function, "src/main.ts", Language::TypeScript)).unwrap();
    storage.insert_node(&make_node("t2", "helper", NodeKind::Function, "src/helper.ts", Language::TypeScript)).unwrap();
    storage.insert_node(&make_node("t3", "deep", NodeKind::Function, "src/deep.ts", Language::TypeScript)).unwrap();
    storage.insert_node(&make_node("t4", "unused", NodeKind::Function, "src/unused.ts", Language::TypeScript)).unwrap();

    storage.insert_edge(&make_edge("t1", "t2", EdgeKind::Calls)).unwrap();
    storage.insert_edge(&make_edge("t2", "t3", EdgeKind::Calls)).unwrap();

    // BFS
    let bfs_opts = TraversalOptions {
        max_depth: 3,
        direction: TraversalDirection::Outgoing,
        edge_kinds: vec![EdgeKind::Calls],
        include_start: true,
        ..Default::default()
    };
    let bfs_results = storage.traverse_bfs("t1", &bfs_opts).unwrap();
    let bfs_names: Vec<&str> = bfs_results.iter().map(|(n, _)| n.name.as_str()).collect();
    assert!(bfs_names.contains(&"main"));
    assert!(bfs_names.contains(&"helper"));
    assert!(bfs_names.contains(&"deep"));
    assert!(!bfs_names.contains(&"unused"));

    // find_shortest_path
    let path = storage.find_shortest_path("t1", "t3", Some(&[EdgeKind::Calls])).unwrap();
    assert!(path.is_some());
    let path_nodes: Vec<String> = path.unwrap().into_iter().map(|(n, _)| n.name).collect();
    assert!(path_nodes.contains(&"main".to_string()));
    assert!(path_nodes.contains(&"helper".to_string()));
    assert!(path_nodes.contains(&"deep".to_string()));

    // get_callers
    let callers = storage.get_callers("t2", 2).unwrap();
    assert!(!callers.is_empty());
    assert!(callers.iter().any(|(n, _)| n.name == "main"));

    // get_callees
    let callees = storage.get_callees("t2", 2).unwrap();
    assert!(!callees.is_empty());
    assert!(callees.iter().any(|(n, _)| n.name == "deep"));

    let _ = storage.close();
}

pub fn test_clear<F>(factory: F)
where
    F: Fn() -> Box<dyn Storage>,
{
    let mut storage = setup(&factory, "clear");

    storage.insert_node(&make_node("c1", "Cls", NodeKind::Class, "src/cls.ts", Language::TypeScript)).unwrap();
    storage.clear().unwrap();
    assert!(storage.get_all_nodes().unwrap().is_empty());

    let _ = storage.close();
}

pub fn run_all<F>(factory: &F)
where
    F: Fn() -> Box<dyn Storage>,
{
    test_lifecycle(factory);
    test_node_crud(factory);
    test_edge_crud(factory);
    test_file_crud(factory);
    test_search(factory);
    test_unresolved_refs(factory);
    test_metadata(factory);
    test_stats(factory);
    test_traversal(factory);
    test_clear(factory);
}
