param([string]$ConfigPath)
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false)
$cfg=Get-Content -LiteralPath $ConfigPath -Raw -Encoding UTF8 | ConvertFrom-Json
if($cfg.action -eq 'detect'){
 @{word=($null -ne [Type]::GetTypeFromProgID('Word.Application'));excel=($null -ne [Type]::GetTypeFromProgID('Excel.Application'))}|ConvertTo-Json -Compress
 exit
}
$app=$null
$document=$null
$owned=$false
$processName=if([IO.Path]::GetExtension($cfg.input).ToLowerInvariant() -eq '.docx'){'WINWORD'}else{'EXCEL'}
$existing=@(Get-Process -Name $processName -ErrorAction SilentlyContinue | ForEach-Object {$_.Id})
Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class PdfToolkitWindow { [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid); }'
function Register-OfficeProcess {
 param([IntPtr]$windowHandle)
 [uint32]$officeId=0
 [void][PdfToolkitWindow]::GetWindowThreadProcessId($windowHandle,[ref]$officeId)
 $process=Get-Process -Id $officeId
 $script:owned=($existing -notcontains $officeId) -and ($process.ProcessName -eq $processName)
 if(-not $script:owned){throw '既存のOfficeとは別の変換プロセスを開始できませんでした。'}
 if($cfg.pidFile){@{id=$process.Id;ticks=$process.StartTime.ToUniversalTime().Ticks;name=$processName}|ConvertTo-Json -Compress | Set-Content -LiteralPath $cfg.pidFile -Encoding UTF8}
}
try{
 if([IO.Path]::GetExtension($cfg.input).ToLowerInvariant() -eq '.docx'){
  $app=New-Object -ComObject Word.Application
  $document=$app.Documents.Add()
  Register-OfficeProcess ([IntPtr]$document.ActiveWindow.Hwnd)
  $document.Close($false)
  [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($document)
  $document=$null
  $app.Visible=$false
  $app.DisplayAlerts=0
  $app.AutomationSecurity=3
  $document=$app.Documents.Open([string]$cfg.input,$false,$true,$false)
  $document.ExportAsFixedFormat([string]$cfg.output,17)
 }else{
  $app=New-Object -ComObject Excel.Application
  Register-OfficeProcess ([IntPtr]$app.Hwnd)
  $app.Visible=$false
  $app.DisplayAlerts=$false
  $app.AskToUpdateLinks=$false
  $app.AutomationSecurity=3
  $document=$app.Workbooks.Open([string]$cfg.input,0,$true)
  if($cfg.action -eq 'sheets'){
   $names=@();foreach($sheet in $document.Worksheets){$names+=@{name=$sheet.Name;visible=($sheet.Visible -eq -1)}}
   ConvertTo-Json -InputObject @($names) -Compress
  }else{
   if($cfg.sheets.Count -gt 0){
    $first=$true
    foreach($name in $cfg.sheets){$document.Worksheets.Item([string]$name).Select($first);$first=$false}
    $app.ActiveSheet.ExportAsFixedFormat(0,[string]$cfg.output)
   }else{$document.ExportAsFixedFormat(0,[string]$cfg.output)}
  }
 }
}finally{
 if($null -ne $document){$document.Close($false);[void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($document)}
 if($null -ne $app){if($owned){$app.Quit()};[void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($app)}
 [GC]::Collect();[GC]::WaitForPendingFinalizers()
}


