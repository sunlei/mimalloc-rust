// A shared library whose `malloc` and `free` are resolved when it is loaded.
#include <stdlib.h>

// Found through DEP_MIMALLOC_INCLUDE_DIR.
#include <mimalloc.h>

void *dep_malloc(size_t size) { return malloc(size); }

void dep_free(void *ptr) { free(ptr); }

int dep_mimalloc_version(void) { return MI_MALLOC_VERSION; }
