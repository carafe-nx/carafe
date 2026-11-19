#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include "ntstatus.h"
#define WIN32_NO_STATUS
#include "windef.h"
#include "winternl.h"
#define IS_HIGH_SURROGATE(c) (((c) & 0xfc00) == 0xd800)
#define IS_LOW_SURROGATE(c) (((c) & 0xfc00) == 0xdc00)
#define INVALID_NT_CHARS '*','?','<','>','|','"'
#define INVALID_DOS_CHARS INVALID_NT_CHARS,'+','=',',',';','[',']',' ','\345'
#define MAX_DIR_ENTRY_LEN 255
#define TRACE(...) ((void)0)
static USHORT *uctable, *lctable;
static const BOOL is_case_sensitive = FALSE;
static unsigned int wide_length(const WCHAR *s) { unsigned int n=0; while(s[n]) ++n; return n; }
static const WCHAR *wide_find(const WCHAR *s,WCHAR c) { do { if(*s==c) return s; }while(*s++); return NULL; }
#define wcslen(s) wide_length(s)
#define wcschr(s,c) wide_find(s,c)
#define towupper(c) ntdll_towupper(c)
#define towlower(c) ntdll_towlower(c)
#define wcsupr(s) ntdll_wcsupr(s)
union file_directory_info {
    ULONG next;
    FILE_DIRECTORY_INFORMATION dir;
    FILE_BOTH_DIRECTORY_INFORMATION both;
    FILE_FULL_DIRECTORY_INFORMATION full;
    FILE_ID_BOTH_DIRECTORY_INFORMATION id_both;
    FILE_ID_EXTD_BOTH_DIRECTORY_INFORMATION extd_both;
    FILE_ID_FULL_DIRECTORY_INFORMATION id_full;
    FILE_ID_GLOBAL_TX_DIR_INFORMATION id_tx;
    FILE_NAMES_INFORMATION names;
};
struct dir_data_names { const WCHAR *long_name, *short_name; const char *unix_name; };
struct dir_data { unsigned int size,count,pos; struct { dev_t dev; ino_t ino; } id; struct dir_data_names *names; };
struct directory_file_entry { unsigned int name_len; };
static const WCHAR *returned_name;
static char *directory;
static NTSTATUS server_status;
static unsigned int request_count, seen_mask_bytes, seen_restart;
struct fake_request { unsigned int handle, restart_scan; };
static struct fake_request active_request;
static void *reply_buffer;
#define SERVER_START_REQ(name) do { struct fake_request *req=&active_request;
#define SERVER_END_REQ } while(0)
static unsigned int wine_server_obj_handle(HANDLE handle) { return (uintptr_t)handle; }
static void wine_server_add_data(struct fake_request *req,const void *data,unsigned int size) { (void)req; (void)data; seen_mask_bytes=size; }
static void wine_server_set_reply(struct fake_request *req,void *buffer,unsigned int size) { (void)req; (void)size; reply_buffer=buffer; }
static NTSTATUS wine_server_call(struct fake_request *req)
{
    ++request_count; seen_restart=req->restart_scan;
    if(server_status) return server_status;
    struct directory_file_entry *entry=reply_buffer;
    entry->name_len=wcslen(returned_name)*2;
    memcpy(entry+1,returned_name,entry->name_len);
    return STATUS_SUCCESS;
}
static NTSTATUS server_get_unix_name(HANDLE handle,char **name) { (void)handle; *name=strdup(directory); return STATUS_SUCCESS; }
static int get_file_info(const char *path,struct stat *st,ULONG *attributes,ULONG *tag) { *attributes=0; *tag=0; return stat(path,st); }
static int is_ignored_file(const struct stat *st) { (void)st; return 0; }
static void fill_file_info(const struct stat *st,ULONG attrs,void *buffer,FILE_INFORMATION_CLASS cls) { (void)st; (void)attrs; (void)buffer; (void)cls; }
static int ntdll_wcstoumbs(const WCHAR *,DWORD,char *,DWORD,BOOL);
#include "client_impl.h"
static int ntdll_wcstoumbs(const WCHAR *src,DWORD units,char *dst,DWORD capacity,BOOL strict)
{
    unsigned int bytes;
    NTSTATUS status=utf8_wcstombs(dst,capacity,&bytes,src,units);
    return strict && status==STATUS_SOME_NOT_MAPPED ? -1 : (int)bytes;
}
static unsigned int checks;
static void check(int value) { ++checks; if(!value) { fprintf(stderr,"client FAIL %u\n",checks); abort(); } }
int main(int argc,char **argv)
{
    assert(argc==2);
    FILE *nls=fopen(argv[1],"rb"); assert(nls); fseek(nls,0,SEEK_END); long size=ftell(nls); rewind(nls);
    USHORT *table=malloc(size); assert(table); assert(fread(table,1,size,nls)==(size_t)size); fclose(nls);
    uctable=table+2; lctable=table+table[1]+2;
    char temp[]="/tmp/carafe-client-XXXXXX"; directory=mkdtemp(temp); assert(directory);
    char path[1024]; snprintf(path,sizeof(path),"%s/МАЯК.sav",directory); FILE *file=fopen(path,"wb"); assert(file); fclose(file);
    returned_name=u"МАЯК.sav";
    UNICODE_STRING mask={16,16,(WCHAR *)u"МАЯК.sav"};
    const FILE_INFORMATION_CLASS classes[]={FileDirectoryInformation,FileFullDirectoryInformation,FileBothDirectoryInformation,
        FileIdBothDirectoryInformation,FileIdFullDirectoryInformation,FileIdExtdBothDirectoryInformation,
        FileIdGlobalTxDirectoryInformation,FileNamesInformation};
    unsigned char buffer[1024];
    for(unsigned int i=0;i<sizeof(classes)/sizeof(*classes);++i)
    {
        unsigned int base=dir_info_size(classes[i],0);
        for(unsigned int cut=0;cut<3;++cut)
        {
            unsigned int length=base+(cut==0?16:cut==1?3:0);
            memset(buffer,0xab,sizeof(buffer));
            IO_STATUS_BLOCK io={0}; request_count=seen_mask_bytes=seen_restart=0;
            NTSTATUS status=horizon_query_directory_file((HANDLE)1,&io,buffer,length,classes[i],TRUE,&mask);
            check(status==(cut?STATUS_BUFFER_OVERFLOW:STATUS_SUCCESS));
            check(io.Status==status && io.Information==length);
            check(buffer[length]==0xab);
            check(!memcmp(buffer+base,u"МАЯК.sav",length-base));
            check(request_count==1 && seen_mask_bytes==16 && seen_restart==1);
            if(!cut && classes[i]==FileBothDirectoryInformation)
                check(((FILE_BOTH_DIRECTORY_INFORMATION *)buffer)->ShortNameLength!=0);
        }
    }
    IO_STATUS_BLOCK io={0};
    server_status=STATUS_NO_SUCH_FILE;
    check(horizon_query_directory_file((HANDLE)1,&io,buffer,sizeof(buffer),FileNamesInformation,TRUE,&mask)==STATUS_NO_SUCH_FILE);
    check(io.Information==0 && io.Status==STATUS_NO_SUCH_FILE);
    server_status=STATUS_NO_MORE_FILES;
    check(horizon_query_directory_file((HANDLE)1,&io,buffer,sizeof(buffer),FileNamesInformation,FALSE,NULL)==STATUS_NO_MORE_FILES);
    check(io.Information==0 && io.Status==STATUS_NO_MORE_FILES);
    server_status=STATUS_OBJECT_TYPE_MISMATCH;
    check(horizon_query_directory_file((HANDLE)1,&io,buffer,sizeof(buffer),FileNamesInformation,FALSE,NULL)==STATUS_BAD_DEVICE_TYPE);
    check(io.Information==0 && io.Status==STATUS_BAD_DEVICE_TYPE);
    unlink(path); rmdir(directory); free(table);
    printf("PASS %u client checks: real Wine layouts, Horizon client, get_dir_data_entry, status/Information/overflow/short names\n",checks);
}
