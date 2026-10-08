#include "romfs_reader.h"

#include <string.h>

#define ROMFS_NONE UINT32_MAX
#define ROMFS_NAME_MAX 0x300

typedef struct {
    romfs_dir entry;
    char name[ROMFS_NAME_MAX];
} DirEntry;

typedef struct {
    romfs_file entry;
    char name[ROMFS_NAME_MAX];
} FileEntry;

static Result notFound(void) {
    return MAKERESULT(Module_Libnx, LibnxError_NotFound);
}

static Result readEntry(
    RomfsReader* reader,
    u64 table_offset,
    u64 table_size,
    u32 offset,
    void* out,
    size_t fixed_size,
    const u32* name_len
) {
    if (offset >= table_size || table_size - offset < fixed_size) {
        return notFound();
    }

    Result rc = fsStorageRead(&reader->storage, table_offset + offset, out, fixed_size);
    if (R_FAILED(rc)) {
        return rc;
    }

    if (*name_len > ROMFS_NAME_MAX || table_size - offset - fixed_size < *name_len) {
        return notFound();
    }

    return fsStorageRead(&reader->storage, table_offset + offset + fixed_size, (u8*)out + fixed_size, *name_len);
}

static bool nameEquals(const char* name, u32 name_len, const char* component, size_t component_len) {
    return name_len == component_len && memcmp(name, component, component_len) == 0;
}

static Result findChildDir(RomfsReader* reader, u32 parent, const char* component, size_t len, u32* out) {
    DirEntry dir;
    const u64 table_offset = reader->header.dirTableOff;
    const u64 table_size = reader->header.dirTableSize;

    Result rc = readEntry(reader, table_offset, table_size, parent, &dir, sizeof(dir.entry), &dir.entry.nameLen);
    if (R_FAILED(rc)) {
        return rc;
    }

    for (u32 child = dir.entry.childDir; child != ROMFS_NONE; child = dir.entry.sibling) {
        rc = readEntry(reader, table_offset, table_size, child, &dir, sizeof(dir.entry), &dir.entry.nameLen);
        if (R_FAILED(rc)) {
            return rc;
        }
        if (nameEquals(dir.name, dir.entry.nameLen, component, len)) {
            *out = child;
            return 0;
        }
    }

    return notFound();
}

static Result findChildFile(RomfsReader* reader, u32 parent, const char* component, size_t len, FileEntry* out) {
    DirEntry dir;
    Result rc = readEntry(
        reader,
        reader->header.dirTableOff,
        reader->header.dirTableSize,
        parent,
        &dir,
        sizeof(dir.entry),
        &dir.entry.nameLen
    );
    if (R_FAILED(rc)) {
        return rc;
    }

    const u64 table_offset = reader->header.fileTableOff;
    const u64 table_size = reader->header.fileTableSize;
    for (u32 child = dir.entry.childFile; child != ROMFS_NONE; child = out->entry.sibling) {
        rc = readEntry(reader, table_offset, table_size, child, out, sizeof(out->entry), &out->entry.nameLen);
        if (R_FAILED(rc)) {
            return rc;
        }
        if (nameEquals(out->name, out->entry.nameLen, component, len)) {
            return 0;
        }
    }

    return notFound();
}

Result romfsReaderOpen(RomfsReader* reader) {
    Result rc = fsOpenDataStorageByCurrentProcess(&reader->storage);
    if (R_FAILED(rc)) {
        return rc;
    }

    rc = fsStorageRead(&reader->storage, 0, &reader->header, sizeof(reader->header));
    if (R_SUCCEEDED(rc) && reader->header.headerSize != sizeof(reader->header)) {
        rc = MAKERESULT(Module_Libnx, LibnxError_BadInput);
    }
    if (R_FAILED(rc)) {
        fsStorageClose(&reader->storage);
    }
    return rc;
}

void romfsReaderClose(RomfsReader* reader) {
    fsStorageClose(&reader->storage);
}

Result romfsReaderFind(RomfsReader* reader, const char* path, u64* data_offset, u64* data_size) {
    if (path[0] != '/') {
        return notFound();
    }

    u32 dir = 0;
    const char* component = path + 1;
    for (;;) {
        const char* slash = strchr(component, '/');
        if (slash == NULL) {
            break;
        }

        Result rc = findChildDir(reader, dir, component, (size_t)(slash - component), &dir);
        if (R_FAILED(rc)) {
            return rc;
        }
        component = slash + 1;
    }

    FileEntry file;
    Result rc = findChildFile(reader, dir, component, strlen(component), &file);
    if (R_FAILED(rc)) {
        return rc;
    }

    *data_offset = file.entry.dataOff;
    *data_size = file.entry.dataSize;
    return 0;
}

Result romfsReaderRead(RomfsReader* reader, u64 data_offset, void* out, u64 size) {
    return fsStorageRead(&reader->storage, reader->header.fileDataOff + data_offset, out, size);
}
