; Inno Setup script for mdedit (one statically linked exe, no DLLs). Modelled on mdview.iss of the
; Markdown viewer (Kaan88/markdown-viewer-rs, packaging\windows): the same [Setup] choices, wizard
; pages, task wording and registry layout; only the names, the AppId, the exe and the extensions
; differ, and there is one more task (the "Edit with mdedit" context-menu entry).
; build-installer.ps1 (or tools\release.ps1) builds the release exe first; this script reads it
; from target\release.
; Build:  ISCC.exe /DAppVersion=0.3.1 mdedit.iss   (see build-installer.ps1)

#ifndef AppVersion
  #define AppVersion "0.3.1"
#endif
; Numeric a.b.c.0 for the exe version resource (strips any -prerelease suffix).
#define Dash Pos("-", AppVersion)
#if Dash > 0
  #define NumVer Copy(AppVersion, 1, Dash - 1)
#else
  #define NumVer AppVersion
#endif

#define AppName "mdedit"
#define DisplayName "mdedit"
#define AppExe "mdedit.exe"
#define ProgId "mdedit.TextFile"

; The extensions mdedit is offered for (text, data and the source files it colours), without the
; dot, each followed by a comma. Registered as "Open with" and in Default apps; none becomes a
; default by itself.
#define Exts "txt,log,ini,cfg,conf,env,properties,json,xml,yaml,yml,toml,csv,md,markdown,ts,tsx,mts,cts,js,jsx,mjs,cjs,c,h,cc,cpp,hpp,cs,java,go,py,rb,php,rs,kt,kts,swift,sql,sh,bash,zsh,ps1,psm1,bat,cmd,html,htm,css,scss,less,dart,zig,lua,pl,r,diff,patch,cmake,gradle,groovy,scala,hs,tex,"
#define Rest ""
#define Ext ""
#define N 0
#expr Rest = Exts
; Takes the next extension off Rest into Ext.
#sub NextExt
  #expr Ext = Copy(Rest, 1, Pos(",", Rest) - 1)
  #expr Rest = Copy(Rest, Pos(",", Rest) + 1)
#endsub

[Setup]
AppId={{C58A9AB1-8337-4056-9953-04E92C05F414}
AppName={#DisplayName}
AppVersion={#AppVersion}
AppPublisher=Kaan88
AppPublisherURL=https://github.com/Kaan88/markdown-editor-rs
VersionInfoVersion={#NumVer}.0
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
; Per-user only: no elevation, and every registry key goes under HKCU (HKA resolves to it). The
; app's updater swaps the running exe in its own folder, which is writable here.
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
SetupIconFile=..\..\assets\mdedit.ico
UninstallDisplayIcon={app}\mdedit.ico
UninstallDisplayName={#DisplayName}
; Registers the associations and calls SHChangeNotify(SHCNE_ASSOCCHANGED) at the end.
ChangesAssociations=yes
; Close a running editor during upgrade or uninstall; do not relaunch it.
CloseApplications=yes
RestartApplications=no
OutputDir=..\..\target\installer
OutputBaseFilename=mdedit-setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
; Checked by default. Windows does not let an installer take the default handler for an extension
; (the per-user choice is protected), so mdedit registers as a handler and as an application in
; Settings > Default apps; the last page offers to open that page so the user can pick it.
Name: "assoc"; Description: "Register mdedit for text and source files (.txt, .log, .ini, .json, .md, .rs, .ts, .py and more)"; GroupDescription: "File types:"
Name: "contextmenu"; Description: "Add ""Edit with mdedit"" to the context menu of all files"; GroupDescription: "File types:"; Flags: unchecked
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "..\..\target\release\mdedit.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\assets\mdedit.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#DisplayName}"; Filename: "{app}\{#AppExe}"; IconFilename: "{app}\mdedit.ico"
Name: "{autodesktop}\{#DisplayName}"; Filename: "{app}\{#AppExe}"; IconFilename: "{app}\mdedit.ico"; Tasks: desktopicon

[Registry]
; mdedit is only OFFERED under "Open with". Nothing here writes an extension's default value
; (the "(Default)" of HKCU\Software\Classes\.txt etc.), so the user's current choice is untouched.
Root: HKA; Subkey: "Software\Classes\{#ProgId}"; ValueType: string; ValueName: ""; ValueData: "Text document"; Flags: uninsdeletekey; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\{#ProgId}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\mdedit.ico"; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\{#ProgId}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: assoc

Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#DisplayName}"; Flags: uninsdeletekey; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: assoc

; Registered application: lists mdedit in Settings > Apps > Default apps, where the user can make it
; the default for each extension.
Root: HKA; Subkey: "Software\{#AppName}"; Flags: uninsdeletekey; Tasks: assoc
Root: HKA; Subkey: "Software\{#AppName}\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#DisplayName}"; Tasks: assoc
Root: HKA; Subkey: "Software\{#AppName}\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "A light, fast text editor."; Tasks: assoc
Root: HKA; Subkey: "Software\{#AppName}\Capabilities"; ValueType: string; ValueName: "ApplicationIcon"; ValueData: "{app}\mdedit.ico"; Tasks: assoc
Root: HKA; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "{#AppName}"; ValueData: "Software\{#AppName}\Capabilities"; Flags: uninsdeletevalue; Tasks: assoc

; Takes the next extension and writes its entries.
#sub Each
#emit NextExt
#emit ExtEntries
#endsub

; One group of four entries per extension: "Open with", the application's supported types, and
; the capabilities of the registered application.
#sub ExtEntries
Root: HKA; Subkey: "Software\Classes\.{#Ext}"; Flags: uninsdeletekeyifempty; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.{#Ext}\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".{#Ext}"; ValueData: ""; Tasks: assoc
Root: HKA; Subkey: "Software\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".{#Ext}"; ValueData: "{#ProgId}"; Tasks: assoc
#endsub
#for {N = 0; Rest != ""; N = N + 1} Each

; The optional context-menu entry for every file.
Root: HKA; Subkey: "Software\Classes\*\shell\{#AppName}"; ValueType: string; ValueName: ""; ValueData: "Edit with {#DisplayName}"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\{#AppName}"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\mdedit.ico"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\{#AppName}\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: contextmenu

[Run]
Filename: "{app}\{#AppExe}"; Description: "Launch mdedit"; Flags: nowait postinstall skipifsilent
