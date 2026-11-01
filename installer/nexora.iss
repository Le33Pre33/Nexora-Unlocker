; Скрипт установщика Nexora Unlocker (Inno Setup 6).
; Версия передаётся из package.bat: ISCC /DAppVersion=1.2.3 nexora.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{B7A1C2E4-5D3F-4A8B-9C61-2F0E7D9A4B13}
AppName=Nexora Unlocker
AppVersion={#AppVersion}
AppPublisher=Nexora
DefaultDirName={autopf}\Nexora Unlocker
DefaultGroupName=Nexora Unlocker
DisableProgramGroupPage=yes
DisableDirPage=no
; По умолчанию установка для текущего пользователя (без UAC); в диалоге можно выбрать для всех.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputBaseFilename=NexoraUnlocker-Setup-{#AppVersion}
SetupIconFile=..\assets\nexora.ico
UninstallDisplayIcon={app}\nexora-unlocker.exe
UninstallDisplayName=Nexora Unlocker
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
#if FileExists(CompilerPath + "Languages\Russian.isl")
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
#else
Name: "english"; MessagesFile: "compiler:Default.isl"
#endif

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "..\target\release\nexora-unlocker.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Nexora Unlocker"; Filename: "{app}\nexora-unlocker.exe"
Name: "{autodesktop}\Nexora Unlocker"; Filename: "{app}\nexora-unlocker.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\nexora-unlocker.exe"; Description: "{cm:LaunchProgram,Nexora Unlocker}"; Flags: nowait postinstall skipifsilent
