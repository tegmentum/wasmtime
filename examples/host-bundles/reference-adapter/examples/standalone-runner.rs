//! Standalone runner demonstrating host adapter integration
//!
//! This shows how to integrate a host adapter with wasmtime's component model.
//! It can serve as a reference for integrating adapters into the CLI.

use anyhow::Result;
use wasmtime::*;
use wasmtime::component::*;
use keyvalue_host_adapter::{KeyValueStoreImpl, add_to_linker};

// Host state that includes the key-value store
struct HostState {
    wasi: wasmtime_wasi::WasiCtx,
    table: wasmtime_wasi::ResourceTable,
    keyvalue: KeyValueStoreImpl,
}

impl wasmtime_wasi::WasiView for HostState {
    fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
        wasmtime_wasi::WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

fn main() -> Result<()> {
    println!("Wasmtime Host Adapter Example");
    println!("==============================\n");

    // Create engine with component model support
    let mut config = Config::new();
    config.wasm_component_model(true);
    let engine = Engine::new(&config)?;

    println!("✓ Created engine with component model support");

    // Create linker and add WASI + key-value store
    let mut linker = Linker::<HostState>::new(&engine);

    // Add WASI support
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;
    println!("✓ Added WASI to linker");

    // Add key-value store adapter
    add_to_linker(&mut linker, |state: &mut HostState| &mut state.keyvalue)?;
    println!("✓ Added key-value store adapter to linker");

    println!("\n📝 Host adapter successfully configured!");
    println!("\nTo use this with a component:");
    println!("1. Create a component that imports keyvalue:store/store");
    println!("2. Load it with Component::from_file()");
    println!("3. Instantiate with linker.instantiate()");
    println!("\nExample WIT for your component:");
    println!("```wit");
    println!("package my:app;");
    println!();
    println!("world app {");
    println!("    import keyvalue:store/store@0.1.0;");
    println!("}");
    println!("```");

    Ok(())
}
