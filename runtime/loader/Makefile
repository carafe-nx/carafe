.SUFFIXES:

ifeq ($(strip $(DEVKITPRO)),)
$(error "Please set DEVKITPRO in your environment. export DEVKITPRO=<path to>/devkitpro")
endif

TOPDIR ?= $(CURDIR)
include $(DEVKITPRO)/libnx/switch_rules

TARGET		:=	carafe-loader
BUILD		:=	build
SOURCES		:=	source
INCLUDES	:=	source
HBLOADER	:=	../../external/nx-hbloader/source
CONFIG_JSON	:=	carafe-loader.json

APP_TITLE	:=	Carafe Loader Test
APP_AUTHOR	:=	Carafe
APP_VERSION	:=	0.1.0
APP_TITLEID	:=	05459df512e44000

ARCH	:=	-march=armv8-a+crc+crypto -mtune=cortex-a57 -mtp=soft -fPIE

CFLAGS	:=	-g -Wall -Wextra -O2 -ffunction-sections \
			$(ARCH) $(DEFINES)

CFLAGS	+=	$(INCLUDE) -D__SWITCH__ -DVERSION='"$(APP_VERSION)"'

ASFLAGS	:=	-g $(ARCH)
LDFLAGS	=	-specs=$(DEVKITPRO)/libnx/switch.specs -g $(ARCH) -Wl,-wrap,exit -Wl,-Map,$(notdir $*.map)

LIBS	:=	-lnx

LIBDIRS	:=	$(PORTLIBS) $(LIBNX)

ifneq ($(BUILD),$(notdir $(CURDIR)))

export OUTPUT	:=	$(CURDIR)/$(BUILD)/$(TARGET)
export TOPDIR	:=	$(CURDIR)

export VPATH	:=	$(foreach dir,$(SOURCES),$(CURDIR)/$(dir)) $(CURDIR)/$(HBLOADER)
export DEPSDIR	:=	$(CURDIR)/$(BUILD)

CFILES		:=	$(foreach dir,$(SOURCES),$(notdir $(wildcard $(dir)/*.c)))
SFILES		:=	trampoline.s

export LD	:=	$(CC)
export OFILES	:=	$(CFILES:.c=.o) $(SFILES:.s=.o)

export INCLUDE	:=	$(foreach dir,$(INCLUDES),-I$(CURDIR)/$(dir)) \
			$(foreach dir,$(LIBDIRS),-I$(dir)/include) \
			-I$(CURDIR)/$(BUILD)

export LIBPATHS	:=	$(foreach dir,$(LIBDIRS),-L$(dir)/lib)

export APP_JSON	:=	$(TOPDIR)/$(CONFIG_JSON)
export NACPFLAGS	+=	--titleid=$(APP_TITLEID)

.PHONY: $(BUILD) clean all

all: $(BUILD)

$(BUILD):
	@[ -d $@ ] || mkdir -p $@
	@$(MAKE) --no-print-directory -C $(BUILD) -f $(CURDIR)/Makefile

clean:
	@echo clean ...
	@rm -fr $(BUILD)

else

.PHONY: all

DEPENDS	:=	$(OFILES:.o=.d)

all	:	$(OUTPUT).nso $(OUTPUT).npdm $(OUTPUT).nacp

$(OUTPUT).nso	:	$(OUTPUT).elf

$(OUTPUT).nacp	:	$(MAKEFILE_LIST)
	@nacptool --create "$(APP_TITLE)" "$(APP_AUTHOR)" "$(APP_VERSION)" $@ $(NACPFLAGS)
	@echo built ... $(notdir $@)

$(OUTPUT).elf	:	$(OFILES)

-include $(DEPENDS)

endif
