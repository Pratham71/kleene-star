// Every file package.json points at (icons, grammar, snippets, language
// configuration, the main script) must be tracked by git and not excluded
// by .vscodeignore; otherwise the packaged extension silently lacks it.
// The file icon was once lost this way, to a `*.svg` rule in .gitignore.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const root = path.join(__dirname, "..");
const manifest = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));

/// Every string in the manifest that looks like a path to a file of ours.
function referencedFiles(value, found = new Set()) {
  if (typeof value === "string") {
    if (/^\.?\.?\/?[\w-]+(\/[\w.-]+)*\.(png|svg|json|js)$/.test(value) && !value.startsWith("http")) {
      found.add(path.normalize(value));
    }
  } else if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(value)) {
      // Dependencies are names and version ranges, not files.
      if (!/dependencies$/i.test(key)) referencedFiles(child, found);
    }
  }
  return found;
}

const files = [...referencedFiles(manifest)];

/// A .vscodeignore pattern as a regular expression: `**/` is any folders,
/// `**` anything, `*` anything within one folder.
function globToRegExp(glob) {
  let source = "";
  for (let i = 0; i < glob.length; i++) {
    if (glob.startsWith("**/", i)) {
      source += "(.*/)?";
      i += 2;
    } else if (glob.startsWith("**", i)) {
      source += ".*";
      i += 1;
    } else if (glob[i] === "*") {
      source += "[^/]*";
    } else if (/[\w\/-]/.test(glob[i])) {
      source += glob[i];
    } else {
      source += "\\" + glob[i];
    }
  }
  return new RegExp("^" + source + "$");
}

test("package.json refers to files", () => {
  for (const expected of ["extension.js", path.normalize("images/icon.png"), path.normalize("images/kstar-file.svg")]) {
    assert.ok(files.includes(expected), `${expected} not found among ${files.join(", ")}`);
  }
});

test("every file package.json refers to is tracked by git", () => {
  const tracked = new Set(
    execFileSync("git", ["ls-files"], { cwd: root, encoding: "utf8" })
      .split("\n")
      .filter(Boolean)
      .map((f) => path.normalize(f)),
  );
  const missing = files.filter((f) => !tracked.has(f));
  assert.deepStrictEqual(missing, [], "these files would be missing from the packaged extension");
});

test("no file package.json refers to is excluded from the package", () => {
  const ignore = fs
    .readFileSync(path.join(root, ".vscodeignore"), "utf8")
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("#"));
  for (const file of files) {
    const unix = file.split(path.sep).join("/");
    for (const pattern of ignore) {
      const regex = globToRegExp(pattern);
      assert.ok(!regex.test(unix), `${unix} is excluded by "${pattern}" in .vscodeignore`);
    }
  }
});
