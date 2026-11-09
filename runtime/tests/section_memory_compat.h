#include <stddef.h>
#include <sys/types.h>
#include <unistd.h>

static void *host_alloc_pages( size_t size );
static void host_free_pages( void *pointer, size_t size );

#define HOST_PAGE ((size_t)getpagesize())
#define HORIZON_MEMFILE_PAGE_SIZE HOST_PAGE
#define HORIZON_MEMFILE_ALLOC_PAGES(size) host_alloc_pages( size )
#define HORIZON_MEMFILE_FREE_PAGES(ptr, size) host_free_pages( ptr, size )
#include "horizon_memfile.h"
