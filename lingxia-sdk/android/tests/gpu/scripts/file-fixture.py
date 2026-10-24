"""Local Android file probe server; reverse tcp:39871 through adb."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import time


def pdf():
    content = b"BT /F1 24 Tf 60 740 Td (GPU document preview) Tj ET\n"
    content += b"% transfer fixture padding\n" * 24000
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
        b"/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        b"<< /Length " + str(len(content)).encode() + b" >>\nstream\n" + content + b"endstream",
    ]
    out = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for number, obj in enumerate(objects, 1):
        offsets.append(len(out))
        out.extend(f"{number} 0 obj\n".encode() + obj + b"\nendobj\n")
    xref = len(out)
    out.extend(b"xref\n0 6\n0000000000 65535 f \n")
    for offset in offsets[1:]:
        out.extend(f"{offset:010d} 00000 n \n".encode())
    out.extend(f"trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    return bytes(out)


BODY = pdf()


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path not in ("/sample.pdf", "/slow.pdf"):
            self.send_error(404)
            return
        offset = 0
        value = self.headers.get("Range", "")
        if value.startswith("bytes="):
            offset = int(value[6:].split("-")[0])
        self.send_response(206 if offset else 200)
        self.send_header("Content-Type", "application/pdf")
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("ETag", '"gpu-file-fixture-v1"')
        self.send_header("Content-Length", str(len(BODY) - offset))
        if offset:
            self.send_header("Content-Range", f"bytes {offset}-{len(BODY)-1}/{len(BODY)}")
        self.end_headers()
        try:
            for pos in range(offset, len(BODY), 8192):
                self.wfile.write(BODY[pos:pos + 8192])
                self.wfile.flush()
                if self.path == "/slow.pdf":
                    time.sleep(0.12)
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
            pass


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", 39871), Handler).serve_forever()
