# Security policy

## Supported versions

Only the latest release of Kleene Star gets security fixes.

## Reporting a vulnerability

Please do not report security problems in a public issue. Instead, open a
private report: on the repository's **Security** tab choose **Report a
vulnerability** (GitHub's private vulnerability reporting). Only the
maintainer can see it.

Useful things to include: what the problem is, the smallest program, file
or command that shows it, what an attacker could do with it, and the
output of `kstar version`.

You should get a first answer within a week. Once a fix is released, the
report can be published as a security advisory, crediting you if you wish.

## What counts

Kleene Star runs programs that you write or choose to run, and its
programs cannot read files, use the network or run other programs (`draw`
only writes Graphviz files where you ask it to). Problems worth reporting
include, for example:

- a program or input that makes `kstar` or `kstar-lsp` write outside the
  folder it was asked to, or run something it should not;
- a way for the installer or the install scripts to install something
  other than the published, checksummed release;
- anything in the release workflow that could let someone else change a
  published release.

A program that runs for a long time or uses a lot of memory is usually a
bug rather than a vulnerability (there are limits on searches and
recursion); report it as a normal issue unless it bypasses those limits in
a way that matters, such as hanging the language server in an editor.
