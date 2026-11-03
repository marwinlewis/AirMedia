; ==============================================================================
; AirMedia - Inno Setup Script
; Production-grade installer avoiding antivirus false positives:
; - Standard PE installer structure trusted by Windows Defender
; - Per-user installation mode (no admin UAC elevation prompt required)
; - Clean uninstaller registered in Windows Add/Remove Programs
; - Shell file associations registered cleanly in HKCU
; ==============================================================================

#ifndef MyAppName
#define MyAppName "AirMedia"
#endif

#ifndef MyAppVersion
#define MyAppVersion "0.1.0"
#endif

#ifndef MyAppPublisher
#define MyAppPublisher "AirMedia Open Source Project"
#endif

#ifndef MyAppURL
#define MyAppURL "https://github.com/marwinlewis/airmedia"
#endif

#define MyAppExeName "airmedia.exe"

[Setup]
AppId={{5E4C2048-A8A1-4F23-BBE0-5FCE0A1BA47F}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} v{#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=airmedia-v{#MyAppVersion}-windows-x64-setup
SetupIconFile=..\assets\windows\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
; Per-user install only. Allowing an "all users" override makes Setup show an
; install-mode dialog and relaunch itself elevated, which looks like the
; installer starting a second time.
PrivilegesRequired=lowest
; Refuse to start a second Setup while one is already running (e.g. repeated
; double-clicks while antivirus is still scanning the first one).
SetupMutex=AirMediaSetupMutex,Global\AirMediaSetupMutex
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=no
; Tell Explorer to refresh icons/associations so Start search picks up the app icon.
ChangesAssociations=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "associate_video"; Description: "Associate AirMedia with common video files (.mp4, .mkv, .webm)"; GroupDescription: "File Associations:"

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\icon.png"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\{#MyAppExeName}"; IconIndex: 0; AppUserModelID: "AirMedia.AirMedia"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\{#MyAppExeName}"; IconIndex: 0; Tasks: desktopicon

[Registry]
; File associations in HKCU (safe, no admin required, trusted by AV)
Root: HKCU; Subkey: "Software\Classes\.mp4\OpenWithProgids"; ValueType: string; ValueName: "AirMedia.Video"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate_video
Root: HKCU; Subkey: "Software\Classes\.mkv\OpenWithProgids"; ValueType: string; ValueName: "AirMedia.Video"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate_video
Root: HKCU; Subkey: "Software\Classes\.webm\OpenWithProgids"; ValueType: string; ValueName: "AirMedia.Video"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate_video
Root: HKCU; Subkey: "Software\Classes\AirMedia.Video"; ValueType: string; ValueName: ""; ValueData: "AirMedia Video File"; Flags: uninsdeletekey; Tasks: associate_video
Root: HKCU; Subkey: "Software\Classes\AirMedia.Video\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"; Tasks: associate_video
Root: HKCU; Subkey: "Software\Classes\AirMedia.Video\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Tasks: associate_video

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
