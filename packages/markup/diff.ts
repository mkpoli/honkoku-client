export interface Change {
  kind: "equal" | "insert" | "delete";
  text: string;
}
/** Linear-space LCS over Unicode code points; common edges avoid work on long saves. */
export function diffSource(before: string, after: string): Change[] {
  const a = Array.from(before),
    b = Array.from(after),
    result: Change[] = [];
  const emit = (kind: Change["kind"], text: string) => {
    if (!text) return;
    const last = result.at(-1);
    if (last?.kind === kind) last.text += text;
    else result.push({ kind, text });
  };
  function scores(x: string[], y: string[]) {
    let row = new Uint32Array(y.length + 1);
    for (const c of x) {
      const next = new Uint32Array(y.length + 1);
      for (let j = 0; j < y.length; j++)
        next[j + 1] = c === y[j] ? row[j] + 1 : Math.max(row[j + 1], next[j]);
      row = next;
    }
    return row;
  }
  function walk(x: string[], y: string[]) {
    let prefix = 0;
    while (prefix < x.length && prefix < y.length && x[prefix] === y[prefix])
      prefix++;
    emit("equal", x.slice(0, prefix).join(""));
    x = x.slice(prefix);
    y = y.slice(prefix);
    let suffix = 0;
    while (
      suffix < x.length &&
      suffix < y.length &&
      x[x.length - 1 - suffix] === y[y.length - 1 - suffix]
    )
      suffix++;
    const tail = x.slice(x.length - suffix).join("");
    x = x.slice(0, x.length - suffix);
    y = y.slice(0, y.length - suffix);
    const alphabet = new Set(x);
    if (!x.length) emit("insert", y.join(""));
    else if (!y.length) emit("delete", x.join(""));
    else if (x.length === 1) {
      const match = y.indexOf(x[0]);
      if (match < 0) {
        emit("delete", x[0]);
        emit("insert", y.join(""));
      } else {
        emit("insert", y.slice(0, match).join(""));
        emit("equal", x[0]);
        emit("insert", y.slice(match + 1).join(""));
      }
    } else if (!y.some((c) => alphabet.has(c))) {
      emit("delete", x.join(""));
      emit("insert", y.join(""));
    } else {
      const mid = Math.floor(x.length / 2);
      const left = scores(x.slice(0, mid), y),
        right = scores(x.slice(mid).reverse(), [...y].reverse());
      let split = 0;
      for (let j = 1; j <= y.length; j++)
        if (
          left[j] + right[y.length - j] >
          left[split] + right[y.length - split]
        )
          split = j;
      walk(x.slice(0, mid), y.slice(0, split));
      walk(x.slice(mid), y.slice(split));
    }
    emit("equal", tail);
  }
  walk(a, b);
  return result;
}
