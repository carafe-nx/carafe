ROOT := $(abspath $(CURDIR)/../..)
SRC := $(ROOT)/external/hacbrewpack
BUILD := $(CURDIR)/build
OUT ?= $(ROOT)/app/crates/carafe-desktop/binaries

ARCHES := aarch64 x86_64
EXES := $(foreach arch,$(ARCHES),$(OUT)/hacbrewpack-$(arch)-pc-windows-msvc.exe)

.PHONY: all fetch clean

all: $(EXES)

fetch:
	git -C $(ROOT) submodule update --init --depth 1 external/hacbrewpack

$(OUT)/hacbrewpack-%-pc-windows-msvc.exe: hacbrewpack.manifest manifest.rc | fetch
	rm -rf $(BUILD)/$*
	mkdir -p $(BUILD)/$* $(OUT)
	cp -r $(SRC)/. $(BUILD)/$*/
	cp hacbrewpack.manifest manifest.rc $(BUILD)/$*/
	cd $(BUILD)/$* && $*-w64-mingw32-windres manifest.rc -O coff -o manifest.o
	printf 'CC = %s\nCFLAGS = -O2 -Wall -Wextra -std=gnu11\nLDFLAGS = manifest.o -static -lmbedtls -lmbedx509 -lmbedcrypto\n' \
		$*-w64-mingw32-clang > $(BUILD)/$*/config.mk
	$(MAKE) -C $(BUILD)/$*/mbedtls lib CC=$*-w64-mingw32-clang AR=$*-w64-mingw32-ar WINDOWS=1 \
		CFLAGS='-O2 -include sys/time.h'
	$(MAKE) -C $(BUILD)/$* hacbrewpack CC=$*-w64-mingw32-clang
	cp $(BUILD)/$*/hacbrewpack.exe $@

clean:
	rm -rf $(BUILD) $(EXES)
