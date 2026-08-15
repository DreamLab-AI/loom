#!/usr/bin/env python3
"""DiffusionGemma management server — self-recovering OpenAI-compatible front-end.

The llama.cpp diffusion fork's `llama-diffusion-gemma-visual-server` is NOT a network
server: it speaks a line protocol over stdin/stdout. This wrapper owns one as a persistent
child, **supervises it** (auto-respawn with backoff after a crash), and exposes:

  OpenAI:  POST /v1/chat/completions     GET /v1/models
  Live UI: GET  /                        POST /ui/generate
  Mgmt:    GET  /health                  GET  /admin/status
           POST /admin/restart           POST /admin/stop      POST /admin/start

Recovery is layered: (1) a supervisor thread respawns the backend the moment it dies, so
a crash self-heals in seconds instead of leaving `/v1/models` up but completions 500ing;
(2) `/admin/restart` forces a clean reload on demand; (3) Docker's healthcheck +
`restart: unless-stopped` recycle the whole container if the process itself wedges.

Single-context backend => generations are serialized behind one lock. Stdlib only.
"""
import json
import os
import queue
import subprocess
import sys
import threading
import time
import tempfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# --- config (env-overridable) ------------------------------------------------
HERE = os.path.dirname(os.path.abspath(__file__))
SERVER_BIN = os.environ.get(
    "DG_SERVER_BIN",
    os.path.join(HERE, "bin", "llama-diffusion-gemma-visual-server"))
MODEL = os.environ.get("DG_MODEL", "/models/diffusiongemma-26B-A4B-it-Q8_0.gguf")
HOST = os.environ.get("DG_HOST", "0.0.0.0")
PORT = int(os.environ.get("DG_PORT", "8084"))
NGL = os.environ.get("NGL", "99")
GPU = os.environ.get("CUDA_VISIBLE_DEVICES", "0")
DEFAULT_BLOCKS = int(os.environ.get("DG_DEFAULT_BLOCKS", "8"))
MODEL_NAME = os.environ.get("DG_MODEL_NAME", "diffusiongemma-26B-A4B-it-Q8_0")
ADMIN_TOKEN = os.environ.get("DG_ADMIN_TOKEN", "")
READY_TIMEOUT = int(os.environ.get("DG_READY_TIMEOUT", "600"))   # model-load patience (s)
GEN_WAIT = int(os.environ.get("DG_GEN_WAIT", "120"))             # how long a request waits for recovery
POLL_INTERVAL = float(os.environ.get("DG_POLL_INTERVAL", "3"))   # supervisor cadence (s)
MAX_BACKOFF = int(os.environ.get("DG_MAX_BACKOFF", "60"))        # respawn backoff ceiling (s)


def log(msg):
    sys.stderr.write(f"[dg {time.strftime('%H:%M:%S')}] {msg}\n")
    sys.stderr.flush()


class Backend:
    """Owns + supervises the persistent visual-server child."""

    def __init__(self):
        self.lock = threading.Lock()      # serialize generations AND (re)spawns
        self.proc = None
        self.n_vocab = 0
        self.maxtok = 0
        self.canvas_length = None         # learned from the first STATS line
        self.log = open("/tmp/diffgemma-visual-server.log", "ab", buffering=0)
        # supervision/observability state
        self.state = "init"               # init|starting|ready|restarting|stopped|failed
        self.started_at = None
        self.restarts = 0                 # successful respawns since process start
        self.last_error = ""
        self.last_exit = None
        self.stop_requested = False       # set by /admin/stop; supervisor won't respawn
        self._fail_streak = 0             # consecutive spawn failures (drives backoff)

    # --- lifecycle -----------------------------------------------------------
    def _spawn_locked(self):
        """Launch the child and block until it prints READY. Caller holds self.lock.
        Raises on missing binary/model, child death, or READY timeout."""
        if not os.path.exists(SERVER_BIN):
            raise FileNotFoundError(f"server binary not found: {SERVER_BIN}")
        if not os.path.exists(MODEL):
            raise FileNotFoundError(f"model not found: {MODEL}")
        env = dict(os.environ)
        env["CUDA_VISIBLE_DEVICES"] = GPU
        env["NGL"] = NGL
        env.setdefault("MAXTOK", "0")     # 0 = auto-size the largest context that fits
        self.state = "starting"
        log(f"launching backend (gpu={GPU} ngl={NGL}) {SERVER_BIN}")
        proc = subprocess.Popen(
            [SERVER_BIN, MODEL],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log,
            env=env, text=True, bufsize=1)

        # Read stdout for "READY <n_vocab> <MAXTOK>" with a hard timeout (model load is slow).
        result = {}
        done = threading.Event()

        def reader():
            try:
                while True:
                    line = proc.stdout.readline()
                    if not line:
                        result["error"] = "backend exited before READY (see /tmp/diffgemma-visual-server.log)"
                        return
                    line = line.strip()
                    if line.startswith("READY"):
                        parts = line.split()
                        result["n_vocab"] = int(parts[1])
                        result["maxtok"] = int(parts[2]) if len(parts) > 2 else 0
                        return
            except Exception as e:  # noqa: BLE001
                result["error"] = f"reader failed: {e}"
            finally:
                done.set()

        threading.Thread(target=reader, daemon=True).start()
        if not done.wait(READY_TIMEOUT):
            proc.kill()
            raise TimeoutError(f"backend not READY within {READY_TIMEOUT}s")
        if "error" in result:
            try:
                proc.kill()
            except Exception:
                pass
            raise RuntimeError(result["error"])
        self.proc = proc
        self.n_vocab = result["n_vocab"]
        self.maxtok = result["maxtok"]
        self.started_at = time.time()
        self.state = "ready"
        self._fail_streak = 0
        log(f"backend READY n_vocab={self.n_vocab} maxtok={self.maxtok} pid={proc.pid}")

    def _kill_locked(self):
        """Reap the current child (graceful QUIT -> terminate -> kill). Caller holds the lock."""
        if not self.proc:
            return
        try:
            self.proc.stdin.write("QUIT\n")
            self.proc.stdin.flush()
        except Exception:
            pass
        for step in (self.proc.terminate, self.proc.kill):
            try:
                self.proc.wait(timeout=5)
                break
            except Exception:
                try:
                    step()
                except Exception:
                    pass
        self.last_exit = self.proc.returncode
        self.proc = None

    def start(self):
        with self.lock:
            self.stop_requested = False
            self._spawn_locked()

    def restart(self):
        """Force a clean reload (admin-triggered). Blocks until READY."""
        with self.lock:
            self.state = "restarting"
            self.stop_requested = False
            self._kill_locked()
            self._spawn_locked()
            self.restarts += 1

    def stop(self):
        """Stop the backend and keep it stopped (supervisor will not respawn)."""
        with self.lock:
            self.stop_requested = True
            self._kill_locked()
            self.state = "stopped"

    def alive(self):
        return self.proc is not None and self.proc.poll() is None

    def wait_ready(self, timeout):
        """Block up to `timeout`s for the backend to be alive (e.g. mid-recovery)."""
        deadline = time.time() + timeout
        while not self.alive() and not self.stop_requested and time.time() < deadline:
            time.sleep(0.5)
        return self.alive()

    def status(self):
        return {
            "model": MODEL_NAME,
            "state": self.state,
            "backend": "alive" if self.alive() else "dead",
            "pid": self.proc.pid if self.alive() else None,
            "restarts": self.restarts,
            "uptime_s": int(time.time() - self.started_at) if (self.alive() and self.started_at) else 0,
            "n_vocab": self.n_vocab,
            "maxtok": self.maxtok,
            "canvas_length": self.canvas_length,
            "gpu": GPU,
            "last_exit": self.last_exit,
            "last_error": self.last_error or None,
            "stop_requested": self.stop_requested,
        }

    # --- generation (line protocol, unchanged) -------------------------------
    def generate(self, messages, seed=0, n_blocks=DEFAULT_BLOCKS, on_record=None):
        if not self.alive():
            self.wait_ready(GEN_WAIT)     # give the supervisor a chance to bring it back
        with self.lock:
            if not self.alive():
                raise RuntimeError("backend not running (recovery in progress; retry shortly)")
            req = {"seed": int(seed), "n_blocks": int(n_blocks), "messages": messages}
            tf = tempfile.NamedTemporaryFile(
                mode="w", suffix=".dgreq", delete=False, encoding="utf-8")
            try:
                json.dump(req, tf)
                tf.flush()
                tf.close()
                self.proc.stdin.write(tf.name + "\n")
                self.proc.stdin.flush()
                final_text = ""
                stats = {}
                while True:
                    line = self.proc.stdout.readline()
                    if not line:
                        raise RuntimeError("backend closed stream mid-request")
                    line = line.rstrip("\n")
                    if line == "DONE":
                        break
                    if line.startswith("ERR"):
                        raise RuntimeError("backend error: " + line)
                    tag = line[:1]
                    if tag == "F":
                        p = line.split(" ", 4)
                        text = json.loads(p[4]) if len(p) == 5 else ""
                        if on_record:
                            on_record("frame", {"block": int(p[1]), "step": int(p[2]),
                                                 "total": int(p[3]), "text": text})
                    elif tag == "C":
                        p = line.split(" ", 2)
                        final_text = json.loads(p[2]) if len(p) == 3 else ""
                        if on_record:
                            on_record("commit", {"block": int(p[1]), "text": final_text})
                    elif line.startswith("STATS"):
                        for kv in line.split()[1:]:
                            if "=" in kv:
                                k, v = kv.split("=", 1)
                                stats[k] = v
                        if "canvas" in stats:
                            self.canvas_length = int(stats["canvas"])
                        if on_record:
                            on_record("stats", stats)
                return {"text": final_text, "stats": stats}
            finally:
                try:
                    os.unlink(tf.name)
                except OSError:
                    pass


BACKEND = Backend()


def supervisor_loop():
    """Respawn the backend whenever it dies unexpectedly, with exponential backoff."""
    while True:
        time.sleep(POLL_INTERVAL)
        if BACKEND.stop_requested or BACKEND.alive():
            continue
        backoff = min(2 ** BACKEND._fail_streak, MAX_BACKOFF)
        log(f"backend down (state={BACKEND.state}); respawn attempt in {backoff}s")
        time.sleep(backoff)
        if BACKEND.stop_requested or BACKEND.alive():
            continue
        try:
            with BACKEND.lock:
                if BACKEND.stop_requested or BACKEND.alive():
                    continue
                BACKEND.state = "restarting"
                BACKEND._kill_locked()        # reap the dead child
                BACKEND._spawn_locked()
                BACKEND.restarts += 1
            log("backend respawned by supervisor")
        except Exception as e:  # noqa: BLE001
            BACKEND._fail_streak += 1
            BACKEND.last_error = str(e)
            BACKEND.state = "failed"
            log(f"supervisor respawn failed (streak={BACKEND._fail_streak}): {e}")


def split_channels(text):
    """DiffusionGemma emits reasoning in a '<|channel>thought ... <channel|>' block before
    the final answer. Return (reasoning, answer)."""
    head, sep, tail = text.rpartition("<channel|>")
    if not sep:
        return "", text
    reasoning = head
    if reasoning.startswith("<|channel>"):
        reasoning = reasoning[len("<|channel>"):]
        if reasoning[:7] == "thought":
            reasoning = reasoning[7:]
    return reasoning.strip(), tail.strip()


def _split_regions(text):
    """Split DiffusionGemma's cumulative output into (reasoning, answer, closed) using RAW
    (unstripped) substrings, so the lengths grow monotonically as the canvas fills — required for
    correct streaming deltas. `reasoning` is the text after the `<|channel>[thought]` opener;
    `answer` is the text after the `<channel|>` closer. `closed` is False while the reasoning
    block is still open (closer not yet emitted) so the caller keeps it inside <think>."""
    opener, closer = "<|channel>", "<channel|>"
    i = text.find(opener)
    if i == -1:
        return "", text, True                      # no reasoning channel → all answer
    rest = text[i + len(opener):]
    if rest.startswith("thought"):
        rest = rest[len("thought"):]
    j = rest.find(closer)
    if j == -1:
        return rest, "", False                     # reasoning still streaming, not closed
    return rest[:j], rest[j + len(closer):], True


def _blocks_for(body):
    if "n_blocks" in body:
        return int(body["n_blocks"])
    mt = body.get("max_tokens")
    if mt and BACKEND.canvas_length:
        return max(1, -(-int(mt) // BACKEND.canvas_length))  # ceil-div
    return DEFAULT_BLOCKS


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        sys.stderr.write("[http] " + (a[0] % a[1:]) + "\n")

    def _json(self, code, obj):
        data = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(data)

    def _sse_open(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()

    def _sse(self, obj):
        self.wfile.write(f"data: {json.dumps(obj)}\n\n".encode())
        self.wfile.flush()

    def _read_body(self):
        n = int(self.headers.get("Content-Length", "0"))
        return json.loads(self.rfile.read(n) or b"{}")

    def _admin_ok(self):
        """Admin endpoints require the bearer token IF one is configured; otherwise open
        (a warning is logged at startup). Lets `restart` work out-of-the-box yet be locked down."""
        if not ADMIN_TOKEN:
            return True
        return self.headers.get("Authorization", "") == f"Bearer {ADMIN_TOKEN}"

    # --- routes --------------------------------------------------------------
    def do_GET(self):
        if self.path == "/health":
            st = BACKEND.status()
            alive = st["backend"] == "alive"
            st["status"] = "ok" if alive else ("stopped" if st["stop_requested"] else "down")
            self._json(200 if alive else 503, st)
        elif self.path == "/admin/status":
            if not self._admin_ok():
                return self._json(401, {"error": "admin token required"})
            self._json(200, BACKEND.status())
        elif self.path in ("/v1/models", "/models"):
            self._json(200, {"object": "list", "data": [{
                "id": MODEL_NAME, "object": "model",
                "created": int(time.time()), "owned_by": "unsloth-diffusiongemma"}]})
        elif self.path in ("/", "/index.html"):
            html = UI_HTML.encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(html)))
            self.end_headers()
            self.wfile.write(html)
        else:
            self._json(404, {"error": "not found"})

    def do_POST(self):
        try:
            if self.path == "/v1/chat/completions":
                self._chat()
            elif self.path == "/ui/generate":
                self._ui_generate()
            elif self.path in ("/admin/restart", "/admin/stop", "/admin/start"):
                self._admin(self.path.rsplit("/", 1)[1])
            else:
                self._json(404, {"error": "not found"})
        except BrokenPipeError:
            pass
        except Exception as e:  # noqa: BLE001
            try:
                self._json(500, {"error": str(e)})
            except Exception:
                pass

    def _admin(self, action):
        if not self._admin_ok():
            return self._json(401, {"error": "admin token required"})
        try:
            if action == "restart":
                BACKEND.restart()
            elif action == "stop":
                BACKEND.stop()
            elif action == "start":
                BACKEND.start()
            self._json(200, {"ok": True, "action": action, **BACKEND.status()})
        except Exception as e:  # noqa: BLE001
            BACKEND.last_error = str(e)
            self._json(500, {"ok": False, "action": action, "error": str(e)})

    def _chat(self):
        body = self._read_body()
        messages = body.get("messages")
        if not messages:
            return self._json(400, {"error": "messages required"})
        seed = body.get("seed", 0)
        n_blocks = _blocks_for(body)
        created = int(time.time())
        cid = f"chatcmpl-dg-{created}"

        if body.get("stream"):
            self._sse_open()

            def chunk(content):
                self._sse({"id": cid, "object": "chat.completion.chunk", "created": created,
                           "model": MODEL_NAME,
                           "choices": [{"index": 0, "delta": {"content": content},
                                        "finish_reason": None}]})

            # Stream reasoning wrapped in <think>…</think> (the form Open WebUI renders as a
            # collapsible block on every version) then the answer. Stateful, so the thinking is
            # never leaked into the answer mid-stream — even before the <channel|> closer appears,
            # which is exactly what broke the old per-commit split_channels approach.
            st = {"r": 0, "a": 0, "open": False, "closed": False}

            def cb(kind, payload):
                if kind != "commit":
                    return
                reasoning, answer, closed = _split_regions(payload["text"])
                if not st["closed"] and len(reasoning) > st["r"]:
                    if not st["open"]:
                        chunk("<think>\n")
                        st["open"] = True
                    chunk(reasoning[st["r"]:])
                    st["r"] = len(reasoning)
                if closed and answer:
                    if st["open"] and not st["closed"]:
                        chunk("\n</think>\n\n")
                        st["closed"] = True
                    if len(answer) > st["a"]:
                        chunk(answer[st["a"]:])
                        st["a"] = len(answer)
            try:
                BACKEND.generate(messages, seed, n_blocks, on_record=cb)
                if st["open"] and not st["closed"]:        # reasoning emitted but no answer text
                    chunk("\n</think>\n")
                self._sse({"id": cid, "object": "chat.completion.chunk", "created": created,
                           "model": MODEL_NAME,
                           "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]})
                self.wfile.write(b"data: [DONE]\n\n")
                self.wfile.flush()
            except Exception as e:  # noqa: BLE001
                self._sse({"error": str(e)})
            return

        res = BACKEND.generate(messages, seed, n_blocks)
        st = res["stats"]
        reasoning, answer = split_channels(res["text"])
        msg = {"role": "assistant", "content": answer}
        if reasoning:
            msg["reasoning_content"] = reasoning
        self._json(200, {
            "id": cid, "object": "chat.completion", "created": created, "model": MODEL_NAME,
            "choices": [{"index": 0, "finish_reason": "stop", "message": msg}],
            "usage": {"prompt_tokens": int(st.get("prompt_n", 0)),
                      "completion_tokens": int(st.get("predicted_n", 0)),
                      "total_tokens": int(st.get("prompt_n", 0)) + int(st.get("predicted_n", 0))},
            "timings": st})

    def _ui_generate(self):
        body = self._read_body()
        messages = body.get("messages") or [
            {"role": "user", "content": body.get("prompt", "Hello!")}]
        seed = body.get("seed", 0)
        n_blocks = _blocks_for(body)
        self._sse_open()
        q = queue.Queue()

        def worker():
            try:
                r = BACKEND.generate(messages, seed, n_blocks,
                                     on_record=lambda k, p: q.put((k, p)))
                q.put(("done", r["stats"]))
            except Exception as e:  # noqa: BLE001
                q.put(("error", {"message": str(e)}))

        threading.Thread(target=worker, daemon=True).start()
        while True:
            kind, payload = q.get()
            self._sse({"kind": kind, **payload})
            if kind in ("done", "error"):
                break


UI_HTML = """<!doctype html><html><head><meta charset=utf-8>
<title>DiffusionGemma - live denoise</title>
<style>
 body{font:15px/1.5 system-ui,sans-serif;max-width:820px;margin:24px auto;padding:0 16px;color:#111}
 h1{font-size:19px} textarea{width:100%;height:80px;font:14px monospace}
 #canvas{white-space:pre-wrap;background:#0b1021;color:#d6e2ff;padding:14px;
   border-radius:8px;min-height:120px;font:14px/1.55 monospace}
 #status{color:#666;font-size:13px;margin:6px 0} button{padding:8px 16px;font-size:14px}
 .commit{color:#7CFC9A}
</style></head><body>
<h1>DiffusionGemma <small style="color:#888">26B-A4B Q8_0 - live denoise</small></h1>
<textarea id=p>Write a haiku about diffusion models.</textarea><br>
<label>blocks <input id=nb type=number value=4 min=1 max=32 style=width:60px></label>
<button onclick=go()>Generate</button>
<div id=status></div>
<div id=canvas></div>
<script>
function go(){
 const c=document.getElementById('canvas'),s=document.getElementById('status');
 c.textContent='';s.textContent='starting...';
 fetch('/ui/generate',{method:'POST',headers:{'Content-Type':'application/json'},
   body:JSON.stringify({prompt:document.getElementById('p').value,
     n_blocks:+document.getElementById('nb').value})}).then(r=>{
  const rd=r.body.getReader(),dec=new TextDecoder();let buf='';
  function pump(){return rd.read().then(({done,value})=>{
   if(done)return; buf+=dec.decode(value,{stream:true});
   let i; while((i=buf.indexOf('\\n\\n'))>=0){
    const ln=buf.slice(0,i).trim();buf=buf.slice(i+2);
    if(!ln.startsWith('data:'))continue;
    const d=JSON.parse(ln.slice(5).trim());
    if(d.kind==='frame'){c.textContent=d.text;
      s.textContent='block '+d.block+'  step '+d.step+'/'+d.total;}
    else if(d.kind==='commit'){c.innerHTML='<span class=commit>'+
      d.text.replace(/[<>&]/g,x=>({'<':'&lt;','>':'&gt;','&':'&amp;'}[x]))+'</span>';}
    else if(d.kind==='stats'){s.textContent='done: '+d.predicted_n+' tok, '+
      (+d.wall_ms).toFixed(0)+' ms, '+d.steps+' steps';}
    else if(d.kind==='error'){s.textContent='error: '+d.message;}
   } return pump();});}
  return pump();});
}
</script></body></html>"""


def main():
    if not ADMIN_TOKEN:
        log("WARNING: DG_ADMIN_TOKEN unset — /admin/* endpoints are UNAUTHENTICATED. "
            "Set DG_ADMIN_TOKEN to require a bearer token.")
    try:
        BACKEND.start()
    except Exception as e:  # noqa: BLE001 — don't die; serve /health and let the supervisor retry
        BACKEND.last_error = str(e)
        BACKEND.state = "failed"
        BACKEND._fail_streak = 1
        log(f"initial backend start failed: {e} — supervisor will keep retrying")
    threading.Thread(target=supervisor_loop, daemon=True, name="supervisor").start()
    httpd = ThreadingHTTPServer((HOST, PORT), Handler)
    log(f"DiffusionGemma management server on http://{HOST}:{PORT}")
    log(f"  UI {HOST}:{PORT}/  | OpenAI POST /v1/chat/completions | health GET /health "
        f"| admin POST /admin/restart")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        BACKEND.stop()


if __name__ == "__main__":
    main()
