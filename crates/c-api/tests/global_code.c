#include <wasm.h>
#include <wasmtime.h>
#include <wasmtime/wat.h>

#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
  wasm_engine_t *engine;
  wasmtime_module_t *module;
  uintptr_t module_start;
} held_module_t;

typedef struct {
  held_module_t *items;
  size_t len;
  size_t cap;
} held_list_t;

static void held_list_push(held_list_t *list, wasm_engine_t *engine,
                           wasmtime_module_t *module,
                           uintptr_t module_start) {
  if (list->len == list->cap) {
    size_t new_cap = list->cap == 0 ? 8 : list->cap * 2;
    held_module_t *next =
        (held_module_t *)realloc(list->items, new_cap * sizeof(*next));
    if (!next) {
      fprintf(stderr, "out of memory\n");
      exit(1);
    }
    list->items = next;
    list->cap = new_cap;
  }
  list->items[list->len].engine = engine;
  list->items[list->len].module = module;
  list->items[list->len].module_start = module_start;
  list->len++;
}

static bool address_is_mapped(uintptr_t addr) {
#ifdef __linux__
  FILE *maps = fopen("/proc/self/maps", "r");
  if (!maps) {
    fprintf(stderr, "failed to open /proc/self/maps\n");
    exit(1);
  }

  char line[512];
  while (fgets(line, sizeof(line), maps) != NULL) {
    uintptr_t start = 0;
    uintptr_t end = 0;
    if (sscanf(line, "%lx-%lx", &start, &end) == 2) {
      if (start <= addr && addr < end) {
        fclose(maps);
        return true;
      }
    }
  }

  fclose(maps);
  return false;
#else
  (void)addr;
  return false;
#endif
}

static uintptr_t module_start(wasmtime_module_t *module) {
  void *start = NULL;
  void *end = NULL;
  wasmtime_module_image_range(module, &start, &end);
  if (start == NULL || end == NULL || (uintptr_t)start >= (uintptr_t)end) {
    fprintf(stderr, "invalid module image range\n");
    exit(1);
  }
  return (uintptr_t)start;
}

static void assert_mapped(uintptr_t addr, const char *context) {
#ifdef __linux__
  if (!address_is_mapped(addr)) {
    fprintf(stderr, "%s: expected mapped address %#lx\n", context,
            (unsigned long)addr);
    exit(1);
  }
#else
  (void)addr;
  (void)context;
#endif
}

static void assert_unmapped(uintptr_t addr, const char *context) {
#ifdef __linux__
  if (address_is_mapped(addr)) {
    fprintf(stderr, "%s: expected unmapped address %#lx\n", context,
            (unsigned long)addr);
    exit(1);
  }
#else
  (void)addr;
  (void)context;
#endif
}

static bool held_list_pop(held_list_t *list, held_module_t *out) {
  if (list->len == 0) {
    return false;
  }
  list->len--;
  *out = list->items[list->len];
  return true;
}

static void held_list_drop_all(held_list_t *list) {
  for (size_t i = 0; i < list->len; i++) {
    wasmtime_module_delete(list->items[i].module);
    wasm_engine_delete(list->items[i].engine);
  }
  free(list->items);
  list->items = NULL;
  list->len = 0;
  list->cap = 0;
}

static void exit_if_error(wasmtime_error_t *error, const char *context) {
  if (!error) {
    return;
  }
  wasm_name_t message;
  wasmtime_error_message(error, &message);
  fprintf(stderr, "%s: %.*s\n", context, (int)message.size, message.data);
  wasm_byte_vec_delete(&message);
  wasmtime_error_delete(error);
  exit(1);
}

static void compile_module(wasm_engine_t *engine, const char *wat,
                           wasmtime_module_t **out) {
  wasm_byte_vec_t wasm;
  wasmtime_error_t *error =
      wasmtime_wat2wasm(wat, strlen(wat), &wasm);
  exit_if_error(error, "wat2wasm failed");

  error = wasmtime_module_new(engine, (const uint8_t *)wasm.data, wasm.size,
                              out);
  wasm_byte_vec_delete(&wasm);
  exit_if_error(error, "module_new failed");
}

static wasm_engine_t *new_engine_with_signals_disabled(void) {
  wasm_config_t *config = wasm_config_new();
  if (!config) {
    fprintf(stderr, "failed to allocate config\n");
    exit(1);
  }
  wasmtime_config_signals_based_traps_set(config, false);
  wasm_engine_t *engine = wasm_engine_new_with_config(config);
  if (!engine) {
    fprintf(stderr, "failed to create engine\n");
    exit(1);
  }
  return engine;
}

static void test_unregisters_on_module_drop(void) {
  for (size_t i = 0; i < 600; i++) {
    wasm_engine_t *engine = wasm_engine_new();
    if (!engine) {
      fprintf(stderr, "failed to create engine\n");
      exit(1);
    }
    char wat[128];
    snprintf(wat, sizeof(wat),
             "(module (memory (export \"mem\") 1) "
             "(data (i32.const 0) \"%zu\") (func (export \"f\")))",
             i);
    wasmtime_module_t *module = NULL;
    compile_module(engine, wat, &module);
    uintptr_t pc = module_start(module);
    assert_mapped(pc, "on_module_drop pre-drop");
    wasmtime_module_delete(module);
    wasm_engine_delete(engine);
    assert_unmapped(pc, "on_module_drop post-drop");
  }
}

static void test_unregisters_same_module(void) {
  const char *wat =
      "(module (func (export \"test\") (result i32) i32.const 42))";

  for (size_t i = 0; i < 500; i++) {
    wasm_engine_t *engine = new_engine_with_signals_disabled();
    wasmtime_module_t *module = NULL;
    compile_module(engine, wat, &module);
    uintptr_t pc = module_start(module);
    assert_mapped(pc, "same_module pre-drop");
    wasmtime_module_delete(module);
    wasm_engine_delete(engine);
    assert_unmapped(pc, "same_module post-drop");

    if (i % 100 == 0) {
      fprintf(stderr, "Iteration %zu\n", i);
    }
  }
}

static void test_unregisters_same_engine(void) {
  const char *wat =
      "(module (func (export \"test\") (result i32) i32.const 42))";
  wasm_engine_t *engine = new_engine_with_signals_disabled();

  for (size_t i = 0; i < 500; i++) {
    wasmtime_module_t *module = NULL;
    compile_module(engine, wat, &module);
    uintptr_t pc = module_start(module);
    assert_mapped(pc, "same_engine pre-drop");
    wasmtime_module_delete(module);
    assert_unmapped(pc, "same_engine post-drop");

    if (i % 100 == 0) {
      fprintf(stderr, "Iteration %zu\n", i);
    }
  }

  wasm_engine_delete(engine);
}

static void test_unregisters_under_pressure(void) {
  const char *wat =
      "(module (memory (export \"mem\") 1) (data (i32.const 0) \"pressure\") "
      "(func (export \"test\") (result i32) i32.const 42))";

  held_list_t held = {0};

  for (size_t i = 0; i < 1000; i++) {
    wasm_engine_t *engine = new_engine_with_signals_disabled();
    wasmtime_module_t *module = NULL;
    compile_module(engine, wat, &module);
    uintptr_t pc = module_start(module);
    assert_mapped(pc, "under_pressure pre-drop");

    if (i % 3 == 0) {
      held_list_push(&held, engine, module, pc);
    } else {
      wasmtime_module_delete(module);
      wasm_engine_delete(engine);
      assert_unmapped(pc, "under_pressure post-drop");
    }

    if (i % 10 == 0) {
      held_module_t dropped;
      if (held_list_pop(&held, &dropped)) {
        wasmtime_module_delete(dropped.module);
        wasm_engine_delete(dropped.engine);
        assert_unmapped(dropped.module_start,
                        "under_pressure delayed post-drop");
      }
    }
  }

  for (size_t i = 0; i < held.len; i++) {
    assert_mapped(held.items[i].module_start, "under_pressure final pre-drop");
  }
  held_list_drop_all(&held);
}

typedef struct {
  size_t thread_id;
  const char *wat;
} thread_ctx_t;

static void *thread_pressure(void *arg) {
  thread_ctx_t *ctx = (thread_ctx_t *)arg;
  held_list_t held = {0};

  for (size_t i = 0; i < 1000; i++) {
    wasm_engine_t *engine = new_engine_with_signals_disabled();
    wasmtime_module_t *module = NULL;
    compile_module(engine, ctx->wat, &module);
    uintptr_t pc = module_start(module);
    assert_mapped(pc, "threaded pre-drop");

    if ((i + ctx->thread_id) % 4 == 0) {
      held_list_push(&held, engine, module, pc);
    } else {
      wasmtime_module_delete(module);
      wasm_engine_delete(engine);
      assert_unmapped(pc, "threaded post-drop");
    }

    if (i % 25 == 0) {
      held_module_t dropped;
      if (held_list_pop(&held, &dropped)) {
        wasmtime_module_delete(dropped.module);
        wasm_engine_delete(dropped.engine);
        assert_unmapped(dropped.module_start, "threaded delayed post-drop");
      }
    }
  }

  for (size_t i = 0; i < held.len; i++) {
    assert_mapped(held.items[i].module_start, "threaded final pre-drop");
  }
  held_list_drop_all(&held);
  return NULL;
}

static void test_unregisters_under_threaded_pressure(void) {
  const char *wat =
      "(module (memory (export \"mem\") 1) (data (i32.const 0) \"threaded\") "
      "(func (export \"test\") (result i32) i32.const 42))";

  pthread_t threads[4];
  thread_ctx_t ctx[4];

  for (size_t i = 0; i < 4; i++) {
    ctx[i].thread_id = i;
    ctx[i].wat = wat;
    if (pthread_create(&threads[i], NULL, thread_pressure, &ctx[i]) != 0) {
      fprintf(stderr, "failed to create thread %zu\n", i);
      exit(1);
    }
  }

  for (size_t i = 0; i < 4; i++) {
    pthread_join(threads[i], NULL);
  }
}

int main(void) {
  test_unregisters_on_module_drop();
  test_unregisters_same_module();
  test_unregisters_same_engine();
  test_unregisters_under_pressure();
  test_unregisters_under_threaded_pressure();
  return 0;
}
