/** Bound Edge Function request bodies before JSON/text parse (P0.10). */

export type ReadBodyResult =
  | { ok: true; text: string }
  | { ok: false; status: 413; error: string };

/**
 * Reject oversized bodies via Content-Length first, then cap cumulative
 * stream bytes so missing/lying Content-Length cannot buffer unbounded.
 */
export async function readBodyTextBounded(
  req: Request,
  maxBytes: number,
): Promise<ReadBodyResult> {
  const contentLength = req.headers.get("content-length");
  if (contentLength !== null) {
    const declared = Number(contentLength);
    if (Number.isFinite(declared) && declared > maxBytes) {
      return { ok: false, status: 413, error: "Request body too large" };
    }
  }

  if (!req.body) {
    return { ok: true, text: "" };
  }

  const reader = req.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      if (!value) continue;
      total += value.byteLength;
      if (total > maxBytes) {
        try {
          await reader.cancel();
        } catch {
          // ignore cancel errors
        }
        return { ok: false, status: 413, error: "Request body too large" };
      }
      chunks.push(value);
    }
  } catch {
    return { ok: false, status: 413, error: "Request body too large" };
  }

  const merged = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return { ok: true, text: new TextDecoder().decode(merged) };
}
