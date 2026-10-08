#include <stdio.h>
#include <string.h>

#include "app_exit.h"
#include "fixed_image.h"
#include "romfs_reader.h"

#define RELAUNCH_COUNTER "/carafe-relaunch"
#define RELAUNCH_MAX 4
#define LOG_NAME "carafe-loader.log"
#define ADDRESS_SPACE_4G 0x100000000ULL
#define PAGE_MASK 0xFFFULL

#define PE_MACHINE_I386 0x14C
#define PE_MAGIC_PE32 0x10B
#define PE_RELOCS_STRIPPED 0x0001
#define PE_DYNAMIC_BASE 0x0040
#define PE_RELOC_DIRECTORY 5

typedef struct {
    u64 base;
    u64 size;
} ImageRange;

static u64 g_programId;

static u16 readU16(const u8* bytes) {
    u16 value;
    memcpy(&value, bytes, sizeof(value));
    return value;
}

static u32 readU32(const u8* bytes) {
    u32 value;
    memcpy(&value, bytes, sizeof(value));
    return value;
}

static void logLine(const char* text, bool restart) {
    FsFileSystem sd;
    if (R_FAILED(fsOpenSdCardFileSystem(&sd))) {
        return;
    }

    char dir[64];
    char path[96];
    snprintf(dir, sizeof(dir), "/switch/carafe/%016lx", g_programId);
    snprintf(path, sizeof(path), "%s/" LOG_NAME, dir);
    fsFsCreateDirectory(&sd, "/switch");
    fsFsCreateDirectory(&sd, "/switch/carafe");
    fsFsCreateDirectory(&sd, dir);
    if (restart) {
        fsFsDeleteFile(&sd, path);
    }
    fsFsCreateFile(&sd, path, 0, 0);

    FsFile file;
    if (R_SUCCEEDED(fsFsOpenFile(&sd, path, FsOpenMode_Write | FsOpenMode_Append, &file))) {
        s64 size = 0;
        char line[192];
        const int length = snprintf(line, sizeof(line), "[carafe-loader] %s\n", text);
        if (R_SUCCEEDED(fsFileGetSize(&file, &size)) && length > 0) {
            fsFileWrite(&file, size, line, (u64)length, FsWriteOption_Flush);
        }
        fsFileClose(&file);
    }
    fsFsClose(&sd);
}

static bool gameImagePath(const char* argv, char* out, size_t out_size) {
    const char* start = strchr(argv, '"');
    const char* end = start != NULL ? strchr(start + 1, '"') : NULL;
    if (end == NULL || strncmp(start + 1, "sdmc:/", 6) != 0) {
        return false;
    }

    const char* path = start + 1 + strlen("sdmc:");
    const size_t length = (size_t)(end - path);
    if (length + 1 > out_size) {
        return false;
    }
    memcpy(out, path, length);
    out[length] = '\0';
    return true;
}

static bool readFixedImage(const char* argv, ImageRange* out) {
    char path[768];
    if (!gameImagePath(argv, path, sizeof(path))) {
        return false;
    }

    RomfsReader reader;
    if (R_FAILED(romfsReaderOpen(&reader))) {
        return false;
    }

    u64 offset = 0;
    u64 file_size = 0;
    u8 dos[0x40];
    u8 nt[4 + 20 + 96 + 8 * (PE_RELOC_DIRECTORY + 1)];
    bool read = R_SUCCEEDED(romfsReaderFind(&reader, path, &offset, &file_size))
        && file_size >= sizeof(dos)
        && R_SUCCEEDED(romfsReaderRead(&reader, offset, dos, sizeof(dos)))
        && dos[0] == 'M' && dos[1] == 'Z';
    const u32 nt_offset = read ? readU32(dos + 0x3C) : 0;
    read = read
        && nt_offset < file_size && file_size - nt_offset >= sizeof(nt)
        && R_SUCCEEDED(romfsReaderRead(&reader, offset + nt_offset, nt, sizeof(nt)));
    romfsReaderClose(&reader);

    if (!read || memcmp(nt, "PE\0\0", 4) != 0) {
        return false;
    }

    const u8* file_header = nt + 4;
    const u8* optional = file_header + 20;
    if (readU16(file_header) != PE_MACHINE_I386 || readU16(optional) != PE_MAGIC_PE32) {
        return false;
    }

    const u16 characteristics = readU16(file_header + 18);
    const u64 image_base = readU32(optional + 28);
    const u64 image_size = readU32(optional + 56);
    const u16 dll_characteristics = readU16(optional + 70);
    const u32 directory_count = readU32(optional + 92);
    const u32 relocations = directory_count > PE_RELOC_DIRECTORY
        ? readU32(optional + 96 + 8 * PE_RELOC_DIRECTORY + 4)
        : 0;
    const bool movable = (relocations != 0 && !(characteristics & PE_RELOCS_STRIPPED))
        || (dll_characteristics & PE_DYNAMIC_BASE);
    const u64 end = (image_base + image_size + PAGE_MASK) & ~PAGE_MASK;
    if (movable || image_size == 0 || end > ADDRESS_SPACE_4G) {
        return false;
    }

    out->base = image_base & ~PAGE_MASK;
    out->size = end - out->base;
    return true;
}

static bool addressSpaceIs32Bit(void) {
    u64 base = 0;
    u64 size = 0;
    return R_SUCCEEDED(svcGetInfo(&base, InfoType_AslrRegionAddress, CUR_PROCESS_HANDLE, 0))
        && R_SUCCEEDED(svcGetInfo(&size, InfoType_AslrRegionSize, CUR_PROCESS_HANDLE, 0))
        && base + size <= ADDRESS_SPACE_4G;
}

static bool findTaken(ImageRange image, MemoryInfo* taken) {
    u64 at = image.base;
    while (at < image.base + image.size) {
        u32 page_info = 0;
        memset(taken, 0, sizeof(*taken));
        if (R_FAILED(svcQueryMemory(taken, &page_info, at))) {
            taken->addr = at;
            return true;
        }
        if (taken->type != MemType_Unmapped || taken->addr + taken->size <= at) {
            return true;
        }
        at = taken->addr + taken->size;
    }
    return false;
}

static u32 readCounter(FsFileSystem* save) {
    u32 value = 0;
    u64 read = 0;
    FsFile file;
    if (R_SUCCEEDED(fsFsOpenFile(save, RELAUNCH_COUNTER, FsOpenMode_Read, &file))) {
        if (R_FAILED(fsFileRead(&file, 0, &value, sizeof(value), FsReadOption_None, &read)) || read != sizeof(value)) {
            value = 0;
        }
        fsFileClose(&file);
    }
    return value;
}

static Result writeCounter(FsFileSystem* save, u32 value) {
    if (value == 0) {
        fsFsDeleteFile(save, RELAUNCH_COUNTER);
        return fsFsCommit(save);
    }

    fsFsCreateFile(save, RELAUNCH_COUNTER, sizeof(value), 0);
    FsFile file;
    Result rc = fsFsOpenFile(save, RELAUNCH_COUNTER, FsOpenMode_Write, &file);
    if (R_SUCCEEDED(rc)) {
        rc = fsFileWrite(&file, 0, &value, sizeof(value), FsWriteOption_Flush);
        fsFileClose(&file);
    }
    return R_SUCCEEDED(rc) ? fsFsCommit(save) : rc;
}

void fixedImageClaim(const char* argv) {
    ImageRange image;
    if (!addressSpaceIs32Bit() || !readFixedImage(argv, &image)) {
        return;
    }

    svcGetInfo(&g_programId, InfoType_ProgramId, CUR_PROCESS_HANDLE, 0);
    FsFileSystem save;
    const bool has_save = R_SUCCEEDED(fsOpen_DeviceSaveData(&save, g_programId));
    const u32 attempt = has_save ? readCounter(&save) : 0;

    MemoryInfo taken;
    const bool is_taken = findTaken(image, &taken);
    char line[160];
    snprintf(line, sizeof(line), "start: fixed image 0x%lx-0x%lx, attempt %u, save data %s",
             image.base, image.base + image.size, attempt, has_save ? "open" : "missing");
    logLine(line, attempt == 0);
    if (!is_taken) {
        snprintf(line, sizeof(line), "0x%lx-0x%lx is free after %u restarts; reserved",
                 image.base, image.base + image.size, attempt);
        logLine(line, false);
        virtmemLock();
        virtmemAddReservation((void*)image.base, image.size);
        virtmemUnlock();
        if (has_save && attempt != 0) {
            writeCounter(&save, 0);
        }
    } else {
        snprintf(line, sizeof(line), "0x%lx-0x%lx is taken: 0x%lx+0x%lx type=%u perm=%u",
                 image.base, image.base + image.size, taken.addr, taken.size, taken.type, taken.perm);
        logLine(line, false);
        if (!has_save || attempt >= RELAUNCH_MAX) {
            logLine(has_save ? "still taken after 4 restarts; starting anyway" : "no save data to count restarts; starting anyway", false);
            if (has_save) {
                writeCounter(&save, 0);
            }
        } else if (R_FAILED(writeCounter(&save, attempt + 1))) {
            logLine("could not record the attempt; starting anyway", false);
        } else {
            snprintf(line, sizeof(line), "restarting the program (%u of %u)", attempt + 1, RELAUNCH_MAX);
            logLine(line, false);
            fsFsClose(&save);
            const Result rc = appRestartProgram(CUR_PROCESS_HANDLE);
            snprintf(line, sizeof(line), "the system did not restart the program (0x%x); starting anyway", rc);
            logLine(line, false);
            return;
        }
    }
    if (has_save) {
        fsFsClose(&save);
    }
}
