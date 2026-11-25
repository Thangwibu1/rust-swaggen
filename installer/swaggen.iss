; Inno Setup 6 production installer script for Swaggen Desktop (Windows x64)
; Build with: iscc /DAppVersion=0.1.3 /DBundleDir=..\result\v0.1.3 installer\swaggen.iss

#ifndef AppVersion
  #define AppVersion "0.1.3"
#endif
#ifndef BundleDir
  #define BundleDir "..\result\v0.1.3"
#endif

[Setup]
AppId={{8D62C37A-5F41-4A1E-9B72-7C8E2F4910AB}
AppName=Swaggen
AppVersion={#AppVersion}
AppPublisher=Swaggen
AppPublisherURL=https://swaggen-api.ngocthang.io.vn
DefaultDirName={localappdata}\Swaggen
DefaultGroupName=Swaggen
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
OutputDir=..\dist\installer
OutputBaseFilename=Swaggen-Setup-{#AppVersion}-x64
UninstallDisplayIcon={app}\bin\swaggen-desktop.exe
ChangesEnvironment=yes
CloseApplications=force
RestartApplications=yes
UsePreviousAppDir=yes
UsePreviousGroup=yes
UsePreviousTasks=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &Desktop shortcut"; GroupDescription: "Additional shortcuts:"
Name: "addpath"; Description: "Add swaggen-cli to user PATH"; GroupDescription: "Developer tools:"

[Files]
Source: "{#BundleDir}\bin\swaggen-desktop.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#BundleDir}\bin\swaggen-cli.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#BundleDir}\config\*"; DestDir: "{app}\config"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#BundleDir}\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#BundleDir}\Swaggen.cmd"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BundleDir}\Swaggen-Portable.cmd"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BundleDir}\README.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Swaggen"; Filename: "{app}\bin\swaggen-desktop.exe"; WorkingDir: "{app}"
Name: "{group}\Uninstall Swaggen"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Swaggen"; Filename: "{app}\bin\swaggen-desktop.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}\bin"; Tasks: addpath; Check: NeedsAddPath(ExpandConstant('{app}\bin'))

[Run]
Filename: "{app}\bin\swaggen-desktop.exe"; Description: "Launch Swaggen"; Flags: nowait postinstall skipifsilent

[Code]
function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Uppercase(Param) + ';', ';' + Uppercase(OrigPath) + ';') = 0;
end;
