; The Windows installer dressed as the splash: art.py's pictures behind the welcome and the finish, dimmed between.
; iscc /DAppVersion=0.1.0 /DArt=<art.py's folder> /DExe=<oni.exe> /DOutput=<folder> /DTag=<name suffix> oni.iss

#if Ver < EncodeVer(6, 7, 0)
  #error Inno Setup 6.7 or later draws the wizard's background picture
#endif
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef Tag
  #define Tag AppVersion
#endif
#ifndef Output
  #define Output "out"
#endif
#define AppName "Open Negative Initiative"

[Setup]
; One identity across versions, so an install upgrades the last one in place.
AppId={{829E4D84-996E-4FC2-A4CE-71A52D4CDE4B}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppName}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableWelcomePage=no
; A tester installs without an administrator; the dialog offers every user's Program Files.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#Output}
OutputBaseFilename=ONI-setup-{#Tag}
SetupIconFile=oni.ico
UninstallDisplayIcon={app}\ONI.exe
UninstallDisplayName={#AppName}
Compression=lzma2/max
SolidCompression=yes
CloseApplications=yes
WizardStyle=modern dark
WizardBackColor=#1b1b1b
WizardBackImageFile={#Art}\splash-596.png,{#Art}\splash-894.png,{#Art}\splash-1272.png,{#Art}\splash-1630.png
WizardImageFile=
WizardSmallImageFile={#Art}\small-55.png,{#Art}\small-83.png,{#Art}\small-110.png,{#Art}\small-138.png
ShowLanguageDialog=auto

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Exe}"; DestDir: "{app}"; DestName: "ONI.exe"; Flags: ignoreversion
Source: "{#Art}\splash-*.png"; Flags: dontcopy noencryption
Source: "{#Art}\page-*.png"; Flags: dontcopy noencryption

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\ONI.exe"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\ONI.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\ONI.exe"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[Code]
const
  Widths = '596,894,1272,1630';

// The pictures of one kind at every size, for Setup to pick the one that fits the window.
function Pictures(Kind: String): TArrayOfGraphic;
var
  Rest, Name: String;
  I, Comma: Integer;
begin
  Rest := Widths;
  I := 0;
  while Rest <> '' do begin
    Comma := Pos(',', Rest);
    if Comma = 0 then Comma := Length(Rest) + 1;
    Name := Kind + '-' + Copy(Rest, 1, Comma - 1) + '.png';
    Delete(Rest, 1, Comma);
    ExtractTemporaryFile(Name);
    SetArrayLength(Result, I + 1);
    Result[I] := TPngImage.Create;
    Result[I].LoadFromFile(ExpandConstant('{tmp}\') + Name);
    I := I + 1;
  end;
end;

var
  Splash, Dimmed: TArrayOfGraphic;
  Shown: Boolean;

// Every picture extracted and loaded once, at the wizard's start, as the pages swap them often.
procedure InitializeWizard;
begin
  try
    Splash := Pictures('splash');
    Dimmed := Pictures('page');
  except
    Log('No background pictures: ' + GetExceptionMessage);
  end;
end;

// The splash greets and closes; the pages between keep the photograph dimmed under their text.
procedure CurPageChanged(CurPageID: Integer);
var
  Wanted: Boolean;
begin
  Wanted := (CurPageID = wpWelcome) or (CurPageID = wpFinished);
  if (Wanted = Shown) or (GetArrayLength(Splash) = 0) or (GetArrayLength(Dimmed) = 0) then Exit;
  if Wanted then
    WizardSetBackImage(Splash, True, True, 255)
  else
    WizardSetBackImage(Dimmed, True, True, 255);
  Shown := Wanted;
end;
