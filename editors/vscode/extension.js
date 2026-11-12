// The Kleene Star extension.
//
// Everything that understands the language lives in two programs: the
// `kstar-lsp` language server (errors, hover, definitions, completion,
// outline, formatting) and the `kstar` command (run, check, trace). This
// file finds them, starts the server, and adds commands that run `kstar` on
// the current file.

const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");
const { execFile } = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");

const windows = process.platform === "win32";

let client;
let output;
let terminal;

/// The folders where an installer or `cargo install` puts the programs,
/// tried after the PATH.
function installFolders() {
  const home = os.homedir();
  const folders = [path.join(home, ".cargo", "bin"), path.join(home, ".kstar", "bin")];
  if (windows && process.env.LOCALAPPDATA) {
    folders.push(path.join(process.env.LOCALAPPDATA, "Programs", "Kleene Star", "bin"));
  }
  if (!windows) {
    folders.push("/usr/local/bin", path.join(home, ".local", "bin"));
  }
  return folders;
}

/// The full path of program `name`: the setting if it is set, otherwise the
/// first match on the PATH or in the install folders. `undefined` if none.
function find(name, setting) {
  const configured = vscode.workspace.getConfiguration("kstar").get(setting);
  if (configured) {
    return fs.existsSync(configured) ? configured : undefined;
  }
  const file = windows ? `${name}.exe` : name;
  const folders = (process.env.PATH || "").split(path.delimiter).filter(Boolean).concat(installFolders());
  for (const folder of folders) {
    const candidate = path.join(folder, file);
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return undefined;
}

/// Says that `name` was not found, and offers the two ways out.
async function missing(name, setting) {
  const configured = vscode.workspace.getConfiguration("kstar").get(setting);
  const problem = configured
    ? `Kleene Star: "${configured}" (the kstar.${setting} setting) does not exist.`
    : `Kleene Star: cannot find \`${name}\` on your PATH or in the usual install folders.`;
  const choice = await vscode.window.showErrorMessage(
    `${problem} Install Kleene Star (it includes ${name}), or set kstar.${setting} to its full path.`,
    "Open Settings",
    "How to Install"
  );
  if (choice === "Open Settings") {
    vscode.commands.executeCommand("workbench.action.openSettings", `kstar.${setting}`);
  } else if (choice === "How to Install") {
    const readme = vscode.Uri.file(path.join(__dirname, "README.md"));
    vscode.commands.executeCommand("markdown.showPreview", readme);
  }
}

async function startServer() {
  const command = find("kstar-lsp", "serverPath");
  if (!command) {
    await missing("kstar-lsp", "serverPath");
    return;
  }
  const server = { command, args: [] };
  client = new LanguageClient(
    "kstar",
    "Kleene Star",
    { run: server, debug: server },
    { documentSelector: [{ scheme: "file", language: "kstar" }, { scheme: "untitled", language: "kstar" }] }
  );
  try {
    await client.start();
  } catch (error) {
    client = undefined;
    vscode.window.showErrorMessage(`Kleene Star: ${command} did not start (${error.message}).`);
  }
}

/// The current `.kstar` file, saved, or `undefined` (with a message).
async function currentFile() {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "kstar") {
    vscode.window.showWarningMessage("Kleene Star: open a .kstar file first.");
    return undefined;
  }
  if (editor.document.isUntitled) {
    vscode.window.showWarningMessage("Kleene Star: save the file first.");
    return undefined;
  }
  if (editor.document.isDirty) {
    await editor.document.save();
  }
  return editor.document;
}

/// Runs `kstar` with `args` in the folder of `document`, in a terminal or
/// the Kleene Star output panel (the `kstar.runIn` setting). Arguments are
/// passed straight to the program, so names and words need no quoting.
async function runKstar(document, args) {
  const kstar = find("kstar", "path");
  if (!kstar) {
    await missing("kstar", "path");
    return;
  }
  const file = document.uri.fsPath;
  if (vscode.workspace.getConfiguration("kstar").get("runIn") !== "output") {
    runInTerminal(kstar, args, path.dirname(file));
    return;
  }
  output.clear();
  output.show(true);
  output.appendLine(`> kstar ${args.join(" ")}`);
  execFile(
    kstar,
    [args[0], "--color", "never", ...args.slice(1)],
    { cwd: path.dirname(file), maxBuffer: 64 * 1024 * 1024, windowsHide: true },
    (error, stdout, stderr) => {
      if (stdout) output.append(stdout);
      if (stderr) output.append(stderr);
      const code = error ? (typeof error.code === "number" ? error.code : 1) : 0;
      output.appendLine(code === 0 ? "\n[done]" : `\n[exit code ${code}]`);
    }
  );
}

/// Runs `kstar` in a terminal of its own, the way it runs on the command
/// line, with colours. The terminal starts the program itself rather than a
/// shell, so no quoting is needed on any platform; it stays open afterwards
/// to show the output, and is replaced by the next run.
function runInTerminal(kstar, args, cwd) {
  if (terminal) terminal.dispose();
  terminal = vscode.window.createTerminal({
    name: "Kleene Star",
    shellPath: kstar,
    shellArgs: args,
    cwd,
    iconPath: new vscode.ThemeIcon("play"),
  });
  terminal.show(true);
}

/// The machines defined in `document`, from the language server's outline.
async function machines(document) {
  try {
    const symbols = await vscode.commands.executeCommand("vscode.executeDocumentSymbolProvider", document.uri);
    return (symbols || []).filter((s) => s.kind === vscode.SymbolKind.Class).map((s) => s.name);
  } catch {
    return [];
  }
}

async function trace() {
  const document = await currentFile();
  if (!document) return;
  const names = await machines(document);
  const other = "Another expression...";
  let machine = names.length
    ? await vscode.window.showQuickPick([...names, other], { placeHolder: "Which machine?" })
    : other;
  if (machine === undefined) return;
  if (machine === other) {
    machine = await vscode.window.showInputBox({
      prompt: "The machine to trace: a name, or an expression such as to_dfa(r)",
    });
    if (!machine) return;
  }
  const word = await vscode.window.showInputBox({
    prompt: `The word to run ${machine} on (leave empty for the empty word)`,
    placeHolder: "0101",
  });
  if (word === undefined) return;
  await runKstar(document, ["trace", path.basename(document.uri.fsPath), machine, word]);
}

function activate(context) {
  output = vscode.window.createOutputChannel("Kleene Star");
  const file = async (args) => {
    const document = await currentFile();
    if (document) await runKstar(document, [...args, path.basename(document.uri.fsPath)]);
  };
  context.subscriptions.push(
    output,
    vscode.commands.registerCommand("kstar.run", () => file(["run"])),
    vscode.commands.registerCommand("kstar.check", () => file(["check"])),
    vscode.commands.registerCommand("kstar.trace", trace),
    vscode.commands.registerCommand("kstar.format", () =>
      vscode.commands.executeCommand("editor.action.formatDocument")
    ),
    vscode.commands.registerCommand("kstar.restartServer", async () => {
      if (client) await client.stop();
      client = undefined;
      await startServer();
    }),
    vscode.window.onDidCloseTerminal((closed) => {
      if (closed === terminal) terminal = undefined;
    }),
    { dispose: () => terminal && terminal.dispose() },
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (event.affectsConfiguration("kstar.serverPath")) {
        await vscode.commands.executeCommand("kstar.restartServer");
      }
    }),
    { dispose: () => client && client.stop() }
  );
  return startServer();
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
