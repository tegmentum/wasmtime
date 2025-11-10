# Host Bundle Integration Guide

This guide explains how to integrate the host bundle feature with the component loader from the webassembly-component-orchestration project.

## Architecture Overview

The host bundle system consists of three main components:

1. **Host Bundles** (`src/host_bundle.rs`) - TOML-based configuration and bundle management
2. **Host Adapters** (`src/host_adapter.rs`) - Dynamic library loading and WIT bridging
3. **Component Integration** (`src/commands/run.rs`) - CLI integration and linker setup

## Current Implementation Status

### ✅ Completed Features

- **Bundle Structure**: Defined format for host.toml and bundle layout
- **Manifest Support**: hosts.toml for configuring multiple bundles with search paths
- **CLI Flags**: `--host-bundle` and `--host-config` arguments
- **Dynamic Loading**: Using `libloading` to load native libraries at runtime
- **Validation**: Comprehensive error checking for bundle structure and paths
- **Integration**: Wired into wasmtime CLI's component execution path

### 🚧 Partial Implementation

- **WIT Loading**: Can read WIT files but not yet parsing or generating bindings at runtime
- **ABI Bridging**: Library is loaded but functions not yet callable from components
- **Linker Integration**: Host adapters registered but not connected to linker instances

### ⏳ Future Work

- **Runtime WIT Parsing**: Parse WIT files at runtime to discover interfaces
- **Dynamic Binding Generation**: Generate Component Model bindings on-the-fly
- **Function Registration**: Register host functions with component linker
- **ABI Conversion**: Bridge between native calling conventions and Component Model

## Using the Component Orchestration Loader

The `/Users/zacharywhitley/git/webassembly-component-orchestration` repository provides excellent examples of component loading. Here's how to leverage it:

### Pattern 1: Static Host Adapters (Recommended for Production)

This approach uses compile-time binding generation similar to `pkcs11-host-adapter`:

```rust
// In your host adapter crate (e.g., duckdb-host-adapter)

// Generate bindings at build time
wit_bindgen::generate!({
    world: "duckdb:host/duckdb",
    path: ["path/to/wit/duckdb.wit"],
});

// Implement the host interface
struct DuckDbHost {
    library: libloading::Library,
}

impl DuckDbHost {
    pub fn new(lib_path: &Path) -> Result<Self> {
        let library = unsafe { libloading::Library::new(lib_path)? };
        Ok(Self { library })
    }
}

// Implement generated traits
impl exports::duckdb::extension::runtime::Guest for DuckDbHost {
    fn query(sql: String) -> Result<Vec<String>, String> {
        // Load function symbol from library
        let query_fn: Symbol<extern "C" fn(*const u8, usize, *mut *mut u8, *mut usize) -> i32> =
            unsafe { self.library.get(b"duckdb_query")? };

        // Marshal arguments and call native function
        // ... implementation ...
    }
}

// Add to linker
pub fn add_to_linker<T>(linker: &mut Linker<T>) -> Result<()> {
    // Use generated bindings to add to linker
    duckdb::extension::runtime::add_to_linker(linker, |_ctx| /* ... */)?;
    Ok(())
}
```

**Benefits**:
- Type-safe at compile time
- No runtime WIT parsing overhead
- Better error messages
- Easier to debug

**Drawbacks**:
- Requires rebuild for new interfaces
- Can't load arbitrary bundles at runtime

### Pattern 2: Dynamic Host Adapters (Experimental)

For truly dynamic loading, you need runtime WIT parsing:

```rust
use wit_parser::{Resolve, UnresolvedPackage};
use wasmtime::component::{Linker, ComponentType};

pub fn load_dynamic_host(
    linker: &mut Linker<T>,
    wit_path: &Path,
    lib_path: &Path,
) -> Result<()> {
    // Parse WIT at runtime
    let mut resolve = Resolve::new();
    let pkg = UnresolvedPackage::parse_file(wit_path)?;
    let pkg_id = resolve.push(pkg)?;

    // Load native library
    let library = unsafe { libloading::Library::new(lib_path)? };

    // For each interface in the WIT package
    for (_, world) in resolve.worlds.iter() {
        // For each import in the world
        for (name, item) in &world.imports {
            match item {
                WorldItem::Function(func) => {
                    // Look up symbol in library
                    let symbol_name = format_symbol_name(name);
                    let func_ptr = unsafe {
                        library.get::<*mut ()>(symbol_name.as_bytes())?
                    };

                    // Create adapter that bridges Component Model ABI to native call
                    let adapter = create_abi_adapter(func, func_ptr)?;

                    // Register with linker
                    linker.instance(world.name)?
                        .func_new(&name, adapter)?;
                }
                _ => { /* handle other item types */ }
            }
        }
    }

    Ok(())
}
```

**Challenges**:
- Component Model ABI is complex
- Need to handle all WIT types (records, variants, resources, etc.)
- Memory management across boundary
- Error handling and cleanup

### Recommended Hybrid Approach

1. **Common Host Interfaces**: Use static bindings (Pattern 1)
   - WASI interfaces
   - Popular extensions (DuckDB, PKCS#11, etc.)
   - Ship as compiled adapters

2. **User-Provided Hosts**: Support dynamic loading (Pattern 2)
   - Allow users to drop in bundles
   - Use runtime WIT parsing
   - Trade some safety for flexibility

## Integration Steps

### Step 1: Build Static Host Adapters

Create a separate crate for each common host:

```
wasmtime-host-adapters/
  duckdb/
    Cargo.toml
    build.rs      # wit-bindgen invocation
    wit/
      duckdb.wit
    src/
      lib.rs      # Adapter implementation
  pkcs11/
    ...
```

### Step 2: Update Wasmtime CLI to Use Adapters

```rust
// In src/commands/run.rs

use wasmtime_host_adapters::{duckdb, pkcs11};

fn setup_component_linker<T>(linker: &mut Linker<T>, bundles: &HostBundles) -> Result<()> {
    // Add static adapters
    for bundle in bundles.bundles() {
        match bundle.name() {
            "duckdb" => {
                let host = duckdb::DuckDbHost::new(bundle.lib_path())?;
                duckdb::add_to_linker(linker, move |_ctx| &host)?;
            }
            "pkcs11" => {
                let host = pkcs11::Pkcs11Host::new(bundle.lib_path())?;
                pkcs11::add_to_linker(linker, move |_ctx| &host)?;
            }
            unknown => {
                // Fall back to dynamic loading for unknown bundles
                load_dynamic_host(linker, bundle.wit_path(), bundle.lib_path())?;
            }
        }
    }
    Ok(())
}
```

### Step 3: Implement Dynamic Loader (Future Work)

The dynamic loader would need to:

1. Parse WIT using `wit-parser`
2. Enumerate exported functions
3. Load symbols from native library
4. Generate adapters that handle:
   - Lowering Component Model arguments to native types
   - Calling native function
   - Lifting native return values to Component Model types
5. Register adapters with linker

## Example: Complete Host Adapter

See `libs/pkcs11-host-adapter` in the orchestration repo for a complete example:

```bash
cd ~/git/webassembly-compoent-orchestration/libs/pkcs11-host-adapter
```

Key files:
- `src/lib.rs` - Adapter implementation
- `Cargo.toml` - wit-bindgen configuration
- `build.rs` - Build-time code generation

## Testing

### Test Bundle Structure

Create a test bundle:

```bash
mkdir -p test_bundles/example_host/{wit,lib}

cat > test_bundles/example_host/host.toml <<EOF
[host]
name = "example"
lib = "lib/libexample.so"
wit = "wit/example.wit"
EOF

cat > test_bundles/example_host/wit/example.wit <<EOF
package example:interface@1.0.0;

interface runtime {
  greet: func(name: string) -> string;
}

world example-runtime {
  import runtime;
}
EOF
```

### Test with Wasmtime CLI

```bash
# Build a component that imports the interface
wasm-tools component wit example.wit

# Run with host bundle
cargo run --features component-model -- component run \
  --host-bundle test_bundles/example_host \
  example_component.wasm
```

## Next Steps

1. **Prototype Static Adapter**
   - Choose a simple interface (e.g., key-value store)
   - Implement using Pattern 1
   - Test end-to-end with wasmtime CLI

2. **Implement Core Dynamic Loading**
   - Add wit-parser dependency
   - Parse WIT at runtime
   - Create simple function adapters (strings only)

3. **Expand Type Support**
   - Add support for integers, floats
   - Handle records and variants
   - Support resources (handles)

4. **Error Handling**
   - Proper cleanup on failures
   - Clear error messages
   - Validation of ABI compatibility

5. **Documentation**
   - Host adapter authoring guide
   - Bundle creation tutorial
   - Security best practices

## References

- **Orchestration Repo**: `/Users/zacharywhitley/git/webassembly-compoent-orchestration`
- **PKCS#11 Adapter**: `libs/pkcs11-host-adapter/`
- **Wasmtime Host**: `hosts/wasmtime/src/exec.rs`
- **Component Model**: https://component-model.bytecodealliance.org/
- **wit-bindgen**: https://github.com/bytecodealliance/wit-bindgen
- **wit-parser**: https://github.com/bytecodealliance/wasm-tools/tree/main/crates/wit-parser
