#include <stdbool.h>
#include <ctype.h>
#include <dirent.h>
#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#define PATH_SIZE 768
#define WINE_ROOT "/switch/wine"
#define USERS_DIR "/drive_c/users/"
#define DXVK_CACHE_DIR "/AppData/Local/dxvk"
#define FEX_CACHE_DIR "/AppData/Local/fex-emu"
#define RW_DEVICE "carafe-rw"
#define RO_DEVICE "carafe-ro"
typedef enum { RouteBase,RouteVolatile,RouteUnion } Route;
typedef struct { Route route; char base[PATH_SIZE],rw[PATH_SIZE],ro[PATH_SIZE]; } Resolved;
struct _reent { int _errno; };
static struct _reent context;
#define _REENT (&context)
static bool g_installed=true;
static const char g_volatile_root[]="/switch/carafe/title";
static int g_base,g_rw,g_ro;
static bool startsWith(const char *s,const char *p) { return !strncmp(s,p,strlen(p)); }
static bool format(char *s,size_t n,const char *f,...) {
    va_list args; va_start(args,f); int count=vsnprintf(s,n,f,args); va_end(args);
    return count>=0 && (size_t)count<n;
}
static bool canonicalize(struct _reent *r,const char *p,char *out,size_t size) { (void)r; return format(out,size,"%s",p); }
static int devStat(struct _reent *r,int device,const char *path,struct stat *st) {
    (void)device;
    if (!strcmp(path,"sdmc:/switch/carafe/title/logs/A~1.SAV")) path="sdmc:/switch/carafe/title/logs/a.sav";
    int rc=stat(path,st); if(rc) r->_errno=errno; return rc;
}
static int fixtureStat(const char *path,struct stat *st) {
    if (!strcmp(path,"ums0:/SAVE/a.sav") || !strcmp(path,"ums0:/Save/A.SAV")) path="ums0:/Save/a.sav";
    return stat(path,st);
}
#define stat(path,st) fixtureStat(path,st)
@@
typedef uint32_t DWORD;
typedef int (*Progress)(void);
typedef struct { DWORD dwCopyFlags; BOOL *pfCancel; Progress pProgressRoutine; } COPYFILE2_EXTENDED_PARAMETERS;
typedef struct { DWORD FileAttributes; } FILE_BASIC_INFORMATION;
typedef Progress PCOPYFILE2_PROGRESS_ROUTINE;
#define TRACE(...) ((void)0)
#define WARN(...) ((void)0)
#define FIXME(...) ((void)0)
enum { ERROR_INVALID_PARAMETER=87,ERROR_NOT_ENOUGH_MEMORY=8,ERROR_FILE_NOT_FOUND=2,ERROR_PATH_NOT_FOUND=3,
       ERROR_SHARING_VIOLATION=32,GENERIC_READ=1,GENERIC_WRITE=2,FILE_SHARE_READ=1,FILE_SHARE_WRITE=2,FILE_SHARE_DELETE=4,
       OPEN_EXISTING=3,CREATE_ALWAYS=2,CREATE_NEW=1,FILE_ATTRIBUTE_READONLY=1,FileBasicInformation=4,
       COPY_FILE_FAIL_IF_EXISTS=1,COPY_FILE_RESTARTABLE=2,COPY_FILE_COPY_SYMLINK=4,COPY_FILE_OPEN_SOURCE_FOR_WRITE=8 };
#define INVALID_HANDLE_VALUE (-1)
static DWORD last_error;
static int creates, expected_dest;
static FILE *bytes;
static void SetLastError(DWORD e) { last_error=e; }
static DWORD GetLastError(void) { return last_error; }
static DWORD RtlNtStatusToDosError(NTSTATUS s) { return s; }
static int GetProcessHeap(void) { return 0; }
#define HeapAlloc(h,f,n) ((void)(h),(void)(f),malloc(n))
#define HeapFree(h,f,p) ((void)(h),(void)(f),free(p))
static HANDLE CreateFileW(const WCHAR *p,DWORD access,DWORD sharing,void *sa,DWORD disp,DWORD attrs,HANDLE tmpl) {
    (void)access;(void)sharing;(void)sa;(void)attrs;(void)tmpl;
    if (disp==CREATE_ALWAYS) { assert(*p==expected_dest); ++creates; assert(!ftruncate(fileno(bytes),0)); }
    return *p;
}
static BOOL CloseHandle(HANDLE h) { (void)h; return TRUE; }
static BOOL set_ntstatus(NTSTATUS s) { SetLastError(s); return !s; }
static NTSTATUS NtQueryInformationFile(HANDLE h,IO_STATUS_BLOCK *io,FILE_BASIC_INFORMATION *info,DWORD n,int cls) {
    (void)h;(void)io;(void)n; assert(cls==FileBasicInformation); info->FileAttributes=0; return 0;
}
#define NtSetInformationFile(...) ((void)0)
static BOOL ReadFile(HANDLE h,void *buf,DWORD len,DWORD *count,void *overlap) {
    (void)h;(void)buf;(void)len;(void)overlap; *count=0; return TRUE;
}
static BOOL WriteFile(HANDLE h,const void *buf,DWORD len,DWORD *count,void *overlap) {
    (void)h;(void)buf;(void)len;(void)count;(void)overlap; abort();
}
static BOOL copy_file(const WCHAR *,const WCHAR *,COPYFILE2_EXTENDED_PARAMETERS *);
static void check_copy(WCHAR source,WCHAR dest,DWORD error) {
    bytes=tmpfile(); assert(bytes && fwrite("source",1,6,bytes)==6 && !fflush(bytes));
    creates=0; expected_dest=dest;
    assert(copy_file(&source,&dest,NULL)==!error && last_error==error && creates==!error);
    if (error) { char payload[6]; rewind(bytes); assert(fread(payload,1,6,bytes)==6 && !memcmp(payload,"source",6)); }
    assert(!fclose(bytes));
}
