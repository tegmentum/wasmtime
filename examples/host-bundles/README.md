# Host Bundles Example

This example demonstrates how to use host bundles and host manifests with the Wasmtime CLI to provide native host implementations to WebAssembly components.

## Overview

Wasmtime now supports loading host implementations via two mechanisms:

1. **Host Bundles** - Self-contained directories with WIT definitions and native libraries
2. **Host Manifests** - Configuration files that reference multiple host bundles

## Host Bundle Structure

A host bundle is a directory containing:

```
my_host_bundle/
  host.toml           # Bundle configuration
  wit/                # WIT definitions
    my-interface/
      runtime.wit
  lib/                # Native library implementation
    libmy_host.so     # (or .dylib on macOS, .dll on Windows)
```

### host.toml Format

```toml
[host]
name = "my-host"
lib = "lib/libmy_host.so"
wit = "wit/my-interface"
```

## Using Host Bundles

### Option 1: Direct Bundle Loading

Load individual bundles using the `--host-bundle` flag:

```bash
wasmtime component run \
  --host-bundle ./duckdb_host \
  --host-bundle ./pkcs11_host \
  my_component.wasm
```

### Option 2: Host Manifest

Create a `hosts.toml` manifest to configure multiple hosts:

```toml
[global]
search_paths = ["./hosts", "/usr/local/share/wasmtime/hosts"]

[[host]]
name = "duckdb"
bundle = "duckdb_host"   # Looked up in search_paths

[[host]]
name = "pkcs11"
wit = "/opt/pkcs11-host/pkcs11.wit"
lib = "/opt/pkcs11-host/libpkcs11_host.so"
```

Then run with:

```bash
wasmtime component run --host-config hosts.toml my_component.wasm
```

## Example: DuckDB Host Bundle

Here's a complete example of a DuckDB host bundle:

### Directory Structure

```
duckdb_host/
  host.toml
  wit/
    duckdb-extension/
      runtime.wit
  lib/
    libduckdb_host.dylib
```

### host.toml

```toml
[host]
name = "duckdb"
lib = "lib/libduckdb_host.dylib"
wit = "wit/duckdb-extension"
```

### wit/duckdb-extension/runtime.wit

```wit
package duckdb:extension;

interface runtime {
  // Execute a SQL query and return results
  query: func(sql: string) -> result<list<string>, string>;

  // Open a database connection
  open: func(path: string) -> result<u32, string>;

  // Close a database connection
  close: func(handle: u32) -> result<_, string>;
}

world duckdb-runtime {
  export runtime;
}
```

## Host Manifest Examples

### Example 1: Multiple Bundles with Search Paths

```toml
[global]
search_paths = ["./hosts", "~/.wasmtime/hosts"]

[[host]]
name = "duckdb"
bundle = "duckdb_host"

[[host]]
name = "sqlite"
bundle = "sqlite_host"
```

### Example 2: Mixed Bundle and Explicit Paths

```toml
[global]
search_paths = ["./hosts"]

[[host]]
name = "duckdb"
bundle = "duckdb_host"

[[host]]
name = "custom-crypto"
wit = "/opt/crypto/crypto.wit"
lib = "/opt/crypto/libcrypto_host.so"
```

## Building a Host Implementation

To create a host implementation:

1. **Define your WIT interface** - Specify the host functions your component needs
2. **Implement the native library** - Build a shared library that implements the WIT interface
3. **Create the bundle structure** - Organize files according to the bundle layout
4. **Write host.toml** - Configure the bundle metadata

### Native Library Requirements

The native library must:
- Export functions matching the WIT interface signatures
- Follow the Component Model ABI conventions
- Be compiled for the target platform

## Distribution

Host bundles can be distributed as:

1. **Directories** - Simply copy the bundle directory
2. **Tarballs** - Package as `.tar.gz` for easy distribution
3. **System installation** - Install to standard locations like `/usr/local/share/wasmtime/hosts/`

Example installation:

```bash
# Extract bundle
tar -xzf duckdb_host.tar.gz -C ~/.wasmtime/hosts/

# Use in manifest
cat > hosts.toml <<EOF
[global]
search_paths = ["~/.wasmtime/hosts"]

[[host]]
name = "duckdb"
bundle = "duckdb_host"
EOF
```

## Current Limitations

**Note**: The current implementation validates and loads host bundles but does not yet fully integrate them into the component linker. The full integration requires:

1. Dynamic loading of WIT definitions
2. Runtime linking of native libraries
3. ABI bridging between native code and the Component Model

These features are planned for future releases.

## Advanced Usage

### Combining with WASI

Host bundles work alongside WASI interfaces:

```bash
wasmtime component run \
  --host-bundle ./my_host \
  --dir /data \
  --env DATABASE_URL=postgres://localhost/mydb \
  my_component.wasm
```

### Multiple Instances

You can load the same bundle multiple times with different configurations (when full integration is complete):

```toml
[[host]]
name = "duckdb-primary"
bundle = "duckdb_host"

[[host]]
name = "duckdb-secondary"
bundle = "duckdb_host"
```

## See Also

- [Component Model Documentation](https://component-model.bytecodealliance.org/)
- [WIT Format Specification](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md)
- [Wasmtime Component Guide](https://docs.wasmtime.dev/lang-rust.html)
