#else
/* PaintCraft pinned-build boundary. C allocator layout/lifetime is owned by
 * the executable's charged Rust shim; no system-allocator fallback exists. */
void *paintcraft_dav1d_alloc(size_t size, size_t alignment, unsigned kind);
void *paintcraft_dav1d_realloc(void *pointer, size_t size, unsigned kind);
void paintcraft_dav1d_free(void *pointer);
#define dav1d_malloc(type, sz) paintcraft_dav1d_alloc(sz, 16, type)
#define dav1d_realloc(type, ptr, sz) paintcraft_dav1d_realloc(ptr, sz, type)
#define dav1d_free(ptr) paintcraft_dav1d_free(ptr)
#define dav1d_alloc_aligned(type, sz, align) paintcraft_dav1d_alloc(sz, align, type)
#define dav1d_free_aligned(ptr) paintcraft_dav1d_free(ptr)

