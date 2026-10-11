#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/iosupport.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <time.h>

#include <switch.h>

#include "carafe_loading.h"
#include "carafe_overlay.h"

#ifndef CARAFE_SAVE_SIZE
#define CARAFE_SAVE_SIZE 0x20000000LL
#endif
#ifndef CARAFE_SAVE_JOURNAL_SIZE
#define CARAFE_SAVE_JOURNAL_SIZE 0x8000000LL
#endif

#define RO_DEVICE "carafe-ro"
#define RW_DEVICE "carafe-rw"
#define WINE_ROOT "/switch/wine"
#define GAMES_DIR WINE_ROOT "/drive_c/Games/"
#define PATH_SIZE FS_MAX_PATH
#define COPY_CHUNK 0x10000
#define COMMIT_POLL_NS 1000000000LL
#define COMMIT_MAX_WAIT_NS 10000000000ULL
#define COMMIT_QUIET_NS 2000000000ULL
#define LOG_HISTORY 5
#define COMMIT_EARLY_BYTES (CARAFE_SAVE_JOURNAL_SIZE / 2)
#define USERS_DIR "/drive_c/users/"
#define DXVK_CACHE_DIR "/AppData/Local/dxvk"

typedef enum {
    RouteBase,
    RouteVolatile,
    RouteUnion,
} Route;

typedef struct {
    Route route;
    char base[PATH_SIZE];
    char rw[PATH_SIZE];
    char ro[PATH_SIZE];
} Resolved;

typedef struct OverlayFile {
    const devoptab_t* dev;
    void* state;
    bool writer;
    int flags;
    Mutex lock;
    struct OverlayFile* prev;
    struct OverlayFile* next;
    char rw_path[PATH_SIZE];
} OverlayFile;

typedef struct {
    const devoptab_t* dev;
    DIR_ITER iter;
    bool open;
} SubDir;

typedef struct {
    SubDir first;
    SubDir second;
    bool union_dir;
    bool first_done;
    char rw_path[PATH_SIZE];
} OverlayDir;

static const devoptab_t* g_base;
static const devoptab_t* g_ro;
static const devoptab_t* g_rw;
static FsFileSystem* g_rw_fs;
static devoptab_t g_overlay;
static char g_volatile_root[64];
static char g_log_path[PATH_SIZE];
static bool g_log_ready;

static Mutex g_commit_mutex;
static OverlayFile* g_writers;
static bool g_dirty;
static u64 g_last_change;
static u64 g_dirty_since;
static u64 g_unsaved_bytes;
static u64 g_started;
static bool g_exit_locked;
static Thread g_commit_thread;
static u8 g_commit_stack[0x8000] __attribute__((aligned(0x1000)));
static volatile bool g_stop;
static bool g_thread_running;
static bool g_installed;
static u64 g_bytes_read;
static bool g_game_opened;

static void logLine(const char* fmt, ...) __attribute__((format(printf, 1, 2)));

static void logLine(const char* fmt, ...) {
    char line[384];
    int prefix = snprintf(line, sizeof(line), "[carafe-overlay] ");

    va_list args;
    va_start(args, fmt);
    vsnprintf(line + prefix, sizeof(line) - prefix, fmt, args);
    va_end(args);

    svcOutputDebugString(line, strlen(line));
    if (g_log_ready) {
        FILE* log = fopen(g_log_path, "a");
        if (log != NULL) {
            fprintf(log, "%s\n", line);
            fclose(log);
        }
    }
}

static void* enter(struct _reent* r, const devoptab_t* dev) {
    void* saved = r->deviceData;
    r->deviceData = dev->deviceData;
    return saved;
}

static void leave(struct _reent* r, void* saved) {
    r->deviceData = saved;
}

static int fail(struct _reent* r, int error) {
    r->_errno = error;
    return -1;
}

static void markChanged(void) {
    u64 now = armGetSystemTick();
    __atomic_store_n(&g_last_change, now, __ATOMIC_RELAXED);
    if (!__atomic_exchange_n(&g_dirty, true, __ATOMIC_RELAXED)) {
        __atomic_store_n(&g_dirty_since, now, __ATOMIC_RELAXED);
    }
}

static bool startsWith(const char* text, const char* prefix) {
    return strncmp(text, prefix, strlen(prefix)) == 0;
}

static bool format(char* out, size_t size, const char* fmt, ...) __attribute__((format(printf, 3, 4)));

static bool format(char* out, size_t size, const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    int written = vsnprintf(out, size, fmt, args);
    va_end(args);
    return written >= 0 && (size_t)written < size;
}

typedef struct {
    char* lower;
    char* actual;
} IndexEntry;

static IndexEntry* g_index;
static size_t g_index_capacity;
static size_t g_index_count;

static u64 hashLower(const char* text, size_t len) {
    u64 hash = 1469598103934665603ULL;
    for (size_t i = 0; i < len; i++) {
        hash ^= (unsigned char)tolower((unsigned char)text[i]);
        hash *= 1099511628211ULL;
    }
    return hash;
}

static bool sameLower(const char* lower, const char* text, size_t len) {
    for (size_t i = 0; i < len; i++) {
        if (lower[i] != (char)tolower((unsigned char)text[i])) {
            return false;
        }
    }
    return lower[len] == '\0';
}

static const char* indexFind(const char* path, size_t len) {
    if (g_index_capacity == 0) {
        return NULL;
    }
    for (size_t slot = hashLower(path, len) & (g_index_capacity - 1);
         g_index[slot].lower != NULL;
         slot = (slot + 1) & (g_index_capacity - 1)) {
        if (sameLower(g_index[slot].lower, path, len)) {
            return g_index[slot].actual;
        }
    }
    return NULL;
}

static bool indexInsert(IndexEntry* table, size_t capacity, char* lower, char* actual) {
    size_t slot = hashLower(lower, strlen(lower)) & (capacity - 1);
    while (table[slot].lower != NULL) {
        if (strcmp(table[slot].lower, lower) == 0) {
            return false;
        }
        slot = (slot + 1) & (capacity - 1);
    }
    table[slot].lower = lower;
    table[slot].actual = actual;
    return true;
}

static bool indexGrow(void) {
    size_t capacity = g_index_capacity == 0 ? 4096 : g_index_capacity * 2;
    IndexEntry* table = calloc(capacity, sizeof(*table));
    if (table == NULL) {
        return false;
    }
    for (size_t i = 0; i < g_index_capacity; i++) {
        if (g_index[i].lower != NULL) {
            indexInsert(table, capacity, g_index[i].lower, g_index[i].actual);
        }
    }
    free(g_index);
    g_index = table;
    g_index_capacity = capacity;
    return true;
}

static bool indexAdd(const char* actual) {
    if ((g_index_count + 1) * 2 > g_index_capacity && !indexGrow()) {
        return false;
    }
    size_t len = strlen(actual);
    char* copy = malloc(len * 2 + 2);
    if (copy == NULL) {
        return false;
    }
    char* lower = copy + len + 1;
    memcpy(copy, actual, len + 1);
    for (size_t i = 0; i <= len; i++) {
        lower[i] = (char)tolower((unsigned char)actual[i]);
    }
    if (indexInsert(g_index, g_index_capacity, lower, copy)) {
        g_index_count++;
    } else {
        free(copy);
    }
    return true;
}

static bool isDxvkCache(const char* rel) {
    const size_t users_len = strlen(USERS_DIR);
    if (strncasecmp(rel, USERS_DIR, users_len) != 0) {
        return false;
    }
    const char* user_end = strchr(rel + users_len, '/');
    const size_t cache_len = strlen(DXVK_CACHE_DIR);
    return user_end != NULL
        && strncasecmp(user_end, DXVK_CACHE_DIR, cache_len) == 0
        && (user_end[cache_len] == '\0' || user_end[cache_len] == '/');
}

static bool isVolatile(const char* rel) {
    return isDxvkCache(rel)
        || strcmp(rel, "/logs") == 0
        || startsWith(rel, "/logs/")
        || (startsWith(rel, "/wine-nx-session-") && strchr(rel + 1, '/') == NULL)
        || startsWith(rel, "/swap-poc");
}

static bool canonicalize(struct _reent* r, const char* rel, char* out, size_t size);

static bool resolve(struct _reent* r, const char* path, Resolved* out) {
    const char* p = startsWith(path, "sdmc:") ? path + 5 : path;
    char absolute[PATH_SIZE];
    if (p[0] != '/') {
        if (!format(absolute, sizeof(absolute), "/%s", p)) {
            return false;
        }
        p = absolute;
    }

    const size_t root_len = strlen(WINE_ROOT);
    const bool under_root = strncmp(p, WINE_ROOT, root_len) == 0 && (p[root_len] == '\0' || p[root_len] == '/');
    if (!under_root) {
        out->route = RouteBase;
        return format(out->base, sizeof(out->base), "sdmc:%s", p);
    }

    const char* rel = p + root_len;
    if (isVolatile(rel)) {
        out->route = RouteVolatile;
        return format(out->base, sizeof(out->base), "sdmc:%s%s", g_volatile_root, rel);
    }

    out->route = RouteUnion;
    char canonical[PATH_SIZE];
    return canonicalize(r, rel, canonical, sizeof(canonical))
        && format(out->rw, sizeof(out->rw), RW_DEVICE ":" WINE_ROOT "%s", canonical)
        && format(out->ro, sizeof(out->ro), RO_DEVICE ":" WINE_ROOT "%s", canonical);
}

static int devStat(struct _reent* r, const devoptab_t* dev, const char* path, struct stat* st) {
    void* saved = enter(r, dev);
    int ret = dev->stat_r(r, path, st);
    leave(r, saved);
    return ret;
}

static bool devExists(struct _reent* r, const devoptab_t* dev, const char* path, struct stat* st) {
    int saved_errno = r->_errno;
    struct stat local;
    bool exists = devStat(r, dev, path, st != NULL ? st : &local) == 0;
    r->_errno = saved_errno;
    return exists;
}

static bool rwExists(const char* rw_path) {
    const char* separator = strchr(rw_path, ':');
    FsDirEntryType type;
    return R_SUCCEEDED(fsFsGetEntryType(g_rw_fs, separator != NULL ? separator + 1 : rw_path, &type));
}

static int devMkdir(struct _reent* r, const devoptab_t* dev, const char* path, int mode) {
    void* saved = enter(r, dev);
    int ret = dev->mkdir_r(r, path, mode);
    leave(r, saved);
    return ret;
}

static void mkdirParentsOn(struct _reent* r, const devoptab_t* dev, const char* path) {
    char partial[PATH_SIZE];
    const char* start = strchr(path, ':');
    if (start == NULL) {
        return;
    }

    for (const char* slash = strchr(start + 2, '/'); slash != NULL; slash = strchr(slash + 1, '/')) {
        size_t len = (size_t)(slash - path);
        memcpy(partial, path, len);
        partial[len] = '\0';
        if (!devExists(r, dev, partial, NULL)) {
            int saved_errno = r->_errno;
            devMkdir(r, dev, partial, 0777);
            r->_errno = saved_errno;
        }
    }
}

static void mkdirParents(struct _reent* r, const char* rw_path) {
    mkdirParentsOn(r, g_rw, rw_path);
}

static int openInner(
    struct _reent* r,
    const devoptab_t* dev,
    const char* path,
    int flags,
    int mode,
    void** out_state
) {
    void* state = calloc(1, dev->structSize);
    if (state == NULL) {
        return fail(r, ENOMEM);
    }

    void* saved = enter(r, dev);
    int ret = dev->open_r(r, state, path, flags, mode);
    leave(r, saved);
    if (ret == -1) {
        free(state);
        return -1;
    }

    *out_state = state;
    return 0;
}

static void closeInner(struct _reent* r, const devoptab_t* dev, void* state) {
    void* saved = enter(r, dev);
    dev->close_r(r, state);
    leave(r, saved);
    free(state);
}

static int copyUp(struct _reent* r, const char* ro_path, const char* rw_path) {
    void* src = NULL;
    void* dst = NULL;
    if (openInner(r, g_ro, ro_path, O_RDONLY, 0, &src) == -1) {
        return -1;
    }
    mkdirParents(r, rw_path);
    if (openInner(r, g_rw, rw_path, O_WRONLY | O_CREAT | O_TRUNC, 0666, &dst) == -1) {
        closeInner(r, g_ro, src);
        return -1;
    }

    char* buffer = malloc(COPY_CHUNK);
    int ret = buffer != NULL ? 0 : fail(r, ENOMEM);
    while (ret == 0) {
        void* saved = enter(r, g_ro);
        ssize_t got = g_ro->read_r(r, src, buffer, COPY_CHUNK);
        leave(r, saved);
        if (got <= 0) {
            ret = got < 0 ? -1 : 0;
            break;
        }

        saved = enter(r, g_rw);
        ssize_t put = g_rw->write_r(r, dst, buffer, (size_t)got);
        leave(r, saved);
        if (put != got) {
            ret = -1;
        }
    }

    free(buffer);
    closeInner(r, g_rw, dst);
    closeInner(r, g_ro, src);
    if (ret == 0) {
        markChanged();
    }
    return ret;
}

static void rotateLog(struct _reent* r, const char* path) {
    size_t len = strlen(path);
    if (len < 5 || strcmp(path + len - 4, ".log") != 0) {
        return;
    }

    int saved_errno = r->_errno;
    char from[PATH_SIZE];
    char to[PATH_SIZE];
    const int stem = (int)(len - 4);
    for (int i = LOG_HISTORY - 1; i >= 0; i--) {
        bool named = i == 0
            ? format(from, sizeof(from), "%s", path)
            : format(from, sizeof(from), "%.*s.%d.log", stem, path, i);
        if (!named || !format(to, sizeof(to), "%.*s.%d.log", stem, path, i + 1)) {
            continue;
        }
        void* saved = enter(r, g_base);
        g_base->unlink_r(r, to);
        g_base->rename_r(r, from, to);
        leave(r, saved);
    }
    r->_errno = saved_errno;
}

static bool wantsWrite(int flags) {
    return (flags & O_ACCMODE) != O_RDONLY || (flags & (O_CREAT | O_TRUNC | O_APPEND)) != 0;
}

static void rememberWriter(OverlayFile* file, const char* rw_path, int flags) {
    format(file->rw_path, sizeof(file->rw_path), "%s", rw_path);
    file->flags = flags & ~(O_CREAT | O_TRUNC | O_EXCL);
    mutexInit(&file->lock);
    mutexLock(&g_commit_mutex);
    file->prev = NULL;
    file->next = g_writers;
    if (g_writers != NULL) {
        g_writers->prev = file;
    }
    g_writers = file;
    mutexUnlock(&g_commit_mutex);
}

static void forgetWriter(OverlayFile* file) {
    if (file->prev != NULL) {
        file->prev->next = file->next;
    } else {
        g_writers = file->next;
    }
    if (file->next != NULL) {
        file->next->prev = file->prev;
    }
}

static void lockFile(OverlayFile* file) {
    if (file->writer) {
        mutexLock(&file->lock);
    }
}

static void unlockFile(OverlayFile* file) {
    if (file->writer) {
        mutexUnlock(&file->lock);
    }
}

static void noteWrite(u64 bytes) {
    __atomic_add_fetch(&g_unsaved_bytes, bytes, __ATOMIC_RELAXED);
    markChanged();
}

static int overlayOpen(struct _reent* r, void* file_struct, const char* path, int flags, int mode) {
    OverlayFile* file = file_struct;
    Resolved p;
    if (!resolve(r, path, &p)) {
        return fail(r, ENAMETOOLONG);
    }

    file->writer = false;
    if (p.route != RouteUnion) {
        if (p.route == RouteVolatile && wantsWrite(flags)) {
            mkdirParentsOn(r, g_base, p.base);
            if (flags & O_TRUNC) {
                rotateLog(r, p.base);
            }
        }
        file->dev = g_base;
        return openInner(r, g_base, p.base, flags, mode, &file->state);
    }

    if (!__atomic_load_n(&g_game_opened, __ATOMIC_RELAXED) && strstr(p.ro, GAMES_DIR) != NULL) {
        __atomic_store_n(&g_game_opened, true, __ATOMIC_RELAXED);
    }

    if (!wantsWrite(flags)) {
        bool in_rw = rwExists(p.rw);
        file->dev = in_rw ? g_rw : g_ro;
        return openInner(r, file->dev, in_rw ? p.rw : p.ro, flags, mode, &file->state);
    }

    if ((flags & O_CREAT) && (flags & O_EXCL) && devExists(r, g_ro, p.ro, NULL)) {
        return fail(r, EEXIST);
    }

    markChanged();
    int ret = 0;
    if (!rwExists(p.rw)) {
        if (!(flags & O_TRUNC) && devExists(r, g_ro, p.ro, NULL)) {
            ret = copyUp(r, p.ro, p.rw);
        } else {
            mkdirParents(r, p.rw);
        }
    }
    if (ret == 0) {
        file->dev = g_rw;
        ret = openInner(r, g_rw, p.rw, flags, mode, &file->state);
    }
    if (ret == -1) {
        return -1;
    }

    file->writer = true;
    rememberWriter(file, p.rw, flags);
    return 0;
}

static int overlayClose(struct _reent* r, void* fd) {
    OverlayFile* file = fd;
    if (file->writer) {
        mutexLock(&g_commit_mutex);
        forgetWriter(file);
        mutexUnlock(&g_commit_mutex);
    }

    int ret = 0;
    if (file->state != NULL) {
        void* saved = enter(r, file->dev);
        ret = file->dev->close_r(r, file->state);
        leave(r, saved);
        free(file->state);
        file->state = NULL;
    }

    if (file->writer) {
        markChanged();
    }
    return ret;
}

static ssize_t overlayWrite(struct _reent* r, void* fd, const char* ptr, size_t len) {
    OverlayFile* file = fd;
    lockFile(file);
    ssize_t ret = -1;
    if (file->state == NULL) {
        fail(r, EIO);
    } else {
        void* saved = enter(r, file->dev);
        ret = file->dev->write_r(r, file->state, ptr, len);
        leave(r, saved);
    }
    unlockFile(file);
    if (ret > 0 && file->writer) {
        noteWrite((u64)ret);
    }
    return ret;
}

static ssize_t overlayRead(struct _reent* r, void* fd, char* ptr, size_t len) {
    OverlayFile* file = fd;
    lockFile(file);
    ssize_t ret = -1;
    if (file->state == NULL) {
        fail(r, EIO);
    } else {
        void* saved = enter(r, file->dev);
        ret = file->dev->read_r(r, file->state, ptr, len);
        leave(r, saved);
    }
    unlockFile(file);
    if (ret > 0) {
        __atomic_add_fetch(&g_bytes_read, (u64)ret, __ATOMIC_RELAXED);
    }
    return ret;
}

static off_t overlaySeek(struct _reent* r, void* fd, off_t pos, int dir) {
    OverlayFile* file = fd;
    lockFile(file);
    off_t ret = -1;
    if (file->state == NULL) {
        fail(r, EIO);
    } else {
        void* saved = enter(r, file->dev);
        ret = file->dev->seek_r(r, file->state, pos, dir);
        leave(r, saved);
    }
    unlockFile(file);
    return ret;
}

static int overlayFstat(struct _reent* r, void* fd, struct stat* st) {
    OverlayFile* file = fd;
    lockFile(file);
    int ret = -1;
    if (file->state == NULL) {
        fail(r, EIO);
    } else {
        void* saved = enter(r, file->dev);
        ret = file->dev->fstat_r(r, file->state, st);
        leave(r, saved);
    }
    unlockFile(file);
    return ret;
}

static int overlayFtruncate(struct _reent* r, void* fd, off_t len) {
    OverlayFile* file = fd;
    if (file->dev->ftruncate_r == NULL) {
        return fail(r, EROFS);
    }
    lockFile(file);
    int ret = -1;
    if (file->state == NULL) {
        fail(r, EIO);
    } else {
        void* saved = enter(r, file->dev);
        ret = file->dev->ftruncate_r(r, file->state, len);
        leave(r, saved);
    }
    unlockFile(file);
    if (ret == 0 && file->writer) {
        noteWrite(0);
    }
    return ret;
}

static int overlayFsync(struct _reent* r, void* fd) {
    OverlayFile* file = fd;
    if (file->dev->fsync_r == NULL) {
        return 0;
    }
    lockFile(file);
    int ret = 0;
    if (file->state != NULL) {
        void* saved = enter(r, file->dev);
        ret = file->dev->fsync_r(r, file->state);
        leave(r, saved);
    }
    unlockFile(file);
    return ret;
}

static int overlayStat(struct _reent* r, const char* path, struct stat* st) {
    Resolved p;
    if (!resolve(r, path, &p)) {
        return fail(r, ENAMETOOLONG);
    }
    if (p.route != RouteUnion) {
        return devStat(r, g_base, p.base, st);
    }
    if (rwExists(p.rw)) {
        return devStat(r, g_rw, p.rw, st);
    }
    return devStat(r, g_ro, p.ro, st);
}

static int overlayMkdir(struct _reent* r, const char* path, int mode) {
    Resolved p;
    if (!resolve(r, path, &p)) {
        return fail(r, ENAMETOOLONG);
    }
    if (p.route != RouteUnion) {
        if (p.route == RouteVolatile) {
            mkdirParentsOn(r, g_base, p.base);
        }
        return devMkdir(r, g_base, p.base, mode);
    }
    if (rwExists(p.rw) || devExists(r, g_ro, p.ro, NULL)) {
        return fail(r, EEXIST);
    }

    mkdirParents(r, p.rw);
    int ret = devMkdir(r, g_rw, p.rw, mode);
    if (ret == 0) {
        markChanged();
    }
    return ret;
}

static int removePath(struct _reent* r, const char* path, bool directory) {
    Resolved p;
    if (!resolve(r, path, &p)) {
        return fail(r, ENAMETOOLONG);
    }

    const devoptab_t* dev = g_base;
    const char* target = p.base;
    if (p.route == RouteUnion) {
        if (!rwExists(p.rw)) {
            return fail(r, devExists(r, g_ro, p.ro, NULL) ? EACCES : ENOENT);
        }
        dev = g_rw;
        target = p.rw;
    }

    void* saved = enter(r, dev);
    int ret = directory ? dev->rmdir_r(r, target) : dev->unlink_r(r, target);
    leave(r, saved);
    if (ret == 0 && dev == g_rw) {
        markChanged();
    }
    return ret;
}

static int overlayUnlink(struct _reent* r, const char* path) {
    return removePath(r, path, false);
}

static int overlayRmdir(struct _reent* r, const char* path) {
    return removePath(r, path, true);
}

static int overlayRename(struct _reent* r, const char* old_name, const char* new_name) {
    Resolved from;
    Resolved to;
    if (!resolve(r, old_name, &from) || !resolve(r, new_name, &to)) {
        return fail(r, ENAMETOOLONG);
    }
    if ((from.route == RouteUnion) != (to.route == RouteUnion)) {
        return fail(r, EXDEV);
    }

    const devoptab_t* dev = g_base;
    const char* old_target = from.base;
    const char* new_target = to.base;
    if (from.route == RouteUnion) {
        if (!rwExists(from.rw)) {
            struct stat st;
            if (!devExists(r, g_ro, from.ro, &st)) {
                return fail(r, ENOENT);
            }
            if (S_ISDIR(st.st_mode) || copyUp(r, from.ro, from.rw) == -1) {
                return fail(r, EACCES);
            }
        }
        mkdirParents(r, to.rw);
        dev = g_rw;
        old_target = from.rw;
        new_target = to.rw;
    }

    void* saved = enter(r, dev);
    int ret = dev->rename_r(r, old_target, new_target);
    leave(r, saved);
    if (ret == 0 && dev == g_rw) {
        markChanged();
    }
    return ret;
}

static int overlayStatvfs(struct _reent* r, const char* path, struct statvfs* buf) {
    Resolved p;
    if (!resolve(r, path, &p)) {
        return fail(r, ENAMETOOLONG);
    }

    const devoptab_t* dev = p.route == RouteUnion ? g_rw : g_base;
    const char* target = p.route == RouteUnion ? RW_DEVICE ":/" : p.base;
    if (dev->statvfs_r == NULL) {
        return fail(r, ENOSYS);
    }

    void* saved = enter(r, dev);
    int ret = dev->statvfs_r(r, target, buf);
    leave(r, saved);
    return ret;
}

static const devoptab_t* attributeTarget(const Resolved* p, const char** target) {
    if (p->route != RouteUnion) {
        *target = p->base;
        return g_base;
    }
    if (!rwExists(p->rw)) {
        return NULL;
    }
    *target = p->rw;
    return g_rw;
}

static int overlayChmod(struct _reent* r, const char* path, mode_t mode) {
    Resolved p;
    if (!resolve(r, path, &p)) {
        return fail(r, ENAMETOOLONG);
    }

    const char* target = NULL;
    const devoptab_t* dev = attributeTarget(&p, &target);
    if (dev == NULL || dev->chmod_r == NULL) {
        return 0;
    }

    void* saved = enter(r, dev);
    int ret = dev->chmod_r(r, target, mode);
    leave(r, saved);
    return ret;
}

static int overlayUtimes(struct _reent* r, const char* path, const struct timeval times[2]) {
    Resolved p;
    if (!resolve(r, path, &p)) {
        return fail(r, ENAMETOOLONG);
    }

    const char* target = NULL;
    const devoptab_t* dev = attributeTarget(&p, &target);
    if (dev == NULL || dev->utimes_r == NULL) {
        return 0;
    }

    void* saved = enter(r, dev);
    int ret = dev->utimes_r(r, target, times);
    leave(r, saved);
    return ret;
}

static bool openSubDir(struct _reent* r, SubDir* sub, const devoptab_t* dev, const char* path) {
    sub->open = false;
    sub->dev = dev;
    sub->iter.device = 0;
    sub->iter.dirStruct = calloc(1, dev->dirStateSize);
    if (sub->iter.dirStruct == NULL) {
        return false;
    }

    void* saved = enter(r, dev);
    DIR_ITER* opened = dev->diropen_r(r, &sub->iter, path);
    leave(r, saved);
    if (opened == NULL) {
        free(sub->iter.dirStruct);
        sub->iter.dirStruct = NULL;
        return false;
    }

    sub->open = true;
    return true;
}

static void closeSubDir(struct _reent* r, SubDir* sub) {
    if (!sub->open) {
        return;
    }
    void* saved = enter(r, sub->dev);
    sub->dev->dirclose_r(r, &sub->iter);
    leave(r, saved);
    free(sub->iter.dirStruct);
    sub->open = false;
}

static int nextSubDir(struct _reent* r, SubDir* sub, char* filename, struct stat* st) {
    if (!sub->open) {
        return fail(r, ENOENT);
    }
    void* saved = enter(r, sub->dev);
    int ret = sub->dev->dirnext_r(r, &sub->iter, filename, st);
    leave(r, saved);
    return ret;
}

static bool isDotEntry(const char* name) {
    return strcmp(name, ".") == 0 || strcmp(name, "..") == 0;
}

static bool rwExistsRel(const char* rel) {
    char path[PATH_SIZE];
    FsDirEntryType type;
    return format(path, sizeof(path), WINE_ROOT "%s", rel)
        && R_SUCCEEDED(fsFsGetEntryType(g_rw_fs, path, &type));
}

static bool rwFindName(struct _reent* r, const char* dir_rel, const char* name, size_t len, char* out) {
    char dir_path[PATH_SIZE];
    if (!format(dir_path, sizeof(dir_path), RW_DEVICE ":" WINE_ROOT "%s", dir_rel)) {
        return false;
    }

    int saved_errno = r->_errno;
    bool found = false;
    SubDir sub;
    if (openSubDir(r, &sub, g_rw, dir_path)) {
        char entry[PATH_SIZE];
        struct stat st;
        while (!found && nextSubDir(r, &sub, entry, &st) == 0) {
            if (strlen(entry) == len && strncasecmp(entry, name, len) == 0) {
                memcpy(out, entry, len);
                found = true;
            }
        }
        closeSubDir(r, &sub);
    }
    r->_errno = saved_errno;
    return found;
}

static bool canonicalize(struct _reent* r, const char* rel, char* out, size_t size) {
    size_t rel_len = strlen(rel);
    const char* exact = indexFind(rel, rel_len);
    if (exact != NULL) {
        return format(out, size, "%s", exact);
    }
    if (rel_len >= size) {
        return false;
    }

    memcpy(out, rel, rel_len + 1);
    size_t pos = 0;
    while (out[pos] == '/') {
        size_t start = pos + 1;
        char* slash = strchr(out + start, '/');
        size_t end = slash != NULL ? (size_t)(slash - out) : rel_len;

        const char* actual = indexFind(out, end);
        if (actual != NULL) {
            memcpy(out, actual, end);
        } else {
            char saved = out[end];
            out[end] = '\0';
            bool exists = rwExistsRel(out);
            out[end] = saved;
            if (!exists) {
                out[pos] = '\0';
                bool found = rwFindName(r, out, rel + start, end - start, out + start);
                out[pos] = '/';
                if (!found) {
                    break;
                }
            }
        }
        pos = end;
    }
    return true;
}

static void indexDirectory(struct _reent* r, const char* rel) {
    char path[PATH_SIZE];
    if (!format(path, sizeof(path), RO_DEVICE ":" WINE_ROOT "%s", rel)) {
        return;
    }

    SubDir sub;
    if (!openSubDir(r, &sub, g_ro, path)) {
        return;
    }

    char name[PATH_SIZE];
    char child[PATH_SIZE];
    struct stat st;
    while (nextSubDir(r, &sub, name, &st) == 0) {
        if (isDotEntry(name)) {
            continue;
        }
        if (!format(child, sizeof(child), "%s/%s", rel, name) || !indexAdd(child)) {
            continue;
        }
        if (S_ISDIR(st.st_mode)) {
            indexDirectory(r, child);
        }
    }
    closeSubDir(r, &sub);
}

static DIR_ITER* overlayDiropen(struct _reent* r, DIR_ITER* dir_state, const char* path) {
    OverlayDir* dir = dir_state->dirStruct;
    Resolved p;
    memset(dir, 0, sizeof(*dir));
    if (!resolve(r, path, &p)) {
        r->_errno = ENAMETOOLONG;
        return NULL;
    }

    if (p.route != RouteUnion) {
        return openSubDir(r, &dir->first, g_base, p.base) ? dir_state : NULL;
    }

    dir->union_dir = true;
    memcpy(dir->rw_path, p.rw, sizeof(dir->rw_path));
    bool has_rw = openSubDir(r, &dir->first, g_rw, p.rw);
    bool has_ro = openSubDir(r, &dir->second, g_ro, p.ro);
    if (!has_rw && !has_ro) {
        r->_errno = ENOENT;
        return NULL;
    }
    return dir_state;
}

static int overlayDirnext(struct _reent* r, DIR_ITER* dir_state, char* filename, struct stat* st) {
    OverlayDir* dir = dir_state->dirStruct;
    if (!dir->first_done) {
        int saved_errno = r->_errno;
        if (nextSubDir(r, &dir->first, filename, st) == 0) {
            return 0;
        }
        if (!dir->union_dir) {
            return -1;
        }
        dir->first_done = true;
        r->_errno = saved_errno;
    }

    char rw_entry[PATH_SIZE];
    while (nextSubDir(r, &dir->second, filename, st) == 0) {
        if (!dir->first.open) {
            return 0;
        }
        if (!format(rw_entry, sizeof(rw_entry), "%s/%s", dir->rw_path, filename)
            || !rwExists(rw_entry)) {
            return 0;
        }
    }
    return -1;
}

static int overlayDirreset(struct _reent* r, DIR_ITER* dir_state) {
    OverlayDir* dir = dir_state->dirStruct;
    dir->first_done = false;
    SubDir* subs[] = { &dir->first, &dir->second };
    for (size_t i = 0; i < 2; i++) {
        if (subs[i]->open) {
            void* saved = enter(r, subs[i]->dev);
            subs[i]->dev->dirreset_r(r, &subs[i]->iter);
            leave(r, saved);
        }
    }
    return 0;
}

static int overlayDirclose(struct _reent* r, DIR_ITER* dir_state) {
    OverlayDir* dir = dir_state->dirStruct;
    closeSubDir(r, &dir->first);
    closeSubDir(r, &dir->second);
    return 0;
}

static int overlayLink(struct _reent* r, const char* existing, const char* new_link) {
    (void)existing;
    (void)new_link;
    return fail(r, ENOSYS);
}

static int overlayChdir(struct _reent* r, const char* name) {
    (void)name;
    return fail(r, ENOSYS);
}

static void suspendWriters(struct _reent* r, off_t* offsets, size_t count) {
    size_t i = 0;
    for (OverlayFile* file = g_writers; file != NULL; file = file->next, i++) {
        mutexLock(&file->lock);
        if (file->state == NULL || i >= count) {
            continue;
        }
        void* saved = enter(r, file->dev);
        offsets[i] = file->dev->seek_r(r, file->state, 0, SEEK_CUR);
        file->dev->close_r(r, file->state);
        leave(r, saved);
        free(file->state);
        file->state = NULL;
    }
}

static int resumeWriters(struct _reent* r, const off_t* offsets, size_t count) {
    int lost = 0;
    size_t i = 0;
    for (OverlayFile* file = g_writers; file != NULL; file = file->next, i++) {
        if (file->state == NULL && i < count) {
            if (openInner(r, g_rw, file->rw_path, file->flags, 0666, &file->state) == 0) {
                void* saved = enter(r, file->dev);
                file->dev->seek_r(r, file->state, offsets[i] < 0 ? 0 : offsets[i], SEEK_SET);
                leave(r, saved);
            } else {
                lost++;
                logLine("reopen failed after commit: %s (errno %d)", file->rw_path, r->_errno);
            }
        }
        mutexUnlock(&file->lock);
    }
    return lost;
}

static void commitNow(const char* reason) {
    struct _reent* r = _REENT;
    int saved_errno = r->_errno;
    u64 start = armGetSystemTick();

    size_t count = 0;
    for (OverlayFile* file = g_writers; file != NULL; file = file->next) {
        count++;
    }
    off_t* offsets = count > 0 ? calloc(count, sizeof(off_t)) : NULL;
    if (count > 0 && offsets == NULL) {
        logLine("commit skipped: no memory for %zu open files", count);
        return;
    }

    suspendWriters(r, offsets, count);
    Result rc = fsdevCommitDevice(RW_DEVICE);
    int lost = resumeWriters(r, offsets, count);
    free(offsets);
    r->_errno = saved_errno;

    if (R_SUCCEEDED(rc)) {
        __atomic_store_n(&g_dirty, false, __ATOMIC_RELAXED);
        __atomic_store_n(&g_unsaved_bytes, 0, __ATOMIC_RELAXED);
    } else {
        __atomic_store_n(&g_last_change, armGetSystemTick(), __ATOMIC_RELAXED);
    }
    u64 end = armGetSystemTick();
    u64 us = armTicksToNs(end - start) / 1000;
    u64 at_ms = armTicksToNs(end - g_started) / 1000000;
    logLine(
        "commit %s at %lu.%03lu s: rc=0x%08X, %zu files reopened, %d lost, %lu us",
        reason,
        at_ms / 1000,
        at_ms % 1000,
        rc,
        count,
        lost,
        us
    );
}

static void commitIfDue(const char* forced_by) {
    mutexLock(&g_commit_mutex);
    if (__atomic_load_n(&g_dirty, __ATOMIC_RELAXED)) {
        u64 now = armGetSystemTick();
        u64 last = __atomic_load_n(&g_last_change, __ATOMIC_RELAXED);
        u64 since = __atomic_load_n(&g_dirty_since, __ATOMIC_RELAXED);
        bool quiet = now - last >= armNsToTicks(COMMIT_QUIET_NS);
        bool overdue = now - since >= armNsToTicks(COMMIT_MAX_WAIT_NS);
        bool full = __atomic_load_n(&g_unsaved_bytes, __ATOMIC_RELAXED) >= (u64)COMMIT_EARLY_BYTES;
        if (forced_by != NULL) {
            commitNow(forced_by);
        } else if (quiet) {
            commitNow("after writes");
        } else if (full) {
            commitNow("journal half full");
        } else if (overdue) {
            commitNow("after 10 s of writes");
        }
    }
    mutexUnlock(&g_commit_mutex);
}

static bool handleAppletMessage(void) {
    u32 message = 0;
    if (R_FAILED(appletGetMessage(&message))) {
        return true;
    }
    bool keep_running = appletProcessMessage(message);
    AppletFocusState focus = appletGetFocusState();
    logLine("applet message %u, focus %d", message, (int)focus);
    if (message == AppletMessage_FocusStateChanged && focus != AppletFocusState_InFocus) {
        commitIfDue("on leaving the game");
    }
    if (!keep_running || message == AppletMessage_ExitRequest) {
        commitIfDue("on close request");
        logLine("close requested, exit unlocked");
        if (g_exit_locked) {
            g_exit_locked = false;
            appletUnlockExit();
        }
        return false;
    }
    return true;
}

static void commitLoop(void* arg) {
    (void)arg;
    bool listening = true;
    while (!g_stop) {
        if (listening && R_SUCCEEDED(eventWait(appletGetMessageEvent(), COMMIT_POLL_NS))) {
            listening = handleAppletMessage();
        } else if (!listening) {
            svcSleepThread(COMMIT_POLL_NS);
        }
        commitIfDue(NULL);
        if (carafeLoadingTick) {
            carafeLoadingTick();
        }
    }
}

static Result openSave(u64 program_id, FsFileSystem* fs) {
    Result rc = fsOpen_DeviceSaveData(fs, program_id);
    if (R_SUCCEEDED(rc)) {
        return rc;
    }

    FsSaveDataAttribute attr = { .application_id = program_id, .save_data_type = FsSaveDataType_Device };
    FsSaveDataCreationInfo creation = {
        .save_data_size = CARAFE_SAVE_SIZE,
        .journal_size = CARAFE_SAVE_JOURNAL_SIZE,
        .available_size = 0x4000,
        .owner_id = program_id,
        .save_data_space_id = FsSaveDataSpaceId_User,
    };
    FsSaveDataMetaInfo meta = { 0 };
    logLine("device save open rc=0x%08X, creating", rc);
    rc = fsCreateSaveDataFileSystem(&attr, &creation, &meta);
    if (R_FAILED(rc)) {
        return rc;
    }
    return fsOpen_DeviceSaveData(fs, program_id);
}

static bool mountSources(u64 program_id) {
    FsStorage storage;
    Result rc = fsOpenDataStorageByCurrentProcess(&storage);
    if (R_SUCCEEDED(rc)) {
        rc = romfsMountFromStorage(storage, 0, RO_DEVICE);
    }
    if (R_FAILED(rc)) {
        logLine("romfs mount rc=0x%08X", rc);
        return false;
    }

    FsFileSystem save;
    rc = openSave(program_id, &save);
    if (R_FAILED(rc) || fsdevMountDevice(RW_DEVICE, save) == -1) {
        logLine("save mount rc=0x%08X", rc);
        romfsUnmount(RO_DEVICE);
        return false;
    }

    g_ro = GetDeviceOpTab(RO_DEVICE ":");
    g_rw = GetDeviceOpTab(RW_DEVICE ":");
    g_rw_fs = fsdevGetDeviceFileSystem(RW_DEVICE);
    return g_ro != NULL && g_rw != NULL && g_rw_fs != NULL
        && strcmp(g_ro->name, RO_DEVICE) == 0
        && strcmp(g_rw->name, RW_DEVICE) == 0;
}

static void openLog(void) {
    char path[PATH_SIZE];
    mkdir("sdmc:/switch", 0777);
    mkdir("sdmc:/switch/carafe", 0777);
    if (format(path, sizeof(path), "sdmc:%s", g_volatile_root)) {
        mkdir(path, 0777);
    }
    if (format(g_log_path, sizeof(g_log_path), "sdmc:%s/carafe-overlay.log", g_volatile_root)) {
        rotateLog(_REENT, g_log_path);
        FILE* log = fopen(g_log_path, "w");
        if (log != NULL) {
            fclose(log);
            g_log_ready = true;
        }
    }
}

static void buildOverlay(void) {
    g_overlay = *g_base;
    g_overlay.structSize = sizeof(OverlayFile);
    g_overlay.dirStateSize = sizeof(OverlayDir);
    g_overlay.deviceData = NULL;
    g_overlay.open_r = overlayOpen;
    g_overlay.close_r = overlayClose;
    g_overlay.write_r = overlayWrite;
    g_overlay.read_r = overlayRead;
    g_overlay.seek_r = overlaySeek;
    g_overlay.fstat_r = overlayFstat;
    g_overlay.stat_r = overlayStat;
    g_overlay.lstat_r = overlayStat;
    g_overlay.link_r = overlayLink;
    g_overlay.unlink_r = overlayUnlink;
    g_overlay.chdir_r = overlayChdir;
    g_overlay.rename_r = overlayRename;
    g_overlay.mkdir_r = overlayMkdir;
    g_overlay.diropen_r = overlayDiropen;
    g_overlay.dirreset_r = overlayDirreset;
    g_overlay.dirnext_r = overlayDirnext;
    g_overlay.dirclose_r = overlayDirclose;
    g_overlay.statvfs_r = overlayStatvfs;
    g_overlay.ftruncate_r = overlayFtruncate;
    g_overlay.fsync_r = overlayFsync;
    g_overlay.chmod_r = overlayChmod;
    g_overlay.fchmod_r = NULL;
    g_overlay.rmdir_r = overlayRmdir;
    g_overlay.utimes_r = overlayUtimes;
    g_overlay.fpathconf_r = NULL;
    g_overlay.pathconf_r = NULL;
    g_overlay.symlink_r = NULL;
    g_overlay.readlink_r = NULL;
}

uint64_t carafeOverlayBytesRead(void) {
    return __atomic_load_n(&g_bytes_read, __ATOMIC_RELAXED);
}

bool carafeOverlayGameOpened(void) {
    return __atomic_load_n(&g_game_opened, __ATOMIC_RELAXED);
}

bool carafeOverlayReadRomfsText(const char* path, char* out, size_t size) {
    char full[PATH_SIZE];
    if (size == 0 || !format(full, sizeof(full), RO_DEVICE ":%s", path)) {
        return false;
    }

    FILE* file = fopen(full, "rb");
    if (file == NULL) {
        return false;
    }
    size_t got = fread(out, 1, size - 1, file);
    fclose(file);
    out[got] = '\0';
    return got > 0;
}

void* carafeOverlayReadRomfsFile(const char* path, size_t limit, size_t* size) {
    char full[PATH_SIZE];
    if (limit == 0 || !format(full, sizeof(full), RO_DEVICE ":%s", path)) {
        return NULL;
    }

    FILE* file = fopen(full, "rb");
    if (file == NULL) {
        return NULL;
    }
    uint8_t* data = malloc(limit);
    if (data == NULL) {
        fclose(file);
        return NULL;
    }
    size_t got = fread(data, 1, limit, file);
    bool whole = got < limit || fgetc(file) == EOF;
    fclose(file);
    if (got == 0 || !whole) {
        free(data);
        return NULL;
    }
    *size = got;
    return data;
}

void carafeOverlayLog(const char* line) {
    logLine("%s", line);
}

static void carafeOverlayShutdown(void) {
    if (!g_installed) {
        return;
    }

    g_stop = true;
    if (g_thread_running) {
        threadWaitForExit(&g_commit_thread);
        threadClose(&g_commit_thread);
        g_thread_running = false;
    }
    commitIfDue("at exit");
    if (g_exit_locked) {
        g_exit_locked = false;
        appletUnlockExit();
    }
    logLine("shutdown");
}

__attribute__((constructor(101))) static void carafeOverlayInstall(void) {
    int device = FindDevice("sdmc:");
    if (device < 0 || devoptab_list[device] == NULL) {
        return;
    }

    u64 program_id = 0;
    svcGetInfo(&program_id, InfoType_ProgramId, CUR_PROCESS_HANDLE, 0);
    snprintf(g_volatile_root, sizeof(g_volatile_root), "/switch/carafe/%016lx", program_id);

    if (!mountSources(program_id)) {
        logLine("not installed: sources are not mounted");
        return;
    }

    u64 index_start = armGetSystemTick();
    indexDirectory(_REENT, "");
    u64 index_ms = armTicksToNs(armGetSystemTick() - index_start) / 1000000;

    mutexInit(&g_commit_mutex);
    g_started = armGetSystemTick();
    g_base = devoptab_list[device];
    buildOverlay();
    devoptab_list[device] = &g_overlay;
    g_installed = true;

    openLog();
    char started[32] = "unknown";
    time_t now = time(NULL);
    struct tm* local = localtime(&now);
    if (local != NULL) {
        strftime(started, sizeof(started), "%Y-%m-%d %H:%M:%S", local);
    }
    logLine("started %s", started);
    logLine("program 0x%016lx, volatile root %s", program_id, g_volatile_root);
    logLine("devices: base %s, read-only %s, writable %s", g_base->name, g_ro->name, g_rw->name);
    logLine("romfs index: %zu entries in %lu ms", g_index_count, index_ms);

    if (atexit(carafeOverlayShutdown) != 0) {
        logLine("not started: exit handler is not registered");
        return;
    }

    Result rc = threadCreate(
        &g_commit_thread,
        commitLoop,
        NULL,
        g_commit_stack,
        sizeof(g_commit_stack),
        0x2C,
        -2
    );
    if (R_SUCCEEDED(rc)) {
        rc = threadStart(&g_commit_thread);
        if (R_SUCCEEDED(rc)) {
            g_thread_running = true;
        } else {
            threadClose(&g_commit_thread);
        }
    }
    Result lock_rc = g_thread_running ? appletLockExit() : MAKERESULT(Module_Libnx, LibnxError_NotInitialized);
    g_exit_locked = R_SUCCEEDED(lock_rc);
    logLine(
        "installed over sdmc:%s, commit thread %s (rc=0x%08X), exit lock rc=0x%08X",
        WINE_ROOT,
        g_thread_running ? "running" : "missing",
        rc,
        lock_rc
    );
}
