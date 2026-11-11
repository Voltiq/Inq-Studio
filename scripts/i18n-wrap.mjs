// Wraps the interface's strings in t(…) and lists them.
//
//   node scripts/i18n-wrap.mjs            list the strings found (writes nothing)
//   node scripts/i18n-wrap.mjs --write    rewrite the sources
//   node scripts/i18n-wrap.mjs --keys     print every key already wrapped, as JSON
//
// A string is taken for interface text when it reads like text (has a space,
// or is a capitalised word) and sits where text is shown: JSX text, a text
// attribute, or an expression that is not a comparison, a key, a class name
// or an argument of a non-display function.
import ts from "typescript";
import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const mode = process.argv[2] ?? "";
const files = ["src/App.tsx", ...["src/components", "src/components/ui"].flatMap((d) => readdirSync(d).filter((f) => f.endsWith(".tsx") && !f.endsWith(".test.tsx")).map((f) => join(d, f)))];

const TEXT_ATTRS = new Set(["label", "title", "placeholder", "helper", "description", "aria-label", "data-tooltip", "alt"]);
const SKIP_ATTRS = new Set([
  "className", "type", "role", "id", "key", "style", "htmlFor", "name", "autoComplete", "appearance", "spacing", "width", "value", "src", "href", "shortcut",
  "data-tooltip-kbd", "viewBox", "d", "fill", "stroke", "tool", "kind", "align", "icon", "accept", "inputMode", "pattern", "target", "rel", "dir", "lang",
]);
const SKIP_PROPS = new Set(["value", "kind", "type", "key", "id", "className", "tool", "icon", "appearance", "spacing", "width", "align", "font", "name", "tab", "position", "style", "color", "family", "extensions", "method", "event", "shortcut"]);
const SKIP_CALLS = /^(invoke|log|querySelector|querySelectorAll|includes|startsWith|endsWith|test|exec|setAttribute|getAttribute|addEventListener|removeEventListener|getContext|toBlob|toDataURL|createElement|cx|padStart|padEnd|split|join|replace|match|indexOf|setItem|getItem|removeItem|slice|localeCompare|toLocaleString|toLocaleLowerCase|has|get|set|add|delete|t|plural|parseChoice|setTool|chooseTool|onTool|setFormat|setSource|setPart|setDpi|setCorner|setMode|setScope|setTab|onTab|setSideTab|RegExp|scrollIntoView|scrollTo|hexToRgb|setData|getData|setProperty|closest|matches|contains|open|import)$/;

const looksLikeText = (s) => {
  const v = s.trim();
  if (v.length < 2 || !/[A-Za-zÀ-ÿ]{2}/.test(v)) return false;
  if (/^[a-z0-9_.:/#%-]+$/.test(v)) return false; // identifiers, class names, units
  if (/^(#|\.|--|var\(|rgba?\(|https?:|\/|[A-Z]:\\)/.test(v)) return false;
  if (/^[A-Z][A-Za-z0-9]*$/.test(v) && !/^[A-Z][a-z]{2,}$/.test(v)) return false; // CamelCase and ACRONYMS
  if (/^(Enter|Escape|Tab|Home|End|Delete|Backspace|Shift|Ctrl|Alt|Arrow\w+|Page(Up|Down)|Helvetica|Arial|Courier|Times|Symbol|Normal|Standard|Identity|Type|Font|Image|Form|Figure|Formula|Document|Popup|Link|Widget|Text|None|Default)$/.test(v)) return false;
  return /\s/.test(v) || /^[A-Z][a-z]{2,}[.…:!?]?$/.test(v);
};

const found = new Map();
const note = (key, file) => found.set(key, [...(found.get(key) ?? []), file]);
const q = (s) => JSON.stringify(s);

function calleeName(call) {
  const e = call.expression;
  if (ts.isIdentifier(e)) return e.text;
  if (ts.isPropertyAccessExpression(e)) return e.name.text;
  return "";
}

function skipped(node) {
  let child = node;
  for (let p = node.parent; p; child = p, p = p.parent) {
    if (ts.isImportDeclaration(p) || ts.isExportDeclaration(p) || ts.isTypeNode(p) || ts.isLiteralTypeNode(p)) return true;
    if (ts.isTypeAliasDeclaration(p) || ts.isInterfaceDeclaration(p) || ts.isEnumDeclaration(p)) return true;
    if (ts.isCaseClause(p) && p.expression === child) return true;
    if (ts.isElementAccessExpression(p) && p.argumentExpression === child) return true;
    if (ts.isBinaryExpression(p) && /^(===|!==|==|!=)$/.test(p.operatorToken.getText())) return child === node || ts.isParenthesizedExpression(child);
    if (ts.isPropertyAssignment(p)) {
      if (p.name === child) return true;
      if (p.initializer === child && SKIP_PROPS.has(p.name.getText().replace(/['"]/g, ""))) return true;
    }
    if (ts.isJsxAttribute(p)) return SKIP_ATTRS.has(p.name.getText());
    if (ts.isCallExpression(p) || ts.isNewExpression(p)) {
      const name = ts.isCallExpression(p) ? calleeName(p) : p.expression.getText();
      if (SKIP_CALLS.test(name)) return true;
    }
    if (ts.isAsExpression(p) && p.type.getText() === "const") return true;
    if (ts.isTemplateExpression(p) && child !== p.head) return false;
    if (ts.isFunctionLike(p) || ts.isJsxExpression(p) || ts.isVariableStatement(p)) continue;
  }
  return false;
}

for (const file of files) {
  const text = readFileSync(file, "utf8");
  const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const edits = [];
  const visit = (node) => {
    if (ts.isJsxText(node)) {
      const raw = node.getText();
      const key = raw.replace(/\s+/g, " ").trim().replace(/&amp;/g, "&").replace(/&nbsp;/g, " ");
      if (key && /[A-Za-zÀ-ÿ]{2}/.test(key) && !/^[a-z]+$/.test(key)) {
        note(key, file);
        const lead = /^\s*\n/.test(raw) || !/^\s/.test(raw) ? "" : '{" "}';
        const trail = /\n\s*$/.test(raw) || !/\s$/.test(raw) ? "" : '{" "}';
        const [pre, post] = [raw.match(/^\s*/)[0], raw.match(/\s*$/)[0]];
        edits.push([node.getStart(sf, false) - 0, node.getEnd(), `${pre.includes("\n") ? pre : ""}${lead}{t(${q(key)})}${trail}${post.includes("\n") ? post : ""}`, node.pos]);
      }
      return;
    }
    if (ts.isJsxAttribute(node) && node.initializer && ts.isStringLiteral(node.initializer)) {
      if (TEXT_ATTRS.has(node.name.getText()) && /[A-Za-zÀ-ÿ]{2}/.test(node.initializer.text)) {
        note(node.initializer.text, file);
        edits.push([node.initializer.getStart(sf), node.initializer.getEnd(), `{t(${q(node.initializer.text)})}`]);
      }
      return;
    }
    if ((ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) && looksLikeText(node.text) && !skipped(node)) {
      note(node.text, file);
      edits.push([node.getStart(sf), node.getEnd(), `t(${q(node.text)})`]);
      return;
    }
    if (ts.isTemplateExpression(node) && !skipped(node)) {
      let key = node.head.text;
      const args = [];
      node.templateSpans.forEach((span, i) => {
        key += `{${i}}` + span.literal.text;
        args.push(span.expression.getText(sf));
      });
      if (looksLikeText(key.replace(/\{\d+\}/g, " x "))) {
        // Expressions inside may hold strings of their own; they are left as they are.
        note(key, file);
        edits.push([node.getStart(sf), node.getEnd(), `t(${q(key)}, ${args.join(", ")})`]);
        return;
      }
    }
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "t") {
      const a = node.arguments[0];
      if (a && ts.isStringLiteral(a)) note(a.text, file);
      return;
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
  if (mode === "--write" && edits.length) {
    let out = text;
    for (const [from, to, repl] of edits.sort((a, b) => b[0] - a[0])) out = out.slice(0, from) + repl + out.slice(to);
    if (!/from "(\.\.?\/)+lib\/i18n"/.test(out)) {
      const depth = file.replace(/\\/g, "/").split("/").length - 2;
      const rel = depth === 0 ? "./lib/i18n" : `${"../".repeat(depth)}lib/i18n`;
      const at = out.lastIndexOf("\nimport ");
      const end = out.indexOf(";\n", at) + 2;
      out = out.slice(0, end) + `import { t } from "${rel}";\n` + out.slice(end);
    }
    writeFileSync(file, out);
  }
}

if (mode === "--keys") console.log(JSON.stringify([...found.keys()], null, 0));
else {
  for (const [k, f] of found) console.log(q(k), "  <", [...new Set(f)].map((x) => x.replace(/.*[\\/]/, "")).join(","));
  console.error(found.size, "strings");
}
