# Carafe's loading screen without black pauses

Why the loading screen used to disappear into black for most of a game's start-up, and how Carafe keeps it on
screen until the game's first frame. The analysis was done on 2026-10-08 against the sources listed below, with
experiments on a host.

**Outcome.** The solution went into Carafe as four runtime patches —
[`0008`](../../../runtime/patches/autorun/0008-runtime-screen-until-first-frame.patch),
[`0009`](../../../runtime/patches/autorun/0009-winnx-vulkan-reserve-screen.patch),
[`0010`](../../../runtime/patches/autorun/0010-vulkan-take-screen-at-swapchain.patch),
[`0011`](../../../runtime/patches/autorun/0011-winnx-opengl-first-frame.patch) — and three changes in
[`runtime/overlay/carafe_loading.c`](../../../runtime/overlay/carafe_loading.c): `__nx_applet_auto_notifyrunning`
is defined as `false`, `appletNotifyRunning` is called once the second copy of the picture is queued (in
`__wrap_wine_nx_osk_copy`), and the "windows draw at ≥ 20 frames/s" rule in `checkFinish` is off while a Vulkan
surface holds the screen in reserve. Confirmed on the console on 2026-10-08: no black pauses before the game's
first frame.

**Checked against our autorun clone and data (2026-10-08):**

- confirmed: Autorun hands the screen to the game as soon as a Vulkan surface is created —
  `nx_vulkan_surface_create` → `wine_nx_gl_acquire_window` (`dlls/win32u/winnx_vulkan.c:105`) →
  `wine_nx_compositor_suspend` (`horizon-wine/source/runtime.c:693`), log line `[NXGL] screen handed to an
  OpenGL surface` (`:695`) and `[NXVK] … a Vulkan surface has the screen` (`winnx_vulkan.c:118`) — the same lines
  appear in the log of a recorded NFSU launch at ~12 s, followed by `frames=12` until ~38 s and a black screen
  on the video;
- confirmed: the console's empty frame at start-up (`runtime.c:3516-3519`), `hold_thread_local_pages` (`:4077`),
  the `p_vkCreateSwapchainKHR` call (`dlls/win32u/vulkan.c:3170`);
- confirmed (libnx@146c3d14, `native_window.c`): `nwindowConfigureBuffer` fails with `AlreadyInitialized` if the
  slot is already configured; `nwindowSetDimensions` fails if any slot is configured; `nwindowReleaseBuffers`
  disconnects the queue (`_nwindowDisconnect`);
- confirmed (`danfromtico/mesa-switch@d4a00ea0`, pinned in `switch-dev@f20b202:Dockerfile`):
  `wsi_CreateViSurfaceNN` only stores the window (`surface->window = pCreateInfo->window`); the window's size and
  buffers are set when the swapchain is created; line numbers differ slightly, the content matches;
- all four patches `0008`–`0011` pass `git apply --check` on our `external/autorun` and do not touch the files of
  Carafe's patches `0001`–`0007`;
- a risk the analysis named and our code confirms: the overlay's "windows draw at ≥ 20 frames/s" rule
  (`checkFinish` in `runtime/overlay/carafe_loading.c`) applies while there are no OpenGL/Vulkan frames. In the
  surface reserve phase the compositor now keeps running, so the rule could hide the loading screen before the
  first frame; it must be off while a surface holds the window in reserve (done in Carafe);
- not checked (closed code, console only): the system splash stays until `appletNotifyRunning` with no timeout,
  HOME during the splash; nvc0 and NVK working at the same time in the reserve phase.

Versions behind every reference: `autorun@636913c`, `mesa-switch@d4a00ea0`, `libnx@146c3d14` (both pinned in
`switch-dev@f20b202:Dockerfile:14,16`, which built the `2026.10.01` image), `dxvk@b1a1c99` (v3.1.1, by
`autorun-horizon-dlls@5d6eccb:switch/wine/drive_c/dxvk64/dxvk-manifest.json`), `vkd3d-proton@3b10bd7a` (3.0.1),
`Atmosphere@36cc9a9f`. `runtime.c` line numbers are for the original file at 636913c unless marked "patch".

The analysis held the solution to these requirements: no second display mechanism (one task, one implementation),
no new picture, timers or waits, and no per-frame cost after the game's first frame.

## 1. Summary

1. **There is a solution, one for all games.** The screen stays with Autorun's compositor, showing Carafe's
   picture, until the game's driver *really* takes the window. For Vulkan that is swapchain creation, not surface
   creation, and DXVK and VKD3D-Proton create the swapchain inside their first Present. For OpenGL, one frame of
   the same picture is put into the new surface at the moment it takes the window. The NSP's system splash stays
   up until the loading screen's first frame.
2. **Size:** four patches to Autorun, 101 changed lines (`runtime.c`, `winnx_vulkan.c`, `vulkan.c`,
   `winnx_opengl.c`), and an overlay change of about 40 lines with one hook point. No new picture, timers or
   waits. After the game's first frame not a single instruction is added per frame.
3. **Proven by code and experiments.** A model built from the real `native_window.c` and `compositor.c` ran 11
   scenarios, and every branch of the patch executed. The GL first-frame code passed 19 checks on Mesa. The chain
   for NFSU (DXVK) is closed in the sources: Mesa NVK does not touch the window before `vkCreateSwapchainKHR`, and
   DXVK calls it from its first `Present`.
4. **Where the window still changes hands before the first frame:** (a) Vulkan programs that create their own
   swapchain long before their first present (DXVK and VKD3D are not among them); until the present it is black,
   as before. (b) At the handover there is a black gap while buffers are created and the first frame is drawn,
   estimated at 1–3 display frames. Removing it completely would need a second vi layer or a Mesa change; both are
   covered below.
5. **Confidence:** high for the logic and call order, all checked in code and in the model. Medium for two
   properties of closed code: that Horizon (am) drops the splash exactly on `NotifyRunning`, that HOME works
   during the splash and there is no timeout; and that vi blanks the layer when the producer disconnects. Both are
   checked in 2 launches (section 4).

## 2. All directions

| Direction | Verdict | Key reference | Reason |
|---|---|---|---|
| Keep the screen with the compositor until the first present: **Vulkan, surface → swapchain** | **Works** (part of the solution) | `mesa-switch@d4a00ea0:src/vulkan/wsi/wsi_common_switch.c:1219-1238` | `vkCreateViSurfaceNN` only stores a pointer: `surface->window = pCreateInfo->window;`. Surface queries only read `nwindowGetDimensions` (`:127-168`, `:291-312`). |
| The same, **swapchain → first present** | **Does not work** without changing Mesa | `wsi_common_switch.c:1140,610`; `libnx@146c3d14:nx/source/display/native_window.c:125-126,179-182` | The swapchain registers its buffers at creation (`nwindowConfigureBuffer`), and libnx refuses while the compositor's buffers are registered: `if (nw->slots_configured & (1UL << slot)) ... LibnxError_AlreadyInitialized`. Model, scenario 10: `nwindowSetDimensions FAILED`, 3× `configure FAILED`, the game never shows. |
| The same for **DXVK / VKD3D** | **Works**: the swapchain is created inside the first present | `dxvk@b1a1c99:src/dxvk/dxvk_presenter.cpp:36-37,81-125`; `src/d3d9/d3d9_swapchain.cpp:860`; `vkd3d-proton@3b10bd7a:libs/vkd3d/swapchain.c:3031,2784-2788` | The surface is created in the `Presenter` constructor (`createSurface()`), the swapchain in `acquireNextImage` → `recreateSwapChain()` on the first `PresentImage`. |
| The same, **OpenGL (EGL)** | **Does not work** as a delay; "a frame of the picture in the new surface" **works** (part of the solution) | `mesa-switch:src/egl/drivers/switch/egl_switch.c:955`; `autorun:dlls/win32u/opengl.c:2370,2096` | `eglCreateWindowSurface` registers buffers right away, and win32u creates the surface already at `SetPixelFormat`. It cannot be delayed, so one frame of the picture goes into the surface that took the window (E3). |
| The same, **software rendering (GDI)** | **Works unchanged**: the screen stays with the compositor anyway | `autorun:dlls/win32u/winnx_drv.c:926` | Windows are compositor layers; the loading screen is drawn over them. |
| **Own `vi:m` layer** (`rc=0x1272`) | **Works under conditions, not recommended** | `libnx:nx/source/services/vi.c:35,41-43`; `nx/source/display/default_window.c:23-25`; `Atmosphere:libvapours/.../vi_results.hpp:20` | `0x1272` = module 114 (vi, confirmed by Atmosphère), description 9. Atmosphère does not describe 9 (only 1, 6, 7), so that is not checked. What is proven: `viInitialize(Default)` already takes **vi:m** (`smGetService(&root_srv, "vi:m")`). A second `viInitialize(Manager)` under `NX_GENERATE_SERVICE_GUARD` returns the same session, where `default_window.c:25` has already opened the "Default" display, and a second `viOpenDisplay` goes to that same session. Hence the guess: the error means "display already open". It could only be avoided by reusing the display libnx opened. But that is a second display mechanism, a second GPU thread next to a GL game (`compositor.h:10-12`: "nouveau is not safe with two threads drawing at once"), and closed Z/visibility semantics. |
| **System splash until the game's first frame** | **Works only as a bridge** to the loading screen's first frame (part of the solution); keeping it until the game's frame **does not fit** | `libnx:nx/source/services/applet.c:10,310-311,2099-2106`; `nx/include/switch/services/applet.h:1507` | `appletNotifyRunning` — "Notify that the app is now running, for the Application logo screen". The splash is the static `NintendoLogo.png`/`StartupMovie.gif` from the NSP's logo section: no live text on it, and keeping it until the game's frame would replace the loading screen with another picture. Whether am has a timeout and what HOME does is closed code and **not checked** (section 5). |
| **Draw in the loader** and pass the window to the runtime | **Does not work** | `runtime.c:3516-3519`; `native_window.c:43-57` | A second display mechanism. The runtime sets the window up again (`consoleInit` + an empty frame), and a producer change is a `bqDisconnect`, after which the layer is black. The loader's time is covered by the NSP splash anyway. |
| **Early compositor start** | **Works** (part of the solution) | patch to `runtime.c` after `hold_thread_local_pages()` (original `:4077`) | Not any earlier: a compositor thread created before the image is mapped can get a TLS page inside a 32-bit game's reservation (`runtime.c:3062-3076`, the Sims 2 case). Before this point console error screens and the SDL launcher may still need the window. |
| A frame of the picture in the **Vulkan** swapchain at creation (GPU) | **Not safe** | `mesa-switch:src/nouveau/vulkan/nvk_physical_device.c:1718-1721`; `dxvk_presenter.cpp` | NVK has one graphics queue (`.queue_count = 1`). A separate submit/present from win32u would race DXVK's submit thread and break external `VkQueue` synchronisation. |
| A frame of the picture in the **Vulkan** swapchain via the CPU (writing the NvMap buffer) | **Works under conditions, not recommended now** | `mesa-switch:src/nouveau/horizon/nouveau_horizon_memory.c:479-495,590-640` | Needs `nvMapCreate` intercepted to learn the buffer's CPU address, plus a block-linear swizzle. A deep dependency on Mesa internals (BO cache) for one class of programs (point 4a of the summary). |

## 3. Recommended solution

### 3.1 What changes

| File | Change |
|---|---|
| `0008` `horizon-wine/source/runtime.c` | `wine_nx_vk_reserve_window()`: a Vulkan surface reserves the window while the compositor keeps drawing. `wine_nx_vk_take_window()`: the compositor hands the window over at swapchain creation. An early `wine_nx_compositor_enabled()` after `hold_thread_local_pages()`. `appletNotifyRunning()` everywhere the screen starts to show something other than the loading screen: a surface taking the window, a compositor failure, the launcher, the recovery message, `park_forever`. |
| `0009` `dlls/win32u/winnx_vulkan.c` | `nx_vulkan_surface_create` calls `wine_nx_vk_reserve_window()` instead of `wine_nx_gl_acquire_window()`. |
| `0010` `dlls/win32u/vulkan.c` | `wine_nx_vk_take_window()` right before `device->p_vkCreateSwapchainKHR(...)`. |
| `0011` `dlls/win32u/winnx_opengl.c` | `nx_screen_first_frame()`: the surface that just took the window from the compositor gets one frame of the keyboard picture, that is, Carafe's loading screen. The existing `nx_osk_draw()` draws it from its own context on the same thread, and then the thread's current context is restored. |
| `runtime/overlay/carafe_loading.c` | `bool __nx_applet_auto_notifyrunning = false;` and `NotifyRunning` on the second copy of the picture, that is, after the loading screen's first frame is queued. The hook is in the existing `__wrap_wine_nx_osk_copy`. |

All four patches were checked with `git apply --check` on 636913c.

### 3.2 How it is enabled only where needed

- **The delayed Vulkan handover** works only in compositor mode. With `framebuffer.txt` /
  `windows-through-opengl=0` the reserve calls the old `wine_nx_gl_acquire_window()` at once (model, scenario 7).
  `take` without a reserve does nothing: swapchain re-creation and `oldSwapchain` work as before (scenario 8).
- **The picture frame in a GL surface** is drawn only if `wine_nx_osk_visible()`, that is, while the loading
  screen is still shown (scenario 6: "no picture, no first frame"), and only when the surface took the window
  from the compositor rather than replacing the window's own surface (`if (!previous)`).
- **The delayed `NotifyRunning`** is enabled only by the strong symbol from Carafe's overlay. In plain Autorun
  libnx calls it itself (`applet.c:310-311`), and the added calls do nothing:
  `if (... g_appletNotifiedRunning) return;` (`applet.c:2100`).
- **Zero work per frame.** Nothing is added to `nx_drawable_swap`, `win32u_vkQueuePresentKHR` or the compositor's
  loop. The new code runs at surface creation, at swapchain creation and when the picture's generation changes
  (once a second, only while the loading screen is visible).

### 3.3 Proof chain: from process start to the game's first frame

Notation: **[code]** — checked in the source; **[E1/E3]** — done in an experiment; **[closed]** — a property of
closed system code, checked on the device.

**Stage 0. The loader (NSO).** Draws nothing and does not initialise applet. The NSP splash is visible.

**Stage 1. The runtime's libnx initialisation.** `appletInitialize` for `AppletType_Application`
(`runtime.c:71`) reaches `if (R_SUCCEEDED(rc) && __nx_applet_auto_notifyrunning) appletNotifyRunning(NULL);`
(`applet.c:310-311`). The overlay defines `__nx_applet_auto_notifyrunning = false`, and the strong definition
overrides `__attribute__((weak)) bool __nx_applet_auto_notifyrunning = true;` (`applet.c:10`) [E2: in all three
link orders]. **Screen:** the NSP splash [closed: am drops it on NotifyRunning, `applet.h:1507`].

**Stage 2. `main` before the image is mapped.** `consoleInit(NULL); consoleUpdate(NULL);` — one empty frame
(`runtime.c:3516-3519`) goes to the layer under the splash. Error screens on this stretch call
`appletNotifyRunning` (patch: recovery, launcher, `park_forever`). **Screen:** the splash.

**Stage 3. Early compositor start** (patch, right after `hold_thread_local_pages()`).
`wine_nx_compositor_enabled()` (`runtime.c:639-659`): `wine_nx_screen_leave_console()` → `consoleExit` on the main
thread, where `consoleInit` ran (`runtime.c:360-363` requires exactly that thread). Then
`wine_nx_compositor_start()` creates the presenter thread (`compositor.c:538-565`) and waits for `STATE_RUNNING`.
The presenter attaches EGL to `nwindowGetDefault()` (`compositor_egl.c:71-91`) and draws its first frame:
`wine_nx_osk_frame()` takes Carafe's picture (`compositor.c:442`), `wine_nx_osk_copy()` is called on a new
generation (`:465-467`), then `backend->swap()` (`:500`). If the compositor did not start, `appletNotifyRunning`
is called at once (scenarios 5 and 7). **Screen:** the splash, with the loading screen already under it.

**Stage 4. The splash goes away.** A second later Carafe changes the picture's generation (its seconds counter)
and calls `wine_nx_compositor_redraw()`. The presenter makes the second copy, and the overlay calls
`appletNotifyRunning`. The first frame has already gone out by then: the presenter copies, draws and calls
`swap()` strictly in order on one thread (`compositor.c:465-503`) [code; E1: `appletNotifyRunning` →
`SCREEN LOADING` with no BLACK in between in every scenario]. **Screen:** Carafe's loading screen with live
seconds.

**Stage 5. Wine starts the program and creates windows.** `winnx_drv.c:926` creates the window layer (a D3D
game's window is black, `:921-925`). The compositor draws the layers and the picture on top. **Screen:** the
loading screen.

**Stage 6a. Vulkan surface** (DXVK: the `Presenter` constructor, `dxvk_presenter.cpp:36-37`; for NFSU at
~12–15 s). Patch 0009: `wine_nx_vk_reserve_window()` sets `wine_nx_gl_window = 1` (the window is no longer given
to a second surface or the framebuffer, scenario 8: "screen busy; Vulkan surface refused"), but **the compositor
does not stop**. `vkCreateViSurfaceNN` only stores the pointer (`wsi_common_switch.c:1235`).
`vkGetPhysicalDeviceSurfaceCapabilitiesKHR` reads `nwindowGetDimensions`: 1280×720, set by `egl_attach`
(`compositor_egl.c:75`), the same value the old `acquire` set (`runtime.c:694`) [E1: `currentExtent 1280x720`].
**Screen:** the loading screen with live seconds, for all 23 s of NFSU's loading.

**Stage 7a. DXVK's first Present.** `PresentImage` → `acquireNextImage` → `recreateSwapChain` →
`vkCreateSwapchainKHR` (`d3d9_swapchain.cpp:860`, `dxvk_presenter.cpp:118-120,539-559`). Patch 0010:
`wine_nx_vk_take_window()` → `wine_nx_compositor_suspend()`. It waits until the presenter releases the window
(`compositor.c:265-266`): `egl_detach` → `eglDestroySurface` → `nwindowReleaseBuffers` → `bqDisconnect`
(`compositor_egl.c:93-99`, `native_window.c:324-339`). Then `nwindowSetDimensions(1280,720)`. After that NVK
creates the swapchain: `nwindowSetDimensions` and 3× `nwindowConfigureBuffer` (`wsi_common_switch.c:1140-1150,610`).
Without `take` this is impossible (scenario 10). DXVK immediately does acquire → blit → present, and
`nx_osk_present` puts the picture over the game's frame (`vulkan.c:3924-3965`). **Screen:** black from
`bqDisconnect` until the game's first frame is queued [closed: vi blanks the layer when the producer disconnects —
as seen on the video, 15–38 s], then "game frame + loading screen". After N frames Carafe hides the picture — the
game.

**Stage 6b/7b. OpenGL** (wined3d or a GL game). The window is taken already at `SetPixelFormat` (`opengl.c:2370`
→ `:2096` → `nx_surface_create`), through the old `wine_nx_gl_acquire_window()`: the compositor is suspended and
`appletNotifyRunning` is called. Then `eglCreateWindowSurface` registers buffers (`egl_switch.c:955`). Patch 0011:
`nx_screen_first_frame()` takes a context with this format's config (`nx_config_for_format`), makes it current on
the surface (Mesa queues a buffer for validate, `egl_switch.c:647`), calls `nx_osk_draw()` and `eglSwapBuffers`
(`:2250`), then restores the thread's previous current context and deletes its own [E3: the picture is on the
surface; the previous context and 9 kinds of its state are untouched; the keyboard texture slot is cleared; the
program's next frame replaces the picture]. **Screen:** the loading screen (static, until the game's first
`SwapBuffers`), then game frames with the picture, then the game [E1, scenarios 3 and 9].

**Stage 6c. Software rendering.** The screen stays with the compositor the whole time; Carafe hides the picture by
its ≥ 20 frames/s rule [E1, scenario 4].

### 3.4 Why nothing else breaks

- **Input.** The input thread starts at window surface creation (`winnx_drv.c:934`), not with the compositor; the
  patch does not touch it.
- **On-screen keyboard.** The same `wine_nx_osk_*` channel is used. If the real keyboard is visible when a GL
  window is taken, the first frame is the keyboard on black. Harmless, and better than before.
- **HOME and sleep.** The compositor runs longer during a Vulkan game's loading — the same state as with GDI
  windows before the handover. Focus logic does not change. The only risk is how HOME behaves while the splash is
  visible: that stretch is now longer (section 5).
- **Exit.** `return_to_launcher` → `wine_nx_compositor_stop()` (`runtime.c:3213`) and `release_screen_buffers()`
  (`:2757-2778`) are unchanged. The reserve flag is reset in `release`. Stopping the presenter during a reserve
  goes through the usual `stopped:` path with `detach` (`compositor.c:506-525`).
- **OpenGL games.** `acquire` is unchanged. The new part is one frame on one thread with no parallel GL. This keeps
  the rule in `compositor.h:10-12`: the presenter is already stopped, and the extra context is used only on the
  game's thread.
- **Vulkan games.** In the reserve phase nvc0 (the compositor, ~1 frame/s) and NVK (the game) work in parallel on
  different Mesa devices. The shared Horizon runtime is protected by mutexes (`nouveau_horizon_runtime.c:15,248-298`,
  `nouveau_horizon_device.c:254-259`). This coexistence already happened before: DXVK creates its `VkDevice` before
  the surface while the compositor draws windows. Not checked on the device for both versions; it is in the plan.
- **Game performance.** After `take` the frame path is identical to 636913c. The compositor holds about 11 MB of
  window buffers longer — only until the first present.

### 3.5 Devil's advocate: how it could fail

| Counterexample | Closed? |
|---|---|
| The compositor starts before the image is mapped → a TLS page in a 32-bit game's reservation | Closed: the start moved after `hold_thread_local_pages()` (originally `runtime.c:4077`). |
| The early compositor takes the window from a console error screen or the launcher | Closed: the start point is after every launcher and console branch (`runtime.c:3600-3810`); the remaining `[FAIL]` paths lead to `park_forever`, which calls `NotifyRunning`. |
| The splash stays forever if the first frame never comes | Closed for every path where something is shown: the compositor did not start (5, 7), the game took the window (9), the launcher, the console, `park_forever`. A hang without a single frame shows the splash instead of a black screen — HOME behaviour is checked in launch 2. |
| No second copy: the picture's generation does not change | Closed conditionally: the seconds on the picture change every second. The log of launch 1 must have the "drawn" line. If not, the game taking the window still calls `NotifyRunning`. |
| Carafe's "≥ 20 frames/s for 2 s" rule fires in the reserve phase because of the cursor | A real risk: before, the compositor was stopped in this phase, and cursor frames from the stick could hide the loading screen early. Closed in Carafe: the rule is off while a surface holds the window in reserve. |
| Restoring the GL context breaks if the previous surface was already destroyed | Closed: the path with `previous` (a format change) is excluded (`if (!previous)`); a restore failure is logged. |
| The black gap at the handover is noticeable | Not fully closed: from `bqDisconnect` to the first queued frame. Only a second layer or a Mesa change (`nwindowConfigureBuffer` after the compositor disconnects) removes it. Measured on a 60 fps video in launch 1. |
| A native Vulkan program creates its swapchain long before its first present | Not closed (scenario 1: 10 model seconds of black against 16 before the patch). Such programs are visible in the log: `[NXVK] screen handed to the Vulkan swapchain` without `present 1` right after. |
| A delayed `NotifyRunning` breaks am's requirements | The documented purpose is "for the Application logo screen"; official games call it when they are ready. am's timeout is unknown — launch 2. |

## 4. Device check (2 launches, 1 build)

**Build.** Carafe with patches `0008–0011` and the overlay changes. Before installing, check the binary:
`aarch64-none-elf-nm wine-nx-runtime.elf | grep __nx_applet_auto_notifyrunning` must show the symbol in `.bss`
(type `B`), not `D`. Type `D` means libnx's weak `true`, that is, the overlay did not make it into the link (E2).

**Launch 1 — NFSU (DXVK), a 60 fps video from the menu to the game, move the left stick during loading.**
Expected log lines in order:

| Step | Line | If it is missing |
|---|---|---|
| 3 | `[TLS] ...` → `[NXCOMP] presenting the screen through OpenGL on ...` | The compositor did not start early: look for `[NXCOMP] cannot present`. |
| 4 | `[CARAFE] loading screen drawn at X s`, with X close to the time of `[NXCOMP] presenting` | The picture was not ready at start; the splash goes away only when the window is taken. |
| 6a | `[NXVK] screen reserved for a Vulkan surface; the compositor keeps it until a swapchain` and right after `[NXVK] hwnd ...: a Vulkan surface has the screen` | Patch 0009 is not applied (`[NXGL] screen handed to an OpenGL surface` instead). |
| 6a–7a | `[PROGRESS] 15s/25s/35s ... frames=` with a **growing** frame count (before it was `frames=12` three times in a row) | The compositor is stopped: the reserve did not work. |
| 7a | `[NXCOMP] screen handed to an OpenGL program`, `[NXVK] screen handed to the Vulkan swapchain`, `[NXVK] floating keyboard 1280x720 ...`, `[NXVK] present 1: result 0` — in a row | A different order → patch 0010 is in the wrong place. |
| 7a | `[CARAFE] loading screen hidden after N s: Vulkan frames` (and **not** "compositor fps") | The compositor frame rule fired (cursor) — see 3.5. |

Video: the NSP splash, then the loading screen (seconds running from ~3 to ~38), then the game. Count the black
frames around the handover frame by frame; at most 3 are expected. Branches covered: early start, `reserve`
(success), `take` (success), `NotifyRunning` on the second copy.

**Launch 2 — an OpenGL game (OpenTTD or a game with `d3d=wine`): press HOME during the splash, come back, wait for
the game, exit through HOME → close.**

| Step | Line | Meaning |
|---|---|---|
| 6b | `[NXGL] screen handed to an OpenGL surface`, then `[NXGL] first frame: the floating keyboard's picture, swap done` | Branch 0011. `swap failed` or `could not be made current again` — look at the EGL error. |
| — | Video: the loading screen stays after GL takes the window, until the game's first frame | The "last queued buffer stays on screen" property 0011 relies on. |
| HOME | The system goes to HOME during the splash (or defers it until the splash goes) and comes back without hanging | am's closed behaviour with a delayed `NotifyRunning`. If HOME does not work during the splash, it is not dangerous, but the first frame should come sooner (section 5). |
| exit | `[EXIT] ...`, `[NXCOMP] presenter ended` | Exit has not changed. |

Branches run only in the model (scenarios 5, 7, 8, 9): compositor failure, reserve without a compositor, a second
Vulkan surface refused, `take` without a reserve, `NotifyRunning` from `acquire`. On the device they are checked by
repeating launch 1 with `framebuffer.txt` (then the log shows `[NXGL] screen handed to an OpenGL surface` from the
reserve) — an optional third launch.

## 5. What remains unproven and how to prove it

1. **The splash and `appletNotifyRunning`** (am is closed). libnx documents the call as needed "for the Application
   logo screen". A timeout and HOME/sleep behaviour during the splash are not checked. Proof — launch 2. A timeout
   would show as the splash disappearing before the first frame.
2. **The layer blanks on `bqDisconnect` and keeps the last queued buffer** (vi is closed). For now this is an
   observation from video (15–38 s of black after the compositor disconnected) plus Android BufferQueue semantics.
   Proof — the videos of launches 1 and 2.
3. **Length of the black gap at the handover.** Estimate: creating three 1280×720 images with zeroing, plus
   acquire, blit and queueing — about 10–40 ms. Not measured. Proof — a 60 fps video. Removing it completely means
   moving `take` into the first `nwindowConfigureBuffer` (`--wrap`), which saves only the allocation, or a second
   layer.
4. **nvc0 (compositor) and NVK (DXVK loading) working in parallel for tens of seconds.** Mesa has the mutexes, but
   this is not checked on the device. Proof: launch 1 with no `NOUVEAU` or `[NXVK]` errors in the log and a
   growing `frames=`.
5. **The meaning of description 9 in `0x1272`** is not documented by vi, Atmosphère or libnx. The "display already
   open in this session" reading rests on the proven reuse of the vi:m session.
6. **Native Vulkan programs with an early swapchain**: what shows before the first present stays black. A CPU frame
   into the NVK buffer (table of directions) would close it if such games turn up. The log sign is given in 3.5.
7. **The overlay hook.** The analysis assumed the hook point (`__wrap_wine_nx_osk_copy`) and a picture generation
   that changes every second; in Carafe both hold in `runtime/overlay/carafe_loading.c`.
