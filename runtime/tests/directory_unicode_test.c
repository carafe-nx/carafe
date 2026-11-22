#include <assert.h>
#include <dirent.h>
#include <errno.h>
#include <limits.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

typedef uint16_t WCHAR, USHORT;
typedef const WCHAR *LPCWSTR;
typedef WCHAR *LPWSTR;
typedef int BOOL, BOOLEAN;
typedef uint32_t DWORD, ULONG, NTSTATUS;
typedef struct { USHORT Length, MaximumLength; WCHAR *Buffer; } UNICODE_STRING;
#define TRUE 1
#define FALSE 0
#define WINAPI
#define STATUS_SUCCESS 0
#define STATUS_SOME_NOT_MAPPED 0x107u
#define STATUS_BUFFER_TOO_SMALL 0xc0000023u
#define STATUS_NO_SUCH_FILE 0xc000000fu
#define STATUS_NO_MORE_FILES 0x80000006u
#define HORIZON_STATUS_INVALID_PARAMETER 0xc000000du
#define HORIZON_STATUS_NO_MEMORY 0xc0000017u
#define HORIZON_STATUS_INVALID_HANDLE 0xc0000008u
#define HORIZON_STATUS_OBJECT_TYPE_MISMATCH 0xc0000024u
#define HORIZON_STATUS_INFO_LENGTH_MISMATCH 0xc0000004u
#define HORIZON_SERVER_OBJECT_FILE 1
#define HORIZON_SERVER_OBJECT_MAPPING 2
#define HORIZON_SECTION_MAP_READ 1
#define HORIZON_SECTION_MAP_WRITE 2
#define IS_HIGH_SURROGATE(c) (((c) & 0xfc00) == 0xd800)
#define IS_LOW_SURROGATE(c) (((c) & 0xfc00) == 0xdc00)
#define INVALID_NT_CHARS '*','?','<','>','|','"'
#define INVALID_DOS_CHARS INVALID_NT_CHARS,'+','=',',',';','[',']',' ','\345'
#define min(a,b) ((a)<(b)?(a):(b))
#define towupper(c) ntdll_towupper(c)
#define towlower(c) ntdll_towlower(c)
#define wcsupr(s) ntdll_wcsupr(s)
#define horizon_trace(...) do { if (0) fprintf(stderr, __VA_ARGS__); } while (0)
static USHORT *uctable, *lctable;
static const BOOL is_case_sensitive = FALSE;
static unsigned int wcslen(const WCHAR *s) { unsigned int n = 0; while (s[n]) ++n; return n; }
static const WCHAR *wcschr(const WCHAR *s, WCHAR c) { do { if (*s == c) return s; } while (*s++); return NULL; }
struct horizon_server_request_header { unsigned int reply_size; };
struct horizon_server_reply_header { unsigned int error, reply_size; };
struct horizon_query_directory_file_request { struct horizon_server_request_header header; unsigned int handle, restart_scan; };
struct horizon_query_directory_file_reply { struct horizon_server_reply_header header; unsigned int total_len; };
struct horizon_directory_file_entry { unsigned int name_len; };
struct horizon_pe_image_info { unsigned int machine, flags; uint64_t size; };
struct horizon_pe_version { unsigned int size; const void *data; };
struct horizon_get_mapping_info_request { struct horizon_server_request_header header; unsigned int handle, access; };
struct horizon_get_mapping_info_reply { struct horizon_server_reply_header header; uint64_t size; unsigned int flags, shared_file, total, ver_len, name_len; };
struct horizon_server_object {
    int type, file_is_dir;
    char *file_name;
    unsigned int dir_enum_index;
    int dir_queried;
    DIR *dir_stream;
    struct dirent *dir_pending;
    int dir_exhausted;
    WCHAR *dir_mask;
    unsigned int dir_mask_len;
    uint64_t mapping_size;
    unsigned int mapping_flags;
    int mapping_has_image;
    struct horizon_pe_image_info mapping_image;
    const struct horizon_pe_version *mapping_version;
    struct horizon_server_object *mapping_shared_file;
};
struct horizon_server_handle_entry { unsigned int handle; struct horizon_server_object *object; };
struct horizon_server_connection { int reply_fd; };
static struct horizon_server_handle_entry handles[3];
static pthread_mutex_t horizon_server_objects_mutex = PTHREAD_MUTEX_INITIALIZER;
static unsigned char captured_reply[256], captured_data[8192];
static unsigned int captured_size;
static struct horizon_server_handle_entry *horizon_server_find_handle_locked(unsigned int handle)
{
    for (unsigned int i = 0; i < 3; ++i) if (handles[i].handle == handle) return &handles[i];
    return NULL;
}
static struct horizon_server_handle_entry *horizon_server_create_handle_for_object_locked(struct horizon_server_object *object)
{
    (void)object;
    return NULL;
}
static unsigned int horizon_server_errno_status(int error) { return error == ENOENT ? STATUS_NO_SUCH_FILE : HORIZON_STATUS_INVALID_PARAMETER; }
static int horizon_dir_entry_is_resource_fork(const char *dir, const char *name) { (void)dir; (void)name; return 0; }
static void horizon_report_resource_fork(const char *dir, const char *name) { (void)dir; (void)name; }
static int horizon_server_write_reply(int fd, const void *reply, unsigned int size, const void *data, unsigned int data_size)
{
    (void)fd;
    assert(size <= sizeof(captured_reply) && data_size <= sizeof(captured_data));
    memcpy(captured_reply, reply, size);
    if (data_size) memcpy(captured_data, data, data_size);
    captured_size = data_size;
    return 0;
}
static int read_error;

static struct dirent *test_readdir(DIR *dir)
{
    if (read_error)
    {
        errno = read_error;
        read_error = 0;
        return NULL;
    }
    return readdir(dir);
}

#define readdir test_readdir
#include "directory_unicode_impl.h"
#undef readdir
#include "directory_unicode_vectors.h"

static unsigned int checks;
static void check(int condition) { ++checks; if (!condition) { fprintf(stderr, "FAIL check %u\n", checks); abort(); } }
static unsigned int query(unsigned int handle, const WCHAR *mask, int restart, unsigned int size)
{
    struct horizon_query_directory_file_request request = {{size}, handle, restart};
    struct horizon_server_connection connection = {0};
    horizon_server_handle_query_directory_file(&connection, (const void *)&request,
        (const void *)mask, mask ? wcslen(mask) * 2 : 0);
    return ((struct horizon_query_directory_file_reply *)captured_reply)->header.error;
}
static void touch(const char *dir, const char *name)
{
    char path[1024];
    snprintf(path, sizeof(path), "%s/%s", dir, name);
    FILE *file = fopen(path, "wb");
    assert(file);
    fclose(file);
}
static void check_name(const WCHAR *expected)
{
    struct horizon_directory_file_entry *entry = (void *)captured_data;
    unsigned int bytes = wcslen(expected) * 2;
    check(entry->name_len == bytes);
    check(!memcmp(entry + 1, expected, bytes));
    check(captured_size == ((sizeof(*entry) + bytes + 3) & ~3u));
}
static void test_codec(void)
{
    static const struct { const char *input; const WCHAR *output; } cases[] = {
        {"МАЯК.sav", u"МАЯК.sav"}, {"test.sav", u"test.sav"}, {"éЙ😀.sav", u"éЙ😀.sav"},
        {"\xff", u"\xfffd"}, {"\xc0\xaf", u"\xfffd\xfffd"}, {"\xe2\x82", u"\xfffd"},
        {"\xed\xa0\x80", u"\xfffd\xfffd"}, {"", u""}
    };
    for (unsigned int i = 0; i < sizeof(cases)/sizeof(*cases); ++i)
    {
        WCHAR buffer[128];
        memset(buffer, 0xab, sizeof(buffer));
        unsigned int bytes = horizon_server_utf16_name_len(cases[i].input);
        check(bytes == 2*wcslen(cases[i].output));
        horizon_server_write_utf16_name((void *)buffer, cases[i].input);
        check(!memcmp(buffer, cases[i].output, bytes));
        check(buffer[bytes/2] == 0xabab);
    }
    WCHAR lone[] = {0xd800};
    char encoded[8];
    DWORD bytes;
    check(RtlUnicodeToUTF8N(encoded, sizeof(encoded), &bytes, lone, sizeof(lone)) == STATUS_SOME_NOT_MAPPED);
    check(bytes == 3 && !memcmp(encoded, "\xef\xbf\xbd", 3));
    WCHAR pair[] = {0xd83d, 0xde00};
    check(horizon_directory_name_matches(pair, 2, u"??", 4));
    check(!horizon_directory_name_matches(pair, 2, u"?", 2));
    check(horizon_directory_name_matches(lone, 1, lone, 2));
    check(!horizon_directory_name_matches(lone, 1, u"\xfffd", 2));
}
static void test_vectors(void)
{
    for (unsigned int i = 0; i < sizeof(vectors)/sizeof(*vectors); ++i)
        for (unsigned int j = 0; j < 18; ++j)
        {
            int actual = horizon_directory_name_matches(vector_names[j], wcslen(vector_names[j]),
                vectors[i].mask, 2*wcslen(vectors[i].mask));
            if (!!actual != vectors[i].found[j]) fprintf(stderr, "vector %u name %u\n", i, j);
            check(!!actual == vectors[i].found[j]);
        }
    check(horizon_directory_name_matches(u"МАЯК.sav", 8, u"маяк.SAV", 16));
    check(horizon_directory_name_matches(u"éЙ.sav", 6, u"Éй.SAV", 12));
    check(!horizon_directory_name_matches(u"ABCD.sav", 8, u"МАЯК.sav", 16));
    WCHAR short_name[13];
    unsigned int length = horizon_directory_short_name(u"longfilename.tmp", 16, short_name);
    check(length != 0);
    check(horizon_directory_name_matches(u"longfilename.tmp", 16, short_name, length*2));
    check(horizon_directory_short_name(u"test.sav", 8, short_name) == 0);
}
static void test_directory(void)
{
    char dir[] = "/tmp/carafe-directory-XXXXXX";
    assert(mkdtemp(dir));
    touch(dir, "МАЯК.sav"); touch(dir, "ABCD.sav"); touch(dir, "test.sav"); touch(dir, "😀.sav");
    struct horizon_server_object first = {.type=HORIZON_SERVER_OBJECT_FILE, .file_is_dir=1, .file_name=dir};
    struct horizon_server_object second = first;
    handles[0] = (struct horizon_server_handle_entry){1, &first};
    handles[1] = (struct horizon_server_handle_entry){2, &second};
    check(query(1, u"МАЯК.sav", 1, 4096) == 0); check_name(u"МАЯК.sav");
    check(query(1, u"ABCD.sav", 0, 4096) == STATUS_NO_MORE_FILES);
    check(query(1, NULL, 1, 4096) == 0); check_name(u"МАЯК.sav");
    check(query(2, u"missing", 1, 4096) == STATUS_NO_SUCH_FILE);
    check(query(2, u"test.sav", 0, 4096) == STATUS_NO_MORE_FILES);
    check(query(2, NULL, 1, 4096) == STATUS_NO_MORE_FILES);
    check(query(1, u"test.sav", 1, 4096) == 0); check_name(u"test.sav");
    check(query(2, u"маяк.SAV", 1, 4096) == 0); check_name(u"МАЯК.sav");
    read_error = EINTR;
    check(query(1, u"😀.sav", 1, 4096) == horizon_server_errno_status(EINTR));
    check(!read_error);
    check(query(1, NULL, 0, 7) == HORIZON_STATUS_INFO_LENGTH_MISMATCH);
    check(captured_size == 0);
    check(query(1, NULL, 0, 4096) == 0); check_name(u"😀.sav");
    check(query(1, u"*.sav", 1, 4096) == 0);
    unsigned int count = 1;
    while (!query(1, NULL, 0, 4096)) ++count;
    check(count == 4);
    check(query(1, NULL, 0, 4096) == STATUS_NO_MORE_FILES);
    check(query(1, u"*", 1, 4096) == 0); check_name(u".");
    check(query(1, NULL, 0, 4096) == 0); check_name(u"..");
    struct horizon_query_directory_file_request request = {{4096}, 1, 1};
    struct horizon_server_connection connection = {0};
    horizon_server_handle_query_directory_file(&connection, (const void *)&request, (const void *)u"a", 1);
    check(((struct horizon_query_directory_file_reply *)captured_reply)->header.error == HORIZON_STATUS_INVALID_PARAMETER);
    check(query(999, u"*", 1, 4096) == HORIZON_STATUS_INVALID_HANDLE);
    free(first.dir_mask); free(second.dir_mask);
    const char *names[] = {"МАЯК.sav", "ABCD.sav", "test.sav", "😀.sav"};
    for (unsigned int i=0; i<4; ++i) { char path[1024]; snprintf(path,sizeof(path),"%s/%s",dir,names[i]); unlink(path); }
    rmdir(dir);
}
static void test_image(void)
{
    static const unsigned char version_data[] = {1,2,3,4};
    struct horizon_pe_version version = {sizeof(version_data), version_data};
    struct horizon_server_object image = {.type=HORIZON_SERVER_OBJECT_MAPPING, .file_name="/games/МАЯК😀.exe",
        .mapping_has_image=1, .mapping_version=&version};
    handles[2] = (struct horizon_server_handle_entry){3, &image};
    unsigned int bytes = 2*wcslen(u"/games/МАЯК😀.exe");
    unsigned int total = sizeof(struct horizon_pe_image_info) + sizeof(version_data) + bytes;
    unsigned int sizes[] = {total, total-1, sizeof(struct horizon_pe_image_info), 0};
    for (unsigned int i=0; i<4; ++i)
    {
        struct horizon_get_mapping_info_request request = {{sizes[i]},3,0};
        struct horizon_server_connection connection = {0};
        horizon_server_handle_get_mapping_info(&connection,(const void *)&request);
        struct horizon_get_mapping_info_reply *reply = (void *)captured_reply;
        check(reply->header.error == 0 && reply->total == total);
        check(reply->name_len == (i == 0 ? bytes : 0));
        check(captured_size == (i == 0 ? total : i == 1 ? sizeof(struct horizon_pe_image_info)+4 : i == 2 ? sizeof(struct horizon_pe_image_info) : 0));
        if (!i) check(!memcmp(captured_data + sizeof(struct horizon_pe_image_info)+4, u"/games/МАЯК😀.exe", bytes));
    }
}
int main(int argc, char **argv)
{
    assert(argc == 2);
    FILE *file = fopen(argv[1], "rb"); assert(file);
    fseek(file,0,SEEK_END); long size=ftell(file); rewind(file);
    USHORT *table = malloc(size); assert(table);
    assert(fread(table,1,size,file) == (size_t)size); fclose(file);
    uctable=table+2; lctable=table+table[1]+2;
    test_codec(); test_vectors(); test_directory(); test_image();
    printf("PASS %u checks: real Wine DOS/Unicode/short-name functions, Horizon directory and image handlers\n", checks);
    free(table);
    return 0;
}
