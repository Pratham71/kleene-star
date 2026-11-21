// The parts of the extension that do not need VS Code, kept here so the
// tests can run them with plain Node.

/// The types `kstar trace` can trace.
const TRACEABLE = new Set(["Dfa", "Nfa", "Pda", "Tm"]);

/// The names of the machines in an outline, for the Trace command: those
/// defined with `dfa`, `nfa`, `pda` or `tm` (shown as classes), and values
/// whose type is a machine, such as `let D = minimize(N)`. `classKind` and
/// `variableKind` are VS Code's numbers for those symbol kinds.
function traceable(symbols, classKind, variableKind) {
  return (symbols || [])
    .filter((s) => s.kind === classKind || (s.kind === variableKind && TRACEABLE.has(s.detail)))
    .map((s) => s.name);
}

/// `0.5.0` as [0, 5, 0]; anything after a `-` (a pre-release) is ignored.
function parseVersion(text) {
  const match = /^v?(\d+)\.(\d+)\.(\d+)/.exec(String(text || "").trim());
  return match ? match.slice(1, 4).map(Number) : undefined;
}

/// A warning when the language server and the extension come from
/// different releases, or `undefined` when they match (or a version is
/// unknown). Each release ships both, and a newer server can offer
/// language features an older extension does not highlight or know.
function versionMismatch(extensionVersion, serverVersion) {
  const ext = parseVersion(extensionVersion);
  const server = parseVersion(serverVersion);
  if (!ext || !server || ext.join(".") === server.join(".")) return undefined;
  const first = server.findIndex((n, i) => n !== ext[i]);
  const newer = server[first] > ext[first];
  const fix = newer
    ? `Update the extension to ${server.join(".")}: run \`kstar update\`, which installs the matching kleene-star-${server.join(".")}.vsix.`
    : `Update Kleene Star itself to ${ext.join(".")} (run \`kstar update\`), or install the extension of release ${server.join(".")}.`;
  return `Kleene Star: the language server is ${server.join(".")} but this extension is ${ext.join(".")}. ${fix}`;
}

module.exports = { traceable, parseVersion, versionMismatch };
