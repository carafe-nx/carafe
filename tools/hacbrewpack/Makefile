ROOT := $(abspath $(CURDIR)/../..)
SRC := $(ROOT)/external/hacbrewpack
BUILD := $(CURDIR)/build
OUT ?= $(ROOT)/app/crates/carafe-desktop/binaries

ARCHES := aarch64 x86_64
EXES := $(foreach arch,$(ARCHES),$(OUT)/hacbrewpack-$(arch)-pc-windows-msvc.exe)
PATCHES := $(sort $(wildcard $(CURDIR)/patches/*.patch))
HOST ?= $(shell rustc -vV | sed -n 's/^host: //p')
NATIVE := $(OUT)/hacbrewpack-$(HOST)

.PHONY: all fetch native clean

all: $(EXES)

fetch:
	git -C $(ROOT) submodule update --init --depth 1 external/hacbrewpack

$(OUT)/hacbrewpack-%-pc-windows-msvc.exe: hacbrewpack.manifest manifest.rc $(PATCHES) | fetch
	rm -rf $(BUILD)/$*
	mkdir -p $(BUILD)/$* $(OUT)
	cp -r $(SRC)/. $(BUILD)/$*/
	rm -f $(BUILD)/$*/.git
	for patch in $(PATCHES); do git -C $(BUILD)/$* apply $$patch || exit 1; done
	cp hacbrewpack.manifest manifest.rc $(BUILD)/$*/
	cd $(BUILD)/$* && $*-w64-mingw32-windres manifest.rc -O coff -o manifest.o
	printf 'CC = %s\nCFLAGS = -O2 -Wall -Wextra -std=gnu11\nLDFLAGS = manifest.o -static -lmbedtls -lmbedx509 -lmbedcrypto\n' \
		$*-w64-mingw32-clang > $(BUILD)/$*/config.mk
	$(MAKE) -C $(BUILD)/$*/mbedtls lib CC=$*-w64-mingw32-clang AR=$*-w64-mingw32-ar WINDOWS=1 \
		CFLAGS='-O2 -include sys/time.h'
	$(MAKE) -C $(BUILD)/$* hacbrewpack CC=$*-w64-mingw32-clang
	cp $(BUILD)/$*/hacbrewpack.exe $@

native: $(NATIVE)

$(NATIVE): hacbrewpack.manifest manifest.rc $(PATCHES) | fetch
	rm -rf $(BUILD)/native
	mkdir -p $(BUILD)/native $(OUT)
	cp -r $(SRC)/. $(BUILD)/native/
	rm -f $(BUILD)/native/.git
	for patch in $(PATCHES); do git -C $(BUILD)/native apply $$patch || exit 1; done
	cp hacbrewpack.manifest manifest.rc $(BUILD)/native/
	printf 'CC = %s\nCFLAGS = -O2 -Wall -Wextra -std=gnu11 -fPIC\nLDFLAGS = -lmbedtls -lmbedx509 -lmbedcrypto\n' \
		"$${CC:-cc}" > $(BUILD)/native/config.mk
	$(MAKE) -C $(BUILD)/native/mbedtls lib CC="$${CC:-cc}" AR="$${AR:-ar}"
	$(MAKE) -C $(BUILD)/native hacbrewpack CC="$${CC:-cc}"
	cp $(BUILD)/native/hacbrewpack $@

clean:
	rm -rf $(BUILD) $(EXES) $(NATIVE)
