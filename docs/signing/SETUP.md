# Setting up code signing with SignPath

What to do once SignPath Foundation has accepted Kleene Star for free code
signing. Until then nothing here is needed: the release workflow skips
signing when the `SIGNPATH_API_TOKEN` secret is missing and releases
unsigned programs, as before. [CODE_SIGNING_POLICY.md](../../CODE_SIGNING_POLICY.md)
is the policy SignPath asks for; keep it true.

The workflow side is done: the `windows-programs` and `installer` jobs of
[release.yml](../../.github/workflows/release.yml) send their files to
SignPath and use the signed ones. What is left is on SignPath's and
GitHub's websites, and only you can do it.

## 1. Turn on two-factor sign-in

SignPath Foundation requires it for everyone who can change the code or
approve signing:

- **GitHub:** *Settings > Password and authentication > Two-factor
  authentication*.
- **SignPath:** in your SignPath user settings, turn on two-factor
  sign-in (or sign in through GitHub with two-factor turned on there).

## 2. Create the project in SignPath

In the SignPath organization SignPath Foundation set up for you:

1. **Connect GitHub.** Add the GitHub.com trusted build system to the
   organization if it is not there yet, and link the project to the
   repository `Pratham71/kleene-star`. Only artifacts built by this
   repository's GitHub Actions can then be signed.
2. **Create a project** with the slug `kleene-star`.
3. **Add two artifact configurations** to the project, by pasting:
   - [programs.xml](programs.xml) with the slug `programs`;
   - [installer.xml](installer.xml) with the slug `installer`.

   The workflow asks for them by these slugs. Each takes a `version`
   parameter, which the workflow fills in from `Cargo.toml`.
4. **Create a signing policy** with the slug `release-signing`, using the
   certificate SignPath Foundation provides, with:
   - *Approval*: manual, with yourself as the approver (every request waits
     for you);
   - the GitHub trusted build system as the required origin.
5. **Create an API token** for a CI user that may submit signing requests
   to this project (*Users > New CI user*, then give it the *Submitter*
   role on the project). Copy the token; SignPath shows it once.

## 3. Add the settings to GitHub

In the repository's *Settings > Secrets and variables > Actions*:

| Kind | Name | Value |
|---|---|---|
| Variable | `SIGNPATH_ORGANIZATION_ID` | Your SignPath organization's ID (a GUID, on its settings page) |
| Variable | `SIGNPATH_PROJECT_SLUG` | `kleene-star` |
| Variable | `SIGNPATH_SIGNING_POLICY_SLUG` | `release-signing` |
| Secret | `SIGNPATH_API_TOKEN` | The CI user's API token |

Signing starts with the next release once the secret exists. To switch it
off again, delete the secret.

## 4. Make a test release

1. In the Actions tab, run the **Release** workflow by hand on `main`
   (*Run workflow*). That is a dry run: it signs but publishes nothing.
2. The `Windows programs` job stops at *sign kstar.exe and kstar-lsp.exe*
   and logs the request's link. Open it in SignPath, check that it comes
   from `main` of this repository, and approve it. The `Windows installer`
   job then asks for the installer the same way; approve that too.
   Each job waits up to about 5 hours 45 minutes for your approval (GitHub
   stops jobs after 6 hours); if it times out, *Re-run failed jobs*.
3. Download the `dry-run-release` artifact and check a file: in Windows,
   *Properties > Digital Signatures* of `kstar.exe` shows the SignPath
   Foundation certificate, or in PowerShell
   `Get-AuthenticodeSignature kstar.exe` says `Valid`.
4. Optionally, now restrict the signing policy's origin to version tags
   (`refs/tags/v*`), so only real releases can be signed (a later dry run
   from `main` is then refused, which is the point).
5. Then make the next real release as usual (CONTRIBUTING.md, *Making a
   release*), approving its two requests as they come.

If a request is refused because a file's product name or version does not
match, compare `installer/check-version-info.ps1`'s line in the job's log
with the artifact configuration.
