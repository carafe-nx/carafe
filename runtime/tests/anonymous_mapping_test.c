#include <stdio.h>
#include <pthread.h>
#include "../../external/autorun/horizon-wine/tests/horizon_memfile_host.h"
#include "../../external/autorun/horizon-wine/source/backing_heap.h"

#define MiB ((size_t)1048576)
#define min(a, b) ((a) < (b) ? (a) : (b))
#define max(a, b) ((a) > (b) ? (a) : (b))

static struct backing_heap physical_heap;
static struct backing_block physical_blocks[768];
static void *physical_start;

static void *physical_alloc( size_t alignment, size_t size )
{
    void *pointer = backing_alloc( &physical_heap, (uintptr_t)physical_start, alignment, size );
    if (!pointer) errno = ENOMEM;
    return pointer;
}

static void physical_free( void *pointer )
{
    if ((uintptr_t)pointer >= (uintptr_t)physical_start &&
        (uintptr_t)pointer < (uintptr_t)physical_start + 48 * MiB)
        backing_free( &physical_heap, pointer );
    else free( pointer );
}

#define aligned_alloc physical_alloc
#define free physical_free
#include "../../external/autorun/dlls/ntdll/unix/horizon_pool.h"
#undef aligned_alloc
#undef free

struct test_mapping
{
    void *address, *source;
    size_t size;
};

static struct horizon_page_pool pages;
static struct test_mapping mappings[64];
static size_t mapping_count;
static int refused_after = -1;

static int map_backing_at( void *address, size_t size, int prot, int fd, off_t offset, int flags, int map_errno )
{
    void *source;
    (void)prot;
    (void)offset;
    (void)flags;
    (void)map_errno;
    assert( fd == -1 );
    if (refused_after >= 0 && mapping_count == (size_t)refused_after)
    {
        errno = EACCES;
        return -1;
    }
    if (!(source = horizon_pages_alloc_any( &pages, size ))) return -1;
    memset( source, 0, size );
    if (!host_anchor( address, source, size ))
    {
        if (!horizon_pages_free( &pages, source, size )) physical_free( source );
        return -1;
    }
    assert( mapping_count < 64 );
    mappings[mapping_count++] = (struct test_mapping){ address, source, size };
    return 0;
}

static int unmap_range_locked( void *address, size_t size )
{
    size_t i;
    for (i = 0; i < mapping_count; i++)
    {
        struct test_mapping *mapping = &mappings[i];
        assert( (char *)mapping->address >= (char *)address );
        assert( (char *)mapping->address + mapping->size <= (char *)address + size );
        assert( !host_unanchor( mapping->address, mapping->source, mapping->size ) );
        if (!horizon_pages_free( &pages, mapping->source, mapping->size )) physical_free( mapping->source );
    }
    mapping_count = 0;
    return 0;
}

static void horizon_trace( const char *format, ... )
{
    (void)format;
    abort();
}

#include "anonymous_mapping_impl.h"

static void pooled_free( void *pointer, size_t size )
{
    if (!horizon_pages_free( &pages, pointer, size )) physical_free( pointer );
}

#undef HORIZON_MEMFILE_ALLOC_PAGES
#undef HORIZON_MEMFILE_FREE_PAGES
#define HORIZON_MEMFILE_ALLOC_PAGES(size) horizon_pages_alloc_any( &pages, size )
#define HORIZON_MEMFILE_FREE_PAGES(pointer, size) pooled_free( pointer, size )
#include "horizon_memfile.h"

static void *section_anchor( void *source, size_t size, void **token )
{
    *token = NULL;
    return host_anchor( NULL, source, size );
}

static int section_unanchor( void *address, void *source, size_t size, void *token )
{
    (void)token;
    return host_unanchor( address, source, size );
}

static const struct horizon_memfile_ops section_ops = {
    section_anchor, section_unanchor, host_alias, host_unalias
};

int main(void)
{
    void *pool = host_alloc_pages( 64 * MiB ), *held[768];
    unsigned char *view;
    size_t original_used, i;
    const size_t request = 2 * MiB + BACKING_UNIT;
    assert( pool );
    physical_start = (void *)(((uintptr_t)pool + 2 * MiB - 1) & ~(uintptr_t)(2 * MiB - 1));
    backing_init( &physical_heap, physical_blocks, (uintptr_t)physical_start, 768 );
    for (i = 0; i < 96; i++) assert( (held[i] = physical_alloc( BACKING_UNIT, 512 * 1024 )) );
    for (i = 0; i < 96; i += 2) physical_free( held[i] );
    original_used = physical_heap.used;
    assert( original_used == 24 * MiB && backing_largest( &physical_heap ) == 512 * 1024 );
    view = host_reserve( request );
    if (map_anonymous_backings( view, request, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, ENOMEM ))
    {
        fprintf( stderr, "FAIL anonymous=2112KiB free=24MiB largest=512KiB errno=%d\n", errno );
        return 1;
    }
    assert( mapping_count > 1 );
    for (i = 0; i < request; i++) assert( !view[i] );
    memset( view, 0x4d, request );
    for (i = 0; i < mapping_count; i++)
        assert( ((unsigned char *)mappings[i].address)[mappings[i].size - 1] == 0x4d );
    assert( !unmap_range_locked( view, request ) );
    assert( physical_heap.used == original_used && !host_live_anchors() );
    refused_after = 2;
    assert( map_anonymous_backings( view, request, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, ENOMEM ) == -1 );
    assert( errno == EACCES && !mapping_count && !host_live_anchors() );
    assert( physical_heap.used == original_used );
    refused_after = -1;
    assert( !munmap( view, request ) );
    for (i = 1; i < 96; i += 2) physical_free( held[i] );
    assert( !physical_heap.used );

    for (i = 0; i < 768; i++) assert( (held[i] = physical_alloc( BACKING_UNIT, BACKING_UNIT )) );
    for (i = 0; i < 768; i += 2) physical_free( held[i] );
    original_used = physical_heap.used;
    assert( backing_largest( &physical_heap ) == BACKING_UNIT );
    view = host_reserve( request );
    assert( !map_anonymous_backings( view, request, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, ENOMEM ) );
    assert( mapping_count == request / BACKING_UNIT );
    memset( view, 0x72, request );
    assert( !unmap_range_locked( view, request ) );
    assert( physical_heap.used == original_used && !host_live_anchors() );
    assert( !munmap( view, request ) );
    for (i = 1; i < 768; i += 2) physical_free( held[i] );
    assert( !physical_heap.used );
    horizon_pages_trim( &pages );
    void *pooled[HORIZON_POOL_PAGES];
    size_t held_count = 0;
    for (i = 0; i < HORIZON_POOL_PAGES; i++)
    {
        assert( (pooled[i] = horizon_pages_alloc( &pages, HORIZON_POOL_PAGE )) );
        memset( pooled[i], 0x31, HORIZON_POOL_PAGE );
    }
    while ((held[held_count] = physical_alloc( BACKING_UNIT, BACKING_UNIT ))) held_count++;
    for (i = 0; i < HORIZON_POOL_PAGES; i += 2)
        assert( horizon_pages_free( &pages, pooled[i], HORIZON_POOL_PAGE ) );
    original_used = physical_heap.used;
    assert( original_used == 48 * MiB );
    view = host_reserve( BACKING_UNIT );
    if (map_anonymous_backings( view, BACKING_UNIT, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, ENOMEM ))
    {
        fprintf( stderr, "FAIL anonymous=64KiB native_free=0 pool_free=1MiB largest=4KiB errno=%d\n", errno );
        return 1;
    }
    assert( mapping_count == BACKING_UNIT / HORIZON_POOL_PAGE );
    for (i = 0; i < BACKING_UNIT; i++) assert( !view[i] );
    memset( view, 0x65, BACKING_UNIT );
    assert( !unmap_range_locked( view, BACKING_UNIT ) && !host_live_anchors() );
    assert( !munmap( view, BACKING_UNIT ) );
    struct horizon_memfile *file = horizon_memfile_alloc( BACKING_UNIT, 0, &section_ops );
    if (!file)
    {
        fprintf( stderr, "FAIL section=64KiB native_free=0 pool_free=1MiB largest=4KiB errno=%d\n", errno );
        return 1;
    }
    const char bytes[8] = "carafe";
    assert( horizon_memfile_pwrite( file, bytes, sizeof(bytes), HORIZON_POOL_PAGE - 4 ) == sizeof(bytes) );
    view = host_reserve( BACKING_UNIT );
    horizon_memfile_ref( file );
    assert( !horizon_memfile_map( file, view, 0, BACKING_UNIT ) );
    horizon_memfile_unref( file );
    assert( !memcmp( view + HORIZON_POOL_PAGE - 4, bytes, sizeof(bytes) ) );
    assert( !horizon_memfile_alias( file, view, 0, BACKING_UNIT, 0 ) );
    horizon_memfile_use( file, 0, BACKING_UNIT, -1 );
    horizon_memfile_unref( file );
    assert( !munmap( view, BACKING_UNIT ) && !host_live_anchors() && !host_alias_count );
    for (i = 1; i < HORIZON_POOL_PAGES; i += 2)
    {
        assert( ((unsigned char *)pooled[i])[0] == 0x31 );
        assert( horizon_pages_free( &pages, pooled[i], HORIZON_POOL_PAGE ) );
    }
    horizon_pages_trim( &pages );
    for (i = 0; i < held_count; i++) physical_free( held[i] );
    assert( !physical_heap.used );
    host_free_pages( pool, 64 * MiB );
    assert( !host_pages_live );
    puts( "anonymous memory: 512 KiB/64 KiB backing holes, 4 KiB pool holes, guest range and rollback passed" );
    return 0;
}
