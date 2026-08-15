#!/usr/bin/env python3
"""Self-contained test for ontology_proxy.py. STDLIB ONLY.

Starts a fake OpenAI-compatible upstream (records every request body and can
be scripted with canned responses) plus proxy instances in scaffold / off /
tools modes on ephemeral ports, then asserts:

  * /health works and reports the fixture index
  * scaffold mode injects an [ONTOLOGY CONTEXT] system message, strips
    ``stream``, and annotates the response with ontology.injected_tokens
  * off mode is a byte-faithful passthrough (no injection, no annotation)
  * tools mode advertises the three ontology tools, executes one canned
    tool round locally, feeds the tool result back upstream, and returns the
    final answer with ontology.tool_calls == 1
  * generic /v1/* passthrough (GET /v1/models)
  * bad JSON -> 400; dead upstream -> 502

Uses the inline fixture index from ontology_scaffold (written to a tempdir).

Run:  python3 test_proxy.py
"""

from __future__ import annotations

import json
import os
import sys
import tempfile
import threading
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ontology_proxy
import ontology_scaffold

FAILURES = 0


def check(name: str, cond: bool, detail: str = "") -> None:
    global FAILURES
    status = "ok" if cond else "FAIL"
    line = "  [%s] %s" % (status, name)
    if detail and not cond:
        line += " — %s" % detail
    print(line)
    if not cond:
        FAILURES += 1


# ---------------------------------------------------------------------------
# Fake upstream
# ---------------------------------------------------------------------------

class FakeUpstream(BaseHTTPRequestHandler):
    requests: list = []   # parsed chat bodies, in order
    script: list = []     # canned responses to pop; empty -> default echo

    def log_message(self, *args):
        pass

    def _json(self, status: int, obj) -> None:
        payload = json.dumps(obj).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        body = json.loads(self.rfile.read(length).decode("utf-8"))
        if self.path == "/v1/chat/completions":
            FakeUpstream.requests.append(body)
            if FakeUpstream.script:
                self._json(200, FakeUpstream.script.pop(0))
            else:
                self._json(200, {
                    "id": "chatcmpl-fake", "object": "chat.completion",
                    "created": 0, "model": "fake",
                    "choices": [{"index": 0, "finish_reason": "stop",
                                 "message": {"role": "assistant",
                                             "content": "canned answer"}}],
                    "usage": {"prompt_tokens": 1, "completion_tokens": 1,
                              "total_tokens": 2},
                    "echo_messages": body.get("messages"),
                    "echo_stream": body.get("stream", "ABSENT"),
                })
        else:
            self._json(404, {"error": "no such path: %s" % self.path})

    def do_GET(self):
        if self.path == "/v1/models":
            self._json(200, {"object": "list",
                             "data": [{"id": "fake-model", "object": "model"}]})
        else:
            self._json(404, {"error": "no such path: %s" % self.path})


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def http_json(method: str, url: str, body=None):
    """Returns (status, parsed_json)."""
    data = json.dumps(body).encode("utf-8") if body is not None else None
    req = urllib.request.Request(
        url, data=data, method=method,
        headers={"Content-Type": "application/json"} if data else {},
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return resp.status, json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        return exc.code, json.loads(exc.read().decode("utf-8"))


def http_raw(method: str, url: str, raw: bytes):
    req = urllib.request.Request(
        url, data=raw, method=method,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return resp.status, json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        return exc.code, json.loads(exc.read().decode("utf-8"))


def start(httpd) -> None:
    t = threading.Thread(target=httpd.serve_forever, daemon=True)
    t.start()


def make_proxy(upstream: str, mode: str, index_path: str):
    cfg = ontology_proxy.Config()
    cfg.host = "127.0.0.1"
    cfg.port = 0                      # ephemeral
    cfg.upstream = upstream
    cfg.mode = mode
    cfg.index_path = index_path
    cfg.budget = 800
    cfg.timeout = 15.0
    httpd, _ = ontology_proxy.build_server(cfg)
    start(httpd)
    return "http://127.0.0.1:%d" % httpd.server_address[1], httpd


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

def main() -> int:
    print("test_proxy: ontology_proxy end-to-end")

    tmpdir = tempfile.mkdtemp(prefix="onto-proxy-test-")
    index_path = os.path.join(tmpdir, "scaffold-index.json")
    with open(index_path, "w", encoding="utf-8") as fh:
        json.dump(ontology_scaffold._FIXTURE, fh)

    upstream_httpd = ThreadingHTTPServer(("127.0.0.1", 0), FakeUpstream)
    start(upstream_httpd)
    upstream = "http://127.0.0.1:%d" % upstream_httpd.server_address[1]

    scaffold_url, _s1 = make_proxy(upstream, "scaffold", index_path)
    off_url, _s2 = make_proxy(upstream, "off", index_path)
    tools_url, _s3 = make_proxy(upstream, "tools", index_path)
    dead_url, _s4 = make_proxy("http://127.0.0.1:9", "scaffold", index_path)

    n_classes = len(ontology_scaffold._FIXTURE["classes"])
    chat = "/v1/chat/completions"

    # --- /health ----------------------------------------------------------
    status, health = http_json("GET", scaffold_url + "/health")
    check("health 200", status == 200)
    check("health ok/mode/upstream", health.get("ok") is True
          and health.get("mode") == "scaffold"
          and health.get("upstream") == upstream, repr(health))
    check("health index_classes", health.get("index_classes") == n_classes,
          repr(health))
    _, health_off = http_json("GET", off_url + "/health")
    check("health reflects off mode", health_off.get("mode") == "off")

    # --- scaffold mode ----------------------------------------------------
    FakeUpstream.requests.clear()
    FakeUpstream.script.clear()
    sent = {
        "model": "m",
        "messages": [{"role": "user", "content": "what is a knowledge graph?"}],
        "stream": True,
    }
    status, resp = http_json("POST", scaffold_url + chat, sent)
    check("scaffold 200", status == 200, repr(resp))
    check("scaffold ontology field",
          resp.get("ontology", {}).get("mode") == "scaffold", repr(resp.get("ontology")))
    check("scaffold injected_tokens > 0",
          resp.get("ontology", {}).get("injected_tokens", 0) > 0,
          repr(resp.get("ontology")))
    check("scaffold upstream called once", len(FakeUpstream.requests) == 1)
    fwd = FakeUpstream.requests[-1]
    check("scaffold stream stripped", "stream" not in fwd, repr(fwd.keys()))
    msgs = fwd.get("messages") or []
    check("scaffold system message injected",
          bool(msgs) and msgs[0].get("role") == "system"
          and ontology_scaffold.HEADER in msgs[0].get("content", ""),
          repr(msgs[:1]))
    check("scaffold user message preserved",
          msgs[-1] == {"role": "user", "content": "what is a knowledge graph?"},
          repr(msgs[-1:]))
    check("scaffold upstream JSON verbatim",
          resp.get("choices", [{}])[0].get("message", {}).get("content")
          == "canned answer")

    # --- off mode: pure passthrough --------------------------------------
    FakeUpstream.requests.clear()
    status, resp = http_json("POST", off_url + chat, sent)
    check("off 200", status == 200)
    check("off no ontology field", "ontology" not in resp, repr(resp.keys()))
    check("off body untouched", FakeUpstream.requests[-1] == sent,
          repr(FakeUpstream.requests[-1]))
    check("off stream preserved", FakeUpstream.requests[-1].get("stream") is True)
    check("off no injection",
          resp.get("echo_messages") == sent["messages"], repr(resp.get("echo_messages")))

    # --- tools mode: one canned tool round -------------------------------
    FakeUpstream.requests.clear()
    FakeUpstream.script[:] = [
        {   # round 1: model asks for ontology_search
            "id": "chatcmpl-t1", "object": "chat.completion", "created": 0,
            "model": "fake",
            "choices": [{"index": 0, "finish_reason": "tool_calls",
                         "message": {"role": "assistant", "content": None,
                                     "tool_calls": [{
                                         "id": "call_1", "type": "function",
                                         "function": {
                                             "name": "ontology_search",
                                             "arguments": json.dumps(
                                                 {"query": "knowledge graph"}),
                                         }}]}}],
        },
        {   # round 2: final answer
            "id": "chatcmpl-t2", "object": "chat.completion", "created": 0,
            "model": "fake",
            "choices": [{"index": 0, "finish_reason": "stop",
                         "message": {"role": "assistant",
                                     "content": "final grounded answer"}}],
        },
    ]
    status, resp = http_json("POST", tools_url + chat, {
        "model": "m",
        "messages": [{"role": "user", "content": "tell me about knowledge graphs"}],
    })
    check("tools 200", status == 200, repr(resp))
    check("tools final answer",
          resp.get("choices", [{}])[0].get("message", {}).get("content")
          == "final grounded answer", repr(resp))
    check("tools ontology field",
          resp.get("ontology") == {"mode": "tools", "tool_calls": 1},
          repr(resp.get("ontology")))
    check("tools two upstream calls", len(FakeUpstream.requests) == 2)
    first, second = FakeUpstream.requests
    advertised = {(t.get("function") or {}).get("name")
                  for t in first.get("tools", [])}
    check("tools advertised",
          {"ontology_search", "ontology_class_get",
           "ontology_neighbours"} <= advertised, repr(advertised))
    tool_msgs = [m for m in second.get("messages", [])
                 if m.get("role") == "tool"]
    check("tool result fed back", len(tool_msgs) == 1
          and tool_msgs[0].get("tool_call_id") == "call_1", repr(tool_msgs))
    tool_payload = json.loads(tool_msgs[0]["content"]) if tool_msgs else {}
    check("tool result contains knowledge-graph match",
          any(m.get("slug") == "knowledge-graph"
              for m in tool_payload.get("matches", [])), repr(tool_payload))
    assistant_tc = [m for m in second.get("messages", [])
                    if m.get("role") == "assistant" and m.get("tool_calls")]
    check("assistant tool_call message replayed", len(assistant_tc) == 1)

    # --- generic /v1/* passthrough ---------------------------------------
    status, models = http_json("GET", scaffold_url + "/v1/models")
    check("models passthrough", status == 200
          and models.get("data", [{}])[0].get("id") == "fake-model", repr(models))

    # --- error surfaces ---------------------------------------------------
    status, err = http_raw("POST", scaffold_url + chat, b"{not json")
    check("bad JSON -> 400", status == 400
          and err.get("error", {}).get("type") == "invalid_request_error",
          repr((status, err)))
    status, err = http_json("POST", dead_url + chat, sent)
    check("dead upstream -> 502", status == 502
          and err.get("error", {}).get("type") == "upstream_error",
          repr((status, err)))

    print("test_proxy: %s" % ("PASS" if FAILURES == 0
                              else "%d FAILURE(S)" % FAILURES))
    return 0 if FAILURES == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
