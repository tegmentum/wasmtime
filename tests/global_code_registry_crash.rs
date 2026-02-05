//! Reproduction test for GLOBAL_CODE registry crash with multiple threads.
//!
//! This test demonstrates a bug where rapid creation and destruction of
//! engines/modules across multiple threads causes SIGABRT due to:
//!
//! 1. Virtual address reuse by the OS for new mmap allocations
//! 2. Arc deferred deallocation leaving entries in the registry
//! 3. The `assert!(prev.is_none())` in `register_code()` aborting the process
//!
//! Run with: cargo test --test global_code_registry_crash -- --nocapture
//!
//! Expected behavior WITHOUT fix: Process aborts with SIGABRT
//! Expected behavior WITH fix: All iterations complete successfully

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use wasmtime::{Config, Engine, Module, Store, Instance};

/// Multi-threaded stress test that triggers GLOBAL_CODE registry crash.
///
/// This creates multiple threads that each rapidly create and destroy
/// engines and modules, maximizing the chance of address reuse collision.
#[test]
fn test_global_code_registry_multithread_crash() {
    const THREADS: usize = 8;
    const ITERATIONS_PER_THREAD: usize = 100;

    let completed = Arc::new(AtomicUsize::new(0));

    eprintln!("=== GLOBAL_CODE Registry Multi-Thread Crash Test ===");
    eprintln!("Running {} threads x {} iterations...", THREADS, ITERATIONS_PER_THREAD);
    eprintln!("Without the fix, this should crash with SIGABRT");
    eprintln!();

    let handles: Vec<_> = (0..THREADS)
        .map(|thread_id| {
            let completed = Arc::clone(&completed);
            thread::spawn(move || {
                let wat = r#"(module (func (export "f") (result i32) i32.const 42))"#;

                for i in 0..ITERATIONS_PER_THREAD {
                    // Create fresh engine each iteration to maximize registrations
                    let mut config = Config::new();
                    config.signals_based_traps(false);

                    let engine = Engine::new(&config)
                        .unwrap_or_else(|e| panic!("Thread {} failed at iteration {}: {:?}", thread_id, i, e));

                    let module = Module::new(&engine, wat)
                        .unwrap_or_else(|e| panic!("Thread {} module compile failed at {}: {:?}", thread_id, i, e));

                    let mut store = Store::new(&engine, ());
                    let _instance = Instance::new(&mut store, &module, &[])
                        .unwrap_or_else(|e| panic!("Thread {} instantiate failed at {}: {:?}", thread_id, i, e));

                    // Explicit drops to trigger unregistration
                    drop(store);
                    drop(module);
                    drop(engine);
                }

                let count = completed.fetch_add(ITERATIONS_PER_THREAD, Ordering::SeqCst);
                eprintln!("  Thread {} completed {} iterations (total: {})",
                         thread_id, ITERATIONS_PER_THREAD, count + ITERATIONS_PER_THREAD);
            })
        })
        .collect();

    for (i, handle) in handles.into_iter().enumerate() {
        handle.join().unwrap_or_else(|_| panic!("Thread {} panicked", i));
    }

    eprintln!();
    eprintln!("=== TEST PASSED ===");
    eprintln!("All {} iterations completed across {} threads.",
              completed.load(Ordering::SeqCst), THREADS);
}

/// Sequential stress test for comparison.
#[test]
fn test_global_code_registry_sequential_crash() {
    const ITERATIONS: usize = 500;

    eprintln!("=== GLOBAL_CODE Registry Sequential Crash Test ===");
    eprintln!("Running {} sequential iterations...", ITERATIONS);
    eprintln!("Without the fix, this crashes around iteration 350-400");
    eprintln!();

    let wat = r#"(module (func (export "f") (result i32) i32.const 42))"#;

    for i in 0..ITERATIONS {
        let mut config = Config::new();
        config.signals_based_traps(false);

        let engine = Engine::new(&config)
            .unwrap_or_else(|e| panic!("Failed at iteration {}: {:?}", i, e));

        let module = Module::new(&engine, wat)
            .unwrap_or_else(|e| panic!("Module compile failed at {}: {:?}", i, e));

        let mut store = Store::new(&engine, ());
        let _instance = Instance::new(&mut store, &module, &[])
            .unwrap_or_else(|e| panic!("Instantiate failed at {}: {:?}", i, e));

        if (i + 1) % 100 == 0 {
            eprintln!("  Completed {} iterations...", i + 1);
        }
    }

    eprintln!();
    eprintln!("=== SEQUENTIAL TEST PASSED ===");
    eprintln!("All {} iterations completed.", ITERATIONS);
}

/// Aggressive concurrent test with held references to increase collision probability.
#[test]
fn test_global_code_registry_aggressive_crash() {
    const THREADS: usize = 16;
    const ITERATIONS_PER_THREAD: usize = 50;
    const HOLD_COUNT: usize = 10;

    let completed = Arc::new(AtomicUsize::new(0));

    eprintln!("=== GLOBAL_CODE Registry Aggressive Crash Test ===");
    eprintln!("Running {} threads x {} iterations, holding {} references...",
              THREADS, ITERATIONS_PER_THREAD, HOLD_COUNT);
    eprintln!();

    let handles: Vec<_> = (0..THREADS)
        .map(|thread_id| {
            let completed = Arc::clone(&completed);
            thread::spawn(move || {
                let wat = r#"(module
                    (memory (export "mem") 1)
                    (func (export "f1") (result i32) i32.const 1)
                    (func (export "f2") (result i32) i32.const 2)
                    (func (export "f3") (result i32) i32.const 3)
                )"#;

                // Hold references to delay deallocation
                let mut held: Vec<(Engine, Module)> = Vec::with_capacity(HOLD_COUNT);

                for i in 0..ITERATIONS_PER_THREAD {
                    let mut config = Config::new();
                    config.signals_based_traps(false);

                    let engine = Engine::new(&config)
                        .unwrap_or_else(|e| panic!("Thread {} failed at {}: {:?}", thread_id, i, e));

                    let module = Module::new(&engine, wat)
                        .unwrap_or_else(|e| panic!("Thread {} compile failed at {}: {:?}", thread_id, i, e));

                    let mut store = Store::new(&engine, ());
                    let _instance = Instance::new(&mut store, &module, &[])
                        .unwrap_or_else(|e| panic!("Thread {} instantiate failed at {}: {:?}", thread_id, i, e));

                    // Hold reference to delay deallocation
                    if held.len() >= HOLD_COUNT {
                        held.remove(0);
                    }
                    held.push((engine, module));
                }

                completed.fetch_add(ITERATIONS_PER_THREAD, Ordering::SeqCst);
                eprintln!("  Thread {} completed {} iterations", thread_id, ITERATIONS_PER_THREAD);
            })
        })
        .collect();

    for (i, handle) in handles.into_iter().enumerate() {
        handle.join().unwrap_or_else(|_| panic!("Thread {} panicked", i));
    }

    eprintln!();
    eprintln!("=== AGGRESSIVE TEST PASSED ===");
    eprintln!("All {} iterations completed.", completed.load(Ordering::SeqCst));
}

/// Extreme stress test - maximum parallelism and rapid cycling.
/// This test uses more threads and more iterations to maximize collision probability.
#[test]
fn test_global_code_registry_extreme_stress() {
    const THREADS: usize = 32;
    const ITERATIONS_PER_THREAD: usize = 200;

    let completed = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(THREADS));

    eprintln!("=== GLOBAL_CODE Registry Extreme Stress Test ===");
    eprintln!("Running {} threads x {} iterations (total: {} engine creations)...",
              THREADS, ITERATIONS_PER_THREAD, THREADS * ITERATIONS_PER_THREAD);
    eprintln!();

    let handles: Vec<_> = (0..THREADS)
        .map(|thread_id| {
            let completed = Arc::clone(&completed);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                // Minimal WAT to reduce compilation overhead
                let wat = r#"(module (func (export "f")))"#;

                // Wait for all threads to be ready
                barrier.wait();

                for i in 0..ITERATIONS_PER_THREAD {
                    let mut config = Config::new();
                    config.signals_based_traps(false);

                    let engine = Engine::new(&config)
                        .unwrap_or_else(|e| panic!("Thread {} failed at iteration {}: {:?}", thread_id, i, e));

                    let module = Module::new(&engine, wat)
                        .unwrap_or_else(|e| panic!("Thread {} module compile failed at {}: {:?}", thread_id, i, e));

                    let mut store = Store::new(&engine, ());
                    let _instance = Instance::new(&mut store, &module, &[])
                        .unwrap_or_else(|e| panic!("Thread {} instantiate failed at {}: {:?}", thread_id, i, e));

                    // Immediate drops - no holding references
                    drop(store);
                    drop(module);
                    drop(engine);
                }

                let count = completed.fetch_add(ITERATIONS_PER_THREAD, Ordering::SeqCst);
                if thread_id == 0 || thread_id == THREADS - 1 {
                    eprintln!("  Thread {} completed (total: {})", thread_id, count + ITERATIONS_PER_THREAD);
                }
            })
        })
        .collect();

    for (i, handle) in handles.into_iter().enumerate() {
        handle.join().unwrap_or_else(|_| panic!("Thread {} panicked", i));
    }

    eprintln!();
    eprintln!("=== EXTREME STRESS TEST PASSED ===");
    eprintln!("All {} engine creations completed.", completed.load(Ordering::SeqCst));
}

/// Interleaved create/destroy test - creates timing overlap between threads.
/// One set of threads creates engines while another set destroys them.
#[test]
fn test_global_code_registry_interleaved_create_destroy() {
    const CREATORS: usize = 8;
    const DESTROYERS: usize = 8;
    const ITERATIONS: usize = 100;

    use std::sync::mpsc;

    eprintln!("=== GLOBAL_CODE Registry Interleaved Create/Destroy Test ===");
    eprintln!("Running {} creators and {} destroyers x {} iterations...",
              CREATORS, DESTROYERS, ITERATIONS);
    eprintln!();

    // Channel to pass engines from creators to destroyers
    let (tx, rx) = mpsc::channel::<(Engine, Module, Store<()>)>();
    let rx = Arc::new(std::sync::Mutex::new(rx));

    // Spawn creator threads
    let creator_handles: Vec<_> = (0..CREATORS)
        .map(|creator_id| {
            let tx = tx.clone();
            thread::spawn(move || {
                let wat = r#"(module (func (export "f") (result i32) i32.const 42))"#;

                for i in 0..ITERATIONS {
                    let mut config = Config::new();
                    config.signals_based_traps(false);

                    let engine = Engine::new(&config)
                        .unwrap_or_else(|e| panic!("Creator {} failed at {}: {:?}", creator_id, i, e));

                    let module = Module::new(&engine, wat)
                        .unwrap_or_else(|e| panic!("Creator {} compile failed at {}: {:?}", creator_id, i, e));

                    let store = Store::new(&engine, ());

                    // Send to destroyer
                    let _ = tx.send((engine, module, store));
                }

                eprintln!("  Creator {} completed {} iterations", creator_id, ITERATIONS);
            })
        })
        .collect();

    // Drop original sender so receivers know when done
    drop(tx);

    // Spawn destroyer threads
    let destroyer_handles: Vec<_> = (0..DESTROYERS)
        .map(|destroyer_id| {
            let rx = Arc::clone(&rx);
            thread::spawn(move || {
                let mut destroyed = 0;
                loop {
                    let item = {
                        let rx = rx.lock().unwrap();
                        rx.recv()
                    };

                    match item {
                        Ok((engine, module, store)) => {
                            // Explicit drops in specific order
                            drop(store);
                            drop(module);
                            drop(engine);
                            destroyed += 1;
                        }
                        Err(_) => break, // Channel closed
                    }
                }
                eprintln!("  Destroyer {} destroyed {} engines", destroyer_id, destroyed);
            })
        })
        .collect();

    // Wait for all threads
    for handle in creator_handles {
        handle.join().expect("Creator panicked");
    }
    for handle in destroyer_handles {
        handle.join().expect("Destroyer panicked");
    }

    eprintln!();
    eprintln!("=== INTERLEAVED TEST PASSED ===");
}

/// Burst allocation test - allocate many engines at once, then release all.
/// This creates memory pressure that might trigger address reuse.
#[test]
fn test_global_code_registry_burst_allocation() {
    const BURST_SIZE: usize = 50;
    const BURSTS: usize = 20;

    eprintln!("=== GLOBAL_CODE Registry Burst Allocation Test ===");
    eprintln!("Running {} bursts of {} engines each...", BURSTS, BURST_SIZE);
    eprintln!();

    let wat = r#"(module
        (memory (export "mem") 10)
        (func (export "f") (result i32) i32.const 42)
    )"#;

    for burst in 0..BURSTS {
        // Allocate burst
        let mut engines: Vec<(Engine, Module, Store<()>)> = Vec::with_capacity(BURST_SIZE);

        for _ in 0..BURST_SIZE {
            let mut config = Config::new();
            config.signals_based_traps(false);

            let engine = Engine::new(&config).expect("Engine creation failed");
            let module = Module::new(&engine, wat).expect("Module creation failed");
            let store = Store::new(&engine, ());

            engines.push((engine, module, store));
        }

        // Release all at once
        drop(engines);

        if (burst + 1) % 5 == 0 {
            eprintln!("  Completed {} bursts...", burst + 1);
        }
    }

    eprintln!();
    eprintln!("=== BURST ALLOCATION TEST PASSED ===");
    eprintln!("All {} bursts completed.", BURSTS);
}

/// Mixed workload test - combines all patterns for maximum stress.
#[test]
fn test_global_code_registry_mixed_workload() {
    const THREADS: usize = 16;
    const DURATION_SECS: u64 = 5;

    use std::time::{Duration, Instant};

    let completed = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));

    eprintln!("=== GLOBAL_CODE Registry Mixed Workload Test ===");
    eprintln!("Running {} threads for {} seconds...", THREADS, DURATION_SECS);
    eprintln!();

    let handles: Vec<_> = (0..THREADS)
        .map(|thread_id| {
            let completed = Arc::clone(&completed);
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                let wat = r#"(module (func (export "f") (result i32) i32.const 42))"#;
                let mut local_count = 0;
                let mut held: Vec<(Engine, Module)> = Vec::new();

                while !stop.load(Ordering::Relaxed) {
                    let mut config = Config::new();
                    config.signals_based_traps(false);

                    let engine = match Engine::new(&config) {
                        Ok(e) => e,
                        Err(e) => {
                            eprintln!("Thread {} engine error: {:?}", thread_id, e);
                            continue;
                        }
                    };

                    let module = match Module::new(&engine, wat) {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!("Thread {} module error: {:?}", thread_id, e);
                            continue;
                        }
                    };

                    // Randomly hold or release
                    if local_count % 3 == 0 {
                        // Hold reference
                        if held.len() >= 5 {
                            held.remove(0);
                        }
                        held.push((engine, module));
                    } else {
                        // Immediate release
                        drop(module);
                        drop(engine);
                    }

                    local_count += 1;
                }

                completed.fetch_add(local_count, Ordering::SeqCst);
            })
        })
        .collect();

    // Run for specified duration
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(DURATION_SECS) {
        thread::sleep(Duration::from_millis(100));
    }
    stop.store(true, Ordering::Relaxed);

    // Wait for threads
    for handle in handles {
        let _ = handle.join();
    }

    eprintln!();
    eprintln!("=== MIXED WORKLOAD TEST PASSED ===");
    eprintln!("Completed {} engine creations in {} seconds.",
              completed.load(Ordering::SeqCst), DURATION_SECS);
}
