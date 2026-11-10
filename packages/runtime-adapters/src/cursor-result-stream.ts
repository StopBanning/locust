/** Cursor repeats its streamed answer in result. Bound strings while receiving that
 * record so an arbitrarily long answer cannot hide the terminal status after it.
 * Keep JSON escapes whole across chunks; malformed JSON still fails in the adapter.
 * Other records pass through unchanged and retain the runner's ordinary cap.
 */
export function cursorResultStream(): (chunk: string) => string {
  let head = "";
  let result: boolean | undefined;
  let quoted = false;
  let escape = "";
  let unicodeLeft = 0;
  let kept = 0;
  const stringCap = 64 * 1024;
  return (chunk) => {
    const out: string[] = [];
    const accept = (char: string): void => {
      if (!result) { out.push(char); return; }
      if (!quoted) {
        out.push(char);
        if (char === '"') { quoted = true; kept = 0; }
      } else if (escape.length > 0) {
        escape += char;
        if (escape.length === 2 && char === 'u') unicodeLeft = 4;
        else if (unicodeLeft > 0) unicodeLeft--;
        if (unicodeLeft === 0) {
          if (kept + escape.length <= stringCap) out.push(escape);
          kept += escape.length;
          escape = "";
        }
      } else if (char === '"') { out.push(char); quoted = false; }
      else if (char === '\\') escape = char;
      else { if (kept < stringCap) out.push(char); kept++; }
    };
    for (const char of chunk) {
      if (result === undefined) {
        head += char;
        if (head.length < 64 && char !== '\n') continue;
        result = /^\s*\{\s*"type"\s*:\s*"result"\s*[,}]/.test(head);
        for (const first of head) accept(first);
        head = "";
      } else accept(char);
      if (char === '\n') { result = undefined; quoted = false; escape = ""; unicodeLeft = 0; }
    }
    return out.join("");
  };
}
