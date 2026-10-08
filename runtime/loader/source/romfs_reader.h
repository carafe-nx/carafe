#pragma once

#include <switch.h>

typedef struct {
    FsStorage storage;
    romfs_header header;
} RomfsReader;

Result romfsReaderOpen(RomfsReader* reader);
void romfsReaderClose(RomfsReader* reader);
Result romfsReaderFind(RomfsReader* reader, const char* path, u64* data_offset, u64* data_size);
Result romfsReaderRead(RomfsReader* reader, u64 data_offset, void* out, u64 size);
