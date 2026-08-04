// Tests for Foundry v0.2 JS APIs: localStorage and fetch

use foundry_runtime::js::*;

// ===== localStorage Tests =====

#[test]
fn test_localstorage_set_get() {
    let mut engine = JsEngine::new();
    engine
        .execute(r#"localStorage.setItem("key1", "hello");"#)
        .unwrap();
    engine
        .execute(r#"localStorage.setItem("key2", "world");"#)
        .unwrap();

    // Verify via JS — getItem returns the value
    let result = engine.execute(
        r#"
            var v = localStorage.getItem("key1");
            console.log(v);
        "#,
    );
    assert!(result.is_ok());
}

#[test]
fn test_localstorage_get_missing_returns_null() {
    let mut engine = JsEngine::new();
    let result = engine.execute(
        r#"
        var v = localStorage.getItem("nonexistent");
        if (v !== null) { throw "expected null"; }
    "#,
    );
    assert!(result.is_ok());
}

#[test]
fn test_localstorage_remove_item() {
    let mut engine = JsEngine::new();
    engine
        .execute(r#"localStorage.setItem("key", "val");"#)
        .unwrap();
    engine
        .execute(r#"localStorage.removeItem("key");"#)
        .unwrap();

    let result = engine.execute(
        r#"
        var v = localStorage.getItem("key");
        if (v !== null) { throw "expected null after remove"; }
    "#,
    );
    assert!(result.is_ok());
}

#[test]
fn test_localstorage_clear() {
    let mut engine = JsEngine::new();
    engine
        .execute(r#"localStorage.setItem("a", "1");"#)
        .unwrap();
    engine
        .execute(r#"localStorage.setItem("b", "2");"#)
        .unwrap();
    engine.execute("localStorage.clear();").unwrap();

    let result = engine.execute(
        r#"
        var a = localStorage.getItem("a");
        var b = localStorage.getItem("b");
        if (a !== null || b !== null) { throw "expected null after clear"; }
    "#,
    );
    assert!(result.is_ok());
}

#[test]
fn test_localstorage_overwrite() {
    let mut engine = JsEngine::new();
    engine
        .execute(r#"localStorage.setItem("k", "old");"#)
        .unwrap();
    engine
        .execute(r#"localStorage.setItem("k", "new");"#)
        .unwrap();

    let result = engine.execute(
        r#"
        var v = localStorage.getItem("k");
        if (v !== "new") { throw "expected new, got " + v; }
    "#,
    );
    assert!(result.is_ok());
}

#[test]
fn test_localstorage_persistence_to_file() {
    let dir = std::env::temp_dir().join("foundry_test_storage");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("storage.json");

    // Write with one engine
    {
        let mut engine = JsEngine::with_storage_path(Some(path.clone()));
        engine
            .execute(r#"localStorage.setItem("persist", "yes");"#)
            .unwrap();
    }

    // Read with a new engine — should load from file
    {
        let mut engine = JsEngine::with_storage_path(Some(path.clone()));
        let result = engine.execute(
            r#"
            var v = localStorage.getItem("persist");
            if (v !== "yes") { throw "expected yes, got " + v; }
        "#,
        );
        assert!(result.is_ok());
    }

    let _ = std::fs::remove_dir_all(&dir);
}

// ===== LocalStorage struct tests =====

#[test]
fn test_localstorage_struct_basic() {
    let mut storage = LocalStorage::new(None);
    assert_eq!(storage.length(), 0);

    storage.set_item("a".to_string(), "1".to_string());
    assert_eq!(storage.get_item("a"), Some(&"1".to_string()));
    assert_eq!(storage.length(), 1);

    storage.remove_item("a");
    assert_eq!(storage.get_item("a"), None);
    assert_eq!(storage.length(), 0);
}

#[test]
fn test_localstorage_struct_clear() {
    let mut storage = LocalStorage::new(None);
    storage.set_item("x".to_string(), "1".to_string());
    storage.set_item("y".to_string(), "2".to_string());
    storage.clear();
    assert_eq!(storage.length(), 0);
}

// ===== fetch() Tests =====

#[test]
#[ignore = "requires network access"]
fn test_fetch_returns_object() {
    let mut engine = JsEngine::new();
    let result = engine.execute(
        r#"
        var resp = fetch("https://httpbin.org/get");
        if (typeof resp !== "object") { throw "expected object"; }
        if (typeof resp.status !== "number") { throw "expected status number"; }
        if (typeof resp.text !== "string") { throw "expected text string"; }
        if (resp.ok !== true) { throw "expected ok=true"; }
        if (resp.status !== 200) { throw "expected status 200, got " + resp.status; }
    "#,
    );
    assert!(result.is_ok(), "fetch test failed: {:?}", result);
}

#[test]
fn test_fetch_bad_url() {
    let mut engine = JsEngine::new();
    let result = engine.execute(
        r#"
        var resp = fetch("http://localhost:1");
        if (resp.ok !== false) { throw "expected ok=false for bad URL"; }
    "#,
    );
    assert!(result.is_ok());
}

// ===== DOM Mutation Tests (existing, ensure no regression) =====

#[test]
fn test_dom_mutation_set_text() {
    let mut engine = JsEngine::new();
    engine
        .execute(
            r#"
        var el = document.getElementById("counter");
        el.setTextContent("42");
    "#,
        )
        .unwrap();

    let mutations = engine.take_mutations();
    assert_eq!(mutations.len(), 1);
    match &mutations[0] {
        DomMutation::SetTextContent(id, text) => {
            assert_eq!(id, "counter");
            assert_eq!(text, "42");
        }
        _ => panic!("expected SetTextContent"),
    }
}

#[test]
fn test_dom_mutation_set_style() {
    let mut engine = JsEngine::new();
    engine
        .execute(
            r#"
        var el = document.getElementById("box");
        el.setStyle("background-color", "red");
    "#,
        )
        .unwrap();

    let mutations = engine.take_mutations();
    assert_eq!(mutations.len(), 1);
    match &mutations[0] {
        DomMutation::SetStyle(id, prop, val) => {
            assert_eq!(id, "box");
            assert_eq!(prop, "background-color");
            assert_eq!(val, "red");
        }
        _ => panic!("expected SetStyle"),
    }
}

#[test]
fn test_dom_mutation_classes() {
    let mut engine = JsEngine::new();
    engine
        .execute(
            r#"
        var el = document.getElementById("item");
        el.addClass("active");
        el.removeClass("hidden");
    "#,
        )
        .unwrap();

    let mutations = engine.take_mutations();
    assert_eq!(mutations.len(), 2);
    match &mutations[0] {
        DomMutation::AddClass(id, class) => {
            assert_eq!(id, "item");
            assert_eq!(class, "active");
        }
        _ => panic!("expected AddClass"),
    }
    match &mutations[1] {
        DomMutation::RemoveClass(id, class) => {
            assert_eq!(id, "item");
            assert_eq!(class, "hidden");
        }
        _ => panic!("expected RemoveClass"),
    }
}
