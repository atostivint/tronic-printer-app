; Inno Setup script. The release workflow builds it with: iscc /DAppVersion=0.2.0 installer\tronic-printer.iss
; The result is installer\Output\TronicPrinter-Setup-<version>.exe

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
; Never change AppId: it is how a new version finds and replaces the old one.
AppId={{6B0F3C1E-5A7D-4E2B-9C48-7D1A2F5E8B31}
AppName=Tronic Printer
AppVersion={#AppVersion}
AppPublisher=Tronic Printer
DefaultDirName={autopf}\Tronic Printer
DefaultGroupName=Tronic Printer
UninstallDisplayIcon={app}\tronic-printer.exe
SetupIconFile=..\assets\icon.ico
OutputDir=Output
OutputBaseFilename=TronicPrinter-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Per-user by default (no admin prompt); the user can choose "for all users".
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
; Update in place: close the running app first, and start it again afterwards.
CloseApplications=yes
RestartApplications=no
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "..\target\release\tronic-printer.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\tronic-print.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\tronic-mcp.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Tronic Printer"; Filename: "{app}\tronic-printer.exe"
Name: "{autodesktop}\Tronic Printer"; Filename: "{app}\tronic-printer.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\tronic-printer.exe"; Description: "Start Tronic Printer"; Flags: nowait postinstall skipifsilent
