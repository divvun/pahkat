#define MyAppName "Pahkat Service"
#define MyAppPublisher "Universitetet i Tromsø - Norges arktiske universitet"
#define MyAppURL "http://divvun.no"
#define PahkatSvcExe "pahkat-service.exe"  
#define PahkatClientExe "pahkatc.exe"

[Setup]
AppId={{6B3A048B-BB81-4865-86CA-61A0DF038CFE}
AppName={#MyAppName}
AppVersion={#Version}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DisableProgramGroupPage=yes
OutputBaseFilename=install
Compression=lzma
SolidCompression=yes      
DefaultDirName={commonpf}\Pahkat Service
SignedUninstaller=yes
SignTool=signtool
MinVersion=6.3.9200                 

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
// Must be run from the root directory of the pahkat project
Source: "..\..\dist\{#PahkatSvcExe}"; DestDir: "{app}"
Source: "..\..\dist\{#PahkatClientExe}"; DestDir: "{app}"

[Run]
Filename: "{app}\{#PahkatSvcExe}"; Parameters: "service install"; StatusMsg: "Installing service..."; Flags: runhidden

[UninstallRun]
Filename: "{app}\{#PahkatSvcExe}"; Parameters: "service stop"; Flags: runhidden; StatusMsg: "Stopping service..."
Filename: "{app}\{#PahkatSvcExe}"; Parameters: "service uninstall"; Flags: runhidden; StatusMsg: "Uninstalling service..."

[Code]
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
    // Stop the service
    ExtractTemporaryFile('{#PahkatSvcExe}');
    Exec(ExpandConstant('{tmp}\{#PahkatSvcExe}'), 'service stop', '', SW_HIDE, ewWaitUntilTerminated, ResultCode)
end;
