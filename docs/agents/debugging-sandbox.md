# Running & debugging Cerebrite in a sandbox

How to get the real, native Tauri app (not just the bare frontend) running and
interactively debuggable from inside a headless sandbox.

## Two ways to run this app, and why they're different

- **Frontend only** (`npx vite --host 0.0.0.0 --port 1420`): fast, no Rust
  build needed, good for pure UI/CSS work. But `window.__TAURI_INTERNALS__` is
  never injected outside the real Tauri shell, so any code path that calls
  `invoke(...)` (creating a page, vault lookups, anything backed by a Tauri
  command) throws `TypeError: can't access property "invoke",
  window.__TAURI_INTERNALS__ is undefined`. This is expected in this mode, not
  a bug — don't try to "fix" it.
- **The real app** (`npm run tauri dev`): builds and runs the actual Rust
  binary with a real webview. This is the only way to exercise
  invoke-backed functionality. It needs a display, which this sandbox doesn't
  have — see below.

Default to frontend-only for quick UI iteration. Switch to the full route
only when the user needs to click through actual functionality.

## Full route: native app under a virtual display, viewed over VNC

```bash
# one-time per sandbox
sudo apt-get install -y x11vnc   # webkit2gtk-4.1 is normally already present; check with
                                  # `pkg-config --exists webkit2gtk-4.1` first

# launch the real app headlessly
cd <repo root>
CHOKIDAR_USEPOLLING=true CHOKIDAR_INTERVAL=300 \
  nohup xvfb-run -a --server-args="-screen 0 1280x800x24" npm run tauri dev \
  > /tmp/tauri-dev.log 2>&1 &
disown

# once Xvfb is up (check `ps aux | grep Xvfb` for the `:NN` display number),
# serve that display over VNC — note the WAYLAND_DISPLAY unset, see gotcha below
ps aux | grep Xvfb   # find the -auth path and display number, e.g. :99
env -u WAYLAND_DISPLAY nohup x11vnc -display :99 -auth <auth-file-from-above> \
  -forever -shared -rfbport 5900 -listen 0.0.0.0 -nopw \
  > /tmp/x11vnc.log 2>&1 &
disown
```

Tell the user to run on their host:

```bash
sbx ports <sandbox-name> --publish 5900:5900/tcp
```

then connect any VNC client to `localhost:5900`. That shows the actual native
Cerebrite window with a working Tauri bridge.

## Gotchas hit while wiring this up

- **`CHOKIDAR_USEPOLLING=true` is required, not optional.** Without it, Vite's
  native file watcher (chokidar) intermittently crashes with
  `Error: EIO: i/o error, scandir '<repo root>'` right as Cargo finishes
  linking the binary — heavy concurrent I/O from the Rust build appears to
  destabilize this sandbox's mounted filesystem's directory listing just long
  enough to break a live `readdir`. Polling mode sidesteps the native watcher
  entirely and has been reliable. If you see this crash again, check that the
  env var actually reached the `vite` process (it must be set before `npm run
  tauri dev`, not after).
- **`WAYLAND_DISPLAY` may already be set in the shell** (unrelated to this
  app) and makes `x11vnc` misdetect the session as Wayland and refuse to
  start ("Wayland display server detected... Exiting"). Unset it for the
  `x11vnc` invocation specifically (`env -u WAYLAND_DISPLAY ...`); don't
  unset it globally, something else in the sandbox may depend on it.
- **A corrupted `cargo` incremental cache is possible** if `src-tauri/target`
  was touched by an interrupted/killed build (this repo's `target/` has also
  been used for cross-compiling to Android — ticket 15 — which raises the
  odds of a stale/partial cache). Symptom: a build error like
  `unable to copy .../incremental/.../*.o ... No such file or directory`, or
  `rm`/`mv` on specific `.o` files failing with `Bad file descriptor` (a
  genuine, non-transient I/O-layer corruption — retrying the same rm doesn't
  help). Fix: don't fight the broken files; `mv src-tauri/target
  src-tauri/target-broken-$(date +%s)` (a full-directory `mv` succeeds even
  when individual files inside are unreadable) and let a fresh `target/`
  build from scratch. Clean up the broken directory in the background
  afterwards — do this only when nothing else is about to scan the repo
  tree, or the delete can itself trigger the scandir EIO above.
- **Two `vite` instances fighting over port 1420** if a standalone
  frontend-only `vite` is still running when you switch to `tauri dev` (which
  spawns its own `beforeDevCommand`). Kill any prior `vite`/`tauri` processes
  by PID (not by a `pkill -f` pattern guess — `vite --host` does not match
  the actual command line `node .../vite.js --host ...`) before starting the
  full route.
- **First-time `cargo` build is slow** (~3-4 min observed): `git2` (vendored
  OpenSSL) and `rusqlite` (bundled SQLite) both compile their C dependencies
  from source. Subsequent runs reuse the incremental cache and finish in
  under a second unless the cache gets corrupted (see above).
