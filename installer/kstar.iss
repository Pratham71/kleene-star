; The Windows installer, kstar-setup-x64.exe, built with Inno Setup 6 by
; the release workflow:
;
;   iscc /DAppVersion=0.2.0 /DSourceDir=<folder with kstar.exe> /O<output> installer\kstar.iss
;
; It installs kstar.exe and kstar-lsp.exe into "<install folder>\bin", by
; default %LOCALAPPDATA%\Programs\Kleene Star (or Program Files when run as
; administrator), optionally adds that folder to the PATH, and associates
; .kstar files with the K* icon. The uninstaller removes all three.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\target\release"
#endif

[Setup]
AppId={{6A0C3E57-2B0D-4C11-9E55-4B1D2A7F0C21}
AppName=Kleene Star
AppVersion={#AppVersion}
AppVerName=Kleene Star {#AppVersion}
AppPublisher=Kleene Star
DefaultDirName={autopf}\Kleene Star
DefaultGroupName=Kleene Star
DisableProgramGroupPage=yes
; Per user by default, so no administrator rights are needed; the first page
; offers "for all users" too, and /CURRENTUSER or /ALLUSERS choose silently.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline dialog
LicenseFile=..\LICENSE
OutputBaseFilename=kstar-setup-x64
SetupIconFile=..\assets\kstar.ico
UninstallDisplayIcon={app}\bin\kstar.exe
UninstallDisplayName=Kleene Star {#AppVersion}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesEnvironment=yes
ChangesAssociations=yes
WizardStyle=modern
Compression=lzma2
SolidCompression=yes

[Tasks]
Name: addtopath; Description: "Add Kleene Star to the PATH, so ""kstar"" works in any terminal"
Name: associate; Description: "Associate .kstar files with Kleene Star (and show the K* icon)"

[Files]
Source: "{#SourceDir}\kstar.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#SourceDir}\kstar-lsp.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "..\assets\kstar.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{group}\Kleene Star REPL"; Filename: "{app}\bin\kstar.exe"; Parameters: "repl"; IconFilename: "{app}\kstar.ico"; Comment: "Try Kleene Star interactively"
Name: "{group}\Kleene Star README"; Filename: "{app}\README.md"
Name: "{group}\Uninstall Kleene Star"; Filename: "{uninstallexe}"

[Registry]
; HKA is HKEY_CURRENT_USER for a per-user install and HKEY_LOCAL_MACHINE for
; an install for all users.
Root: HKA; Subkey: "Software\Classes\.kstar"; ValueType: string; ValueName: ""; ValueData: "KleeneStar.Program"; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\KleeneStar.Program"; ValueType: string; ValueName: ""; ValueData: "Kleene Star program"; Flags: uninsdeletekey; Tasks: associate
Root: HKA; Subkey: "Software\Classes\KleeneStar.Program\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\kstar.ico"; Tasks: associate
; Opening a .kstar file runs it in a console window that stays open.
Root: HKA; Subkey: "Software\Classes\KleeneStar.Program\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{cmd}"" /k """"{app}\bin\kstar.exe"" run ""%1"""""; Tasks: associate

[Code]
const
  UserEnvironment = 'Environment';
  SystemEnvironment = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';

function EnvironmentRoot: Integer;
begin
  if IsAdminInstallMode then Result := HKEY_LOCAL_MACHINE else Result := HKEY_CURRENT_USER;
end;

function EnvironmentKey: String;
begin
  if IsAdminInstallMode then Result := SystemEnvironment else Result := UserEnvironment;
end;

function BinDir: String;
begin
  Result := ExpandConstant('{app}\bin');
end;

{ The PATH with every entry equal to BinDir (ignoring case and a trailing
  backslash) left out. }
function PathWithout(Paths: String): String;
var
  Rest, Entry: String;
  P: Integer;
begin
  Result := '';
  Rest := Paths;
  while Rest <> '' do
  begin
    P := Pos(';', Rest);
    if P = 0 then
    begin
      Entry := Rest;
      Rest := '';
    end else begin
      Entry := Copy(Rest, 1, P - 1);
      Delete(Rest, 1, P);
    end;
    if (Entry <> '') and (CompareText(RemoveBackslashUnlessRoot(Entry), RemoveBackslashUnlessRoot(BinDir)) <> 0) then
    begin
      if Result <> '' then Result := Result + ';';
      Result := Result + Entry;
    end;
  end;
end;

procedure AddToPath;
var
  Paths: String;
begin
  if not RegQueryStringValue(EnvironmentRoot, EnvironmentKey, 'Path', Paths) then Paths := '';
  Paths := PathWithout(Paths);
  if Paths <> '' then Paths := Paths + ';';
  RegWriteExpandStringValue(EnvironmentRoot, EnvironmentKey, 'Path', Paths + BinDir);
end;

procedure RemoveFromPath;
var
  Paths: String;
begin
  if RegQueryStringValue(EnvironmentRoot, EnvironmentKey, 'Path', Paths) then
    RegWriteExpandStringValue(EnvironmentRoot, EnvironmentKey, 'Path', PathWithout(Paths));
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if WizardIsTaskSelected('addtopath') then AddToPath else RemoveFromPath;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then RemoveFromPath;
end;
