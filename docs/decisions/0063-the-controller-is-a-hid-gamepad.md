# 63. The controller is a HID gamepad

Date: 2026-11-05

## Context

On Windows a game finds a controller in one of four ways: XInput, DirectInput 8, Windows.Gaming.Input or raw HID
access. Only XInput talks to the Xbox driver directly; the other three see the controller as a HID device that the
Plug and Play manager announces: an entry under `HKLM\System\CurrentControlSet\Enum`, a device interface under
`Control\DeviceClasses`, and a device file that answers HID ioctls and gives one input report per read.

Wine does the same with drivers in a separate process: `winebus.sys` turns an SDL or udev controller into a HID
device, `winexinput.sys` shows an Xbox pad twice, as `&IG_` (the HID gamepad that Windows' XUSB driver shows, with
both triggers on one axis) and as `&XI_` (Wine's own device with every axis, which XInput and Windows.Gaming.Input
use), and `hidclass.sys` serves the reads and ioctls. `services.exe`, `plugplay` and `winedevice.exe` run them.

Autorun runs one process and keeps the server inside it (`dlls/ntdll/unix/horizon.c`); there is no `fork` on Horizon,
so the service processes never start (`NtCreateUserProcess` cannot spawn them), and a 32-bit game could not load the
64-bit drivers into its own process either. Autorun gives the pad to XInput directly (`horizon-wine/source/
xinput_unix.c`) and to DirectInput through a device of its own (`dlls/dinput/joystick_nx.c`), which replaces the HID
path of DirectInput (`dlls/dinput/dinput.c:291`, `388` at Autorun `636913ca`). No HID device exists, so:

- Windows.Gaming.Input, which builds its controllers from the `&XI_` HID interfaces through DirectInput
  (`dlls/windows.gaming.input/main.c:69-98`, `provider.c:759-812`), finds none. Cat Mail Co (Unity) loads only
  `windows.gaming.input.dll` for the pad, and Megabonk reads it through Rewired on Windows.Gaming.Input;
- a DirectInput 8 game sees Autorun's device, which has no HID objects and is not the device a Windows PC shows.

Autorun's server already plays the kernel for two Wine drivers: `\Device\Afd` (sockets) and `\Device\Nsi` are
objects of the in-process server whose ioctls it answers itself.

## Decision

- **The pad is a HID device of the in-process kernel.** The overlay's HID driver (`runtime/overlay/carafe_hid.c`)
  shows player 1 as Wine's `winexinput` shows an Xbox 360 pad, vendor `045E`, product `028E`:
  `HID\VID_045E&PID_028E&IG_00\0000` under the HID interface class and `HID\VID_045E&PID_028E&XI_00\0000` under
  Wine's XInput class. The `&IG_` descriptor is `winexinput`'s (`dlls/winexinput.sys/main.c:56-108`), with its report
  layout; the `&XI_` descriptor is built by `winebus`'s own builder (`dlls/winebus.sys/hid.c`,
  `hid_device_add_gamepad`), compiled into the runtime, so both match what Wine shows on Linux. The preparsed data
  comes from `hidparse.sys`'s `HidP_GetCollectionDescription`, compiled into the runtime too, in the layout Wine's
  `hid.dll` reads; it holds no pointers and is the same for 32-bit and 64-bit programs.
- **The server keeps device files** (`0128`): a name the driver knows (`\??\HID#…`, as `hid.dll`, DirectInput and
  raw input open it) gives a device object; its ioctls go to the driver; `ReadFile` takes the oldest queued report or
  becomes an overlapped operation that the next report completes; the object is signalled when its I/O ends, so a
  synchronous `ReadFile`, which waits on the file, works too. A read or write done at once fills the status block
  (`0129`), as the Switch branch of `ntdll` already did for ioctls.
- **Reads follow `hidclass.sys`** (`dlls/hidclass.sys/device.c`): each open has its own queue of 32 reports (2–512
  through `IOCTL_SET_NUM_DEVICE_INPUT_BUFFERS`), the oldest is dropped when it is full, a read shorter than the input
  report fails with `STATUS_INVALID_BUFFER_SIZE`. Product string, collection information and descriptor, input report
  on demand and poll frequency answer as `hidclass.sys` does; the device has no feature or output reports.
- **The reports carry the XInput state** (`0130`): the bindings, the floating keyboard and the keyboard mode are applied
  once, in `xinput_unix.c`, so XInput, DirectInput and Windows.Gaming.Input see the same pad. The runtime's input
  thread, which already polls the controller every 16 ms for the cursor (`dlls/win32u/winnx_drv.c:90-104`), hands
  the state to the driver (`0131`); a report is sent when the state changed, as a pad does. A new open starts with the
  current report in its queue, so a program does not see the axes in a corner until the first change.
- **The Plug and Play entries are written when the registry is loaded** (`runtime/overlay/carafe_registry.c`), and
  only when the controls are in controller mode; in keyboard and mouse mode the pad is no controller, as XInput
  already says. They are laid out as win32u lays out the monitors (`dlls/win32u/sysparams.c:1079-1105`): the
  interface key under `Control\DeviceClasses` is an ordinary key, its `#` and `#\Control` (with `Linked`) are
  volatile, and so is the device's instance key under `Enum`, so nothing of a device is left after the session.
  Only a key itself is made volatile (`0132`); a parent it lacks is an ordinary key, since a volatile `Enum` or
  `DeviceClasses` would refuse the ordinary keys win32u writes there for the monitors, and without monitors DXVK
  finds no adapter.
- **DirectInput finds joysticks among the HID devices again** (`runtime/patches/wine-pe/0004`): `dinput.dll` and
  `dinput8.dll` are built from Autorun's source without the `joystick_nx` path and replace the prebuilt ones
  (`runtime/wine-pe/build-pe.sh` now builds a patched source into every DLL that compiles it).

In the lab (2026-11-05) the gamepad probe of Carafe Lab (`tools/probes/gamepad.c`), 32-bit and 64-bit, finds both
devices through SetupAPI, DirectInput 8 shows `Controller (XBOX 360 For Windows)` as a gamepad with 5 axes, 10
buttons and a hat, and Windows.Gaming.Input one `Gamepad` and one `RawGameController`; every Switch button, both
sticks, both triggers and a chord show in XInput, DirectInput 8 and Windows.Gaming.Input within the same poll, mapped
as on Windows. Cat Mail Co moves through its menus with the pad and shows Xbox button prompts; Megabonk's menus and
its run (moving and turning) answer the pad. Indiana Jones opens the `&IG_` device and reads its reports, but its
title screen answers only the keyboard bindings it starts with. Raw input still lists no HID device
(`IOCTL_HID_GET_WINE_RAWINPUT_HANDLE` is not answered), and the pad has no rumble.

## Rejected

- **Running Wine's driver stack** (`services.exe`, `plugplay`, `winedevice.exe` with `winebus.sys`, `winexinput.sys`,
  `hidclass.sys`). It needs processes Horizon cannot start; hosting the drivers inside the game would put 64-bit
  drivers into 32-bit games and a second kernel API (`ntoskrnl.exe`) beside the program.
- **Giving Windows.Gaming.Input a device of its own**, as `joystick_nx` does for DirectInput. Each API would get a
  different pad, and programs that open the HID device themselves would still find nothing.
