import path from "node:path";

const CLASS_SOURCE_EXTENSIONS = new Set([
  ".java",
  ".js",
  ".kt",
  ".mjs",
  ".ts",
  ".tsx",
  ".vue",
]);
const REGEX_PREFIX_WORDS = new Set([
  "await",
  "case",
  "delete",
  "in",
  "instanceof",
  "new",
  "of",
  "return",
  "throw",
  "typeof",
  "void",
  "yield",
]);

export function supportsClassDeclarations(filePath) {
  return CLASS_SOURCE_EXTENSIONS.has(path.posix.extname(filePath));
}

function isRegexLiteralStart(characters, slashIndex) {
  let cursor = slashIndex - 1;
  while (cursor >= 0 && /\s/u.test(characters[cursor])) cursor -= 1;
  if (cursor < 0) return true;

  const previous = characters[cursor];
  if ("([{:;,=!?&|+-*%^~<>".includes(previous)) return true;
  if (!/[A-Za-z_$]/u.test(previous)) return false;

  const wordEnd = cursor + 1;
  while (cursor >= 0 && /[\w$]/u.test(characters[cursor])) cursor -= 1;
  return REGEX_PREFIX_WORDS.has(characters.slice(cursor + 1, wordEnd).join(""));
}

function sanitizeForStructure(source) {
  const characters = [...source];
  let state = "code";
  let regexCharacterClass = false;

  for (let index = 0; index < characters.length; index += 1) {
    const current = characters[index];
    const next = characters[index + 1];
    const third = characters[index + 2];

    if (state === "line-comment") {
      if (current === "\n") state = "code";
      else characters[index] = " ";
      continue;
    }
    if (state === "block-comment") {
      if (current === "*" && next === "/") {
        characters[index] = " ";
        characters[index + 1] = " ";
        index += 1;
        state = "code";
      } else if (current !== "\n") {
        characters[index] = " ";
      }
      continue;
    }
    if (state === "triple-string") {
      if (current === '"' && next === '"' && third === '"') {
        characters[index] = " ";
        characters[index + 1] = " ";
        characters[index + 2] = " ";
        index += 2;
        state = "code";
      } else if (current !== "\n") {
        characters[index] = " ";
      }
      continue;
    }
    if (state === "regex") {
      if (current === "\n") {
        state = "code";
        regexCharacterClass = false;
      } else if (current === "\\") {
        characters[index] = " ";
        if (next !== undefined && next !== "\n") {
          characters[index + 1] = " ";
          index += 1;
        }
      } else {
        if (current === "[") regexCharacterClass = true;
        if (current === "]") regexCharacterClass = false;
        characters[index] = " ";
        if (current === "/" && !regexCharacterClass) state = "code";
      }
      continue;
    }
    if (state === "single-string" || state === "double-string" || state === "template-string") {
      const terminator =
        state === "single-string" ? "'" : state === "double-string" ? '"' : "`";
      if (current === "\\") {
        characters[index] = " ";
        if (next !== undefined && next !== "\n") {
          characters[index + 1] = " ";
          index += 1;
        }
      } else if (current === terminator) {
        characters[index] = " ";
        state = "code";
      } else if (current !== "\n") {
        characters[index] = " ";
      }
      continue;
    }

    if (current === "/" && next === "/") {
      characters[index] = " ";
      characters[index + 1] = " ";
      index += 1;
      state = "line-comment";
    } else if (current === "/" && next === "*") {
      characters[index] = " ";
      characters[index + 1] = " ";
      index += 1;
      state = "block-comment";
    } else if (current === "/" && isRegexLiteralStart(characters, index)) {
      characters[index] = " ";
      regexCharacterClass = false;
      state = "regex";
    } else if (current === '"' && next === '"' && third === '"') {
      characters[index] = " ";
      characters[index + 1] = " ";
      characters[index + 2] = " ";
      index += 2;
      state = "triple-string";
    } else if (current === "'" || current === '"' || current === "`") {
      characters[index] = " ";
      state = current === "'" ? "single-string" : current === '"' ? "double-string" : "template-string";
    }
  }

  return characters.join("");
}

function lineNumberAt(source, index) {
  let line = 1;
  for (let cursor = 0; cursor < index; cursor += 1) {
    if (source[cursor] === "\n") line += 1;
  }
  return line;
}

function closingBraceIndex(source, openingBraceIndex) {
  let depth = 0;
  for (let index = openingBraceIndex; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") depth -= 1;
    if (depth === 0) return index;
  }
  return undefined;
}

export function findClassSpans(source) {
  const sanitized = sanitizeForStructure(source);
  const declarations = [];
  const occurrences = new Map();
  const classPattern =
    /(?<![\w$.:])class\b(?:(?=\s*(?:\{|extends\b))|\s+([A-Za-z_$][\w$]*)\b)/gu;

  for (const match of sanitized.matchAll(classPattern)) {
    const openingBraceIndex = sanitized.indexOf("{", match.index + match[0].length);
    if (openingBraceIndex === -1) continue;
    const endIndex = closingBraceIndex(sanitized, openingBraceIndex);
    if (endIndex === undefined) continue;

    const baseName = match[1] ?? "<anonymous>";
    const occurrence = (occurrences.get(baseName) ?? 0) + 1;
    occurrences.set(baseName, occurrence);
    const name = occurrence === 1 ? baseName : `${baseName}[${occurrence}]`;
    const startLine = lineNumberAt(sanitized, match.index);
    const endLine = lineNumberAt(sanitized, endIndex);
    declarations.push({ name, startLine, endLine, lineCount: endLine - startLine + 1 });
  }

  return declarations;
}

export function evaluateClassSizes(classesByFile, allowlist, maximumLines) {
  const failures = [];
  let oversizedCount = 0;

  for (const [filePath, classes] of classesByFile) {
    for (const classSpan of classes.values()) {
      if (classSpan.lineCount <= maximumLines) continue;

      oversizedCount += 1;
      const allowedMaximum = allowlist[filePath]?.[classSpan.name];
      const label = `${filePath}#${classSpan.name}`;
      if (allowedMaximum === undefined) {
        failures.push(
          `${label}: ${classSpan.lineCount} lines (new class violation; maximum is ${maximumLines})`,
        );
      } else if (classSpan.lineCount > allowedMaximum) {
        failures.push(`${label}: grew from allowed ${allowedMaximum} to ${classSpan.lineCount} lines`);
      }
    }
  }

  for (const [filePath, allowedClasses] of Object.entries(allowlist)) {
    const classes = classesByFile.get(filePath);
    if (!classes) {
      failures.push(`${filePath}: stale class allowlist entry; file does not support class declarations`);
      continue;
    }
    for (const [className, allowedMaximum] of Object.entries(allowedClasses)) {
      const classSpan = classes.get(className);
      const label = `${filePath}#${className}`;
      if (!classSpan) {
        failures.push(`${label}: stale class allowlist entry; class no longer exists`);
      } else if (classSpan.lineCount <= maximumLines) {
        failures.push(`${label}: now ${classSpan.lineCount} lines; remove its stale allowlist entry`);
      } else if (allowedMaximum <= maximumLines) {
        failures.push(`${label}: allowlist maximum ${allowedMaximum} must exceed ${maximumLines}`);
      }
    }
  }

  return { failures, oversizedCount };
}
