# Host Bundle Feature Implementation Summary

## Overview

This branch implements support for loading host interfaces in the Wasmtime CLI through two mechanisms:
1. **Host Bundles** - Self-contained packages with WIT + native implementation
2. **Host Manifests** - Configuration files to manage multiple hosts

## Implementation Details

### Files Created

1. **src/host_bundle.rs** (368 lines)
   - `HostBundle` - Represents a single host bundle
   - `HostBundleConfig` - Bundle configuration from host.toml
   - `HostManifest` - Manifest configuration from hosts.toml
   - `HostBundles` - Collection manager for loaded bundles
   - Parsing and validation logic
   - Search path resolution

2. **examples/host-bundles/** - Complete documentation and examples
   - README.md - Comprehensive usage guide
   - example_host/ - Sample bundle structure
   - hosts.toml - Example manifest configuration

### Files Modified

1. **src/lib.rs**
   - Added `host_bundle` module (gated by `component-model` feature)

2. **src/commands/run.rs**
   - Added `--host-bundle` CLI flag (accepts multiple bundles)
   - Added `--host-config` CLI flag (accepts manifest path)
   - Added `load_host_bundles()` method
   - Integrated bundle loading in `new_store_and_linker()`
   - Added validation and diagnostic output

3. **src/commands/wizer.rs**
   - Updated `RunCommand` initialization with new fields

4. **Cargo.toml**
   - Added `toml` to dependencies

## Usage Examples

### Load Single Bundle
```bash
wasmtime component run --host-bundle ./duckdb_host app.wasm
```

### Load Multiple Bundles
```bash
wasmtime component run \
  --host-bundle ./duckdb_host \
  --host-bundle ./pkcs11_host \
  app.wasm
```

### Use Manifest
```bash
wasmtime component run --host-config hosts.toml app.wasm
```

## Host Bundle Structure

```
my_host_bundle/
  host.toml           # Bundle metadata
  wit/                # WIT interface definitions
    my-interface/
      world.wit
  lib/                # Native library
    libmy_host.so
```

### host.toml Format

```toml
[host]
name = "my-host"
lib = "lib/libmy_host.so"
wit = "wit/my-interface"
```

## Host Manifest Format (hosts.toml)

```toml
[global]
search_paths = ["./hosts", "/usr/local/share/wasmtime/hosts"]

[[host]]
name = "duckdb"
bundle = "duckdb_host"

[[host]]
name = "pkcs11"
wit = "/opt/pkcs11-host/pkcs11.wit"
lib = "/opt/pkcs11-host/libpkcs11_host.so"
```

## Key Features

✅ Bundle validation and loading
✅ Manifest parsing with search path resolution
✅ Support for both bundle references and explicit paths
✅ Multiple bundle loading via repeated --host-bundle flags
✅ Comprehensive error messages with context
✅ Feature-gated behind `component-model` feature
✅ Full documentation and examples

## Current Status

**Phase 1 (Completed)**: Infrastructure and Validation
- ✅ Data structures for bundles and manifests
- ✅ TOML parsing and validation
- ✅ CLI flags and argument handling
- ✅ File existence and path validation
- ✅ Search path resolution
- ✅ Documentation and examples

**Phase 2 (Future Work)**: Dynamic Linking Integration
- ⏳ Load WIT definitions into component linker
- ⏳ Dynamic library loading (dlopen/LoadLibrary)
- ⏳ ABI bridging to Component Model
- ⏳ Function registration with linker
- ⏳ Lifetime management of loaded libraries

## Technical Notes

### Design Decisions

1. **Separate host.toml from WIT**: Allows clear separation of metadata from interface
2. **Support both bundles and explicit paths**: Flexibility for different deployment scenarios
3. **Search paths in manifests**: Easy distribution and installation
4. **Feature-gated**: Only available when component-model is enabled

### Integration Points

The current implementation:
- Loads and validates bundles during `new_store_and_linker()`
- Only activates for component targets (not core modules)
- Provides diagnostic output showing loaded bundles
- Has TODO markers for future integration work

### Future Enhancement Requirements

To complete the integration:

1. **WIT Loading**: Use wasmtime's component model APIs to load WIT dynamically
2. **Dynamic Linking**: Use `libloading` or platform APIs for shared library loading
3. **ABI Bridging**: Implement canonical ABI conversion between native and wasm
4. **Symbol Resolution**: Discover and bind exported functions from loaded libraries
5. **Safety**: Ensure proper error handling and cleanup of dynamic resources

## Testing

### Build Verification
```bash
cargo check --features component-model
```

### Manual Testing
1. Create a host bundle with the example structure
2. Run: `wasmtime component run --host-bundle ./example_host test.wasm`
3. Verify bundle validation and diagnostic output

## Contributing

To extend this feature:

1. Implement dynamic WIT loading in `src/host_bundle.rs`
2. Add library loading using `libloading` crate
3. Bridge native functions to component linker
4. Add integration tests in `tests/`
5. Update documentation with complete examples

## References

- Component Model: https://component-model.bytecodealliance.org/
- WIT Format: https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md
- Wasmtime Component Guide: https://docs.wasmtime.dev/

## Commit

Branch: `feat/host-interface-bundles`
Commit: `dae846836`
