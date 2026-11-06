import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

function client() {
  const requests = [];
  class XHR {
    upload = {};
    constructor() { requests.push(this); }
    open() {}
    setRequestHeader() {}
    send(body) { this.body = body; }
    abort() { this.onabort(); }
  }
  const exports = {};
  const source = ts.transpileModule(readFileSync("src/lib/api/client.ts", "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  vm.runInNewContext(source, { exports, process: { env: {} }, XMLHttpRequest: XHR, FormData, URLSearchParams });
  return { api: exports, requests };
}

test("oversized documents reject before constructing or sending a request", async () => {
  const { api, requests } = client();
  const result = api.uploadDocument({ size: api.DOCUMENT_MAX_BYTES + 1 }, () => assert.fail("no progress expected"));
  await assert.rejects(result.promise, /20 MB limit/);
  assert.equal(requests.length, 0);
});

test("exactly 20 MB is accepted; byte progress transitions to processing and success", async () => {
  const { api, requests } = client();
  const updates = [];
  const file = new File([new Uint8Array(api.DOCUMENT_MAX_BYTES)], "boundary.txt");
  const result = api.uploadDocument(file, value => updates.push(value));
  const xhr = requests[0];
  assert.equal(xhr.body.get("file").size, api.DOCUMENT_MAX_BYTES);
  xhr.upload.onprogress({ lengthComputable: true, loaded: 1024, total: file.size });
  assert.equal(updates[0].loaded, 1024);
  xhr.upload.onload();
  assert.equal(updates[1].phase, "processing");
  xhr.status = 200;
  xhr.responseText = JSON.stringify({ document: { format: "text" }, structure: { sections: [] } });
  xhr.onload();
  assert.equal((await result.promise).document.format, "text");
});

test("cancellation while processing rejects without a result", async () => {
  const { api, requests } = client();
  const result = api.uploadDocument(new File(["hello"], "test.txt"), () => {});
  requests[0].upload.onload();
  result.abort();
  await assert.rejects(result.promise, /Upload cancelled/);
});

test("server size rejection remains visible to the caller", async () => {
  const { api, requests } = client();
  const result = api.uploadDocument(new File(["hello"], "test.txt"), () => {});
  requests[0].status = 413;
  requests[0].responseText = JSON.stringify({ detail: { error: "file_too_large", max_bytes: 20971520 } });
  requests[0].onload();
  await assert.rejects(result.promise, /20 MB limit/);
});
