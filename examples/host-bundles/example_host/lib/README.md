# Native Library Directory

This directory should contain the compiled native library that implements the WIT interface.

For example:
- Linux: `libexample_host.so`
- macOS: `libexample_host.dylib`
- Windows: `example_host.dll`

## Building the Native Library

The native library must implement the functions defined in `../wit/example-interface/world.wit` following the Component Model ABI.

Example implementation (pseudo-code):

```c
#include <stdint.h>
#include <string.h>

// Implement greet function
void example_greet(const char* name, char* result, size_t result_len) {
    snprintf(result, result_len, "Hello, %s!", name);
}

// Implement add function
uint32_t example_add(uint32_t a, uint32_t b) {
    return a + b;
}

// Implement get-config function
int example_get_config(const char* key, char* value, size_t value_len) {
    // Lookup configuration value
    // Return 0 on success, non-zero on error
    if (strcmp(key, "version") == 0) {
        strncpy(value, "1.0.0", value_len);
        return 0;
    }
    return -1;
}
```

Note: The actual implementation must follow the Component Model canonical ABI, which may require additional glue code or using a binding generator.
