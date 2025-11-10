//! Reference host adapter for key-value store
//!
//! This is a complete example of how to implement a host adapter
//! following the pattern from webassembly-component-orchestration.

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;
use anyhow::Result;
use wasmtime::component::*;

// Generate bindings from WIT
// The host provides (imports to the component) the keyvalue:store/store interface
wit_bindgen::generate!({
    world: "keyvalue-host",
    path: "../wit/keyvalue.wit",
});

/// In-memory key-value store implementation
#[derive(Clone)]
pub struct KeyValueStoreImpl {
    data: Arc<Mutex<HashMap<String, String>>>,
}

impl KeyValueStoreImpl {
    /// Create a new key-value store
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for KeyValueStoreImpl {
    fn default() -> Self {
        Self::new()
    }
}

// Implement the host trait for providing the interface to components
impl keyvalue::store::store::Host for KeyValueStoreImpl {
    fn set(&mut self, key: String, value: String) -> Result<Result<(), String>> {
        let mut data = self.data.lock();
        if data.contains_key(&key) {
            return Ok(Err(format!("Key '{}' already exists", key)));
        }
        data.insert(key, value);
        Ok(Ok(()))
    }

    fn get(&mut self, key: String) -> Result<Option<String>> {
        let data = self.data.lock();
        Ok(data.get(&key).cloned())
    }

    fn delete(&mut self, key: String) -> Result<Result<(), String>> {
        let mut data = self.data.lock();
        if data.remove(&key).is_none() {
            return Ok(Err(format!("Key '{}' not found", key)));
        }
        Ok(Ok(()))
    }

    fn list_keys(&mut self) -> Result<Vec<String>> {
        let data = self.data.lock();
        Ok(data.keys().cloned().collect())
    }

    fn exists(&mut self, key: String) -> Result<bool> {
        let data = self.data.lock();
        Ok(data.contains_key(&key))
    }

    fn clear(&mut self) -> Result<()> {
        let mut data = self.data.lock();
        data.clear();
        Ok(())
    }
}

/// Add the key-value store to a component linker
///
/// This is the main entry point for integrating this adapter with wasmtime.
///
/// # Example
///
/// ```rust,ignore
/// use wasmtime::component::Linker;
/// use keyvalue_host_adapter::add_to_linker;
///
/// let mut linker = Linker::new(&engine);
/// add_to_linker(&mut linker, |state: &mut MyState| &mut state.keyvalue)?;
/// ```
pub fn add_to_linker<T>(
    linker: &mut Linker<T>,
    f: impl Fn(&mut T) -> &mut KeyValueStoreImpl + Send + Sync + Copy + 'static,
) -> Result<()> {
    keyvalue::store::store::add_to_linker(linker, f)?;
    Ok(())
}

/// Convenience function to add a default key-value store to the linker
pub fn add_to_linker_with_default<T>(
    linker: &mut Linker<T>,
) -> Result<KeyValueStoreImpl>
where
    T: Send,
{
    let store_impl = KeyValueStoreImpl::new();
    Ok(store_impl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut store = KeyValueStoreImpl::new();

        // Test set and get
        assert!(store.set("foo".to_string(), "bar".to_string()).unwrap().is_ok());
        assert_eq!(store.get("foo".to_string()).unwrap(), Some("bar".to_string()));

        // Test exists
        assert!(store.exists("foo".to_string()).unwrap());
        assert!(!store.exists("baz".to_string()).unwrap());

        // Test list
        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 1);
        assert!(keys.contains(&"foo".to_string()));

        // Test delete
        assert!(store.delete("foo".to_string()).unwrap().is_ok());
        assert_eq!(store.get("foo".to_string()).unwrap(), None);

        // Test clear
        store.set("a".to_string(), "1".to_string()).unwrap().unwrap();
        store.set("b".to_string(), "2".to_string()).unwrap().unwrap();
        store.clear().unwrap();
        assert_eq!(store.list_keys().unwrap().len(), 0);
    }

    #[test]
    fn test_duplicate_key() {
        let mut store = KeyValueStoreImpl::new();
        assert!(store.set("dup".to_string(), "val1".to_string()).unwrap().is_ok());
        assert!(store.set("dup".to_string(), "val2".to_string()).unwrap().is_err());
    }

    #[test]
    fn test_delete_nonexistent() {
        let mut store = KeyValueStoreImpl::new();
        assert!(store.delete("nonexistent".to_string()).unwrap().is_err());
    }
}
