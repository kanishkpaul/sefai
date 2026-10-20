"""Dedicated MCP transport fixture; stdio and authenticated loopback HTTP."""
import argparse
import base64
import hashlib
from urllib.parse import parse_qs, urlencode, urlparse
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

parser = argparse.ArgumentParser()
parser.add_argument("--http", action="store_true")
parser.add_argument("--mode", default="normal")
options = parser.parse_args()


def reply(request):
    if "id" not in request:
        return None
    method = request["method"]
    params = request.get("params", {})
    if method == "initialize":
        result = {"protocolVersion": "2099-01-01" if options.mode == "version" else "2025-11-25",
                  "serverInfo": {"name": "sefai-fixture", "version": "1"},
                  "capabilities": {"tools": {}, "resources": {}, "prompts": {}}}
    elif method == "tools/list":
        second = bool(params.get("cursor"))
        name = "echo" if not second or options.mode == "duplicate" else "remember"
        result = {"tools": [{"name": name, "description": "Echo an untrusted value" if name == "echo" else "Create fixture output",
                             "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}}]}
        if not second:
            result["nextCursor"] = "page-2"
    elif method == "resources/list":
        result = {"resources": [{"uri": "fixture://note", "name": "note"}]}
    elif method == "prompts/list":
        result = {"prompts": [{"name": "greet"}]}
    elif method == "resources/read":
        result = {"contents": [{"uri": "fixture://note", "text": "untrusted fixture resource"}]}
    elif method == "prompts/get":
        result = {"messages": [{"role": "user", "content": {"type": "text", "text": "untrusted fixture prompt"}}]}
    elif method == "tools/call":
        text = params.get("arguments", {}).get("text", "")
        if params["name"] == "remember":
            with open("fixture-write.txt", "x", encoding="utf-8") as target:
                target.write(text)
        if options.mode == "disconnect":
            os._exit(0)
        result = {"content": [{"type": "text", "text": text}], "structuredContent": {"echo": text}, "isError": False}
    else:
        return {"jsonrpc": "2.0", "id": request["id"], "error": {"code": -32601, "message": "unsupported fixture method"}}
    return {"jsonrpc": "2.0", "id": request["id"], "result": result}


def encode(request):
    if options.mode == "disconnect-init":
        os._exit(0)
    if options.mode == "malformed":
        return b"not-json"
    if options.mode == "oversized":
        return b"x" * (65536 + 1)
    result = reply(request)
    return json.dumps(result).encode() if result else b""


if options.http:
    authorization = {}

    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *args):
            pass

        def send_json(self, value):
            body = json.dumps(value).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            base = f"http://127.0.0.1:{self.server.server_port}"
            target = urlparse(self.path)
            if target.path.startswith("/.well-known/oauth-protected-resource"):
                self.send_json({"resource": base + "/mcp", "authorization_servers": ["http://127.0.0.1:1" if options.mode == "oauth-untrusted-origin" else base], "scopes_supported": ["tools"]})
            elif target.path == "/.well-known/oauth-authorization-server":
                self.send_json({"issuer": base + "/other" if options.mode == "oauth-wrong-issuer" else base, "authorization_endpoint": base + "/authorize", "token_endpoint": base + "/token", "response_types_supported": ["code"], "grant_types_supported": ["authorization_code"], "code_challenge_methods_supported": ["S256"]})
            elif target.path == "/authorize":
                values = {key: value[0] for key, value in parse_qs(target.query).items()}
                authorization.update(values)
                callback = values["redirect_uri"] + "?" + urlencode({"code": "fixture-code", "state": "invalid" if options.mode == "oauth-bad-state" else values["state"], "iss": base})
                self.send_response(302)
                self.send_header("Location", callback)
                self.send_header("Content-Length", "0")
                self.end_headers()
            elif target.path == "/mcp":
                self.send_response(401)
                self.send_header("WWW-Authenticate", f'Bearer resource_metadata="{base}/.well-known/oauth-protected-resource"')
                self.send_header("Content-Length", "0")
                self.end_headers()
            else:
                self.send_error(404)

        def do_POST(self):
            length = int(self.headers.get("content-length", "0"))
            if length > 65536:
                self.send_error(413)
                return
            raw = self.rfile.read(length)
            if self.path == "/token":
                values = {key: value[0] for key, value in parse_qs(raw.decode()).items()}
                challenge = base64.urlsafe_b64encode(hashlib.sha256(values.get("code_verifier", "").encode()).digest()).rstrip(b"=").decode()
                if values.get("code") != "fixture-code" or challenge != authorization.get("code_challenge") or values.get("resource") != f"http://127.0.0.1:{self.server.server_port}/mcp":
                    self.send_error(400)
                    return
                with open("fixture-oauth-verified.txt", "w", encoding="utf-8") as marker:
                    marker.write("PKCE and resource verified")
                self.send_json({"access_token": "fixture-only-token", "token_type": "Bearer", "expires_in": 300, "scope": "tools"})
                return
            request = json.loads(raw)
            if self.headers.get("authorization") != "Bearer fixture-only-token":
                self.send_error(401)
                return
            if options.mode == "redirect":
                self.send_response(307)
                self.send_header("Location", "/must-not-follow")
                self.send_header("Content-Length", "0")
                self.end_headers()
                return
            body = encode(request)
            if options.mode == "sse" and body:
                body = b"data: " + body + b"\n\n"
            self.send_response(200 if body else 202)
            self.send_header("Content-Type", "text/event-stream" if options.mode == "sse" else "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_DELETE(self):
            self.send_response(204)
            self.send_header("Content-Length", "0")
            self.end_headers()

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    print(f"http://127.0.0.1:{server.server_port}/mcp", flush=True)
    server.serve_forever()
else:
    if options.mode == "child":
        import subprocess
        child = subprocess.Popen([sys.executable, "-c", "import os, threading; print(os.getpid(), flush=True); threading.Event().wait()"], stdout=subprocess.PIPE)
        with open("fixture-child.txt", "w", encoding="utf-8") as marker:
            marker.write(child.stdout.readline().decode().strip())
    with open("fixture-started.txt", "w", encoding="utf-8") as marker:
        marker.write(str(os.getpid()))
    for line in sys.stdin:
        request = json.loads(line)
        body = encode(request)
        if body:
            sys.stdout.buffer.write(body + b"\n")
            sys.stdout.buffer.flush()
