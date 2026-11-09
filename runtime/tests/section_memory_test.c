#include <stdio.h>
#include <pthread.h>
#include "../../external/autorun/horizon-wine/tests/horizon_memfile_host.h"
#include "../../external/autorun/horizon-wine/source/backing_heap.h"

#define MiB ((size_t)1048576)
static struct backing_heap physical_heap;
static struct backing_block physical_blocks[768];
static void *physical_start;

static void *section_alloc( size_t size )
{
    void *pointer = backing_alloc( &physical_heap, (uintptr_t)physical_start, HOST_PAGE, size );
    if (!pointer) errno = ENOMEM;
    return pointer;
}

static void section_free( void *pointer, size_t size )
{
    assert( backing_size( &physical_heap, pointer ) >= size );
    backing_free( &physical_heap, pointer );
}

#undef HORIZON_MEMFILE_ALLOC_PAGES
#undef HORIZON_MEMFILE_FREE_PAGES
#define HORIZON_MEMFILE_ALLOC_PAGES(size) section_alloc( size )
#define HORIZON_MEMFILE_FREE_PAGES(ptr, size) section_free( ptr, size )
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

static const struct horizon_memfile_ops operations = {
    section_anchor, section_unanchor, host_alias, host_unalias
};

static void release_view( struct horizon_memfile *file, void *view, size_t offset, size_t size )
{
    assert( !horizon_memfile_alias( file, view, offset, size, 0 ) );
    horizon_memfile_use( file, offset, size, -1 );
    horizon_memfile_unref( file );
    assert( !munmap( view, size ) );
}

int main(void)
{
    void *pool = host_alloc_pages( 64 * MiB ), *held[6];
    struct horizon_memfile *file;
    unsigned char bytes[512], result[512], *first, *second;
    size_t original_used, i;

    assert( pool );
    physical_start = (void *)(((uintptr_t)pool + BACKING_UNIT - 1) & ~(uintptr_t)(BACKING_UNIT - 1));
    backing_init( &physical_heap, physical_blocks, (uintptr_t)physical_start, 768 );
    for (i = 0; i < 6; i++) assert( (held[i] = section_alloc( 8 * MiB )) );
    for (i = 0; i < 6; i += 2) section_free( held[i], 8 * MiB );
    original_used = physical_heap.used;
    assert( original_used == 24 * MiB && backing_largest( &physical_heap ) == 8 * MiB );
    file = horizon_memfile_alloc( 16 * MiB, 0, &operations );
    if (!file)
    {
        fprintf( stderr, "FAIL section=16MiB free=24MiB largest=8MiB errno=%d\n", errno );
        return 1;
    }
    assert( physical_heap.used == original_used + 16 * MiB );
    memset( bytes, 0x37, sizeof(bytes) );
    assert( horizon_memfile_pwrite( file, (char *)bytes, sizeof(bytes), 2 * MiB - 256 ) == sizeof(bytes) );
    assert( horizon_memfile_pread( file, (char *)result, sizeof(result), 2 * MiB - 256 ) == sizeof(result) );
    assert( !memcmp( bytes, result, sizeof(bytes) ) );

    first = host_reserve( 8 * MiB );
    second = host_reserve( 8 * MiB );
    horizon_memfile_ref( file );
    horizon_memfile_ref( file );
    assert( !horizon_memfile_map( file, first, 0, 8 * MiB ) );
    assert( !horizon_memfile_map( file, second, 4 * MiB, 8 * MiB ) );
    assert( !memcmp( first + 2 * MiB - 256, bytes, sizeof(bytes) ) );
    first[4 * MiB + 128] = 0x6d;
    assert( second[128] == 0x6d );
    memset( bytes, 0x53, sizeof(bytes) );
    assert( horizon_memfile_pwrite( file, (char *)bytes, sizeof(bytes), 6 * MiB - 256 ) == sizeof(bytes) );
    assert( !memcmp( second + 2 * MiB - 256, bytes, sizeof(bytes) ) );
    horizon_memfile_unref( file );
    release_view( file, first, 0, 8 * MiB );
    assert( second[128] == 0x6d );
    release_view( file, second, 4 * MiB, 8 * MiB );
    assert( !host_alias_count && !host_live_anchors() && physical_heap.used == original_used );

    assert( !horizon_memfile_alloc( 32 * MiB, 0, &operations ) && errno == ENOMEM );
    assert( physical_heap.used == original_used );
    file = horizon_memfile_alloc( 16 * MiB, 0, &operations );
    assert( file );
    first = host_reserve( 8 * MiB );
    host_fail_alias_countdown = 2;
    assert( horizon_memfile_map( file, first, 0, 8 * MiB ) == -1 );
    host_fail_alias_countdown = 0;
    assert( !host_alias_count && !host_live_anchors() );
    horizon_memfile_unref( file );
    assert( !munmap( first, 8 * MiB ) && physical_heap.used == original_used );

    file = horizon_memfile_alloc( 16 * MiB, 1, &operations );
    assert( file );
    assert( !horizon_memfile_add_committed( file, 0, 16 * MiB, 2 * MiB - HOST_PAGE, 2 * HOST_PAGE ) );
    unsigned long long range;
    int committed;
    assert( !horizon_memfile_committed_range( file, 0, 16 * MiB, 2 * MiB - HOST_PAGE, &range, &committed ) );
    assert( committed && range == 2 * HOST_PAGE );
    horizon_memfile_unref( file );
    assert( physical_heap.used == original_used );
    for (i = 1; i < 6; i += 2) section_free( held[i], 8 * MiB );
    assert( !physical_heap.used );

    void *small[96];
    const size_t small_size = 512 * 1024, tail_size = 16 * MiB + HOST_PAGE;
    for (i = 0; i < 96; i++) assert( (small[i] = section_alloc( small_size )) );
    for (i = 0; i < 96; i += 2) section_free( small[i], small_size );
    original_used = physical_heap.used;
    assert( backing_largest( &physical_heap ) == small_size );
    file = horizon_memfile_alloc( tail_size, 0, &operations );
    assert( file );
    assert( physical_heap.used == original_used + 16 * MiB + BACKING_UNIT );
    memset( bytes, 0x29, sizeof(bytes) );
    assert( horizon_memfile_pwrite( file, (char *)bytes, sizeof(bytes), small_size - 256 ) == sizeof(bytes) );
    assert( horizon_memfile_pread( file, (char *)result, sizeof(result), small_size - 256 ) == sizeof(result) );
    assert( !memcmp( bytes, result, sizeof(bytes) ) );
    assert( horizon_memfile_pwrite( file, (char *)bytes, sizeof(bytes), tail_size - 256 ) == 256 );
    first = host_reserve( 2 * MiB );
    assert( !horizon_memfile_map( file, first, 0, 2 * MiB ) );
    assert( !memcmp( first + small_size - 256, bytes, sizeof(bytes) ) );
    assert( !horizon_memfile_alias( file, first, 0, 2 * MiB, 0 ) );
    horizon_memfile_use( file, 0, 2 * MiB, -1 );
    assert( !munmap( first, 2 * MiB ) );
    horizon_memfile_unref( file );
    assert( physical_heap.used == original_used && !host_alias_count && !host_live_anchors() );
    for (i = 1; i < 96; i += 2) section_free( small[i], small_size );
    assert( !physical_heap.used );
    for (i = 0; i < 96; i++) assert( (small[i] = section_alloc( small_size )) );
    for (i = 0; i < 4; i++) section_free( small[i], small_size );
    section_free( small[8], small_size );
    section_free( small[10], small_size );
    original_used = physical_heap.used;
    assert( backing_largest( &physical_heap ) == 2 * MiB );
    file = horizon_memfile_alloc( 3 * MiB, 0, &operations );
    assert( file );
    memset( bytes, 0x61, sizeof(bytes) );
    assert( horizon_memfile_pwrite( file, (char *)bytes, sizeof(bytes), 2 * MiB - 256 ) == sizeof(bytes) );
    first = host_reserve( 3 * MiB );
    second = host_reserve( MiB );
    horizon_memfile_ref( file );
    horizon_memfile_ref( file );
    assert( !horizon_memfile_map( file, first, 0, 3 * MiB ) );
    assert( !horizon_memfile_map( file, second, 2 * MiB, MiB ) );
    horizon_memfile_unref( file );
    assert( !memcmp( first + 2 * MiB - 256, bytes, sizeof(bytes) ) );
    first[2 * MiB + small_size - 1] = 0x76;
    assert( second[small_size - 1] == 0x76 );
    release_view( file, first, 0, 3 * MiB );
    assert( second[small_size - 1] == 0x76 );
    release_view( file, second, 2 * MiB, MiB );
    assert( physical_heap.used == original_used && !host_alias_count && !host_live_anchors() );
    for (i = 4; i < 96; i++)
        if (i != 8 && i != 10) section_free( small[i], small_size );
    assert( !physical_heap.used );
    host_free_pages( pool, 64 * MiB );
    assert( !host_pages_live );
    puts( "section memory: fragmented allocation, shared views, lifetime and rollback passed" );
    return 0;
}
