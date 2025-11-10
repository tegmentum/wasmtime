# Key-Value Store Host Adapter

This is a **reference implementation** of a host adapter for wasmtime's component model, demonstrating how to create host implementations that can be dynamically loaded.

## Overview

This adapter provides a simple in-memory key-value store interface to WebAssembly components. It follows the pattern established in the `webassembly-component-orchestration` project's `pkcs11-host-adapter`.

## Project Structure

```
reference-adapter/
├── Cargo.toml                    # Adapter crate configuration
├── src/
│   └── lib.rs                    # Host adapter implementation
├── wit/
│   └── keyvalue.wit              # WIT interface definition
├── examples/
│   ├── standalone-runner.rs      # Standalone integration example
│   └── test-component/          # Test component that uses the adapter
│       ├── Cargo.toml
│       ├── wit/world.wit
│       └── src/lib.rs
└── README.md                     # This file
```

## WIT Interface

The adapter provides the following interface:

```wit
package keyvalue:store@0.1.0;

interface store {
    set: func(key: string, value: string) -> result<_, string>;
    get: func(key: string) -> option<string>;
    delete: func(key: string) -> result<_, string>;
    list-keys: func() -> list<string>;
    exists: func(key: string) -> bool;
    clear: func();
}
```

## Building

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add wasm32-wasip2 target
rustup target add wasm32-wasip2

# Install wasm-tools (for component manipulation)
cargo install wasm-tools
```

### Build the Adapter

```bash
cd examples/host-bundles/reference-adapter

# Build the adapter library
cargo build --release

# Build the standalone runner example
cargo build --example standalone-runner --release
```

### Build the Test Component

```bash
cd examples/test-component

# Build as a component
cargo build --target wasm32-wasip2 --release

# The component will be at:
# target/wasm32-wasip2/release/keyvalue_test_component.wasm
```

## Running

### Standalone Runner

The standalone runner demonstrates how to integrate the adapter:

```bash
cargo run --example standalone-runner

# Output:
# Wasmtime Host Adapter Example
# ==============================
#
# ✓ Created engine with component model support
# ✓ Added WASI to linker
# ✓ Added key-value store adapter to linker
#
# 📝 Host adapter successfully configured!
```

### With a Component (Manual Integration)

```rust
use wasmtime::*;
use wasmtime::component::*;
use keyvalue_host_adapter::{KeyValueStoreImpl, add_to_linker};

// Define your host state
struct HostState {
    wasi: wasmtime_wasi::WasiCtx,
    table: wasmtime_wasi::ResourceTable,
    keyvalue: KeyValueStoreImpl,
}

// Setup engine and linker
let engine = Engine::default();
let mut linker = Linker::<HostState>::new(&engine);

// Add WASI
wasmtime_wasi::add_to_linker_sync(&mut linker)?;

// Add key-value adapter
add_to_linker(&mut linker, |state: &mut HostState| &mut state.keyvalue)?;

// Load and run your component
let component = Component::from_file(&engine, "component.wasm")?;
let mut store = Store::new(&engine, HostState { /* ... */ });
let instance = linker.instantiate(&mut store, &component)?;
```

## Integration with Wasmtime CLI

To integrate this adapter into the wasmtime CLI's host bundle system:

### Option 1: Static Registration (Recommended)

Modify `src/commands/run.rs` to recognize "keyvalue" bundles:

```rust
// In new_store_and_linker()
#[cfg(feature = "component-model")]
if !self.host_bundles.is_empty() || self.host_config.is_some() {
    let host_bundles = self.load_host_bundles()?;

    if let CliLinker::Component(component_linker) = &mut linker {
        for bundle in host_bundles.bundles() {
            match bundle.name() {
                "keyvalue" => {
                    // Add key-value adapter
                    keyvalue_host_adapter::add_to_linker(
                        component_linker,
                        |state: &mut Host| &mut state.keyvalue
                    )?;
                }
                _ => {
                    // Fall back to dynamic loading
                    eprintln!("Warning: Unknown bundle '{}'", bundle.name());
                }
            }
        }
    }
}
```

### Option 2: Build as Shared Library

Build the adapter as a shared library that can be loaded dynamically:

```bash
# Build as shared library
cargo build --release

# The output will be:
# target/release/libkeyvalue_host_adapter.{so,dylib,dll}
```

Then create a host bundle:

```
keyvalue_host/
  host.toml
  wit/
    keyvalue.wit
  lib/
    libkeyvalue_host.so
```

`host.toml`:
```toml
[host]
name = "keyvalue"
lib = "lib/libkeyvalue_host.so"
wit = "wit/keyvalue.wit"
```

Use with CLI:
```bash
wasmtime component run \
  --host-bundle ./keyvalue_host \
  component.wasm
```

## Testing

Run the adapter tests:

```bash
cargo test
```

Expected output:
```
running 3 tests
test tests::test_basic_operations ... ok
test tests::test_delete_nonexistent ... ok
test tests::test_duplicate_key ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured
```

## Key Implementation Details

### 1. WIT Binding Generation

```rust
wit_bindgen::generate!({
    world: "keyvalue-host",
    path: "../wit/keyvalue.wit",
});
```

This generates the `keyvalue::store::store::Host` trait that we implement.

### 2. Host Trait Implementation

```rust
impl keyvalue::store::store::Host for KeyValueStoreImpl {
    fn set(&mut self, key: String, value: String) -> Result<Result<(), String>> {
        // Implementation
    }
    // ... other methods
}
```

Note the double `Result`: The outer `Result` is for wasmtime errors, the inner `Result<(), String>` maps to the WIT `result<_, string>`.

### 3. Linker Integration

```rust
pub fn add_to_linker<T>(
    linker: &mut Linker<T>,
    f: impl Fn(&mut T) -> &mut KeyValueStoreImpl + Send + Sync + Copy + 'static,
) -> Result<()> {
    keyvalue::store::store::add_to_linker(linker, f)?;
    Ok(())
}
```

The function `f` is a closure that extracts the `KeyValueStoreImpl` from your host state.

## Comparison with Dynamic Loading

This static adapter approach:

**Pros:**
- ✅ Type-safe at compile time
- ✅ No runtime WIT parsing overhead
- ✅ Better error messages
- ✅ Easier to debug

**Cons:**
- ❌ Requires rebuild for new interfaces
- ❌ Can't load arbitrary bundles

For dynamic loading, see the `INTEGRATION_GUIDE.md` in the parent directory.

## Creating Your Own Adapter

To create a new host adapter:

1. **Define WIT interface:**
   ```wit
   package my:interface@0.1.0;

   interface my-api {
       do-something: func(input: string) -> result<string, string>;
   }

   world my-host {
       export my-api;
   }
   ```

2. **Create adapter crate:**
   ```toml
   [dependencies]
   wasmtime = { version = "27.0", features = ["component-model"] }
   wit-bindgen = "0.38"
   anyhow = "1"
   ```

3. **Implement Host trait:**
   ```rust
   wit_bindgen::generate!({
       world: "my-host",
       path: "../wit/my-interface.wit",
   });

   pub struct MyHostImpl {
       // Your state
   }

   impl my::interface::my_api::Host for MyHostImpl {
       fn do_something(&mut self, input: String) -> Result<Result<String, String>> {
           // Your implementation
       }
   }

   pub fn add_to_linker<T>(
       linker: &mut wasmtime::component::Linker<T>,
       f: impl Fn(&mut T) -> &mut MyHostImpl + Send + Sync + Copy + 'static,
   ) -> anyhow::Result<()> {
       my::interface::my_api::add_to_linker(linker, f)?;
       Ok(())
   }
   ```

4. **Build and integrate** following the patterns in this example.

## References

- **Component Model**: https://component-model.bytecodealliance.org/
- **wit-bindgen**: https://github.com/bytecodealliance/wit-bindgen
- **WIT Format**: https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md
- **Orchestration PKCS#11 Example**: `~/git/webassembly-compoent-orchestration/libs/pkcs11-host-adapter/`

## License

Apache-2.0 WITH LLVM-exception (same as Wasmtime)
