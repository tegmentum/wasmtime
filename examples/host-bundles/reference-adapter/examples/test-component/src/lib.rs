//! Test component that uses the key-value store host interface

wit_bindgen::generate!({
    world: "test-runner",
    path: "../wit",
    additional_derives: [serde::Serialize, serde::Deserialize],
});

struct Component;

impl Guest for Component {
    fn run_tests() -> String {
        let mut results = Vec::new();

        // Test 1: Set and Get
        results.push("Test 1: Set and Get");
        match keyvalue::store::store::set("test-key".to_string(), "test-value".to_string()) {
            Ok(_) => {
                match keyvalue::store::store::get("test-key".to_string()) {
                    Some(value) if value == "test-value" => {
                        results.push("  ✓ Set and get successful");
                    }
                    _ => results.push("  ✗ Get returned unexpected value"),
                }
            }
            Err(e) => results.push(&format!("  ✗ Set failed: {}", e)),
        }

        // Test 2: Exists
        results.push("\nTest 2: Exists");
        if keyvalue::store::store::exists("test-key".to_string()) {
            results.push("  ✓ Exists check passed");
        } else {
            results.push("  ✗ Exists check failed");
        }

        // Test 3: List Keys
        results.push("\nTest 3: List Keys");
        let keys = keyvalue::store::store::list_keys();
        if keys.contains(&"test-key".to_string()) {
            results.push(&format!("  ✓ List keys returned {} key(s)", keys.len()));
        } else {
            results.push("  ✗ List keys did not contain expected key");
        }

        // Test 4: Delete
        results.push("\nTest 4: Delete");
        match keyvalue::store::store::delete("test-key".to_string()) {
            Ok(_) => {
                if !keyvalue::store::store::exists("test-key".to_string()) {
                    results.push("  ✓ Delete successful");
                } else {
                    results.push("  ✗ Key still exists after delete");
                }
            }
            Err(e) => results.push(&format!("  ✗ Delete failed: {}", e)),
        }

        // Test 5: Duplicate key error
        results.push("\nTest 5: Duplicate Key");
        let _ = keyvalue::store::store::set("dup".to_string(), "val1".to_string());
        match keyvalue::store::store::set("dup".to_string(), "val2".to_string()) {
            Err(_) => results.push("  ✓ Duplicate key correctly rejected"),
            Ok(_) => results.push("  ✗ Duplicate key was accepted"),
        }

        // Test 6: Clear
        results.push("\nTest 6: Clear");
        keyvalue::store::store::clear();
        if keyvalue::store::store::list_keys().is_empty() {
            results.push("  ✓ Clear successful");
        } else {
            results.push("  ✗ Store not empty after clear");
        }

        results.join("\n")
    }
}

export!(Component);
