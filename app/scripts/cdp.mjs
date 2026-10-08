import { writeFileSync } from "node:fs";

const usage = `usage: node scripts/cdp.mjs <command> [...]
  targets                       list debuggable pages
  screenshot <out.png>          capture the window
  shot-at <width> <out.png>     capture the page laid out at this viewport width
  eval <expression>             evaluate JS (awaited) and print the result as JSON
  click <css selector>          click the first matching element
  type <css selector> <text>    set an input's value the way React notices
  mouse <x> <y>                 left click at viewport coordinates
  drag <x1> <y1> <x2> <y2>      left-button drag between viewport coordinates
  console [seconds]             print console messages and exceptions (default 3 s)
  goto <path>                   navigate the app to a route, e.g. /wizard
  dialog <path>                 make the next open-file/folder dialog return this path
env: CARAFE_CDP_PORT (default 9222)`;

const [command, ...args] = process.argv.slice(2);
if (!command) {
  console.error(usage);
  process.exit(2);
}

const port = process.env.CARAFE_CDP_PORT ?? "9222";
const targets = await fetch(`http://127.0.0.1:${port}/json`)
  .then((response) => response.json())
  .catch(() => {
    console.error(`no debugger on port ${port}: start the app with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=${port}`);
    process.exit(1);
  });

if (command === "targets") {
  for (const target of targets) {
    console.log(`${target.type}\t${target.title}\t${target.url}`);
  }
  process.exit(0);
}

const page = targets.find((target) => target.type === "page");
if (!page) {
  console.error("no page target");
  process.exit(1);
}

const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  socket.onopen = resolve;
  socket.onerror = reject;
});

let nextId = 0;
const pending = new Map();
const listeners = [];
socket.onmessage = (message) => {
  const data = JSON.parse(message.data);
  if (data.id !== undefined && pending.has(data.id)) {
    const { resolve, reject } = pending.get(data.id);
    pending.delete(data.id);
    if (data.error) {
      reject(new Error(`${data.error.message} (${data.error.code})`));
    } else {
      resolve(data.result);
    }
    return;
  }
  for (const listener of listeners) {
    listener(data);
  }
};

function send(method, params = {}) {
  const id = ++nextId;
  socket.send(JSON.stringify({ id, method, params }));
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
}

async function evaluate(expression) {
  const result = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) {
    const details = result.exceptionDetails;
    throw new Error(details.exception?.description ?? details.text);
  }
  return result.result.value;
}

async function mouse(type, x, y) {
  await send("Input.dispatchMouseEvent", { type, x, y, button: "left", buttons: type === "mouseReleased" ? 0 : 1, clickCount: 1 });
}

function describe(arg) {
  if (arg.type === "string") {
    return arg.value;
  }
  if (arg.value !== undefined) {
    return JSON.stringify(arg.value);
  }
  return arg.description ?? arg.type;
}

try {
  switch (command) {
    case "screenshot": {
      const { data } = await send("Page.captureScreenshot", { format: "png" });
      writeFileSync(args[0] ?? "screenshot.png", Buffer.from(data, "base64"));
      console.log(args[0] ?? "screenshot.png");
      break;
    }
    case "shot-at": {
      const width = Number(args[0]);
      const height = await evaluate("innerHeight");
      await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 0, mobile: false });
      await new Promise((resolve) => setTimeout(resolve, 600));
      const { data } = await send("Page.captureScreenshot", { format: "png" });
      await send("Emulation.clearDeviceMetricsOverride");
      writeFileSync(args[1] ?? "screenshot.png", Buffer.from(data, "base64"));
      console.log(args[1] ?? "screenshot.png");
      break;
    }
    case "eval":
      console.log(JSON.stringify(await evaluate(args.join(" ")), null, 2));
      break;
    case "click": {
      const selector = JSON.stringify(args[0]);
      const found = await evaluate(`(() => { const el = document.querySelector(${selector}); if (!el) return false; el.click(); return true; })()`);
      console.log(found ? "clicked" : "not found");
      break;
    }
    case "type": {
      const selector = JSON.stringify(args[0]);
      const text = JSON.stringify(args.slice(1).join(" "));
      const found = await evaluate(`(() => {
        const el = document.querySelector(${selector});
        if (!el) return false;
        const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), "value").set;
        setter.call(el, ${text});
        el.dispatchEvent(new Event("input", { bubbles: true }));
        return true;
      })()`);
      console.log(found ? "typed" : "not found");
      break;
    }
    case "mouse": {
      const [x, y] = args.map(Number);
      await mouse("mousePressed", x, y);
      await mouse("mouseReleased", x, y);
      console.log("clicked");
      break;
    }
    case "drag": {
      const [x1, y1, x2, y2] = args.map(Number);
      await mouse("mousePressed", x1, y1);
      for (let step = 1; step <= 10; step++) {
        await mouse("mouseMoved", x1 + ((x2 - x1) * step) / 10, y1 + ((y2 - y1) * step) / 10);
      }
      await mouse("mouseReleased", x2, y2);
      console.log("dragged");
      break;
    }
    case "console": {
      listeners.push((event) => {
        if (event.method === "Runtime.consoleAPICalled") {
          console.log(`[${event.params.type}] ${event.params.args.map(describe).join(" ")}`);
        } else if (event.method === "Runtime.exceptionThrown") {
          const details = event.params.exceptionDetails;
          console.log(`[exception] ${details.exception?.description ?? details.text}`);
        } else if (event.method === "Log.entryAdded") {
          console.log(`[${event.params.entry.level}] ${event.params.entry.text} ${event.params.entry.url ?? ""}`);
        }
      });
      await send("Runtime.enable");
      await send("Log.enable");
      await new Promise((resolve) => setTimeout(resolve, Number(args[0] ?? 3) * 1000));
      break;
    }
    case "goto": {
      const path = JSON.stringify(args[0] ?? "/");
      await evaluate(`(() => { history.pushState(null, "", ${path}); dispatchEvent(new PopStateEvent("popstate")); return location.pathname; })()`);
      console.log("navigated");
      break;
    }
    case "dialog": {
      const path = JSON.stringify(args.join(" "));
      const armed = await evaluate(`(() => {
        const original = window.__carafeOriginalFetch ?? window.fetch;
        window.__carafeOriginalFetch = original;
        const stub = (input, init) => {
          const url = typeof input === "string" ? input : input.url;
          if (decodeURIComponent(url).endsWith("plugin:dialog|open")) {
            window.fetch = original;
            return Promise.resolve(new Response(JSON.stringify(${path}), {
              headers: { "Content-Type": "application/json", "Tauri-Response": "ok" },
            }));
          }
          return original(input, init);
        };
        window.fetch = stub;
        return window.fetch === stub;
      })()`);
      if (!armed) {
        throw new Error("could not intercept the dialog: do not click, a real dialog would open");
      }
      console.log("armed: the next dialog returns the path");
      break;
    }
    default:
      console.error(usage);
      process.exitCode = 2;
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
} finally {
  socket.close();
}
