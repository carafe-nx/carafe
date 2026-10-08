ROOT := $(abspath $(CURDIR)/..)
AUTORUN := $(ROOT)/external/autorun
WINE := $(AUTORUN)/horizon-wine
BUILD := $(CURDIR)/build
PE := $(BUILD)/wine-pe
SWITCH := $(BUILD)/wine-switch
NPDM := $(BUILD)/npdm
OUT ?= $(ROOT)/app/crates/carafe-desktop/resources/runtime

JOBS ?= $(shell nproc)
TITLE_ID ?= 055d76078fd54000
ADDRESS_SPACE ?= 39
SAVE_SIZE ?= 0x10000000
SAVE_JOURNAL_SIZE ?= 0x4000000

OVERLAY_OBJECTS := $(SWITCH)/CMakeFiles/wine-nx-runtime.dir$(CURDIR)/overlay

.PHONY: all fetch check-image headers wine loader layout clean

all: layout

fetch:
	git -C $(ROOT) submodule update --init external/autorun external/nx-hbloader
	git -C $(AUTORUN) submodule update --init horizon-dlls
	sh $(WINE)/tools/bootstrap-box64-core.sh
	sh $(WINE)/tools/bootstrap-wma.sh

check-image: fetch
	@image="$$(sed -n 's/^FROM \([^@]*\).*/\1/p' $(ROOT)/docker/Dockerfile)"; \
	autorun="$$(cat $(WINE)/switch-dev.txt)"; \
	if [ "$$image" != "$$autorun" ]; then \
		echo "docker/Dockerfile uses $$image, Autorun needs $$autorun" >&2; \
		exit 1; \
	fi

$(PE)/Makefile: | check-image
	mkdir -p $(PE)
	cd $(PE) && $(AUTORUN)/configure --enable-archs=aarch64,arm64ec,i386 \
		--enable-winebox64=aarch64 --enable-winebox64ec=arm64ec \
		--disable-tests --without-x --without-freetype --without-alsa --without-pulse \
		--without-dbus --without-fontconfig --without-udev --without-usb \
		--without-gstreamer --enable-winegstreamer --without-vulkan

headers: $(PE)/Makefile
	$(MAKE) -C $(PE) -j$(JOBS) include/all

$(SWITCH)/build.ninja: | headers
	cmake -S $(WINE) -B $(SWITCH) -G Ninja \
		-DCMAKE_TOOLCHAIN_FILE=$(WINE)/cmake/switch-devkitA64.cmake \
		-DWINE_NX_PE_BUILD_DIR=$(PE) -DWINE_NX_AMD64=ON -DWINE_NX_FEX=ON \
		-DWINE_NX_BOX64_INTERPRETER=ON -DWINE_NX_BOX64_DYNAREC=ON \
		-DWINE_NX_USB_STORAGE=ON -DWINE_NX_BOOT_BUNDLE= -DCMAKE_BUILD_TYPE=Release \
		-DCMAKE_PROJECT_INCLUDE=$(CURDIR)/overlay/carafe-overlay.cmake \
		"-DCARAFE_OVERLAY_DEFINITIONS=CARAFE_SAVE_SIZE=$(SAVE_SIZE);CARAFE_SAVE_JOURNAL_SIZE=$(SAVE_JOURNAL_SIZE)"

wine: headers $(SWITCH)/build.ninja
	rm -rf $(OVERLAY_OBJECTS)
	cmake --build $(SWITCH) --target wine-nx-runtime-nro -j $(JOBS)
	rm -rf $(SWITCH)/stage
	python3 $(WINE)/tools/package-amd64.py --build $(SWITCH) --fex --vulkan --dxvk --vkd3d \
		--stage-output $(SWITCH)/stage

loader: fetch
	$(MAKE) -B -C loader

layout: wine loader
	rm -rf $(OUT) $(NPDM)
	mkdir -p $(OUT)/exefs $(OUT)/control $(OUT)/logo $(OUT)/symbols $(OUT)/romfs/switch \
		$(OUT)/romfs/carafe/loading $(NPDM)
	python3 -c 'import json, sys; src, dst, tid, space = sys.argv[1:]; n = json.load(open(src)); \
		t = "0x" + tid; n["name"] = "Application"; \
		n["title_id"] = n["title_id_range_min"] = n["title_id_range_max"] = t; \
		n["address_space_type"] = {"39": 3, "32": 2}[space]; n["system_resource_size"] = "0x1000000"; \
		json.dump(n, open(dst, "w"), indent=4)' \
		$(CURDIR)/loader/carafe-loader.json $(NPDM)/carafe-runtime.json $(TITLE_ID) $(ADDRESS_SPACE)
	npdmtool $(NPDM)/carafe-runtime.json $(OUT)/exefs/main.npdm 2>&1 | grep -v 'field not present' || true
	test -s $(OUT)/exefs/main.npdm
	cp loader/build/carafe-loader.nso $(OUT)/exefs/main
	cp $(DEVKITPRO)/libnx/default_icon.jpg $(OUT)/control/icon_AmericanEnglish.dat
	cp $(ROOT)/assets/switch/NintendoLogo.png $(ROOT)/assets/switch/StartupMovie.gif $(OUT)/logo/
	cp -r $(AUTORUN)/horizon-dlls/switch/. $(OUT)/romfs/switch/
	cp -r $(SWITCH)/stage/switch/. $(OUT)/romfs/switch/
	cp $(SWITCH)/wine-nx-runtime.elf $(OUT)/symbols/
	cp $(ROOT)/assets/switch/NintendoLogo.png $(ROOT)/assets/switch/StartupMovie.gif \
		$(OUT)/romfs/carafe/loading/
	@echo "runtime: $$(find $(OUT)/romfs -type f | wc -l) files in romfs, $$(du -sh $(OUT) | cut -f1) in $(OUT)"

clean:
	rm -rf $(BUILD) $(OUT)
	$(MAKE) -C loader clean
