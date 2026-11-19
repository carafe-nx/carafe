from pathlib import Path
import re
import subprocess as sp
import tempfile

root = Path(__file__).resolve().parents[2]
runtime = root / 'runtime'
autorun = root / 'external/autorun'
nt = autorun / 'dlls/ntdll/unix/file.c'
kb = autorun / 'dlls/kernelbase/file.c'

def extract(nt_text, kb_text):
    case = nt_text.split('    case FSCTL_GET_OBJECT_ID:\n', 1)[1].split('    case FSCTL_SET_SPARSE:', 1)[0]
    same = re.search(r'static (?:BOOL is_same_file|NTSTATUS compare_file_identity)\([^\n]*\n\{.*?\n\}', kb_text, re.S)[0]
    adapter = '' if 'compare_file_identity' in same else '\nstatic NTSTATUS compare_file_identity(HANDLE a,HANDLE b,BOOL *same) { *same=is_same_file(a,b); return 0; }\n'
    return 'static NTSTATUS control(HANDLE handle, unsigned code, void *in_buffer, size_t in_size, void *out_buffer, size_t out_size) {\n' + \
        'NTSTATUS status=0; size_t size=0; switch(code) { case FSCTL_GET_OBJECT_ID:\n' + \
        case + 'default: return STATUS_NOT_SUPPORTED; } (void)size; return status; }\n' + middle + same + adapter + fixture.split('\n@@\n')[1] + re.search(r'static BOOL copy_file\([^\n]*\n\{.*?\n\}',kb_text,re.S)[0] + main

header = r'''
#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/stat.h>
#include <errno.h>
typedef int HANDLE, BOOL;
typedef uint16_t WCHAR;
typedef uint32_t NTSTATUS, ULONG;
typedef struct { unsigned char ObjectId[16], spare[48]; } FILE_OBJECTID_BUFFER;
typedef struct { ULONG FileNameLength; WCHAR FileName[1]; } FILE_NAME_INFORMATION;
typedef struct { uint16_t Length, MaximumLength; WCHAR *Buffer; } UNICODE_STRING;
typedef int IO_STATUS_BLOCK;
enum { FSCTL_GET_OBJECT_ID=1, FileNameInformation=9, TRUE=1, FALSE=0 };
#define STATUS_NOT_SUPPORTED 0xc00000bbu
#define STATUS_BUFFER_TOO_SMALL 0xc0000023u
#define STATUS_INVALID_PARAMETER 0xc000000du
#define STATUS_UNSUCCESSFUL 0xc0000001u
#define STATUS_SUCCESS 0
#define ULongToHandle(x) ((HANDLE)(x))
#define HandleToULong(x) ((ULONG)(x))
extern int carafeOverlayCompareFiles(const char *,const char *,int *) __attribute__((weak));
extern void fixture_init(char *,char *);
static int mode, closes;
static NTSTATUS server_get_unix_fd(HANDLE h,int access,int *fd,int *owned,void *a,void *b)
{ (void)access; (void)a; (void)b; *fd=h; *owned=1; return 0; }
static int backend_stat(int fd,struct stat *st)
{ memset(st,0,sizeof(*st)); if(mode==2) { errno=EIO; return -1; } if(mode==1) st->st_ino=fd==2?2:1; return 0; }
static void backend_close(int fd) { (void)fd; ++closes; errno=EBADF; }
static NTSTATUS errno_to_status(int e) { return e==EIO?0xc0000185u:e==ENOENT?0xc0000034u:STATUS_INVALID_PARAMETER; }
#define fstat backend_stat
#define close backend_close
static NTSTATUS server_get_unix_name(HANDLE h,char **out)
{
    const char *paths[]={"sdmc:/switch/wine/logs/a.sav","sdmc:/switch/wine/logs/b.sav","sdmc:/switch/carafe/title/logs/a.sav",NULL,"ums0:/a.sav","ums1:/a.sav","sdmc:/switch/carafe/title/logs/A~1.SAV","ums0:/SAVE/a.sav","ums0:/Save/b.sav","ums0:/Save/A.SAV"};
    const char *p=paths[h-1];
    if(!p) return STATUS_BUFFER_TOO_SMALL;
    *out=malloc(strlen(p)+1); assert(*out); strcpy(*out,p); return 0;
}
#define ARRAY_SIZE(a) (sizeof(a)/sizeof((a)[0]))
#define CP_UTF8 65001
#define MB_ERR_INVALID_CHARS 8
static int MultiByteToWideChar(int cp,int flags,const char *s,int n,WCHAR *out,size_t cap)
{
    assert(cp==CP_UTF8 && flags==MB_ERR_INVALID_CHARS && n==-1);
    size_t len=strlen(s)+1;
    if(len>cap) return 0;
    for(size_t i=0;i<len;i++) { assert((unsigned char)s[i]<128); out[i]=s[i]; }
    return len;
}
static void RtlInitUnicodeString(UNICODE_STRING *u,WCHAR *s)
{ u->Buffer=s; u->Length=0; while(*s++) u->Length+=2; u->MaximumLength=u->Length+2; }
static BOOL RtlEqualUnicodeString(const UNICODE_STRING *a,const UNICODE_STRING *b,BOOL fold)
{
    assert(fold); if(a->Length!=b->Length) return 0;
    for(unsigned i=0;i<a->Length/2;i++) if((a->Buffer[i]|32)!=(b->Buffer[i]|32)) return 0;
    return 1;
}
'''
middle = r'''
static NTSTATUS NtFsControlFile(HANDLE h,int e,void *a,void *b,IO_STATUS_BLOCK *io,int code,void *in,int n,void *out,size_t size)
{ (void)e;(void)a;(void)b;(void)io; return control(h,code,in,n,out,size); }
'''
main = r'''
int main(int argc,char **argv)
{
    assert(argc==2);
    int green=argv[1][0]=='g';
    BOOL different,same;
    assert(!compare_file_identity(1,2,&different) && !compare_file_identity(1,3,&same));
    printf("zero inode: distinct=%d same=%d\n",different,same);
    assert(same && different==!green);
    if(!green) return different;
    check_copy(1,2,0); check_copy(1,5,0); check_copy(5,6,0);
    check_copy(1,1,32); check_copy(1,3,32);
    check_copy(1,4,STATUS_BUFFER_TOO_SMALL); check_copy(1,7,STATUS_INVALID_PARAMETER);
    check_copy(8,9,0); check_copy(8,10,32);
    mode=1; assert(!compare_file_identity(1,2,&same) && !same && !compare_file_identity(1,3,&same) && same);
    mode=2; FILE_OBJECTID_BUFFER id, original; memset(&id,0x5a,sizeof(id)); original=id;
    int before=closes;
    assert(control(1,FSCTL_GET_OBJECT_ID,NULL,0,&id,sizeof(id))==0xc0000185u && closes==before+1 && !memcmp(&id,&original,sizeof(id)));
    assert(compare_file_identity(1,2,&same)==0xc0000185u);
    check_copy(1,2,0xc0000185u);
    puts("PASS mixed-case FAT distinct/self, overlay alias, cross-backend, exact self, 8.3 and fstat error preserve bytes");
}
'''

fixture = Path(__file__).with_name('copy_identity_fixture.inc').read_text()
with tempfile.TemporaryDirectory() as temp:
    temp = Path(temp)
    text=(runtime/'overlay/carafe_overlay.c').read_text()
    def part(first,last):
        start=text.index(first)
        return text[start:text.index(last,start)]
    bridge=part('static int identityTarget(', 'static int overlayMkdir(')
    overlay=temp/'overlay.c'
    overlay.write_text(fixture.split('\n@@\n')[0]+'\n'+part('static bool isUserCache(','static time_t nowFileTime(')+bridge)
    for path in ('sdmc:/switch/carafe/title/logs/a.sav','sdmc:/switch/carafe/title/logs/b.sav','ums0:/a.sav','ums1:/a.sav','ums0:/Save/a.sav','ums0:/Save/b.sav'):
        file=temp/path
        file.parent.mkdir(parents=True,exist_ok=True)
        file.write_bytes(b'source')
    for src, patch in ((nt, runtime/'patches/autorun/0141-object-id-requires-a-valid-inode.patch'),
                       (kb, runtime/'patches/wine-pe/0142-copy-file-checks-names-when-object-id-is-unavailable.patch')):
        dest = temp / src.relative_to(autorun)
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(src.read_bytes())
        sp.run(['git','apply',str(patch)],cwd=temp,check=True)
    for mode, nt_text, kb_text in (('red',nt.read_text(),kb.read_text()),
                                   ('green',(temp/nt.relative_to(autorun)).read_text(),(temp/kb.relative_to(autorun)).read_text())):
        c, exe = temp/'check.c', temp/'check'
        c.write_text(header+extract(nt_text,kb_text))
        sp.run(['cc','-std=gnu11','-D_GNU_SOURCE','-D__SWITCH__','-O2','-pthread',str(c),str(overlay),'-o',str(exe)],check=True)
        result = sp.run([str(exe),mode],cwd=temp)
        print(mode, 'exit', result.returncode, flush=True)
        assert result.returncode==(mode=='red')
