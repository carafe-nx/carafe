#include <stdio.h>
#include <pthread.h>
#include "../../external/autorun/horizon-wine/tests/horizon_memfile_host.h"
#include "horizon_memfile.h"

typedef uintptr_t ULONG_PTR;
typedef uint64_t u64;
typedef unsigned int Result;
typedef struct { void *addr; size_t size; } VirtmemReservation;

#define R_FAILED(value) ((value) != 0)
#define R_MODULE(value) ((value) & 0x1ff)
#define R_DESCRIPTION(value) ((value) >> 9)
#define Module_Kernel 1
#define KernelError_InvalidMemoryState 106
#define KernelError_ResourceExhausted 103
#define KernelError_OutOfMemory 104
#define WARN(...) ((void)0)
#define SECTION_NONE 0
#define SECTION_HOLE 1
#define SECTION_ALIASED 2
#define SECTION_ANCHOR 3

struct horizon_mapping
{
    void *addr, *anchor_source;
    size_t size, section_offset;
    struct horizon_memfile *section;
    VirtmemReservation *reservation;
    unsigned char section_state;
    int prot;
};

struct horizon_region { uint64_t addr, size; unsigned int type; };

static pthread_mutex_t mapping_mutex = PTHREAD_MUTEX_INITIALIZER;
static struct horizon_mapping *mappings[64];
static struct { char *start, *cursor, *end; } anchor_regions[1];
static unsigned int anchor_region_count = 1, section_anchors, section_view_maps;
static size_t section_anchor_bytes;
static unsigned int mapping_pool, reservation_count, collision_count;
static int fail_mapping_allocation, refuse_alias;
static char *arena, *arena_end;

static struct horizon_mapping *find_overlap_mapping( void *addr, size_t size )
{
    for (unsigned int i = 0; i < 64; i++)
        if (mappings[i] && host_ranges_overlap( addr, size, mappings[i]->addr, mappings[i]->size ))
            return mappings[i];
    return NULL;
}

static void list_add_mapping( struct horizon_mapping *mapping )
{
    unsigned int i;
    for (i = 0; i < 64 && mappings[i]; i++);
    assert( i < 64 );
    mappings[i] = mapping;
    mapping_pool++;
}

static void list_remove_mapping( struct horizon_mapping *mapping )
{
    unsigned int i;
    for (i = 0; i < 64 && mappings[i] != mapping; i++);
    assert( i < 64 );
    mappings[i] = NULL;
    mapping_pool--;
}

static struct horizon_mapping *alloc_mapping( void *addr, size_t size, void *backing,
                                              size_t offset, VirtmemReservation *reservation, int prot )
{
    (void)backing;
    (void)offset;
    if (fail_mapping_allocation) return NULL;
    struct horizon_mapping *mapping = calloc( 1, sizeof(*mapping) );
    assert( mapping );
    mapping->addr = addr;
    mapping->size = size;
    mapping->reservation = reservation;
    mapping->prot = prot;
    return mapping;
}

static void horizon_object_free( void *pool, void *object )
{
    assert( pool == &mapping_pool );
    free( object );
}

static void virtmemLock(void) {}
static void virtmemUnlock(void) {}

static VirtmemReservation *reserve_fixed_range_locked( void *addr, size_t size )
{
    VirtmemReservation *reservation = malloc( sizeof(*reservation) );
    assert( reservation );
    *reservation = (VirtmemReservation){addr, size};
    reservation_count++;
    return reservation;
}

static VirtmemReservation *reserve_fixed_range( void *addr, size_t size )
{
    return reserve_fixed_range_locked( addr, size );
}

static void remove_reservation( VirtmemReservation *reservation )
{
    assert( reservation && reservation_count );
    reservation_count--;
    free( reservation );
}

static void *virtmemFindCodeMemory( size_t size, size_t guard )
{
    (void)size;
    (void)guard;
    return NULL;
}

static int horizon_query_region( void *context, uint64_t address, struct horizon_region *region )
{
    (void)context;
    *region = (struct horizon_region){address, (uintptr_t)arena_end - address, 0};
    for (unsigned int i = 0; i < HOST_MAX_ANCHORS; i++)
    {
        if (!host_anchors[i].live) continue;
        uintptr_t start = (uintptr_t)host_anchors[i].addr;
        uintptr_t end = start + host_anchors[i].size;
        if (address >= start && address < end)
        {
            *region = (struct horizon_region){start, end - start, 9};
            return 1;
        }
        if (start > address && start - address < region->size) region->size = start - address;
    }
    return 1;
}

static void *find_anchor_address_locked( size_t size );

static int map_code_memory_range( void *addr, void *source, size_t size, int prot, int error )
{
    (void)prot;
    (void)error;
    return host_anchor( addr, source, size ) ? 0 : -1;
}

static int unmap_code_memory_range( void *addr, void *source, size_t size )
{
    return host_unanchor( addr, source, size );
}

static unsigned int envGetOwnProcessHandle(void) { return 1; }

static Result svcMapProcessMemory( void *dst, unsigned int handle, u64 src, size_t size )
{
    assert( handle == 1 );
    for (unsigned int i = 0; i < HOST_MAX_ANCHORS; i++)
        if (host_anchors[i].live && host_ranges_overlap( dst, size, host_anchors[i].addr, host_anchors[i].size ))
        {
            collision_count++;
            return 0xd401;
        }
    if (refuse_alias) return (Result)refuse_alias;
    return host_alias( dst, (void *)(uintptr_t)src, size ) ? 0xd401 : 0;
}

static Result svcUnmapProcessMemory( void *dst, unsigned int handle, u64 src, size_t size )
{
    assert( handle == 1 );
    return host_unalias( dst, (void *)(uintptr_t)src, size ) ? 0xd401 : 0;
}

static void section_failure( const char *stage, void *dst, void *src, size_t size, int error )
{
    (void)stage;
    (void)dst;
    (void)src;
    (void)size;
    (void)error;
}

static void horizon_trace( const char *format, ... ) { (void)format; }
static size_t page_align_size( size_t size ) { return (size + HOST_PAGE - 1) & ~(HOST_PAGE - 1); }
static int unmap_range_locked( void *addr, size_t size );

#include "section_mapping_impl.h"

static void *find_anchor_address_locked( size_t size ) { return find_anchor_run_locked( size ); }

static int unmap_range_locked( void *addr, size_t size )
{
    struct horizon_mapping *mapping = find_overlap_mapping( addr, size );
    if (!mapping) return 0;
    assert( mapping->addr == addr && mapping->size == size && mapping->section );
    if (mapping->section_state == SECTION_ALIASED)
        assert( !horizon_memfile_alias( mapping->section, addr, mapping->section_offset, size, 0 ) );
    list_remove_mapping( mapping );
    remove_reservation( mapping->reservation );
    free_section_range( mapping );
    return 0;
}

static const struct horizon_memfile_ops operations = {
    horizon_section_anchor, horizon_section_unanchor, horizon_section_alias, horizon_section_unalias
};

static void reset_anchor_cursor( char *target )
{
    anchor_regions[0].start = target + 0x21000;
    anchor_regions[0].cursor = anchor_regions[0].start;
    anchor_regions[0].end = arena_end;
}

int main(void)
{
    const size_t size = 0x160000;
    const int flags = MAP_SHARED | MAP_FIXED;
    arena = host_reserve( 8 * 1048576 );
    arena_end = arena + 8 * 1048576;
    char *target = arena + 1048576;
    reset_anchor_cursor( target );
    struct horizon_memfile *file = horizon_memfile_alloc( size, 0, &operations );
    assert( file );
    void *mapped = horizon_mmap_section( target, size, PROT_READ | PROT_WRITE, flags, file, 0 );
    if (mapped == MAP_FAILED)
    {
        fprintf( stderr, "FAIL pending section destination reused by anchor: collisions=%u errno=%d size=%#zx\n",
                 collision_count, errno, size );
        horizon_memfile_unref( file );
        assert( !mapping_pool && !reservation_count && !host_live_anchors() );
        return 1;
    }
    assert( mapped == target && !collision_count );
    mapped = horizon_mmap_section( arena + 4 * 1048576, size, PROT_READ | PROT_WRITE, flags, file, 0 );
    assert( mapped != MAP_FAILED );
    target[123] = 0x6d;
    assert( ((char *)mapped)[123] == 0x6d );
    horizon_memfile_unref( file );
    assert( !unmap_range_locked( target, size ) );
    assert( ((char *)mapped)[123] == 0x6d );
    assert( !unmap_range_locked( mapped, size ) );
    assert( !mapping_pool && !reservation_count && !host_live_anchors() && !host_pages_live );
    file = horizon_memfile_alloc( size, 0, &operations );
    assert( file );
    refuse_alias = 0xd401;
    reset_anchor_cursor( target );
    assert( horizon_mmap_section( target, size, PROT_READ | PROT_WRITE, flags, file, 0 ) == MAP_FAILED );
    assert( errno == EEXIST );
    assert( !mapping_pool && !reservation_count && !host_live_anchors() );
    refuse_alias = 0xd001;
    reset_anchor_cursor( target );
    assert( horizon_mmap_section( target, size, PROT_READ | PROT_WRITE, flags, file, 0 ) == MAP_FAILED );
    assert( errno == ENOMEM );
    assert( !mapping_pool && !reservation_count && !host_live_anchors() );
    refuse_alias = 0;
    fail_mapping_allocation = 1;
    assert( horizon_mmap_section( target, size, PROT_READ | PROT_WRITE, flags, file, 0 ) == MAP_FAILED );
    assert( !mapping_pool && !reservation_count && !host_live_anchors() );
    fail_mapping_allocation = 0;
    reset_anchor_cursor( target );
    assert( horizon_mmap_section( target, size, PROT_READ | PROT_WRITE, flags, file, 0 ) == target );
    assert( !unmap_range_locked( target, size ) );
    horizon_memfile_unref( file );
    assert( !mapping_pool && !reservation_count && !host_live_anchors() && !host_alias_count && !host_pages_live );
    file = horizon_memfile_alloc( size, 1, &operations );
    assert( file );
    assert( horizon_mmap_section( target, size, PROT_NONE, flags, file, 0 ) == target );
    assert( !host_live_anchors() );
    assert( !unmap_range_locked( target, size ) );
    horizon_memfile_unref( file );
    assert( !mapping_pool && !reservation_count && !host_pages_live );
    assert( !munmap( arena, 8 * 1048576 ) );
    puts( "PASS pending section placement, shared views, closed descriptor, rollback and reuse" );
    return 0;
}
