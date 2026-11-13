#ifndef Stage
  #error Stage must identify the prepared application directory
#endif
#ifndef Output
  #error Output must identify the installer output directory
#endif
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
[Setup]
AppId={{A97E0F22-5E88-47AA-B726-A10A80CA2171}
AppName=uFFmpeg
AppVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\uFFmpeg
DefaultGroupName=uFFmpeg
PrivilegesRequired=lowest
MinVersion=10.0.19041
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#Output}
OutputBaseFilename=uFFmpeg-{#AppVersion}-windows-x64
SetupIconFile={#Stage}\uffmpeg.ico
UninstallDisplayIcon={app}\uffmpeg.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
#ifdef Signed
SignTool=release
SignedUninstaller=yes
#endif
[Files]
Source: "{#Stage}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion
[Icons]
Name: "{group}\uFFmpeg"; Filename: "{app}\uffmpeg.exe"
Name: "{group}\uFFmpeg licenses"; Filename: "{app}\THIRD-PARTY.txt"
[Run]
Filename: "{app}\uffmpeg.exe"; Description: "Open uFFmpeg"; Flags: postinstall nowait skipifsilent
