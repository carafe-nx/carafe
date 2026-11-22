from pathlib import Path
import subprocess
import sys
import tempfile

source, headers = map(Path, sys.argv[1:3])
text = source.read_text()

def block(start, at=0):
    begin = text.index(start, at)
    brace = text.index('{', begin)
    depth = 1
    end = brace + 1
    while depth:
        depth += (text[end] == '{') - (text[end] == '}')
        end += 1
    return text[begin:end]

mark = block('static void horizon_server_mark_window_update_locked(')
publish = 'static void horizon_server_publish_window_paint_locked('
helper = block(publish) if publish in text else ''
pos = text.index('if (window->paint_flags & HORIZON_SET_WINPOS_PAINT_SURFACE)')
surface = block('if (window->paint_flags & HORIZON_SET_WINPOS_PAINT_SURFACE)', pos)
refresh = text[text.index('if (geometry_changed || (request->swp_flags &', pos):].split(';', 1)[0] + ';'
code = r'''
#include <stdio.h>
#include "horizon_msg_queue.h"
#define HORIZON_WS_VISIBLE 1
#define HORIZON_SET_WINPOS_PAINT_SURFACE 1
#define HORIZON_SWP_SHOWWINDOW 1
#define HORIZON_SWP_HIDEWINDOW 2
#define horizon_trace(...) ((void)0)
struct horizon_rectangle { int left, top, right, bottom; };
struct horizon_user_window {
    unsigned int tid, style, handle, paint_flags;
    int has_update_rect, needs_erase;
    struct horizon_rectangle update_rect;
};
static struct horizon_user_window a = {1, 1, 1}, b = {1, 1, 2}, c = {2, 1, 3};
static struct horizon_msgq owner = {.tid=1}, other = {.tid=2};
static struct horizon_msgqs horizon_msg_queues;
static int horizon_server_timer_clock(void) { return 0; }
static void horizon_server_refresh_queue_locked(struct horizon_msgq *q, unsigned long long now)
{
    int pending = q == &owner ? a.has_update_rect || b.has_update_rect : c.has_update_rect;
    horizon_msgq_update(q, pending ? HORIZON_MSGQ_QS_PAINT : 0);
}
static void horizon_server_refresh_queues_locked(void)
{
    horizon_server_refresh_queue_locked(&owner, 0);
    horizon_server_refresh_queue_locked(&other, 0);
}
static int horizon_server_rect_empty(const struct horizon_rectangle *r) { return r->left == r->right; }
static struct horizon_rectangle horizon_server_union_rect(struct horizon_rectangle a, struct horizon_rectangle b) { return a; }
static void horizon_server_window_screen_rects_locked(const struct horizon_user_window *w,
        struct horizon_rectangle *outer, struct horizon_rectangle *client)
{
    *outer = *client = (struct horizon_rectangle){0, 0, 10, 10};
}
''' + helper + mark + '\n' + r'''
int main(void)
{
    const unsigned int paint = HORIZON_MSGQ_QS_PAINT;
    struct horizon_rectangle dirty = {0, 0, 10, 10};
    struct horizon_user_window *window = &b;
    struct { int swp_flags; } args = {0}, *request = &args;
    struct { int surface_win; } reply = {0};
    int geometry_changed = 0;
    owner.next = &other;
    horizon_msg_queues.head = &owner;
    owner.changed_mask = paint;
    a.has_update_rect = 1;
    horizon_server_refresh_queues_locked();
    owner.changed_bits = 0;
    horizon_server_mark_window_update_locked(&b, dirty, 1);
    horizon_server_refresh_queues_locked();
    int edge = owner.wake_bits == paint && owner.changed_bits == paint && horizon_msgq_signaled(&owner);
    int foreign = other.changed_bits || other.wake_bits;
    a.has_update_rect = b.has_update_rect = 0;
    owner.changed_bits = 0;
    horizon_server_refresh_queues_locked();
    int idle = !owner.wake_bits && !owner.changed_bits;
    b.paint_flags = HORIZON_SET_WINPOS_PAINT_SURFACE;
''' + surface + '\n' + refresh + '\n' + r'''
    int surface_ok = b.has_update_rect && reply.surface_win == b.handle && owner.wake_bits == paint && owner.changed_bits == paint;
    owner.changed_bits = 0;
    horizon_server_refresh_queues_locked();
    int peek = owner.wake_bits == paint && !owner.changed_bits;
    b.has_update_rect = 0;
    horizon_server_refresh_queues_locked();
    int validate = !owner.wake_bits && !owner.changed_bits;
    printf("reinvalidate=%d surface=%d foreign=%d idle=%d peek=%d validate=%d\n",
            edge, surface_ok, foreign, idle, peek, validate);
    return !(edge && surface_ok && !foreign && idle && peek && validate);
}
'''
with tempfile.TemporaryDirectory() as directory:
    c = Path(directory) / 'paint.c'
    exe = c.with_suffix('')
    c.write_text(code)
    subprocess.run(['cc', '-I', str(headers), str(c), '-o', str(exe)], check=True)
    sys.exit(subprocess.run([exe]).returncode)
